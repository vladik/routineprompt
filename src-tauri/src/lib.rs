use domain::{AssetKind, DisplayMode, MediaAsset, PendingAcks, Reminder, Schedule};
use scheduler::Scheduler;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewWindowBuilder, WebviewUrl};
use uuid::Uuid;

// ── App state ────────────────────────────────────────────────────────────────

pub struct AppState {
    pub db_path: PathBuf,
    pub assets_dir: PathBuf,
    pub shared: Arc<Mutex<Vec<Reminder>>>,
    pub pending: PendingAcks,
    pub popup_queue: Arc<Mutex<VecDeque<Reminder>>>,
    pub popup_active: Arc<AtomicBool>,
}

// ── Serde-friendly transfer types ────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ReminderDto {
    pub id: String,
    pub title: String,
    pub message: String,
    pub image_asset_ids: Vec<String>,
    pub sound_asset_id: Option<String>,
    pub sched_type: String,   // "daily" | "weekly" | "once"
    pub sched_time: String,   // "HH:MM" or RFC3339 for once
    pub sched_days: Option<String>, // "0,1,4" weekday indices
    pub display_mode: String, // "gentle" | "persistent" | "focus"
    pub enabled: bool,
    pub refire_after_mins: Option<u8>,
    pub refire_until_ack: bool,
    pub min_dismissal_mins: Option<u8>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AssetDto {
    pub id: String,
    pub name: String,
    pub kind: String, // "image" | "sound"
    pub filename: String,
    pub created_at: String,
}

#[derive(serde::Serialize, Clone)]
pub struct EventDto {
    pub title: String,
    pub fired_at: String,
    pub acknowledged_at: Option<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct PopupPayload {
    pub reminder: ReminderDto,
    pub event_id: Option<String>,
    pub more_pending: usize,
    pub assets_dir: String,
    pub assets: Vec<AssetDto>,
}

// ── Conversion helpers ────────────────────────────────────────────────────────

fn reminder_to_dto(r: &Reminder) -> ReminderDto {
    use chrono::Timelike;
    let (sched_type, sched_time, sched_days) = match &r.schedule {
        Schedule::Daily { at } => (
            "daily".into(),
            format!("{:02}:{:02}", at.hour(), at.minute()),
            None,
        ),
        Schedule::Weekly { days, at } => {
            let d = days.iter().map(|d| weekday_idx(*d).to_string()).collect::<Vec<_>>().join(",");
            ("weekly".into(), format!("{:02}:{:02}", at.hour(), at.minute()), Some(d))
        }
        Schedule::Once { at } => ("once".into(), at.to_rfc3339(), None),
    };
    ReminderDto {
        id: r.id.to_string(),
        title: r.title.clone(),
        message: r.message.clone(),
        image_asset_ids: r.image_asset_ids.iter().map(|u| u.to_string()).collect(),
        sound_asset_id: r.sound_asset_id.map(|u| u.to_string()),
        sched_type,
        sched_time,
        sched_days,
        display_mode: encode_display_mode(&r.display_mode).into(),
        enabled: r.enabled,
        refire_after_mins: r.refire_after_mins,
        refire_until_ack: r.refire_until_ack,
        min_dismissal_mins: r.min_dismissal_mins,
        created_at: r.created_at.to_rfc3339(),
        updated_at: r.updated_at.to_rfc3339(),
    }
}

fn dto_to_reminder(dto: &ReminderDto) -> anyhow::Result<Reminder> {
    use anyhow::Context;
    use chrono::{NaiveTime, DateTime, Utc};
    let parse_time = |s: &str| {
        NaiveTime::parse_from_str(s, "%H:%M")
            .with_context(|| format!("sched_time {s:?}"))
    };
    let parse_dt = |s: &str, field: &str| {
        s.parse::<DateTime<Utc>>()
            .or_else(|_| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .with_context(|| format!("{field} {s:?}"))
    };
    let schedule = match dto.sched_type.as_str() {
        "daily" => Schedule::Daily { at: parse_time(&dto.sched_time)? },
        "weekly" => {
            let days = dto.sched_days.as_deref().unwrap_or("").split(',')
                .filter_map(|n| n.parse::<u8>().ok())
                .filter_map(idx_weekday)
                .collect();
            Schedule::Weekly { days, at: parse_time(&dto.sched_time)? }
        }
        _ => Schedule::Once {
            at: parse_dt(&dto.sched_time, "sched_time (once)")?,
        },
    };
    Ok(Reminder {
        id: dto.id.parse().with_context(|| format!("id {:?}", dto.id))?,
        title: dto.title.clone(),
        message: dto.message.clone(),
        image_asset_ids: dto.image_asset_ids.iter().filter_map(|s| s.parse().ok()).collect(),
        sound_asset_id: dto.sound_asset_id.as_ref().and_then(|s| s.parse().ok()),
        schedule,
        display_mode: parse_display_mode(&dto.display_mode),
        enabled: dto.enabled,
        refire_after_mins: dto.refire_after_mins,
        refire_until_ack: dto.refire_until_ack,
        min_dismissal_mins: dto.min_dismissal_mins,
        created_at: parse_dt(&dto.created_at, "created_at")?,
        updated_at: parse_dt(&dto.updated_at, "updated_at")?,
    })
}

fn asset_to_dto(a: &MediaAsset) -> AssetDto {
    AssetDto {
        id: a.id.to_string(),
        name: a.name.clone(),
        kind: a.kind.as_str().into(),
        filename: a.filename.clone(),
        created_at: a.created_at.to_rfc3339(),
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
fn list_reminders(state: State<AppState>) -> Result<Vec<ReminderDto>, String> {
    let store = storage::ReminderStore::open(&state.db_path).map_err(|e| e.to_string())?;
    let reminders = store.list().map_err(|e| e.to_string())?;
    Ok(reminders.iter().map(reminder_to_dto).collect())
}

#[tauri::command]
fn save_reminder(dto: ReminderDto, state: State<AppState>) -> Result<(), String> {
    let reminder = dto_to_reminder(&dto).map_err(|e| e.to_string())?;
    let store = storage::ReminderStore::open(&state.db_path).map_err(|e| e.to_string())?;
    store.upsert(&reminder).map_err(|e| e.to_string())?;
    // Sync shared list for scheduler
    let updated = store.list().unwrap_or_default();
    *state.shared.lock().unwrap() = updated;
    Ok(())
}

#[tauri::command]
fn delete_reminder(id: String, state: State<AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let store = storage::ReminderStore::open(&state.db_path).map_err(|e| e.to_string())?;
    store.delete(uuid).map_err(|e| e.to_string())?;
    let updated = store.list().unwrap_or_default();
    *state.shared.lock().unwrap() = updated;
    Ok(())
}

#[tauri::command]
fn toggle_reminder(id: String, enabled: bool, state: State<AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let store = storage::ReminderStore::open(&state.db_path).map_err(|e| e.to_string())?;
    store.set_enabled(uuid, enabled).map_err(|e| e.to_string())?;
    let updated = store.list().unwrap_or_default();
    *state.shared.lock().unwrap() = updated;
    Ok(())
}

#[tauri::command]
fn list_assets(state: State<AppState>) -> Result<Vec<AssetDto>, String> {
    let store = storage::ReminderStore::open(&state.db_path).map_err(|e| e.to_string())?;
    let assets = store.list_assets().map_err(|e| e.to_string())?;
    Ok(assets.iter().map(asset_to_dto).collect())
}

#[tauri::command]
fn delete_asset(id: String, state: State<AppState>) -> Result<usize, String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let store = storage::ReminderStore::open(&state.db_path).map_err(|e| e.to_string())?;
    if let Ok(assets) = store.list_assets() {
        if let Some(asset) = assets.iter().find(|a| a.id == uuid) {
            let _ = std::fs::remove_file(state.assets_dir.join(&asset.filename));
        }
    }
    store.delete_asset(uuid).map_err(|e| e.to_string())?;

    // Remove the deleted asset from any reminders that reference it
    let reminders = store.list().map_err(|e| e.to_string())?;
    let mut affected = 0usize;
    for mut r in reminders {
        let had_image = r.image_asset_ids.contains(&uuid);
        let had_sound = r.sound_asset_id == Some(uuid);
        if had_image || had_sound {
            r.image_asset_ids.retain(|&x| x != uuid);
            if had_sound { r.sound_asset_id = None; }
            r.updated_at = chrono::Utc::now();
            let _ = store.upsert(&r);
            affected += 1;
        }
    }
    if affected > 0 {
        let updated = store.list().unwrap_or_default();
        *state.shared.lock().unwrap() = updated;
    }
    Ok(affected)
}

#[tauri::command]
async fn upload_asset(
    path: String,
    kind: String,
    state: State<'_, AppState>,
) -> Result<AssetDto, String> {
    let src = PathBuf::from(&path);
    let ext = src.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_lowercase();
    let asset_id = Uuid::new_v4();
    let filename = format!("{}.{}", asset_id, ext);
    let dest = state.assets_dir.join(&filename);
    std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    let name = src.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Asset")
        .to_string();
    let asset_kind = if kind == "image" { AssetKind::Image } else { AssetKind::Sound };
    let asset = MediaAsset {
        id: asset_id,
        name,
        kind: asset_kind,
        filename,
        created_at: chrono::Utc::now(),
    };
    let store = storage::ReminderStore::open(&state.db_path).map_err(|e| e.to_string())?;
    store.upsert_asset(&asset).map_err(|e| e.to_string())?;
    Ok(asset_to_dto(&asset))
}

#[tauri::command]
fn recent_events(state: State<AppState>) -> Result<Vec<EventDto>, String> {
    let store = storage::ReminderStore::open(&state.db_path).map_err(|e| e.to_string())?;
    let events = store.recent_events(30).map_err(|e| e.to_string())?;
    Ok(events.into_iter().map(|(title, fired, acked)| EventDto {
        title,
        fired_at: fired.to_rfc3339(),
        acknowledged_at: acked.map(|t| t.to_rfc3339()),
    }).collect())
}

#[tauri::command]
fn acknowledge_popup(event_id: Option<String>, reminder_id: String, state: State<AppState>) {
    // Clear from pending re-fire map
    if let Ok(uuid) = reminder_id.parse::<Uuid>() {
        state.pending.lock().unwrap().remove(&uuid);
    }
    // Log acknowledgement
    if let (Some(eid_str), Ok(store)) = (event_id, storage::ReminderStore::open(&state.db_path)) {
        if let Ok(eid) = eid_str.parse::<Uuid>() {
            let _ = store.log_acknowledged(eid);
        }
    }
    // Release popup queue
    state.popup_active.store(false, Ordering::Relaxed);
}

#[tauri::command]
fn get_assets_dir(state: State<AppState>) -> String {
    state.assets_dir.to_string_lossy().into_owned()
}


// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    register_autolaunch();

    let db_path = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("RoutinePrompt")
        .join("reminders.db");

    let store = storage::ReminderStore::open(&db_path).expect("open db");
    let initial_reminders = store.list().unwrap_or_default();
    drop(store);

    let assets_dir = db_path.parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("assets");
    let _ = std::fs::create_dir_all(&assets_dir);

    let shared: Arc<Mutex<Vec<Reminder>>> = Arc::new(Mutex::new(initial_reminders));
    let pending: PendingAcks = Arc::new(Mutex::new(HashMap::new()));
    let (sched_tx, sched_rx) = mpsc::channel::<Reminder>();
    let popup_queue: Arc<Mutex<VecDeque<Reminder>>> = Arc::new(Mutex::new(VecDeque::new()));
    let popup_active: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    Scheduler::new(shared.clone(), pending.clone()).start(sched_tx);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            db_path: db_path.clone(),
            assets_dir: assets_dir.clone(),
            shared,
            pending,
            popup_queue: popup_queue.clone(),
            popup_active: popup_active.clone(),
        })
        .invoke_handler(tauri::generate_handler![
            list_reminders,
            save_reminder,
            delete_reminder,
            toggle_reminder,
            list_assets,
            delete_asset,
            upload_asset,
            recent_events,
            acknowledge_popup,
            get_assets_dir,
        ])
        .setup(move |app| {
            build_tray(app.handle())?;

            // Create persistent hidden popup window — WebView2 warms up at launch so
            // show() is instant when a reminder fires.
            let popup_win = WebviewWindowBuilder::new(
                app.handle(),
                "popup",
                WebviewUrl::App("popup.html".into()),
            )
            .title("Reminder")
            .fullscreen(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .decorations(false)
            .visible(false)
            .build()?;

            let popup_active_ev = popup_active.clone();
            popup_win.on_window_event(move |event| {
                if matches!(event, tauri::WindowEvent::Destroyed) {
                    popup_active_ev.store(false, Ordering::Relaxed);
                }
            });

            // Poll scheduler + manage popup queue
            let app_handle = app.handle().clone();
            let popup_queue_poll = popup_queue.clone();
            let popup_active_poll = popup_active.clone();
            let db_path_poll = db_path.clone();
            let assets_dir_poll = assets_dir.clone();

            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(200));

                    // Drain scheduler channel into queue
                    while let Ok(reminder) = sched_rx.try_recv() {
                        popup_queue_poll.lock().unwrap().push_back(reminder);
                    }

                    // Show next popup only when none is active
                    if !popup_active_poll.load(Ordering::Relaxed) {
                        let next = popup_queue_poll.lock().unwrap().pop_front();
                        if let Some(reminder) = next {
                            let more_pending = popup_queue_poll.lock().unwrap().len();
                            popup_active_poll.store(true, Ordering::Relaxed);

                            let event_id = storage::ReminderStore::open(&db_path_poll)
                                .ok()
                                .and_then(|s| s.log_fired(reminder.id).ok())
                                .map(|id| id.to_string());

                            let assets = storage::ReminderStore::open(&db_path_poll)
                                .ok()
                                .and_then(|s| s.list_assets().ok())
                                .unwrap_or_default()
                                .iter()
                                .map(asset_to_dto)
                                .collect();

                            let payload = PopupPayload {
                                reminder: reminder_to_dto(&reminder),
                                event_id,
                                more_pending,
                                assets_dir: assets_dir_poll.to_string_lossy().into_owned(),
                                assets,
                            };

                            if let Some(win) = app_handle.get_webview_window("popup") {
                                // Emit payload first so DOM updates before the window is visible
                                let _ = win.emit("popup-trigger", &payload);
                                let _ = win.show();
                                let _ = win.set_focus();
                            } else {
                                // Window was destroyed — release lock so queue can continue
                                popup_active_poll.store(false, Ordering::Relaxed);
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Tray ──────────────────────────────────────────────────────────────────────

fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState};
    use tauri::menu::{Menu, MenuItem};

    let quit = MenuItem::with_id(app, "quit", "Quit RoutinePrompt", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit])?;

    TrayIconBuilder::new()
        .tooltip("RoutinePrompt")
        .menu(&menu)
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "quit" {
                app.exit(0);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                } else {
                    let _ = WebviewWindowBuilder::new(
                        app,
                        "main",
                        WebviewUrl::App("index.html".into()),
                    )
                    .title("RoutinePrompt")
                    .inner_size(400.0, 640.0)
                    .build();
                }
            }
        })
        .build(app)?;
    Ok(())
}

// ── Auto-launch ───────────────────────────────────────────────────────────────

fn register_autolaunch() {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_WRITE};
    use winreg::RegKey;
    let Ok(exe) = std::env::current_exe() else { return };
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(run) = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        KEY_WRITE,
    ) else { return };
    let _ = run.set_value("RoutinePrompt", &exe.to_string_lossy().as_ref());
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn encode_display_mode(m: &DisplayMode) -> &'static str {
    match m {
        DisplayMode::Gentle => "gentle",
        DisplayMode::Persistent => "persistent",
        DisplayMode::Focus => "focus",
    }
}

fn parse_display_mode(s: &str) -> DisplayMode {
    match s {
        "gentle" => DisplayMode::Gentle,
        "focus" => DisplayMode::Focus,
        _ => DisplayMode::Persistent,
    }
}

fn weekday_idx(d: chrono::Weekday) -> u8 {
    use chrono::Weekday::*;
    match d { Mon=>0, Tue=>1, Wed=>2, Thu=>3, Fri=>4, Sat=>5, Sun=>6 }
}

fn idx_weekday(n: u8) -> Option<chrono::Weekday> {
    use chrono::Weekday::*;
    match n { 0=>Some(Mon),1=>Some(Tue),2=>Some(Wed),3=>Some(Thu),4=>Some(Fri),5=>Some(Sat),6=>Some(Sun),_=>None }
}

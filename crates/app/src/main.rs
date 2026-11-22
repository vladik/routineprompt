use domain::{PendingAcks, Reminder};
use gpui::{App, QuitMode, SharedString};
use muda::{Menu, MenuEvent, MenuItem};
use scheduler::Scheduler;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use tray_icon::{TrayIconBuilder, TrayIconEvent, MouseButton};
use ui::main_window::open_main_window;

fn main() -> anyhow::Result<()> {
    register_autolaunch();

    let db_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("RoutinePrompt")
        .join("reminders.db");

    let store = storage::ReminderStore::open(&db_path)?;
    let initial_reminders = store.list().unwrap_or_default();
    drop(store);

    let shared: Arc<Mutex<Vec<Reminder>>> = Arc::new(Mutex::new(initial_reminders));
    let pending: PendingAcks = Arc::new(Mutex::new(HashMap::new()));
    let (sched_tx, sched_rx) = mpsc::channel::<Reminder>();
    Scheduler::new(shared.clone(), pending.clone()).start(sched_tx);

    let _tray = build_tray_icon();

    let pending_for_app = pending.clone();
    let shared_for_app = shared.clone();
    let db_path_for_app = db_path.clone();

    gpui_platform::application()
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx: &mut App| {
            let shared_for_window = shared_for_app.clone();
            let db_path_for_window = db_path_for_app.clone();

            open_main_window(cx, shared_for_window.clone(), db_path_for_window.clone());

            let shared_poll = shared_for_app.clone();
            let pending_spawn = pending_for_app.clone();
            let db_path_spawn = db_path_for_app.clone();
            cx.spawn(async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(200))
                        .await;

                    // Fire due reminders as popups
                    while let Ok(reminder) = sched_rx.try_recv() {
                        let p = pending_spawn.clone();
                        let db = db_path_spawn.clone();
                        cx.update(|cx| {
                            ui::popup::open_popup(cx, &reminder, &p, &db);
                        });
                    }

                    // Handle tray right-click menu
                    while let Ok(event) = MenuEvent::receiver().try_recv() {
                        if event.id.0 == "quit" {
                            cx.update(|cx| cx.quit());
                            return;
                        }
                    }

                    // Handle tray left-click → reopen main window
                    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                        if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
                            let db = db_path_for_window.clone();
                            let shared = shared_poll.clone();
                            cx.update(move |cx| {
                                if cx.windows().is_empty() {
                                    open_main_window(cx, shared, db);
                                }
                            });
                        }
                    }
                }
            })
            .detach();
        });

    Ok(())
}

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

fn build_tray_icon() -> Option<tray_icon::TrayIcon> {
    let mut rgba = Vec::with_capacity(4 * 32 * 32);
    for _ in 0..(32 * 32) {
        rgba.extend_from_slice(&[79u8, 70, 229, 255]); // indigo
    }
    let icon = tray_icon::Icon::from_rgba(rgba, 32, 32).ok()?;

    let menu = Menu::new();
    let quit_item = MenuItem::with_id("quit", "Quit RoutinePrompt", true, None);
    menu.append(&quit_item).ok()?;

    TrayIconBuilder::new()
        .with_tooltip(SharedString::from("RoutinePrompt").as_ref())
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .ok()
}

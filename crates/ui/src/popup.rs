use domain::{AssetKind, DisplayMode, MediaAsset, PendingAcks, Reminder};
use gpui::{
    App, Bounds, Context, Render, SharedString, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, rgb, size,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

pub struct ReminderPopup {
    reminder: Reminder,
    pending: PendingAcks,
    event_id: Option<Uuid>,
    opened_at: std::time::Instant,
    db_path: PathBuf,
    assets_dir: PathBuf,
    assets: Vec<MediaAsset>,
    // Keep alive for the duration of the popup
    _audio: Option<(rodio::OutputStream, rodio::OutputStreamHandle, rodio::Sink)>,
}

impl ReminderPopup {
    fn can_dismiss(&self) -> bool {
        match self.reminder.min_dismissal_mins {
            None => true,
            Some(mins) => self.opened_at.elapsed().as_secs() >= mins as u64 * 60,
        }
    }

    fn secs_remaining(&self) -> u64 {
        match self.reminder.min_dismissal_mins {
            None => 0,
            Some(mins) => {
                let total = mins as u64 * 60;
                let elapsed = self.opened_at.elapsed().as_secs();
                total.saturating_sub(elapsed)
            }
        }
    }

    fn image_paths(&self) -> Vec<PathBuf> {
        self.reminder
            .image_asset_ids
            .iter()
            .take(3)
            .filter_map(|id| {
                let asset = self.assets.iter().find(|a| a.id == *id && a.kind == AssetKind::Image)?;
                let p = self.assets_dir.join(&asset.filename);
                if p.exists() { Some(p) } else { None }
            })
            .collect()
    }
}

impl Render for ReminderPopup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = SharedString::from(self.reminder.title.clone());
        let message = SharedString::from(self.reminder.message.clone());
        let images = self.image_paths();
        let can_dismiss = self.can_dismiss();
        let secs_remaining = self.secs_remaining();
        let n = images.len();

        let (img_w, img_h) = match n {
            1 => (240.0f32, 240.0f32),
            2 => (160.0f32, 160.0f32),
            _ => (120.0f32, 120.0f32),
        };

        let got_it_handler = {
            let pending = self.pending.clone();
            let reminder_id = self.reminder.id;
            let db_path = self.db_path.clone();
            let event_id = self.event_id;
            cx.listener(move |_this, _event, window, _cx| {
                pending.lock().unwrap().remove(&reminder_id);
                if let (Ok(store), Some(eid)) =
                    (storage::ReminderStore::open(&db_path), event_id)
                {
                    let _ = store.log_acknowledged(eid);
                }
                window.remove_window();
            })
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(rgb(0x1a1a2e))
            .gap_6()
            .when(n > 0, |d| {
                d.child(
                    div()
                        .flex()
                        .gap_3()
                        .children(images.into_iter().enumerate().map(|(i, path)| {
                            div()
                                .id(SharedString::from(format!("popup-img-{}", i)))
                                .w(px(img_w))
                                .h(px(img_h))
                                .rounded_xl()
                                .overflow_hidden()
                                .child(
                                    gpui::img(Arc::<std::path::Path>::from(path.as_path()))
                                        .size_full()
                                        .object_fit(gpui::ObjectFit::Cover),
                                )
                        })),
                )
            })
            .child(
                div()
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .text_center()
                    .px_8()
                    .child(title),
            )
            .when(!self.reminder.message.is_empty(), |d| {
                d.child(
                    div()
                        .text_xl()
                        .text_color(rgb(0xcccccc))
                        .text_center()
                        .px_8()
                        .child(message),
                )
            })
            .when(!can_dismiss, |d| {
                let mins = secs_remaining / 60;
                let secs = secs_remaining % 60;
                let wait_text = if mins > 0 {
                    format!("Please wait {}:{:02}", mins, secs)
                } else {
                    format!("Please wait 0:{:02}", secs)
                };
                d.child(
                    div()
                        .mt_8()
                        .px_10()
                        .py_4()
                        .rounded_lg()
                        .bg(rgb(0x374151))
                        .text_xl()
                        .text_color(rgb(0x9ca3af))
                        .child(SharedString::from(wait_text)),
                )
            })
            .when(can_dismiss, |d| {
                d.child(
                    div()
                        .id("got-it")
                        .mt_8()
                        .px_10()
                        .py_4()
                        .rounded_lg()
                        .bg(rgb(0x4f46e5))
                        .text_xl()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .cursor_pointer()
                        .on_click(got_it_handler)
                        .child("Got it"),
                )
            })
    }
}

fn play_sound(
    reminder: &Reminder,
    assets: &[MediaAsset],
    assets_dir: &Path,
) -> Option<(rodio::OutputStream, rodio::OutputStreamHandle, rodio::Sink)> {
    let sound_id = reminder.sound_asset_id?;
    let asset = assets.iter().find(|a| a.id == sound_id && a.kind == AssetKind::Sound)?;
    let path = assets_dir.join(&asset.filename);
    if !path.exists() {
        return None;
    }
    let (stream, handle) = rodio::OutputStream::try_default().ok()?;
    let sink = rodio::Sink::try_new(&handle).ok()?;
    let file = std::fs::File::open(&path).ok()?;
    let source = rodio::Decoder::new(std::io::BufReader::new(file)).ok()?;
    sink.append(source);
    Some((stream, handle, sink))
}

fn make_popup(
    reminder: Reminder,
    pending: PendingAcks,
    db_path: PathBuf,
    assets_dir: PathBuf,
    assets: Vec<MediaAsset>,
    event_id: Option<Uuid>,
) -> ReminderPopup {
    let _audio = play_sound(&reminder, &assets, &assets_dir);
    ReminderPopup {
        reminder,
        pending,
        event_id,
        opened_at: std::time::Instant::now(),
        db_path,
        assets_dir,
        assets,
        _audio,
    }
}

pub fn open_popup(
    cx: &mut App,
    reminder: &Reminder,
    pending: &PendingAcks,
    db_path: &Path,
    assets_dir: &Path,
) {
    let assets = storage::ReminderStore::open(db_path)
        .ok()
        .and_then(|s| s.list_assets().ok())
        .unwrap_or_default();

    let event_id = storage::ReminderStore::open(db_path)
        .ok()
        .and_then(|store| store.log_fired(reminder.id).ok());

    match reminder.display_mode {
        DisplayMode::Focus => {
            open_focus_popup(cx, reminder, pending, db_path, assets_dir, assets, event_id)
        }
        DisplayMode::Persistent => {
            open_persistent_popup(cx, reminder, pending, db_path, assets_dir, assets, event_id)
        }
        DisplayMode::Gentle => {
            open_gentle_popup(cx, reminder, pending, db_path, assets_dir, assets, event_id)
        }
    }
}

fn open_focus_popup(
    cx: &mut App,
    reminder: &Reminder,
    pending: &PendingAcks,
    db_path: &Path,
    assets_dir: &Path,
    assets: Vec<MediaAsset>,
    event_id: Option<Uuid>,
) {
    let reminder = reminder.clone();
    let pending = pending.clone();
    let db_path = db_path.to_path_buf();
    let assets_dir = assets_dir.to_path_buf();
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Fullscreen(Bounds::default())),
            kind: gpui::WindowKind::PopUp,
            ..Default::default()
        },
        move |_, cx| {
            cx.new(|_| make_popup(reminder, pending, db_path, assets_dir, assets, event_id))
        },
    )
    .ok();
}

fn open_persistent_popup(
    cx: &mut App,
    reminder: &Reminder,
    pending: &PendingAcks,
    db_path: &Path,
    assets_dir: &Path,
    assets: Vec<MediaAsset>,
    event_id: Option<Uuid>,
) {
    let reminder = reminder.clone();
    let pending = pending.clone();
    let db_path = db_path.to_path_buf();
    let assets_dir = assets_dir.to_path_buf();
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: gpui::Point { x: px(200.0), y: px(150.0) },
                size: size(px(480.0), px(400.0)),
            })),
            kind: gpui::WindowKind::PopUp,
            ..Default::default()
        },
        move |_, cx| {
            cx.new(|_| make_popup(reminder, pending, db_path, assets_dir, assets, event_id))
        },
    )
    .ok();
}

fn open_gentle_popup(
    cx: &mut App,
    reminder: &Reminder,
    pending: &PendingAcks,
    db_path: &Path,
    assets_dir: &Path,
    assets: Vec<MediaAsset>,
    event_id: Option<Uuid>,
) {
    let reminder = reminder.clone();
    let pending = pending.clone();
    let db_path = db_path.to_path_buf();
    let assets_dir = assets_dir.to_path_buf();
    let window = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: gpui::Point { x: px(20.0), y: px(20.0) },
                size: size(px(320.0), px(200.0)),
            })),
            kind: gpui::WindowKind::PopUp,
            ..Default::default()
        },
        move |_, cx| {
            cx.new(|_| make_popup(reminder, pending, db_path, assets_dir, assets, event_id))
        },
    );
    if let Ok(handle) = window {
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(5))
                .await;
            handle
                .update(cx, |_view, window, _cx| {
                    window.remove_window();
                })
                .ok();
        })
        .detach();
    }
}

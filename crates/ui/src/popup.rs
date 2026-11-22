use domain::{DisplayMode, PendingAcks, Reminder};
use gpui::{
    App, Bounds, Context, Render, SharedString, Window, WindowBounds, WindowOptions,
    div, img, prelude::*, px, rgb, size,
};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

pub struct ReminderPopup {
    reminder: Reminder,
    pending: PendingAcks,
    event_id: Option<Uuid>,
    opened_at: std::time::Instant,
    db_path: PathBuf,
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
}

impl Render for ReminderPopup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = SharedString::from(self.reminder.title.clone());
        let message = SharedString::from(self.reminder.message.clone());
        let image_path = self.reminder.image_path.clone();
        let can_dismiss = self.can_dismiss();
        let secs_remaining = self.secs_remaining();

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
            .when_some(image_path, |d, path| {
                let p = PathBuf::from(&path);
                if p.exists() {
                    d.child(
                        img(Arc::<std::path::Path>::from(p.as_path()))
                            .w(px(240.0))
                            .h(px(240.0))
                            .rounded_xl()
                            .object_fit(gpui::ObjectFit::Cover),
                    )
                } else {
                    d
                }
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

pub fn open_popup(
    cx: &mut App,
    reminder: &Reminder,
    pending: &PendingAcks,
    db_path: &std::path::Path,
) {
    let event_id = storage::ReminderStore::open(db_path)
        .ok()
        .and_then(|store| store.log_fired(reminder.id).ok());

    match reminder.display_mode {
        DisplayMode::Focus => open_focus_popup(cx, reminder, pending, db_path, event_id),
        DisplayMode::Persistent => open_persistent_popup(cx, reminder, pending, db_path, event_id),
        DisplayMode::Gentle => open_gentle_popup(cx, reminder, pending, db_path, event_id),
    }
}

fn make_popup(
    reminder: Reminder,
    pending: PendingAcks,
    db_path: PathBuf,
    event_id: Option<Uuid>,
) -> ReminderPopup {
    ReminderPopup {
        reminder,
        pending,
        event_id,
        opened_at: std::time::Instant::now(),
        db_path,
    }
}

fn open_focus_popup(
    cx: &mut App,
    reminder: &Reminder,
    pending: &PendingAcks,
    db_path: &std::path::Path,
    event_id: Option<Uuid>,
) {
    let reminder = reminder.clone();
    let pending = pending.clone();
    let db_path = db_path.to_path_buf();
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Fullscreen(Bounds::default())),
            kind: gpui::WindowKind::PopUp,
            ..Default::default()
        },
        move |_, cx| cx.new(|_| make_popup(reminder, pending, db_path, event_id)),
    )
    .ok();
}

fn open_persistent_popup(
    cx: &mut App,
    reminder: &Reminder,
    pending: &PendingAcks,
    db_path: &std::path::Path,
    event_id: Option<Uuid>,
) {
    let reminder = reminder.clone();
    let pending = pending.clone();
    let db_path = db_path.to_path_buf();
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: gpui::Point { x: px(200.0), y: px(150.0) },
                size: size(px(480.0), px(400.0)),
            })),
            kind: gpui::WindowKind::PopUp,
            ..Default::default()
        },
        move |_, cx| cx.new(|_| make_popup(reminder, pending, db_path, event_id)),
    )
    .ok();
}

fn open_gentle_popup(
    cx: &mut App,
    reminder: &Reminder,
    pending: &PendingAcks,
    db_path: &std::path::Path,
    event_id: Option<Uuid>,
) {
    let reminder = reminder.clone();
    let pending = pending.clone();
    let db_path = db_path.to_path_buf();
    let window = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: gpui::Point { x: px(20.0), y: px(20.0) },
                size: size(px(320.0), px(200.0)),
            })),
            kind: gpui::WindowKind::PopUp,
            ..Default::default()
        },
        move |_, cx| cx.new(|_| make_popup(reminder, pending, db_path, event_id)),
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

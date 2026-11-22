use domain::{DisplayMode, Reminder};
use gpui::{
    App, Bounds, Context, Render, SharedString, Window, WindowBounds,
    WindowOptions, div, img, prelude::*, px, rgb, size,
};
use std::sync::Arc;
use std::path::PathBuf;

pub struct ReminderPopup {
    reminder: Reminder,
}

impl Render for ReminderPopup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = SharedString::from(self.reminder.title.clone());
        let message = SharedString::from(self.reminder.message.clone());
        let image_path = self.reminder.image_path.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(rgb(0x1a1a2e))
            .gap_6()
            // Photo (if set and file exists)
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
            // Title
            .child(
                div()
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .text_center()
                    .px_8()
                    .child(title),
            )
            // Message (if set)
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
            // Got it button
            .child(
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
                    .on_click(cx.listener(|_this, _event, window, _cx| {
                        window.remove_window();
                    }))
                    .child("Got it"),
            )
    }
}

pub fn open_popup(cx: &mut App, reminder: &Reminder) {
    match reminder.display_mode {
        DisplayMode::Focus => open_focus_popup(cx, reminder),
        DisplayMode::Persistent => open_persistent_popup(cx, reminder),
        DisplayMode::Gentle => open_gentle_popup(cx, reminder),
    }
}

fn open_focus_popup(cx: &mut App, reminder: &Reminder) {
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Fullscreen(Bounds::default())),
            ..Default::default()
        },
        {
            let reminder = reminder.clone();
            move |_, cx| cx.new(|_| ReminderPopup { reminder })
        },
    )
    .ok();
}

fn open_persistent_popup(cx: &mut App, reminder: &Reminder) {
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: gpui::Point { x: px(200.0), y: px(150.0) },
                size: size(px(480.0), px(400.0)),
            })),
            ..Default::default()
        },
        {
            let reminder = reminder.clone();
            move |_, cx| cx.new(|_| ReminderPopup { reminder })
        },
    )
    .ok();
}

fn open_gentle_popup(cx: &mut App, reminder: &Reminder) {
    let window = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: gpui::Point { x: px(20.0), y: px(20.0) },
                size: size(px(320.0), px(200.0)),
            })),
            ..Default::default()
        },
        {
            let reminder = reminder.clone();
            move |_, cx| cx.new(|_| ReminderPopup { reminder })
        },
    );

    // Auto-close after 5 seconds
    if let Ok(handle) = window {
        cx.spawn(async move |mut cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(5)).await;
            handle.update(cx, |_view, window, _cx| {
                window.remove_window();
            }).ok();
        })
        .detach();
    }
}

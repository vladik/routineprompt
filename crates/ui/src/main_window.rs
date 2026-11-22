use crate::text_input::TextInput;
use chrono::{NaiveTime, Timelike, Utc, Weekday};
use domain::{DisplayMode, Reminder, Schedule};
use gpui::{
    App, Bounds, Context, Entity, Render, SharedString, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use std::sync::{Arc, Mutex};
use storage::ReminderStore;
use uuid::Uuid;

#[derive(Clone, PartialEq)]
enum Mode {
    List,
    Form,
}

pub struct MainWindow {
    store: ReminderStore,
    shared: Arc<Mutex<Vec<Reminder>>>,
    reminders: Vec<Reminder>,
    mode: Mode,

    // Form state
    form_title: Entity<TextInput>,
    form_image: Entity<TextInput>,
    form_hour: u8,
    form_minute: u8,
    form_days: [bool; 7],
    form_display_mode: DisplayMode,
    form_editing_id: Option<Uuid>,
}

impl MainWindow {
    pub fn new(
        cx: &mut Context<Self>,
        store: ReminderStore,
        shared: Arc<Mutex<Vec<Reminder>>>,
        reminders: Vec<Reminder>,
    ) -> Self {
        let form_title = cx.new(|cx| TextInput::new(cx, "e.g. Time for dinner"));
        let form_image = cx.new(|cx| TextInput::new(cx, "Path to photo (optional)"));
        Self {
            store,
            shared,
            reminders,
            mode: Mode::List,
            form_title,
            form_image,
            form_hour: 8,
            form_minute: 0,
            form_days: [true, true, true, true, true, true, true],
            form_display_mode: DisplayMode::Persistent,
            form_editing_id: None,
        }
    }

    fn sync_shared(&self) {
        *self.shared.lock().unwrap() = self.reminders.clone();
    }

    fn open_add_form(&mut self, cx: &mut Context<Self>) {
        self.form_editing_id = None;
        self.form_title.update(cx, |t, cx| {
            t.value.clear();
            cx.notify();
        });
        self.form_image.update(cx, |t, cx| {
            t.value.clear();
            cx.notify();
        });
        self.form_hour = 8;
        self.form_minute = 0;
        self.form_days = [true, true, true, true, true, true, true];
        self.form_display_mode = DisplayMode::Persistent;
        self.mode = Mode::Form;
        cx.notify();
    }

    fn open_edit_form(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(r) = self.reminders.iter().find(|r| r.id == id).cloned() {
            self.form_editing_id = Some(id);
            let title = r.title.clone();
            let image = r.image_path.clone().unwrap_or_default();
            self.form_title.update(cx, |t, cx| {
                t.value = title;
                cx.notify();
            });
            self.form_image.update(cx, |t, cx| {
                t.value = image;
                cx.notify();
            });
            match &r.schedule {
                Schedule::Daily { at } | Schedule::Weekly { at, .. } => {
                    self.form_hour = at.hour() as u8;
                    self.form_minute = at.minute() as u8;
                }
                Schedule::Once { at } => {
                    let local = at.with_timezone(&chrono::Local);
                    self.form_hour = local.hour() as u8;
                    self.form_minute = local.minute() as u8;
                }
            }
            if let Schedule::Weekly { days, .. } = &r.schedule {
                self.form_days = [false; 7];
                for d in days {
                    self.form_days[weekday_idx(*d)] = true;
                }
            } else {
                self.form_days = [true; 7];
            }
            self.form_display_mode = r.display_mode.clone();
            self.mode = Mode::Form;
            cx.notify();
        }
    }

    fn save_form(&mut self, cx: &mut Context<Self>) {
        let title = self.form_title.read(cx).value.clone();
        if title.trim().is_empty() {
            return;
        }

        let image_path = {
            let p = self.form_image.read(cx).value.clone();
            if p.is_empty() { None } else { Some(p) }
        };

        let active_days: Vec<Weekday> = self.form_days
            .iter()
            .enumerate()
            .filter(|(_, &on)| on)
            .map(|(i, _)| idx_weekday(i))
            .collect();

        let at = NaiveTime::from_hms_opt(self.form_hour as u32, self.form_minute as u32, 0)
            .unwrap_or_default();

        let schedule = if active_days.len() == 7 {
            Schedule::Daily { at }
        } else {
            Schedule::Weekly { days: active_days, at }
        };

        let now = Utc::now();

        let reminder = if let Some(id) = self.form_editing_id {
            if let Some(existing) = self.reminders.iter_mut().find(|r| r.id == id) {
                existing.title = title;
                existing.image_path = image_path;
                existing.schedule = schedule;
                existing.display_mode = self.form_display_mode.clone();
                existing.updated_at = now;
                let _ = self.store.upsert(existing);
                self.sync_shared();
                self.mode = Mode::List;
                cx.notify();
                return;
            }
            return;
        } else {
            Reminder {
                id: Uuid::new_v4(),
                title,
                message: String::new(),
                image_path,
                sound_path: None,
                schedule,
                display_mode: self.form_display_mode.clone(),
                enabled: true,
                created_at: now,
                updated_at: now,
            }
        };

        let _ = self.store.upsert(&reminder);
        self.reminders.push(reminder);
        self.sync_shared();
        self.mode = Mode::List;
        cx.notify();
    }

    fn toggle_reminder(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(r) = self.reminders.iter_mut().find(|r| r.id == id) {
            r.enabled = !r.enabled;
            let _ = self.store.set_enabled(id, r.enabled);
        }
        self.sync_shared();
        cx.notify();
    }

    fn delete_reminder(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.reminders.retain(|r| r.id != id);
        let _ = self.store.delete(id);
        self.sync_shared();
        cx.notify();
    }

    fn render_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let reminders = self.reminders.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf2f2f7))
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .py_3()
                    .bg(rgb(0xffffff))
                    .border_b_1()
                    .border_color(rgb(0xe5e5ea))
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x000000))
                            .child("RoutinePrompt"),
                    )
                    .child(
                        div()
                            .id("add-btn")
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_full()
                            .bg(rgb(0x4f46e5))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(rgb(0xffffff))
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("+")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_add_form(cx);
                            })),
                    ),
            )
            // Reminder list
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .py_2()
                    .children(reminders.iter().enumerate().map(|(i, r)| {
                        self.render_reminder_row(r, i, cx)
                    })),
            )
    }

    fn render_reminder_row(&self, r: &Reminder, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let id = r.id;
        let enabled = r.enabled;
        let title = r.title.clone();
        let time_str = schedule_display(&r.schedule);

        div()
            .id(SharedString::from(format!("row-{}", index)))
            .flex()
            .items_center()
            .px_4()
            .py_3()
            .bg(rgb(0xffffff))
            .border_b_1()
            .border_color(rgb(0xe5e5ea))
            .gap_3()
            // Toggle
            .child(
                div()
                    .id(SharedString::from(format!("toggle-{}", index)))
                    .w(px(44.0))
                    .h(px(26.0))
                    .rounded_full()
                    .bg(if enabled { rgb(0x34c759) } else { rgb(0xe5e5ea) })
                    .flex()
                    .items_center()
                    .px(px(3.0))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_reminder(id, cx);
                    }))
                    .child(
                        div()
                            .size(px(20.0))
                            .rounded_full()
                            .bg(rgb(0xffffff))
                            .when(enabled, |d| d.ml_auto()),
                    ),
            )
            // Text (click to edit)
            .child(
                div()
                    .id(SharedString::from(format!("text-{}", index)))
                    .flex_1()
                    .flex()
                    .flex_col()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_edit_form(id, cx);
                    }))
                    .child(
                        div()
                            .text_base()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(0x000000))
                            .child(SharedString::from(title)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x8e8e93))
                            .child(SharedString::from(time_str)),
                    ),
            )
            // Delete button
            .child(
                div()
                    .id(SharedString::from(format!("del-{}", index)))
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(rgb(0xff3b30))
                    .text_base()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.delete_reminder(id, cx);
                    }))
                    .child("✕"),
            )
    }

    fn render_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_editing = self.form_editing_id.is_some();
        let hour = self.form_hour;
        let minute = self.form_minute;
        let days = self.form_days;
        let mode = self.form_display_mode.clone();
        let day_labels = ["M", "T", "W", "T", "F", "S", "S"];

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf2f2f7))
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .bg(rgb(0xffffff))
                    .border_b_1()
                    .border_color(rgb(0xe5e5ea))
                    .child(
                        div()
                            .id("back-btn")
                            .text_base()
                            .text_color(rgb(0x4f46e5))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = Mode::List;
                                cx.notify();
                            }))
                            .child("← Back"),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x000000))
                            .child(if is_editing { "Edit Reminder" } else { "Add Reminder" }),
                    ),
            )
            // Form body
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    // Title field
                    .child(self.render_form_section("Title", div().child(self.form_title.clone())))
                    // Time picker
                    .child(self.render_form_section(
                        "Time",
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.render_time_spinner(hour, true, cx))
                            .child(div().text_xl().text_color(rgb(0x000000)).child(":"))
                            .child(self.render_time_spinner(minute, false, cx)),
                    ))
                    // Days picker
                    .child(self.render_form_section(
                        "Repeat",
                        div()
                            .flex()
                            .gap_1()
                            .children(day_labels.iter().enumerate().map(|(i, &label)| {
                                let selected = days[i];
                                div()
                                    .id(SharedString::from(format!("day-{}", i)))
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .bg(if selected { rgb(0x4f46e5) } else { rgb(0xe5e5ea) })
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(if selected { rgb(0xffffff) } else { rgb(0x636366) })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.form_days[i] = !this.form_days[i];
                                        cx.notify();
                                    }))
                                    .child(label)
                            })),
                    ))
                    // Display mode
                    .child(self.render_form_section(
                        "Style",
                        div()
                            .flex()
                            .gap_2()
                            .children([DisplayMode::Gentle, DisplayMode::Persistent, DisplayMode::Focus].iter().map(|m| {
                                let selected = *m == mode;
                                let label = match m {
                                    DisplayMode::Gentle => "Gentle",
                                    DisplayMode::Persistent => "Persistent",
                                    DisplayMode::Focus => "Focus",
                                };
                                let m = m.clone();
                                div()
                                    .id(SharedString::from(format!("mode-{}", label)))
                                    .px_3()
                                    .py_1()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(rgb(0x4f46e5))
                                    .bg(if selected { rgb(0x4f46e5) } else { rgb(0xffffff) })
                                    .text_sm()
                                    .text_color(if selected { rgb(0xffffff) } else { rgb(0x4f46e5) })
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.form_display_mode = m.clone();
                                        cx.notify();
                                    }))
                                    .child(label)
                            })),
                    ))
                    // Photo
                    .child(self.render_form_section("Photo", div().child(self.form_image.clone())))
                    // Buttons
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .pt_2()
                            .child(
                                div()
                                    .id("cancel-btn")
                                    .flex_1()
                                    .py_3()
                                    .rounded_xl()
                                    .bg(rgb(0xe5e5ea))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(rgb(0x000000))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mode = Mode::List;
                                        cx.notify();
                                    }))
                                    .child("Cancel"),
                            )
                            .child(
                                div()
                                    .id("save-btn")
                                    .flex_1()
                                    .py_3()
                                    .rounded_xl()
                                    .bg(rgb(0x4f46e5))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(rgb(0xffffff))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.save_form(cx);
                                    }))
                                    .child("Save"),
                            ),
                    ),
            )
    }

    fn render_form_section(&self, label: &'static str, content: impl IntoElement) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(0x636366))
                    .child(label),
            )
            .child(content)
    }

    fn render_time_spinner(&self, value: u8, is_hour: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let (max, label) = if is_hour { (23u8, format!("{:02}", value)) } else { (59u8, format!("{:02}", value)) };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .child(
                div()
                    .id(if is_hour { "hour-up" } else { "min-up" })
                    .w(px(44.0))
                    .h(px(32.0))
                    .rounded_md()
                    .bg(rgb(0xe5e5ea))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(rgb(0x000000))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if is_hour {
                            this.form_hour = (this.form_hour + 1) % (max + 1);
                        } else {
                            this.form_minute = (this.form_minute + 1) % (max + 1);
                        }
                        cx.notify();
                    }))
                    .child("▲"),
            )
            .child(
                div()
                    .w(px(44.0))
                    .h(px(40.0))
                    .rounded_md()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xd1d5db))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x000000))
                    .child(SharedString::from(label)),
            )
            .child(
                div()
                    .id(if is_hour { "hour-dn" } else { "min-dn" })
                    .w(px(44.0))
                    .h(px(32.0))
                    .rounded_md()
                    .bg(rgb(0xe5e5ea))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(rgb(0x000000))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if is_hour {
                            this.form_hour = this.form_hour.checked_sub(1).unwrap_or(max);
                        } else {
                            this.form_minute = this.form_minute.checked_sub(1).unwrap_or(max);
                        }
                        cx.notify();
                    }))
                    .child("▼"),
            )
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.mode {
            Mode::List => self.render_list(cx).into_any_element(),
            Mode::Form => self.render_form(cx).into_any_element(),
        }
    }
}

fn schedule_display(s: &Schedule) -> String {
    match s {
        Schedule::Once { at } => {
            let local = at.with_timezone(&chrono::Local);
            format!("Once at {}", local.format("%b %-d, %-I:%M %p"))
        }
        Schedule::Daily { at } => format!("Daily at {}", at.format("%-I:%M %p")),
        Schedule::Weekly { days, at } => {
            let day_names: Vec<&str> = days.iter().map(|d| weekday_short(*d)).collect();
            format!("{} at {}", day_names.join(", "), at.format("%-I:%M %p"))
        }
    }
}

fn weekday_short(d: Weekday) -> &'static str {
    match d {
        Weekday::Mon => "Mon", Weekday::Tue => "Tue", Weekday::Wed => "Wed",
        Weekday::Thu => "Thu", Weekday::Fri => "Fri", Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

fn weekday_idx(d: Weekday) -> usize {
    match d {
        Weekday::Mon => 0, Weekday::Tue => 1, Weekday::Wed => 2,
        Weekday::Thu => 3, Weekday::Fri => 4, Weekday::Sat => 5, Weekday::Sun => 6,
    }
}

fn idx_weekday(i: usize) -> Weekday {
    match i {
        0 => Weekday::Mon, 1 => Weekday::Tue, 2 => Weekday::Wed,
        3 => Weekday::Thu, 4 => Weekday::Fri, 5 => Weekday::Sat,
        _ => Weekday::Sun,
    }
}

pub fn open_main_window(cx: &mut App, shared: Arc<Mutex<Vec<Reminder>>>, db_path: std::path::PathBuf) {
    let bounds = Bounds::centered(None, size(px(400.0), px(640.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some(SharedString::from("RoutinePrompt")),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |_, cx| {
            let store = storage::ReminderStore::open(&db_path).expect("open db");
            let reminders = store.list().unwrap_or_default();
            cx.new(|cx| MainWindow::new(cx, store, shared, reminders))
        },
    )
    .ok();
}

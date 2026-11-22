use crate::text_input::TextInput;
use chrono::{Local, NaiveTime, Timelike, Utc, Weekday};
use domain::{AssetKind, DisplayMode, MediaAsset, Reminder, Schedule};
use gpui::{
    App, Bounds, Context, Entity, Render, ScrollHandle, SharedString, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use storage::ReminderStore;
use uuid::Uuid;

#[derive(Clone, PartialEq)]
enum Mode {
    List,
    Form,
    Analytics,
    Media,
    Picker { kind: AssetKind, image_slot: Option<usize> },
}

pub struct MainWindow {
    store: ReminderStore,
    shared: Arc<Mutex<Vec<Reminder>>>,
    reminders: Vec<Reminder>,
    assets: Vec<MediaAsset>,
    assets_dir: PathBuf,
    mode: Mode,

    form_title: Entity<TextInput>,
    form_hour: u8,
    form_minute: u8,
    form_days: [bool; 7],
    form_display_mode: DisplayMode,
    form_editing_id: Option<Uuid>,
    form_image_ids: [Option<Uuid>; 3],
    form_sound_id: Option<Uuid>,
    system_12h: bool,
    form_is_pm: bool,
    form_refire: bool,
    form_refire_after_mins: u8,
    form_refire_until_ack: bool,
    form_min_dismissal: bool,
    form_min_dismissal_mins: u8,
    form_scroll: ScrollHandle,
    // File picker result arrives here from a background thread; polled in render().
    pending_file_pick: Option<(mpsc::Receiver<Option<PathBuf>>, AssetKind, Option<usize>)>,
}

impl MainWindow {
    pub fn new(
        cx: &mut Context<Self>,
        store: ReminderStore,
        shared: Arc<Mutex<Vec<Reminder>>>,
        reminders: Vec<Reminder>,
        assets_dir: PathBuf,
    ) -> Self {
        let _ = std::fs::create_dir_all(&assets_dir);
        let assets = store.list_assets().unwrap_or_default();
        let form_title = cx.new(|cx| TextInput::new(cx, "e.g. Time for dinner"));
        Self {
            store,
            shared,
            reminders,
            assets,
            assets_dir,
            mode: Mode::List,
            form_title,
            form_hour: 8,
            form_minute: 0,
            form_days: [true; 7],
            form_display_mode: DisplayMode::Persistent,
            form_editing_id: None,
            form_image_ids: [None; 3],
            form_sound_id: None,
            system_12h: detect_system_12h(),
            form_is_pm: false,
            form_refire: false,
            form_refire_after_mins: 10,
            form_refire_until_ack: true,
            form_min_dismissal: false,
            form_min_dismissal_mins: 2,
            form_scroll: ScrollHandle::new(),
            pending_file_pick: None,
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
        self.form_hour = 8;
        self.form_minute = 0;
        self.form_days = [true; 7];
        self.form_display_mode = DisplayMode::Persistent;
        self.form_image_ids = [None; 3];
        self.form_sound_id = None;
        self.form_is_pm = false;
        self.form_refire = false;
        self.form_refire_after_mins = 10;
        self.form_refire_until_ack = true;
        self.form_min_dismissal = false;
        self.form_min_dismissal_mins = 2;
        self.assets = self.store.list_assets().unwrap_or_default();
        self.mode = Mode::Form;
        cx.notify();
    }

    fn open_edit_form(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(r) = self.reminders.iter().find(|r| r.id == id).cloned() {
            self.form_editing_id = Some(id);
            let title = r.title.clone();
            self.form_title.update(cx, |t, cx| {
                t.value = title;
                cx.notify();
            });
            match &r.schedule {
                Schedule::Daily { at } | Schedule::Weekly { at, .. } => {
                    self.form_hour = at.hour() as u8;
                    self.form_minute = at.minute() as u8;
                }
                Schedule::Once { at } => {
                    let local = at.with_timezone(&Local);
                    self.form_hour = local.hour() as u8;
                    self.form_minute = local.minute() as u8;
                }
            }
            self.form_is_pm = self.form_hour >= 12;
            if let Schedule::Weekly { days, .. } = &r.schedule {
                self.form_days = [false; 7];
                for d in days {
                    self.form_days[weekday_idx(*d)] = true;
                }
            } else {
                self.form_days = [true; 7];
            }
            self.form_display_mode = r.display_mode.clone();
            self.form_image_ids = {
                let mut slots = [None; 3];
                for (i, &id) in r.image_asset_ids.iter().take(3).enumerate() {
                    slots[i] = Some(id);
                }
                slots
            };
            self.form_sound_id = r.sound_asset_id;
            self.form_refire = r.refire_after_mins.is_some();
            self.form_refire_after_mins = r.refire_after_mins.unwrap_or(10);
            self.form_refire_until_ack = r.refire_until_ack;
            self.form_min_dismissal = r.min_dismissal_mins.is_some();
            self.form_min_dismissal_mins = r.min_dismissal_mins.unwrap_or(2);
            self.assets = self.store.list_assets().unwrap_or_default();
            self.mode = Mode::Form;
            cx.notify();
        }
    }

    fn save_form(&mut self, cx: &mut Context<Self>) {
        let title = self.form_title.read(cx).value.clone();
        if title.trim().is_empty() {
            return;
        }

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

        let refire_after_mins = if self.form_refire { Some(self.form_refire_after_mins) } else { None };
        let min_dismissal_mins = if self.form_min_dismissal { Some(self.form_min_dismissal_mins) } else { None };
        let image_asset_ids: Vec<Uuid> = self.form_image_ids.iter().flatten().copied().collect();
        let sound_asset_id = self.form_sound_id;

        let now = Utc::now();

        if let Some(id) = self.form_editing_id {
            if let Some(existing) = self.reminders.iter_mut().find(|r| r.id == id) {
                existing.title = title;
                existing.schedule = schedule;
                existing.display_mode = self.form_display_mode.clone();
                existing.image_asset_ids = image_asset_ids;
                existing.sound_asset_id = sound_asset_id;
                existing.refire_after_mins = refire_after_mins;
                existing.refire_until_ack = self.form_refire_until_ack;
                existing.min_dismissal_mins = min_dismissal_mins;
                existing.updated_at = now;
                let _ = self.store.upsert(existing);
                self.sync_shared();
                self.mode = Mode::List;
                cx.notify();
            }
            return;
        }

        let reminder = Reminder {
            id: Uuid::new_v4(),
            title,
            message: String::new(),
            image_asset_ids,
            sound_asset_id,
            schedule,
            display_mode: self.form_display_mode.clone(),
            refire_after_mins,
            refire_until_ack: self.form_refire_until_ack,
            min_dismissal_mins,
            enabled: true,
            created_at: now,
            updated_at: now,
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

    fn apply_file_pick(&mut self, src: PathBuf, kind: AssetKind, slot: Option<usize>, cx: &mut Context<Self>) {
        let ext = src.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin")
            .to_lowercase();
        let asset_id = Uuid::new_v4();
        let filename = format!("{}.{}", asset_id, ext);
        let dest = self.assets_dir.join(&filename);
        if std::fs::copy(&src, &dest).is_ok() {
            let name = src.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Asset")
                .to_string();
            let asset = domain::MediaAsset {
                id: asset_id,
                name,
                kind,
                filename,
                created_at: chrono::Utc::now(),
            };
            let _ = self.store.upsert_asset(&asset);
            self.assets = self.store.list_assets().unwrap_or_default();
            if let Some(s) = slot {
                self.form_image_ids[s] = Some(asset_id);
            } else {
                self.form_sound_id = Some(asset_id);
            }
            self.mode = Mode::Form;
            cx.notify();
        }
    }

    fn render_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let reminders = self.reminders.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf2f2f7))
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
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("media-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_lg()
                                    .bg(rgb(0xf2f2f7))
                                    .text_sm()
                                    .text_color(rgb(0x636366))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.assets = this.store.list_assets().unwrap_or_default();
                                        this.mode = Mode::Media;
                                        cx.notify();
                                    }))
                                    .child("Media"),
                            )
                            .child(
                                div()
                                    .id("analytics-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_lg()
                                    .bg(rgb(0xf2f2f7))
                                    .text_sm()
                                    .text_color(rgb(0x636366))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mode = Mode::Analytics;
                                        cx.notify();
                                    }))
                                    .child("Activity"),
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
                    ),
            )
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

    fn render_reminder_row(
        &self,
        r: &Reminder,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
        let system_12h = self.system_12h;
        let form_refire = self.form_refire;
        let form_refire_after = self.form_refire_after_mins;
        let form_refire_until_ack = self.form_refire_until_ack;
        let form_min_dismissal = self.form_min_dismissal;
        let form_min_dismissal_mins = self.form_min_dismissal_mins;

        let ampm_handler = cx.listener(|this, _, _, cx| {
            this.form_hour = (this.form_hour + 12) % 24;
            this.form_is_pm = this.form_hour >= 12;
            cx.notify();
        });
        let refire_toggle = cx.listener(|this, _, _, cx| {
            this.form_refire = !this.form_refire;
            cx.notify();
        });
        let refire_ack_toggle = cx.listener(|this, _, _, cx| {
            this.form_refire_until_ack = !this.form_refire_until_ack;
            cx.notify();
        });
        let refire_5 = cx.listener(|this, _, _, cx| { this.form_refire_after_mins = 5; cx.notify(); });
        let refire_10 = cx.listener(|this, _, _, cx| { this.form_refire_after_mins = 10; cx.notify(); });
        let refire_30 = cx.listener(|this, _, _, cx| { this.form_refire_after_mins = 30; cx.notify(); });
        let dismissal_toggle = cx.listener(|this, _, _, cx| {
            this.form_min_dismissal = !this.form_min_dismissal;
            cx.notify();
        });
        let dismissal_1 = cx.listener(|this, _, _, cx| { this.form_min_dismissal_mins = 1; cx.notify(); });
        let dismissal_2 = cx.listener(|this, _, _, cx| { this.form_min_dismissal_mins = 2; cx.notify(); });
        let dismissal_5 = cx.listener(|this, _, _, cx| { this.form_min_dismissal_mins = 5; cx.notify(); });
        let dismissal_10 = cx.listener(|this, _, _, cx| { this.form_min_dismissal_mins = 10; cx.notify(); });

        // Image slots
        let img_slot_0 = self.form_image_ids[0];
        let img_slot_1 = self.form_image_ids[1];
        let img_slot_2 = self.form_image_ids[2];
        let img_name_0 = img_slot_0.and_then(|id| self.assets.iter().find(|a| a.id == id).map(|a| a.name.clone()));
        let img_name_1 = img_slot_1.and_then(|id| self.assets.iter().find(|a| a.id == id).map(|a| a.name.clone()));
        let img_name_2 = img_slot_2.and_then(|id| self.assets.iter().find(|a| a.id == id).map(|a| a.name.clone()));

        let sound_label = match self.form_sound_id {
            Some(id) => self.assets.iter()
                .find(|a| a.id == id)
                .map(|a| format!("\u{1F50A} {}", a.name))
                .unwrap_or_else(|| "\u{1F50A} Sound".into()),
            None => "+ Add sound".into(),
        };
        let has_sound = self.form_sound_id.is_some();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf2f2f7))
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
                            .child("\u{2190} Back"),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x000000))
                            .child(if is_editing { "Edit Reminder" } else { "Add Reminder" }),
                    ),
            )
            .child(
                div()
                    .id("form-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.form_scroll)
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    // Title
                    .child(self.render_form_section("Title", div().child(self.form_title.clone())))
                    // Time
                    .child(self.render_form_section(
                        "Time",
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.render_time_spinner(hour, true, cx))
                            .child(div().text_xl().text_color(rgb(0x000000)).child(":"))
                            .child(self.render_time_spinner(minute, false, cx))
                            .when(system_12h, |d| {
                                let is_pm = hour >= 12;
                                d.child(
                                    div()
                                        .id("ampm-toggle")
                                        .px_4()
                                        .py_2()
                                        .rounded_lg()
                                        .bg(rgb(0x4f46e5))
                                        .text_xl()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(rgb(0xffffff))
                                        .cursor_pointer()
                                        .on_click(ampm_handler)
                                        .child(if is_pm { "PM" } else { "AM" }),
                                )
                            }),
                    ))
                    // Repeat
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
                    // Style
                    .child(self.render_form_section(
                        "Style",
                        div()
                            .flex()
                            .gap_2()
                            .children(
                                [DisplayMode::Gentle, DisplayMode::Persistent, DisplayMode::Focus]
                                    .iter()
                                    .map(|m| {
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
                                    }),
                            ),
                    ))
                    // Images
                    .child(self.render_form_section(
                        "Images (up to 3)",
                        div()
                            .flex()
                            .gap_3()
                            .child(self.render_image_slot(0, img_slot_0, img_name_0, cx))
                            .child(self.render_image_slot(1, img_slot_1, img_name_1, cx))
                            .child(self.render_image_slot(2, img_slot_2, img_name_2, cx)),
                    ))
                    // Sound
                    .child(self.render_form_section(
                        "Sound",
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .id("sound-slot")
                                    .px_4()
                                    .py_2()
                                    .rounded_xl()
                                    .bg(if has_sound { rgb(0x4f46e5) } else { rgb(0xe5e5ea) })
                                    .text_sm()
                                    .text_color(if has_sound { rgb(0xffffff) } else { rgb(0x636366) })
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.assets = this.store.list_assets().unwrap_or_default();
                                        this.mode = Mode::Picker { kind: AssetKind::Sound, image_slot: None };
                                        cx.notify();
                                    }))
                                    .child(SharedString::from(sound_label)),
                            )
                            .when(has_sound, |d| {
                                d.child(
                                    div()
                                        .id("sound-clear")
                                        .px_3()
                                        .py_2()
                                        .rounded_lg()
                                        .bg(rgb(0xffe5e5))
                                        .text_sm()
                                        .text_color(rgb(0xff3b30))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.form_sound_id = None;
                                            cx.notify();
                                        }))
                                        .child("Remove"),
                                )
                            }),
                    ))
                    // If nobody responds
                    .child(self.render_form_section(
                        "If nobody responds",
                        {
                            let refire_pill = div()
                                .id("refire-toggle")
                                .w(px(44.0))
                                .h(px(26.0))
                                .rounded_full()
                                .bg(if form_refire { rgb(0x34c759) } else { rgb(0xe5e5ea) })
                                .flex()
                                .items_center()
                                .px(px(3.0))
                                .cursor_pointer()
                                .on_click(refire_toggle)
                                .child(
                                    div()
                                        .size(px(20.0))
                                        .rounded_full()
                                        .bg(rgb(0xffffff))
                                        .when(form_refire, |d| d.ml_auto()),
                                );
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .child(refire_pill)
                                        .child(div().text_base().text_color(rgb(0x000000)).child("Try again if nobody responds")),
                                )
                                .when(form_refire, move |d| {
                                    d.child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(div().text_sm().text_color(rgb(0x636366)).child("After:"))
                                            .child(self.render_pill_btn("refire-5", "5 min", form_refire_after == 5, refire_5))
                                            .child(self.render_pill_btn("refire-10", "10 min", form_refire_after == 10, refire_10))
                                            .child(self.render_pill_btn("refire-30", "30 min", form_refire_after == 30, refire_30)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .id("refire-ack-toggle")
                                                    .w(px(44.0))
                                                    .h(px(26.0))
                                                    .rounded_full()
                                                    .bg(if form_refire_until_ack { rgb(0x34c759) } else { rgb(0xe5e5ea) })
                                                    .flex()
                                                    .items_center()
                                                    .px(px(3.0))
                                                    .cursor_pointer()
                                                    .on_click(refire_ack_toggle)
                                                    .child(
                                                        div()
                                                            .size(px(20.0))
                                                            .rounded_full()
                                                            .bg(rgb(0xffffff))
                                                            .when(form_refire_until_ack, |d| d.ml_auto()),
                                                    ),
                                            )
                                            .child(div().text_sm().text_color(rgb(0x000000)).child("Keep trying until they respond")),
                                    )
                                })
                        },
                    ))
                    // Dismissal
                    .child(self.render_form_section(
                        "Dismissal",
                        {
                            let dismissal_pill = div()
                                .id("dismissal-toggle")
                                .w(px(44.0))
                                .h(px(26.0))
                                .rounded_full()
                                .bg(if form_min_dismissal { rgb(0x34c759) } else { rgb(0xe5e5ea) })
                                .flex()
                                .items_center()
                                .px(px(3.0))
                                .cursor_pointer()
                                .on_click(dismissal_toggle)
                                .child(
                                    div()
                                        .size(px(20.0))
                                        .rounded_full()
                                        .bg(rgb(0xffffff))
                                        .when(form_min_dismissal, |d| d.ml_auto()),
                                );
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .child(dismissal_pill)
                                        .child(div().text_base().text_color(rgb(0x000000)).child("Lock the OK button for a while")),
                                )
                                .when(form_min_dismissal, move |d| {
                                    d.child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(div().text_sm().text_color(rgb(0x636366)).child("Wait:"))
                                            .child(self.render_pill_btn("dismissal-1", "1 min", form_min_dismissal_mins == 1, dismissal_1))
                                            .child(self.render_pill_btn("dismissal-2", "2 min", form_min_dismissal_mins == 2, dismissal_2))
                                            .child(self.render_pill_btn("dismissal-5", "5 min", form_min_dismissal_mins == 5, dismissal_5))
                                            .child(self.render_pill_btn("dismissal-10", "10 min", form_min_dismissal_mins == 10, dismissal_10)),
                                    )
                                })
                        },
                    ))
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

    fn render_image_slot(
        &self,
        slot: usize,
        asset_id: Option<Uuid>,
        name: Option<String>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let full_path = asset_id.and_then(|id| {
            self.assets.iter().find(|a| a.id == id).map(|a| self.assets_dir.join(&a.filename))
        });
        let has_image = full_path.as_ref().map_or(false, |p| p.exists());
        let label = name.unwrap_or_else(|| "+".into());

        div()
            .id(SharedString::from(format!("img-slot-{}", slot)))
            .w(px(90.0))
            .h(px(90.0))
            .rounded_xl()
            .bg(if asset_id.is_some() { rgb(0x4f46e5) } else { rgb(0xe5e5ea) })
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .overflow_hidden()
            .text_sm()
            .text_color(if asset_id.is_some() { rgb(0xffffff) } else { rgb(0x636366) })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.assets = this.store.list_assets().unwrap_or_default();
                this.mode = Mode::Picker { kind: AssetKind::Image, image_slot: Some(slot) };
                cx.notify();
            }))
            .when(has_image, |d| {
                let p = full_path.clone().unwrap();
                d.child(
                    gpui::img(Arc::<std::path::Path>::from(p.as_path()))
                        .size_full()
                        .object_fit(gpui::ObjectFit::Cover),
                )
            })
            .when(!has_image, |d| d.child(SharedString::from(label)))
    }

    fn render_pill_btn<H>(
        &self,
        id: &'static str,
        label: &'static str,
        selected: bool,
        handler: H,
    ) -> impl IntoElement
    where
        H: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    {
        div()
            .id(id)
            .px_3()
            .py_1()
            .rounded_full()
            .border_1()
            .border_color(rgb(0x4f46e5))
            .bg(if selected { rgb(0x4f46e5) } else { rgb(0xffffff) })
            .text_sm()
            .text_color(if selected { rgb(0xffffff) } else { rgb(0x4f46e5) })
            .cursor_pointer()
            .on_click(handler)
            .child(label)
    }

    fn render_form_section(
        &self,
        label: &'static str,
        content: impl IntoElement,
    ) -> impl IntoElement {
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

    fn render_time_spinner(
        &self,
        value: u8,
        is_hour: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let max = if is_hour { 23u8 } else { 59u8 };
        let label = if is_hour && self.system_12h {
            let display = if value == 0 { 12u8 } else if value > 12 { value - 12 } else { value };
            format!("{:02}", display)
        } else {
            format!("{:02}", value)
        };

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
                    .child("\u{25B2}"),
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
                    .child("\u{25BC}"),
            )
    }

    fn render_picker(
        &self,
        kind: AssetKind,
        image_slot: Option<usize>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_image = kind == AssetKind::Image;
        let title = if is_image { "Pick Image" } else { "Pick Sound" };
        let assets: Vec<MediaAsset> = self.assets.iter()
            .filter(|a| a.kind == kind)
            .cloned()
            .collect();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf2f2f7))
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
                            .id("picker-back")
                            .text_base()
                            .text_color(rgb(0x4f46e5))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = Mode::Form;
                                cx.notify();
                            }))
                            .child("\u{2190} Back"),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x000000))
                            .child(title),
                    ),
            )
            .child(
                div()
                    .px_4()
                    .py_3()
                    .child(
                        div()
                            .id("picker-upload")
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(rgb(0x4f46e5))
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let (filter_name, filters): (&'static str, &'static [&'static str]) =
                                    if is_image {
                                        ("Images", &["png", "jpg", "jpeg", "gif", "webp"])
                                    } else {
                                        ("Sounds", &["mp3", "wav", "m4a", "ogg", "aac"])
                                    };
                                // Run the native file dialog on a background thread so we never
                                // block GPUI's render loop (blocking it triggers GPU TDR on Windows).
                                let (tx, rx) = mpsc::channel::<Option<PathBuf>>();
                                std::thread::spawn(move || {
                                    let _ = tx.send(
                                        rfd::FileDialog::new()
                                            .add_filter(filter_name, filters)
                                            .pick_file(),
                                    );
                                });
                                this.pending_file_pick = Some((
                                    rx,
                                    if is_image { AssetKind::Image } else { AssetKind::Sound },
                                    image_slot,
                                ));
                                cx.notify();
                            }))
                            .child(if is_image { "Upload image\u{2026}" } else { "Upload sound\u{2026}" }),
                    ),
            )
            .child(
                div()
                    .id("picker-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.form_scroll)
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(assets.is_empty(), |d| {
                        d.child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x8e8e93))
                                .child(if is_image { "No images yet" } else { "No sounds yet" }),
                        )
                    })
                    .children(assets.into_iter().map(|asset| {
                        let asset_id = asset.id;
                        let asset_name = asset.name.clone();
                        let is_img = asset.kind == AssetKind::Image;
                        let full_path = self.assets_dir.join(&asset.filename);
                        let path_exists = full_path.exists();

                        div()
                            .id(SharedString::from(format!("asset-{}", asset_id)))
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_4()
                            .py_3()
                            .bg(rgb(0xffffff))
                            .rounded_xl()
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Mode::Picker { image_slot, .. } = &this.mode {
                                    let slot = *image_slot;
                                    if let Some(s) = slot {
                                        this.form_image_ids[s] = Some(asset_id);
                                    } else {
                                        this.form_sound_id = Some(asset_id);
                                    }
                                }
                                this.mode = Mode::Form;
                                cx.notify();
                            }))
                            .when(is_img && path_exists, |d| {
                                d.child(
                                    gpui::img(Arc::<std::path::Path>::from(full_path.as_path()))
                                        .w(px(48.0))
                                        .h(px(48.0))
                                        .rounded_lg()
                                        .object_fit(gpui::ObjectFit::Cover),
                                )
                            })
                            .when(!is_img, |d| {
                                d.child(
                                    div()
                                        .w(px(48.0))
                                        .h(px(48.0))
                                        .rounded_lg()
                                        .bg(rgb(0xf2f2f7))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xl()
                                        .child("\u{1F50A}"),
                                )
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .text_base()
                                    .text_color(rgb(0x000000))
                                    .child(SharedString::from(asset_name)),
                            )
                    })),
            )
    }

    fn render_media_library(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let assets = self.store.list_assets().unwrap_or_default();
        let (images, sounds): (Vec<_>, Vec<_>) =
            assets.iter().partition(|a| a.kind == AssetKind::Image);

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf2f2f7))
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
                            .id("media-back")
                            .text_base()
                            .text_color(rgb(0x4f46e5))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = Mode::List;
                                cx.notify();
                            }))
                            .child("\u{2190} Back"),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x000000))
                            .child("Media Library"),
                    ),
            )
            .child(
                div()
                    .id("media-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.form_scroll)
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(0x636366))
                            .child(SharedString::from(format!("IMAGES ({})", images.len()))),
                    )
                    .when(images.is_empty(), |d| {
                        d.child(div().text_sm().text_color(rgb(0x8e8e93)).child("No images"))
                    })
                    .children(images.into_iter().map(|asset| self.render_asset_row(asset, cx)))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(0x636366))
                            .pt_4()
                            .child(SharedString::from(format!("SOUNDS ({})", sounds.len()))),
                    )
                    .when(sounds.is_empty(), |d| {
                        d.child(div().text_sm().text_color(rgb(0x8e8e93)).child("No sounds"))
                    })
                    .children(sounds.into_iter().map(|asset| self.render_asset_row(asset, cx))),
            )
    }

    fn render_asset_row(&self, asset: &MediaAsset, cx: &mut Context<Self>) -> impl IntoElement {
        let asset_id = asset.id;
        let name = asset.name.clone();
        let is_image = asset.kind == AssetKind::Image;
        let full_path = self.assets_dir.join(&asset.filename);
        let path_exists = full_path.exists();

        div()
            .id(SharedString::from(format!("lib-asset-{}", asset_id)))
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .bg(rgb(0xffffff))
            .rounded_xl()
            .when(is_image && path_exists, |d| {
                d.child(
                    gpui::img(Arc::<std::path::Path>::from(full_path.as_path()))
                        .w(px(48.0))
                        .h(px(48.0))
                        .rounded_lg()
                        .object_fit(gpui::ObjectFit::Cover),
                )
            })
            .when(!is_image, |d| {
                d.child(
                    div()
                        .w(px(48.0))
                        .h(px(48.0))
                        .rounded_lg()
                        .bg(rgb(0xf2f2f7))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xl()
                        .child("\u{1F50A}"),
                )
            })
            .child(
                div()
                    .flex_1()
                    .text_base()
                    .text_color(rgb(0x000000))
                    .child(SharedString::from(name)),
            )
            .child(
                div()
                    .id(SharedString::from(format!("lib-del-{}", asset_id)))
                    .px_3()
                    .py_1()
                    .rounded_lg()
                    .bg(rgb(0xffe5e5))
                    .text_sm()
                    .text_color(rgb(0xff3b30))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(a) = this.assets.iter().find(|a| a.id == asset_id).cloned() {
                            let _ = std::fs::remove_file(this.assets_dir.join(&a.filename));
                        }
                        let _ = this.store.delete_asset(asset_id);
                        this.assets = this.store.list_assets().unwrap_or_default();
                        cx.notify();
                    }))
                    .child("Delete"),
            )
    }

    fn render_analytics(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let events = self.store.recent_events(30).unwrap_or_default();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf2f2f7))
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
                            .id("analytics-back")
                            .text_base()
                            .text_color(rgb(0x4f46e5))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = Mode::List;
                                cx.notify();
                            }))
                            .child("\u{2190} Back"),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x000000))
                            .child("Activity"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(events.is_empty(), |d| {
                        d.child(
                            div()
                                .text_base()
                                .text_color(rgb(0x8e8e93))
                                .text_center()
                                .pt_8()
                                .child("No activity yet"),
                        )
                    })
                    .children(events.into_iter().map(|(title, fired_at, acked_at)| {
                        let local_fire = fired_at.with_timezone(&chrono::Local);
                        let time_str = local_fire.format("%-I:%M %p, %b %-d").to_string();
                        let status = match acked_at {
                            Some(acked) => {
                                let secs = (acked - fired_at).num_seconds();
                                if secs < 60 { format!("acknowledged in {}s", secs) }
                                else { format!("acknowledged in {}m", secs / 60) }
                            }
                            None => "not acknowledged".to_string(),
                        };
                        div()
                            .flex()
                            .flex_col()
                            .px_4()
                            .py_3()
                            .bg(rgb(0xffffff))
                            .rounded_xl()
                            .gap_1()
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
                                    .child(SharedString::from(format!("{} \u{2014} {}", time_str, status))),
                            )
                    })),
            )
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Poll for background file picker result — avoids blocking GPUI's render thread.
        if let Some((rx, kind, slot)) = self.pending_file_pick.take() {
            match rx.try_recv() {
                Ok(Some(src)) => self.apply_file_pick(src, kind, slot, cx),
                Ok(None) => {} // user cancelled
                Err(mpsc::TryRecvError::Empty) => {
                    self.pending_file_pick = Some((rx, kind, slot));
                    cx.notify(); // keep polling each frame until dialog closes
                }
                Err(mpsc::TryRecvError::Disconnected) => {} // thread died
            }
        }

        match self.mode.clone() {
            Mode::List => self.render_list(cx).into_any_element(),
            Mode::Form => self.render_form(cx).into_any_element(),
            Mode::Analytics => self.render_analytics(cx).into_any_element(),
            Mode::Media => self.render_media_library(cx).into_any_element(),
            Mode::Picker { kind, image_slot } => {
                self.render_picker(kind, image_slot, cx).into_any_element()
            }
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
        Weekday::Thu => 3, Weekday::Fri => 4, Weekday::Sat => 5,
        Weekday::Sun => 6,
    }
}

fn idx_weekday(i: usize) -> Weekday {
    match i {
        0 => Weekday::Mon, 1 => Weekday::Tue, 2 => Weekday::Wed,
        3 => Weekday::Thu, 4 => Weekday::Fri, 5 => Weekday::Sat,
        _ => Weekday::Sun,
    }
}

fn detect_system_12h() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(intl) = hkcu.open_subkey(r"Control Panel\International") {
        if let Ok(fmt) = intl.get_value::<String, _>("sTimeFormat") {
            return fmt.contains('h') || fmt.contains('t');
        }
    }
    true
}

pub fn open_main_window(
    cx: &mut App,
    shared: Arc<Mutex<Vec<Reminder>>>,
    db_path: std::path::PathBuf,
) {
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: gpui::Point { x: px(100.0), y: px(80.0) },
                size: size(px(400.0), px(640.0)),
            })),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some(SharedString::from("RoutinePrompt")),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |_, cx| {
            let store = storage::ReminderStore::open(&db_path).expect("open db");
            let reminders = store.list().unwrap_or_default();
            let assets_dir = db_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("assets");
            cx.new(|cx| MainWindow::new(cx, store, shared, reminders, assets_dir))
        },
    )
    .ok();
}

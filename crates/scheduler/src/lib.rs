use chrono::{Datelike, Local, NaiveTime, Timelike, Weekday};
use domain::{Reminder, Schedule};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Scheduler {
    reminders: Arc<Mutex<Vec<Reminder>>>,
}

impl Scheduler {
    pub fn new(reminders: Arc<Mutex<Vec<Reminder>>>) -> Self {
        Self { reminders }
    }

    pub fn start(self, tx: std::sync::mpsc::Sender<Reminder>) {
        std::thread::spawn(move || {
            let mut last_fired_minute: Option<(u32, u32)> = None;
            loop {
                let now = Local::now();
                let current_minute = (now.hour(), now.minute());

                if last_fired_minute != Some(current_minute) {
                    last_fired_minute = Some(current_minute);
                    let reminders = self.reminders.lock().unwrap().clone();
                    for reminder in reminders {
                        if reminder.enabled && is_due(&reminder.schedule, now) {
                            let _ = tx.send(reminder);
                        }
                    }
                }

                // Sleep until the next minute, checking every 10s to avoid drift
                let secs_into_minute = now.second();
                let secs_to_next = if secs_into_minute < 55 {
                    10
                } else {
                    (60 - secs_into_minute) + 1
                };
                std::thread::sleep(Duration::from_secs(secs_to_next as u64));
            }
        });
    }
}

fn is_due(schedule: &Schedule, now: chrono::DateTime<Local>) -> bool {
    let h = now.hour();
    let m = now.minute();
    let target = NaiveTime::from_hms_opt(h, m, 0).unwrap();

    match schedule {
        Schedule::Once { at } => {
            let local = at.with_timezone(&Local);
            local.date_naive() == now.date_naive()
                && NaiveTime::from_hms_opt(local.hour(), local.minute(), 0).unwrap() == target
        }
        Schedule::Daily { at } => {
            NaiveTime::from_hms_opt(at.hour(), at.minute(), 0).unwrap() == target
        }
        Schedule::Weekly { days, at } => {
            let today: Weekday = now.weekday();
            days.contains(&today)
                && NaiveTime::from_hms_opt(at.hour(), at.minute(), 0).unwrap() == target
        }
    }
}

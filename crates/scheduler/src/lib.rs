use chrono::{Datelike, Local, NaiveTime, Timelike, Weekday};
use domain::{PendingAcks, Reminder, Schedule};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Scheduler {
    reminders: Arc<Mutex<Vec<Reminder>>>,
    pending: PendingAcks,
}

impl Scheduler {
    pub fn new(reminders: Arc<Mutex<Vec<Reminder>>>, pending: PendingAcks) -> Self {
        Self { reminders, pending }
    }

    pub fn start(self, tx: std::sync::mpsc::Sender<Reminder>) {
        std::thread::spawn(move || {
            let mut last_fired_minute: Option<(u32, u32)> = None;
            loop {
                let now = Local::now();
                let current_minute = (now.hour(), now.minute());

                // Normal scheduled firing — once per new minute
                if last_fired_minute != Some(current_minute) {
                    last_fired_minute = Some(current_minute);
                    let reminders = self.reminders.lock().unwrap().clone();
                    for reminder in &reminders {
                        if reminder.enabled && is_due(&reminder.schedule, now) {
                            let _ = tx.send(reminder.clone());
                            if reminder.refire_after_mins.is_some() {
                                self.pending
                                    .lock()
                                    .unwrap()
                                    .insert(reminder.id, std::time::Instant::now());
                            }
                        }
                    }
                }

                // Re-fire unacknowledged reminders
                {
                    let now_instant = std::time::Instant::now();
                    let pending_snapshot: Vec<(uuid::Uuid, std::time::Instant)> = self
                        .pending
                        .lock()
                        .unwrap()
                        .iter()
                        .map(|(k, v)| (*k, *v))
                        .collect();
                    let reminders = self.reminders.lock().unwrap().clone();
                    for (id, last_fire) in pending_snapshot {
                        if let Some(reminder) =
                            reminders.iter().find(|r| r.id == id && r.enabled)
                        {
                            if let Some(refire_mins) = reminder.refire_after_mins {
                                let elapsed = now_instant.duration_since(last_fire).as_secs();
                                if elapsed >= refire_mins as u64 * 60 {
                                    let _ = tx.send(reminder.clone());
                                    self.pending
                                        .lock()
                                        .unwrap()
                                        .insert(id, now_instant);
                                }
                            }
                        }
                    }
                }

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

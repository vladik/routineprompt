use chrono::{DateTime, NaiveTime, Utc, Weekday};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DisplayMode {
    Gentle,
    Persistent,
    Focus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Schedule {
    Once { at: DateTime<Utc> },
    Daily { at: NaiveTime },
    Weekly { days: Vec<Weekday>, at: NaiveTime },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: Uuid,
    pub title: String,
    pub message: String,
    pub image_path: Option<String>,
    pub sound_path: Option<String>,
    pub schedule: Schedule,
    pub display_mode: DisplayMode,
    pub refire_after_mins: Option<u8>,
    pub refire_until_ack: bool,
    pub min_dismissal_mins: Option<u8>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Reminder {
    pub fn new(title: impl Into<String>, message: impl Into<String>, schedule: Schedule) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            message: message.into(),
            image_path: None,
            sound_path: None,
            schedule,
            display_mode: DisplayMode::Persistent,
            refire_after_mins: None,
            refire_until_ack: false,
            min_dismissal_mins: None,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }
}

pub type PendingAcks = std::sync::Arc<
    std::sync::Mutex<std::collections::HashMap<uuid::Uuid, std::time::Instant>>,
>;

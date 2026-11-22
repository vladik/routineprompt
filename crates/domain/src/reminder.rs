use chrono::{DateTime, NaiveTime, Utc, Weekday};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How visually intrusive the reminder is when it fires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DisplayMode {
    /// Small popup, auto-dismisses after a timeout.
    Gentle,
    /// Popup stays until the user clicks "Got it".
    Persistent,
    /// Full-screen, always-on-top. Can't be ignored.
    Focus,
}

/// When a reminder repeats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Schedule {
    /// Fires once at a specific point in time.
    Once { at: DateTime<Utc> },
    /// Fires every day at a given local time.
    Daily { at: NaiveTime },
    /// Fires on specific days of the week at a given local time.
    Weekly { days: Vec<Weekday>, at: NaiveTime },
}

/// A single reminder as stored and managed by the app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: Uuid,
    pub title: String,
    pub message: String,
    pub image_path: Option<String>,
    pub sound_path: Option<String>,
    pub schedule: Schedule,
    pub display_mode: DisplayMode,
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
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }
}

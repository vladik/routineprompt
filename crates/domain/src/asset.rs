use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssetKind {
    Image,
    Sound,
}

impl AssetKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AssetKind::Image => "image",
            AssetKind::Sound => "sound",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "image" => AssetKind::Image,
            _ => AssetKind::Sound,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAsset {
    pub id: Uuid,
    pub name: String,
    pub kind: AssetKind,
    pub filename: String,
    pub created_at: DateTime<Utc>,
}

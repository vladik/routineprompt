use anyhow::{Context, Result};
use chrono::{DateTime, NaiveTime, Utc, Weekday};
use domain::{DisplayMode, Reminder, Schedule};
use rusqlite::{Connection, params};
use std::path::Path;
use uuid::Uuid;

pub struct ReminderStore {
    conn: Connection,
}

impl ReminderStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening database at {}", path.display()))?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS reminders (
                id           TEXT PRIMARY KEY,
                title        TEXT NOT NULL,
                message      TEXT NOT NULL DEFAULT '',
                image_path   TEXT,
                sound_path   TEXT,
                sched_type   TEXT NOT NULL,
                sched_time   TEXT NOT NULL,
                sched_days   TEXT,
                display_mode TEXT NOT NULL DEFAULT 'persistent',
                enabled      INTEGER NOT NULL DEFAULT 1,
                created_at   TEXT NOT NULL,
                updated_at   TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<Reminder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, message, image_path, sound_path,
                    sched_type, sched_time, sched_days,
                    display_mode, enabled, created_at, updated_at
             FROM reminders ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, bool>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (id, title, message, image_path, sound_path,
                 sched_type, sched_time, sched_days,
                 display_mode_str, enabled, created_at, updated_at) = row?;

            let schedule = parse_schedule(&sched_type, &sched_time, sched_days.as_deref())?;
            let display_mode = parse_display_mode(&display_mode_str);

            out.push(Reminder {
                id: Uuid::parse_str(&id)?,
                title,
                message,
                image_path,
                sound_path,
                schedule,
                display_mode,
                enabled,
                created_at: created_at.parse::<DateTime<Utc>>()?,
                updated_at: updated_at.parse::<DateTime<Utc>>()?,
            });
        }
        Ok(out)
    }

    pub fn upsert(&self, r: &Reminder) -> Result<()> {
        let (sched_type, sched_time, sched_days) = encode_schedule(&r.schedule);
        self.conn.execute(
            "INSERT INTO reminders
                (id, title, message, image_path, sound_path,
                 sched_type, sched_time, sched_days,
                 display_mode, enabled, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(id) DO UPDATE SET
                title=excluded.title, message=excluded.message,
                image_path=excluded.image_path, sound_path=excluded.sound_path,
                sched_type=excluded.sched_type, sched_time=excluded.sched_time,
                sched_days=excluded.sched_days, display_mode=excluded.display_mode,
                enabled=excluded.enabled, updated_at=excluded.updated_at",
            params![
                r.id.to_string(), r.title, r.message,
                r.image_path, r.sound_path,
                sched_type, sched_time, sched_days,
                encode_display_mode(&r.display_mode),
                r.enabled,
                r.created_at.to_rfc3339(),
                r.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.conn.execute("DELETE FROM reminders WHERE id=?1", params![id.to_string()])?;
        Ok(())
    }

    pub fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE reminders SET enabled=?1, updated_at=?2 WHERE id=?3",
            params![enabled, Utc::now().to_rfc3339(), id.to_string()],
        )?;
        Ok(())
    }
}

fn encode_schedule(s: &Schedule) -> (String, String, Option<String>) {
    match s {
        Schedule::Once { at } => ("once".into(), at.to_rfc3339(), None),
        Schedule::Daily { at } => ("daily".into(), at.format("%H:%M").to_string(), None),
        Schedule::Weekly { days, at } => {
            let days_str = days.iter().map(|d| weekday_to_num(d).to_string()).collect::<Vec<_>>().join(",");
            ("weekly".into(), at.format("%H:%M").to_string(), Some(days_str))
        }
    }
}

fn parse_schedule(sched_type: &str, sched_time: &str, sched_days: Option<&str>) -> Result<Schedule> {
    match sched_type {
        "once" => Ok(Schedule::Once { at: sched_time.parse::<DateTime<Utc>>()? }),
        "daily" => Ok(Schedule::Daily { at: NaiveTime::parse_from_str(sched_time, "%H:%M")? }),
        "weekly" => {
            let days = sched_days.unwrap_or("").split(',')
                .filter_map(|n| n.parse::<u8>().ok())
                .filter_map(num_to_weekday)
                .collect();
            Ok(Schedule::Weekly {
                days,
                at: NaiveTime::parse_from_str(sched_time, "%H:%M")?,
            })
        }
        other => anyhow::bail!("unknown schedule type: {}", other),
    }
}

fn encode_display_mode(m: &DisplayMode) -> &'static str {
    match m {
        DisplayMode::Gentle => "gentle",
        DisplayMode::Persistent => "persistent",
        DisplayMode::Focus => "focus",
    }
}

fn parse_display_mode(s: &str) -> DisplayMode {
    match s {
        "gentle" => DisplayMode::Gentle,
        "focus" => DisplayMode::Focus,
        _ => DisplayMode::Persistent,
    }
}

fn weekday_to_num(d: &Weekday) -> u8 {
    match d {
        Weekday::Mon => 0, Weekday::Tue => 1, Weekday::Wed => 2,
        Weekday::Thu => 3, Weekday::Fri => 4, Weekday::Sat => 5, Weekday::Sun => 6,
    }
}

fn num_to_weekday(n: u8) -> Option<Weekday> {
    match n {
        0 => Some(Weekday::Mon), 1 => Some(Weekday::Tue), 2 => Some(Weekday::Wed),
        3 => Some(Weekday::Thu), 4 => Some(Weekday::Fri), 5 => Some(Weekday::Sat),
        6 => Some(Weekday::Sun), _ => None,
    }
}

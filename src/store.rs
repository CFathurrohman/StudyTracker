//! Offline data: one JSON file in %LOCALAPPDATA%\StudyTracker (override with STUDYTRACKER_DATA_DIR).
//! Writes go to a temp file first and are then renamed over the old one, so a crash never
//! leaves a half-written file. The previous good file is kept as data.bak.json.

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Subject {
    pub id: u64,
    pub name: String,
    pub color: u32,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: u64,
    pub subject_id: Option<u64>,
    pub title: String,
    pub est_pomos: u32,
    #[serde(default)]
    pub done_pomos: u32,
    #[serde(default)]
    pub done: bool,
    pub created: i64,
    /// Study day this to-do belongs to (YYYY-MM-DD); None = no date (Tasks page only).
    #[serde(default)]
    pub day: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Session {
    pub id: u64,
    pub subject_id: Option<u64>,
    #[serde(default)]
    pub task_id: Option<u64>,
    /// Unix seconds.
    pub start: i64,
    /// Focused seconds (pauses and removed idle time excluded).
    pub secs: i64,
    #[serde(default)]
    pub note: String,
    /// "pomodoro" | "stopwatch" | "countdown" | "manual"
    #[serde(default)]
    pub kind: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub scheme: String,
    pub accent: String,
    pub focus_min: u32,
    pub short_min: u32,
    pub long_min: u32,
    pub long_every: u32,
    pub countdown_min: u32,
    pub auto_breaks: bool,
    pub auto_focus: bool,
    pub daily_goal_min: u32,
    pub idle_enabled: bool,
    pub idle_min: u32,
    pub sound: bool,
    pub float_visible: bool,
    pub float_pos: Option<[f32; 2]>,
    pub float_opacity: f32,
    pub mascot_name: String,
    pub mascot_enabled: bool,
    pub hotkeys: bool,
    pub user_name: String,
    pub scene: String,
    pub glass_tint: f32,
    pub cat_roam: bool,
    /// Hour a new study day starts (0-6): sessions before it count for the previous day.
    pub day_start_hour: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scheme: "midnight".into(),
            accent: "auto".into(),
            focus_min: 25,
            short_min: 5,
            long_min: 15,
            long_every: 4,
            countdown_min: 45,
            auto_breaks: true,
            auto_focus: false,
            daily_goal_min: 120,
            idle_enabled: true,
            idle_min: 5,
            sound: true,
            float_visible: true,
            float_pos: None,
            float_opacity: 0.94,
            mascot_name: "Meng".into(),
            mascot_enabled: true,
            hotkeys: true,
            user_name: "Student".into(),
            scene: "night".into(),
            glass_tint: 0.35,
            cat_roam: true,
            day_start_hour: 4,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Data {
    pub version: u32,
    pub next_id: u64,
    pub subjects: Vec<Subject>,
    pub tasks: Vec<Task>,
    pub sessions: Vec<Session>,
    pub settings: Settings,
    /// One intention per day, keyed YYYY-MM-DD.
    pub intentions: std::collections::BTreeMap<String, String>,
    pub ddays: Vec<DDay>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DDay {
    pub id: u64,
    pub name: String,
    /// YYYY-MM-DD
    pub date: String,
}

pub const SUBJECT_COLORS: &[u32] = &[
    0xFF8A3D, 0xFFC233, 0x32D74B, 0x34D3A0, 0x38BDF8, 0x0A84FF, 0x6E6CF0, 0xA78BFA, 0xBF5AF2, 0xFF4F79,
];

impl Data {
    fn fresh() -> Self {
        let mut d = Data { version: 1, next_id: 1, ..Default::default() };
        for (name, color) in [("Math", 0x38BDF8), ("English", 0xFF8A3D), ("Programming", 0x34D3A0)] {
            let id = d.new_id();
            d.subjects.push(Subject { id, name: name.into(), color, archived: false });
        }
        d
    }

    pub fn new_id(&mut self) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        id
    }

    pub fn subject(&self, id: Option<u64>) -> Option<&Subject> {
        id.and_then(|id| self.subjects.iter().find(|s| s.id == id))
    }

    pub fn task(&self, id: Option<u64>) -> Option<&Task> {
        id.and_then(|id| self.tasks.iter().find(|t| t.id == id))
    }

    pub fn subject_color(&self, id: Option<u64>) -> u32 {
        self.subject(id).map_or(0x8B91A1, |s| s.color)
    }

    pub fn subject_name(&self, id: Option<u64>) -> String {
        self.subject(id).map_or_else(|| "No subject".into(), |s| s.name.clone())
    }

    /// Focused seconds per local day.
    pub fn secs_on(&self, day: NaiveDate) -> i64 {
        self.sessions.iter().filter(|s| local_day(s.start) == day).map(|s| s.secs).sum()
    }

    pub fn secs_between(&self, from: NaiveDate, to: NaiveDate) -> i64 {
        self.sessions
            .iter()
            .filter(|s| {
                let d = local_day(s.start);
                d >= from && d <= to
            })
            .map(|s| s.secs)
            .sum()
    }

    /// Days in a row (ending today, or yesterday if nothing yet today) with at least `min_secs` focused.
    pub fn streak(&self, min_secs: i64) -> u32 {
        let mut day = today();
        if self.secs_on(day) < min_secs {
            day = day.pred_opt().unwrap();
        }
        let mut n = 0;
        while self.secs_on(day) >= min_secs {
            n += 1;
            day = day.pred_opt().unwrap();
        }
        n
    }

    pub fn best_streak(&self, min_secs: i64) -> u32 {
        let mut days: Vec<NaiveDate> = self.sessions.iter().map(|s| local_day(s.start)).collect();
        days.sort();
        days.dedup();
        let (mut best, mut cur, mut prev): (u32, u32, Option<NaiveDate>) = (0, 0, None);
        for d in days {
            if self.secs_on(d) < min_secs {
                cur = 0;
                prev = None;
                continue;
            }
            cur = if prev.and_then(|p| p.succ_opt()) == Some(d) { cur + 1 } else { 1 };
            best = best.max(cur);
            prev = Some(d);
        }
        best
    }
}

static DAY_START_HOUR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(4);

/// Sets the hour a study day starts (late-night sessions count for the day before).
pub fn set_day_start(hour: u32) {
    DAY_START_HOUR.store(hour.min(6), std::sync::atomic::Ordering::Relaxed);
}

pub fn day_start_hour() -> u32 {
    DAY_START_HOUR.load(std::sync::atomic::Ordering::Relaxed)
}

/// Today's study day.
pub fn today() -> NaiveDate {
    local_day(now())
}

/// Unix time where a study day begins (its date at the day-start hour).
pub fn day_begin(day: NaiveDate) -> i64 {
    let dt = day.and_hms_opt(day_start_hour(), 0, 0).unwrap();
    Local.from_local_datetime(&dt).earliest().map_or(0, |d| d.timestamp())
}

pub fn day_key(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

pub fn now() -> i64 {
    Local::now().timestamp()
}

pub fn local(ts: i64) -> DateTime<Local> {
    Local.timestamp_opt(ts, 0).single().unwrap_or_else(Local::now)
}

/// The study day a moment belongs to.
pub fn local_day(ts: i64) -> NaiveDate {
    (local(ts) - chrono::Duration::hours(day_start_hour() as i64)).date_naive()
}

pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("STUDYTRACKER_DATA_DIR") {
        return PathBuf::from(d);
    }
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("StudyTracker")
}

fn data_file() -> PathBuf {
    data_dir().join("data.json")
}

/// The app used to be called FathStudy: bring its data over once, leaving the old folder as a backup.
fn migrate_old_name() {
    if std::env::var_os("STUDYTRACKER_DATA_DIR").is_some() || data_file().exists() {
        return;
    }
    let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else { return };
    let old = base.join("FathStudy");
    if !old.join("data.json").exists() {
        return;
    }
    let new = data_dir();
    let _ = std::fs::create_dir_all(&new);
    for name in ["data.json", "data.bak.json"] {
        if old.join(name).exists() {
            let _ = std::fs::copy(old.join(name), new.join(name));
        }
    }
}

pub fn load() -> Data {
    migrate_old_name();
    let path = data_file();
    let read = |p: &PathBuf| std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str::<Data>(&s).ok());
    if let Some(d) = read(&path) {
        return d;
    }
    // Main file missing or damaged: fall back to the backup, keeping the damaged file aside.
    if path.exists() {
        let _ = std::fs::copy(&path, data_dir().join(format!("data.broken-{}.json", now())));
    }
    read(&data_dir().join("data.bak.json")).unwrap_or_else(Data::fresh)
}

pub fn save(data: &Data) -> std::io::Result<()> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    let path = data_file();
    let tmp = dir.join("data.json.tmp");
    let json = serde_json::to_string_pretty(data).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, json)?;
    if path.exists() {
        let _ = std::fs::copy(&path, dir.join("data.bak.json"));
    }
    std::fs::rename(&tmp, &path)
}

/// Writes sessions as CSV and the whole data as JSON into <data dir>\exports. Returns the folder.
pub fn export(data: &Data) -> std::io::Result<PathBuf> {
    let dir = data_dir().join("exports");
    std::fs::create_dir_all(&dir)?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let mut csv = String::from("date,start,minutes,subject,task,kind,note\n");
    let esc = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut sessions = data.sessions.clone();
    sessions.sort_by_key(|s| s.start);
    for s in &sessions {
        let t = local(s.start);
        csv.push_str(&format!(
            "{},{},{:.1},{},{},{},{}\n",
            t.format("%Y-%m-%d"),
            t.format("%H:%M"),
            s.secs as f64 / 60.0,
            esc(&data.subject_name(s.subject_id)),
            esc(&data.task(s.task_id).map(|t| t.title.clone()).unwrap_or_default()),
            s.kind,
            esc(&s.note)
        ));
    }
    std::fs::write(dir.join(format!("sessions-{stamp}.csv")), csv)?;
    std::fs::write(
        dir.join(format!("studytracker-{stamp}.json")),
        serde_json::to_string_pretty(data).map_err(std::io::Error::other)?,
    )?;
    Ok(dir)
}

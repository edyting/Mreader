//! Database model types.

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub id: String,
    pub title: String,
    pub path: String,
    pub format: String,
    pub word_count: i64,
    pub progress_word: i64,
    pub last_opened: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub document_id: String,
    pub paragraph_index: i64,
    pub word_index: i64,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingSession {
    pub id: String,
    pub document_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub words_read: i64,
    pub duration_secs: i64,
    pub avg_wpm: i64,
    pub peak_wpm: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStat {
    pub day: String,
    pub reading_secs: i64,
    pub words_read: i64,
    pub sessions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregateStats {
    pub total_reading_secs: i64,
    pub total_words: i64,
    pub average_wpm: f64,
    pub highest_wpm: i64,
    pub documents_completed: i64,
    pub session_count: i64,
    pub daily_goal_minutes: i64,
}

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub fn today_iso() -> String {
    Utc::now().date_naive().format("%Y-%m-%d").to_string()
}

pub fn parse_day(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

//! Reading session statistics tracking.

use std::time::{Duration, Instant};

use crate::database::{now_rfc3339, AggregateStats, ReadingSession, Storage};

#[derive(Debug)]
pub struct SessionTracker {
    pub document_id: String,
    pub session_id: String,
    elapsed_active: Duration,
    active_since: Option<Instant>,
    started_rfc: String,
    words_at_start: usize,
    pub peak_wpm: u32,
    pub current_wpm: u32,
    active: bool,
}

impl SessionTracker {
    pub fn start(document_id: &str, word_index: usize, wpm: u32) -> Self {
        Self {
            document_id: document_id.to_string(),
            session_id: uuid::Uuid::new_v4().to_string(),
            elapsed_active: Duration::ZERO,
            active_since: Some(Instant::now()),
            started_rfc: now_rfc3339(),
            words_at_start: word_index,
            peak_wpm: wpm,
            current_wpm: wpm,
            active: true,
        }
    }

    pub fn note_wpm(&mut self, wpm: u32) {
        self.current_wpm = wpm;
        if wpm > self.peak_wpm {
            self.peak_wpm = wpm;
        }
    }

    pub fn pause(&mut self) {
        if let Some(active_since) = self.active_since.take() {
            self.elapsed_active += active_since.elapsed();
        }
    }

    pub fn resume(&mut self) {
        if self.active_since.is_none() && self.active {
            self.active_since = Some(Instant::now());
        }
    }

    pub fn elapsed_secs(&self) -> i64 {
        let elapsed = self.elapsed_active
            + self
                .active_since
                .map(|active_since| active_since.elapsed())
                .unwrap_or_default();
        elapsed.as_secs() as i64
    }

    pub fn finish(
        &mut self,
        storage: &Storage,
        final_word_index: usize,
    ) -> anyhow::Result<ReadingSession> {
        self.active = false;
        self.pause();
        let words_read = final_word_index
            .saturating_sub(self.words_at_start)
            .saturating_add(1) as i64;
        let duration_secs = self.elapsed_secs().max(1);
        let avg_wpm = if duration_secs > 0 {
            ((words_read as f64) / (duration_secs as f64 / 60.0)).round() as i64
        } else {
            self.current_wpm as i64
        };
        let session = ReadingSession {
            id: self.session_id.clone(),
            document_id: self.document_id.clone(),
            started_at: self.started_rfc.clone(),
            ended_at: Some(now_rfc3339()),
            words_read,
            duration_secs,
            avg_wpm,
            peak_wpm: self.peak_wpm as i64,
        };
        storage.save_session(&session)?;
        storage.add_daily_reading(duration_secs, words_read)?;
        storage.record_speed_sample(avg_wpm.max(self.current_wpm as i64))?;
        Ok(session)
    }
}

pub fn format_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

pub fn summarize(storage: &Storage) -> anyhow::Result<AggregateStats> {
    storage.aggregate_stats()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_duration_parts() {
        assert_eq!(format_duration(65), "1m 5s");
        assert_eq!(format_duration(3661), "1h 1m");
    }

    #[test]
    fn counts_the_starting_word_in_a_session() {
        let storage = Storage::open_in_memory().unwrap();
        let mut tracker = SessionTracker::start("doc", 10, 200);
        let session = tracker.finish(&storage, 10).unwrap();
        assert_eq!(session.words_read, 1);
    }
}

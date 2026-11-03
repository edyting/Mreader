//! SQLite persistence.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use super::models::*;
use crate::config::AppConfig;

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn open_default() -> anyhow::Result<Self> {
        let dir = AppConfig::data_dir();
        std::fs::create_dir_all(&dir)?;
        let database_path = dir.join("mreader.db");
        let legacy_path = AppConfig::legacy_data_dir().join("rustpace.db");
        if !database_path.exists() && legacy_path.exists() {
            std::fs::copy(legacy_path, &database_path)?;
        }
        Self::open(&database_path)
    }

    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        let storage = Self { conn };
        storage.migrate()?;
        Ok(storage)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self { conn };
        storage.migrate()?;
        Ok(storage)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS library (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                format TEXT NOT NULL,
                word_count INTEGER NOT NULL DEFAULT 0,
                progress_word INTEGER NOT NULL DEFAULT 0,
                last_opened TEXT NOT NULL,
                completed INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS bookmarks (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                paragraph_index INTEGER NOT NULL,
                word_index INTEGER NOT NULL,
                note TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                FOREIGN KEY(document_id) REFERENCES library(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                words_read INTEGER NOT NULL DEFAULT 0,
                duration_secs INTEGER NOT NULL DEFAULT 0,
                avg_wpm INTEGER NOT NULL DEFAULT 0,
                peak_wpm INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS daily_stats (
                day TEXT PRIMARY KEY,
                reading_secs INTEGER NOT NULL DEFAULT 0,
                words_read INTEGER NOT NULL DEFAULT 0,
                sessions INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS speed_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                day TEXT NOT NULL,
                wpm INTEGER NOT NULL,
                recorded_at TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    pub fn upsert_library_entry(&self, entry: &LibraryEntry) -> anyhow::Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO library (id, title, path, format, word_count, progress_word, last_opened, completed)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(path) DO UPDATE SET
                title=excluded.title,
                format=excluded.format,
                word_count=excluded.word_count,
                progress_word=excluded.progress_word,
                last_opened=excluded.last_opened,
                completed=excluded.completed,
                id=excluded.id
            "#,
            params![
                entry.id,
                entry.title,
                entry.path,
                entry.format,
                entry.word_count,
                entry.progress_word,
                entry.last_opened,
                entry.completed as i64
            ],
        )?;
        Ok(())
    }

    pub fn update_progress(
        &self,
        document_id: &str,
        progress_word: i64,
        completed: bool,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE library SET progress_word=?1, completed=?2, last_opened=?3 WHERE id=?4",
            params![progress_word, completed as i64, now_rfc3339(), document_id],
        )?;
        Ok(())
    }

    pub fn recent_documents(&self, limit: i64) -> anyhow::Result<Vec<LibraryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, path, format, word_count, progress_word, last_opened, completed
             FROM library ORDER BY last_opened DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(LibraryEntry {
                id: row.get(0)?,
                title: row.get(1)?,
                path: row.get(2)?,
                format: row.get(3)?,
                word_count: row.get(4)?,
                progress_word: row.get(5)?,
                last_opened: row.get(6)?,
                completed: row.get::<_, i64>(7)? != 0,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn find_by_path(&self, path: &str) -> anyhow::Result<Option<LibraryEntry>> {
        self.conn
            .query_row(
                "SELECT id, title, path, format, word_count, progress_word, last_opened, completed
                 FROM library WHERE path=?1",
                params![path],
                |row| {
                    Ok(LibraryEntry {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        path: row.get(2)?,
                        format: row.get(3)?,
                        word_count: row.get(4)?,
                        progress_word: row.get(5)?,
                        last_opened: row.get(6)?,
                        completed: row.get::<_, i64>(7)? != 0,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn add_bookmark(&self, bm: &Bookmark) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO bookmarks (id, document_id, paragraph_index, word_index, note, created_at)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                bm.id,
                bm.document_id,
                bm.paragraph_index,
                bm.word_index,
                bm.note,
                bm.created_at
            ],
        )?;
        Ok(())
    }

    pub fn bookmarks_for(&self, document_id: &str) -> anyhow::Result<Vec<Bookmark>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, document_id, paragraph_index, word_index, note, created_at
             FROM bookmarks WHERE document_id=?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![document_id], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                document_id: row.get(1)?,
                paragraph_index: row.get(2)?,
                word_index: row.get(3)?,
                note: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn delete_bookmark(&self, id: &str) -> anyhow::Result<()> {
        self.conn
            .execute("DELETE FROM bookmarks WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn save_session(&self, session: &ReadingSession) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sessions
             (id, document_id, started_at, ended_at, words_read, duration_secs, avg_wpm, peak_wpm)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                session.id,
                session.document_id,
                session.started_at,
                session.ended_at,
                session.words_read,
                session.duration_secs,
                session.avg_wpm,
                session.peak_wpm
            ],
        )?;
        Ok(())
    }

    pub fn add_daily_reading(&self, secs: i64, words: i64) -> anyhow::Result<()> {
        let day = today_iso();
        self.conn.execute(
            r#"
            INSERT INTO daily_stats (day, reading_secs, words_read, sessions)
            VALUES (?1, ?2, ?3, 1)
            ON CONFLICT(day) DO UPDATE SET
                reading_secs = reading_secs + excluded.reading_secs,
                words_read = words_read + excluded.words_read,
                sessions = sessions + 1
            "#,
            params![day, secs, words],
        )?;
        Ok(())
    }

    pub fn record_speed_sample(&self, wpm: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO speed_samples (day, wpm, recorded_at) VALUES (?1, ?2, ?3)",
            params![today_iso(), wpm, now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn speed_history(&self, limit: i64) -> anyhow::Result<Vec<(String, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT recorded_at, wpm FROM speed_samples ORDER BY id DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut out: Vec<_> = rows.filter_map(|r| r.ok()).collect();
        out.reverse();
        Ok(out)
    }

    pub fn daily_stats(&self, limit: i64) -> anyhow::Result<Vec<DailyStat>> {
        let mut stmt = self.conn.prepare(
            "SELECT day, reading_secs, words_read, sessions FROM daily_stats
             ORDER BY day DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(DailyStat {
                day: row.get(0)?,
                reading_secs: row.get(1)?,
                words_read: row.get(2)?,
                sessions: row.get(3)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn aggregate_stats(&self) -> anyhow::Result<AggregateStats> {
        let total_reading_secs: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(duration_secs),0) FROM sessions",
            [],
            |r| r.get(0),
        )?;
        let total_words: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(words_read),0) FROM sessions",
            [],
            |r| r.get(0),
        )?;
        let session_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
        let highest_wpm: i64 =
            self.conn
                .query_row("SELECT COALESCE(MAX(peak_wpm),0) FROM sessions", [], |r| {
                    r.get(0)
                })?;
        let documents_completed: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM library WHERE completed=1", [], |r| {
                    r.get(0)
                })?;
        let average_wpm = if total_reading_secs > 0 {
            (total_words as f64 / (total_reading_secs as f64 / 60.0)).max(0.0)
        } else {
            0.0
        };
        Ok(AggregateStats {
            total_reading_secs,
            total_words,
            average_wpm,
            highest_wpm,
            documents_completed,
            session_count,
            daily_goal_minutes: 30,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_and_bookmarks_roundtrip() {
        let db = Storage::open_in_memory().unwrap();
        let entry = LibraryEntry {
            id: "doc1".into(),
            title: "Demo".into(),
            path: "/tmp/demo.txt".into(),
            format: "TXT".into(),
            word_count: 100,
            progress_word: 10,
            last_opened: now_rfc3339(),
            completed: false,
        };
        db.upsert_library_entry(&entry).unwrap();
        db.update_progress("doc1", 50, false).unwrap();
        let recent = db.recent_documents(5).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].progress_word, 50);

        let bm = Bookmark {
            id: "bm1".into(),
            document_id: "doc1".into(),
            paragraph_index: 2,
            word_index: 50,
            note: "important".into(),
            created_at: now_rfc3339(),
        };
        db.add_bookmark(&bm).unwrap();
        let bms = db.bookmarks_for("doc1").unwrap();
        assert_eq!(bms.len(), 1);
        assert_eq!(bms[0].note, "important");
    }

    #[test]
    fn sessions_and_stats() {
        let db = Storage::open_in_memory().unwrap();
        let session = ReadingSession {
            id: "s1".into(),
            document_id: "doc1".into(),
            started_at: now_rfc3339(),
            ended_at: Some(now_rfc3339()),
            words_read: 400,
            duration_secs: 120,
            avg_wpm: 200,
            peak_wpm: 300,
        };
        db.save_session(&session).unwrap();
        db.add_daily_reading(120, 400).unwrap();
        db.record_speed_sample(250).unwrap();
        let agg = db.aggregate_stats().unwrap();
        assert_eq!(agg.session_count, 1);
        assert_eq!(agg.highest_wpm, 300);
        assert!(!db.speed_history(10).unwrap().is_empty());
    }
}

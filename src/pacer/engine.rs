//! Core visual pacing engine.

use crate::document::{Document, Paragraph, WordToken};
use crate::pacer::modes::{advance_step, PacingMode};
use crate::pacer::timing::{ms_per_phrase, ms_per_word, PaceClock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaceState {
    Idle,
    Running,
    Paused,
    Finished,
}

#[derive(Debug, Clone)]
pub struct PaceSnapshot {
    pub word_index: usize,
    pub paragraph_index: usize,
    pub phrase_end: usize,
    pub state: PaceState,
    pub wpm: u32,
    pub mode: PacingMode,
    pub progress: f64,
    pub current_text: String,
}

#[derive(Debug)]
pub struct PacerEngine {
    pub mode: PacingMode,
    pub wpm: u32,
    pub phrase_size: usize,
    pub paragraph_pause_ms: u64,
    pub state: PaceState,
    pub word_index: usize,
    total_words: usize,
    paragraph_starts: Vec<usize>,
    words: Vec<String>,
    clock: PaceClock,
    pending_paragraph_pause: bool,
    paragraph_pause_deadline: Option<Instant>,
}

impl Default for PacerEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PacerEngine {
    pub fn new() -> Self {
        Self {
            mode: PacingMode::WordHighlight,
            wpm: 200,
            phrase_size: 3,
            paragraph_pause_ms: 400,
            state: PaceState::Idle,
            word_index: 0,
            total_words: 0,
            paragraph_starts: Vec::new(),
            words: Vec::new(),
            clock: PaceClock::new(),
            pending_paragraph_pause: false,
            paragraph_pause_deadline: None,
        }
    }

    pub fn load_document(&mut self, doc: &Document) {
        self.words.clear();
        self.paragraph_starts.clear();
        for (pi, para) in doc.paragraphs.iter().enumerate() {
            self.paragraph_starts.push(self.words.len());
            for token in &para.words {
                self.words.push(token.text.clone());
            }
            let _ = pi;
        }
        self.total_words = self.words.len();
        self.word_index = doc
            .progress_word_index
            .min(self.total_words.saturating_sub(1));
        self.state = PaceState::Idle;
        self.clock.stop();
        self.pending_paragraph_pause = false;
        self.paragraph_pause_deadline = None;
    }

    pub fn load_from_paragraphs(&mut self, paragraphs: &[Paragraph]) {
        self.words.clear();
        self.paragraph_starts.clear();
        for para in paragraphs {
            self.paragraph_starts.push(self.words.len());
            for token in &para.words {
                self.words.push(token.text.clone());
            }
        }
        self.total_words = self.words.len();
        self.word_index = 0;
        self.state = PaceState::Idle;
        self.clock.stop();
        self.pending_paragraph_pause = false;
        self.paragraph_pause_deadline = None;
    }

    pub fn set_wpm(&mut self, wpm: u32) {
        self.wpm = wpm.clamp(50, 1000);
    }

    pub fn start(&mut self) {
        if self.total_words == 0 {
            return;
        }
        if self.word_index >= self.total_words {
            self.word_index = 0;
        }
        self.state = PaceState::Running;
        self.clock.start();
        self.pending_paragraph_pause = false;
        self.paragraph_pause_deadline = None;
    }

    pub fn pause(&mut self) {
        if self.state == PaceState::Running {
            self.state = PaceState::Paused;
            self.clock.pause();
        }
    }

    pub fn resume(&mut self) {
        if self.state == PaceState::Paused {
            self.state = PaceState::Running;
            self.clock.resume();
        }
    }

    pub fn toggle_play_pause(&mut self) {
        match self.state {
            PaceState::Running => self.pause(),
            PaceState::Paused => self.resume(),
            PaceState::Idle | PaceState::Finished => self.start(),
        }
    }

    pub fn stop(&mut self) {
        self.state = PaceState::Idle;
        self.clock.stop();
        self.pending_paragraph_pause = false;
        self.paragraph_pause_deadline = None;
    }

    pub fn restart(&mut self) {
        self.word_index = 0;
        self.start();
    }

    pub fn next_word(&mut self) {
        let step = advance_step(self.mode, self.phrase_size);
        let next = self.word_index.saturating_add(step);
        if next >= self.total_words {
            self.word_index = self.total_words.saturating_sub(1);
            self.state = PaceState::Finished;
            self.clock.pause();
        } else {
            let crossed = self.crosses_paragraph(self.word_index, next);
            self.word_index = next;
            if crossed {
                self.begin_paragraph_pause();
            }
        }
    }

    pub fn prev_word(&mut self) {
        let step = advance_step(self.mode, self.phrase_size);
        self.word_index = self.word_index.saturating_sub(step);
        if self.state == PaceState::Finished {
            self.state = PaceState::Paused;
        }
    }

    pub fn next_paragraph(&mut self) {
        if let Some(idx) = self.paragraph_index_of(self.word_index) {
            if idx + 1 < self.paragraph_starts.len() {
                self.word_index = self.paragraph_starts[idx + 1];
            }
        }
    }

    pub fn prev_paragraph(&mut self) {
        if let Some(idx) = self.paragraph_index_of(self.word_index) {
            if idx > 0 {
                self.word_index = self.paragraph_starts[idx - 1];
            } else {
                self.word_index = 0;
            }
        }
    }

    pub fn jump_to_paragraph(&mut self, paragraph_index: usize) {
        if let Some(&start) = self.paragraph_starts.get(paragraph_index) {
            self.word_index = start;
        }
    }

    pub fn paragraph_index_of(&self, word_index: usize) -> Option<usize> {
        if self.paragraph_starts.is_empty() {
            return None;
        }
        let mut result = 0;
        for (i, &start) in self.paragraph_starts.iter().enumerate() {
            if start <= word_index {
                result = i;
            } else {
                break;
            }
        }
        Some(result)
    }

    fn crosses_paragraph(&self, from: usize, to: usize) -> bool {
        self.paragraph_index_of(from) != self.paragraph_index_of(to)
    }

    fn begin_paragraph_pause(&mut self) {
        if self.paragraph_pause_ms > 0 {
            self.pending_paragraph_pause = true;
            self.paragraph_pause_deadline =
                Some(Instant::now() + Duration::from_millis(self.paragraph_pause_ms));
            self.clock.pause();
        }
    }

    fn current_interval_ms(&self) -> f64 {
        match self.mode {
            PacingMode::PhrasePacing => {
                let remaining = self.total_words.saturating_sub(self.word_index);
                let count = self.phrase_size.min(remaining).max(1);
                ms_per_phrase(self.wpm, count)
            }
            _ => ms_per_word(self.wpm),
        }
    }

    /// Tick the engine using monotonic time. Returns true if the position changed.
    pub fn tick(&mut self) -> bool {
        if self.state != PaceState::Running || self.total_words == 0 {
            return false;
        }

        if self.pending_paragraph_pause {
            if self
                .paragraph_pause_deadline
                .is_some_and(|deadline| Instant::now() < deadline)
            {
                return false;
            }
            self.pending_paragraph_pause = false;
            self.paragraph_pause_deadline = None;
            self.clock.resume();
            return false;
        }

        let interval = self.current_interval_ms();
        let steps = self.clock.due_steps(interval);
        if steps == 0 {
            return false;
        }

        let mut changed = false;
        for _ in 0..steps {
            if self.word_index + 1 >= self.total_words {
                self.word_index = self.total_words.saturating_sub(1);
                self.state = PaceState::Finished;
                self.clock.pause();
                changed = true;
                break;
            }
            let next = self
                .word_index
                .saturating_add(advance_step(self.mode, self.phrase_size));
            if next >= self.total_words {
                self.word_index = self.total_words.saturating_sub(1);
                self.state = PaceState::Finished;
                self.clock.pause();
                changed = true;
                break;
            }
            let crossed = self.crosses_paragraph(self.word_index, next);
            self.word_index = next;
            changed = true;
            if crossed && self.paragraph_pause_ms > 0 {
                self.begin_paragraph_pause();
                break;
            }
        }
        changed
    }

    pub fn phrase_end(&self) -> usize {
        match self.mode {
            PacingMode::PhrasePacing => (self.word_index + self.phrase_size).min(self.total_words),
            _ => (self.word_index + 1).min(self.total_words),
        }
    }

    pub fn progress(&self) -> f64 {
        if self.total_words == 0 {
            0.0
        } else if self.state == PaceState::Finished {
            1.0
        } else {
            (self.word_index as f64 / self.total_words as f64).clamp(0.0, 1.0)
        }
    }

    pub fn total_words(&self) -> usize {
        self.total_words
    }

    pub fn current_word(&self) -> Option<&str> {
        self.words.get(self.word_index).map(String::as_str)
    }

    pub fn current_phrase(&self) -> String {
        let end = self.phrase_end();
        self.words[self.word_index..end].join(" ")
    }

    pub fn snapshot(&self) -> PaceSnapshot {
        PaceSnapshot {
            word_index: self.word_index,
            paragraph_index: self.paragraph_index_of(self.word_index).unwrap_or(0),
            phrase_end: self.phrase_end(),
            state: self.state,
            wpm: self.wpm,
            mode: self.mode,
            progress: self.progress(),
            current_text: if self.mode == PacingMode::PhrasePacing || self.mode == PacingMode::Rsvp
            {
                self.current_phrase()
            } else {
                self.current_word().unwrap_or("").to_string()
            },
        }
    }

    /// Optimal focal letter index for RSVP (ORP heuristic).
    pub fn focal_letter_index(word: &str) -> usize {
        let chars: Vec<char> = word.chars().collect();
        let len = chars.len();
        if len == 0 {
            return 0;
        }
        match len {
            1 => 0,
            2..=5 => 1.min(len - 1),
            6..=9 => 2.min(len - 1),
            _ => 3.min(len - 1),
        }
    }

    pub fn words_as_tokens(&self) -> Vec<WordToken> {
        self.words
            .iter()
            .enumerate()
            .map(|(i, t)| WordToken {
                text: t.clone(),
                global_index: i,
                paragraph_index: self.paragraph_index_of(i).unwrap_or(0),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DocumentFormat, ParagraphKind};

    fn sample_doc() -> Document {
        let mut doc = Document::new("Test", DocumentFormat::Txt, "test.txt");
        doc.paragraphs = vec![
            Paragraph::from_text(0, "Hello world from Mreader.", ParagraphKind::Body),
            Paragraph::from_text(1, "Second paragraph here now.", ParagraphKind::Body),
        ];
        doc.rebuild_word_indices();
        doc
    }

    #[test]
    fn loads_and_counts_words() {
        let doc = sample_doc();
        let mut engine = PacerEngine::new();
        engine.load_document(&doc);
        assert_eq!(engine.total_words(), 8);
    }

    #[test]
    fn does_not_advance_past_end() {
        let doc = sample_doc();
        let mut engine = PacerEngine::new();
        engine.load_document(&doc);
        engine.start();
        for _ in 0..100 {
            engine.next_word();
        }
        assert_eq!(engine.word_index, engine.total_words() - 1);
        assert_eq!(engine.state, PaceState::Finished);
    }

    #[test]
    fn phrase_grouping() {
        let doc = sample_doc();
        let mut engine = PacerEngine::new();
        engine.mode = PacingMode::PhrasePacing;
        engine.phrase_size = 3;
        engine.load_document(&doc);
        assert_eq!(engine.phrase_end(), 3);
        assert_eq!(engine.current_phrase(), "Hello world from");
    }

    #[test]
    fn focal_letter_heuristic() {
        assert_eq!(PacerEngine::focal_letter_index("a"), 0);
        assert_eq!(PacerEngine::focal_letter_index("cat"), 1);
        assert_eq!(PacerEngine::focal_letter_index("reading"), 2);
    }

    #[test]
    fn paragraph_navigation() {
        let doc = sample_doc();
        let mut engine = PacerEngine::new();
        engine.load_document(&doc);
        engine.next_paragraph();
        assert_eq!(engine.word_index, 4);
        engine.prev_paragraph();
        assert_eq!(engine.word_index, 0);
    }

    #[test]
    fn paragraph_pause_does_not_accumulate_pacer_time() {
        let doc = sample_doc();
        let mut engine = PacerEngine::new();
        engine.load_document(&doc);
        engine.paragraph_pause_ms = 1;
        engine.start();
        engine.begin_paragraph_pause();
        let paused_elapsed = engine.clock.elapsed();
        std::thread::sleep(Duration::from_millis(20));

        assert!(!engine.tick());
        assert_eq!(engine.word_index, 0);
        assert!(engine.clock.elapsed().saturating_sub(paused_elapsed) < Duration::from_millis(10));
        assert!(engine.clock.is_running());
    }
}

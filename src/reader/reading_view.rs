//! Focus reading view state helpers.

use crate::document::Document;
use crate::pacer::modes::PacingMode;
use crate::reader::text_layout::{build_pango_markup, DisplayDocument};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Focus,
    Original,
}

#[derive(Debug)]
pub struct ReadingViewState {
    pub view_mode: ViewMode,
    pub display: Option<DisplayDocument>,
    pub search_query: String,
    pub original_page: i32,
}

impl Default for ReadingViewState {
    fn default() -> Self {
        Self {
            view_mode: ViewMode::Focus,
            display: None,
            search_query: String::new(),
            original_page: 0,
        }
    }
}

impl ReadingViewState {
    pub fn load(&mut self, doc: &Document) {
        self.display = Some(DisplayDocument::from_document(doc));
        self.original_page = 0;
        if !doc.supports_original_view {
            self.view_mode = ViewMode::Focus;
        }
    }

    pub fn markup_for(
        &self,
        doc: &Document,
        start: usize,
        end: usize,
        mode: PacingMode,
        color: &str,
        opacity: f64,
    ) -> String {
        build_pango_markup(&doc.paragraphs, start, end, mode, color, opacity)
    }

    pub fn find_next(&self, from_word: usize) -> Option<usize> {
        let q = self.search_query.trim().to_lowercase();
        if q.is_empty() {
            return None;
        }
        let display = self.display.as_ref()?;
        if display.word_spans.is_empty() {
            return None;
        }
        // Prefer span-based search against document words via plain reconstruction
        for i in (from_word + 1)..display.word_spans.len() {
            let (a, b) = display.word_spans[i];
            if display.plain[a..b].to_lowercase().contains(&q) {
                return Some(i);
            }
        }
        for i in 0..=from_word.min(display.word_spans.len().saturating_sub(1)) {
            let (a, b) = display.word_spans[i];
            if display.plain[a..b].to_lowercase().contains(&q) {
                return Some(i);
            }
        }
        None
    }
}

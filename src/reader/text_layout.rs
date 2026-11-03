//! Helpers for mapping word indices to display text offsets.

use crate::document::{Document, Paragraph, ParagraphKind};
use crate::pacer::modes::PacingMode;

#[derive(Debug, Clone)]
pub struct DisplayDocument {
    pub plain: String,
    /// Byte range [start, end) for each global word index in `plain`.
    pub word_spans: Vec<(usize, usize)>,
    pub paragraph_spans: Vec<(usize, usize)>,
}

impl DisplayDocument {
    pub fn from_document(doc: &Document) -> Self {
        let mut plain = String::new();
        let mut word_spans = Vec::new();
        let mut paragraph_spans = Vec::new();

        for (i, para) in doc.paragraphs.iter().enumerate() {
            if i > 0 {
                plain.push('\n');
                plain.push('\n');
            }
            let para_start = plain.len();
            let prefix = match para.kind {
                ParagraphKind::Heading1 => "",
                ParagraphKind::Heading2 => "",
                ParagraphKind::Heading3 => "",
                ParagraphKind::ListItem => {
                    if !(para.text.starts_with('-')
                        || para.text.starts_with('*')
                        || para.text.starts_with('•'))
                    {
                        "• "
                    } else {
                        ""
                    }
                }
                _ => "",
            };
            if !prefix.is_empty() {
                plain.push_str(prefix);
            }

            for (wi, word) in para.words.iter().enumerate() {
                if wi > 0 {
                    plain.push(' ');
                }
                let start = plain.len();
                plain.push_str(&word.text);
                let end = plain.len();
                word_spans.push((start, end));
            }
            let para_end = plain.len();
            paragraph_spans.push((para_start, para_end));
        }

        Self {
            plain,
            word_spans,
            paragraph_spans,
        }
    }

    pub fn span_for_range(&self, start_word: usize, end_word: usize) -> Option<(usize, usize)> {
        if self.word_spans.is_empty() || start_word >= self.word_spans.len() {
            return None;
        }
        let end_word = end_word.min(self.word_spans.len()).max(start_word + 1);
        let start = self.word_spans[start_word].0;
        let end = self.word_spans[end_word - 1].1;
        Some((start, end))
    }

    pub fn character_offset(&self, byte_offset: usize) -> usize {
        self.plain[..byte_offset].chars().count()
    }
}

pub fn paragraph_css_class(kind: ParagraphKind) -> &'static str {
    match kind {
        ParagraphKind::Heading1 => "heading1",
        ParagraphKind::Heading2 => "heading2",
        ParagraphKind::Heading3 => "heading3",
        ParagraphKind::ListItem => "list-item",
        ParagraphKind::Quote => "quote",
        ParagraphKind::Body => "body",
    }
}

pub fn build_pango_markup(
    paragraphs: &[Paragraph],
    active_start: usize,
    active_end: usize,
    mode: PacingMode,
    highlight_color: &str,
    opacity: f64,
) -> String {
    let mut out = String::new();
    let alpha = ((opacity.clamp(0.0, 1.0)) * 255.0).round() as u8;
    let bg = format!("{highlight_color}{alpha:02X}");

    for para in paragraphs {
        let weight = match para.kind {
            ParagraphKind::Heading1 => "bold",
            ParagraphKind::Heading2 => "bold",
            ParagraphKind::Heading3 => "bold",
            _ => "normal",
        };
        let size = match para.kind {
            ParagraphKind::Heading1 => "xx-large",
            ParagraphKind::Heading2 => "x-large",
            ParagraphKind::Heading3 => "large",
            _ => "medium",
        };
        out.push_str(&format!("<span weight=\"{weight}\" size=\"{size}\">"));
        for (i, word) in para.words.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            let gi = word.global_index;
            let active = gi >= active_start && gi < active_end;
            let escaped = glib::markup_escape_text(&word.text);
            match mode {
                PacingMode::WordHighlight | PacingMode::PhrasePacing if active => {
                    out.push_str(&format!("<span background=\"{bg}\">{escaped}</span>"));
                }
                PacingMode::MovingUnderline if active => {
                    out.push_str(&format!("<span underline=\"single\">{escaped}</span>"));
                }
                PacingMode::ReadingPointer if active => {
                    out.push_str(&format!("▶<span weight=\"bold\">{escaped}</span>"));
                }
                _ => out.push_str(&escaped),
            }
        }
        out.push_str("</span>\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DocumentFormat, ParagraphKind};

    #[test]
    fn word_spans_map_correctly() {
        let mut doc = crate::document::Document::new("t", DocumentFormat::Txt, "t.txt");
        doc.paragraphs = vec![
            Paragraph::from_text(0, "One two", ParagraphKind::Body),
            Paragraph::from_text(1, "Three", ParagraphKind::Body),
        ];
        doc.rebuild_word_indices();
        let layout = DisplayDocument::from_document(&doc);
        assert_eq!(layout.word_spans.len(), 3);
        assert_eq!(
            &layout.plain[layout.word_spans[0].0..layout.word_spans[0].1],
            "One"
        );
        assert_eq!(
            &layout.plain[layout.word_spans[2].0..layout.word_spans[2].1],
            "Three"
        );
    }

    #[test]
    fn byte_spans_convert_to_text_buffer_character_offsets() {
        let mut doc = crate::document::Document::new("t", DocumentFormat::Txt, "t.txt");
        doc.paragraphs = vec![Paragraph::from_text(0, "Café déjà vu", ParagraphKind::Body)];
        doc.rebuild_word_indices();
        let layout = DisplayDocument::from_document(&doc);
        let (start, end) = layout.word_spans[1];
        assert_eq!(&layout.plain[start..end], "déjà");
        assert_eq!(layout.character_offset(start), 5);
        assert_eq!(layout.character_offset(end), 9);
    }
}

//! Document loading and text extraction.

pub mod docx;
pub mod epub;
pub mod html;
pub mod pdf;
pub mod rtf;
pub mod text;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DocumentError {
    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),
    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse document: {0}")]
    Parse(String),
    #[error("Document appears corrupted or empty")]
    Corrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentFormat {
    Pdf,
    Docx,
    Epub,
    Txt,
    Markdown,
    Html,
    Rtf,
}

impl DocumentFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "pdf" => Some(Self::Pdf),
            "docx" => Some(Self::Docx),
            "epub" => Some(Self::Epub),
            "txt" => Some(Self::Txt),
            "md" | "markdown" => Some(Self::Markdown),
            "html" | "htm" => Some(Self::Html),
            "rtf" => Some(Self::Rtf),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Docx => "Word",
            Self::Epub => "EPUB",
            Self::Txt => "Text",
            Self::Markdown => "Markdown",
            Self::Html => "HTML",
            Self::Rtf => "RTF",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParagraphKind {
    Body,
    Heading1,
    Heading2,
    Heading3,
    ListItem,
    Quote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordToken {
    pub text: String,
    pub global_index: usize,
    pub paragraph_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paragraph {
    pub index: usize,
    pub kind: ParagraphKind,
    pub text: String,
    pub words: Vec<WordToken>,
    pub chapter_index: Option<usize>,
    pub page_index: Option<usize>,
}

impl Paragraph {
    pub fn from_text(index: usize, text: &str, kind: ParagraphKind) -> Self {
        let words: Vec<WordToken> = tokenize_words(text)
            .into_iter()
            .map(|t| WordToken {
                text: t,
                global_index: 0,
                paragraph_index: index,
            })
            .collect();
        Self {
            index,
            kind,
            text: text.to_string(),
            words,
            chapter_index: None,
            page_index: None,
        }
    }

    pub fn word_count(&self) -> usize {
        self.words.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub index: usize,
    pub title: String,
    pub paragraph_start: usize,
    pub paragraph_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub page_count: Option<usize>,
}

impl Default for DocumentMeta {
    fn default() -> Self {
        Self {
            title: None,
            author: None,
            language: None,
            page_count: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub format: DocumentFormat,
    pub meta: DocumentMeta,
    pub paragraphs: Vec<Paragraph>,
    pub chapters: Vec<Chapter>,
    pub progress_word_index: usize,
    pub supports_original_view: bool,
}

impl Document {
    pub fn new(title: &str, format: DocumentFormat, path: impl Into<PathBuf>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.to_string(),
            path: path.into(),
            format,
            meta: DocumentMeta::default(),
            paragraphs: Vec::new(),
            chapters: Vec::new(),
            progress_word_index: 0,
            supports_original_view: format == DocumentFormat::Pdf,
        }
    }

    pub fn rebuild_word_indices(&mut self) {
        let mut global = 0usize;
        for (pi, para) in self.paragraphs.iter_mut().enumerate() {
            para.index = pi;
            for word in &mut para.words {
                word.global_index = global;
                word.paragraph_index = pi;
                global += 1;
            }
        }
    }

    pub fn total_words(&self) -> usize {
        self.paragraphs.iter().map(|p| p.word_count()).sum()
    }

    pub fn plain_text(&self) -> String {
        self.paragraphs
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn word_at(&self, global_index: usize) -> Option<&WordToken> {
        for para in &self.paragraphs {
            for word in &para.words {
                if word.global_index == global_index {
                    return Some(word);
                }
            }
        }
        None
    }
}

/// Split text into word tokens, keeping punctuation attached where natural.
pub fn tokenize_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

pub fn count_words(text: &str) -> usize {
    tokenize_words(text).len()
}

/// Build paragraphs from a block of text, preserving blank-line boundaries.
pub fn paragraphs_from_text(text: &str) -> Vec<Paragraph> {
    let mut paragraphs = Vec::new();
    let mut buffer = String::new();
    let mut index = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !buffer.trim().is_empty() {
                let kind = detect_paragraph_kind(&buffer);
                paragraphs.push(Paragraph::from_text(index, buffer.trim(), kind));
                index += 1;
                buffer.clear();
            }
        } else if looks_like_list_item(trimmed) {
            if !buffer.trim().is_empty() {
                let kind = detect_paragraph_kind(&buffer);
                paragraphs.push(Paragraph::from_text(index, buffer.trim(), kind));
                index += 1;
                buffer.clear();
            }
            paragraphs.push(Paragraph::from_text(
                index,
                trimmed,
                ParagraphKind::ListItem,
            ));
            index += 1;
        } else {
            if !buffer.is_empty() {
                buffer.push(' ');
            }
            buffer.push_str(trimmed);
        }
    }

    if !buffer.trim().is_empty() {
        let kind = detect_paragraph_kind(&buffer);
        paragraphs.push(Paragraph::from_text(index, buffer.trim(), kind));
    }

    paragraphs
}

fn looks_like_list_item(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    if matches!(bytes[0], b'-' | b'*' | b'+') && bytes.len() > 1 && bytes[1] == b' ' {
        return true;
    }
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    i > 0 && i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1] == b' '
}

fn detect_paragraph_kind(text: &str) -> ParagraphKind {
    let t = text.trim();
    if t.starts_with('#') {
        let level = t.chars().take_while(|c| *c == '#').count();
        return match level {
            1 => ParagraphKind::Heading1,
            2 => ParagraphKind::Heading2,
            _ => ParagraphKind::Heading3,
        };
    }
    if t.len() < 80
        && !t.ends_with('.')
        && t.chars().any(|c| c.is_alphabetic())
        && t == t.to_uppercase()
        && t.split_whitespace().count() <= 12
    {
        return ParagraphKind::Heading2;
    }
    ParagraphKind::Body
}

/// Open and parse a document from disk. Never panics on bad input.
pub fn open_document(path: &Path) -> Result<Document, DocumentError> {
    let format = DocumentFormat::from_path(path).ok_or_else(|| {
        DocumentError::UnsupportedFormat(
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown")
                .to_string(),
        )
    })?;

    let result = match format {
        DocumentFormat::Pdf => pdf::load(path),
        DocumentFormat::Docx => docx::load(path),
        DocumentFormat::Epub => epub::load(path),
        DocumentFormat::Txt => text::load_txt(path),
        DocumentFormat::Markdown => text::load_markdown(path),
        DocumentFormat::Html => html::load(path),
        DocumentFormat::Rtf => rtf::load(path),
    };

    match result {
        Ok(mut doc) => {
            if doc.paragraphs.is_empty() {
                return Err(DocumentError::Corrupted);
            }
            doc.rebuild_word_indices();
            if doc.title.trim().is_empty() {
                doc.title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string();
            }
            Ok(doc)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_basic() {
        assert_eq!(count_words("Hello, brave world!"), 3);
    }

    #[test]
    fn paragraphs_preserve_lists() {
        let text = "Intro paragraph.\n\n- item one\n- item two\n\nClosing.";
        let paras = paragraphs_from_text(text);
        assert!(paras.iter().any(|p| p.kind == ParagraphKind::ListItem));
        assert!(paras.len() >= 4);
    }

    #[test]
    fn format_from_path() {
        assert_eq!(
            DocumentFormat::from_path(Path::new("a.PDF")),
            Some(DocumentFormat::Pdf)
        );
        assert_eq!(DocumentFormat::from_path(Path::new("x.xyz")), None);
    }
}

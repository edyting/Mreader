//! Plain text and Markdown loaders.

use std::path::Path;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use super::{
    paragraphs_from_text, Chapter, Document, DocumentError, DocumentFormat, DocumentMeta,
    Paragraph, ParagraphKind,
};

pub fn load_txt(path: &Path) -> Result<Document, DocumentError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            let bytes = std::fs::read(path)?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
        Err(e) => return Err(DocumentError::Io(e)),
    };

    if text.trim().is_empty() {
        return Err(DocumentError::Corrupted);
    }

    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let mut doc = Document::new(&title, DocumentFormat::Txt, path);
    doc.meta = DocumentMeta {
        title: Some(title.clone()),
        ..Default::default()
    };
    doc.paragraphs = paragraphs_from_text(&text);
    doc.supports_original_view = false;
    Ok(doc)
}

pub fn load_markdown(path: &Path) -> Result<Document, DocumentError> {
    let content = std::fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Err(DocumentError::Corrupted);
    }

    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let mut doc = Document::new(&title, DocumentFormat::Markdown, path);
    doc.meta.title = Some(title);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(&content, options);
    let mut paragraphs: Vec<Paragraph> = Vec::new();
    let mut chapters: Vec<Chapter> = Vec::new();
    let mut current = String::new();
    let mut current_kind = ParagraphKind::Body;
    let mut index = 0usize;

    let flush = |paragraphs: &mut Vec<Paragraph>,
                 current: &mut String,
                 kind: ParagraphKind,
                 index: &mut usize| {
        let text = current.trim();
        if !text.is_empty() {
            paragraphs.push(Paragraph::from_text(*index, text, kind));
            *index += 1;
        }
        current.clear();
    };

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut paragraphs, &mut current, current_kind, &mut index);
                current_kind = match level {
                    pulldown_cmark::HeadingLevel::H1 => ParagraphKind::Heading1,
                    pulldown_cmark::HeadingLevel::H2 => ParagraphKind::Heading2,
                    _ => ParagraphKind::Heading3,
                };
            }
            Event::End(TagEnd::Heading(_)) => {
                let heading_text = current.trim().to_string();
                let para_index = index;
                flush(&mut paragraphs, &mut current, current_kind, &mut index);
                if matches!(
                    current_kind,
                    ParagraphKind::Heading1 | ParagraphKind::Heading2
                ) {
                    if let Some(last) = chapters.last_mut() {
                        last.paragraph_end = para_index;
                    }
                    chapters.push(Chapter {
                        index: chapters.len(),
                        title: heading_text,
                        paragraph_start: para_index,
                        paragraph_end: para_index,
                    });
                }
                current_kind = ParagraphKind::Body;
            }
            Event::Start(Tag::Item) => {
                flush(&mut paragraphs, &mut current, current_kind, &mut index);
                current_kind = ParagraphKind::ListItem;
            }
            Event::End(TagEnd::Item) => {
                flush(&mut paragraphs, &mut current, current_kind, &mut index);
                current_kind = ParagraphKind::Body;
            }
            Event::Start(Tag::Paragraph) | Event::Start(Tag::BlockQuote(_)) => {
                flush(&mut paragraphs, &mut current, current_kind, &mut index);
            }
            Event::End(TagEnd::Paragraph) | Event::End(TagEnd::BlockQuote(_)) => {
                flush(&mut paragraphs, &mut current, current_kind, &mut index);
            }
            Event::Text(t) | Event::Code(t) => {
                if !current.is_empty() && !current.ends_with(' ') {
                    current.push(' ');
                }
                current.push_str(&t);
            }
            Event::SoftBreak | Event::HardBreak => {
                current.push(' ');
            }
            _ => {}
        }
    }
    flush(&mut paragraphs, &mut current, current_kind, &mut index);

    if let Some(last) = chapters.last_mut() {
        last.paragraph_end = paragraphs.len();
    }

    if paragraphs.is_empty() {
        paragraphs = paragraphs_from_text(&content);
    }

    doc.paragraphs = paragraphs;
    doc.chapters = chapters;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn load_simple_txt() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "Hello world.\n\nSecond paragraph.").unwrap();
        let doc = load_txt(f.path()).unwrap();
        assert_eq!(doc.paragraphs.len(), 2);
        assert_eq!(doc.total_words(), 4);
    }

    #[test]
    fn load_markdown_headings() {
        let mut f = NamedTempFile::with_suffix(".md").unwrap();
        writeln!(
            f,
            "# Title\n\nHello **world**.\n\n## Section\n\n- one\n- two"
        )
        .unwrap();
        let doc = load_markdown(f.path()).unwrap();
        assert!(doc.paragraphs.len() >= 3);
        assert!(!doc.chapters.is_empty());
    }

    #[test]
    fn empty_txt_is_corrupted() {
        let f = NamedTempFile::new().unwrap();
        let err = load_txt(f.path()).unwrap_err();
        assert!(matches!(err, DocumentError::Corrupted));
    }
}

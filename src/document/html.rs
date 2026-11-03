//! HTML document loading with sanitisation.

use std::path::Path;

use html5ever::tendril::TendrilSink;
use html5ever::{parse_document, ParseOpts};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use super::{
    paragraphs_from_text, Document, DocumentError, DocumentFormat, Paragraph, ParagraphKind,
};

pub fn load(path: &Path) -> Result<Document, DocumentError> {
    let content = std::fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Err(DocumentError::Corrupted);
    }
    parse_html_string(&content, path)
}

pub fn parse_html_string(content: &str, path: &Path) -> Result<Document, DocumentError> {
    let dom = parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut content.as_bytes())
        .map_err(|e| DocumentError::Parse(e.to_string()))?;

    let mut title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();
    let mut blocks: Vec<(ParagraphKind, String)> = Vec::new();
    walk(&dom.document, &mut title, &mut blocks, false);

    let mut doc = Document::new(&title, DocumentFormat::Html, path);
    doc.meta.title = Some(title.clone());

    let mut paragraphs = Vec::new();
    for (i, (kind, text)) in blocks.into_iter().enumerate() {
        let cleaned = collapse_ws(&text);
        if !cleaned.is_empty() {
            paragraphs.push(Paragraph::from_text(i, &cleaned, kind));
        }
    }

    if paragraphs.is_empty() {
        // Fallback strip tags roughly
        let stripped = strip_tags(content);
        paragraphs = paragraphs_from_text(&stripped);
    }

    if paragraphs.is_empty() {
        return Err(DocumentError::Corrupted);
    }

    doc.paragraphs = paragraphs;
    Ok(doc)
}

fn walk(
    handle: &Handle,
    title: &mut String,
    blocks: &mut Vec<(ParagraphKind, String)>,
    in_title: bool,
) {
    let node = handle;
    match &node.data {
        NodeData::Element { name, .. } => {
            let tag = name.local.as_ref();
            // Skip non-content
            if matches!(tag, "script" | "style" | "noscript" | "svg" | "head") {
                if tag == "head" {
                    for child in node.children.borrow().iter() {
                        walk_title_only(child, title);
                    }
                }
                return;
            }

            let kind = match tag {
                "h1" => Some(ParagraphKind::Heading1),
                "h2" => Some(ParagraphKind::Heading2),
                "h3" | "h4" | "h5" | "h6" => Some(ParagraphKind::Heading3),
                "li" => Some(ParagraphKind::ListItem),
                "p" | "blockquote" | "div" | "section" | "article" => Some(ParagraphKind::Body),
                "br" => {
                    if let Some((_, last)) = blocks.last_mut() {
                        last.push(' ');
                    }
                    None
                }
                _ => None,
            };

            if let Some(k) = kind {
                let mut text = String::new();
                collect_text(node, &mut text);
                let cleaned = collapse_ws(&text);
                if !cleaned.is_empty() {
                    if k == ParagraphKind::Heading1 && title == "Untitled" {
                        *title = cleaned.clone();
                    }
                    blocks.push((k, cleaned));
                }
                return; // don't double-walk children
            }

            for child in node.children.borrow().iter() {
                walk(child, title, blocks, in_title);
            }
        }
        NodeData::Text { contents } => {
            if in_title {
                let t = contents.borrow();
                if !t.trim().is_empty() {
                    *title = collapse_ws(&t);
                }
            }
        }
        _ => {
            for child in node.children.borrow().iter() {
                walk(child, title, blocks, in_title);
            }
        }
    }
}

fn walk_title_only(handle: &Handle, title: &mut String) {
    match &handle.data {
        NodeData::Element { name, .. } if name.local.as_ref() == "title" => {
            let mut text = String::new();
            collect_text(handle, &mut text);
            let cleaned = collapse_ws(&text);
            if !cleaned.is_empty() {
                *title = cleaned;
            }
        }
        _ => {
            for child in handle.children.borrow().iter() {
                walk_title_only(child, title);
            }
        }
    }
}

fn collect_text(handle: &Handle, out: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => {
            out.push_str(&contents.borrow());
        }
        NodeData::Element { name, .. } => {
            let tag = name.local.as_ref();
            if matches!(tag, "script" | "style") {
                return;
            }
            if tag == "br" {
                out.push(' ');
            }
            for child in handle.children.borrow().iter() {
                collect_text(child, out);
            }
            if matches!(tag, "p" | "div" | "li" | "h1" | "h2" | "h3" | "tr") {
                out.push(' ');
            }
        }
        _ => {
            for child in handle.children.borrow().iter() {
                collect_text(child, out);
            }
        }
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn parse_simple_html() {
        let mut f = NamedTempFile::with_suffix(".html").unwrap();
        writeln!(
            f,
            "<html><head><title>Demo</title></head><body><h1>Hello</h1><p>World of rust.</p><ul><li>One</li></ul></body></html>"
        )
        .unwrap();
        let doc = load(f.path()).unwrap();
        assert_eq!(doc.meta.title.as_deref(), Some("Demo"));
        assert!(doc.total_words() >= 4);
    }

    #[test]
    fn corrupted_empty() {
        let f = NamedTempFile::with_suffix(".html").unwrap();
        assert!(matches!(load(f.path()), Err(DocumentError::Corrupted)));
    }
}

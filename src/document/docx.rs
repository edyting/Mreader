//! DOCX extraction via ZIP + WordprocessingML XML.

use std::io::{Read, Write};
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;
use zip::ZipArchive;

use super::{Document, DocumentError, DocumentFormat, Paragraph, ParagraphKind};

pub fn load(path: &Path) -> Result<Document, DocumentError> {
    let file = std::fs::File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| DocumentError::Parse(format!("Invalid DOCX/ZIP: {e}")))?;

    let mut xml = String::new();
    {
        let mut doc_xml = archive
            .by_name("word/document.xml")
            .map_err(|_| DocumentError::Parse("Missing word/document.xml".into()))?;
        doc_xml
            .read_to_string(&mut xml)
            .map_err(|e| DocumentError::Parse(e.to_string()))?;
    }

    let title = extract_core_title(&mut archive).unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    });

    let paragraphs = parse_document_xml(&xml)?;
    if paragraphs.is_empty() {
        return Err(DocumentError::Corrupted);
    }

    let mut doc = Document::new(&title, DocumentFormat::Docx, path);
    doc.meta.title = Some(title);
    doc.paragraphs = paragraphs;
    doc.supports_original_view = false;
    Ok(doc)
}

fn extract_core_title(archive: &mut ZipArchive<std::fs::File>) -> Option<String> {
    let mut file = archive.by_name("docProps/core.xml").ok()?;
    let mut xml = String::new();
    file.read_to_string(&mut xml).ok()?;
    let start = xml.find("<dc:title>")? + "<dc:title>".len();
    let end = xml[start..].find("</dc:title>")? + start;
    let title = xml[start..end].trim();
    if title.is_empty() {
        None
    } else {
        Some(unescape_xml(title))
    }
}

fn parse_document_xml(xml: &str) -> Result<Vec<Paragraph>, DocumentError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut in_paragraph = false;
    let mut in_text = false;
    let mut style = String::new();
    let mut index = 0usize;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match name.as_str() {
                    "p" => {
                        in_paragraph = true;
                        current.clear();
                        style.clear();
                    }
                    "pStyle" => {
                        for a in e.attributes().flatten() {
                            let local = a.key.local_name();
                            let key = String::from_utf8_lossy(local.as_ref()).into_owned();
                            if key == "val" {
                                style = String::from_utf8_lossy(&a.value).into_owned();
                            }
                        }
                    }
                    "t" => in_text = true,
                    "tab" if in_paragraph => current.push('\t'),
                    "br" | "cr" if in_paragraph => current.push(' '),
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match name.as_str() {
                    "tab" if in_paragraph => current.push('\t'),
                    "br" | "cr" if in_paragraph => current.push(' '),
                    "pStyle" => {
                        for a in e.attributes().flatten() {
                            let local = a.key.local_name();
                            let key = String::from_utf8_lossy(local.as_ref()).into_owned();
                            if key == "val" {
                                style = String::from_utf8_lossy(&a.value).into_owned();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) if in_text && in_paragraph => {
                let text = t.decode().unwrap_or_default();
                current.push_str(&text);
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match name.as_str() {
                    "t" => in_text = false,
                    "p" => {
                        in_paragraph = false;
                        let text = current.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !text.is_empty() {
                            let kind = style_to_kind(&style, &text);
                            paragraphs.push(Paragraph::from_text(index, &text, kind));
                            index += 1;
                        }
                        current.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocumentError::Parse(format!("XML error: {e}"))),
            _ => {}
        }
        buf.clear();
    }

    Ok(paragraphs)
}

fn style_to_kind(style: &str, text: &str) -> ParagraphKind {
    let s = style.to_lowercase();
    if s.contains("heading1") || s == "title" {
        ParagraphKind::Heading1
    } else if s.contains("heading2") {
        ParagraphKind::Heading2
    } else if s.contains("heading") {
        ParagraphKind::Heading3
    } else if s.contains("list") || text.starts_with('•') {
        ParagraphKind::ListItem
    } else {
        ParagraphKind::Body
    }
}

fn unescape_xml(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Write a minimal DOCX for tests / original-view fallback note.
#[allow(dead_code)]
pub fn write_plaintext_fallback(path: &Path, text: &str) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    f.write_all(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_document_xml() {
        let xml = r#"<?xml version="1.0"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
          <w:body>
            <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p>
            <w:p><w:r><w:t>Hello world from DOCX.</w:t></w:r></w:p>
          </w:body>
        </w:document>"#;
        let paras = parse_document_xml(xml).unwrap();
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0].kind, ParagraphKind::Heading1);
        assert_eq!(paras[1].words.len(), 4);
    }

    #[test]
    fn bad_zip_is_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.docx");
        std::fs::write(&path, b"not a zip").unwrap();
        let err = load(&path).unwrap_err();
        assert!(matches!(err, DocumentError::Parse(_)));
    }
}

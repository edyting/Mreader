//! EPUB loading.

use std::path::Path;

use epub::doc::EpubDoc;

use super::{
    html, paragraphs_from_text, Chapter, Document, DocumentError, DocumentFormat, Paragraph,
    ParagraphKind,
};

pub fn load(path: &Path) -> Result<Document, DocumentError> {
    let mut epub = EpubDoc::new(path).map_err(|e| DocumentError::Parse(format!("EPUB: {e}")))?;

    let title = epub
        .get_title()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });
    let author = epub.mdata("creator").map(|m| m.value.clone());
    let language = epub.mdata("language").map(|m| m.value.clone());

    let mut paragraphs: Vec<Paragraph> = Vec::new();
    let mut chapters: Vec<Chapter> = Vec::new();
    let spine_len = epub.get_num_chapters();

    for spine_idx in 0..spine_len {
        if !epub.set_current_chapter(spine_idx) {
            continue;
        }
        let chapter_title = epub
            .get_current_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("Chapter {}", spine_idx + 1));

        let content = match epub.get_current_str() {
            Some((data, _mime)) => data,
            None => continue,
        };

        let chapter_start = paragraphs.len();
        let mut chapter_paras = extract_from_xhtml(&content, path);

        if chapter_paras.is_empty() {
            let stripped = strip_tags(&content);
            chapter_paras = paragraphs_from_text(&stripped);
        }

        // Prepend chapter heading if useful
        if !chapter_paras
            .first()
            .map(|p| matches!(p.kind, ParagraphKind::Heading1 | ParagraphKind::Heading2))
            .unwrap_or(false)
        {
            let nice = humanize_id(&chapter_title);
            paragraphs.push(Paragraph::from_text(
                paragraphs.len(),
                &nice,
                ParagraphKind::Heading2,
            ));
        }

        for mut p in chapter_paras {
            p.chapter_index = Some(spine_idx);
            p.index = paragraphs.len();
            paragraphs.push(p);
        }

        chapters.push(Chapter {
            index: spine_idx,
            title: humanize_id(&chapter_title),
            paragraph_start: chapter_start,
            paragraph_end: paragraphs.len(),
        });
    }

    if paragraphs.is_empty() {
        return Err(DocumentError::Corrupted);
    }

    let mut doc = Document::new(&title, DocumentFormat::Epub, path);
    doc.meta.title = Some(title);
    doc.meta.author = author;
    doc.meta.language = language;
    doc.paragraphs = paragraphs;
    doc.chapters = chapters;
    doc.supports_original_view = false;
    Ok(doc)
}

fn extract_from_xhtml(content: &str, path: &Path) -> Vec<Paragraph> {
    match html::parse_html_string(content, path) {
        Ok(doc) => doc.paragraphs,
        Err(_) => paragraphs_from_text(&strip_tags(content)),
    }
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
    // decode a few entities
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

fn humanize_id(id: &str) -> String {
    let base = Path::new(id)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(id);
    base.replace(['_', '-'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_epub_errors() {
        let err = load(Path::new("/tmp/nope-rustpace.epub")).unwrap_err();
        assert!(matches!(
            err,
            DocumentError::Io(_) | DocumentError::Parse(_)
        ));
    }
}

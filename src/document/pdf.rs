//! PDF text extraction and page rendering via Poppler.

use std::path::Path;

use super::{paragraphs_from_text, Chapter, Document, DocumentError, DocumentFormat, Paragraph};

pub fn load(path: &Path) -> Result<Document, DocumentError> {
    let uri = path_to_uri(path)?;
    let poppler_doc = poppler::Document::from_file(&uri, None)
        .map_err(|e| DocumentError::Parse(format!("Unable to open PDF: {e}")))?;

    let n_pages = poppler_doc.n_pages();
    if n_pages <= 0 {
        return Err(DocumentError::Corrupted);
    }

    let title = poppler_doc
        .title()
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });

    let author = poppler_doc.author().map(|s| s.to_string());

    let mut paragraphs: Vec<Paragraph> = Vec::new();
    let mut chapters: Vec<Chapter> = Vec::new();
    let mut para_index = 0usize;

    for page_idx in 0..n_pages {
        let Some(page) = poppler_doc.page(page_idx) else {
            continue;
        };
        let text = page.text().map(|s| s.to_string()).unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }

        let chapter_start = para_index;
        let page_paras = paragraphs_from_text(&text);
        for mut p in page_paras {
            p.page_index = Some(page_idx as usize);
            p.chapter_index = Some(page_idx as usize);
            p.index = para_index;
            paragraphs.push(p);
            para_index += 1;
        }

        chapters.push(Chapter {
            index: page_idx as usize,
            title: format!("Page {}", page_idx + 1),
            paragraph_start: chapter_start,
            paragraph_end: para_index,
        });
    }

    if paragraphs.is_empty() {
        return Err(DocumentError::Parse(
            "PDF contains no extractable text (it may be image-only)".into(),
        ));
    }

    let mut doc = Document::new(&title, DocumentFormat::Pdf, path);
    doc.meta.title = Some(title);
    doc.meta.author = author;
    doc.meta.page_count = Some(n_pages as usize);
    doc.paragraphs = paragraphs;
    doc.chapters = chapters;
    doc.supports_original_view = true;
    Ok(doc)
}

fn path_to_uri(path: &Path) -> Result<String, DocumentError> {
    let abs = path
        .canonicalize()
        .or_else(|_| Ok::<_, std::io::Error>(path.to_path_buf()))
        .map_err(DocumentError::Io)?;
    Ok(glib::filename_to_uri(abs, None)
        .map_err(|e| DocumentError::Parse(e.to_string()))?
        .to_string())
}

/// Render a PDF page to a cairo image surface for Original View.
pub fn render_page(
    path: &Path,
    page_index: i32,
    scale: f64,
) -> Result<(cairo::ImageSurface, f64, f64), DocumentError> {
    let uri = path_to_uri(path)?;
    let poppler_doc = poppler::Document::from_file(&uri, None)
        .map_err(|e| DocumentError::Parse(format!("Unable to open PDF: {e}")))?;
    let page = poppler_doc
        .page(page_index)
        .ok_or_else(|| DocumentError::Parse(format!("Missing page {page_index}")))?;
    let (w, h) = page.size();
    let width = (w * scale).ceil() as i32;
    let height = (h * scale).ceil() as i32;
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
        .map_err(|e| DocumentError::Parse(format!("Cairo surface: {e}")))?;
    {
        let ctx = cairo::Context::new(&surface)
            .map_err(|e| DocumentError::Parse(format!("Cairo context: {e}")))?;
        ctx.set_source_rgb(1.0, 1.0, 1.0);
        ctx.paint()
            .map_err(|e| DocumentError::Parse(format!("Cairo paint: {e}")))?;
        ctx.scale(scale, scale);
        page.render(&ctx);
    }
    Ok((surface, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_errors() {
        let err = load(Path::new("/tmp/does-not-exist-rustpace.pdf")).unwrap_err();
        assert!(matches!(
            err,
            DocumentError::Io(_) | DocumentError::Parse(_)
        ));
    }
}

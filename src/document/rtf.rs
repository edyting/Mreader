//! RTF document loading.

use std::path::Path;

use super::{paragraphs_from_text, Document, DocumentError, DocumentFormat};

pub fn load(path: &Path) -> Result<Document, DocumentError> {
    let bytes = std::fs::read(path)?;
    if bytes.is_empty() {
        return Err(DocumentError::Corrupted);
    }

    let raw = String::from_utf8_lossy(&bytes);
    let text = match rtf_parser::document::RtfDocument::try_from(raw.as_ref()) {
        Ok(doc) => {
            let t = doc.get_text();
            if t.trim().is_empty() {
                strip_rtf_fallback(&raw)
            } else {
                t
            }
        }
        Err(_) => strip_rtf_fallback(&raw),
    };

    if text.trim().is_empty() {
        return Err(DocumentError::Corrupted);
    }

    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let mut doc = Document::new(&title, DocumentFormat::Rtf, path);
    doc.meta.title = Some(title);
    doc.paragraphs = paragraphs_from_text(&text);
    if doc.paragraphs.is_empty() {
        return Err(DocumentError::Corrupted);
    }
    Ok(doc)
}

/// Conservative RTF stripper used when the structured parser fails.
pub fn strip_rtf_fallback(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut skip_destination = false;

    while let Some(c) = chars.next() {
        match c {
            '{' => {}
            '}' => {
                skip_destination = false;
            }
            '\\' => {
                let mut word = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_alphabetic() {
                        word.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if chars.peek() == Some(&'-') {
                    chars.next();
                }
                while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    chars.next();
                }
                if chars.peek() == Some(&' ') {
                    chars.next();
                }
                match word.as_str() {
                    "par" | "line" => out.push('\n'),
                    "tab" => out.push('\t'),
                    "emdash" => out.push('—'),
                    "endash" => out.push('–'),
                    "bullet" => out.push('•'),
                    "*" => skip_destination = true,
                    "" => {
                        if let Some(next) = chars.next() {
                            if next == '\'' {
                                let h1 = chars.next().unwrap_or('0');
                                let h2 = chars.next().unwrap_or('0');
                                if let Ok(byte) = u8::from_str_radix(&format!("{h1}{h2}"), 16) {
                                    out.push(byte as char);
                                }
                            } else if matches!(next, '{' | '}' | '\\') {
                                out.push(next);
                            }
                        }
                    }
                    _ => {}
                }
            }
            '\r' | '\n' => {}
            _ if !skip_destination => {
                if !c.is_control() {
                    out.push(c);
                }
            }
            _ => {}
        }
    }

    let mut cleaned = String::new();
    let mut blank = 0;
    for line in out.lines() {
        let t = line.trim();
        if t.is_empty() {
            blank += 1;
            if blank <= 1 {
                cleaned.push('\n');
            }
        } else {
            blank = 0;
            cleaned.push_str(t);
            cleaned.push('\n');
        }
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn strip_simple_rtf() {
        let rtf = r#"{\rtf1\ansi\deff0 {\fonttbl{\f0 Times;}}
\f0\fs24 Hello world.\par
Second paragraph.}
"#;
        let text = strip_rtf_fallback(rtf);
        assert!(text.to_lowercase().contains("hello"));
        assert!(text.to_lowercase().contains("second"));
    }

    #[test]
    fn load_rtf_file() {
        let mut f = NamedTempFile::with_suffix(".rtf").unwrap();
        f.write_all(br#"{\rtf1\ansi\deff0\f0\fs24 Hello from RTF.\par Another line.}"#)
            .unwrap();
        let doc = load(f.path()).unwrap();
        assert!(doc.total_words() >= 4);
    }
}

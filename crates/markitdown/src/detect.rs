//! Format detection. Priority order: extension → MIME → magic bytes → content sniffing.
//! (research.md §4)

use std::io::Cursor;

use crate::source::SourceContent;
use crate::util::looks_like_text;

/// Detected format. `Unknown` means unsupported (results in an `UnsupportedFormat` error).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectedFormat {
    PlainText,
    Html,
    Csv,
    Json,
    Xml,
    Pdf,
    Docx,
    Pptx,
    Xlsx,
    Epub,
    Zip,
    Unknown,
}

impl DetectedFormat {
    /// Display name for error messages.
    pub fn label(self) -> &'static str {
        match self {
            DetectedFormat::PlainText => "text",
            DetectedFormat::Html => "html",
            DetectedFormat::Csv => "csv",
            DetectedFormat::Json => "json",
            DetectedFormat::Xml => "xml",
            DetectedFormat::Pdf => "pdf",
            DetectedFormat::Docx => "docx",
            DetectedFormat::Pptx => "pptx",
            DetectedFormat::Xlsx => "xlsx",
            DetectedFormat::Epub => "epub",
            DetectedFormat::Zip => "zip",
            DetectedFormat::Unknown => "unknown",
        }
    }
}

/// Classifies a ZIP-based container by its internal entry names (OOXML / EPub / plain ZIP).
fn classify_zip(bytes: &[u8]) -> DetectedFormat {
    let Ok(archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return DetectedFormat::Zip;
    };
    let names: Vec<&str> = archive.file_names().collect();
    let has = |prefix: &str| names.iter().any(|n| n.starts_with(prefix));
    if has("word/") {
        DetectedFormat::Docx
    } else if has("ppt/") {
        DetectedFormat::Pptx
    } else if has("xl/") {
        DetectedFormat::Xlsx
    } else if names.contains(&"META-INF/container.xml") {
        DetectedFormat::Epub
    } else {
        DetectedFormat::Zip
    }
}

fn by_extension(ext: &str) -> Option<DetectedFormat> {
    Some(match ext {
        "html" | "htm" | "xhtml" => DetectedFormat::Html,
        "pdf" => DetectedFormat::Pdf,
        "csv" | "tsv" => DetectedFormat::Csv,
        "json" => DetectedFormat::Json,
        "xml" => DetectedFormat::Xml,
        "docx" | "docm" => DetectedFormat::Docx,
        "pptx" | "pptm" => DetectedFormat::Pptx,
        "xlsx" | "xlsm" | "xlsb" | "xls" | "ods" => DetectedFormat::Xlsx,
        "epub" => DetectedFormat::Epub,
        "txt" | "text" | "md" | "markdown" | "log" => DetectedFormat::PlainText,
        _ => return None,
    })
}

fn by_mime(mime: &str) -> Option<DetectedFormat> {
    let base = mime.split(';').next().unwrap_or("").trim().to_lowercase();
    Some(match base.as_str() {
        "text/html" | "application/xhtml+xml" => DetectedFormat::Html,
        "application/pdf" => DetectedFormat::Pdf,
        "text/csv" => DetectedFormat::Csv,
        "application/json" | "text/json" => DetectedFormat::Json,
        "application/xml" | "text/xml" => DetectedFormat::Xml,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            DetectedFormat::Docx
        }
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            DetectedFormat::Pptx
        }
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "application/vnd.ms-excel"
        | "application/vnd.oasis.opendocument.spreadsheet" => DetectedFormat::Xlsx,
        "application/epub+zip" => DetectedFormat::Epub,
        "text/plain" => DetectedFormat::PlainText,
        _ => return None,
    })
}

/// Detects the format from a SourceContent.
pub fn detect(src: &SourceContent) -> DetectedFormat {
    // 1. Extension. For the zip extension, peek inside to classify.
    if let Some(ext) = src.extension() {
        if ext == "zip" {
            return classify_zip(&src.bytes);
        }
        if let Some(fmt) = by_extension(&ext) {
            return fmt;
        }
    }

    // 2. MIME.
    if let Some(mime) = &src.mime {
        if mime
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("application/zip")
        {
            return classify_zip(&src.bytes);
        }
        if let Some(fmt) = by_mime(mime) {
            return fmt;
        }
    }

    // 3. Magic bytes.
    if let Some(kind) = infer::get(&src.bytes) {
        match kind.mime_type() {
            "application/pdf" => return DetectedFormat::Pdf,
            "application/zip" => return classify_zip(&src.bytes),
            "text/html" => return DetectedFormat::Html,
            _ => {}
        }
    }

    // 4. Content sniffing: treat as PlainText if it looks like text.
    if looks_like_text(&src.bytes) {
        return DetectedFormat::PlainText;
    }

    DetectedFormat::Unknown
}

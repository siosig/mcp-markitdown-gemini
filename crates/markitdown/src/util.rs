//! Shared utilities for converters (text decoding, Markdown table, XML name handling, ZIP reading).

use std::io::{Cursor, Read};

/// Reads an entry with the specified name from a ZIP byte slice.
pub fn read_zip_entry(zip_bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).ok()?;
    let mut file = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Lists entry names from a ZIP byte slice that start with the specified prefix.
pub fn list_zip_entries(zip_bytes: &[u8], prefix: &str) -> Vec<String> {
    let Ok(archive) = zip::ZipArchive::new(Cursor::new(zip_bytes)) else {
        return Vec::new();
    };
    archive
        .file_names()
        .filter(|n| n.starts_with(prefix))
        .map(|n| n.to_string())
        .collect()
}

/// Decodes a byte slice into text.
///
/// If a charset hint is provided it takes priority; otherwise the encoding is guessed
/// with `chardetng` (FR-015).
pub fn decode_text(bytes: &[u8], charset: Option<&str>) -> String {
    if let Some(label) = charset {
        if let Some(enc) = encoding_rs::Encoding::for_label(label.as_bytes()) {
            let (cow, _, _) = enc.decode(bytes);
            return cow.into_owned();
        }
    }
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
    detector.feed(bytes, true);
    let enc = detector.guess(None, chardetng::Utf8Detection::Allow);
    let (cow, _, _) = enc.decode(bytes);
    cow.into_owned()
}

/// Decodes a quick-xml Text event and returns a string with XML entities unescaped.
pub fn decode_xml_text(t: &quick_xml::events::BytesText) -> String {
    match t.decode() {
        Ok(cow) => quick_xml::escape::unescape(&cow)
            .map(|u| u.into_owned())
            .unwrap_or_else(|_| cow.into_owned()),
        Err(_) => String::new(),
    }
}

/// Determines whether the leading sample looks like text (no NUL bytes, high ratio of printable characters).
pub fn looks_like_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    let sample = &bytes[..bytes.len().min(4096)];
    if sample.contains(&0) {
        return false;
    }
    if std::str::from_utf8(sample).is_ok() {
        return true;
    }
    // Even if not valid UTF-8, treat as text if control characters are sparse (legacy encodings).
    let printable = sample
        .iter()
        .filter(|&&b| b >= 0x20 || b == b'\n' || b == b'\r' || b == b'\t')
        .count();
    (printable as f64) / (sample.len() as f64) > 0.85
}

/// Extracts the local name (`p`) from an XML qualified name (`w:p`, etc.).
pub fn local_name(qname: &[u8]) -> &[u8] {
    match qname.iter().position(|&b| b == b':') {
        Some(i) => &qname[i + 1..],
        None => qname,
    }
}

/// Escapes newlines and pipe characters in a cell to safe representations for Markdown tables.
fn escape_cell(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\n', '\r'], " ")
        .trim()
        .to_string()
}

/// Formats tabular data as a Markdown table. The first row is treated as the header.
///
/// Returns an empty string if rows is empty. Column count is padded to the maximum row width.
pub fn markdown_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }

    let mut out = String::new();
    let render_row = |out: &mut String, row: &[String]| {
        out.push('|');
        for c in 0..cols {
            let cell = row.get(c).map(|s| escape_cell(s)).unwrap_or_default();
            out.push(' ');
            out.push_str(&cell);
            out.push_str(" |");
        }
        out.push('\n');
    };

    render_row(&mut out, &rows[0]);
    out.push('|');
    for _ in 0..cols {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in &rows[1..] {
        render_row(&mut out, row);
    }
    out
}

#![allow(clippy::expect_used, clippy::unwrap_used)] // Allowed in test code
//! Integration tests for format-specific conversion (US1 / quickstart S1, S5, S6).

use std::path::PathBuf;

fn fixture_uri(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path.push(name);
    format!("file://{}", path.display())
}

fn convert(name: &str) -> markitdown::ConversionResult {
    markitdown::convert(&fixture_uri(name)).unwrap_or_else(|e| panic!("convert {name} failed: {e}"))
}

#[test]
fn text_plain() {
    let md = convert("sample.txt").markdown;
    assert!(md.contains("Hello plain text"), "got: {md}");
}

#[test]
fn text_shift_jis() {
    // FR-015: Convert non-UTF-8 encodings without mojibake.
    let md = convert("sample_sjis.txt").markdown;
    assert!(md.contains("日本語のテキスト"), "got: {md}");
}

#[test]
fn csv_table() {
    let md = convert("sample.csv").markdown;
    assert!(md.contains("| Name | Age | City |"), "got: {md}");
    assert!(md.contains("Alice"), "got: {md}");
    assert!(md.contains("| --- |"), "missing separator: {md}");
}

#[test]
fn json_fenced() {
    let md = convert("sample.json").markdown;
    assert!(md.contains("```json"), "got: {md}");
    assert!(md.contains("\"name\""), "got: {md}");
    assert!(md.contains("Alice"), "got: {md}");
}

#[test]
fn xml_fenced() {
    let md = convert("sample.xml").markdown;
    assert!(md.contains("```xml"), "got: {md}");
    assert!(md.contains("Hello"), "got: {md}");
}

#[test]
fn html_structure_and_title() {
    let result = markitdown::convert(&fixture_uri("sample.html")).expect("html");
    assert!(
        result.markdown.contains("# Heading One"),
        "got: {}",
        result.markdown
    );
    assert!(
        result.markdown.contains("**bold**"),
        "got: {}",
        result.markdown
    );
    assert!(
        result.markdown.contains("first"),
        "got: {}",
        result.markdown
    );
    assert_eq!(result.title.as_deref(), Some("Sample Page"));
}

#[test]
fn pdf_text() {
    let md = convert("sample.pdf").markdown;
    assert!(md.contains("Hello PDF from markitdown"), "got: {md}");
}

#[test]
fn docx_headings_and_table() {
    let md = convert("sample.docx").markdown;
    assert!(md.contains("# Document Title"), "got: {md}");
    assert!(
        md.contains("This is a paragraph of body text."),
        "got: {md}"
    );
    assert!(md.contains("- Bullet item"), "got: {md}");
    assert!(md.contains("| H1 | H2 |"), "got: {md}");
    assert!(md.contains("| a | b |"), "got: {md}");
}

#[test]
fn pptx_slides() {
    let md = convert("sample.pptx").markdown;
    assert!(md.contains("## Slide 1"), "got: {md}");
    assert!(md.contains("Slide Title Text"), "got: {md}");
    assert!(md.contains("A bullet line on the slide."), "got: {md}");
}

#[test]
fn xlsx_sheet_table() {
    let md = convert("sample.xlsx").markdown;
    assert!(md.contains("## Sheet1"), "got: {md}");
    assert!(md.contains("| Product | Qty |"), "got: {md}");
    assert!(md.contains("Apple"), "got: {md}");
}

#[test]
fn epub_chapters_and_title() {
    let result = markitdown::convert(&fixture_uri("sample.epub")).expect("epub");
    assert!(
        result.markdown.contains("Chapter One"),
        "got: {}",
        result.markdown
    );
    assert!(
        result.markdown.contains("Once upon a time in markitdown."),
        "got: {}",
        result.markdown
    );
    assert_eq!(result.title.as_deref(), Some("Sample EPub Title"));
}

#[test]
fn zip_recursive_concat() {
    // FR-008: Concatenate contained files with headings.
    let md = convert("sample.zip").markdown;
    assert!(md.contains("### note.txt"), "got: {md}");
    assert!(md.contains("Inside the zip."), "got: {md}");
    assert!(md.contains("### data.csv"), "got: {md}");
    assert!(md.contains("| k | v |"), "got: {md}");
}

// --- Error paths (SC-004 / FR-009) ---

#[test]
fn error_missing_file() {
    let err = markitdown::convert("file:///no/such/path/missing.txt").unwrap_err();
    assert!(
        matches!(err, markitdown::MarkItDownError::Acquire { .. }),
        "got: {err:?}"
    );
}

#[test]
fn error_unsupported_scheme() {
    let err = markitdown::convert("ftp://example.com/x.txt").unwrap_err();
    assert!(
        matches!(err, markitdown::MarkItDownError::UnsupportedScheme(_)),
        "got: {err:?}"
    );
}

#[test]
fn error_unsupported_format() {
    // Binary (containing NUL) + unknown extension → Unknown → UnsupportedFormat.
    let err = markitdown::convert_bytes(
        vec![0u8, 1, 2, 3, 255, 254],
        Some("blob.xyz".to_string()),
        None,
    )
    .unwrap_err();
    assert!(
        matches!(err, markitdown::MarkItDownError::UnsupportedFormat(_)),
        "got: {err:?}"
    );
}

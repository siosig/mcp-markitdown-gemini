#![allow(clippy::expect_used, clippy::unwrap_used)] // Allowed in test code
//! Integration tests for data URI fetching (US3 / quickstart S3).

#[test]
fn data_uri_plain_html() {
    let result = markitdown::convert("data:text/html,<h1>Hi%20There</h1>").expect("data convert");
    assert!(
        result.markdown.contains("# Hi There"),
        "got: {}",
        result.markdown
    );
}

#[test]
fn data_uri_base64_text() {
    // "Hello data" encoded as base64.
    let uri = "data:text/plain;base64,SGVsbG8gZGF0YQ==";
    let md = markitdown::convert(uri).expect("data convert").markdown;
    assert!(md.contains("Hello data"), "got: {md}");
}

#[test]
fn data_uri_invalid_is_error() {
    let err = markitdown::convert("data:not-a-valid-data-uri").unwrap_err();
    assert!(
        matches!(err, markitdown::MarkItDownError::Acquire { .. }),
        "got: {err:?}"
    );
}

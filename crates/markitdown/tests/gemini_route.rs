#![allow(clippy::expect_used, clippy::unwrap_used)] // Allowed in test code
//! Integration tests for the PDF Gemini conversion route (002-pdf-gemini-conversion).
//!
//! Uses a local mock HTTP server that routes by model name in the request path, so flash
//! and pro responses can be controlled independently. Gemini config is injected explicitly
//! (no env mutation) for hermetic tests.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use markitdown::{EngineConfig, GeminiConfig};

fn fixture_uri(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path.push(name);
    format!("file://{}", path.display())
}

/// A canned HTTP response for one tier.
#[derive(Clone)]
struct Canned {
    status: u16,
    body: String,
    /// Optional delay before responding (used to trigger client timeouts).
    delay: Option<Duration>,
}

impl Canned {
    fn ok(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            delay: None,
        }
    }
    fn status(code: u16, body: impl Into<String>) -> Self {
        Self {
            status: code,
            body: body.into(),
            delay: None,
        }
    }
    fn delayed(d: Duration) -> Self {
        Self {
            status: 200,
            body: success_body("late"),
            delay: Some(d),
        }
    }
}

fn reason_phrase(code: u16) -> &'static str {
    match code {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// Builds a successful generateContent body whose single candidate carries `text` and STOP.
fn success_body(text: &str) -> String {
    serde_json::json!({
        "candidates": [{
            "content": { "parts": [{ "text": text }] },
            "finishReason": "STOP"
        }]
    })
    .to_string()
}

/// Builds a "successful" body that the heuristic will judge insufficient (empty text, STOP).
fn insufficient_body() -> String {
    serde_json::json!({
        "candidates": [{
            "content": { "parts": [{ "text": "" }] },
            "finishReason": "STOP"
        }]
    })
    .to_string()
}

fn error_body(code: u16, message: &str, status: &str) -> String {
    serde_json::json!({
        "error": { "code": code, "message": message, "status": status }
    })
    .to_string()
}

/// Spawns a mock Gemini server. Routes each request to `flash` or `pro` by the model id in the
/// request path, serving the same canned response for repeated calls to that tier.
fn spawn_gemini_mock(flash: Canned, pro: Canned) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    thread::spawn(move || loop {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let flash = flash.clone();
        let pro = pro.clone();
        thread::spawn(move || {
            let mut buf = [0u8; 16384];
            let n = stream.read(&mut buf).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]);
            let request_line = head.lines().next().unwrap_or("");
            let canned = if request_line.contains("gemini-3.1-pro-preview") {
                pro
            } else {
                flash
            };
            if let Some(d) = canned.delay {
                thread::sleep(d);
            }
            let response = format!(
                "HTTP/1.1 {code} {phrase}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
                code = canned.status,
                phrase = reason_phrase(canned.status),
                len = canned.body.len(),
                body = canned.body,
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        });
    });
    port
}

fn gemini_config(port: u16) -> EngineConfig {
    let mut g = GeminiConfig::new("test-key");
    g.base_url = format!("http://127.0.0.1:{port}");
    g.timeout_secs = 10;
    EngineConfig { gemini: Some(g) }
}

// ---------------------------------------------------------------------------
// US1: Gemini conversion + escalation
// ---------------------------------------------------------------------------

#[test]
fn flash_sufficient_returns_primary_without_escalation() {
    // pro returns a distinctive marker; if it were called the result would contain it.
    let port = spawn_gemini_mock(
        Canned::ok(success_body(
            "# From Flash\n\n| A | B |\n| --- | --- |\n| 1 | 2 |",
        )),
        Canned::ok(success_body("# FROM PRO SHOULD NOT APPEAR")),
    );
    let cfg = gemini_config(port);
    let md = markitdown::convert_with_config(&fixture_uri("sample.pdf"), &cfg)
        .expect("gemini primary convert")
        .markdown;
    assert!(
        md.contains("From Flash"),
        "expected flash output, got: {md}"
    );
    assert!(
        !md.contains("FROM PRO"),
        "pro must not be called when flash is sufficient: {md}"
    );
}

#[test]
fn flash_insufficient_escalates_to_pro() {
    let port = spawn_gemini_mock(
        Canned::ok(insufficient_body()),
        Canned::ok(success_body("# From Pro\n\nRecovered content.")),
    );
    let cfg = gemini_config(port);
    let md = markitdown::convert_with_config(&fixture_uri("sample.pdf"), &cfg)
        .expect("gemini escalated convert")
        .markdown;
    assert!(
        md.contains("From Pro"),
        "expected escalated pro output, got: {md}"
    );
}

// ---------------------------------------------------------------------------
// US2: backward compatibility (local fallback / non-PDF unaffected)
// ---------------------------------------------------------------------------

#[test]
fn unset_key_uses_local_extraction() {
    // EngineConfig::default() has gemini = None: local route, no network.
    let cfg = EngineConfig::default();
    let md = markitdown::convert_with_config(&fixture_uri("sample.pdf"), &cfg)
        .expect("local convert")
        .markdown;
    assert!(
        md.contains("Hello PDF from markitdown"),
        "expected local extraction, got: {md}"
    );
}

#[test]
fn non_pdf_unaffected_when_gemini_enabled() {
    // Point Gemini at an unused port; non-PDF must not touch it.
    let cfg = gemini_config(1); // port 1: nothing listening
    let md = markitdown::convert_with_config(&fixture_uri("sample.html"), &cfg)
        .expect("html convert")
        .markdown;
    assert!(
        md.contains("# Heading One"),
        "expected html conversion unaffected, got: {md}"
    );
}

// ---------------------------------------------------------------------------
// US3: failure handling (no local fallback, server keeps running)
// ---------------------------------------------------------------------------

#[test]
fn invalid_key_errors_without_local_fallback() {
    let port = spawn_gemini_mock(
        Canned::status(
            400,
            error_body(400, "API key not valid", "INVALID_ARGUMENT"),
        ),
        Canned::ok(success_body("unused")),
    );
    let cfg = gemini_config(port);
    let err = markitdown::convert_with_config(&fixture_uri("sample.pdf"), &cfg)
        .expect_err("must error on invalid key");
    let msg = err.to_string();
    assert!(msg.contains("pdf-gemini"), "expected gemini error: {msg}");
    assert!(
        !msg.contains("Hello PDF from markitdown"),
        "must not fall back to local extraction: {msg}"
    );

    // Engine remains usable for subsequent requests (stateless continuity).
    let next =
        markitdown::convert_with_config(&fixture_uri("sample.csv"), &EngineConfig::default())
            .expect("subsequent request still works")
            .markdown;
    assert!(next.contains("Alice"), "got: {next}");
}

#[test]
fn rate_limit_errors() {
    let port = spawn_gemini_mock(
        Canned::status(
            429,
            error_body(429, "Resource exhausted", "RESOURCE_EXHAUSTED"),
        ),
        Canned::ok(success_body("unused")),
    );
    let cfg = gemini_config(port);
    let err = markitdown::convert_with_config(&fixture_uri("sample.pdf"), &cfg)
        .expect_err("must error on 429");
    assert!(err.to_string().contains("rate-limit"), "got: {err}");
}

#[test]
fn server_error_errors() {
    let port = spawn_gemini_mock(
        Canned::status(503, error_body(503, "overloaded", "UNAVAILABLE")),
        Canned::ok(success_body("unused")),
    );
    let cfg = gemini_config(port);
    let err = markitdown::convert_with_config(&fixture_uri("sample.pdf"), &cfg)
        .expect_err("must error on 5xx");
    assert!(err.to_string().contains("server"), "got: {err}");
}

#[test]
fn timeout_errors() {
    let port = spawn_gemini_mock(
        Canned::delayed(Duration::from_secs(3)),
        Canned::ok("unused"),
    );
    let mut cfg = gemini_config(port);
    if let Some(g) = cfg.gemini.as_mut() {
        g.timeout_secs = 1;
    }
    let err = markitdown::convert_with_config(&fixture_uri("sample.pdf"), &cfg)
        .expect_err("must error on timeout");
    assert!(err.to_string().contains("timeout"), "got: {err}");
}

#[test]
fn escalation_still_insufficient_errors() {
    let port = spawn_gemini_mock(
        Canned::ok(insufficient_body()),
        Canned::ok(insufficient_body()),
    );
    let cfg = gemini_config(port);
    let err = markitdown::convert_with_config(&fixture_uri("sample.pdf"), &cfg)
        .expect_err("must error when pro still insufficient");
    assert!(
        err.to_string().contains("escalation insufficient"),
        "got: {err}"
    );
}

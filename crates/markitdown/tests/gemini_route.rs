#![allow(clippy::expect_used, clippy::unwrap_used)] // Allowed in test code
//! Integration tests for the PDF Gemini conversion route (002-pdf-gemini-conversion).
//!
//! Uses a local mock HTTP server that routes by call order (first request = primary tier,
//! later ones = escalation tier), so the two tiers' responses can be controlled independently
//! even though they now share one model id. Gemini config is injected explicitly (no env
//! mutation) for hermetic tests.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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

/// Shared handle for captured request bodies (as parsed JSON) for one model tier.
type CapturedBodies = Arc<Mutex<Vec<serde_json::Value>>>;

/// Finds the index of the start of the `\r\n\r\n` header/body separator, if the full header
/// block has been received yet.
fn find_header_terminator(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Parses the `Content-Length` header (case-insensitive) out of a raw HTTP header block.
fn parse_content_length(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

/// Reads one HTTP request off `stream` (headers + full body per `Content-Length`) and returns
/// the request line together with the raw body bytes. Returns `None` if the connection closed
/// before a full header block was received.
fn read_request(stream: &mut std::net::TcpStream) -> Option<(String, Vec<u8>)> {
    let mut buf = Vec::with_capacity(16384);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_header_terminator(&buf) {
            break pos;
        }
        if buf.len() > 1_000_000 {
            return None;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let request_line = head.lines().next().unwrap_or("").to_string();
    let content_length = parse_content_length(&head);
    let body_start = header_end + 4;
    let mut body = buf[body_start.min(buf.len())..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Some((request_line, body))
}

/// Spawns a mock Gemini server. Routes the **first** request to `flash` (the primary tier) and
/// every later one to `pro` (the escalation tier), serving the same canned response for repeated
/// calls to that tier. Also parses and captures each request's JSON body into a tier-specific
/// shared list, so callers can later inspect exactly what was sent (e.g. the `thinkingConfig`
/// field).
///
/// Routing is by call order, not by model id: both tiers now name the same `-latest` alias and
/// differ only in `thinkingLevel`, so the request path no longer identifies the tier. Order is
/// unambiguous because escalation is issued only after the primary response has been assessed.
fn spawn_gemini_mock(flash: Canned, pro: Canned) -> (u16, CapturedBodies, CapturedBodies) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let flash_bodies: CapturedBodies = Arc::new(Mutex::new(Vec::new()));
    let pro_bodies: CapturedBodies = Arc::new(Mutex::new(Vec::new()));
    let flash_bodies_srv = Arc::clone(&flash_bodies);
    let pro_bodies_srv = Arc::clone(&pro_bodies);
    let seen = Arc::new(AtomicUsize::new(0));
    thread::spawn(move || loop {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let flash = flash.clone();
        let pro = pro.clone();
        let flash_bodies = Arc::clone(&flash_bodies_srv);
        let pro_bodies = Arc::clone(&pro_bodies_srv);
        let seen = Arc::clone(&seen);
        thread::spawn(move || {
            let Some((_request_line, body)) = read_request(&mut stream) else {
                return;
            };
            let is_pro = seen.fetch_add(1, Ordering::SeqCst) > 0;
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) {
                let bucket = if is_pro { &pro_bodies } else { &flash_bodies };
                bucket.lock().expect("lock captured bodies").push(value);
            }
            let canned = if is_pro { pro } else { flash };
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
    (port, flash_bodies, pro_bodies)
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
    let (port, flash_bodies, _pro_bodies) = spawn_gemini_mock(
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

    // Verify the primary request carried thinkingLevel "low", not the 2.5-series numeric
    // thinkingBudget (deprecated for the 3.x line the alias resolves to).
    let bodies = flash_bodies.lock().expect("lock flash bodies");
    assert_eq!(
        bodies.len(),
        1,
        "expected exactly one flash request, got: {bodies:?}"
    );
    let thinking_config = &bodies[0]["generationConfig"]["thinkingConfig"];
    assert_eq!(
        thinking_config["thinkingLevel"],
        serde_json::json!("low"),
        "expected thinkingLevel low, got: {thinking_config:?}"
    );
    assert!(
        thinking_config.get("thinkingBudget").is_none(),
        "thinkingConfig must not contain the legacy thinkingBudget field, got: {thinking_config:?}"
    );
}

#[test]
fn flash_insufficient_escalates_to_pro() {
    let (port, _flash_bodies, pro_bodies) = spawn_gemini_mock(
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

    // Verify the escalated request carried thinkingLevel "high" — with both tiers on the same
    // model, this field IS the escalation.
    let bodies = pro_bodies.lock().expect("lock pro bodies");
    assert_eq!(
        bodies.len(),
        1,
        "expected exactly one pro request, got: {bodies:?}"
    );
    let thinking_config = &bodies[0]["generationConfig"]["thinkingConfig"];
    assert_eq!(
        thinking_config["thinkingLevel"],
        serde_json::json!("high"),
        "expected thinkingLevel high, got: {thinking_config:?}"
    );
    assert!(
        thinking_config.get("thinkingBudget").is_none(),
        "thinkingConfig must not contain the legacy thinkingBudget field, got: {thinking_config:?}"
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
    let (port, _flash_bodies, _pro_bodies) = spawn_gemini_mock(
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
    let (port, _flash_bodies, _pro_bodies) = spawn_gemini_mock(
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
    let (port, _flash_bodies, _pro_bodies) = spawn_gemini_mock(
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
    let (port, _flash_bodies, _pro_bodies) = spawn_gemini_mock(
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
    let (port, _flash_bodies, _pro_bodies) = spawn_gemini_mock(
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

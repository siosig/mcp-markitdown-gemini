#![allow(clippy::expect_used, clippy::unwrap_used)] // Allowed in test code
//! Async-context regression test for the Gemini PDF route (004-genai-rs-migration, T013).
//!
//! Permanent guard against a `BlockingInsideRuntime` regression — i.e. writing the Gemini
//! call back to a direct SDK invocation from tokio context. The gemini-genai SDK's blocking
//! client returns `Error::BlockingInsideRuntime` whenever it detects a tokio runtime
//! context, and it detects it even on `tokio::task::spawn_blocking` threads — which is
//! exactly how markitdown-mcp invokes the conversion engine
//! (`crates/markitdown-mcp/src/server.rs`). The implementation therefore escapes to a plain
//! OS thread (`std::thread::scope`) inside `gemini::client::generate()`. This test drives a
//! Gemini-routed PDF conversion from a `spawn_blocking` task on a multi-thread tokio
//! runtime and must keep passing; it fails with `BlockingInsideRuntime` if that escape
//! hatch is ever removed.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

use markitdown::{EngineConfig, GeminiConfig};

/// Canned generateContent success body: STOP finish reason and a markdown payload long
/// enough for the sufficiency heuristic (non-empty, garble-free; the density check does
/// not apply to the small fixture PDF).
const CANNED_BODY: &str = r##"{"candidates":[{"content":{"parts":[{"text":"# Async Context OK\n\nMock gemini markdown body long enough to pass the density heuristic."}]},"finishReason":"STOP"}]}"##;

/// Reads one HTTP request off `stream`: headers up to the `\r\n\r\n` terminator, then the
/// body for the advertised `Content-Length` (0 if absent). Mirrors `read_request` in
/// markitdown's `gemini_route.rs`. Returns `false` if the connection closed before a full
/// header block was received.
fn read_full_request(stream: &mut TcpStream) -> bool {
    let mut buf: Vec<u8> = Vec::with_capacity(16384);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return false;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > 1_000_000 {
            return false;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let content_length: usize = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    let mut body_len = buf.len().saturating_sub(header_end + 4);
    while body_len < content_length {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        body_len += n;
    }
    true
}

/// Spawns a one-shot mock Gemini server on an ephemeral port: accepts a single connection,
/// reads the request to completion, answers with `CANNED_BODY`, closes the stream, exits.
fn spawn_one_shot_gemini_mock() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock gemini listener");
    let port = listener.local_addr().expect("mock listener addr").port();
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        if !read_full_request(&mut stream) {
            return;
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
            len = CANNED_BODY.len(),
            body = CANNED_BODY,
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });
    port
}

/// Permanent guard against a `BlockingInsideRuntime` regression (a rewrite back to calling
/// the SDK directly from tokio context). Reproduces the production shape end to end: a
/// multi-thread tokio runtime driving a Gemini-routed PDF conversion from inside
/// `tokio::task::spawn_blocking`, exactly like markitdown-mcp's `convert_to_markdown`
/// handler does.
#[tokio::test(flavor = "multi_thread")]
async fn pdf_gemini_conversion_succeeds_inside_spawn_blocking() {
    let fixture = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../markitdown/tests/fixtures/sample.pdf"
    ))
    .canonicalize()
    .expect("sample.pdf fixture must exist under crates/markitdown/tests/fixtures");
    let uri = format!("file://{}", fixture.display());

    let port = spawn_one_shot_gemini_mock();

    let result = tokio::task::spawn_blocking(move || {
        let mut gemini = GeminiConfig::new("test-key");
        gemini.base_url = format!("http://127.0.0.1:{port}");
        gemini.timeout_secs = 10;
        let config = EngineConfig {
            gemini: Some(gemini),
        };
        markitdown::convert_with_config(&uri, &config)
    })
    .await
    .expect("spawn_blocking task must not panic");

    let converted = match result {
        Ok(converted) => converted,
        Err(err) => panic!(
            "PDF Gemini conversion must succeed inside spawn_blocking; a \
             `BlockingInsideRuntime` mention in this error means the OS-thread escape \
             hatch in gemini::client::generate() has regressed: {err}"
        ),
    };
    assert!(
        converted.markdown.contains("Async Context OK"),
        "expected mock gemini markdown in the output, got: {}",
        converted.markdown
    );
}

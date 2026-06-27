#![allow(clippy::expect_used, clippy::unwrap_used)] // テストコードでは許容
//! http/https 取得の統合テスト (US2 / quickstart S2)。ローカルのモック HTTP サーバを使う。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

/// 1 リクエストだけ受けて固定レスポンスを返すモックサーバを起動し、ポートを返す。
fn spawn_mock(body: &'static str, content_type: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
                ct = content_type,
                len = body.len(),
                body = body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    port
}

#[test]
fn http_html_conversion() {
    let port = spawn_mock(
        "<html><head><title>Remote</title></head><body><h1>Remote Heading</h1></body></html>",
        "text/html; charset=utf-8",
    );
    let uri = format!("http://127.0.0.1:{port}/page.html");
    let result = markitdown::convert(&uri).expect("http convert");
    assert!(
        result.markdown.contains("# Remote Heading"),
        "got: {}",
        result.markdown
    );
    assert_eq!(result.title.as_deref(), Some("Remote"));
}

#[test]
fn http_unreachable_is_error() {
    // 待ち受けていないポートへ接続 → Acquire エラー (FR-009)。
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener); // ポートを解放して接続不可にする
    let uri = format!("http://127.0.0.1:{port}/x");
    let err = markitdown::convert(&uri).unwrap_err();
    assert!(
        matches!(err, markitdown::MarkItDownError::Acquire { .. }),
        "got: {err:?}"
    );
}

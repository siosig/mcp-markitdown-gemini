#![allow(clippy::expect_used, clippy::unwrap_used)] // Allowed in test code
//! End-to-end tests via the MCP protocol (US1 / quickstart S1, S5, S7).
//! Launches the built binary as a stdio child process and verifies it using the rmcp client.

use std::path::PathBuf;

use rmcp::model::CallToolRequestParams;
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;
use tokio::process::Command;

/// Builds a file URI pointing to a fixture in the markitdown crate.
fn fixture_uri(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../markitdown/tests/fixtures");
    path.push(name);
    let abs = path.canonicalize().expect("fixture exists");
    format!("file://{}", abs.display())
}

async fn connect() -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_markitdown-mcp"));
    cmd.stderr(std::process::Stdio::null());
    let transport = TokioChildProcess::new(cmd).expect("spawn server");
    ().serve(transport).await.expect("connect to server")
}

fn arguments(uri: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    map.insert(
        "uri".to_string(),
        serde_json::Value::String(uri.to_string()),
    );
    map
}

#[tokio::test]
async fn lists_convert_tool() {
    // SC-006: The tool name convert_to_markdown is exposed.
    let client = connect().await;
    let tools = client.list_all_tools().await.expect("list tools");
    assert!(
        tools.iter().any(|t| t.name == "convert_to_markdown"),
        "tools: {:?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );
    client.cancel().await.ok();
}

#[tokio::test]
async fn converts_file_uri() {
    // US1: Converts HTML at a file URI and returns Markdown.
    let client = connect().await;
    let result = client
        .call_tool(
            CallToolRequestParams::new("convert_to_markdown")
                .with_arguments(arguments(&fixture_uri("sample.html"))),
        )
        .await
        .expect("call tool");

    assert_ne!(result.is_error, Some(true), "unexpected error: {result:?}");
    let text = result
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert!(text.contains("# Heading One"), "got: {text}");
    client.cancel().await.ok();
}

#[tokio::test]
async fn error_is_reported_and_server_survives() {
    // SC-004 / FR-009/010: Failures are returned with isError, and the server continues to handle subsequent requests.
    let client = connect().await;

    let bad = client
        .call_tool(
            CallToolRequestParams::new("convert_to_markdown")
                .with_arguments(arguments("file:///no/such/missing.txt")),
        )
        .await
        .expect("call tool (error case still returns a result)");
    assert_eq!(bad.is_error, Some(true), "expected isError: {bad:?}");

    // A subsequent successful request on the same connection should succeed (server continues running).
    let ok = client
        .call_tool(
            CallToolRequestParams::new("convert_to_markdown")
                .with_arguments(arguments(&fixture_uri("sample.txt"))),
        )
        .await
        .expect("subsequent call");
    assert_ne!(ok.is_error, Some(true), "server did not survive: {ok:?}");

    client.cancel().await.ok();
}

//! MCP サーバー実装。単一ツール `convert_to_markdown` を公開する。
//! contracts/convert_to_markdown.md に準拠。

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler};
use serde::Deserialize;

/// `convert_to_markdown` ツールの入力。
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ConvertArgs {
    /// Resource URI. Supported schemes: http, https, file, data.
    pub uri: String,
}

/// MCP サーバー本体。変換は `markitdown` エンジンへ委譲する。
#[derive(Clone)]
pub struct MarkItDownServer {
    // `#[tool_router]` が生成する `Self::tool_router()` をハンドラが利用するため、
    // フィールド自体は dead_code 解析上は未読扱いになる。保持は生成パターンに従う。
    #[expect(
        dead_code,
        reason = "router is rebuilt via the generated tool_router() fn"
    )]
    tool_router: ToolRouter<Self>,
}

impl MarkItDownServer {
    /// ツールルーターを初期化したサーバーを生成する。
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for MarkItDownServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl MarkItDownServer {
    /// URI の指す文書を Markdown に変換して返す。
    ///
    /// 変換は同期的に重い処理になりうるため `spawn_blocking` で実行し、
    /// 失敗時は `isError` 付きのツールエラーを返す (FR-009/010, SC-004)。
    #[tool(
        name = "convert_to_markdown",
        description = "Convert a resource described by an http:, https:, file:, or data: URI to markdown."
    )]
    async fn convert_to_markdown(
        &self,
        Parameters(args): Parameters<ConvertArgs>,
    ) -> CallToolResult {
        let uri = args.uri;
        tracing::info!(uri = %uri, "convert_to_markdown");

        match tokio::task::spawn_blocking(move || markitdown::convert(&uri)).await {
            Ok(Ok(result)) => CallToolResult::success(vec![Content::text(result.markdown)]),
            Ok(Err(err)) => {
                tracing::warn!(error = %err, "conversion failed");
                CallToolResult::error(vec![Content::text(format!("Error: {err}"))])
            }
            Err(join_err) => {
                tracing::error!(error = %join_err, "conversion task panicked");
                CallToolResult::error(vec![Content::text(format!(
                    "Error: internal task failure: {join_err}"
                ))])
            }
        }
    }
}

#[tool_handler]
impl ServerHandler for MarkItDownServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Convert a document at an http/https/file/data URI to Markdown \
                 via the convert_to_markdown tool.",
            )
    }
}

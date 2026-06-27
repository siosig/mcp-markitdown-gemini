//! markitdown-mcp: `convert_to_markdown` ツールを公開する MCP サーバー。
//! microsoft/markitdown-mcp 互換 (ツール名・引数・対応スキーム)。

mod server;
mod transport;

use clap::Parser;

/// CLI 起動オプション。
#[derive(Debug, Parser)]
#[command(
    name = "markitdown-mcp",
    version,
    about = "MCP server that converts documents at a URI to Markdown"
)]
struct Cli {
    /// (後続予定) stdio ではなく HTTP/SSE トランスポートで公開する。v1 では未対応。
    #[arg(long)]
    http: bool,

    /// HTTP 公開時のバインド先ホスト (既定: localhost)。
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// HTTP 公開時のポート。
    #[arg(long, default_value_t = 3001)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // stdout は MCP(JSON-RPC) 専用のため、ログは stderr へ出力する。
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    if cli.http {
        return Err(format!(
            "HTTP/SSE transport ({}:{}) is not yet implemented; v1 supports stdio only",
            cli.host, cli.port
        )
        .into());
    }

    tracing::info!("starting markitdown-mcp over stdio");
    transport::serve_stdio().await?;
    Ok(())
}

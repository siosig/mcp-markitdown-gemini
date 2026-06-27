//! markitdown-mcp: MCP server that exposes the `convert_to_markdown` tool.
//! Compatible with microsoft/markitdown-mcp (tool name, arguments, and supported schemes).

mod server;
mod transport;

use clap::Parser;

/// CLI startup options.
#[derive(Debug, Parser)]
#[command(
    name = "markitdown-mcp",
    version,
    about = "MCP server that converts documents at a URI to Markdown"
)]
struct Cli {
    /// (Planned for a future release) Expose the server over HTTP/SSE transport instead of stdio. Not supported in v1.
    #[arg(long)]
    http: bool,

    /// Host to bind to when serving over HTTP (default: localhost).
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind to when serving over HTTP.
    #[arg(long, default_value_t = 3001)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // stdout is reserved for MCP (JSON-RPC), so logs are written to stderr.
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

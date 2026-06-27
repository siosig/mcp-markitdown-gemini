//! Transport selection. v1 supports stdio only. HTTP/SSE is planned for a future release (FR-011/012/013).
//! Separation point for loose coupling from the conversion logic.

use rmcp::transport::stdio;
use rmcp::ServiceExt;

use crate::server::MarkItDownServer;

/// Starts the server over stdio transport and waits until it exits.
pub async fn serve_stdio() -> Result<(), Box<dyn std::error::Error>> {
    let service = MarkItDownServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

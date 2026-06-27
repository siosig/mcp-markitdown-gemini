//! トランスポート選択。v1 は stdio のみ。HTTP/SSE は後続 (FR-011/012/013)。
//! 変換ロジックと疎結合化する分離点。

use rmcp::transport::stdio;
use rmcp::ServiceExt;

use crate::server::MarkItDownServer;

/// stdio トランスポートでサーバーを起動し、終了まで待機する。
pub async fn serve_stdio() -> Result<(), Box<dyn std::error::Error>> {
    let service = MarkItDownServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

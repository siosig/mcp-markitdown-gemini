//! URI 取得層。スキーム別に `Acquirer` 実装へ dispatch する (data-model.md §Acquirer)。

pub mod data;
pub mod file;
pub mod http;

use url::Url;

use crate::error::MarkItDownError;
use crate::source::SourceContent;

/// URI からコンテンツを取得する共通インターフェース。
pub trait Acquirer {
    /// `url` の指すコンテンツを取得する。
    fn acquire(&self, url: &Url) -> Result<SourceContent, MarkItDownError>;
}

/// URI 文字列をパースし、スキームに応じた Acquirer で取得する (FR-004)。
pub fn acquire(uri: &str) -> Result<SourceContent, MarkItDownError> {
    let url = Url::parse(uri).map_err(|e| MarkItDownError::InvalidUri(format!("{uri}: {e}")))?;
    match url.scheme() {
        "file" => file::FileAcquirer.acquire(&url),
        "http" | "https" => http::HttpAcquirer::default().acquire(&url),
        "data" => data::DataAcquirer.acquire(&url),
        other => Err(MarkItDownError::UnsupportedScheme(other.to_string())),
    }
}

/// URL パスの末尾セグメントをファイル名として取り出す。
pub(crate) fn filename_from_url(url: &Url) -> Option<String> {
    url.path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

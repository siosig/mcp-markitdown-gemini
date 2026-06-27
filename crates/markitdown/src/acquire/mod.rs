//! URI acquisition layer. Dispatches to `Acquirer` implementations based on the scheme (data-model.md §Acquirer).

pub mod data;
pub mod file;
pub mod http;

use url::Url;

use crate::error::MarkItDownError;
use crate::source::SourceContent;

/// Common interface for acquiring content from a URI.
pub trait Acquirer {
    /// Acquires the content pointed to by `url`.
    fn acquire(&self, url: &Url) -> Result<SourceContent, MarkItDownError>;
}

/// Parses a URI string and acquires its content using the appropriate Acquirer for the scheme (FR-004).
pub fn acquire(uri: &str) -> Result<SourceContent, MarkItDownError> {
    let url = Url::parse(uri).map_err(|e| MarkItDownError::InvalidUri(format!("{uri}: {e}")))?;
    match url.scheme() {
        "file" => file::FileAcquirer.acquire(&url),
        "http" | "https" => http::HttpAcquirer::default().acquire(&url),
        "data" => data::DataAcquirer.acquire(&url),
        other => Err(MarkItDownError::UnsupportedScheme(other.to_string())),
    }
}

/// Extracts the last path segment of a URL as the filename.
pub(crate) fn filename_from_url(url: &Url) -> Option<String> {
    url.path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

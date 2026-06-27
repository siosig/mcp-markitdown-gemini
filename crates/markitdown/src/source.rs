//! Acquired content and its hints (data-model.md: SourceContent / Origin).

use std::path::PathBuf;
use url::Url;

/// Source origin type.
#[derive(Debug, Clone)]
pub enum Origin {
    /// Local file (`file:`).
    File(PathBuf),
    /// Remote resource (`http:` / `https:`).
    Remote(Url),
    /// Inline-embedded data (`data:`).
    Inline,
}

/// Raw data fetched from a URI by the acquire layer, along with hints for format detection.
#[derive(Debug, Clone)]
pub struct SourceContent {
    /// Raw bytes fetched from the source.
    pub bytes: Vec<u8>,
    /// MIME type derived from the Content-Type header or data URI (if available).
    pub mime: Option<String>,
    /// Filename used for extension-based format detection (if available).
    pub filename: Option<String>,
    /// Charset hint (if available).
    pub charset: Option<String>,
    /// Source origin.
    pub origin: Origin,
}

impl SourceContent {
    /// Extracts the lowercased file extension from `filename`.
    pub fn extension(&self) -> Option<String> {
        let name = self.filename.as_ref()?;
        let ext = std::path::Path::new(name).extension()?;
        Some(ext.to_string_lossy().to_lowercase())
    }
}

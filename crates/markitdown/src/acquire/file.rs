//! `file:` scheme acquisition (US1).

use url::Url;

use super::Acquirer;
use crate::error::MarkItDownError;
use crate::source::{Origin, SourceContent};

/// Acquirer that reads local files.
pub struct FileAcquirer;

impl Acquirer for FileAcquirer {
    fn acquire(&self, url: &Url) -> Result<SourceContent, MarkItDownError> {
        let path = url
            .to_file_path()
            .map_err(|_| MarkItDownError::acquire(url.as_str(), "invalid file path in URI"))?;

        let bytes = std::fs::read(&path)
            .map_err(|e| MarkItDownError::acquire(url.as_str(), e.to_string()))?;

        let filename = path.file_name().map(|n| n.to_string_lossy().into_owned());

        Ok(SourceContent {
            bytes,
            mime: None,
            filename,
            charset: None,
            origin: Origin::File(path),
        })
    }
}

//! `data:` scheme acquisition (US3). Decodes MIME type and body using the `data-url` crate.

use url::Url;

use super::Acquirer;
use crate::error::MarkItDownError;
use crate::source::{Origin, SourceContent};

/// Acquirer that decodes data URIs.
pub struct DataAcquirer;

impl Acquirer for DataAcquirer {
    fn acquire(&self, url: &Url) -> Result<SourceContent, MarkItDownError> {
        let parsed = data_url::DataUrl::process(url.as_str()).map_err(|e| {
            MarkItDownError::acquire(url.as_str(), format!("invalid data URI: {e:?}"))
        })?;

        let (bytes, _fragment) = parsed
            .decode_to_vec()
            .map_err(|e| MarkItDownError::acquire(url.as_str(), format!("decode failed: {e:?}")))?;

        let mime = parsed.mime_type();
        let mime_string = format!("{}/{}", mime.type_, mime.subtype);
        let charset = mime
            .parameters
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("charset"))
            .map(|(_, v)| v.clone());

        Ok(SourceContent {
            bytes,
            mime: Some(mime_string),
            filename: None,
            charset,
            origin: Origin::Inline,
        })
    }
}

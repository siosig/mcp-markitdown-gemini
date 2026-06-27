//! EPub conversion. Retrieves chapter HTML in spine order, converts each to Markdown using `htmd`, and concatenates the results.

use std::io::Cursor;

use epub::doc::EpubDoc;

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;

/// Converts each chapter of an EPub to Markdown and concatenates the results.
pub struct EpubConverter;

impl Converter for EpubConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let mut doc = EpubDoc::from_reader(Cursor::new(src.bytes.clone()))
            .map_err(|e| MarkItDownError::decode("epub", e.to_string()))?;

        let title = doc.mdata("title").map(|item| item.value.clone());
        let mut markdown = String::new();

        loop {
            if let Some((content, _mime)) = doc.get_current_str() {
                if let Ok(part) = htmd::convert(&content) {
                    let trimmed = part.trim();
                    if !trimmed.is_empty() {
                        markdown.push_str(trimmed);
                        markdown.push_str("\n\n");
                    }
                }
            }
            if !doc.go_next() {
                break;
            }
        }

        Ok(ConversionResult {
            markdown: markdown.trim_end().to_string(),
            title,
        })
    }
}

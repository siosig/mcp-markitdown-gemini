//! PDF conversion. Extracts body text using `pdf-extract` (FR-006a: exact layout/table reproduction is not guaranteed).

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;

/// Extracts body text from a PDF.
pub struct PdfConverter;

impl Converter for PdfConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let text = pdf_extract::extract_text_from_mem(&src.bytes)
            .map_err(|e| MarkItDownError::decode("pdf", e.to_string()))?;
        Ok(ConversionResult::text(text.trim().to_string()))
    }
}

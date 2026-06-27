//! PDF 変換。`pdf-extract` で本文テキストを抽出する (FR-006a: レイアウト/表完全再現は非保証)。

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;

/// PDF から本文テキストを抽出する。
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

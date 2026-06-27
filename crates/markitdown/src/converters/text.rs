//! プレーンテキスト変換 (エンコーディング検出付き, FR-015)。

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::decode_text;

/// テキストをそのまま Markdown 本文として返す。
pub struct TextConverter;

impl Converter for TextConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let text = decode_text(&src.bytes, src.charset.as_deref());
        Ok(ConversionResult::text(text))
    }
}

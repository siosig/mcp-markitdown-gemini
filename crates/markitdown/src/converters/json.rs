//! JSON 変換 → 整形した ```json コードフェンス。

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::decode_text;

/// JSON を整形して Markdown コードフェンスに包む。
pub struct JsonConverter;

impl Converter for JsonConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let text = decode_text(&src.bytes, src.charset.as_deref());
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| MarkItDownError::decode("json", e.to_string()))?;
        let pretty = serde_json::to_string_pretty(&value)
            .map_err(|e| MarkItDownError::decode("json", e.to_string()))?;
        Ok(ConversionResult::text(format!("```json\n{pretty}\n```\n")))
    }
}

//! CSV / TSV 変換 → Markdown 表。

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::{decode_text, markdown_table};

/// CSV を Markdown 表へ変換する。
pub struct CsvConverter;

impl Converter for CsvConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let text = decode_text(&src.bytes, src.charset.as_deref());

        // 拡張子が tsv のときはタブ区切り。
        let delimiter = match src.extension().as_deref() {
            Some("tsv") => b'\t',
            _ => b',',
        };

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .delimiter(delimiter)
            .from_reader(text.as_bytes());

        let mut rows: Vec<Vec<String>> = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|e| MarkItDownError::decode("csv", e.to_string()))?;
            rows.push(record.iter().map(|f| f.to_string()).collect());
        }

        Ok(ConversionResult::text(markdown_table(&rows)))
    }
}

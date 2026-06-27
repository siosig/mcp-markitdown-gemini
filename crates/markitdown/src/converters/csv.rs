//! CSV / TSV conversion to Markdown table.

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::{decode_text, markdown_table};

/// Converts CSV to a Markdown table.
pub struct CsvConverter;

impl Converter for CsvConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let text = decode_text(&src.bytes, src.charset.as_deref());

        // Use tab delimiter when the file extension is tsv.
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

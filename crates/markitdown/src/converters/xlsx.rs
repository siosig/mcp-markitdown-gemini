//! Excel (XLSX/XLSM/XLS/ODS) 変換。`calamine` でシートを Markdown 表へ。

use std::io::Cursor;

use calamine::{Data, Reader};

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::markdown_table;

/// 各シートを `## SheetName` 見出し + Markdown 表へ変換する。
pub struct XlsxConverter;

impl Converter for XlsxConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let cursor = Cursor::new(src.bytes.clone());
        let mut workbook = calamine::open_workbook_auto_from_rs(cursor)
            .map_err(|e| MarkItDownError::decode("xlsx", e.to_string()))?;

        let mut markdown = String::new();
        for sheet_name in workbook.sheet_names() {
            let range = workbook
                .worksheet_range(&sheet_name)
                .map_err(|e| MarkItDownError::decode("xlsx", e.to_string()))?;

            markdown.push_str(&format!("## {sheet_name}\n\n"));

            let rows: Vec<Vec<String>> = range
                .rows()
                .map(|row| row.iter().map(format_cell).collect())
                .collect();

            if rows.is_empty() {
                markdown.push_str("_(empty sheet)_\n\n");
            } else {
                markdown.push_str(&markdown_table(&rows));
                markdown.push('\n');
            }
        }

        Ok(ConversionResult::text(markdown.trim_end().to_string()))
    }
}

/// セル値を文字列化する。空セルは空文字。
fn format_cell(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        other => other.to_string(),
    }
}

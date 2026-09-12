//! DOCX conversion. Parses `word/document.xml` and converts headings, paragraphs, lists, and tables to Markdown.

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::{decode_xml_text, local_name, markdown_table, read_zip_entry};

/// Converts a DOCX file to Markdown.
pub struct DocxConverter;

/// Returns the Markdown heading level from a pStyle styleId. `Title` maps to 1, `HeadingN` maps to N.
fn heading_level(style: &str) -> Option<usize> {
    let lower = style.to_ascii_lowercase();
    if lower == "title" {
        return Some(1);
    }
    let digits = lower.strip_prefix("heading")?;
    let level: usize = digits.trim().parse().ok()?;
    (1..=6).contains(&level).then_some(level)
}

#[derive(Default)]
struct ParaState {
    text: String,
    style: Option<String>,
    is_list: bool,
}

impl Converter for DocxConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let xml = read_zip_entry(&src.bytes, "word/document.xml")
            .ok_or_else(|| MarkItDownError::decode("docx", "missing word/document.xml"))?;

        let mut reader = Reader::from_reader(xml.as_slice());
        reader.config_mut().trim_text(false);

        let mut markdown = String::new();
        let mut buf = Vec::new();

        let mut para = ParaState::default();
        let mut in_table = false;
        let mut table_rows: Vec<Vec<String>> = Vec::new();
        let mut cur_row: Vec<String> = Vec::new();
        let mut cell_text = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => match local_name(e.name().as_ref()) {
                    "tbl" => {
                        in_table = true;
                        table_rows.clear();
                    }
                    "tr" => cur_row.clear(),
                    "tc" => cell_text.clear(),
                    "numPr" => para.is_list = true,
                    _ => {}
                },
                Ok(Event::Empty(e)) => {
                    if local_name(e.name().as_ref()) == "pStyle" {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()) == "val" {
                                para.style = Some(attr.value.into_owned());
                            }
                        }
                    }
                }
                Ok(Event::Text(t)) => {
                    let text = decode_xml_text(&t);
                    if in_table {
                        cell_text.push_str(&text);
                    } else {
                        para.text.push_str(&text);
                    }
                }
                Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                    "tc" => cur_row.push(cell_text.trim().to_string()),
                    "tr" => table_rows.push(std::mem::take(&mut cur_row)),
                    "tbl" => {
                        in_table = false;
                        markdown.push_str(&markdown_table(&table_rows));
                        markdown.push('\n');
                    }
                    "p" if !in_table => {
                        emit_paragraph(&mut markdown, &para);
                        para = ParaState::default();
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => return Err(MarkItDownError::decode("docx", e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(ConversionResult::text(markdown.trim().to_string()))
    }
}

/// Writes a single paragraph to Markdown.
fn emit_paragraph(markdown: &mut String, para: &ParaState) {
    let text = para.text.trim();
    if text.is_empty() {
        return;
    }
    if let Some(level) = para.style.as_deref().and_then(heading_level) {
        markdown.push_str(&"#".repeat(level));
        markdown.push(' ');
        markdown.push_str(text);
        markdown.push_str("\n\n");
    } else if para.is_list {
        markdown.push_str("- ");
        markdown.push_str(text);
        markdown.push('\n');
    } else {
        markdown.push_str(text);
        markdown.push_str("\n\n");
    }
}

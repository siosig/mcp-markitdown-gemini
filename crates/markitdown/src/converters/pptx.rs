//! PPTX conversion. Parses `ppt/slides/slideN.xml` files in numeric order and converts text and tables to Markdown.

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::{decode_xml_text, list_zip_entries, local_name, markdown_table, read_zip_entry};

/// Converts a PPTX file to Markdown, one slide at a time.
pub struct PptxConverter;

/// Extracts the numeric index 12 from `ppt/slides/slide12.xml` (used for sorting).
fn slide_number(name: &str) -> u32 {
    name.trim_start_matches("ppt/slides/slide")
        .trim_end_matches(".xml")
        .parse()
        .unwrap_or(u32::MAX)
}

impl Converter for PptxConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let mut slides: Vec<String> = list_zip_entries(&src.bytes, "ppt/slides/slide")
            .into_iter()
            .filter(|n| n.ends_with(".xml"))
            .collect();
        slides.sort_by_key(|n| slide_number(n));

        if slides.is_empty() {
            return Err(MarkItDownError::decode("pptx", "no slides found"));
        }

        let mut markdown = String::new();
        for (idx, slide) in slides.iter().enumerate() {
            markdown.push_str(&format!("## Slide {}\n\n", idx + 1));
            if let Some(xml) = read_zip_entry(&src.bytes, slide) {
                markdown.push_str(&parse_slide(&xml)?);
            }
            markdown.push('\n');
        }

        Ok(ConversionResult::text(markdown.trim().to_string()))
    }
}

/// Extracts text paragraphs and tables from a single slide's XML.
fn parse_slide(xml: &[u8]) -> Result<String, MarkItDownError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut out = String::new();
    let mut buf = Vec::new();

    let mut para = String::new();
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
                _ => {}
            },
            Ok(Event::Text(t)) => {
                let text = decode_xml_text(&t);
                if in_table {
                    cell_text.push_str(&text);
                } else {
                    para.push_str(&text);
                }
            }
            Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                "tc" => cur_row.push(cell_text.trim().to_string()),
                "tr" => table_rows.push(std::mem::take(&mut cur_row)),
                "tbl" => {
                    in_table = false;
                    out.push_str(&markdown_table(&table_rows));
                    out.push('\n');
                }
                // Finalize the text line at the end of a:p (paragraph). Text inside tables is handled on the cell side.
                "p" => {
                    if !in_table {
                        let line = para.trim();
                        if !line.is_empty() {
                            out.push_str(line);
                            out.push_str("\n\n");
                        }
                    }
                    para.clear();
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(MarkItDownError::decode("pptx", e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(out)
}

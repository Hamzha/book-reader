use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use super::Extracted;
use crate::htmlutil::plain_to_html;

pub fn extract(path: &Path) -> Result<Extracted, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|_| "This DOCX has no document.xml.".to_string())?
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;
    let text = docx_xml_to_text(&xml)?;
    let title = super::title_from_path(path);
    Ok(Extracted::reflow(title, "Unknown".into(), plain_to_html(&text)))
}

fn docx_xml_to_text(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut in_text = false;
    let mut paragraph = String::new();
    let mut paragraphs = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = local_name(e.local_name().as_ref());
                if name == "t" {
                    in_text = true;
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.local_name().as_ref());
                if name == "br" || name == "cr" {
                    paragraph.push('\n');
                }
                if name == "tab" {
                    paragraph.push('\t');
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.local_name().as_ref());
                if name == "t" {
                    in_text = false;
                }
                if name == "p" {
                    let trimmed = paragraph.trim();
                    if !trimmed.is_empty() {
                        paragraphs.push(trimmed.to_string());
                    }
                    paragraph.clear();
                }
            }
            Ok(Event::Text(t)) => {
                if in_text {
                    paragraph.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
        buf.clear();
    }
    Ok(paragraphs.join("\n\n"))
}

fn local_name(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(test)]
mod tests {
    use super::docx_xml_to_text;

    #[test]
    fn reads_paragraphs() {
        let xml = r#"<w:document xmlns:w="http://x"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p><w:p><w:r><w:t>World</w:t></w:r></w:p></w:body></w:document>"#;
        let text = docx_xml_to_text(xml).unwrap();
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }
}

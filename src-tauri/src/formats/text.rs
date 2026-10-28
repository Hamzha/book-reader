use std::path::Path;

use super::{Extracted, Format};
use crate::htmlutil::{extract_body, plain_to_html};

pub fn extract(path: &Path, format: Format) -> Result<Extracted, String> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let (text, _, _) = encoding_rs::UTF_8.decode(&raw);
    let text = if text.contains('\u{fffd}') {
        let (win, _, _) = encoding_rs::WINDOWS_1252.decode(&raw);
        win.to_string()
    } else {
        text.to_string()
    };
    let title = super::title_from_path(path);
    let html = match format {
        Format::Markdown => {
            let parser = pulldown_cmark::Parser::new(&text);
            let mut html = String::new();
            pulldown_cmark::html::push_html(&mut html, parser);
            html
        }
        Format::Html => extract_body(&text),
        _ => plain_to_html(&text),
    };
    Ok(Extracted::reflow(title, "Unknown".into(), html))
}

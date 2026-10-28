use std::path::Path;

use lopdf::Object;

use super::Extracted;

pub fn extract(path: &Path) -> Result<Extracted, String> {
    let doc = lopdf::Document::load(path).map_err(|e| format!("Could not open this PDF: {e}"))?;
    let pages = doc.get_pages();
    let mut numbers: Vec<u32> = pages.keys().copied().collect();
    numbers.sort_unstable();
    let mut plain_pages = Vec::new();
    for number in &numbers {
        let text = doc.extract_text(&[*number]).unwrap_or_default();
        plain_pages.push(text);
    }
    let _ = Object::Null;
    Ok(Extracted {
        title: super::title_from_path(path),
        author: "Unknown".into(),
        chapters: Vec::new(),
        toc: numbers
            .iter()
            .enumerate()
            .map(|(i, _)| crate::model::TocItem {
                title: format!("Page {}", i + 1),
                chapter: i,
            })
            .collect(),
        cover: None,
        cover_ext: "jpg".into(),
        images: Vec::new(),
        page_count: numbers.len() as i64,
        plain_pages,
    })
}

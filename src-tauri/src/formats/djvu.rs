use std::path::Path;

use super::Extracted;

pub fn extract(path: &Path) -> Result<Extracted, String> {
    let doc = djvu_rs::Document::open(path).map_err(|e| format!("Could not open this DJVU file: {e}"))?;
    let count = doc.page_count();
    if count == 0 {
        return Err("This DJVU file has no pages.".into());
    }
    let mut plain_pages = Vec::with_capacity(count);
    let mut toc = Vec::with_capacity(count);
    for index in 0..count {
        let text = doc
            .page(index)
            .ok()
            .and_then(|page| page.text().ok().flatten())
            .unwrap_or_default();
        plain_pages.push(text);
        toc.push(crate::model::TocItem {
            title: format!("Page {}", index + 1),
            chapter: index,
        });
    }
    Ok(Extracted {
        title: super::title_from_path(path),
        author: "Unknown".into(),
        chapters: Vec::new(),
        toc,
        cover: None,
        cover_ext: "png".into(),
        images: Vec::new(),
        page_count: count as i64,
        plain_pages,
    })
}

pub fn render_page(path: &Path, index: usize) -> Result<Vec<u8>, String> {
    let doc = djvu_rs::Document::open(path).map_err(|e| e.to_string())?;
    let page = doc.page(index).map_err(|e| e.to_string())?;
    let pixmap = page.render().map_err(|e| e.to_string())?;
    let width = page.width();
    let height = page.height();
    let data = pixmap_bytes(&pixmap)?;
    encode_png(width, height, data)
}

fn pixmap_bytes(pixmap: &djvu_rs::Pixmap) -> Result<Vec<u8>, String> {
    if !pixmap.data.is_empty() {
        return Ok(pixmap.data.clone());
    }
    Err("This DJVU page rendered with no pixels.".into())
}

fn encode_png(width: u32, height: u32, data: Vec<u8>) -> Result<Vec<u8>, String> {
    let image = image::RgbaImage::from_raw(width, height, data)
        .ok_or_else(|| "The DJVU page size did not match its pixels.".to_string())?;
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(bytes.into_inner())
}

use std::path::{Path, PathBuf};

use super::{Chapter, Extracted, PackedFile};
use crate::htmlutil::extract_body;
use crate::model::TocItem;

pub fn extract(path: &Path, asset_base: &str) -> Result<Extracted, String> {
    let mut doc = epub::doc::EpubDoc::new(path).map_err(|e| e.to_string())?;
    let title = doc
        .get_title()
        .unwrap_or_else(|| super::title_from_path(path));
    let author = doc
        .mdata("creator")
        .map(|item| item.value.clone())
        .unwrap_or_else(|| "Unknown".into());
    let cover = doc.get_cover().map(|(bytes, _mime)| bytes);

    let mut images = Vec::new();
    let resource_paths: Vec<(String, PathBuf)> = doc
        .resources
        .iter()
        .map(|(id, item)| (id.clone(), item.path.clone()))
        .collect();
    for (index, (_id, res_path)) in resource_paths.iter().enumerate() {
        let name = res_path.to_string_lossy().replace('\\', "/");
        if !super::is_image_name(&name) {
            continue;
        }
        if let Some(bytes) = doc.get_resource_by_path(res_path) {
            let file_name = super::safe_file_name(&name, index);
            images.push(PackedFile {
                name: file_name,
                bytes,
            });
        }
    }

    let spine_len = doc.spine.len();
    let mut chapters = Vec::new();
    for index in 0..spine_len {
        if !doc.set_current_chapter(index) {
            continue;
        }
        let Some((raw, _mime)) = doc.get_current_str() else {
            continue;
        };
        let current_path = doc.get_current_path().unwrap_or_default();
        let mut html = extract_body(&raw);
        html = rewrite_images(&html, &current_path, &images, &resource_paths, asset_base);
        let title = chapter_title(&html).unwrap_or_else(|| format!("Chapter {}", index + 1));
        chapters.push(Chapter { title, html });
    }
    if chapters.is_empty() {
        return Err("This EPUB has no readable chapters.".into());
    }

    let toc = flatten_toc(&doc.toc, &chapters);
    let toc = if toc.is_empty() {
        chapters
            .iter()
            .enumerate()
            .map(|(i, c)| TocItem {
                title: c.title.clone(),
                chapter: i,
            })
            .collect()
    } else {
        toc
    };

    Ok(Extracted {
        title,
        author,
        page_count: chapters.len() as i64,
        chapters,
        toc,
        cover,
        cover_ext: "jpg".into(),
        images,
        plain_pages: Vec::new(),
    })
}

fn chapter_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    for tag in ["h1", "h2", "title"] {
        let open = format!("<{tag}");
        if let Some(start) = lower.find(&open) {
            let after = html[start..].find('>').map(|p| start + p + 1)?;
            let close = format!("</{tag}>");
            let end = lower[after..].find(&close).map(|p| after + p)?;
            let text = crate::htmlutil::strip_tags(&html[after..end]);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

fn rewrite_images(
    html: &str,
    chapter_path: &Path,
    images: &[PackedFile],
    resources: &[(String, PathBuf)],
    asset_base: &str,
) -> String {
    let mut out = html.to_string();
    let chapter_dir = chapter_path.parent().unwrap_or(Path::new(""));
    for (index, (_id, res_path)) in resources.iter().enumerate() {
        let name = res_path.to_string_lossy().replace('\\', "/");
        if !super::is_image_name(&name) {
            continue;
        }
        let file_name = super::safe_file_name(&name, index);
        if images.iter().all(|img| img.name != file_name) {
            continue;
        }
        let url = format!("{asset_base}{file_name}");
        let rel = res_path
            .strip_prefix(chapter_dir)
            .unwrap_or(res_path)
            .to_string_lossy()
            .replace('\\', "/");
        let file = res_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        for needle in [name.clone(), rel, file] {
            if needle.is_empty() {
                continue;
            }
            out = out.replace(&format!("src=\"{needle}\""), &format!("src=\"{url}\""));
            out = out.replace(&format!("src='{needle}'"), &format!("src='{url}'"));
            out = out.replace(&format!("href=\"{needle}\""), &format!("href=\"{url}\""));
        }
    }
    out
}

fn flatten_toc(points: &[epub::doc::NavPoint], chapters: &[Chapter]) -> Vec<TocItem> {
    let mut out = Vec::new();
    fn walk(points: &[epub::doc::NavPoint], chapters: &[Chapter], out: &mut Vec<TocItem>) {
        for point in points {
            let target = point.content.to_string_lossy().replace('\\', "/");
            let file = Path::new(&target)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let chapter = chapters
                .iter()
                .position(|c| !file.is_empty() && c.html.contains(&file))
                .unwrap_or(0);
            out.push(TocItem {
                title: point.label.clone(),
                chapter,
            });
            walk(&point.children, chapters, out);
        }
    }
    walk(points, chapters, &mut out);
    out
}

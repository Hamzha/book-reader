// UnRAR source code may be used in any software to handle RAR archives without
// limitations free of charge, but cannot be used to develop RAR (WinRAR)
// compatible archiver and to re-create RAR compression algorithm, which is
// proprietary. Distribution of modified UnRAR source code in separate form or
// as a part of other software is permitted, provided that full text of this
// paragraph, starting from "UnRAR source code" words, is included in license,
// or in documentation if license is not available, and in source code comments
// of resulting package.

use std::io::Read;
use std::path::Path;

use super::{Extracted, Format, PackedFile};

pub fn extract(path: &Path, format: Format) -> Result<Extracted, String> {
    let mut images = match format {
        Format::Cbz => read_zip(path)?,
        Format::Cbr => read_rar(path)?,
        Format::Cb7 => read_7z(path)?,
        _ => return Err("Not a comic archive.".into()),
    };
    images.sort_by(|a, b| super::natural_cmp(&a.name, &b.name));
    if images.is_empty() {
        return Err("This comic has no images.".into());
    }
    let images = images
        .into_iter()
        .enumerate()
        .map(|(index, file)| PackedFile {
            name: super::safe_file_name(&file.name, index),
            bytes: file.bytes,
        })
        .collect::<Vec<_>>();
    let page_count = images.len() as i64;
    Ok(Extracted {
        title: super::title_from_path(path),
        author: "Unknown".into(),
        chapters: Vec::new(),
        toc: (0..images.len())
            .map(|i| crate::model::TocItem {
                title: format!("Page {}", i + 1),
                chapter: i,
            })
            .collect(),
        cover: images.first().map(|img| img.bytes.clone()),
        cover_ext: "jpg".into(),
        images,
        page_count,
        plain_pages: Vec::new(),
    })
}

fn read_zip(path: &Path) -> Result<Vec<PackedFile>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut images = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if !super::is_image_name(&name) {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        images.push(PackedFile { name, bytes });
    }
    Ok(images)
}

fn read_rar(path: &Path) -> Result<Vec<PackedFile>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = unrar_rs::RarArchive::open(file).map_err(|e| e.to_string())?;
    let names: Vec<String> = archive.entries().map(|member| member.name.clone()).collect();
    let mut images = Vec::new();
    for (index, name) in names.iter().enumerate() {
        if !super::is_image_name(name) {
            let _ = archive.by_index(index).and_then(|entry| entry.skip());
            continue;
        }
        let mut bytes = Vec::new();
        archive
            .by_index(index)
            .map_err(|e| e.to_string())?
            .copy_to(&mut bytes)
            .map_err(|e| e.to_string())?;
        images.push(PackedFile {
            name: name.clone(),
            bytes,
        });
    }
    Ok(images)
}

fn read_7z(path: &Path) -> Result<Vec<PackedFile>, String> {
    let temp = std::env::temp_dir().join(format!("book-reader-cb7-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let result = (|| {
        sevenz_rust::decompress_file(path, &temp).map_err(|e| e.to_string())?;
        let mut images = Vec::new();
        collect_images(&temp, &temp, &mut images)?;
        Ok(images)
    })();
    let _ = std::fs::remove_dir_all(&temp);
    result
}

fn collect_images(root: &Path, dir: &Path, out: &mut Vec<PackedFile>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_images(root, &path, out)?;
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if super::is_image_name(&rel) {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            out.push(PackedFile { name: rel, bytes });
        }
    }
    Ok(())
}

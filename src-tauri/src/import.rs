use std::path::{Path, PathBuf};

use crate::db::{Db, NewBook};
use crate::formats::{self, Extracted};
use crate::htmlutil::{snippet_around, strip_tags};
use crate::model::*;

pub fn import_paths(db: &Db, data_dir: &Path, paths: &[String]) -> ImportReport {
    let mut files = Vec::new();
    for raw in paths {
        collect(Path::new(raw), &mut files);
    }
    let mut added = Vec::new();
    let mut failed = Vec::new();
    for path in files {
        match import_one(db, data_dir, &path) {
            Ok(book) => added.push(book),
            Err(message) => failed.push(ImportFailure {
                path: path.to_string_lossy().to_string(),
                message,
            }),
        }
    }
    ImportReport { added, failed }
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        if let Ok(read) = std::fs::read_dir(path) {
            for entry in read.flatten() {
                collect(&entry.path(), out);
            }
        }
        return;
    }
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if name.starts_with('.') || name == "Thumbs.db" {
        return;
    }
    if formats::is_supported(path) {
        out.push(path.to_path_buf());
    }
}

fn import_one(db: &Db, data_dir: &Path, path: &Path) -> Result<Book, String> {
    let format = formats::detect(path)?;
    let path_key = normalize(path);
    let existing = db.find_by_path_key(&path_key)?;
    let id = existing.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let package = data_dir.join("books").join(&id);
    if package.exists() {
        let _ = std::fs::remove_dir_all(&package);
    }
    std::fs::create_dir_all(&package).map_err(|e| e.to_string())?;
    let asset_base = format!("http://book.localhost/asset/{id}/");
    let extracted = formats::extract(path, format, &asset_base)?;
    write_package(&package, &extracted)?;
    let cover_path = write_cover(&package, &extracted)?;
    let page_count = if extracted.page_count > 0 {
        extracted.page_count
    } else if !extracted.chapters.is_empty() {
        extracted.chapters.len() as i64
    } else {
        extracted.images.len() as i64
    };
    let new_book = NewBook {
        id: id.clone(),
        path: path.to_string_lossy().to_string(),
        path_key,
        title: extracted.title,
        author: extracted.author,
        format: format.as_str().into(),
        kind: format.kind().into(),
        cover_path,
        added_at: now_ms(),
        page_count,
        package_dir: package.to_string_lossy().to_string(),
    };
    db.upsert_book(&new_book)?;
    db.get_book(&id)?.ok_or_else(|| "The book was imported but could not be loaded.".into())
}

fn write_package(dir: &Path, extracted: &Extracted) -> Result<(), String> {
    let toc_path = dir.join("toc.json");
    std::fs::write(&toc_path, serde_json::to_vec(&extracted.toc).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if !extracted.chapters.is_empty() {
        let chapters = dir.join("chapters");
        std::fs::create_dir_all(&chapters).map_err(|e| e.to_string())?;
        for (index, chapter) in extracted.chapters.iter().enumerate() {
            let file = chapters.join(format!("{index:04}.html"));
            std::fs::write(file, &chapter.html).map_err(|e| e.to_string())?;
            let title = chapters.join(format!("{index:04}.title"));
            std::fs::write(title, &chapter.title).map_err(|e| e.to_string())?;
        }
    }
    if !extracted.images.is_empty() {
        let images = dir.join("images");
        std::fs::create_dir_all(&images).map_err(|e| e.to_string())?;
        for image in &extracted.images {
            std::fs::write(images.join(&image.name), &image.bytes).map_err(|e| e.to_string())?;
        }
    }
    if !extracted.plain_pages.is_empty() {
        let text = dir.join("text");
        std::fs::create_dir_all(&text).map_err(|e| e.to_string())?;
        for (index, page) in extracted.plain_pages.iter().enumerate() {
            std::fs::write(text.join(format!("{index:04}.txt")), page).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn write_cover(dir: &Path, extracted: &Extracted) -> Result<Option<String>, String> {
    let Some(bytes) = &extracted.cover else {
        return Ok(None);
    };
    let ext = sniff_ext(bytes);
    let name = format!("cover.{ext}");
    let path = dir.join(&name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().to_string()))
}

pub fn chapter_html(db: &Db, book_id: &str, index: i64) -> Result<String, String> {
    let package = package_of(db, book_id)?;
    let path = package.join("chapters").join(format!("{index:04}.html"));
    std::fs::read_to_string(path).map_err(|_| "That chapter is missing.".to_string())
}

pub fn chapter_count(package: &Path) -> i64 {
    let dir = package.join("chapters");
    std::fs::read_dir(dir)
        .map(|read| {
            read.flatten()
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("html"))
                .count() as i64
        })
        .unwrap_or(0)
}

pub fn image_count(package: &Path) -> i64 {
    let dir = package.join("images");
    std::fs::read_dir(dir)
        .map(|read| read.flatten().filter(|e| e.path().is_file()).count() as i64)
        .unwrap_or(0)
}

pub fn load_toc(package: &Path) -> Vec<TocItem> {
    let path = package.join("toc.json");
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn search_book(db: &Db, book_id: &str, query: &str) -> Result<Vec<SearchHit>, String> {
    let query = query.trim();
    if query.len() < 2 {
        return Ok(Vec::new());
    }
    let package = package_of(db, book_id)?;
    let mut hits = Vec::new();
    let chapters = package.join("chapters");
    if chapters.is_dir() {
        let mut files: Vec<_> = std::fs::read_dir(&chapters)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("html"))
            .collect();
        files.sort();
        for path in files {
            let index: i64 = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let html = std::fs::read_to_string(&path).unwrap_or_default();
            let text = strip_tags(&html);
            if let Some(snippet) = snippet_around(&text, query) {
                hits.push(SearchHit {
                    chapter: index,
                    snippet,
                });
            }
            if hits.len() >= 40 {
                break;
            }
        }
    }
    let text_dir = package.join("text");
    if hits.is_empty() && text_dir.is_dir() {
        let mut files: Vec<_> = std::fs::read_dir(&text_dir)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .collect();
        files.sort();
        for path in files {
            let index: i64 = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            if let Some(snippet) = snippet_around(&text, query) {
                hits.push(SearchHit {
                    chapter: index,
                    snippet,
                });
            }
            if hits.len() >= 40 {
                break;
            }
        }
    }
    Ok(hits)
}

pub fn package_of(db: &Db, book_id: &str) -> Result<PathBuf, String> {
    let dir = db
        .package_dir(book_id)?
        .ok_or_else(|| "That book is not in the library.".to_string())?;
    Ok(PathBuf::from(dir))
}

pub fn normalize(path: &Path) -> String {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    abs.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_lowercase()
}

pub fn export_notebook(db: &Db, book_id: &str, format: &str) -> Result<String, String> {
    let book = db.get_book(book_id)?.ok_or_else(|| "That book is not in the library.".to_string())?;
    let notes = db.list_annotations(book_id)?;
    let mut out = String::new();
    if format == "md" {
        out.push_str(&format!("# {}\n\n", book.title));
    } else {
        out.push_str(&book.title);
        out.push_str("\n\n");
    }
    for note in notes {
        let place = format!("chapter {}, page {}", note.locator.chapter + 1, note.locator.page + 1);
        match note.kind.as_str() {
            "highlight" => {
                let quote = note.text.unwrap_or_default();
                if format == "md" {
                    out.push_str(&format!("- Highlight ({}) — {place}\n  > {quote}\n", note.color.unwrap_or_else(|| "yellow".into())));
                } else {
                    out.push_str(&format!("Highlight ({place}): {quote}\n"));
                }
            }
            "note" => {
                let body = note.note.unwrap_or_default();
                let quote = note.text.unwrap_or_default();
                if format == "md" {
                    out.push_str(&format!("- Note — {place}\n  > {quote}\n  {body}\n"));
                } else {
                    out.push_str(&format!("Note ({place}): {quote}\n{body}\n"));
                }
            }
            "bookmark" => {
                if format == "md" {
                    out.push_str(&format!("- Bookmark — {place}\n"));
                } else {
                    out.push_str(&format!("Bookmark ({place})\n"));
                }
            }
            "stamp" => {
                let stamp = note.stamp.unwrap_or_else(|| "star".into());
                if format == "md" {
                    out.push_str(&format!("- Stamp: {stamp} — {place}\n"));
                } else {
                    out.push_str(&format!("Stamp {stamp} ({place})\n"));
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::import_paths;
    use crate::db::Db;

    #[test]
    fn imports_plain_text_without_deleting_the_original() {
        let dir = std::env::temp_dir().join(format!("book-reader-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let book = dir.join("hello.txt");
        std::fs::write(&book, "Hello world\n\nA second paragraph.").unwrap();
        let db = Db::open(&dir.join("library.sqlite")).unwrap();
        let report = import_paths(&db, &dir.join("data"), &[book.to_string_lossy().to_string()]);
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert_eq!(report.added.len(), 1);
        assert_eq!(report.added[0].format, "txt");
        let html = super::chapter_html(&db, &report.added[0].id, 0).unwrap();
        assert!(html.contains("Hello world"));
        assert!(book.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn sniff_ext(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if bytes.starts_with(b"GIF8") {
        "gif"
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" {
        "webp"
    } else {
        "jpg"
    }
}

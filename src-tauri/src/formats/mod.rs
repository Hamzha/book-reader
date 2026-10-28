use std::path::{Path, PathBuf};

use serde::Serialize;

pub mod comic;
pub mod djvu;
pub mod docx;
pub mod epub;
pub mod fb2;
pub mod mobi;
pub mod pdf;
pub mod rtf;
pub mod text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Epub,
    Html,
    Markdown,
    Txt,
    Docx,
    Rtf,
    Fb2,
    Mobi,
    Azw,
    Azw3,
    Pdf,
    Djvu,
    Cbz,
    Cbr,
    Cb7,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Epub => "epub",
            Format::Html => "html",
            Format::Markdown => "md",
            Format::Txt => "txt",
            Format::Docx => "docx",
            Format::Rtf => "rtf",
            Format::Fb2 => "fb2",
            Format::Mobi => "mobi",
            Format::Azw => "azw",
            Format::Azw3 => "azw3",
            Format::Pdf => "pdf",
            Format::Djvu => "djvu",
            Format::Cbz => "cbz",
            Format::Cbr => "cbr",
            Format::Cb7 => "cb7",
        }
    }

    pub fn kind(self) -> &'static str {
        match self {
            Format::Pdf => "document",
            Format::Djvu => "djvu",
            Format::Cbz | Format::Cbr | Format::Cb7 => "comic",
            _ => "reflow",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub title: String,
    pub html: String,
}

#[derive(Debug, Clone)]
pub struct PackedFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Extracted {
    pub title: String,
    pub author: String,
    pub chapters: Vec<Chapter>,
    pub toc: Vec<crate::model::TocItem>,
    pub cover: Option<Vec<u8>>,
    #[allow(dead_code)]
    pub cover_ext: String,
    pub images: Vec<PackedFile>,
    pub page_count: i64,
    pub plain_pages: Vec<String>,
}

impl Extracted {
    pub fn reflow(title: String, author: String, html: String) -> Self {
        Self {
            title,
            author,
            chapters: vec![Chapter {
                title: "Text".into(),
                html,
            }],
            toc: vec![crate::model::TocItem {
                title: "Text".into(),
                chapter: 0,
            }],
            cover: None,
            cover_ext: "jpg".into(),
            images: Vec::new(),
            page_count: 1,
            plain_pages: Vec::new(),
        }
    }
}

pub fn detect(path: &Path) -> Result<Format, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let by_ext = match ext.as_str() {
        "epub" => Some(Format::Epub),
        "html" | "htm" => Some(Format::Html),
        "md" | "markdown" => Some(Format::Markdown),
        "txt" | "text" => Some(Format::Txt),
        "docx" => Some(Format::Docx),
        "rtf" => Some(Format::Rtf),
        "fb2" => Some(Format::Fb2),
        "mobi" => Some(Format::Mobi),
        "azw" => Some(Format::Azw),
        "azw3" | "kf8" => Some(Format::Azw3),
        "pdf" => Some(Format::Pdf),
        "djvu" | "djv" => Some(Format::Djvu),
        "cbz" => Some(Format::Cbz),
        "cbr" => Some(Format::Cbr),
        "cb7" => Some(Format::Cb7),
        _ => None,
    };
    if let Some(fmt) = by_ext {
        return Ok(fmt);
    }
    let header = read_header(path, 128)?;
    if header.starts_with(b"%PDF") {
        return Ok(Format::Pdf);
    }
    if header.starts_with(b"Rar!") || header.starts_with(b"\x52\x61\x72\x21\x1a\x07") {
        return Ok(Format::Cbr);
    }
    if header.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Ok(Format::Cb7);
    }
    if header.starts_with(b"AT&TFORM")
        || (header.len() >= 12 && header.starts_with(b"FORM") && (&header[8..12] == b"DJVU" || &header[8..12] == b"DJVM"))
    {
        return Ok(Format::Djvu);
    }
    if header.windows(8).any(|w| w == b"BOOKMOBI" || w == b"TEXtREAd") {
        return Ok(Format::Mobi);
    }
    if header.starts_with(b"{\\rtf") {
        return Ok(Format::Rtf);
    }
    if header.starts_with(b"PK") {
        return sniff_zip(path);
    }
    let text = String::from_utf8_lossy(&header).to_ascii_lowercase();
    if text.contains("<fictionbook") || text.contains("<html") || text.contains("<?xml") {
        if text.contains("fictionbook") {
            return Ok(Format::Fb2);
        }
        return Ok(Format::Html);
    }
    Err("This file is not a supported book.".into())
}

pub fn is_supported(path: &Path) -> bool {
    detect(path).is_ok()
}

fn read_header(path: &Path, n: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; n];
    let got = file.read(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(got);
    Ok(buf)
}

fn sniff_zip(path: &Path) -> Result<Format, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut names = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            names.push(entry.name().to_string());
        }
    }
    if names.iter().any(|n| n == "mimetype") {
        if let Ok(mut mimetype) = archive.by_name("mimetype") {
            use std::io::Read;
            let mut buf = String::new();
            mimetype.read_to_string(&mut buf).ok();
            if buf.contains("epub") {
                return Ok(Format::Epub);
            }
        }
    }
    if names.iter().any(|n| n.ends_with("word/document.xml")) {
        return Ok(Format::Docx);
    }
    if names.iter().any(|n| is_image_name(n)) {
        return Ok(Format::Cbz);
    }
    Err("This zip file is not an EPUB, DOCX, or comic.".into())
}

pub fn extract(path: &Path, format: Format, asset_base: &str) -> Result<Extracted, String> {
    match format {
        Format::Epub => epub::extract(path, asset_base),
        Format::Html | Format::Markdown | Format::Txt => text::extract(path, format),
        Format::Docx => docx::extract(path),
        Format::Rtf => rtf::extract(path),
        Format::Fb2 => fb2::extract(path),
        Format::Mobi | Format::Azw | Format::Azw3 => mobi::extract(path),
        Format::Pdf => pdf::extract(path),
        Format::Djvu => djvu::extract(path),
        Format::Cbz | Format::Cbr | Format::Cb7 => comic::extract(path, format),
    }
}

pub fn is_image_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.contains("__macosx") || lower.ends_with('/') {
        return false;
    }
    matches!(
        Path::new(&lower)
            .extension()
            .and_then(|e| e.to_str()),
        Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp")
    )
}

pub fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string()
}

pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let ac: Vec<Chunk> = chunks(a);
    let bc: Vec<Chunk> = chunks(b);
    ac.cmp(&bc)
}

#[derive(PartialEq, Eq)]
enum Chunk {
    Text(String),
    Num(u64, String),
}

impl PartialOrd for Chunk {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Chunk {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Chunk::Num(a, _), Chunk::Num(b, _)) => a.cmp(b),
            (Chunk::Text(a), Chunk::Text(b)) => a.cmp(b),
            (Chunk::Num(_, _), Chunk::Text(_)) => std::cmp::Ordering::Less,
            (Chunk::Text(_), Chunk::Num(_, _)) => std::cmp::Ordering::Greater,
        }
    }
}

fn chunks(s: &str) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.peek().copied() {
        if c.is_ascii_digit() {
            let mut raw = String::new();
            while let Some(d) = chars.peek().copied() {
                if d.is_ascii_digit() {
                    raw.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            let n = raw.parse::<u64>().unwrap_or(0);
            out.push(Chunk::Num(n, raw));
        } else {
            let mut raw = String::new();
            while let Some(d) = chars.peek().copied() {
                if d.is_ascii_digit() {
                    break;
                }
                raw.push(d.to_ascii_lowercase());
                chars.next();
            }
            out.push(Chunk::Text(raw));
        }
    }
    out
}

pub fn safe_file_name(name: &str, index: usize) -> String {
    let path = PathBuf::from(name.replace('\\', "/"));
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    let ext = ext.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
    format!("{index:04}.{ext}")
}

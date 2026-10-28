use std::path::Path;

use base64::Engine;
use quick_xml::events::Event;
use quick_xml::Reader;

use super::{Chapter, Extracted};
use crate::htmlutil::plain_to_html;
use crate::model::TocItem;

pub fn extract(path: &Path) -> Result<Extracted, String> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let (xml, _, _) = encoding_rs::UTF_8.decode(&raw);
    parse_fb2(&xml)
}

fn parse_fb2(xml: &str) -> Result<Extracted, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut title = String::new();
    let mut first = String::new();
    let mut last = String::new();
    let mut section_title = String::new();
    let mut collecting_section_title = false;
    let mut paragraph = String::new();
    let mut in_p = false;
    let mut chapters: Vec<(String, Vec<String>)> = Vec::new();
    let mut current_paras: Vec<String> = Vec::new();
    let mut current_title = "Chapter".to_string();
    let mut cover: Option<Vec<u8>> = None;
    let mut binary_id = String::new();
    let mut binary_buf = String::new();
    let mut in_binary = false;
    let mut cover_id = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if name == "section" {
                    if !current_paras.is_empty() {
                        chapters.push((current_title.clone(), current_paras.clone()));
                        current_paras.clear();
                    }
                    current_title = "Chapter".into();
                }
                if name == "title" && path.iter().any(|p| p == "section") {
                    collecting_section_title = true;
                    section_title.clear();
                }
                if name == "p" || name == "v" || name == "subtitle" {
                    in_p = true;
                    paragraph.clear();
                }
                if name == "binary" {
                    in_binary = true;
                    binary_buf.clear();
                    binary_id.clear();
                    for attr in e.attributes().flatten() {
                        if attr.key.local_name().as_ref() == b"id" {
                            binary_id = String::from_utf8_lossy(&attr.value).to_string();
                        }
                    }
                }
                if name == "image" && cover_id.is_empty() {
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                        if key == "href" {
                            let value = String::from_utf8_lossy(&attr.value).to_string();
                            cover_id = value.trim_start_matches('#').to_string();
                        }
                    }
                }
                path.push(name);
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if (name == "p" || name == "v" || name == "subtitle") && in_p {
                    let text = paragraph.trim().to_string();
                    if collecting_section_title && !text.is_empty() {
                        if !section_title.is_empty() {
                            section_title.push(' ');
                        }
                        section_title.push_str(&text);
                    } else if path.iter().any(|p| p == "body") && !text.is_empty() && !path.iter().any(|p| p == "title-info") {
                        current_paras.push(text);
                    }
                    in_p = false;
                    paragraph.clear();
                }
                if name == "title" && collecting_section_title {
                    collecting_section_title = false;
                    if !section_title.trim().is_empty() {
                        current_title = section_title.trim().to_string();
                    }
                }
                if name == "section" && !current_paras.is_empty() {
                    chapters.push((current_title.clone(), std::mem::take(&mut current_paras)));
                    current_title = "Chapter".into();
                }
                if name == "binary" {
                    in_binary = false;
                    if !cover_id.is_empty() && binary_id == cover_id {
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(binary_buf.trim()) {
                            cover = Some(bytes);
                        }
                    } else if cover.is_none() && !binary_buf.trim().is_empty() && cover_id.is_empty() {
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(binary_buf.trim()) {
                            cover = Some(bytes);
                        }
                    }
                    binary_buf.clear();
                }
                path.pop();
            }
            Ok(Event::Text(t)) => {
                let text = t.unescape().unwrap_or_default();
                if in_binary {
                    binary_buf.push_str(&text);
                } else if in_p {
                    paragraph.push_str(&text);
                } else if path.last().map(|s| s.as_str()) == Some("book-title") {
                    title.push_str(&text);
                } else if path.last().map(|s| s.as_str()) == Some("first-name") {
                    first.push_str(&text);
                } else if path.last().map(|s| s.as_str()) == Some("last-name") {
                    last.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
        buf.clear();
    }
    if !current_paras.is_empty() {
        chapters.push((current_title, current_paras));
    }
    if chapters.is_empty() {
        chapters.push(("Text".into(), vec![]));
    }
    let author = format!("{first} {last}").trim().to_string();
    let author = if author.is_empty() { "Unknown".into() } else { author };
    let title = if title.trim().is_empty() { "Untitled".into() } else { title.trim().to_string() };
    let mut toc = Vec::new();
    let chapters = chapters
        .into_iter()
        .enumerate()
        .map(|(i, (name, paras))| {
            toc.push(TocItem {
                title: name.clone(),
                chapter: i,
            });
            let html = if paras.is_empty() {
                "<p></p>".into()
            } else {
                plain_to_html(&paras.join("\n\n"))
            };
            Chapter { title: name, html }
        })
        .collect();
    Ok(Extracted {
        title,
        author,
        chapters,
        toc,
        cover,
        cover_ext: "jpg".into(),
        images: Vec::new(),
        page_count: 0,
        plain_pages: Vec::new(),
    })
}

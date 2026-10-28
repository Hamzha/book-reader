use std::path::Path;

use super::Extracted;
use crate::htmlutil::{extract_body, plain_to_html};

pub fn extract(path: &Path) -> Result<Extracted, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let book = parse_mobi(&data)?;
    let title = if book.title.trim().is_empty() {
        super::title_from_path(path)
    } else {
        book.title
    };
    let author = if book.author.trim().is_empty() {
        "Unknown".into()
    } else {
        book.author
    };
    let html = if looks_like_html(&book.text) {
        extract_body(&book.text)
    } else {
        plain_to_html(&book.text)
    };
    if html.trim().is_empty() {
        return Err("This Kindle file has no readable text.".into());
    }
    Ok(Extracted::reflow(title, author, html))
}

#[derive(Debug)]
struct MobiText {
    title: String,
    author: String,
    text: String,
}

fn looks_like_html(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("<html") || lower.contains("<body") || lower.contains("<p")
}

fn parse_mobi(data: &[u8]) -> Result<MobiText, String> {
    if data.len() < 100 {
        return Err("This Kindle file is too small to read.".into());
    }
    let num_records = u16be(data, 76) as usize;
    if num_records < 2 || 78 + num_records * 8 > data.len() {
        return Err("This Kindle file is not a valid MOBI book.".into());
    }
    let mut offsets = Vec::with_capacity(num_records + 1);
    for i in 0..num_records {
        offsets.push(u32be(data, 78 + i * 8) as usize);
    }
    offsets.push(data.len());
    let record0 = record(data, &offsets, 0)?;
    let compression = u16be(record0, 0);
    let text_length = u32be(record0, 4) as usize;
    let text_records = u16be(record0, 8) as usize;
    let encryption = u16be(record0, 12);
    if encryption != 0 {
        return Err(
            "This Kindle file is locked by DRM. Book Reader only opens DRM-free files.".into(),
        );
    }
    if record0.len() < 20 || &record0[16..20] != b"MOBI" {
        let text = decode_records(data, &offsets, 1, text_records, compression, text_length)?;
        return Ok(MobiText {
            title: pdb_name(data),
            author: String::new(),
            text,
        });
    }
    let header_len = u32be(record0, 20) as usize;
    let encoding = if record0.len() >= 32 {
        u32be(record0, 28)
    } else {
        1252
    };
    let (title_offset, title_len) = if record0.len() >= 16 + 0x58 + 4 {
        (
            u32be(record0, 16 + 0x54) as usize,
            u32be(record0, 16 + 0x58) as usize,
        )
    } else {
        (0, 0)
    };
    let title = if title_offset + title_len <= record0.len() && title_len > 0 {
        decode_bytes(&record0[title_offset..title_offset + title_len], encoding)
    } else {
        pdb_name(data)
    };
    let exth_start = 16 + header_len;
    let (author, kf8) = parse_exth(record0, exth_start);
    let mut text = decode_records(data, &offsets, 1, text_records, compression, text_length)?;
    if let Some(boundary) = kf8 {
        if let Some(kf8_text) = read_kf8(data, &offsets, boundary) {
            if looks_like_html(&kf8_text) || kf8_text.len() > text.len() {
                text = kf8_text;
            }
        }
    }
    if !looks_readable(&text) {
        return Err(
            "This Kindle file looks encrypted or is not a supported DRM-free book.".into(),
        );
    }
    Ok(MobiText { title, author, text })
}

fn read_kf8(data: &[u8], offsets: &[usize], boundary: usize) -> Option<String> {
    let record0 = record(data, offsets, boundary).ok()?;
    let start = record0.windows(4).position(|w| w == b"MOBI")?;
    if start < 16 {
        return None;
    }
    let base = start - 16;
    let header = &record0[base..];
    if header.len() < 16 {
        return None;
    }
    let compression = u16be(header, 0);
    let text_length = u32be(header, 4) as usize;
    let text_records = u16be(header, 8) as usize;
    let encryption = u16be(header, 12);
    if encryption != 0 {
        return None;
    }
    decode_records(
        data,
        offsets,
        boundary + 1,
        text_records,
        compression,
        text_length,
    )
    .ok()
}

fn parse_exth(record0: &[u8], start: usize) -> (String, Option<usize>) {
    if start + 12 > record0.len() || &record0[start..start + 4] != b"EXTH" {
        return (String::new(), None);
    }
    let count = u32be(record0, start + 8) as usize;
    let mut pos = start + 12;
    let mut author = String::new();
    let mut kf8 = None;
    for _ in 0..count {
        if pos + 8 > record0.len() {
            break;
        }
        let kind = u32be(record0, pos);
        let len = u32be(record0, pos + 4) as usize;
        if len < 8 || pos + len > record0.len() {
            break;
        }
        let payload = &record0[pos + 8..pos + len];
        if kind == 100 && author.is_empty() {
            author = String::from_utf8_lossy(payload).trim_matches('\0').trim().to_string();
        }
        if kind == 121 && payload.len() >= 4 {
            kf8 = Some(u32be(payload, 0) as usize);
        }
        pos += len;
    }
    (author, kf8)
}

fn decode_records(
    data: &[u8],
    offsets: &[usize],
    first: usize,
    count: usize,
    compression: u16,
    text_length: usize,
) -> Result<String, String> {
    let mut raw = Vec::new();
    for n in 0..count {
        let rec = record(data, offsets, first + n)?;
        match compression {
            1 => raw.extend_from_slice(rec),
            2 => raw.extend(palmdoc_decompress(rec)),
            17480 => {
                return Err(
                    "This Kindle file uses Huff/CDIC compression, which this reader does not open."
                        .into(),
                );
            }
            other => return Err(format!("Unsupported Kindle compression ({other}).")),
        }
    }
    if text_length > 0 && text_length < raw.len() {
        raw.truncate(text_length);
    }
    let (text, _, _) = encoding_rs::UTF_8.decode(&raw);
    if text.contains('\u{fffd}') {
        let (win, _, _) = encoding_rs::WINDOWS_1252.decode(&raw);
        Ok(win.to_string())
    } else {
        Ok(text.to_string())
    }
}

fn looks_readable(text: &str) -> bool {
    let sample: String = text.chars().take(400).collect();
    if sample.trim().is_empty() {
        return false;
    }
    let letters = sample.chars().filter(|c| c.is_alphabetic() || c.is_whitespace() || *c == '<' || *c == '>').count();
    letters * 2 > sample.chars().count()
}

pub fn palmdoc_decompress(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        i += 1;
        if b == 0 || (9..=0x7f).contains(&b) {
            out.push(b);
        } else if (1..=8).contains(&b) {
            let n = b as usize;
            let end = (i + n).min(data.len());
            out.extend_from_slice(&data[i..end]);
            i = end;
        } else if (0x80..=0xbf).contains(&b) {
            if i >= data.len() {
                break;
            }
            let next = data[i];
            i += 1;
            let distance = ((((b as usize) << 8) | next as usize) >> 3) & 0x7ff;
            let length = (next & 0x7) as usize + 3;
            if distance == 0 || distance > out.len() {
                continue;
            }
            for _ in 0..length {
                let ch = out[out.len() - distance];
                out.push(ch);
            }
        } else {
            out.push(b' ');
            out.push(b ^ 0x80);
        }
    }
    out
}

fn decode_bytes(bytes: &[u8], encoding: u32) -> String {
    let enc = if encoding == 65001 {
        encoding_rs::UTF_8
    } else {
        encoding_rs::WINDOWS_1252
    };
    let (text, _, _) = enc.decode(bytes);
    text.trim_matches('\0').trim().to_string()
}

fn pdb_name(data: &[u8]) -> String {
    let end = data.iter().take(32).position(|b| *b == 0).unwrap_or(32);
    String::from_utf8_lossy(&data[..end]).trim().to_string()
}

fn record<'a>(data: &'a [u8], offsets: &[usize], index: usize) -> Result<&'a [u8], String> {
    if index + 1 >= offsets.len() {
        return Err("Kindle record is missing.".into());
    }
    let start = offsets[index];
    let end = offsets[index + 1].min(data.len());
    if start >= end || end > data.len() {
        return Err("Kindle record is out of range.".into());
    }
    Ok(&data[start..end])
}

fn u16be(data: &[u8], offset: usize) -> u16 {
    data.get(offset..offset + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .unwrap_or(0)
}

fn u32be(data: &[u8], offset: usize) -> u32 {
    data.get(offset..offset + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::palmdoc_decompress;

    fn sample(encryption: u16, text: &[u8]) -> Vec<u8> {
        let record0_at = 78 + 16;
        let record1_at = record0_at + 32;
        let mut data = vec![0u8; record1_at + text.len()];
        data[60..68].copy_from_slice(b"BOOKMOBI");
        data[76..78].copy_from_slice(&2u16.to_be_bytes());
        data[78..82].copy_from_slice(&(record0_at as u32).to_be_bytes());
        data[86..90].copy_from_slice(&(record1_at as u32).to_be_bytes());
        data[record0_at..record0_at + 2].copy_from_slice(&1u16.to_be_bytes());
        data[record0_at + 4..record0_at + 8].copy_from_slice(&(text.len() as u32).to_be_bytes());
        data[record0_at + 8..record0_at + 10].copy_from_slice(&1u16.to_be_bytes());
        data[record0_at + 12..record0_at + 14].copy_from_slice(&encryption.to_be_bytes());
        data[record0_at + 16..record0_at + 20].copy_from_slice(b"MOBI");
        data[record1_at..].copy_from_slice(text);
        data
    }

    #[test]
    fn decompresses_literals_and_backrefs() {
        let data = [b'H', b'i', 0x80, 0x08];
        let out = palmdoc_decompress(&data);
        assert_eq!(&out[..2], b"Hi");
    }

    #[test]
    fn refuses_encrypted_header() {
        let err = super::parse_mobi(&sample(1, b"Hello")).unwrap_err();
        assert!(err.contains("DRM"), "{err}");
    }

    #[test]
    fn reads_plain_mobi_text() {
        let book = super::parse_mobi(&sample(0, b"Hello from a book.")).unwrap();
        assert!(book.text.contains("Hello from a book."));
    }
}

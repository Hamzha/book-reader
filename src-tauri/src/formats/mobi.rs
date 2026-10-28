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
        let text = decode_records(data, &offsets, record0, 1, text_records, compression, text_length)?;
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
    let (title_offset, title_len) = if record0.len() >= 0x5c {
        (u32be(record0, 0x54) as usize, u32be(record0, 0x58) as usize)
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
    let mut text = decode_records(data, &offsets, record0, 1, text_records, compression, text_length)?;
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
        header,
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
    header: &[u8],
    first: usize,
    count: usize,
    compression: u16,
    text_length: usize,
) -> Result<String, String> {
    let trail = trail_info(header);
    let mut huff = if compression == 17480 {
        Some(load_huff(data, offsets, header)?)
    } else {
        None
    };
    let mut raw = Vec::new();
    for n in 0..count {
        let rec = trim_trailing(record(data, offsets, first + n)?, trail);
        match compression {
            1 => raw.extend_from_slice(rec),
            2 => raw.extend(palmdoc_decompress(rec)),
            17480 => {
                let table = huff.as_mut().ok_or("This Kindle file is missing its compression table.")?;
                raw.extend(table.unpack(rec));
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

#[derive(Clone, Copy)]
struct Trail {
    multibyte: bool,
    trailers: u32,
}

fn trail_info(header: &[u8]) -> Trail {
    if header.len() < 0xf4 {
        return Trail { multibyte: false, trailers: 0 };
    }
    let header_len = u32be(header, 0x14);
    let version = u32be(header, 0x68);
    if header_len < 0xe4 || version < 5 {
        return Trail { multibyte: false, trailers: 0 };
    }
    let mut flags = u16be(header, 0xf2);
    let multibyte = flags & 1 != 0;
    let mut trailers = 0u32;
    while flags > 1 {
        if flags & 2 != 0 {
            trailers += 1;
        }
        flags >>= 1;
    }
    Trail { multibyte, trailers }
}

fn trim_trailing<'a>(data: &'a [u8], trail: Trail) -> &'a [u8] {
    let mut end = data.len();
    for _ in 0..trail.trailers {
        if end == 0 {
            break;
        }
        let num = trailing_entry_size(&data[..end]);
        if num == 0 || num > end {
            break;
        }
        end -= num;
    }
    if trail.multibyte && end > 0 {
        let num = ((data[end - 1] & 3) as usize) + 1;
        if num <= end {
            end -= num;
        }
    }
    &data[..end]
}

fn trailing_entry_size(data: &[u8]) -> usize {
    let start = data.len().saturating_sub(4);
    let mut num = 0usize;
    for &byte in &data[start..] {
        if byte & 0x80 != 0 {
            num = 0;
        }
        num = (num << 7) | (byte & 0x7f) as usize;
    }
    num
}

struct Huff {
    dict1: Vec<(u8, bool, u32)>,
    mincode: Vec<u32>,
    maxcode: Vec<u32>,
    dictionary: Vec<Option<(Vec<u8>, bool)>>,
}

fn load_huff(data: &[u8], offsets: &[usize], header: &[u8]) -> Result<Huff, String> {
    if header.len() < 0x78 {
        return Err("This Kindle file is missing its compression table.".into());
    }
    let start = u32be(header, 0x70) as usize;
    let count = u32be(header, 0x74) as usize;
    if count < 2 {
        return Err("This Kindle file is missing its compression table.".into());
    }
    let mut records = Vec::with_capacity(count);
    for index in 0..count {
        records.push(record(data, offsets, start + index)?);
    }
    let mut huff = parse_huff(records[0])?;
    for cdic in &records[1..] {
        append_cdic(&mut huff, cdic)?;
    }
    Ok(huff)
}

fn parse_huff(huff: &[u8]) -> Result<Huff, String> {
    if huff.len() < 24 || &huff[..8] != b"HUFF\x00\x00\x00\x18" {
        return Err("This Kindle file has a broken compression table.".into());
    }
    let off1 = u32be(huff, 8) as usize;
    let off2 = u32be(huff, 12) as usize;
    if off1 + 256 * 4 > huff.len() || off2 + 64 * 4 > huff.len() {
        return Err("This Kindle file has a broken compression table.".into());
    }
    let mut dict1 = Vec::with_capacity(256);
    for index in 0..256 {
        let value = u32be(huff, off1 + index * 4);
        let codelen = (value & 0x1f) as u8;
        if codelen == 0 {
            return Err("This Kindle file has a broken compression table.".into());
        }
        let term = value & 0x80 != 0;
        let mut maxcode = value >> 8;
        if codelen <= 8 {
            maxcode = shift_max(maxcode, codelen as usize);
        }
        dict1.push((codelen, term, maxcode));
    }
    let mut evens = vec![0u32];
    let mut odds = vec![0u32];
    for index in 0..32 {
        evens.push(u32be(huff, off2 + index * 8));
        odds.push(u32be(huff, off2 + index * 8 + 4));
    }
    let mut mincode = Vec::with_capacity(33);
    let mut maxcode = Vec::with_capacity(33);
    for codelen in 0..33 {
        mincode.push(shift_min(evens[codelen], codelen));
        maxcode.push(shift_max(odds[codelen], codelen));
    }
    Ok(Huff { dict1, mincode, maxcode, dictionary: Vec::new() })
}

fn shift_min(value: u32, codelen: usize) -> u32 {
    let shift = 32usize.saturating_sub(codelen);
    if shift >= 32 { 0 } else { ((value as u64) << shift) as u32 }
}

fn shift_max(value: u32, codelen: usize) -> u32 {
    let shift = 32usize.saturating_sub(codelen);
    if shift >= 32 {
        u32::MAX
    } else {
        ((((value as u64) + 1) << shift) - 1) as u32
    }
}

fn append_cdic(huff: &mut Huff, cdic: &[u8]) -> Result<(), String> {
    if cdic.len() < 16 || &cdic[..8] != b"CDIC\x00\x00\x00\x10" {
        return Err("This Kindle file has a broken compression table.".into());
    }
    let phrases = u32be(cdic, 8) as usize;
    let bits = u32be(cdic, 12) as usize;
    if bits > 16 {
        return Err("This Kindle file has a broken compression table.".into());
    }
    let n = (1usize << bits).min(phrases.saturating_sub(huff.dictionary.len()));
    if 16 + n * 2 > cdic.len() {
        return Err("This Kindle file has a broken compression table.".into());
    }
    for index in 0..n {
        let off = u16be(cdic, 16 + index * 2) as usize;
        if 16 + off + 2 > cdic.len() {
            return Err("This Kindle file has a broken compression table.".into());
        }
        let blen = u16be(cdic, 16 + off) as usize;
        let len = blen & 0x7fff;
        let flag = blen & 0x8000 != 0;
        let start = 18 + off;
        if start + len > cdic.len() {
            return Err("This Kindle file has a broken compression table.".into());
        }
        huff.dictionary.push(Some((cdic[start..start + len].to_vec(), flag)));
    }
    Ok(())
}

impl Huff {
    fn unpack(&mut self, data: &[u8]) -> Vec<u8> {
        let mut padded = data.to_vec();
        padded.extend_from_slice(&[0; 8]);
        let mut out = Vec::new();
        let mut bits_left = data.len() as i64 * 8;
        let mut pos = 0usize;
        let mut n = 32i64;
        let mut word = read_u64(&padded, pos);
        while bits_left > 0 {
            if n <= 0 {
                pos += 4;
                if pos + 8 > padded.len() {
                    break;
                }
                word = read_u64(&padded, pos);
                n += 32;
            }
            let code = ((word >> n) & 0xffff_ffff) as u32;
            let index = (code >> 24) as usize;
            let Some(&(mut codelen, term, mut maxcode)) = self.dict1.get(index) else { break };
            if !term {
                while (codelen as usize) < self.mincode.len() && code < self.mincode[codelen as usize] {
                    codelen += 1;
                }
                if (codelen as usize) >= self.maxcode.len() {
                    break;
                }
                maxcode = self.maxcode[codelen as usize];
            }
            if codelen == 0 {
                break;
            }
            n -= codelen as i64;
            bits_left -= codelen as i64;
            if bits_left < 0 {
                break;
            }
            let shift = 32usize.saturating_sub(codelen as usize);
            let slot = if shift >= 32 { 0 } else { ((maxcode.wrapping_sub(code)) >> shift) as usize };
            let entry = self.dictionary.get(slot).cloned().flatten();
            match entry {
                Some((slice, true)) => out.extend_from_slice(&slice),
                Some((slice, false)) => {
                    if let Some(item) = self.dictionary.get_mut(slot) {
                        *item = None;
                    }
                    let decoded = self.unpack(&slice);
                    if let Some(item) = self.dictionary.get_mut(slot) {
                        *item = Some((decoded.clone(), true));
                    }
                    out.extend_from_slice(&decoded);
                }
                None => break,
            }
        }
        out
    }
}

fn read_u64(data: &[u8], offset: usize) -> u64 {
    data.get(offset..offset + 8)
        .map(|b| u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
        .unwrap_or(0)
}

pub fn write_book(path: &Path, title: &str, html: &str) -> Result<(), String> {
    if html.trim().is_empty() {
        return Err("There is no text to write.".into());
    }
    let pieces = split_text(html, 4096);
    let record0 = header_record(title, html.len(), pieces.len());
    let mut records: Vec<Vec<u8>> = Vec::with_capacity(pieces.len() + 1);
    records.push(record0);
    for piece in pieces {
        records.push(piece.into_bytes());
    }
    let mut file = vec![0u8; 78 + records.len() * 8];
    let name = pdb_title(title);
    file[..name.len()].copy_from_slice(&name);
    file[60..68].copy_from_slice(b"BOOKMOBI");
    file[76..78].copy_from_slice(&(records.len() as u16).to_be_bytes());
    let mut cursor = file.len() as u32;
    for (index, record) in records.iter().enumerate() {
        let at = 78 + index * 8;
        file[at..at + 4].copy_from_slice(&cursor.to_be_bytes());
        cursor += record.len() as u32;
    }
    for record in records {
        file.extend(record);
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(path, file).map_err(|e| e.to_string())
}

fn header_record(title: &str, text_length: usize, text_records: usize) -> Vec<u8> {
    let mut record = vec![0u8; 0x100];
    record[0..2].copy_from_slice(&1u16.to_be_bytes());
    record[4..8].copy_from_slice(&(text_length as u32).to_be_bytes());
    record[8..10].copy_from_slice(&(text_records as u16).to_be_bytes());
    record[10..12].copy_from_slice(&4096u16.to_be_bytes());
    record[16..20].copy_from_slice(b"MOBI");
    record[0x14..0x18].copy_from_slice(&0x68u32.to_be_bytes());
    record[0x1c..0x20].copy_from_slice(&65001u32.to_be_bytes());
    let title_bytes = truncate_bytes(title, 0x70);
    record[0x54..0x58].copy_from_slice(&0x80u32.to_be_bytes());
    record[0x58..0x5c].copy_from_slice(&(title_bytes.len() as u32).to_be_bytes());
    record[0x80..0x80 + title_bytes.len()].copy_from_slice(&title_bytes);
    record
}

fn pdb_title(title: &str) -> Vec<u8> {
    let mut name = truncate_bytes(title, 31);
    if name.is_empty() {
        name.extend_from_slice(b"Book");
    }
    name.resize(32, 0);
    name
}

fn truncate_bytes(text: &str, max: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for ch in text.chars() {
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        if out.len() + encoded.len() > max {
            break;
        }
        out.extend_from_slice(encoded.as_bytes());
    }
    out
}

fn split_text(text: &str, max: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if rest.len() <= max {
            pieces.push(rest.to_string());
            break;
        }
        let mut end = max;
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        if end == 0 {
            end = rest.chars().next().map(|ch| ch.len_utf8()).unwrap_or(rest.len());
        }
        pieces.push(rest[..end].to_string());
        rest = &rest[end..];
    }
    pieces
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

    #[test]
    fn drops_kindle_trailing_bytes() {
        let text = b"Hello\x80\x02";
        let record0_len = 0x100;
        let record0_at = 78 + 16;
        let record1_at = record0_at + record0_len;
        let mut data = vec![0u8; record1_at + text.len()];
        data[60..68].copy_from_slice(b"BOOKMOBI");
        data[76..78].copy_from_slice(&2u16.to_be_bytes());
        data[78..82].copy_from_slice(&(record0_at as u32).to_be_bytes());
        data[86..90].copy_from_slice(&(record1_at as u32).to_be_bytes());
        data[record0_at..record0_at + 2].copy_from_slice(&1u16.to_be_bytes());
        data[record0_at + 4..record0_at + 8].copy_from_slice(&(text.len() as u32).to_be_bytes());
        data[record0_at + 8..record0_at + 10].copy_from_slice(&1u16.to_be_bytes());
        data[record0_at + 16..record0_at + 20].copy_from_slice(b"MOBI");
        data[record0_at + 0x14..record0_at + 0x18].copy_from_slice(&0xe8u32.to_be_bytes());
        data[record0_at + 0x68..record0_at + 0x6c].copy_from_slice(&5u32.to_be_bytes());
        data[record0_at + 0xf2..record0_at + 0xf4].copy_from_slice(&2u16.to_be_bytes());
        data[record1_at..].copy_from_slice(text);
        let book = super::parse_mobi(&data).unwrap();
        assert_eq!(book.text, "Hello");
    }

    #[test]
    fn writes_a_mobi_our_reader_can_open() {
        let dir = std::env::temp_dir().join(format!("book-reader-mobi-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.mobi");
        let html = "<html><body><h1>Sample</h1><p>Hello from a page.</p></body></html>";
        super::write_book(&path, "Sample", html).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let book = super::parse_mobi(&bytes).unwrap();
        assert_eq!(book.title, "Sample");
        assert!(book.text.contains("Hello from a page."));
        let _ = std::fs::remove_dir_all(dir);
    }
}

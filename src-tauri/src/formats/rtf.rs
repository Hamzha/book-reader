use std::path::Path;

use super::Extracted;
use crate::htmlutil::plain_to_html;

pub fn extract(path: &Path) -> Result<Extracted, String> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let (text, _, _) = encoding_rs::WINDOWS_1252.decode(&raw);
    let plain = rtf_to_text(&text);
    let title = super::title_from_path(path);
    Ok(Extracted::reflow(title, "Unknown".into(), plain_to_html(&plain)))
}

pub fn rtf_to_text(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    let mut depth = 0i32;
    let mut skip_until = -1i32;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if skip_until >= 0 && depth < skip_until {
                    skip_until = -1;
                }
                i += 1;
            }
            b'\\' => {
                i += 1;
                if i >= bytes.len() {
                    break;
                }
                match bytes[i] {
                    b'\\' | b'{' | b'}' => {
                        if skip_until < 0 {
                            out.push(bytes[i] as char);
                        }
                        i += 1;
                    }
                    b'\'' => {
                        if i + 2 < bytes.len() {
                            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00");
                            let code = u8::from_str_radix(hex, 16).unwrap_or(0);
                            if skip_until < 0 {
                                let bytes = [code];
                                let (s, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
                                out.push_str(&s);
                            }
                            i += 3;
                        } else {
                            break;
                        }
                    }
                    b'\n' | b'\r' => i += 1,
                    c if c.is_ascii_alphabetic() => {
                        let start = i;
                        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                            i += 1;
                        }
                        let word = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                        let mut negative = false;
                        if i < bytes.len() && bytes[i] == b'-' {
                            negative = true;
                            i += 1;
                        }
                        let num_start = i;
                        while i < bytes.len() && bytes[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num = std::str::from_utf8(&bytes[num_start..i])
                            .ok()
                            .and_then(|s| s.parse::<i32>().ok())
                            .map(|n| if negative { -n } else { n });
                        if i < bytes.len() && bytes[i] == b' ' {
                            i += 1;
                        }
                        if word == "u" {
                            if let Some(cp) = num {
                                if skip_until < 0 {
                                    let u = cp.rem_euclid(65536) as u32;
                                    if let Some(ch) = char::from_u32(u) {
                                        out.push(ch);
                                    }
                                }
                                if i < bytes.len() && bytes[i] != b'\\' && bytes[i] != b'{' && bytes[i] != b'}' {
                                    i += 1;
                                }
                            }
                            continue;
                        }
                        if skip_until >= 0 {
                            continue;
                        }
                        match word {
                            "par" | "line" => out.push('\n'),
                            "tab" => out.push('\t'),
                            "emdash" => out.push('—'),
                            "endash" => out.push('–'),
                            "lquote" => out.push('‘'),
                            "rquote" => out.push('’'),
                            "ldblquote" => out.push('“'),
                            "rdblquote" => out.push('”'),
                            "bullet" => out.push('•'),
                            "fonttbl" | "colortbl" | "stylesheet" | "info" | "pict" | "object"
                            | "header" | "footer" | "footnote" | "xe" | "tc" | "field" | "datafield" => {
                                skip_until = depth;
                            }
                            _ => {}
                        }
                    }
                    _ => i += 1,
                }
            }
            b'\r' | b'\n' => i += 1,
            c => {
                if skip_until < 0 {
                    out.push(c as char);
                }
                i += 1;
            }
        }
    }
    out.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::rtf_to_text;

    #[test]
    fn reads_plain_and_hex() {
        let text = rtf_to_text(r"{\rtf1\ansi Hello\~world\par Second \'41.}");
        assert!(text.contains("Hello"));
        assert!(text.contains("Second"));
        assert!(text.contains('A'));
    }
}

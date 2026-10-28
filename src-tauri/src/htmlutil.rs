pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn plain_to_html(text: &str) -> String {
    let trimmed = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut html = String::new();
    for para in trimmed.split("\n\n") {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }
        let body = escape(para).replace('\n', "<br/>");
        html.push_str("<p>");
        html.push_str(&body);
        html.push_str("</p>\n");
    }
    if html.is_empty() {
        html.push_str("<p></p>");
    }
    html
}

pub fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut last_space = false;
    for c in html.chars() {
        if c == '<' {
            in_tag = true;
            continue;
        }
        if c == '>' && in_tag {
            in_tag = false;
            if !last_space {
                out.push(' ');
                last_space = true;
            }
            continue;
        }
        if in_tag {
            continue;
        }
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Pull the inner HTML of the body, dropping scripts.
pub fn extract_body(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<body").and_then(|i| html[i..].find('>').map(|r| i + r + 1));
    let end = lower.rfind("</body>");
    let slice = match (start, end) {
        (Some(s), Some(e)) if e > s => &html[s..e],
        _ => html,
    };
    strip_blocks(slice, "script")
}

fn strip_blocks(input: &str, tag: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::new();
    let mut rest = 0;
    while let Some(rel) = lower[rest..].find(&open) {
        let start = rest + rel;
        out.push_str(&input[rest..start]);
        if let Some(end_rel) = lower[start..].find(&close) {
            rest = start + end_rel + close.len();
        } else {
            rest = input.len();
            break;
        }
    }
    out.push_str(&input[rest..]);
    out
}

pub fn snippet_around(text: &str, query: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let q = query.to_lowercase();
    let idx = lower.find(&q)?;
    let start = idx.saturating_sub(40);
    let end = (idx + q.len() + 60).min(text.len());
    let mut snip = String::new();
    if start > 0 {
        snip.push('…');
    }
    snip.push_str(text[start..end].trim());
    if end < text.len() {
        snip.push('…');
    }
    Some(snip)
}

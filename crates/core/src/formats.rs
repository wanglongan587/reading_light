use crate::library::{NormalizedEpub, pack, unpack};
use anyhow::{Context, Result, ensure};
use flate2::read::ZlibDecoder;
use roxmltree::Document;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    io::Read,
};

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn build_chapters_epub(
    title: &str,
    author: &str,
    chapters: &[(String, String)],
    digest_seed: &[u8],
) -> Result<NormalizedEpub> {
    ensure!(!chapters.is_empty(), "文档中未找到可阅读的正文内容");
    let mut files = BTreeMap::new();
    files.insert("mimetype".into(), b"application/epub+zip".to_vec());
    files.insert(
        "META-INF/container.xml".into(),
        br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec(),
    );

    let mut manifest = String::new();
    let mut spine = String::new();
    let mut nav = String::new();
    for (i, (heading, body_xhtml)) in chapters.iter().enumerate() {
        manifest.push_str(&format!(
            "<item id=\"c{i}\" href=\"c{i}.xhtml\" media-type=\"application/xhtml+xml\"/>"
        ));
        spine.push_str(&format!("<itemref idref=\"c{i}\"/>"));
        nav.push_str(&format!(
            "<li><a href=\"c{i}.xhtml\">{}</a></li>",
            xml_escape(heading)
        ));
        let doc = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>{h}</title></head><body><h1>{h}</h1>{body_xhtml}</body></html>",
            h = xml_escape(heading),
        );
        files.insert(format!("c{i}.xhtml"), doc.into_bytes());
    }

    let id = format!("{:x}", Sha256::digest(digest_seed));
    files.insert(
        "book.opf".into(),
        format!(
            "<?xml version=\"1.0\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"id\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"id\">{id}</dc:identifier><dc:title>{title}</dc:title><dc:creator>{author}</dc:creator><dc:language>zh-CN</dc:language><meta property=\"dcterms:modified\">2026-01-01T00:00:00Z</meta></metadata><manifest><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>{manifest}</manifest><spine>{spine}</spine></package>",
            title = xml_escape(title),
            author = xml_escape(author),
        )
        .into_bytes(),
    );
    files.insert(
        "nav.xhtml".into(),
        format!(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><title>目录</title></head><body><nav epub:type=\"toc\"><ol>{nav}</ol></nav></body></html>"
        )
        .into_bytes(),
    );

    Ok(NormalizedEpub {
        bytes: pack(&files)?,
        title: title.to_owned(),
        author: author.to_owned(),
        cover: None,
    })
}

// =========================================================================
// 1. Markdown (.md / .markdown) -> Structured Multi-Chapter Reflowable EPUB
// =========================================================================

fn render_inline_md(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(input.len() + 16);
    let mut i = 0;

    while i < n {
        // Inline code `...`
        if chars[i] == '`'
            && let Some(end) = (i + 1..n).find(|&j| chars[j] == '`')
        {
            let inner: String = chars[i + 1..end].iter().collect();
            out.push_str(&format!("<code>{}</code>", xml_escape(&inner)));
            i = end + 1;
            continue;
        }
        // Bold **...**
        if i + 1 < n
            && chars[i] == '*'
            && chars[i + 1] == '*'
            && let Some(end) =
                (i + 2..n.saturating_sub(1)).find(|&j| chars[j] == '*' && chars[j + 1] == '*')
        {
            let inner: String = chars[i + 2..end].iter().collect();
            out.push_str(&format!("<strong>{}</strong>", render_inline_md(&inner)));
            i = end + 2;
            continue;
        }
        // Italic *...*
        if chars[i] == '*'
            && i + 1 < n
            && !chars[i + 1].is_whitespace()
            && let Some(end) =
                (i + 1..n).find(|&j| chars[j] == '*' && !chars[j - 1].is_whitespace())
        {
            let inner: String = chars[i + 1..end].iter().collect();
            out.push_str(&format!("<em>{}</em>", xml_escape(&inner)));
            i = end + 1;
            continue;
        }
        // Markdown link [label](url) -> safe plain text label (no external href in offline EPUB)
        if chars[i] == '['
            && let Some(mid) = (i + 1..n).find(|&j| chars[j] == ']')
            && mid + 1 < n
            && chars[mid + 1] == '('
            && let Some(close) = (mid + 2..n).find(|&j| chars[j] == ')')
        {
            let label: String = chars[i + 1..mid].iter().collect();
            out.push_str(&format!("<em>{}</em>", xml_escape(&label)));
            i = close + 1;
            continue;
        }
        match chars[i] {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            ch => out.push(ch),
        }
        i += 1;
    }
    out
}

pub fn markdown_to_epub(raw: &str, fallback_title: &str) -> Result<NormalizedEpub> {
    let normalized = raw
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut lines: Vec<&str> = normalized.lines().collect();
    let mut title = fallback_title.trim().to_owned();
    let mut author = "Markdown 笔记".to_owned();

    // Parse optional YAML frontmatter
    if lines.first().map(|l| l.trim()) == Some("---")
        && let Some(end_idx) = lines.iter().skip(1).position(|l| l.trim() == "---")
    {
        for fm_line in &lines[1..=end_idx] {
            if let Some((k, v)) = fm_line.split_once(':') {
                let key = k.trim().to_ascii_lowercase();
                let val = v.trim().trim_matches(['"', '\'']).trim();
                if !val.is_empty() {
                    if key == "title" {
                        title = val.to_owned();
                    } else if key == "author" {
                        author = val.to_owned();
                    }
                }
            }
        }
        lines = lines[end_idx + 2..].to_vec();
    }

    let has_h1 = lines
        .iter()
        .any(|l| l.trim_start().starts_with("# ") && !l.trim_start().starts_with("## "));
    let mut chapters: Vec<(String, String)> = Vec::new();
    let mut cur_heading = if title.is_empty() {
        "未命名笔记".into()
    } else {
        title.clone()
    };
    let mut cur_html = String::new();
    let mut in_code = false;
    let mut code_buf = String::new();
    let mut list_tag: Option<&'static str> = None;

    let close_list = |tag: &mut Option<&'static str>, html: &mut String| {
        if let Some(t) = tag.take() {
            html.push_str(&format!("</{t}>"));
        }
    };

    for raw_line in lines {
        let trimmed = raw_line.trim();
        if trimmed.starts_with("```") {
            close_list(&mut list_tag, &mut cur_html);
            if in_code {
                cur_html.push_str(&format!(
                    "<pre><code>{}</code></pre>",
                    xml_escape(&code_buf)
                ));
                code_buf.clear();
                in_code = false;
            } else {
                in_code = true;
            }
            continue;
        }
        if in_code {
            if !code_buf.is_empty() {
                code_buf.push('\n');
            }
            code_buf.push_str(raw_line);
            continue;
        }
        if trimmed.is_empty() {
            close_list(&mut list_tag, &mut cur_html);
            continue;
        }
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            close_list(&mut list_tag, &mut cur_html);
            cur_html.push_str("<hr/>");
            continue;
        }

        // Chapter split headings
        let is_chapter_split = if has_h1 {
            trimmed.starts_with("# ")
        } else {
            trimmed.starts_with("## ")
        };
        if is_chapter_split {
            close_list(&mut list_tag, &mut cur_html);
            let h_text = trimmed.trim_start_matches('#').trim();
            if !cur_html.trim().is_empty() {
                chapters.push((cur_heading.clone(), std::mem::take(&mut cur_html)));
            } else if chapters.is_empty() && title == fallback_title && !h_text.is_empty() {
                title = h_text.to_owned();
            }
            cur_heading = h_text.to_owned();
            continue;
        }

        // Subheadings
        if let Some(rest) = trimmed.strip_prefix("#### ") {
            close_list(&mut list_tag, &mut cur_html);
            cur_html.push_str(&format!("<h4>{}</h4>", render_inline_md(rest.trim())));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            close_list(&mut list_tag, &mut cur_html);
            cur_html.push_str(&format!("<h3>{}</h3>", render_inline_md(rest.trim())));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            close_list(&mut list_tag, &mut cur_html);
            cur_html.push_str(&format!("<h2>{}</h2>", render_inline_md(rest.trim())));
            continue;
        }

        // Blockquote
        if let Some(rest) = trimmed.strip_prefix("> ") {
            close_list(&mut list_tag, &mut cur_html);
            cur_html.push_str(&format!(
                "<blockquote><p>{}</p></blockquote>",
                render_inline_md(rest.trim())
            ));
            continue;
        }

        // Unordered list
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            if list_tag != Some("ul") {
                close_list(&mut list_tag, &mut cur_html);
                cur_html.push_str("<ul>");
                list_tag = Some("ul");
            }
            cur_html.push_str(&format!("<li>{}</li>", render_inline_md(rest.trim())));
            continue;
        }

        // Ordered list (e.g. "1. item")
        let ol_rest = trimmed.split_once(". ").and_then(|(num, rest)| {
            (!num.is_empty() && num.chars().all(|c| c.is_ascii_digit())).then_some(rest)
        });
        if let Some(rest) = ol_rest {
            if list_tag != Some("ol") {
                close_list(&mut list_tag, &mut cur_html);
                cur_html.push_str("<ol>");
                list_tag = Some("ol");
            }
            cur_html.push_str(&format!("<li>{}</li>", render_inline_md(rest.trim())));
            continue;
        }

        close_list(&mut list_tag, &mut cur_html);
        cur_html.push_str(&format!("<p>{}</p>", render_inline_md(trimmed)));
    }

    close_list(&mut list_tag, &mut cur_html);
    if in_code && !code_buf.is_empty() {
        cur_html.push_str(&format!(
            "<pre><code>{}</code></pre>",
            xml_escape(&code_buf)
        ));
    }
    if !cur_html.trim().is_empty() {
        chapters.push((cur_heading, cur_html));
    }
    ensure!(!chapters.is_empty(), "Markdown 文件没有可阅读内容");
    if title.is_empty() {
        title = "Markdown 笔记".into();
    }
    build_chapters_epub(&title, &author, &chapters, raw.as_bytes())
}

// =========================================================================
// 2. Word Document (.docx) -> Native OOXML Parser -> Reflowable EPUB
// =========================================================================

pub fn docx_to_epub(bytes: &[u8], fallback_title: &str) -> Result<NormalizedEpub> {
    let files = unpack(bytes).context("无法解压 DOCX 文档")?;
    let doc_bytes = files
        .get("word/document.xml")
        .context("DOCX 缺少 word/document.xml 正文")?;
    let doc_xml = std::str::from_utf8(doc_bytes).context("DOCX 正文 XML 编码无效")?;
    let doc = Document::parse(doc_xml).context("DOCX 正文 XML 解析失败")?;

    let mut title = fallback_title.trim().to_owned();
    let mut author = "Word 文档".to_owned();

    if let Some(core_bytes) = files.get("docProps/core.xml")
        && let Ok(core_xml) = std::str::from_utf8(core_bytes)
        && let Ok(core_doc) = Document::parse(core_xml)
    {
        if let Some(t) = core_doc
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "title")
            .and_then(|n| n.text())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            title = t.to_owned();
        }
        if let Some(a) = core_doc
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "creator")
            .and_then(|n| n.text())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            author = a.to_owned();
        }
    }

    if title.is_empty() {
        title = "未命名文档".into();
    }

    let mut chapters: Vec<(String, String)> = Vec::new();
    let mut cur_heading = title.clone();
    let mut cur_html = String::new();
    let mut para_in_chapter = 0usize;

    for p in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "p")
    {
        let style = p
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "pPr")
            .and_then(|ppr| {
                ppr.children()
                    .find(|n| n.is_element() && n.tag_name().name() == "pStyle")
            })
            .and_then(|st| st.attributes().find(|a| a.name() == "val"))
            .map(|a| a.value().to_ascii_lowercase())
            .unwrap_or_default();

        let mut run_html = String::new();
        let mut plain = String::new();
        for run in p
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "r")
        {
            let rpr = run
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == "rPr");
            let is_bold = rpr
                .and_then(|r| {
                    r.children()
                        .find(|n| n.is_element() && n.tag_name().name() == "b")
                })
                .is_some();
            let is_italic = rpr
                .and_then(|r| {
                    r.children()
                        .find(|n| n.is_element() && n.tag_name().name() == "i")
                })
                .is_some();

            let mut seg = String::new();
            for child in run.children().filter(|n| n.is_element()) {
                match child.tag_name().name() {
                    "t" => {
                        if let Some(t) = child.text() {
                            seg.push_str(t);
                        }
                    }
                    "tab" => seg.push_str("  "),
                    "br" => seg.push('\n'),
                    _ => {}
                }
            }
            if seg.is_empty() {
                continue;
            }
            plain.push_str(&seg);
            let escaped = xml_escape(&seg).replace('\n', "<br/>");
            if is_bold && is_italic {
                run_html.push_str(&format!("<strong><em>{escaped}</em></strong>"));
            } else if is_bold {
                run_html.push_str(&format!("<strong>{escaped}</strong>"));
            } else if is_italic {
                run_html.push_str(&format!("<em>{escaped}</em>"));
            } else {
                run_html.push_str(&escaped);
            }
        }

        let trimmed = plain.trim();
        if trimmed.is_empty() {
            continue;
        }
        let is_h1 = style.contains("heading1")
            || style == "1"
            || style == "title"
            || style.contains("标题 1")
            || style.contains("heading 1");
        let is_h2 = style.contains("heading2")
            || style == "2"
            || style.contains("subtitle")
            || style.contains("标题 2")
            || style.contains("heading 2");

        if is_h1 && trimmed.chars().count() <= 120 {
            if !cur_html.trim().is_empty() {
                chapters.push((cur_heading.clone(), std::mem::take(&mut cur_html)));
                para_in_chapter = 0;
            }
            cur_heading = trimmed.to_owned();
            continue;
        }
        if is_h2 && trimmed.chars().count() <= 120 {
            cur_html.push_str(&format!("<h2>{}</h2>", xml_escape(trimmed)));
            continue;
        }
        if para_in_chapter >= 180 && !cur_html.trim().is_empty() {
            chapters.push((cur_heading.clone(), std::mem::take(&mut cur_html)));
            cur_heading = format!("{title} · {}", chapters.len() + 1);
            para_in_chapter = 0;
        }
        cur_html.push_str(&format!("<p>{run_html}</p>"));
        para_in_chapter += 1;
    }

    if !cur_html.trim().is_empty() {
        chapters.push((cur_heading, cur_html));
    }
    ensure!(!chapters.is_empty(), "DOCX 文档中未检测到可阅读段落");
    build_chapters_epub(&title, &author, &chapters, bytes)
}

// =========================================================================
// 3. Pure-Rust PDF Reflow Engine (.pdf) -> Warm Reflowable EPUB
// =========================================================================

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn decode_pdf_string_bytes(raw: &[u8]) -> String {
    if raw.len() >= 2 && raw[0] == 0xFE && raw[1] == 0xFF {
        let units: Vec<u16> = raw[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Ok(s) = std::str::from_utf8(raw) {
        return s.to_owned();
    }
    raw.iter().map(|&b| b as char).collect()
}

fn parse_pdf_literal(data: &[u8], start: usize) -> Option<(Vec<u8>, usize)> {
    if data.get(start) != Some(&b'(') {
        return None;
    }
    let mut out = Vec::new();
    let mut i = start + 1;
    let mut depth = 1usize;
    while i < data.len() {
        let b = data[i];
        if b == b'\\' && i + 1 < data.len() {
            i += 1;
            match data[i] {
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'b' => out.push(0x08),
                b'f' => out.push(0x0C),
                b'(' => out.push(b'('),
                b')' => out.push(b')'),
                b'\\' => out.push(b'\\'),
                b'\r' => {
                    if data.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                }
                b'\n' => {}
                c @ b'0'..=b'7' => {
                    let mut val = (c - b'0') as u32;
                    for _ in 0..2 {
                        if let Some(&next @ b'0'..=b'7') = data.get(i + 1) {
                            i += 1;
                            val = val * 8 + (next - b'0') as u32;
                        } else {
                            break;
                        }
                    }
                    out.push((val & 0xFF) as u8);
                }
                other => out.push(other),
            }
            i += 1;
            continue;
        }
        if b == b'(' {
            depth += 1;
            out.push(b);
        } else if b == b')' {
            depth -= 1;
            if depth == 0 {
                return Some((out, i + 1));
            }
            out.push(b);
        } else {
            out.push(b);
        }
        i += 1;
    }
    None
}

fn parse_pdf_hex(data: &[u8], start: usize) -> Option<(Vec<u8>, usize)> {
    if data.get(start) != Some(&b'<') || data.get(start + 1) == Some(&b'<') {
        return None;
    }
    let mut hex_digits = Vec::new();
    let mut i = start + 1;
    while i < data.len() {
        let b = data[i];
        if b == b'>' {
            if hex_digits.len() % 2 == 1 {
                hex_digits.push(0);
            }
            let out: Vec<u8> = hex_digits
                .chunks_exact(2)
                .map(|pair| (pair[0] << 4) | pair[1])
                .collect();
            return Some((out, i + 1));
        }
        if let Some(d) = match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        } {
            hex_digits.push(d);
        }
        i += 1;
    }
    None
}

fn extract_pdf_info_field(bytes: &[u8], key: &[u8]) -> Option<String> {
    let mut cursor = 0usize;
    while let Some(pos) = find_bytes(&bytes[cursor..], key) {
        let mut idx = cursor + pos + key.len();
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        let parsed = match bytes.get(idx) {
            Some(b'(') => parse_pdf_literal(bytes, idx).map(|(v, _)| v),
            Some(b'<') => parse_pdf_hex(bytes, idx).map(|(v, _)| v),
            _ => None,
        };
        if let Some(raw) = parsed {
            let text = decode_pdf_string_bytes(&raw).trim().to_owned();
            if !text.is_empty() && text.chars().count() <= 160 {
                return Some(text);
            }
        }
        cursor += pos + key.len();
    }
    None
}

fn parse_cmap_unicode(stream: &[u8], map: &mut HashMap<Vec<u8>, String>) {
    let Ok(text) = std::str::from_utf8(stream) else {
        return;
    };
    if !text.contains("beginbfchar") && !text.contains("beginbfrange") {
        return;
    }
    let bytes = text.as_bytes();
    // Parse beginbfchar ... endbfchar
    let mut cur = 0usize;
    while let Some(rel) = find_bytes(&bytes[cur..], b"beginbfchar") {
        let start = cur + rel + b"beginbfchar".len();
        let Some(end_rel) = find_bytes(&bytes[start..], b"endbfchar") else {
            break;
        };
        let section = &bytes[start..start + end_rel];
        let mut i = 0;
        while i < section.len() {
            if section[i] == b'<'
                && let Some((src, next_i)) = parse_pdf_hex(section, i)
            {
                let mut j = next_i;
                while j < section.len() && section[j] != b'<' {
                    j += 1;
                }
                if let Some((dst, after_dst)) = parse_pdf_hex(section, j) {
                    let decoded = decode_utf16be_hex(&dst);
                    if !decoded.is_empty() {
                        map.insert(src, decoded);
                    }
                    i = after_dst;
                    continue;
                }
            }
            i += 1;
        }
        cur = start + end_rel + b"endbfchar".len();
    }

    // Parse beginbfrange ... endbfrange
    let mut cur = 0usize;
    while let Some(rel) = find_bytes(&bytes[cur..], b"beginbfrange") {
        let start = cur + rel + b"beginbfrange".len();
        let Some(end_rel) = find_bytes(&bytes[start..], b"endbfrange") else {
            break;
        };
        let section = &bytes[start..start + end_rel];
        let mut i = 0;
        while i < section.len() {
            if section[i] == b'<'
                && let Some((lo, next1)) = parse_pdf_hex(section, i)
            {
                let mut j = next1;
                while j < section.len() && section[j] != b'<' {
                    j += 1;
                }
                if let Some((hi, next2)) = parse_pdf_hex(section, j) {
                    let mut k = next2;
                    while k < section.len() && section[k].is_ascii_whitespace() {
                        k += 1;
                    }
                    if section.get(k) == Some(&b'<')
                        && let Some((base_dst, after_dst)) = parse_pdf_hex(section, k)
                        && lo.len() == hi.len()
                        && lo.len() <= 2
                        && base_dst.len() == 2
                    {
                        let lo_num = bytes_to_u32(&lo);
                        let hi_num = bytes_to_u32(&hi);
                        let base_cp = u16::from_be_bytes([base_dst[0], base_dst[1]]) as u32;
                        if hi_num >= lo_num && hi_num - lo_num <= 2048 {
                            for offset in 0..=(hi_num - lo_num) {
                                let code = lo_num + offset;
                                let key = if lo.len() == 1 {
                                    vec![code as u8]
                                } else {
                                    (code as u16).to_be_bytes().to_vec()
                                };
                                if let Some(ch) = char::from_u32(base_cp + offset) {
                                    map.insert(key, ch.to_string());
                                }
                            }
                        }
                        i = after_dst;
                        continue;
                    }
                }
            }
            i += 1;
        }
        cur = start + end_rel + b"endbfrange".len();
    }
}

fn bytes_to_u32(b: &[u8]) -> u32 {
    b.iter().fold(0u32, |acc, &x| (acc << 8) | (x as u32))
}

fn decode_utf16be_hex(raw: &[u8]) -> String {
    if raw.len() >= 2 && raw.len() % 2 == 0 {
        let units: Vec<u16> = raw
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        decode_pdf_string_bytes(raw)
    }
}

fn decode_hex_with_cmap(raw: &[u8], cmap: &HashMap<Vec<u8>, String>) -> String {
    if raw.len() >= 2 && raw[0] == 0xFE && raw[1] == 0xFF {
        return decode_pdf_string_bytes(raw);
    }
    if !cmap.is_empty() {
        let mut out = String::new();
        let mut i = 0usize;
        let mut matched_any = false;
        while i < raw.len() {
            if i + 2 <= raw.len()
                && let Some(s) = cmap.get(&raw[i..i + 2])
            {
                out.push_str(s);
                matched_any = true;
                i += 2;
                continue;
            }
            if let Some(s) = cmap.get(&raw[i..i + 1]) {
                out.push_str(s);
                matched_any = true;
                i += 1;
                continue;
            }
            i += 1;
        }
        if matched_any {
            return out;
        }
    }
    if raw.len() >= 2
        && raw.len() % 2 == 0
        && raw.chunks_exact(2).all(|c| {
            let u = u16::from_be_bytes([c[0], c[1]]);
            (0x0020..=0x007E).contains(&u)
                || (0x2000..=0x206F).contains(&u)
                || (0x3000..=0x303F).contains(&u)
                || (0x4E00..=0x9FFF).contains(&u)
                || (0xFF00..=0xFFEF).contains(&u)
        })
    {
        return decode_utf16be_hex(raw);
    }
    decode_pdf_string_bytes(raw)
}

fn extract_text_from_content_stream(stream: &[u8], cmap: &HashMap<Vec<u8>, String>) -> String {
    let mut out = String::new();
    let mut i = 0usize;
    let mut in_text = false;
    let mut last_tokens: Vec<Vec<u8>> = Vec::new();

    while i < stream.len() {
        let b = stream[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'%' {
            while i < stream.len() && stream[i] != b'\n' && stream[i] != b'\r' {
                i += 1;
            }
            continue;
        }
        if b == b'('
            && let Some((lit, next_i)) = parse_pdf_literal(stream, i)
        {
            if in_text {
                last_tokens.push(lit);
            }
            i = next_i;
            continue;
        }
        if b == b'<'
            && stream.get(i + 1) != Some(&b'<')
            && let Some((hex, next_i)) = parse_pdf_hex(stream, i)
        {
            if in_text {
                let mut tagged = Vec::with_capacity(hex.len() + 1);
                tagged.push(0x01); // marker for hex string
                tagged.extend_from_slice(&hex);
                last_tokens.push(tagged);
            }
            i = next_i;
            continue;
        }
        if b == b'[' && in_text {
            // TJ array: collect strings and emit spaces for large negative kerning offsets
            i += 1;
            let mut joined = String::new();
            while i < stream.len() && stream[i] != b']' {
                if stream[i] == b'('
                    && let Some((lit, next_i)) = parse_pdf_literal(stream, i)
                {
                    joined.push_str(&decode_pdf_string_bytes(&lit));
                    i = next_i;
                } else if stream[i] == b'<'
                    && let Some((hex, next_i)) = parse_pdf_hex(stream, i)
                {
                    joined.push_str(&decode_hex_with_cmap(&hex, cmap));
                    i = next_i;
                } else if stream[i] == b'-' || stream[i].is_ascii_digit() {
                    let num_start = i;
                    i += 1;
                    while i < stream.len() && (stream[i].is_ascii_digit() || stream[i] == b'.') {
                        i += 1;
                    }
                    if let Ok(num_str) = std::str::from_utf8(&stream[num_start..i])
                        && let Ok(val) = num_str.parse::<f32>()
                        && val <= -180.0
                        && !joined.ends_with(' ')
                    {
                        joined.push(' ');
                    }
                } else {
                    i += 1;
                }
            }
            if i < stream.len() && stream[i] == b']' {
                i += 1;
            }
            // Wait for TJ operator
            while i < stream.len() && stream[i].is_ascii_whitespace() {
                i += 1;
            }
            if stream.get(i..i + 2) == Some(b"TJ") {
                out.push_str(&joined);
                i += 2;
            }
            continue;
        }

        // Read operator word
        let start = i;
        while i < stream.len()
            && !stream[i].is_ascii_whitespace()
            && !matches!(
                stream[i],
                b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'/' | b'%'
            )
        {
            i += 1;
        }
        if i == start {
            i += 1;
            continue;
        }
        let op = &stream[start..i];
        match op {
            b"BT" => {
                in_text = true;
                last_tokens.clear();
            }
            b"ET" => {
                in_text = false;
                last_tokens.clear();
                if !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
            }
            b"Tj" | b"'" | b"\"" if in_text => {
                if op == b"'" || op == b"\"" {
                    if !out.ends_with('\n') && !out.is_empty() {
                        out.push('\n');
                    }
                }
                if let Some(tok) = last_tokens.pop() {
                    let s = if tok.first() == Some(&0x01) {
                        decode_hex_with_cmap(&tok[1..], cmap)
                    } else {
                        decode_pdf_string_bytes(&tok)
                    };
                    out.push_str(&s);
                }
                last_tokens.clear();
            }
            b"Td" | b"TD" | b"T*" | b"Tm" if in_text => {
                if !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
                last_tokens.clear();
            }
            _ => {}
        }
    }
    out
}

fn is_pdf_heading(line: &str) -> bool {
    let t = line.trim();
    let len = t.chars().count();
    if !(2..=64).contains(&len) {
        return false;
    }
    if t.ends_with(['。', '，', ',', ';', '；', '!', '！', '?']) {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("chapter ")
        || lower.starts_with("part ")
        || lower.starts_with("section ")
        || lower == "preface"
        || lower == "introduction"
        || lower == "epilogue"
        || lower == "conclusion"
    {
        return true;
    }
    if t.starts_with('第')
        && (t.contains('章') || t.contains('节') || t.contains('卷') || t.contains('回'))
    {
        return true;
    }
    if let Some((num, rest)) = t.split_once(". ")
        && !num.is_empty()
        && num.len() <= 3
        && num.chars().all(|c| c.is_ascii_digit())
        && !rest.trim().is_empty()
    {
        return true;
    }
    false
}

fn header_footer_fingerprint(line: &str) -> String {
    line.trim()
        .chars()
        .filter(|c| !c.is_ascii_digit() && !matches!(c, '-' | '—' | '·' | '|'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn strip_running_headers_footers(pages: &mut [Vec<String>]) {
    if pages.len() < 3 {
        return;
    }
    let threshold = pages.len().div_ceil(2);
    let mut first_counts: HashMap<String, usize> = HashMap::new();
    let mut last_counts: HashMap<String, usize> = HashMap::new();
    for page in pages.iter() {
        if let Some(first) = page.first()
            && first.chars().count() <= 90
        {
            let fp = header_footer_fingerprint(first);
            if fp.chars().count() >= 3 {
                *first_counts.entry(fp).or_insert(0) += 1;
            }
        }
        if page.len() >= 2
            && let Some(last) = page.last()
            && last.chars().count() <= 90
        {
            let fp = header_footer_fingerprint(last);
            if fp.chars().count() >= 3 {
                *last_counts.entry(fp).or_insert(0) += 1;
            }
        }
    }
    for page in pages.iter_mut() {
        if let Some(first) = page.first() {
            let fp = header_footer_fingerprint(first);
            if first_counts.get(&fp).copied().unwrap_or(0) >= threshold {
                page.remove(0);
            }
        }
        if let Some(last) = page.last() {
            let fp = header_footer_fingerprint(last);
            if last_counts.get(&fp).copied().unwrap_or(0) >= threshold {
                page.pop();
            }
        }
    }
}

fn clean_pdf_page_lines(raw: &str) -> Vec<String> {
    raw.lines()
        .map(|raw_line| {
            raw_line
                .chars()
                .filter(|c| !c.is_control() || *c == '\t')
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|trimmed| {
            if trimmed.is_empty() {
                return false;
            }
            let stripped_num = trimmed.trim_matches(['-', '—', '·', ' ']);
            !(stripped_num.len() <= 4
                && !stripped_num.is_empty()
                && stripped_num.chars().all(|c| c.is_ascii_digit()))
        })
        .collect()
}

fn reflow_pdf_lines_into_paragraphs(lines: &[String]) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    for trimmed in lines {
        if is_pdf_heading(trimmed) {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
            paragraphs.push(trimmed.clone());
            continue;
        }
        if !current.is_empty() {
            let ends_sentence =
                current.ends_with(['。', '！', '？', '.', '!', '?', '”', '」', '：', ':']);
            if ends_sentence && current.chars().count() >= 40 {
                paragraphs.push(std::mem::take(&mut current));
            } else if current.ends_with('-')
                && trimmed
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_lowercase())
            {
                current.pop();
            } else {
                let last_cjk = current
                    .chars()
                    .last()
                    .is_some_and(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c));
                let next_cjk = trimmed
                    .chars()
                    .next()
                    .is_some_and(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c));
                if !(last_cjk && next_cjk) {
                    current.push(' ');
                }
            }
        }
        current.push_str(trimmed);
    }
    if !current.is_empty() {
        paragraphs.push(current);
    }
    paragraphs
}

/// Extracts metadata and readable text layers from a PDF document and reflows them into an
/// eye-friendly EPUB that works seamlessly with Glow Reading's warm paper, dark inversion,
/// highlights, footnotes, and TTS.
pub fn pdf_to_epub(bytes: &[u8], fallback_title: &str) -> Result<NormalizedEpub> {
    ensure!(
        bytes.len() >= 8 && find_bytes(&bytes[..bytes.len().min(1024)], b"%PDF-").is_some(),
        "文件不是有效的 PDF 文档"
    );

    let title = extract_pdf_info_field(bytes, b"/Title")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if fallback_title.trim().is_empty() {
                "PDF 慢读文档".into()
            } else {
                fallback_title.trim().to_owned()
            }
        });
    let author = extract_pdf_info_field(bytes, b"/Author").unwrap_or_else(|| "PDF 文档".into());

    // Collect and decompress all PDF streams (skipping obvious image/font binary streams)
    let mut streams: Vec<Vec<u8>> = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel) = find_bytes(&bytes[cursor..], b"stream") {
        let kw_pos = cursor + rel;
        let dict_start = kw_pos.saturating_sub(512);
        let dict_slice = &bytes[dict_start..kw_pos];
        let mut data_start = kw_pos + b"stream".len();
        if bytes.get(data_start) == Some(&b'\r') && bytes.get(data_start + 1) == Some(&b'\n') {
            data_start += 2;
        } else if bytes.get(data_start) == Some(&b'\n') || bytes.get(data_start) == Some(&b'\r') {
            data_start += 1;
        } else {
            cursor = data_start;
            continue;
        }
        let Some(end_rel) = find_bytes(&bytes[data_start..], b"endstream") else {
            break;
        };
        let raw_stream = &bytes[data_start..data_start + end_rel];
        cursor = data_start + end_rel + b"endstream".len();

        let is_image = find_bytes(dict_slice, b"/Subtype /Image").is_some()
            || find_bytes(dict_slice, b"/Subtype/Image").is_some()
            || find_bytes(dict_slice, b"/DCTDecode").is_some()
            || find_bytes(dict_slice, b"/JPXDecode").is_some();
        if is_image || raw_stream.len() > 8 * 1024 * 1024 {
            continue;
        }

        let is_flate = find_bytes(dict_slice, b"/FlateDecode").is_some();
        if is_flate {
            let trimmed_raw = raw_stream.trim_ascii_end();
            let mut decoder = ZlibDecoder::new(trimmed_raw);
            let mut decompressed = Vec::new();
            if decoder
                .by_ref()
                .take(8 * 1024 * 1024)
                .read_to_end(&mut decompressed)
                .is_ok()
                && !decompressed.is_empty()
            {
                streams.push(decompressed);
            }
        } else {
            streams.push(raw_stream.to_vec());
        }
    }

    // Pass 1: build global ToUnicode CMap lookup from any CMap streams
    let mut cmap: HashMap<Vec<u8>, String> = HashMap::new();
    for s in &streams {
        parse_cmap_unicode(s, &mut cmap);
    }

    // Pass 2: extract cleaned raw lines per PDF page stream
    let mut raw_pages: Vec<Vec<String>> = Vec::new();
    for s in &streams {
        if find_bytes(s, b"BT").is_none() {
            continue;
        }
        let extracted = extract_text_from_content_stream(s, &cmap);
        let lines = clean_pdf_page_lines(&extracted);
        let total_chars: usize = lines.iter().map(|l| l.chars().count()).sum();
        if total_chars >= 4 {
            raw_pages.push(lines);
        }
    }

    strip_running_headers_footers(&mut raw_pages);

    let page_blocks: Vec<Vec<String>> = raw_pages
        .iter()
        .map(|lines| reflow_pdf_lines_into_paragraphs(lines))
        .filter(|paras| !paras.is_empty())
        .collect();

    ensure!(
        !page_blocks.is_empty(),
        "该 PDF 暂未检测到可提取的文本层（可能为纯图片扫描版或加密文档），暂无法重排阅读"
    );

    let total_pages = page_blocks.len();
    let reflow_preface = format!(
        "<p><em>PDF 护眼重排 · 原稿共 {total_pages} 页（已自动去除多余白边与重复页眉页脚，适配暖光纸面、划线与听书）</em></p><hr/>"
    );

    let detected_headings = page_blocks
        .iter()
        .flat_map(|p| p.iter())
        .filter(|line| is_pdf_heading(line))
        .count();

    let mut chapters: Vec<(String, String)> = Vec::new();
    if detected_headings >= 2 {
        // Structure chapters around real detected PDF headings for a clean navigable TOC
        let mut cur_heading = title.clone();
        let mut cur_html = reflow_preface;
        let mut has_body = false;
        for paras in &page_blocks {
            for para in paras {
                if is_pdf_heading(para) {
                    if has_body {
                        chapters.push((cur_heading.clone(), std::mem::take(&mut cur_html)));
                        has_body = false;
                    }
                    cur_heading = para.clone();
                } else {
                    cur_html.push_str(&format!("<p>{}</p>", xml_escape(para)));
                    has_body = true;
                }
            }
        }
        if has_body || chapters.is_empty() {
            chapters.push((cur_heading, cur_html));
        }
    } else {
        // Group extracted blocks cleanly into chapters (~5 PDF pages per section when long)
        let chunk_size = if page_blocks.len() <= 6 { 1 } else { 5 };
        for (idx, group) in page_blocks.chunks(chunk_size).enumerate() {
            let start_page = idx * chunk_size + 1;
            let end_page = start_page + group.len() - 1;
            let heading = if page_blocks.len() == 1 {
                title.clone()
            } else if start_page == end_page {
                format!("第 {start_page} 页")
            } else {
                format!("第 {start_page}–{end_page} 页")
            };
            let mut html = if idx == 0 {
                reflow_preface.clone()
            } else {
                String::new()
            };
            for (p_idx, paras) in group.iter().enumerate() {
                if group.len() > 1 {
                    html.push_str(&format!("<h3>— 第 {} 页 —</h3>", start_page + p_idx));
                }
                for para in paras {
                    if is_pdf_heading(para) {
                        html.push_str(&format!("<h2>{}</h2>", xml_escape(para)));
                    } else {
                        html.push_str(&format!("<p>{}</p>", xml_escape(para)));
                    }
                }
            }
            chapters.push((heading, html));
        }
    }

    build_chapters_epub(&title, &author, &chapters, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;

    #[test]
    fn converts_markdown_with_frontmatter_lists_quotes_and_code_to_epub() {
        let md = r#"---
title: 雨夜书房札记
author: 顾微光
---

# 第一章 · 点灯
这是一段带 **强调** 与 *轻斜体* 的段落，还有 `inline_code` 与 [温和链接](https://example.com)。

> 慢一点，书页不会跑掉。

- 泡一杯乌龙
- 打开细雨声景

```
fn breathe() {}
```

# 第二章 · 留白
允许自己在十点半合上书。
"#;
        let epub = markdown_to_epub(md, "fallback").unwrap();
        assert_eq!(epub.title, "雨夜书房札记");
        assert_eq!(epub.author, "顾微光");
        let files = unpack(&epub.bytes).unwrap();
        let c0 = std::str::from_utf8(&files["c0.xhtml"]).unwrap();
        assert!(c0.contains("<strong>强调</strong>"));
        assert!(c0.contains("<blockquote>"));
        assert!(c0.contains("<ul><li>泡一杯乌龙</li>"));
        assert!(c0.contains("<pre><code>fn breathe() {}</code></pre>"));
        assert!(files.contains_key("c1.xhtml"));
    }

    #[test]
    fn converts_minimal_ooxml_docx_to_valid_multi_chapter_epub() {
        let mut docx_files = BTreeMap::new();
        docx_files.insert(
            "docProps/core.xml".into(),
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
              <dc:title>山谷手稿</dc:title>
              <dc:creator>林青</dc:creator>
            </cp:coreProperties>"#
                .as_bytes()
                .to_vec(),
        );
        docx_files.insert(
            "word/document.xml".into(),
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:body>
                <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>序章</w:t></w:r></w:p>
                <w:p><w:r><w:rPr><w:b/></w:rPr><w:t>晚风吹过山谷。</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>后记</w:t></w:r></w:p>
                <w:p><w:r><w:t>愿你睡前安稳。</w:t></w:r></w:p>
              </w:body>
            </w:document>"#
                .as_bytes()
                .to_vec(),
        );
        let docx_bytes = pack(&docx_files).unwrap();
        let epub = docx_to_epub(&docx_bytes, "fallback").unwrap();
        assert_eq!(epub.title, "山谷手稿");
        assert_eq!(epub.author, "林青");
        let norm = crate::library::normalize_epub(&epub.bytes).unwrap();
        assert_eq!(norm.title, "山谷手稿");
        let files = unpack(&norm.bytes).unwrap();
        assert!(files.contains_key("c0.xhtml") && files.contains_key("c1.xhtml"));
        let c0 = std::str::from_utf8(&files["c0.xhtml"]).unwrap();
        assert!(c0.contains("<strong>晚风吹过山谷。</strong>"));
    }

    #[test]
    fn reflows_flate_compressed_pdf_with_cmap_and_utf16be_metadata_to_epub() {
        // Build a real compressed PDF with UTF-16BE Chinese title, a ToUnicode CMap, and FlateDecode text stream
        let cmap_stream = b"/CIDInit /ProcSet findresource begin\n1 beginbfchar\n<0001> <5FAE>\n<0002> <5149>\nendbfchar\nend\n";
        let page_content = b"BT\n/F1 12 Tf\n72 720 Td\n(Quiet evening lamp on warm paper.) Tj\n0 -18 Td\n<00010002> Tj\nET\n";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(page_content).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        pdf.extend_from_slice(
            b"1 0 obj\n<< /Title <FEFF5FAE5149624B518C> /Author (Glow Studio) >>\nendobj\n",
        );
        pdf.extend_from_slice(b"2 0 obj\n<< /Length ");
        pdf.extend_from_slice(cmap_stream.len().to_string().as_bytes());
        pdf.extend_from_slice(b" >>\nstream\n");
        pdf.extend_from_slice(cmap_stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        pdf.extend_from_slice(b"3 0 obj\n<< /Filter /FlateDecode /Length ");
        pdf.extend_from_slice(compressed.len().to_string().as_bytes());
        pdf.extend_from_slice(b" >>\nstream\n");
        pdf.extend_from_slice(&compressed);
        pdf.extend_from_slice(b"\nendstream\nendobj\n%%EOF\n");

        let epub = pdf_to_epub(&pdf, "fallback").unwrap();
        assert_eq!(epub.title, "微光手册");
        assert_eq!(epub.author, "Glow Studio");
        let files = unpack(&epub.bytes).unwrap();
        let c0 = std::str::from_utf8(&files["c0.xhtml"]).unwrap();
        assert!(c0.contains("Quiet evening lamp on warm paper."));
        assert!(c0.contains("微光"));

        // Scanned PDF without text stream should fail gracefully with a friendly message
        let scanned = b"%PDF-1.4\n1 0 obj\n<< /Subtype /Image >>\nstream\n\xFF\xD8\xFF\nendstream\nendobj\n%%EOF";
        assert!(pdf_to_epub(scanned, "scan").is_err());
    }

    #[test]
    fn strips_pdf_running_headers_and_splits_chapters_on_headings() {
        let p1 = b"BT\n(Glow Journal - Running Header) Tj\n0 -20 Td\n(Chapter 1 Morning Tea) Tj\n0 -20 Td\n(First chapter prose rests quietly on warm paper.) Tj\n0 -20 Td\n(Footer - Page 1) Tj\nET";
        let p2 = b"BT\n(Glow Journal - Running Header) Tj\n0 -20 Td\n(More thoughts from the first chapter.) Tj\n0 -20 Td\n(Footer - Page 2) Tj\nET";
        let p3 = b"BT\n(Glow Journal - Running Header) Tj\n0 -20 Td\n(Chapter 2 Evening Rain) Tj\n0 -20 Td\n(Second chapter unfolds slowly beside the rain window.) Tj\n0 -20 Td\n(Footer - Page 3) Tj\nET";

        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n1 0 obj\n<< /Title (Study Book) >>\nendobj\n");
        for (idx, page) in [p1.as_slice(), p2.as_slice(), p3.as_slice()]
            .into_iter()
            .enumerate()
        {
            pdf.extend_from_slice(
                format!("{} 0 obj\n<< /Length {} >>\nstream\n", idx + 2, page.len()).as_bytes(),
            );
            pdf.extend_from_slice(page);
            pdf.extend_from_slice(b"\nendstream\nendobj\n");
        }
        pdf.extend_from_slice(b"%%EOF\n");

        let epub = pdf_to_epub(&pdf, "fallback").unwrap();
        let files = unpack(&epub.bytes).unwrap();
        let nav = std::str::from_utf8(&files["nav.xhtml"]).unwrap();
        assert!(nav.contains("Chapter 1 Morning Tea"));
        assert!(nav.contains("Chapter 2 Evening Rain"));

        let c0 = std::str::from_utf8(&files["c0.xhtml"]).unwrap();
        assert!(!c0.contains("Running Header"));
        assert!(!c0.contains("Footer - Page 1"));
        assert!(c0.contains("First chapter prose rests quietly"));
        assert!(c0.contains("原稿共 3 页"));

        let c1 = std::str::from_utf8(&files["c1.xhtml"]).unwrap();
        assert!(c1.contains("Second chapter unfolds slowly"));
    }
}

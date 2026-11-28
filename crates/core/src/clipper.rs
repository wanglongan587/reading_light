use crate::library::pack;
use anyhow::{Result, ensure};
use html5ever::driver::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Distilled web article extracted cleanly from noisy HTML for distraction-free reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClippedArticle {
    pub title: String,
    pub author: String,
    pub site_name: Option<String>,
    pub source_url: Option<String>,
    pub excerpt: String,
    pub reading_minutes: u32,
    pub xhtml_body: String,
    pub plain_text: String,
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_ws = true;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_ws {
                out.push(' ');
                last_ws = true;
            }
        } else {
            out.push(ch);
            last_ws = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

fn attr_value(node: &Handle, name: &str) -> Option<String> {
    if let NodeData::Element { attrs, .. } = &node.data {
        for attr in attrs.borrow().iter() {
            if attr.name.local.to_string().eq_ignore_ascii_case(name) {
                return Some(attr.value.to_string());
            }
        }
    }
    None
}

fn tag_name(node: &Handle) -> Option<String> {
    if let NodeData::Element { name, .. } = &node.data {
        Some(name.local.to_string().to_ascii_lowercase())
    } else {
        None
    }
}

fn is_noise_node(node: &Handle) -> bool {
    let Some(tag) = tag_name(node) else {
        return false;
    };
    if matches!(
        tag.as_str(),
        "script"
            | "style"
            | "noscript"
            | "template"
            | "iframe"
            | "object"
            | "embed"
            | "form"
            | "input"
            | "button"
            | "select"
            | "textarea"
            | "svg"
            | "canvas"
            | "nav"
            | "footer"
            | "aside"
            | "audio"
            | "video"
            | "dialog"
    ) {
        return true;
    }
    if attr_value(node, "hidden").is_some()
        || attr_value(node, "aria-hidden").as_deref() == Some("true")
    {
        return true;
    }
    let hint = format!(
        "{} {}",
        attr_value(node, "class").unwrap_or_default(),
        attr_value(node, "id").unwrap_or_default()
    )
    .to_ascii_lowercase();
    if hint.trim().is_empty() {
        return false;
    }
    // Avoid discarding high-value article wrappers whose class happens to mention e.g. "has-sidebar"
    if matches!(tag.as_str(), "article" | "main" | "body" | "html") {
        return false;
    }
    [
        "sidebar",
        "comment",
        "disqus",
        "cookie",
        "banner",
        "advert",
        "ad-slot",
        "sponsor",
        "share-bar",
        "social-share",
        "related-posts",
        "recommend",
        "newsletter",
        "subscribe",
        "breadcrumb",
        "pagination",
        "popup",
        "modal",
        "toolbar",
        "topbar",
        "navbar",
        "site-header",
        "site-footer",
        "menu-list",
    ]
    .iter()
    .any(|kw| hint.contains(kw))
}

fn collect_plain_text(node: &Handle, out: &mut String) {
    if is_noise_node(node) {
        return;
    }
    match &node.data {
        NodeData::Text { contents } => {
            let chunk = contents.borrow();
            if !chunk.trim().is_empty() {
                if !out.is_empty() && !out.ends_with([' ', '\n']) {
                    out.push(' ');
                }
                out.push_str(chunk.trim());
            }
        }
        NodeData::Element { name, .. } => {
            let tag = name.local.to_string().to_ascii_lowercase();
            let block = matches!(
                tag.as_str(),
                "p" | "div"
                    | "section"
                    | "article"
                    | "li"
                    | "blockquote"
                    | "pre"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "br"
                    | "hr"
            );
            if block && !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            for child in node.children.borrow().iter() {
                collect_plain_text(child, out);
            }
            if block && !out.ends_with('\n') {
                out.push('\n');
            }
        }
        _ => {
            for child in node.children.borrow().iter() {
                collect_plain_text(child, out);
            }
        }
    }
}

struct MetaInfo {
    title: Option<String>,
    og_title: Option<String>,
    h1_title: Option<String>,
    author: Option<String>,
    site_name: Option<String>,
    description: Option<String>,
}

fn collect_metadata(node: &Handle, meta: &mut MetaInfo) {
    if let Some(tag) = tag_name(node) {
        match tag.as_str() {
            "title" if meta.title.is_none() => {
                let mut text = String::new();
                collect_plain_text(node, &mut text);
                let clean = collapse_whitespace(&text);
                if !clean.is_empty() {
                    meta.title = Some(clean);
                }
            }
            "h1" if meta.h1_title.is_none() => {
                let mut text = String::new();
                collect_plain_text(node, &mut text);
                let clean = collapse_whitespace(&text);
                if clean.chars().count() >= 2 && clean.chars().count() <= 140 {
                    meta.h1_title = Some(clean);
                }
            }
            "meta" => {
                let key = attr_value(node, "property")
                    .or_else(|| attr_value(node, "name"))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let content = attr_value(node, "content")
                    .map(|s| collapse_whitespace(&s))
                    .filter(|s| !s.is_empty());
                if let Some(val) = content {
                    match key.as_str() {
                        "og:title" | "twitter:title" if meta.og_title.is_none() => {
                            meta.og_title = Some(val);
                        }
                        "author" | "article:author" | "dc.creator" if meta.author.is_none() => {
                            meta.author = Some(val);
                        }
                        "og:site_name" | "application-name" if meta.site_name.is_none() => {
                            meta.site_name = Some(val);
                        }
                        "description" | "og:description" if meta.description.is_none() => {
                            meta.description = Some(val);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    for child in node.children.borrow().iter() {
        collect_metadata(child, meta);
    }
}

fn score_candidate(node: &Handle) -> i64 {
    if is_noise_node(node) {
        return -10_000;
    }
    let Some(tag) = tag_name(node) else {
        return 0;
    };
    let mut p_chars = 0i64;
    let mut p_count = 0i64;
    let mut link_chars = 0i64;
    fn walk(n: &Handle, p_chars: &mut i64, p_count: &mut i64, link_chars: &mut i64) {
        if is_noise_node(n) {
            return;
        }
        if let Some(t) = tag_name(n) {
            if matches!(t.as_str(), "p" | "blockquote" | "li" | "pre") {
                let mut buf = String::new();
                collect_plain_text(n, &mut buf);
                let len = buf.trim().chars().count() as i64;
                if len >= 18 {
                    *p_chars += len;
                    *p_count += 1;
                }
                return;
            }
            if t == "a" {
                let mut buf = String::new();
                collect_plain_text(n, &mut buf);
                *link_chars += buf.trim().chars().count() as i64;
                return;
            }
        }
        for child in n.children.borrow().iter() {
            walk(child, p_chars, p_count, link_chars);
        }
    }
    walk(node, &mut p_chars, &mut p_count, &mut link_chars);
    let bonus = match tag.as_str() {
        "article" => 260,
        "main" => 180,
        "section" => 60,
        _ => 0,
    };
    let class_hint = format!(
        "{} {}",
        attr_value(node, "class").unwrap_or_default(),
        attr_value(node, "id").unwrap_or_default()
    )
    .to_ascii_lowercase();
    let semantic_bonus = if [
        "article", "post", "entry", "content", "prose", "markdown", "story", "essay",
    ]
    .iter()
    .any(|kw| class_hint.contains(kw))
    {
        120
    } else {
        0
    };
    bonus + semantic_bonus + p_chars + p_count * 24 - link_chars
}

fn find_best_container(node: &Handle, best: &mut Option<(i64, Handle)>) {
    if is_noise_node(node) {
        return;
    }
    if let Some(tag) = tag_name(node)
        && matches!(
            tag.as_str(),
            "article" | "main" | "section" | "div" | "body"
        )
    {
        let score = score_candidate(node);
        let replace = match best {
            None => score > 0,
            Some((best_score, _)) => score > *best_score,
        };
        if replace {
            *best = Some((score, node.clone()));
        }
    }
    for child in node.children.borrow().iter() {
        find_best_container(child, best);
    }
}

fn render_clean_xhtml(node: &Handle, out: &mut String, plain: &mut String) {
    if is_noise_node(node) {
        return;
    }
    match &node.data {
        NodeData::Text { contents } => {
            let text = collapse_whitespace(&contents.borrow());
            if !text.is_empty() {
                out.push_str(&xml_escape(&text));
                if !plain.is_empty() && !plain.ends_with([' ', '\n']) {
                    plain.push(' ');
                }
                plain.push_str(&text);
            }
        }
        NodeData::Element { name, .. } => {
            let tag = name.local.to_string().to_ascii_lowercase();
            match tag.as_str() {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "blockquote" | "ul" | "ol"
                | "li" | "pre" => {
                    let mut inner_xhtml = String::new();
                    let mut inner_plain = String::new();
                    for child in node.children.borrow().iter() {
                        render_clean_xhtml(child, &mut inner_xhtml, &mut inner_plain);
                    }
                    if !inner_plain.trim().is_empty() {
                        // Demote h1 inside body to h2 so chapter title remains unique h1
                        let emit_tag = if tag == "h1" { "h2" } else { tag.as_str() };
                        out.push_str(&format!("<{emit_tag}>{inner_xhtml}</{emit_tag}>"));
                        if !plain.is_empty() && !plain.ends_with('\n') {
                            plain.push('\n');
                        }
                        plain.push_str(inner_plain.trim());
                        plain.push('\n');
                    }
                }
                "strong" | "b" => {
                    let mut inner = String::new();
                    for child in node.children.borrow().iter() {
                        render_clean_xhtml(child, &mut inner, plain);
                    }
                    if !inner.trim().is_empty() {
                        out.push_str(&format!("<strong>{inner}</strong>"));
                    }
                }
                "em" | "i" => {
                    let mut inner = String::new();
                    for child in node.children.borrow().iter() {
                        render_clean_xhtml(child, &mut inner, plain);
                    }
                    if !inner.trim().is_empty() {
                        out.push_str(&format!("<em>{inner}</em>"));
                    }
                }
                "code" => {
                    let mut inner = String::new();
                    for child in node.children.borrow().iter() {
                        render_clean_xhtml(child, &mut inner, plain);
                    }
                    if !inner.trim().is_empty() {
                        out.push_str(&format!("<code>{inner}</code>"));
                    }
                }
                "br" => {
                    out.push_str("<br/>");
                    plain.push('\n');
                }
                "hr" => {
                    out.push_str("<hr/>");
                }
                // Unwrap generic containers and links (no external URLs inside sanitized EPUB)
                _ => {
                    for child in node.children.borrow().iter() {
                        render_clean_xhtml(child, out, plain);
                    }
                }
            }
        }
        _ => {
            for child in node.children.borrow().iter() {
                render_clean_xhtml(child, out, plain);
            }
        }
    }
}

pub fn estimate_reading_minutes(plain_text: &str) -> u32 {
    let mut cjk = 0usize;
    let mut ascii_words = 0usize;
    let mut in_word = false;
    for ch in plain_text.chars() {
        if ('\u{4E00}'..='\u{9FFF}').contains(&ch) || ('\u{3400}'..='\u{4DBF}').contains(&ch) {
            cjk += 1;
            in_word = false;
        } else if ch.is_ascii_alphanumeric() {
            if !in_word {
                ascii_words += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }
    let mins = (cjk as f32 / 280.0) + (ascii_words as f32 / 200.0);
    (mins.ceil() as u32).max(1)
}

fn host_from_url(url: &str) -> Option<String> {
    let rest = url
        .trim()
        .strip_prefix("https://")
        .or_else(|| url.trim().strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.strip_prefix("www.").unwrap_or(host).to_owned())
    }
}

/// Distills a raw HTML page into a clean, noise-free article suitable for Slow-Read EPUB packaging.
pub fn extract_readable_article(html: &str, source_url: Option<&str>) -> Result<ClippedArticle> {
    ensure!(!html.trim().is_empty(), "网页内容为空，无法剪藏");
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(html.as_bytes()))
        .map_err(|e| anyhow::anyhow!("HTML 解析失败：{e}"))?;

    let mut meta = MetaInfo {
        title: None,
        og_title: None,
        h1_title: None,
        author: None,
        site_name: None,
        description: None,
    };
    collect_metadata(&dom.document, &mut meta);

    let mut best: Option<(i64, Handle)> = None;
    find_best_container(&dom.document, &mut best);
    let container = best.map(|(_, h)| h).unwrap_or_else(|| dom.document.clone());

    let mut xhtml_body = String::new();
    let mut plain_text = String::new();
    render_clean_xhtml(&container, &mut xhtml_body, &mut plain_text);

    // If strict semantic rendering yielded very little (e.g., plain text in <div>s), fall back to line paragraphs
    if plain_text.trim().chars().count() < 24 {
        let mut fallback = String::new();
        collect_plain_text(&container, &mut fallback);
        let paras: Vec<String> = fallback
            .lines()
            .map(collapse_whitespace)
            .filter(|l| l.chars().count() >= 4)
            .collect();
        if !paras.is_empty() {
            plain_text = paras.join("\n");
            xhtml_body = paras
                .iter()
                .enumerate()
                .map(|(i, p)| format!("<p id=\"p{i}\">{}</p>", xml_escape(p)))
                .collect();
        }
    }

    ensure!(
        plain_text.trim().chars().count() >= 12,
        "未检测到足够的正文段落，无法提取可读文章"
    );

    let site_name = meta
        .site_name
        .or_else(|| source_url.and_then(host_from_url));
    let raw_title = meta
        .og_title
        .or(meta.h1_title)
        .or(meta.title)
        .unwrap_or_else(|| "网页慢读剪藏".into());
    // Strip trailing " - Site Name" or " | Site Name" when redundant
    let title = raw_title
        .split(" | ")
        .next()
        .unwrap_or(&raw_title)
        .trim()
        .to_owned();
    let author = meta
        .author
        .or_else(|| site_name.clone())
        .unwrap_or_else(|| "网页剪藏".into());
    let excerpt = meta.description.unwrap_or_else(|| {
        plain_text
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .chars()
            .take(120)
            .collect()
    });
    let reading_minutes = estimate_reading_minutes(&plain_text);

    Ok(ClippedArticle {
        title: if title.is_empty() {
            "网页慢读剪藏".into()
        } else {
            title
        },
        author,
        site_name,
        source_url: source_url.map(|s| s.trim().to_owned()),
        excerpt,
        reading_minutes,
        xhtml_body,
        plain_text: plain_text.trim().to_owned(),
    })
}

/// Packages a distilled `ClippedArticle` into a clean, compliant, offline-ready EPUB 3 file.
pub fn article_to_epub(article: &ClippedArticle) -> Result<Vec<u8>> {
    ensure!(!article.xhtml_body.trim().is_empty(), "文章正文为空");
    let mut files = BTreeMap::new();
    files.insert("mimetype".into(), b"application/epub+zip".to_vec());
    files.insert(
        "META-INF/container.xml".into(),
        br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec(),
    );

    let source_meta = match (&article.site_name, &article.source_url) {
        (Some(site), Some(url)) => format!(
            "<p><em>来源：{} · {} · 预计静读 {} 分钟</em></p><hr/>",
            xml_escape(site),
            xml_escape(url),
            article.reading_minutes
        ),
        (None, Some(url)) => format!(
            "<p><em>来源：{} · 预计静读 {} 分钟</em></p><hr/>",
            xml_escape(url),
            article.reading_minutes
        ),
        (Some(site), None) => format!(
            "<p><em>来源：{} · 预计静读 {} 分钟</em></p><hr/>",
            xml_escape(site),
            article.reading_minutes
        ),
        (None, None) => format!(
            "<p><em>预计静读 {} 分钟</em></p><hr/>",
            article.reading_minutes
        ),
    };

    let chapter_xhtml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>{title}</title></head><body><h1>{title}</h1>{source_meta}{body}</body></html>",
        title = xml_escape(&article.title),
        body = article.xhtml_body
    );
    files.insert("c0.xhtml".into(), chapter_xhtml.into_bytes());

    let id = format!(
        "{:x}",
        Sha256::digest(format!("{}|{}", article.title, article.plain_text).as_bytes())
    );
    files.insert(
        "book.opf".into(),
        format!(
            "<?xml version=\"1.0\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"id\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"id\">{id}</dc:identifier><dc:title>{title}</dc:title><dc:creator>{author}</dc:creator><dc:language>zh-CN</dc:language><meta property=\"dcterms:modified\">2026-01-01T00:00:00Z</meta></metadata><manifest><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/><item id=\"c0\" href=\"c0.xhtml\" media-type=\"application/xhtml+xml\"/></manifest><spine><itemref idref=\"c0\"/></spine></package>",
            title = xml_escape(&article.title),
            author = xml_escape(&article.author),
        )
        .into_bytes(),
    );
    files.insert(
        "nav.xhtml".into(),
        format!(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><title>目录</title></head><body><nav epub:type=\"toc\"><ol><li><a href=\"c0.xhtml\">{}</a></li></ol></nav></body></html>",
            xml_escape(&article.title)
        )
        .into_bytes(),
    );
    pack(&files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distills_noisy_html_and_builds_valid_sanitized_epub() {
        let html = r#"<!doctype html>
        <html>
          <head>
            <title>慢下来的艺术 | 宁静周刊</title>
            <meta name="author" content="林间客" />
            <meta property="og:site_name" content="宁静周刊" />
            <style>.ad { color: red; }</style>
            <script>alert("xss");</script>
          </head>
          <body>
            <nav><a href="/">首页</a><a href="/about">关于</a></nav>
            <div class="sidebar">热门广告推荐与无关弹窗</div>
            <main>
              <article>
                <h1>慢下来的艺术</h1>
                <p>傍晚下起细雨时，把手机调成静音，为自己留出二十分钟阅读。</p>
                <blockquote>好书从不催促读者，它只在灯下静静等候。</blockquote>
                <p>当我们停止追逐每一条推送，思绪才会重新恢复澄澈与弹性。</p>
              </article>
            </main>
            <footer>版权所有</footer>
          </body>
        </html>"#;

        let article =
            extract_readable_article(html, Some("https://quiet.example.com/slow")).unwrap();
        assert_eq!(article.title, "慢下来的艺术");
        assert_eq!(article.author, "林间客");
        assert_eq!(article.site_name.as_deref(), Some("宁静周刊"));
        assert!(article.plain_text.contains("为自己留出二十分钟阅读"));
        assert!(!article.plain_text.contains("热门广告推荐"));
        assert!(!article.xhtml_body.contains("alert("));
        assert!(article.xhtml_body.contains("<blockquote>"));

        let epub = article_to_epub(&article).unwrap();
        let normalized = crate::library::normalize_epub(&epub).unwrap();
        assert_eq!(normalized.title, "慢下来的艺术");
        assert_eq!(normalized.author, "林间客");
    }
}

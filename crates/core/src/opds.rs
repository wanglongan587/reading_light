use crate::model::{OpdsFeedConfig, valid_radio_stream_url};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpdsEntry {
    pub id: String,
    pub title: String,
    pub author: String,
    pub summary: String,
    pub epub_url: Option<String>,
    pub subsection_url: Option<String>,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpdsFeed {
    pub title: String,
    pub entries: Vec<OpdsEntry>,
}

/// Curated public-domain OPDS 1.2 starter catalogs (users may also add self-hosted Calibre/Komga/Kavita feeds).
pub fn preset_feeds() -> Vec<OpdsFeedConfig> {
    vec![OpdsFeedConfig {
        id: "preset-standard-ebooks".into(),
        name: "Standard Ebooks · 精排英文公版馆".into(),
        url: "https://standardebooks.org/feeds/opds".into(),
    }]
}

/// Resolves an OPDS href relative to a feed's HTTP(S) base URL, rejecting unsafe schemes.
pub fn resolve_opds_url(base_url: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.chars().any(char::is_control) {
        return None;
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return valid_radio_stream_url(href).then(|| href.to_owned());
    }
    // Reject non-http schemes like javascript:, file:, data:
    if let Some(colon) = href.find(':') {
        let prefix = &href[..colon];
        if !prefix.is_empty() && !prefix.contains('/') {
            return None;
        }
    }
    let base = base_url.trim();
    if !valid_radio_stream_url(base) {
        return None;
    }
    let scheme_end = base.find("://")? + 3;
    let rest = &base[scheme_end..];
    let (authority, base_path) = match rest.find('/') {
        Some(idx) => (&base[..scheme_end + idx], &rest[idx..]),
        None => (base, "/"),
    };
    let resolved = if href.starts_with('/') {
        format!("{authority}{href}")
    } else {
        let dir = match base_path.rfind('/') {
            Some(idx) => &base_path[..=idx],
            None => "/",
        };
        format!("{authority}{dir}{href}")
    };
    valid_radio_stream_url(&resolved).then_some(resolved)
}

/// Parses an OPDS 1.2 Atom XML feed document safely using `roxmltree`.
pub fn parse_feed(xml_text: &str, base_url: &str) -> Result<OpdsFeed> {
    ensure!(xml_text.len() <= 8 * 1024 * 1024, "OPDS 目录文件过大");
    let doc = roxmltree::Document::parse(xml_text).context("无法解析 OPDS 1.2 Atom XML 内容")?;
    let root = doc.root_element();
    ensure!(
        root.tag_name().name() == "feed",
        "这不是标准的 OPDS 1.2 Atom 书源（缺少 <feed> 根节点）"
    );

    let feed_title = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "title")
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("OPDS 书源")
        .to_owned();

    let mut entries = Vec::new();
    for node in root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "entry")
    {
        let id = child_text(node, "id").unwrap_or_default();
        let title = child_text(node, "title").unwrap_or_else(|| "未命名条目".into());
        let author = node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "author")
            .and_then(|author_node| child_text(author_node, "name"))
            .unwrap_or_else(|| "佚名".into());
        let summary = child_text(node, "summary")
            .or_else(|| child_text(node, "content"))
            .unwrap_or_default();

        let mut epub_url = None;
        let mut subsection_url = None;
        let mut cover_url = None;

        for link in node
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "link")
        {
            let Some(href) = link.attribute("href") else {
                continue;
            };
            let Some(resolved) = resolve_opds_url(base_url, href) else {
                continue;
            };
            let rel = link.attribute("rel").unwrap_or("");
            let media_type = link.attribute("type").unwrap_or("").to_ascii_lowercase();

            if media_type.contains("application/epub+zip")
                && (rel.is_empty() || rel.contains("acquisition"))
                && epub_url.is_none()
            {
                epub_url = Some(resolved);
            } else if (rel == "subsection"
                || media_type.contains("profile=opds-catalog")
                || media_type.contains("application/atom+xml"))
                && subsection_url.is_none()
            {
                subsection_url = Some(resolved);
            } else if (rel.contains("opds-spec.org/image") || rel == "http://opds-spec.org/cover")
                && cover_url.is_none()
            {
                cover_url = Some(resolved);
            }
        }

        if epub_url.is_some() || subsection_url.is_some() {
            entries.push(OpdsEntry {
                id: if id.is_empty() { title.clone() } else { id },
                title,
                author,
                summary: clean_summary(&summary),
                epub_url,
                subsection_url,
                cover_url,
            });
        }
        if entries.len() >= 120 {
            break;
        }
    }

    Ok(OpdsFeed {
        title: feed_title,
        entries,
    })
}

fn child_text(node: roxmltree::Node<'_, '_>, tag: &str) -> Option<String> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == tag)
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn clean_summary(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(220));
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => {
                if ch.is_whitespace() {
                    if !out.ends_with(' ') && !out.is_empty() {
                        out.push(' ');
                    }
                } else {
                    out.push(ch);
                }
            }
            _ => {}
        }
        if out.chars().count() >= 160 {
            out.push('…');
            break;
        }
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_opds_acquisition_and_navigation_entries_safely() {
        let sample = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Calibre 私人书库</title>
  <entry>
    <id>urn:book:1</id>
    <title>瓦尔登湖</title>
    <author><name>亨利·戴维·梭罗</name></author>
    <summary>&lt;p&gt;在湖畔安静地生活。&lt;/p&gt;</summary>
    <link rel="http://opds-spec.org/acquisition" type="application/epub+zip" href="/books/walden.epub"/>
    <link rel="http://opds-spec.org/image/thumbnail" type="image/jpeg" href="covers/1.jpg"/>
  </entry>
  <entry>
    <id>urn:nav:2</id>
    <title>散文与随笔</title>
    <link rel="subsection" type="application/atom+xml;profile=opds-catalog" href="essays.xml"/>
  </entry>
  <entry>
    <id>urn:evil:3</id>
    <title>不安全链接</title>
    <link rel="http://opds-spec.org/acquisition" type="application/epub+zip" href="file:///etc/passwd"/>
  </entry>
</feed>"#;
        let feed = parse_feed(sample, "https://books.example.com/opds/index.xml").unwrap();
        assert_eq!(feed.title, "Calibre 私人书库");
        assert_eq!(feed.entries.len(), 2);
        assert_eq!(
            feed.entries[0].epub_url.as_deref(),
            Some("https://books.example.com/books/walden.epub")
        );
        assert_eq!(
            feed.entries[0].cover_url.as_deref(),
            Some("https://books.example.com/opds/covers/1.jpg")
        );
        assert_eq!(
            feed.entries[1].subsection_url.as_deref(),
            Some("https://books.example.com/opds/essays.xml")
        );
    }
}

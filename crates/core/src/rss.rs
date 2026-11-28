use crate::{clipper, library::pack};
use anyhow::{Context, Result, ensure};
use roxmltree::Document;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Parsed RSS 2.0 or Atom 1.0 feed for Glow Reading's Slow-Read Inbox ("慢读箱").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RssFeed {
    pub title: String,
    pub subtitle: Option<String>,
    pub site_url: Option<String>,
    pub items: Vec<RssItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RssItem {
    pub id: String,
    pub title: String,
    pub author: String,
    pub link: Option<String>,
    pub published_at: Option<String>,
    pub summary: String,
    pub content_html: String,
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn child_text<'a>(node: roxmltree::Node<'a, 'a>, local_names: &[&str]) -> Option<String> {
    node.children()
        .find(|n| n.is_element() && local_names.contains(&n.tag_name().name()))
        .and_then(|n| n.text())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn strip_html_summary(raw: &str) -> String {
    if raw.contains('<') && raw.contains('>') {
        if let Ok(article) = clipper::extract_readable_article(raw, None) {
            return article.plain_text.chars().take(180).collect();
        }
    }
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(180)
        .collect()
}

fn safe_http_link(raw: Option<String>) -> Option<String> {
    let url = raw?.trim().to_owned();
    if (url.starts_with("https://") || url.starts_with("http://"))
        && !url.chars().any(char::is_whitespace)
    {
        Some(url)
    } else {
        None
    }
}

/// Parses an RSS 2.0 or Atom 1.0 XML feed into normalized Slow-Read items.
pub fn parse_feed(xml_str: &str, feed_url: &str) -> Result<RssFeed> {
    let trimmed = xml_str.trim_start_matches('\u{feff}').trim();
    ensure!(!trimmed.is_empty(), "RSS 订阅内容为空");
    let doc = Document::parse(trimmed).context("RSS / Atom XML 解析失败")?;
    let root = doc.root_element();

    match root.tag_name().name() {
        "rss" | "RDF" => parse_rss2(root, feed_url),
        "feed" => parse_atom(root, feed_url),
        other => anyhow::bail!("不支持的订阅格式根节点：<{other}>（支持 RSS 2.0 与 Atom 1.0）"),
    }
}

fn parse_rss2(root: roxmltree::Node<'_, '_>, feed_url: &str) -> Result<RssFeed> {
    let channel = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "channel")
        .unwrap_or(root);
    let feed_title = child_text(channel, &["title"]).unwrap_or_else(|| "慢读订阅源".into());
    let subtitle = child_text(channel, &["description"]);
    let site_url = safe_http_link(child_text(channel, &["link"]));

    let mut items = Vec::new();
    for item_node in channel
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "item")
        .take(40)
    {
        let title = child_text(item_node, &["title"]).unwrap_or_else(|| "无题文章".into());
        let link = safe_http_link(child_text(item_node, &["link"]));
        let author =
            child_text(item_node, &["creator", "author"]).unwrap_or_else(|| feed_title.clone());
        let published_at = child_text(item_node, &["pubDate", "date"]);
        let content_encoded = child_text(item_node, &["encoded", "content"]);
        let description = child_text(item_node, &["description"]).unwrap_or_default();
        let content_html = content_encoded
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| description.clone());
        let summary = strip_html_summary(if !description.is_empty() {
            &description
        } else {
            &content_html
        });
        let id = child_text(item_node, &["guid"])
            .or_else(|| link.clone())
            .unwrap_or_else(|| format!("{feed_url}#{}", items.len()));
        items.push(RssItem {
            id,
            title,
            author,
            link,
            published_at,
            summary,
            content_html,
        });
    }
    ensure!(!items.is_empty(), "该 RSS 订阅源中暂时没有文章条目");
    Ok(RssFeed {
        title: feed_title,
        subtitle,
        site_url,
        items,
    })
}

fn parse_atom(root: roxmltree::Node<'_, '_>, feed_url: &str) -> Result<RssFeed> {
    let feed_title = child_text(root, &["title"]).unwrap_or_else(|| "Atom 慢读源".into());
    let subtitle = child_text(root, &["subtitle"]);
    let site_url = root
        .children()
        .find(|n| {
            n.is_element()
                && n.tag_name().name() == "link"
                && n.attribute("rel").unwrap_or("alternate") == "alternate"
        })
        .and_then(|n| n.attribute("href").map(str::to_owned));
    let site_url = safe_http_link(site_url);

    let mut items = Vec::new();
    for entry in root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "entry")
        .take(40)
    {
        let title = child_text(entry, &["title"]).unwrap_or_else(|| "无题文章".into());
        let link = safe_http_link(
            entry
                .children()
                .find(|n| {
                    n.is_element()
                        && n.tag_name().name() == "link"
                        && n.attribute("rel").unwrap_or("alternate") == "alternate"
                })
                .and_then(|n| n.attribute("href").map(str::to_owned)),
        );
        let author = entry
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "author")
            .and_then(|a| child_text(a, &["name"]))
            .unwrap_or_else(|| feed_title.clone());
        let published_at = child_text(entry, &["published", "updated"]);
        let content = child_text(entry, &["content"]);
        let summary_raw = child_text(entry, &["summary"]).unwrap_or_default();
        let content_html = content
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| summary_raw.clone());
        let summary = strip_html_summary(if !summary_raw.is_empty() {
            &summary_raw
        } else {
            &content_html
        });
        let id = child_text(entry, &["id"])
            .or_else(|| link.clone())
            .unwrap_or_else(|| format!("{feed_url}#{}", items.len()));
        items.push(RssItem {
            id,
            title,
            author,
            link,
            published_at,
            summary,
            content_html,
        });
    }
    ensure!(!items.is_empty(), "该 Atom 订阅源中暂时没有文章条目");
    Ok(RssFeed {
        title: feed_title,
        subtitle,
        site_url,
        items,
    })
}

fn item_body_xhtml(item: &RssItem) -> String {
    if !item.content_html.trim().is_empty() {
        let wrapped = format!("<article>{}</article>", item.content_html);
        if let Ok(article) = clipper::extract_readable_article(&wrapped, item.link.as_deref()) {
            return article.xhtml_body;
        }
    }
    let fallback = if !item.summary.trim().is_empty() {
        item.summary.as_str()
    } else {
        "这篇文章仅提供摘要，你可以点击原链接或在剪藏框输入文章地址获取全文。"
    };
    format!("<p>{}</p>", xml_escape(fallback))
}

/// Converts a single RSS/Atom entry directly into an offline-ready EPUB book.
pub fn item_to_epub(item: &RssItem, feed_title: &str) -> Result<Vec<u8>> {
    let body = item_body_xhtml(item);
    let plain = strip_html_summary(&item.content_html);
    let article = clipper::ClippedArticle {
        title: item.title.clone(),
        author: if item.author.trim().is_empty() {
            feed_title.to_owned()
        } else {
            item.author.clone()
        },
        site_name: Some(feed_title.to_owned()),
        source_url: item.link.clone(),
        excerpt: item.summary.clone(),
        reading_minutes: clipper::estimate_reading_minutes(&plain),
        xhtml_body: body,
        plain_text: if plain.is_empty() {
            item.title.clone()
        } else {
            plain
        },
    };
    clipper::article_to_epub(&article)
}

/// Compiles multiple RSS/Atom entries into a curated "微光周末慢读小报" multi-chapter EPUB issue.
pub fn compile_weekend_digest(
    issue_title: &str,
    issue_date: &str,
    entries: &[(String, RssItem)],
) -> Result<Vec<u8>> {
    ensure!(!entries.is_empty(), "请先拉取至少一篇慢读文章再合成小报");
    let mut files = BTreeMap::new();
    files.insert("mimetype".into(), b"application/epub+zip".to_vec());
    files.insert(
        "META-INF/container.xml".into(),
        br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec(),
    );

    let mut manifest = String::new();
    let mut spine = String::new();
    let mut nav = String::new();

    // Cover editorial preface chapter
    manifest.push_str("<item id=\"c0\" href=\"c0.xhtml\" media-type=\"application/xhtml+xml\"/>");
    spine.push_str("<itemref idref=\"c0\"/>");
    nav.push_str("<li><a href=\"c0.xhtml\">本期卷首语</a></li>");
    let mut toc_list_html = String::new();
    for (idx, (source, item)) in entries.iter().enumerate() {
        toc_list_html.push_str(&format!(
            "<li><strong>{}</strong> — {}（{}）</li>",
            xml_escape(&item.title),
            xml_escape(&item.author),
            xml_escape(source)
        ));
        let cid = idx + 1;
        manifest.push_str(&format!(
            "<item id=\"c{cid}\" href=\"c{cid}.xhtml\" media-type=\"application/xhtml+xml\"/>"
        ));
        spine.push_str(&format!("<itemref idref=\"c{cid}\"/>"));
        nav.push_str(&format!(
            "<li><a href=\"c{cid}.xhtml\">{}</a></li>",
            xml_escape(&item.title)
        ));
        let meta_line = match &item.published_at {
            Some(pub_date) => format!("{} · {} · {}", item.author, source, pub_date),
            None => format!("{} · {}", item.author, source),
        };
        let body = item_body_xhtml(item);
        let xhtml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>{title}</title></head><body><h1>{title}</h1><p><em>{meta}</em></p><hr/>{body}</body></html>",
            title = xml_escape(&item.title),
            meta = xml_escape(&meta_line),
        );
        files.insert(format!("c{cid}.xhtml"), xhtml.into_bytes());
    }

    let preface_xhtml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>{title}</title></head><body><h1>{title}</h1><p><em>刊期：{date} · 共收录 {count} 篇慢读好文</em></p><p>把本周零碎的好文字合订成一册安静的小报。泡一杯热茶，不必追赶信息流，慢慢翻完这一期就好。</p><hr/><ol>{toc_list_html}</ol></body></html>",
        title = xml_escape(issue_title),
        date = xml_escape(issue_date),
        count = entries.len(),
    );
    files.insert("c0.xhtml".into(), preface_xhtml.into_bytes());

    let mut hasher = Sha256::new();
    hasher.update(issue_title.as_bytes());
    hasher.update(issue_date.as_bytes());
    for (src, item) in entries {
        hasher.update(src.as_bytes());
        hasher.update(item.id.as_bytes());
    }
    let id = format!("{:x}", hasher.finalize());

    files.insert(
        "book.opf".into(),
        format!(
            "<?xml version=\"1.0\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"id\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"id\">{id}</dc:identifier><dc:title>{title}</dc:title><dc:creator>微光慢读箱</dc:creator><dc:language>zh-CN</dc:language><meta property=\"dcterms:modified\">2026-01-01T00:00:00Z</meta></metadata><manifest><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>{manifest}</manifest><spine>{spine}</spine></package>",
            title = xml_escape(issue_title),
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
    pack(&files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rss2_and_atom_and_compiles_weekend_digest_epub() {
        let rss_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/">
          <channel>
            <title>林间慢读通讯</title>
            <link>https://forest.example.com</link>
            <description>每周一封安静的书信</description>
            <item>
              <title>秋夜泡茶记</title>
              <link>https://forest.example.com/tea</link>
              <guid>post-1</guid>
              <pubDate>Fri, 27 Nov 2026 20:00:00 GMT</pubDate>
              <content:encoded><![CDATA[<p>烧一壶热水，听雨声慢慢敲打窗台。</p><p>允许今晚只读十页书。</p>]]></content:encoded>
            </item>
          </channel>
        </rss>"#;

        let feed = parse_feed(rss_xml, "https://forest.example.com/rss.xml").unwrap();
        assert_eq!(feed.title, "林间慢读通讯");
        assert_eq!(feed.items.len(), 1);
        assert_eq!(feed.items[0].title, "秋夜泡茶记");

        let atom_xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
          <title>Quiet Letters</title>
          <entry>
            <id>urn:uuid:quiet-1</id>
            <title>On Slow Mornings</title>
            <link href="https://letters.example.com/slow" rel="alternate"/>
            <author><name>Clara</name></author>
            <summary>A quiet essay about unhurried coffee and warm light on open pages.</summary>
          </entry>
        </feed>"#;
        let atom = parse_feed(atom_xml, "https://letters.example.com/atom.xml").unwrap();
        assert_eq!(atom.title, "Quiet Letters");
        assert_eq!(atom.items[0].author, "Clara");

        let single = item_to_epub(&feed.items[0], &feed.title).unwrap();
        let norm_single = crate::library::normalize_epub(&single).unwrap();
        assert_eq!(norm_single.title, "秋夜泡茶记");

        let digest = compile_weekend_digest(
            "微光周末慢读小报 · 2026-11-27",
            "2026-11-27",
            &[
                (feed.title.clone(), feed.items[0].clone()),
                (atom.title.clone(), atom.items[0].clone()),
            ],
        )
        .unwrap();
        let norm_digest = crate::library::normalize_epub(&digest).unwrap();
        assert_eq!(norm_digest.title, "微光周末慢读小报 · 2026-11-27");
    }
}

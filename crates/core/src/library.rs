use crate::{Book, Store, model::now, store::atomic_write};
use anyhow::{Context, Result, bail, ensure};
use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
    path::{Component, Path},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const MAX_ARCHIVE: u64 = 100 * 1024 * 1024;

pub fn safe_path(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['\\', ':', '\0', '%'])
        && !name.starts_with('/')
        && Path::new(name)
            .components()
            .all(|p| matches!(p, Component::Normal(_)))
}

pub fn unpack(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    ensure!(bytes.len() as u64 <= MAX_ARCHIVE, "文件超过 100 MB");
    let mut zip = ZipArchive::new(Cursor::new(bytes)).context("文件不是有效的 ZIP/EPUB")?;
    ensure!(zip.len() <= 10000, "压缩包包含过多文件");
    let mut total = 0;
    let mut files = BTreeMap::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        let name = file.name().trim_end_matches('/').to_owned();
        ensure!(safe_path(&name), "压缩包包含不安全路径");
        ensure!(
            file.unix_mode().is_none_or(|m| m & 0o170000 != 0o120000),
            "不允许符号链接"
        );
        if file.is_dir() {
            continue;
        }
        total += file.size();
        ensure!(
            total <= MAX_ARCHIVE && file.size() <= 32 * 1024 * 1024,
            "解压内容过大"
        );
        ensure!(!files.contains_key(&name), "压缩包有重复路径");
        let expected = file.size();
        let mut data = Vec::new();
        file.take(32 * 1024 * 1024 + 1).read_to_end(&mut data)?;
        ensure!(data.len() as u64 == expected, "压缩包大小异常");
        files.insert(name, data);
    }
    Ok(files)
}

pub fn pack(files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    if let Some(data) = files.get("mimetype") {
        zip.start_file(
            "mimetype",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )?;
        zip.write_all(data)?;
    }
    for (name, data) in files {
        if name == "mimetype" {
            continue;
        }
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )?;
        zip.write_all(data)?;
    }
    Ok(zip.finish()?.into_inner())
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn local_ref(s: &str) -> bool {
    !s.contains(':')
        && !s.starts_with(['/', '\\'])
        && !s.contains('\\')
        && !s.chars().any(char::is_control)
}

fn resolve_href(base_file: &str, href: &str) -> Result<String> {
    ensure!(local_ref(href), "电子书包含远程或无效资源");
    let path = href.split(['?', '#']).next().unwrap_or_default();
    let decoded = percent_decode_str(path)
        .decode_utf8()
        .context("电子书资源路径编码无效")?;
    ensure!(local_ref(&decoded), "电子书资源路径不安全");
    let mut components = base_file.split('/').map(str::to_owned).collect::<Vec<_>>();
    components.pop();
    for part in decoded.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                ensure!(components.pop().is_some(), "资源路径超出书籍范围");
            }
            _ => components.push(part.into()),
        }
    }
    let resolved = components.join("/");
    ensure!(safe_path(&resolved), "无效资源路径");
    Ok(resolved)
}

fn safe_css(css: &str) -> String {
    let mut value = css.to_owned();
    while let Some(start) = value.find("/*") {
        let Some(end) = value[start + 2..].find("*/") else {
            value.truncate(start);
            break;
        };
        value.replace_range(start..start + 2 + end + 2, "");
    }
    let lower = value.to_ascii_lowercase();
    if [
        "expression(",
        "behavior:",
        "-moz-binding",
        "javascript:",
        "\\",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
    {
        String::new()
    } else {
        let mut without_imports = String::new();
        let mut cursor = 0;
        while let Some(relative) = lower[cursor..].find("@import") {
            let start = cursor + relative;
            without_imports.push_str(&value[cursor..start]);
            let after = value[start..]
                .find(';')
                .map(|end| start + end + 1)
                .unwrap_or(value.len());
            cursor = after;
        }
        without_imports.push_str(&value[cursor..]);
        let lower = without_imports.to_ascii_lowercase();
        let mut sanitized = String::new();
        let mut cursor = 0;
        while let Some(relative) = lower[cursor..].find("url(") {
            let start = cursor + relative;
            sanitized.push_str(&without_imports[cursor..start]);
            let Some(close) = without_imports[start + 4..].find(')') else {
                cursor = without_imports.len();
                break;
            };
            let end = start + 4 + close + 1;
            let value = without_imports[start + 4..end - 1]
                .trim()
                .trim_matches(['\'', '"']);
            if local_ref(value) {
                sanitized.push_str(&without_imports[start..end]);
            } else {
                sanitized.push_str("none");
            }
            cursor = end;
        }
        sanitized.push_str(&without_imports[cursor..]);
        sanitized
    }
}

fn clean_xhtml(source: &str) -> Result<String> {
    // EPUB chapters in the wild may contain an XHTML DTD, HTML void tags, or
    // XML-namespaced attributes. Parse them as inert markup and serialize only
    // the supported, sanitized vocabulary instead of executing book content.
    use html5ever::driver::parse_document;
    use html5ever::ns;
    use html5ever::tendril::TendrilSink;
    use markup5ever_rcdom::{Handle, NodeData, RcDom};
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(source.as_bytes()))
        .map_err(|error| anyhow::anyhow!("电子书章节无法解析：{error}"))?;
    fn visit(node: Handle, out: &mut String) {
        let (tag, attrs) = match &node.data {
            NodeData::Text { contents } => {
                out.push_str(&xml(&contents.borrow()));
                return;
            }
            NodeData::Element { name, attrs, .. } => {
                (name.local.to_string(), attrs.borrow().clone())
            }
            _ => {
                for child in node.children.borrow().iter() {
                    visit(child.clone(), out);
                }
                return;
            }
        };
        if [
            "script", "iframe", "object", "embed", "form", "input", "button", "base", "meta",
            "audio", "video",
        ]
        .contains(&tag.as_str())
        {
            return;
        }
        if tag == "style" {
            out.push_str("<style>");
            let css = node
                .children
                .borrow()
                .iter()
                .filter_map(|child| match &child.data {
                    NodeData::Text { contents } => Some(contents.borrow().to_string()),
                    _ => None,
                })
                .collect::<String>();
            out.push_str(&safe_css(&css).replace('<', ""));
            out.push_str("</style>");
            return;
        }
        let allowed = [
            "html",
            "head",
            "title",
            "body",
            "section",
            "article",
            "nav",
            "main",
            "div",
            "p",
            "span",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "em",
            "strong",
            "b",
            "i",
            "u",
            "s",
            "small",
            "sup",
            "sub",
            "br",
            "hr",
            "img",
            "a",
            "link",
            "blockquote",
            "pre",
            "code",
            "ul",
            "ol",
            "li",
            "dl",
            "dt",
            "dd",
            "table",
            "thead",
            "tbody",
            "tfoot",
            "tr",
            "td",
            "th",
            "caption",
            "figure",
            "figcaption",
            "ruby",
            "rt",
            "rp",
        ];
        if !allowed.contains(&tag.as_str()) {
            for child in node.children.borrow().iter() {
                visit(child.clone(), out);
            }
            return;
        }
        out.push('<');
        out.push_str(&tag);
        if tag == "html" {
            out.push_str(" xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"");
        }
        if tag == "nav" {
            out.push_str(" epub:type=\"toc\"");
        }
        for attr in attrs {
            let name = attr.name.local.to_string();
            let value = attr.value.to_string();
            let qualified_name = attr
                .name
                .prefix
                .as_ref()
                .map(|prefix| format!("{prefix}:{name}"))
                .unwrap_or_else(|| name.clone());
            if qualified_name == "xml:lang" {
                out.push_str(&format!(" xml:lang=\"{}\"", xml(&value)));
                continue;
            }
            if name == "type" && value == "toc" {
                out.push_str(" epub:type=\"toc\"");
                continue;
            }
            if name == "style" {
                let css = safe_css(&value);
                out.push_str(&format!(" style=\"{}\"", xml(&css)));
            } else if ["href", "src"].contains(&name.as_str()) && local_ref(&value) {
                out.push_str(&format!(" {name}=\"{}\"", xml(&value)));
            } else if [
                "id", "class", "alt", "title", "lang", "dir", "rel", "type", "colspan", "rowspan",
                "width", "height",
            ]
            .contains(&name.as_str())
            {
                let qualified = if attr.name.ns == ns!(xml) {
                    format!("xml:{name}")
                } else if name == "type"
                    && attr.name.prefix == Some(html5ever::Prefix::from("epub"))
                {
                    format!("epub:{name}")
                } else {
                    name.to_owned()
                };
                out.push_str(&format!(" {qualified}=\"{}\"", xml(&value)));
            }
        }
        out.push('>');
        for child in node.children.borrow().iter() {
            visit(child.clone(), out);
        }
        out.push_str(&format!("</{tag}>"));
    }
    let mut result = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    visit(dom.document, &mut result);
    Ok(result)
}

pub struct NormalizedEpub {
    pub bytes: Vec<u8>,
    pub title: String,
    pub author: String,
    pub cover: Option<(String, Vec<u8>)>,
}
pub fn normalize_epub(bytes: &[u8]) -> Result<NormalizedEpub> {
    let mut files = unpack(bytes)?;
    ensure!(
        !files.contains_key("META-INF/encryption.xml"),
        "暂不支持加密或 DRM 电子书"
    );
    let container = std::str::from_utf8(
        files
            .get("META-INF/container.xml")
            .context("缺少 EPUB 容器信息")?,
    )?;
    let doc = roxmltree::Document::parse(container)?;
    let opf_path = doc
        .descendants()
        .find(|n| n.has_tag_name("rootfile"))
        .and_then(|n| n.attribute("full-path"))
        .context("缺少 EPUB 清单")?
        .to_owned();
    ensure!(safe_path(&opf_path), "无效的清单路径");
    let opf = std::str::from_utf8(files.get(&opf_path).context("找不到 EPUB 清单")?)?;
    let opf_doc = roxmltree::Document::parse(opf)?;
    ensure!(
        !opf_doc
            .descendants()
            .any(|n| n.attribute("property") == Some("rendition:layout")
                && n.text() == Some("pre-paginated")),
        "固定版式 EPUB 暂不支持，请使用可重排版书籍"
    );
    let title = opf_doc
        .descendants()
        .find(|n| n.tag_name().name() == "title")
        .and_then(|n| n.text())
        .unwrap_or("未命名的书")
        .to_owned();
    let author = opf_doc
        .descendants()
        .find(|n| n.tag_name().name() == "creator")
        .and_then(|n| n.text())
        .unwrap_or("作者未注明")
        .to_owned();
    let mut content_paths = Vec::new();
    let mut style_paths = Vec::new();
    for item in opf_doc.descendants().filter(|n| n.has_tag_name("item")) {
        let href = item.attribute("href").context("清单资源缺少路径")?;
        let resolved = resolve_href(&opf_path, href)?;
        match item.attribute("media-type").unwrap_or("") {
            "application/xhtml+xml" | "text/html" => content_paths.push(resolved),
            "text/css" => style_paths.push(resolved),
            _ => {}
        }
    }
    ensure!(!content_paths.is_empty(), "电子书没有可阅读章节");
    let cover_id = opf_doc
        .descendants()
        .find(|n| n.attribute("name") == Some("cover"))
        .and_then(|n| n.attribute("content"));
    let cover_href = opf_doc
        .descendants()
        .find(|n| {
            n.has_tag_name("item")
                && (n
                    .attribute("properties")
                    .is_some_and(|p| p.split_whitespace().any(|s| s == "cover-image"))
                    || cover_id.is_some_and(|id| n.attribute("id") == Some(id)))
        })
        .and_then(|n| n.attribute("href"));
    let cover = cover_href.and_then(|href| {
        let path = resolve_href(&opf_path, href).ok()?;
        let ext = Path::new(href).extension()?.to_str()?.to_lowercase();
        if !["png", "jpg", "jpeg", "webp"].contains(&ext.as_str()) {
            return None;
        }
        files.get(&path).map(|b| (ext, b.clone()))
    });
    for (name, data) in &mut files {
        let ext = Path::new(name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if content_paths.contains(name) || ["xhtml", "html", "htm"].contains(&ext.as_str()) {
            *data = clean_xhtml(std::str::from_utf8(data)?)?.into_bytes();
        } else if style_paths.contains(name) || ext == "css" {
            *data = safe_css(std::str::from_utf8(data)?).into_bytes();
        } else if ["js", "mjs", "svg"].contains(&ext.as_str()) {
            data.clear();
        }
    }
    Ok(NormalizedEpub {
        bytes: pack(&files)?,
        title,
        author,
        cover,
    })
}

pub fn text_epub(text: &str, title: &str, author: &str) -> Result<Vec<u8>> {
    let mut files = BTreeMap::new();
    files.insert("mimetype".into(), b"application/epub+zip".to_vec());
    files.insert("META-INF/container.xml".into(), br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec());
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut chapters: Vec<(String, Vec<String>)> = Vec::new();
    let mut lines = Vec::new();
    let mut label = title.to_owned();
    for line in normalized.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let heading = line.len() < 160
            && ((line.starts_with('第')
                && (line.contains('章') || line.contains('节') || line.contains('卷')))
                || line.starts_with("# "));
        if heading || lines.len() >= 180 {
            if !lines.is_empty() {
                chapters.push((label.clone(), std::mem::take(&mut lines)));
            }
            label = if heading {
                line.trim_start_matches("# ").to_owned()
            } else {
                format!("{} · {}", title, chapters.len() + 1)
            };
            if heading {
                continue;
            }
        }
        lines.push(line.to_owned());
    }
    if !lines.is_empty() {
        chapters.push((label, lines));
    }
    ensure!(!chapters.is_empty(), "文本文件没有可阅读内容");
    let mut manifest = String::new();
    let mut spine = String::new();
    let mut nav = String::new();
    for (i, (heading, paragraphs)) in chapters.iter().enumerate() {
        manifest.push_str(&format!(
            "<item id=\"c{i}\" href=\"c{i}.xhtml\" media-type=\"application/xhtml+xml\"/>"
        ));
        spine.push_str(&format!("<itemref idref=\"c{i}\"/>"));
        nav.push_str(&format!(
            "<li><a href=\"c{i}.xhtml\">{}</a></li>",
            xml(heading)
        ));
        let body = paragraphs
            .iter()
            .enumerate()
            .map(|(n, p)| format!("<p id=\"p{n}\">{}</p>", xml(p)))
            .collect::<String>();
        files.insert(format!("c{i}.xhtml"), format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>{}</title></head><body><h1>{}</h1>{body}</body></html>", xml(heading), xml(heading)).into_bytes());
    }
    let id = format!("{:x}", Sha256::digest(text.as_bytes()));
    files.insert("book.opf".into(), format!("<?xml version=\"1.0\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"id\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"id\">{id}</dc:identifier><dc:title>{}</dc:title><dc:creator>{}</dc:creator><dc:language>zh-CN</dc:language><meta property=\"dcterms:modified\">2026-01-01T00:00:00Z</meta></metadata><manifest><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>{manifest}</manifest><spine>{spine}</spine></package>",xml(title),xml(author)).into_bytes());
    files.insert("nav.xhtml".into(), format!("<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><title>目录</title></head><body><nav epub:type=\"toc\"><ol>{nav}</ol></nav></body></html>").into_bytes());
    pack(&files)
}

pub struct PreparedBook {
    id: String,
    epub: Vec<u8>,
    title: String,
    author: String,
    ext: String,
    cover: Option<(String, Vec<u8>)>,
    selected_encoding: Option<String>,
    original: Option<Vec<u8>>,
}

pub fn prepare_book(path: &Path, encoding: Option<&str>) -> Result<PreparedBook> {
    ensure!(fs::metadata(path)?.len() <= MAX_ARCHIVE, "文件超过 100 MB");
    let bytes = fs::read(path)?;
    let id = format!("{:x}", Sha256::digest(&bytes));
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mut selected_encoding = None;
    let NormalizedEpub {
        bytes: epub,
        title,
        author,
        cover,
    } = match ext.as_str() {
        "epub" => normalize_epub(&bytes)?,
        "txt" => {
            ensure!(bytes.len() <= 20 * 1024 * 1024, "TXT 文件超过 20 MB");
            let use_utf8 = encoding == Some("UTF-8")
                || (encoding.is_none() && std::str::from_utf8(&bytes).is_ok());
            let text = if use_utf8 {
                match std::str::from_utf8(&bytes) {
                    Ok(text) => text.to_owned(),
                    Err(_) if encoding == Some("UTF-8") => {
                        String::from_utf8_lossy(&bytes).into_owned()
                    }
                    Err(error) => return Err(error).context("不是有效 UTF-8，请选择 GB18030"),
                }
            } else {
                let (text, _, errors) = encoding_rs::GB18030.decode(&bytes);
                ensure!(!errors, "无法识别编码，请尝试 UTF-8");
                text.into_owned()
            };
            selected_encoding = Some(if use_utf8 { "UTF-8" } else { "GB18030" }.into());
            let title = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("未命名文本")
                .to_owned();
            NormalizedEpub {
                bytes: text_epub(text.trim_start_matches('\u{feff}'), &title, "本地文本")?,
                title,
                author: "本地文本".into(),
                cover: None,
            }
        }
        _ => bail!("目前支持 EPUB 和 TXT；PDF、MOBI、AZW3 将在后续加入"),
    };
    let original = if ext == "txt" { Some(bytes) } else { None };
    Ok(PreparedBook {
        id,
        epub,
        title,
        author,
        ext,
        cover,
        selected_encoding,
        original,
    })
}

pub fn prepare_epub_bytes(bytes: Vec<u8>) -> Result<PreparedBook> {
    ensure!(bytes.len() as u64 <= MAX_ARCHIVE, "文件超过 100 MB");
    let id = format!("{:x}", Sha256::digest(&bytes));
    let NormalizedEpub {
        bytes: epub,
        title,
        author,
        cover,
    } = normalize_epub(&bytes)?;
    Ok(PreparedBook {
        id,
        epub,
        title,
        author,
        ext: "epub".into(),
        cover,
        selected_encoding: None,
        original: None,
    })
}

impl Store {
    pub fn import_book(&mut self, path: &Path, encoding: Option<&str>) -> Result<String> {
        self.commit_book(prepare_book(path, encoding)?, encoding.is_some())
    }
    pub fn import_epub_bytes(&mut self, bytes: Vec<u8>) -> Result<String> {
        self.commit_book(prepare_epub_bytes(bytes)?, false)
    }
    pub fn commit_book(&mut self, prepared: PreparedBook, replace: bool) -> Result<String> {
        self.commit_book_as(prepared, replace.then_some(String::new()).as_deref())
    }

    pub fn commit_book_as(
        &mut self,
        prepared: PreparedBook,
        replace_id: Option<&str>,
    ) -> Result<String> {
        let PreparedBook {
            id,
            epub,
            title,
            author,
            ext,
            cover,
            selected_encoding,
            original,
        } = prepared;
        if replace_id.is_none() && self.state.books.iter().any(|b| b.id == id) {
            return Ok(id);
        }
        let target_id = replace_id
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .unwrap_or(id.clone());
        let previous = self
            .state
            .books
            .iter()
            .find(|book| book.id == target_id)
            .cloned();
        let dir = self.root.join("books").join(&target_id);
        fs::create_dir_all(&dir)?;
        atomic_write(&dir.join("book.epub"), &epub)?;
        if let Some(bytes) = original {
            atomic_write(&dir.join("original.txt"), &bytes)?;
        }
        let cover = if let Some((ext, data)) = cover {
            let name = format!("cover.{ext}");
            atomic_write(&dir.join(&name), &data)?;
            Some(name)
        } else {
            None
        };
        self.state.books.retain(|b| b.id != target_id);
        self.state.books.push(Book {
            id: target_id.clone(),
            title,
            author,
            format: ext,
            imported_at: now(),
            cover,
            location: if replace_id.is_some() {
                previous.as_ref().and_then(|book| book.location.clone())
            } else {
                None
            },
            bookmarks: if replace_id.is_some() {
                previous
                    .as_ref()
                    .map(|book| book.bookmarks.clone())
                    .unwrap_or_default()
            } else {
                vec![]
            },
            highlights: if replace_id.is_some() {
                previous
                    .as_ref()
                    .map(|book| book.highlights.clone())
                    .unwrap_or_default()
            } else {
                vec![]
            },
            progress: previous.as_ref().and_then(|book| book.progress),
            chapter: previous.as_ref().and_then(|book| book.chapter.clone()),
            encoding: selected_encoding,
            updated_at: Some(now()),
            companion: previous.as_ref().and_then(|book| book.companion.clone()),
            shelf_status: previous.as_ref().and_then(|book| book.shelf_status.clone()),
            tags: previous
                .as_ref()
                .map(|book| book.tags.clone())
                .unwrap_or_default(),
            finished_at: previous.as_ref().and_then(|book| book.finished_at.clone()),
            finish_letter: previous
                .as_ref()
                .and_then(|book| book.finish_letter.clone()),
            completion_ticket_no: previous.as_ref().and_then(|book| book.completion_ticket_no),
            total_reading_minutes: previous
                .as_ref()
                .map(|book| book.total_reading_minutes)
                .unwrap_or(0),
        });
        self.state
            .deleted_book_ids
            .retain(|deleted| deleted != &target_id);
        self.save()?;
        Ok(target_id)
    }

    pub fn install_sample(&mut self) -> Result<()> {
        if self.root.join("sample-installed").exists() {
            return Ok(());
        }
        let text = include_str!("../../../assets/sample.txt");
        let bytes = text_epub(text, "在林间，慢慢来", "微光阅读 · 原创示例")?;
        let path = self.root.join("sample.epub");
        atomic_write(&path, &bytes)?;
        self.import_book(&path, None)?;
        atomic_write(&self.root.join("sample-installed"), b"1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bookmark, Location};
    #[test]
    fn text_roundtrip_is_stable_and_escaped() {
        let a = text_epub(
            "# 开始\n孤独也可以被看见。\n<&>\n# 明天\n慢慢来",
            "一本书",
            "作者",
        )
        .unwrap();
        assert_eq!(
            a,
            text_epub(
                "# 开始\n孤独也可以被看见。\n<&>\n# 明天\n慢慢来",
                "一本书",
                "作者"
            )
            .unwrap()
        );
        let NormalizedEpub { title, .. } = normalize_epub(&a).unwrap();
        assert_eq!(title, "一本书");
        assert!(text_epub("   ", "空", "作者").is_err());
    }
    #[test]
    fn accepts_real_world_xhtml_dtd_and_namespaces() {
        let mut files = unpack(&text_epub("占位", "测试", "作者").unwrap()).unwrap();
        let chapter = r#"<?xml version='1.0' encoding='utf-8'?>
            <!DOCTYPE html PUBLIC '-//W3C//DTD XHTML 1.1//EN'
            'http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd'>
            <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="en">
            <head><meta content="text/css" http-equiv="Content-Style-Type"/><title>Chapter</title></head>
            <body><p xml:lang="en">Alice’s Adventures</p><br><img src="cover.jpg" alt="Cover"></body></html>"#;
        let opf = String::from_utf8(files["book.opf"].clone())
            .unwrap()
            .replace("c0.xhtml", "chapter.xml");
        files.insert("book.opf".into(), opf.into_bytes());
        files.remove("c0.xhtml");
        files.insert("chapter.xml".into(), chapter.into());
        let normalized = normalize_epub(&pack(&files).unwrap()).unwrap();
        let cleaned = unpack(&normalized.bytes).unwrap();
        let output = String::from_utf8(cleaned["chapter.xml"].clone()).unwrap();
        assert!(output.contains("Alice’s Adventures"));
        assert!(output.contains("xml:lang=\"en\""));
        assert!(!output.contains("DOCTYPE"));
    }
    #[test]
    fn generated_navigation_keeps_epub_toc_semantics() {
        let epub = text_epub("# 第一章\n一段话", "目录测试", "作者").unwrap();
        let normalized = normalize_epub(&epub).unwrap();
        let files = unpack(&normalized.bytes).unwrap();
        let nav = String::from_utf8(files["nav.xhtml"].clone()).unwrap();
        assert!(nav.contains("epub:type=\"toc\""), "{nav}");
    }
    #[test]
    fn resolves_percent_encoded_manifest_hrefs_safely() {
        assert_eq!(
            resolve_href("OPS/book.opf", "chapter%20one.xhtml#part").unwrap(),
            "OPS/chapter one.xhtml"
        );
        assert_eq!(
            resolve_href("OPS/book.opf", "%E7%AC%AC%E4%B8%80%E7%AB%A0.xhtml").unwrap(),
            "OPS/第一章.xhtml"
        );
        assert!(resolve_href("OPS/book.opf", "https%3A%2F%2Fexample.invalid/x").is_err());
        assert!(resolve_href("OPS/book.opf", "%2E%2E/%2E%2E/x").is_err());
    }
    #[test]
    fn rejects_paths_and_active_content() {
        for bad in ["../x", "/x", "C:/x", "a\\b", "a/%2e/x", "a/../../x"] {
            assert!(!safe_path(bad));
        }
        assert!(safe_path("OPS/chapter.xhtml"));
        let result = clean_xhtml(r#"<html><body onload="evil()"><script>evil()</script><a href="javascript:evil()">字</a><img src="https://evil/a"/><p>阅读</p></body></html>"#).unwrap();
        assert!(!result.contains("evil"));
        assert!(result.contains("阅读"));
        assert!(safe_css("@import 'https://evil';").is_empty());
        let css = safe_css(
            "/* layout */ @import url('https://evil/x.css'); p{color:#333} img{background:url(../images/a.png)} .x{background:url(https://evil/a.png)}",
        );
        assert!(css.contains("color:#333"));
        assert!(css.contains("url(../images/a.png)"));
        assert!(css.contains("background:none"));
        assert!(!css.contains("evil"));
    }
    #[test]
    fn duplicate_import_and_gb18030() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("中文.txt");
        let (encoded, _, _) = encoding_rs::GB18030.encode("# 第一章\n我在这里等你。");
        fs::write(&path, encoded).unwrap();
        let mut store = Store::open(dir.path().join("data")).unwrap();
        let id = store.import_book(&path, None).unwrap();
        assert_eq!(id, store.import_book(&path, None).unwrap());
        assert_eq!(store.state.books.len(), 1);
        assert_eq!(store.state.books[0].encoding.as_deref(), Some("GB18030"));
        store.remove_book(&id).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn changing_encoding_keeps_book_identity_and_bookmarks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.txt");
        let (encoded, _, _) = encoding_rs::GB18030.encode("# 第一章\n我在这里等你。");
        fs::write(&path, encoded).unwrap();
        let mut store = Store::open(dir.path().join("data")).unwrap();
        let id = store.import_book(&path, None).unwrap();
        let location = Location {
            book_id: id.clone(),
            cfi: "epubcfi(/6/2!/4/2/1:0)".into(),
        };
        store.state.books[0].location = Some(location.clone());
        store.state.books[0].bookmarks.push(Bookmark {
            label: "一页".into(),
            location,
        });
        let prepared = prepare_book(&path, Some("UTF-8")).unwrap();
        let changed = store.commit_book_as(prepared, Some(&id)).unwrap();
        assert_eq!(changed, id);
        assert_eq!(store.state.books.len(), 1);
        assert_eq!(store.state.books[0].encoding.as_deref(), Some("UTF-8"));
        assert_eq!(store.state.books[0].bookmarks.len(), 1);
        assert!(
            store
                .root
                .join("books")
                .join(&id)
                .join("book.epub")
                .exists()
        );
    }

    #[test]
    fn rejects_drm_fixed_layout_and_remote_manifest() {
        let base = text_epub("一段文字", "测试", "作者").unwrap();
        let mut files = unpack(&base).unwrap();
        files.insert("META-INF/encryption.xml".into(), b"<encryption/>".to_vec());
        assert!(normalize_epub(&pack(&files).unwrap()).is_err());
        files.remove("META-INF/encryption.xml");
        let original = String::from_utf8(files["book.opf"].clone()).unwrap();
        files.insert(
            "book.opf".into(),
            original
                .replace(
                    "</metadata>",
                    "<meta property=\"rendition:layout\">pre-paginated</meta></metadata>",
                )
                .into_bytes(),
        );
        assert!(normalize_epub(&pack(&files).unwrap()).is_err());
        files.insert(
            "book.opf".into(),
            original
                .replace(
                    "href=\"c0.xhtml\"",
                    "href=\"https://example.invalid/chapter.xhtml\"",
                )
                .into_bytes(),
        );
        assert!(normalize_epub(&pack(&files).unwrap()).is_err());
    }

    #[test]
    fn media_type_drives_sanitization_not_just_file_extension() {
        let mut files = unpack(&text_epub("一段文字", "测试", "作者").unwrap()).unwrap();
        let opf = String::from_utf8(files["book.opf"].clone())
            .unwrap()
            .replace("c0.xhtml", "chapter.xml");
        files.insert("book.opf".into(), opf.into_bytes());
        files.remove("c0.xhtml");
        files.insert("chapter.xml".into(),br#"<html><head><script>alert(1)</script></head><body onclick="alert(1)"><p>safe</p></body></html>"#.to_vec());
        let cleaned = normalize_epub(&pack(&files).unwrap()).unwrap();
        let parsed = unpack(&cleaned.bytes).unwrap();
        let chapter = String::from_utf8(parsed["chapter.xml"].clone()).unwrap();
        assert!(!chapter.contains("script"));
        assert!(!chapter.contains("onclick"));
        assert!(chapter.contains("safe"));
    }

    #[test]
    fn traversal_archives_are_rejected_before_writing() {
        let mut files = BTreeMap::new();
        files.insert("../outside.txt".into(), b"escape".to_vec());
        assert!(unpack(&pack(&files).unwrap()).is_err());
    }
}

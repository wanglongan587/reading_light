use crate::{
    library::{safe_path, unpack},
    model::{BookCompanionCache, CharacterNode, CharacterRelation, now},
};
use anyhow::{Context, Result, ensure};
use percent_encoding::percent_decode_str;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone)]
pub struct BookChapter {
    pub index: usize,
    pub title: String,
    pub href: String,
    pub text: String,
    pub paragraphs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SpoilerSafeContext {
    pub book_title: String,
    pub book_author: String,
    pub cutoff_chapter_index: usize,
    pub total_chapters: usize,
    pub cutoff_chapter_title: String,
    pub progress_percent: u8,
    pub skipped_future_chapters: usize,
    pub excerpt: String,
    pub recent_tail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtsSegment {
    pub chapter_index: usize,
    pub chapter_title: String,
    pub text: String,
}

fn resolve_epub_href(base_file: &str, href: &str) -> Option<String> {
    if href.contains(':') || href.starts_with(['/', '\\']) || href.contains('\\') {
        return None;
    }
    let path = href.split(['?', '#']).next().unwrap_or_default();
    let decoded = percent_decode_str(path).decode_utf8().ok()?;
    let mut components = base_file.split('/').map(str::to_owned).collect::<Vec<_>>();
    components.pop();
    for part in decoded.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                components.pop()?;
            }
            _ => components.push(part.into()),
        }
    }
    let resolved = components.join("/");
    safe_path(&resolved).then_some(resolved)
}

fn extract_paragraphs_from_xhtml(source: &str) -> (Option<String>, Vec<String>) {
    use html5ever::driver::parse_document;
    use html5ever::tendril::TendrilSink;
    use markup5ever_rcdom::{Handle, NodeData, RcDom};

    let Ok(dom) = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(source.as_bytes()))
    else {
        return (None, vec![]);
    };

    fn collect_text(node: &Handle, out: &mut String) {
        match &node.data {
            NodeData::Text { contents } => out.push_str(&contents.borrow()),
            NodeData::Element { name, .. } => {
                let tag = name.local.as_ref();
                if matches!(tag, "script" | "style" | "head" | "rt" | "rp") {
                    return;
                }
                if tag == "br" {
                    out.push('\n');
                    return;
                }
                for child in node.children.borrow().iter() {
                    collect_text(child, out);
                }
            }
            _ => {
                for child in node.children.borrow().iter() {
                    collect_text(child, out);
                }
            }
        }
    }

    fn walk(node: &Handle, heading: &mut Option<String>, paragraphs: &mut Vec<String>) {
        if let NodeData::Element { name, .. } = &node.data {
            let tag = name.local.as_ref();
            if matches!(tag, "script" | "style" | "head" | "nav") {
                return;
            }
            if matches!(tag, "h1" | "h2" | "h3") {
                let mut text = String::new();
                collect_text(node, &mut text);
                let clean = normalize_ws(&text);
                if !clean.is_empty() {
                    if heading.is_none() {
                        *heading = Some(clean.clone());
                    }
                    paragraphs.push(clean);
                }
                return;
            }
            if matches!(tag, "p" | "li" | "blockquote" | "dd" | "dt") {
                let mut text = String::new();
                collect_text(node, &mut text);
                let clean = normalize_ws(&text);
                if !clean.is_empty() {
                    paragraphs.push(clean);
                }
                return;
            }
        }
        for child in node.children.borrow().iter() {
            walk(child, heading, paragraphs);
        }
    }

    let mut heading = None;
    let mut paragraphs = Vec::new();
    walk(&dom.document, &mut heading, &mut paragraphs);

    // Fallback for EPUBs that place text directly inside <div> / <body> without <p>
    if paragraphs.is_empty() {
        let mut all = String::new();
        collect_text(&dom.document, &mut all);
        for line in all.lines() {
            let clean = normalize_ws(line);
            if !clean.is_empty() {
                paragraphs.push(clean);
            }
        }
    }
    (heading, paragraphs)
}

fn normalize_ws(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() || ch == '\u{3000}' {
            if !prev_space && !out.is_empty() {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_owned()
}

fn parse_nav_labels(
    files: &BTreeMap<String, Vec<u8>>,
    opf_path: &str,
    opf_doc: &roxmltree::Document,
) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    let nav_href = opf_doc
        .descendants()
        .find(|n| {
            n.has_tag_name("item")
                && n.attribute("properties")
                    .is_some_and(|p| p.split_whitespace().any(|tok| tok == "nav"))
        })
        .and_then(|n| n.attribute("href"));
    if let Some(href) = nav_href
        && let Some(nav_path) = resolve_epub_href(opf_path, href)
        && let Some(bytes) = files.get(&nav_path)
        && let Ok(text) = std::str::from_utf8(bytes)
        && let Ok(doc) = roxmltree::Document::parse(text)
    {
        for a in doc.descendants().filter(|n| n.has_tag_name("a")) {
            if let Some(target) = a.attribute("href")
                && let Some(resolved) = resolve_epub_href(&nav_path, target)
            {
                let label = a
                    .descendants()
                    .filter(|n| n.is_text())
                    .filter_map(|n| n.text())
                    .collect::<String>();
                let clean = normalize_ws(&label);
                if !clean.is_empty() {
                    labels.entry(resolved).or_insert(clean);
                }
            }
        }
    }
    labels
}

/// Extract readable chapters in exact EPUB OPF `<spine>` order.
pub fn extract_epub_chapters(epub_bytes: &[u8]) -> Result<Vec<BookChapter>> {
    let files = unpack(epub_bytes)?;
    let container = std::str::from_utf8(
        files
            .get("META-INF/container.xml")
            .context("缺少 EPUB 容器信息")?,
    )?;
    let container_doc = roxmltree::Document::parse(container)?;
    let opf_path = container_doc
        .descendants()
        .find(|n| n.has_tag_name("rootfile"))
        .and_then(|n| n.attribute("full-path"))
        .context("缺少 EPUB 清单")?
        .to_owned();
    let opf = std::str::from_utf8(files.get(&opf_path).context("找不到 EPUB 清单")?)?;
    let opf_doc = roxmltree::Document::parse(opf)?;

    let nav_labels = parse_nav_labels(&files, &opf_path, &opf_doc);
    let mut manifest = HashMap::<String, (String, String)>::new();
    for item in opf_doc.descendants().filter(|n| n.has_tag_name("item")) {
        if let (Some(id), Some(href)) = (item.attribute("id"), item.attribute("href")) {
            let media_type = item.attribute("media-type").unwrap_or("").to_owned();
            if let Some(resolved) = resolve_epub_href(&opf_path, href) {
                manifest.insert(id.to_owned(), (resolved, media_type));
            }
        }
    }

    let mut chapters = Vec::new();
    for itemref in opf_doc.descendants().filter(|n| n.has_tag_name("itemref")) {
        let Some(idref) = itemref.attribute("idref") else {
            continue;
        };
        let Some((path, media_type)) = manifest.get(idref) else {
            continue;
        };
        if !media_type.contains("html") && !path.ends_with(".xhtml") && !path.ends_with(".html") {
            continue;
        }
        let Some(bytes) = files.get(path) else {
            continue;
        };
        let Ok(xhtml) = std::str::from_utf8(bytes) else {
            continue;
        };
        let (heading, paragraphs) = extract_paragraphs_from_xhtml(xhtml);
        if paragraphs.is_empty() {
            continue;
        }
        let index = chapters.len();
        let title = nav_labels
            .get(path)
            .cloned()
            .or(heading)
            .unwrap_or_else(|| format!("第 {} 节", index + 1));
        let text = paragraphs.join("\n");
        chapters.push(BookChapter {
            index,
            title,
            href: path.clone(),
            text,
            paragraphs,
        });
    }
    ensure!(!chapters.is_empty(), "未能从本书提取可阅读章节内容");
    Ok(chapters)
}

/// Parse the 0-based spine index from an EPUB CFI such as `epubcfi(/6/8!/4/2/1:0)`.
pub fn spine_index_from_cfi(cfi: &str) -> Option<usize> {
    let inner = cfi.strip_prefix("epubcfi(")?.strip_suffix(')')?;
    let base = inner.split('!').next()?;
    let steps: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    if steps.len() < 2 {
        return None;
    }
    let step_num: usize = steps[1]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()?;
    if step_num >= 2 && step_num % 2 == 0 {
        Some((step_num / 2).saturating_sub(1))
    } else {
        None
    }
}

/// Determine the highest chapter index & approximate character cutoff that the user has read,
/// strictly excluding all unread future chapters so AI can NEVER spoil the plot.
fn resolve_read_boundary(
    chapters: &[BookChapter],
    current_cfi: Option<&str>,
    current_chapter: Option<&str>,
    progress: Option<f32>,
) -> (usize, usize, u8) {
    let total_chars: usize = chapters.iter().map(|c| c.text.chars().count()).sum();
    let total_chars = total_chars.max(1);

    // 1) Try chapter title match first (most accurate for nav-labeled chapters)
    let matched_by_title = current_chapter
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .and_then(|target| {
            chapters.iter().position(|c| {
                c.title == target || c.title.contains(target) || target.contains(&c.title)
            })
        });

    // 2) Try CFI spine index
    let matched_by_cfi = current_cfi
        .and_then(spine_index_from_cfi)
        .filter(|&idx| idx < chapters.len());

    // 3) Progress fallback
    let clamped_progress = progress.map(|p| p.clamp(0.0, 1.0));
    let matched_by_progress = clamped_progress.map(|p| {
        let target_chars = ((total_chars as f64) * (p as f64)).round() as usize;
        let mut acc = 0usize;
        for (i, ch) in chapters.iter().enumerate() {
            let len = ch.text.chars().count();
            if acc + len >= target_chars || i + 1 == chapters.len() {
                let inside = target_chars.saturating_sub(acc).max(800);
                return (i, inside.min(len));
            }
            acc += len;
        }
        (0, chapters[0].text.chars().count().min(3000))
    });

    let (cutoff_idx, cutoff_chars_in_chapter) =
        if let Some(idx) = matched_by_title.or(matched_by_cfi) {
            let ch_len = chapters[idx].text.chars().count();
            // If progress also exists and maps inside this exact chapter, honor sub-chapter cutoff!
            let within = if let Some((p_idx, p_chars)) = matched_by_progress
                && p_idx == idx
            {
                p_chars.max(1200).min(ch_len)
            } else {
                ch_len
            };
            (idx, within)
        } else if let Some((p_idx, p_chars)) = matched_by_progress {
            (p_idx, p_chars)
        } else {
            // Brand-new book at 0%: only read up to the first 3500 chars of chapter 0
            (0, chapters[0].text.chars().count().min(3500))
        };

    let read_chars: usize = chapters[..cutoff_idx]
        .iter()
        .map(|c| c.text.chars().count())
        .sum::<usize>()
        + cutoff_chars_in_chapter;
    let computed_pct = ((read_chars as f64 / total_chars as f64) * 100.0).round() as u8;
    let progress_percent = clamped_progress
        .map(|p| (p * 100.0).round() as u8)
        .unwrap_or(computed_pct)
        .clamp(1, 100);

    (cutoff_idx, cutoff_chars_in_chapter, progress_percent)
}

/// Build a strictly spoiler-free context window containing ONLY chapters/text up to the user's
/// current reading location. Future chapters are completely excluded.
pub fn build_spoiler_safe_context(
    epub_bytes: &[u8],
    book_title: &str,
    book_author: &str,
    current_cfi: Option<&str>,
    current_chapter: Option<&str>,
    progress: Option<f32>,
) -> Result<SpoilerSafeContext> {
    let chapters = extract_epub_chapters(epub_bytes)?;
    let (cutoff_idx, cutoff_chars, progress_percent) =
        resolve_read_boundary(&chapters, current_cfi, current_chapter, progress);

    let read_chapters = &chapters[..=cutoff_idx];
    let skipped_future_chapters = chapters.len().saturating_sub(cutoff_idx + 1);
    let cutoff_chapter_title = chapters[cutoff_idx].title.clone();

    // Budget up to ~12,000 CJK/Latin characters:
    // - Prioritize full recent tail of the current chapter (up to 4,500 chars)
    // - Distribute remaining ~7,500 chars across earlier read chapters evenly
    let current_text: String = chapters[cutoff_idx]
        .text
        .chars()
        .take(cutoff_chars)
        .collect();
    let recent_tail: String = {
        let total = current_text.chars().count();
        let start = total.saturating_sub(1200);
        current_text.chars().skip(start).collect()
    };

    let mut sections = Vec::new();
    let earlier = &read_chapters[..cutoff_idx];
    if !earlier.is_empty() {
        let per_chapter_budget = (7200 / earlier.len()).clamp(280, 2200);
        for ch in earlier {
            let len = ch.text.chars().count();
            let snippet = if len <= per_chapter_budget {
                ch.text.clone()
            } else {
                let head: String = ch.text.chars().take(per_chapter_budget * 2 / 3).collect();
                let tail_take = per_chapter_budget / 3;
                let tail: String = ch
                    .text
                    .chars()
                    .skip(len.saturating_sub(tail_take))
                    .collect();
                format!("{head}\n…\n{tail}")
            };
            sections.push(format!("【{}】\n{}", ch.title, snippet));
        }
    }

    let cur_budget = 4800usize;
    let cur_len = current_text.chars().count();
    let cur_snippet = if cur_len <= cur_budget {
        current_text
    } else {
        let head: String = current_text.chars().take(1200).collect();
        let tail: String = current_text
            .chars()
            .skip(cur_len.saturating_sub(cur_budget - 1200))
            .collect();
        format!("{head}\n…\n{tail}")
    };
    sections.push(format!(
        "【当前读至：{}】\n{}",
        cutoff_chapter_title, cur_snippet
    ));

    Ok(SpoilerSafeContext {
        book_title: book_title.to_owned(),
        book_author: book_author.to_owned(),
        cutoff_chapter_index: cutoff_idx,
        total_chapters: chapters.len(),
        cutoff_chapter_title,
        progress_percent,
        skipped_future_chapters,
        excerpt: sections.join("\n\n"),
        recent_tail,
    })
}

/// Split chapter text into crisp speakable segments (~80-220 chars each) starting at the user's
/// current chapter so audiobook narration starts speaking quickly and smoothly.
pub fn extract_tts_segments(
    epub_bytes: &[u8],
    current_cfi: Option<&str>,
    current_chapter: Option<&str>,
    progress: Option<f32>,
) -> Result<(Vec<TtsSegment>, usize)> {
    let chapters = extract_epub_chapters(epub_bytes)?;
    let (start_chapter_idx, _, _) =
        resolve_read_boundary(&chapters, current_cfi, current_chapter, progress);

    let mut segments = Vec::new();
    let mut start_segment_idx = 0usize;

    for (ci, ch) in chapters.iter().enumerate() {
        if ci == start_chapter_idx {
            start_segment_idx = segments.len();
        }
        for para in &ch.paragraphs {
            for piece in split_into_tts_chunks(para, 200) {
                if piece.chars().count() >= 2 {
                    segments.push(TtsSegment {
                        chapter_index: ci,
                        chapter_title: ch.title.clone(),
                        text: piece,
                    });
                }
            }
        }
    }
    ensure!(!segments.is_empty(), "本书没有可供朗读的正文段落");
    let start_idx = start_segment_idx.min(segments.len() - 1);
    Ok((segments, start_idx))
}

fn split_into_tts_chunks(paragraph: &str, max_chars: usize) -> Vec<String> {
    let clean = paragraph.trim();
    if clean.is_empty() {
        return vec![];
    }
    if clean.chars().count() <= max_chars {
        return vec![clean.to_owned()];
    }
    let mut chunks = Vec::new();
    let mut buf = String::new();
    let mut count = 0usize;
    for ch in clean.chars() {
        buf.push(ch);
        count += 1;
        let is_sentence_end = matches!(ch, '。' | '！' | '？' | '；' | '.' | '!' | '?' | ';');
        let is_soft_pause = matches!(ch, '，' | ',' | '：' | ':' | '、');
        if (is_sentence_end && count >= 40)
            || (is_soft_pause && count >= max_chars.saturating_sub(30))
            || count >= max_chars
        {
            let trimmed = buf.trim();
            if !trimmed.is_empty() {
                chunks.push(trimmed.to_owned());
            }
            buf.clear();
            count = 0;
        }
    }
    let rest = buf.trim();
    if !rest.is_empty() {
        if rest.chars().count() < 16
            && let Some(last) = chunks.last_mut()
        {
            last.push_str(rest);
        } else {
            chunks.push(rest.to_owned());
        }
    }
    chunks
}

/// Offline zero-API-key preview builder so users can immediately inspect the Spoiler-Free Recap
/// & Character Graph experience before configuring DeepSeek / Qwen / OpenAI / Ollama.
pub fn offline_companion_preview(ctx: &SpoilerSafeContext) -> BookCompanionCache {
    let recent_lines: Vec<&str> = ctx
        .recent_tail
        .lines()
        .map(str::trim)
        .filter(|l| l.chars().count() >= 10)
        .collect();
    let resume_moment = recent_lines
        .last()
        .copied()
        .unwrap_or("正停留在当前章节的字里行间。")
        .chars()
        .take(110)
        .collect::<String>();

    let summary = format!(
        "当前阅读至《{}》的「{}」（约 {}%），系统已严格屏蔽后续 {} 个未读章节以防剧透。已为您提取本段之前的正文脉络；在「主题与设置」配置 DeepSeek、通义千问、OpenAI 或本地 Ollama 大模型后，即可一键生成完整文学级前情提要与精准人物关系图谱。",
        ctx.book_title, ctx.cutoff_chapter_title, ctx.progress_percent, ctx.skipped_future_chapters
    );

    // Light local proper-noun / frequent CJK/Latin name heuristic
    let candidates = extract_candidate_names(&ctx.excerpt);
    let characters: Vec<CharacterNode> = candidates
        .into_iter()
        .take(6)
        .enumerate()
        .map(|(idx, (name, count))| CharacterNode {
            name,
            aliases: vec![],
            role: if idx == 0 {
                "已读高频核心线索".into()
            } else {
                format!("已读提及 {count} 次")
            },
            traits: vec!["本地快速识别".into()],
            status: format!("活跃于「{}」之前", ctx.cutoff_chapter_title),
            first_seen: ctx.cutoff_chapter_title.clone(),
        })
        .collect();

    let mut relations = Vec::new();
    for pair in characters.windows(2) {
        relations.push(CharacterRelation {
            from: pair[0].name.clone(),
            to: pair[1].name.clone(),
            label: "同章共现（配置 AI 解析深层关系）".into(),
            tone: "mystery".into(),
        });
    }

    BookCompanionCache {
        generated_at: now(),
        progress_percent: ctx.progress_percent,
        cutoff_chapter: ctx.cutoff_chapter_title.clone(),
        skipped_future_chapters: ctx.skipped_future_chapters,
        is_ai: false,
        headline: format!("读至 {}% · 防剧透本地脉络预览", ctx.progress_percent),
        story_summary: summary,
        open_threads: vec![
            format!(
                "当前章节「{}」正在展开的伏笔与情绪转折",
                ctx.cutoff_chapter_title
            ),
            "配置任意云端或本地离线大模型后，将自动提炼至今未解悬念".into(),
        ],
        resume_moment,
        characters,
        relations,
    }
}

fn extract_candidate_names(text: &str) -> Vec<(String, usize)> {
    const STOPWORDS: &[&str] = &[
        "一个", "我们", "他们", "她们", "自己", "什么", "没有", "已经", "开始", "时候", "因为",
        "所以", "但是", "如果", "这样", "那样", "可以", "不是", "知道", "觉得", "看着", "说道",
        "起来", "下来", "出来", "进来", "这个", "那个", "这些", "那些", "声音", "时间", "地方",
        "世界", "生活", "故事", "章节", "当前", "读至", "现在", "一直", "还是", "或者", "仿佛",
    ];
    let mut counts: HashMap<String, usize> = HashMap::new();

    // 1. Detect transliterated names with middle dot (e.g. 布恩迪亚 / 阿尔卡蒂奥 / 乌尔苏拉 / 霍塞·阿尔卡蒂奥)
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '·' && i >= 2 && i + 2 < chars.len() {
            let mut start = i;
            while start > 0 && is_cjk(chars[start - 1]) && i - start < 4 {
                start -= 1;
            }
            let mut end = i + 1;
            while end < chars.len() && is_cjk(chars[end]) && end - i <= 5 {
                end += 1;
            }
            if end - start >= 3 {
                let token: String = chars[start..end].iter().collect();
                *counts.entry(token).or_insert(0) += 3;
                i = end;
                continue;
            }
        }
        i += 1;
    }

    // 2. Detect Capitalized Latin proper names (e.g., Gatsby, Aureliano)
    for word in text.split(|c: char| !c.is_ascii_alphabetic()) {
        if word.len() >= 3
            && word.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && word.chars().skip(1).any(|c| c.is_ascii_lowercase())
            && !matches!(
                word,
                "The" | "And" | "But" | "Chapter" | "Part" | "This" | "That"
            )
        {
            *counts.entry(word.to_owned()).or_insert(0) += 2;
        }
    }

    // 3. Detect 2-3 char CJK tokens appearing right before speech verbs (说/问/笑/想/看)
    for (idx, &ch) in chars.iter().enumerate() {
        if matches!(ch, '说' | '问' | '笑' | '想' | '道' | '喊') && idx >= 2 {
            for len in [3usize, 2] {
                if idx >= len && chars[idx - len..idx].iter().all(|&c| is_cjk(c)) {
                    let token: String = chars[idx - len..idx].iter().collect();
                    if !STOPWORDS.contains(&token.as_str()) {
                        *counts.entry(token).or_insert(0) += 2;
                        break;
                    }
                }
            }
        }
    }

    let mut ranked: Vec<(String, usize)> = counts.into_iter().filter(|(_, c)| *c >= 2).collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

fn is_cjk(c: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::text_epub;

    #[test]
    fn spoiler_guardrail_excludes_future_chapters() {
        let epub = text_epub(
            "# 第一章 启程\n少年林远在雨夜离开小镇，遇见了守灯人苏晚。\n苏晚说：“这盏灯不能熄灭。”\n\n# 第二章 迷雾\n林远与苏晚乘上浅海电车，发现了失踪日记。\n\n# 第三章 大结局\n最终反派其实是镇长，所有秘密彻底揭晓！",
            "灯塔之夜",
            "测试作者",
        )
        .unwrap();

        let ctx = build_spoiler_safe_context(
            &epub,
            "灯塔之夜",
            "测试作者",
            None,
            Some("第一章 启程"),
            Some(0.30),
        )
        .unwrap();

        assert_eq!(ctx.cutoff_chapter_index, 0);
        assert_eq!(ctx.skipped_future_chapters, 2);
        assert!(ctx.excerpt.contains("少年林远"));
        assert!(!ctx.excerpt.contains("最终反派其实是镇长"));
        assert!(!ctx.excerpt.contains("大结局"));

        let preview = offline_companion_preview(&ctx);
        assert!(!preview.is_ai);
        assert_eq!(preview.skipped_future_chapters, 2);
        assert!(preview.characters.iter().any(|c| c.name.contains("苏晚")));
    }

    #[test]
    fn cfi_spine_step_and_tts_segmentation_work() {
        assert_eq!(spine_index_from_cfi("epubcfi(/6/2!/4/2/1:0)"), Some(0));
        assert_eq!(spine_index_from_cfi("epubcfi(/6/6!/4/10/1:4)"), Some(2));
        assert_eq!(spine_index_from_cfi("invalid"), None);

        let epub = text_epub(
            "# 第一章\n第一句话慢慢铺开。第二句话继续温柔流淌。\n# 第二章\n这里开启了新的旅程。",
            "测试书",
            "作者",
        )
        .unwrap();
        let (segments, start_idx) =
            extract_tts_segments(&epub, None, Some("第二章"), Some(0.6)).unwrap();
        assert!(!segments.is_empty());
        assert_eq!(segments[start_idx].chapter_title, "第二章");
        assert!(
            segments[start_idx..]
                .iter()
                .any(|s| s.text.contains("新的旅程"))
        );
    }
}

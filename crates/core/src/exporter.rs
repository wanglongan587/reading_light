use crate::{Book, JournalEntry, State, store::atomic_write};
use anyhow::{Result, ensure};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObsidianExportReport {
    pub books_exported: usize,
    pub journal_months_exported: usize,
    pub total_highlights: usize,
    pub files_written: usize,
}

fn sanitize_filename(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\n' | '\r' | '\t' => ' ',
            _ => c,
        })
        .collect();
    let collapsed = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches('.')
        .to_owned();
    let truncated: String = collapsed.chars().take(72).collect();
    if truncated.is_empty() {
        "未命名笔记".into()
    } else {
        truncated
    }
}

fn yaml_escape(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace(['\n', '\r'], " ")
    )
}

fn csv_escape(s: &str) -> String {
    let clean = s.replace(['\r', '\n'], " ");
    if clean.contains([',', '"']) {
        format!("\"{}\"", clean.replace('"', "\"\""))
    } else {
        clean
    }
}

fn highlight_callout_kind(color: Option<&str>) -> (&'static str, &'static str) {
    match color.unwrap_or("amber") {
        "sage" => ("tip", "青苔绿"),
        "rose" => ("example", "浅绯红"),
        "sky" => ("info", "晴空蓝"),
        _ => ("quote", "微光琥珀"),
    }
}

/// Renders a single book's highlights, notes, bookmarks, completion letter, and related journal
/// entries into an Obsidian & Notion friendly Markdown document with YAML frontmatter.
pub fn render_book_markdown(book: &Book, journal: &[JournalEntry]) -> String {
    let mut out = String::new();
    let status = book.effective_status();
    let status_label = Book::status_label(status);
    let progress_pct = (book.progress.unwrap_or(0.0) * 100.0).round() as u32;

    out.push_str("---\n");
    out.push_str(&format!("title: {}\n", yaml_escape(&book.title)));
    out.push_str(&format!("author: {}\n", yaml_escape(&book.author)));
    out.push_str(&format!("format: {}\n", yaml_escape(&book.format)));
    out.push_str(&format!("status: {}\n", yaml_escape(status_label)));
    out.push_str(&format!("progress: {progress_pct}%\n"));
    out.push_str(&format!(
        "reading_minutes: {}\n",
        book.total_reading_minutes
    ));
    if let Some(finished) = &book.finished_at {
        out.push_str(&format!(
            "finished_at: {}\n",
            yaml_escape(finished.get(..10).unwrap_or(finished))
        ));
    }
    if let Some(ticket) = book.completion_ticket_no {
        out.push_str(&format!("ticket_no: \"NO.{ticket:04}\"\n"));
    }
    out.push_str("source: glow-reading\n");
    out.push_str("tags:\n  - 微光阅读\n");
    for tag in &book.tags {
        out.push_str(&format!("  - {}\n", yaml_escape(tag)));
    }
    out.push_str("---\n\n");

    out.push_str(&format!("# 《{}》\n\n", book.title));
    out.push_str(&format!(
        "> 作者：**{}**  ·  状态：**{}**  ·  阅读进度：**{}%**  ·  静读时长：**{} 分钟**\n\n",
        book.author, status_label, progress_pct, book.total_reading_minutes
    ));

    if let Some(letter) = book
        .finish_letter
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let ticket_title = book
            .completion_ticket_no
            .map(|n| format!("完读纪念信 · 票根 NO.{n:04}"))
            .unwrap_or_else(|| "完读纪念信".into());
        out.push_str(&format!("## {ticket_title}\n\n"));
        out.push_str("> [!abstract] 合上书页的余韵\n");
        for line in letter.lines() {
            out.push_str(&format!("> {}\n", line.trim()));
        }
        out.push('\n');
    }

    if !book.highlights.is_empty() {
        out.push_str(&format!(
            "## 书页划线与批注（{} 条）\n\n",
            book.highlights.len()
        ));
        for h in &book.highlights {
            let (callout, color_name) = highlight_callout_kind(h.color.as_deref());
            let date = h.created_at.get(..10).unwrap_or(&h.created_at);
            out.push_str(&format!("> [!{callout}] {color_name} · {date}\n"));
            for line in h.text.trim().lines() {
                out.push_str(&format!("> {}\n", line.trim()));
            }
            if let Some(note) = h.note.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                out.push_str(">\n");
                out.push_str(&format!("> **批注**：{note}\n"));
            }
            out.push('\n');
        }
    }

    if !book.bookmarks.is_empty() {
        out.push_str(&format!("## 书签（{} 处）\n\n", book.bookmarks.len()));
        for bm in &book.bookmarks {
            out.push_str(&format!("- **{}**\n", bm.label.trim()));
        }
        out.push('\n');
    }

    let linked_entries: Vec<&JournalEntry> = journal
        .iter()
        .filter(|e| {
            e.location
                .as_ref()
                .is_some_and(|loc| loc.book_id == book.id)
        })
        .collect();
    if !linked_entries.is_empty() {
        out.push_str(&format!(
            "## 关联手账与回响（{} 篇）\n\n",
            linked_entries.len()
        ));
        for entry in linked_entries {
            let date = entry.created_at.get(..10).unwrap_or(&entry.created_at);
            out.push_str(&format!("### {date} · {}\n\n", entry.input.trim()));
            out.push_str(&format!("{}\n\n", entry.echo.reply.trim()));
        }
    }

    out
}

/// Exports the entire Glow Reading library & journal into an Obsidian-ready folder structure:
/// - `微光书房总览.md` (MOC index with `[[wikilinks]]`)
/// - `书籍/<书名>.md` (per-book YAML frontmatter + callouts + completion letter)
/// - `手账/<YYYY-MM>.md` (monthly grouped journal archives)
pub fn export_obsidian_vault(state: &State, target_dir: &Path) -> Result<ObsidianExportReport> {
    let books_dir = target_dir.join("书籍");
    let journal_dir = target_dir.join("手账");
    fs::create_dir_all(&books_dir)?;
    fs::create_dir_all(&journal_dir)?;

    let mut files_written = 0usize;
    let mut total_highlights = 0usize;
    let mut used_names: BTreeMap<String, usize> = BTreeMap::new();
    let mut moc = String::from(
        "---\ntitle: \"微光书房总览\"\nsource: glow-reading\ntags:\n  - 微光阅读\n  - MOC\n---\n\n# 微光书房总览\n\n",
    );
    moc.push_str(&format!(
        "共藏书 **{} 本** · 手账档案 **{} 篇**\n\n## 我的藏书\n\n| 书名 | 作者 | 状态 | 进度 | 划线数 |\n| :--- | :--- | :--- | :---: | :---: |\n",
        state.books.len(),
        state.journal.len()
    ));

    for book in &state.books {
        let base_name = sanitize_filename(&book.title);
        let counter = used_names.entry(base_name.clone()).or_insert(0);
        *counter += 1;
        let file_stem = if *counter == 1 {
            base_name
        } else {
            format!("{base_name} ({})", *counter)
        };
        let md = render_book_markdown(book, &state.journal);
        atomic_write(&books_dir.join(format!("{file_stem}.md")), md.as_bytes())?;
        files_written += 1;
        total_highlights += book.highlights.len();
        let pct = (book.progress.unwrap_or(0.0) * 100.0).round() as u32;
        moc.push_str(&format!(
            "| [[书籍/{file_stem}\\|{}]] | {} | {} | {}% | {} |\n",
            book.title.replace('|', " "),
            book.author.replace('|', " "),
            Book::status_label(book.effective_status()),
            pct,
            book.highlights.len()
        ));
    }

    // Group journal entries by YYYY-MM
    let mut by_month: BTreeMap<String, Vec<&JournalEntry>> = BTreeMap::new();
    for entry in &state.journal {
        let ym = entry
            .created_at
            .get(..7)
            .filter(|s| s.len() == 7 && s.as_bytes()[4] == b'-')
            .unwrap_or("未归档")
            .to_owned();
        by_month.entry(ym).or_default().push(entry);
    }

    if !by_month.is_empty() {
        moc.push_str("\n## 月度手账档案\n\n");
    }
    for (ym, entries) in &by_month {
        let mut month_md = format!(
            "---\ntitle: \"微光手账 {ym}\"\nsource: glow-reading\ntags:\n  - 微光手账\n---\n\n# 微光手账 · {ym}\n\n"
        );
        for entry in entries {
            let date = entry.created_at.get(..10).unwrap_or(&entry.created_at);
            let kind_tag = match entry.kind.as_str() {
                "ticket" => "#完读票根",
                "mood" => "#心情",
                _ => "#阅读回响",
            };
            let fav = if entry.favorite { " ★" } else { "" };
            month_md.push_str(&format!("## {date}{fav} {kind_tag}\n\n"));
            if let Some(m) = entry.mood.as_deref().filter(|s| !s.is_empty()) {
                month_md.push_str(&format!("- 情绪波段：`{m}`\n"));
            }
            if !entry.input.trim().is_empty() {
                month_md.push_str(&format!(
                    "\n> {}\n",
                    entry.input.trim().replace('\n', "\n> ")
                ));
            }
            month_md.push_str(&format!("\n{}\n\n", entry.echo.reply.trim()));
            for q in &entry.echo.quotes {
                month_md.push_str(&format!(
                    "> [!quote] 《{}》 · {}\n> {}\n\n",
                    q.work,
                    q.author,
                    q.text.trim()
                ));
            }
            month_md.push_str("---\n\n");
        }
        atomic_write(&journal_dir.join(format!("{ym}.md")), month_md.as_bytes())?;
        files_written += 1;
        moc.push_str(&format!(
            "- [[手账/{ym}|微光手账 {ym}]]（{} 篇）\n",
            entries.len()
        ));
    }

    atomic_write(&target_dir.join("微光书房总览.md"), moc.as_bytes())?;
    files_written += 1;

    Ok(ObsidianExportReport {
        books_exported: state.books.len(),
        journal_months_exported: by_month.len(),
        total_highlights,
        files_written,
    })
}

/// Renders the entire shelf into a clean RFC-4180 CSV ready for direct Notion Reading Database import.
pub fn render_notion_csv(books: &[Book]) -> Result<String> {
    ensure!(!books.is_empty(), "书架目前为空，暂时没有可导出的书目");
    let mut csv = String::from(
        "\u{feff}Title,Author,Format,Status,Progress,Tags,Highlights,ReadingMinutes,FinishedAt,LastHighlight\n",
    );
    for b in books {
        let progress = format!("{:.0}%", b.progress.unwrap_or(0.0) * 100.0);
        let tags = b.tags.join("; ");
        let finished = b
            .finished_at
            .as_deref()
            .and_then(|s| s.get(..10))
            .unwrap_or("");
        let last_highlight = b.highlights.last().map(|h| h.text.as_str()).unwrap_or("");
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&b.title),
            csv_escape(&b.author),
            csv_escape(&b.format.to_uppercase()),
            csv_escape(Book::status_label(b.effective_status())),
            csv_escape(&progress),
            csv_escape(&tags),
            b.highlights.len(),
            b.total_reading_minutes,
            csv_escape(finished),
            csv_escape(last_highlight),
        ));
    }
    Ok(csv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;

    #[test]
    fn exports_obsidian_vault_and_notion_csv_with_frontmatter_and_callouts() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("data")).unwrap();
        let book_id = store.install_catalog_book("starlight-post").unwrap();
        store
            .add_highlight_styled(
                &book_id,
                "灯光不必照亮整片原野，只要足够照见眼前这一页。".into(),
                "epubcfi(/6/2!/4/2,/1:0,/1:18)".into(),
                Some("sage".into()),
                Some("读到这句忽然松弛下来".into()),
            )
            .unwrap();
        store
            .complete_book(&book_id, "今晚与慢邮局一起安稳入眠。".into(), None)
            .unwrap();

        let vault = dir.path().join("obsidian_export");
        let report = export_obsidian_vault(&store.state, &vault).unwrap();
        assert_eq!(report.books_exported, 1);
        assert_eq!(report.total_highlights, 1);
        assert_eq!(report.journal_months_exported, 1);
        assert!(vault.join("微光书房总览.md").exists());

        let book_md = fs::read_to_string(vault.join("书籍/星灯与慢邮局.md")).unwrap();
        assert!(book_md.contains("source: glow-reading"));
        assert!(book_md.contains("> [!tip] 青苔绿"));
        assert!(book_md.contains("读到这句忽然松弛下来"));
        assert!(book_md.contains("完读纪念信 · 票根 NO.0001"));

        let csv = render_notion_csv(&store.state.books).unwrap();
        assert!(csv.contains("Title,Author,Format,Status"));
        assert!(csv.contains("星灯与慢邮局"));
        assert!(csv.contains(Book::status_label("finished")));
    }
}

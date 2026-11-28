use crate::model::{Book, Highlight, JournalEntry, ReadingSession, State};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::{collections::BTreeMap, path::Path};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryHit {
    pub entry_id: String,
    /// "highlight" | "journal"
    pub kind: String,
    pub book_id: Option<String>,
    pub book_title: String,
    pub excerpt: String,
    pub note: Option<String>,
    pub created_at: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthlyReadingSummary {
    pub year_month: String,
    pub total_minutes: u32,
    pub active_days: usize,
    pub daily_minutes: Vec<(String, u32)>,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("无法打开本地 SQLite 数据库：{}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000_i64)?;
        let mut db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS app_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                state_header_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS books (
                id TEXT PRIMARY KEY,
                position INTEGER NOT NULL,
                title TEXT NOT NULL,
                author TEXT NOT NULL,
                format TEXT NOT NULL,
                imported_at TEXT NOT NULL,
                progress REAL,
                chapter TEXT,
                updated_at TEXT,
                book_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS highlights (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                text TEXT NOT NULL,
                cfi TEXT NOT NULL,
                created_at TEXT NOT NULL,
                color TEXT,
                note TEXT,
                FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_highlights_book ON highlights(book_id, position);

            CREATE TABLE IF NOT EXISTS journal_entries (
                id TEXT PRIMARY KEY,
                position INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                kind TEXT NOT NULL,
                input TEXT NOT NULL,
                mood TEXT,
                favorite INTEGER NOT NULL DEFAULT 0,
                entry_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_journal_created ON journal_entries(position);

            CREATE TABLE IF NOT EXISTS reading_sessions (
                id TEXT PRIMARY KEY,
                book_id TEXT,
                date TEXT NOT NULL,
                minutes INTEGER NOT NULL,
                recorded_at TEXT NOT NULL,
                theme_id TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_date ON reading_sessions(date);

            CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
                entry_id UNINDEXED,
                kind UNINDEXED,
                book_id UNINDEXED,
                book_title UNINDEXED,
                excerpt UNINDEXED,
                note UNINDEXED,
                created_at UNINDEXED,
                search_tokens,
                tokenize = 'unicode61'
            );
            ",
        )?;

        self.conn.execute(
            "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )?;
        Ok(())
    }

    pub fn has_state(&self) -> Result<bool> {
        let count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM app_meta WHERE id = 1", [], |row| {
                    row.get(0)
                })?;
        Ok(count > 0)
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO schema_meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn load_state(&self) -> Result<Option<State>> {
        let header_json: Option<String> = self
            .conn
            .query_row(
                "SELECT state_header_json FROM app_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let Some(header_json) = header_json else {
            return Ok(None);
        };

        let mut state: State =
            serde_json::from_str(&header_json).context("SQLite 中的应用配置数据无法解析")?;

        // Load books in display order
        let mut book_stmt = self
            .conn
            .prepare("SELECT book_json FROM books ORDER BY position ASC")?;
        let mut books = Vec::new();
        for row in book_stmt.query_map([], |row| row.get::<_, String>(0))? {
            let mut book: Book =
                serde_json::from_str(&row?).context("SQLite 中的书目记录无法解析")?;
            book.highlights.clear();
            books.push(book);
        }

        // Load highlights grouped by book_id
        let mut hl_stmt = self.conn.prepare(
            "SELECT id, book_id, text, cfi, created_at, color, note
             FROM highlights
             ORDER BY book_id ASC, position ASC",
        )?;
        let mut hl_by_book: BTreeMap<String, Vec<Highlight>> = BTreeMap::new();
        for row in hl_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                Highlight {
                    id: row.get(0)?,
                    text: row.get(2)?,
                    cfi: row.get(3)?,
                    created_at: row.get(4)?,
                    color: row.get(5)?,
                    note: row.get(6)?,
                },
            ))
        })? {
            let (book_id, hl) = row?;
            hl_by_book.entry(book_id).or_default().push(hl);
        }
        for book in &mut books {
            if let Some(list) = hl_by_book.remove(&book.id) {
                book.highlights = list;
            }
        }
        state.books = books;

        // Load journal entries in display order
        let mut j_stmt = self
            .conn
            .prepare("SELECT entry_json FROM journal_entries ORDER BY position ASC")?;
        let mut journal = Vec::new();
        for row in j_stmt.query_map([], |row| row.get::<_, String>(0))? {
            let entry: JournalEntry =
                serde_json::from_str(&row?).context("SQLite 中的手账记录无法解析")?;
            journal.push(entry);
        }
        state.journal = journal;

        // Load reading sessions history
        let mut s_stmt = self.conn.prepare(
            "SELECT id, book_id, date, minutes, recorded_at, theme_id
             FROM reading_sessions
             ORDER BY recorded_at ASC, id ASC",
        )?;
        let mut sessions = Vec::new();
        for row in s_stmt.query_map([], |row| {
            Ok(ReadingSession {
                id: row.get(0)?,
                book_id: row.get(1)?,
                date: row.get(2)?,
                minutes: row.get::<_, u32>(3)?,
                recorded_at: row.get(4)?,
                theme_id: row.get(5)?,
            })
        })? {
            sessions.push(row?);
        }
        state.reading_sessions = sessions;

        Ok(Some(state))
    }

    pub fn save_state(&mut self, state: &State) -> Result<()> {
        let tx = self.conn.transaction()?;

        // Persist lightweight header (all settings/preferences/tombstones, without duplicating large arrays)
        let mut header = state.clone();
        header.books.clear();
        header.journal.clear();
        header.reading_sessions.clear();
        let header_json = serde_json::to_string(&header)?;
        let now_ts = crate::model::now();

        tx.execute(
            "INSERT OR REPLACE INTO app_meta (id, state_header_json, updated_at) VALUES (1, ?1, ?2)",
            params![header_json, now_ts],
        )?;

        tx.execute("DELETE FROM highlights", [])?;
        tx.execute("DELETE FROM books", [])?;
        tx.execute("DELETE FROM journal_entries", [])?;
        tx.execute("DELETE FROM reading_sessions", [])?;
        tx.execute("DELETE FROM memory_fts", [])?;

        {
            let mut book_ins = tx.prepare(
                "INSERT INTO books (id, position, title, author, format, imported_at, progress, chapter, updated_at, book_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            let mut hl_ins = tx.prepare(
                "INSERT INTO highlights (id, book_id, position, text, cfi, created_at, color, note)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            let mut fts_ins = tx.prepare(
                "INSERT INTO memory_fts (entry_id, kind, book_id, book_title, excerpt, note, created_at, search_tokens)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;

            for (pos, book) in state.books.iter().enumerate() {
                let mut book_meta = book.clone();
                book_meta.highlights.clear();
                let book_json = serde_json::to_string(&book_meta)?;
                book_ins.execute(params![
                    book.id,
                    pos as i64,
                    book.title,
                    book.author,
                    book.format,
                    book.imported_at,
                    book.progress,
                    book.chapter,
                    book.updated_at,
                    book_json,
                ])?;

                for (hl_pos, hl) in book.highlights.iter().enumerate() {
                    hl_ins.execute(params![
                        hl.id,
                        book.id,
                        hl_pos as i64,
                        hl.text,
                        hl.cfi,
                        hl.created_at,
                        hl.color,
                        hl.note,
                    ])?;

                    let combined = format!(
                        "{} {} {}",
                        book.title,
                        hl.text,
                        hl.note.as_deref().unwrap_or_default()
                    );
                    fts_ins.execute(params![
                        hl.id,
                        "highlight",
                        book.id,
                        book.title,
                        hl.text,
                        hl.note,
                        hl.created_at,
                        tokenize_for_fts(&combined),
                    ])?;
                }
            }

            let mut j_ins = tx.prepare(
                "INSERT INTO journal_entries (id, position, created_at, kind, input, mood, favorite, entry_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for (pos, entry) in state.journal.iter().enumerate() {
                let entry_json = serde_json::to_string(entry)?;
                j_ins.execute(params![
                    entry.id,
                    pos as i64,
                    entry.created_at,
                    entry.kind,
                    entry.input,
                    entry.mood,
                    if entry.favorite { 1_i64 } else { 0_i64 },
                    entry_json,
                ])?;

                let book_id = entry.location.as_ref().map(|l| l.book_id.as_str());
                let book_title = book_id
                    .and_then(|bid| state.books.iter().find(|b| b.id == bid))
                    .map(|b| b.title.as_str())
                    .unwrap_or("微光手账");
                let combined = format!(
                    "{} {} {} {}",
                    book_title,
                    entry.mood.as_deref().unwrap_or_default(),
                    entry.input,
                    entry.echo.reply
                );
                fts_ins.execute(params![
                    entry.id,
                    "journal",
                    book_id,
                    book_title,
                    entry.input,
                    Some(entry.echo.reply.as_str()),
                    entry.created_at,
                    tokenize_for_fts(&combined),
                ])?;
            }

            let mut s_ins = tx.prepare(
                "INSERT INTO reading_sessions (id, book_id, date, minutes, recorded_at, theme_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for session in &state.reading_sessions {
                s_ins.execute(params![
                    session.id,
                    session.book_id,
                    session.date,
                    session.minutes,
                    session.recorded_at,
                    session.theme_id,
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Full-text search across personal highlights and journal entries via SQLite FTS5.
    /// Supports both CJK characters (1+ chars) and Latin/ASCII words with BM25 relevance ranking.
    pub fn search_memories(&self, query: &str, limit: usize) -> Result<Vec<MemoryHit>> {
        let Some(fts_query) = build_fts_query(query) else {
            return Ok(vec![]);
        };
        let cap = limit.clamp(1, 50) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT entry_id, kind, book_id, book_title, excerpt, note, created_at, bm25(memory_fts) AS rank
             FROM memory_fts
             WHERE memory_fts MATCH ?1
             ORDER BY rank ASC, created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_query, cap], |row| {
            Ok(MemoryHit {
                entry_id: row.get(0)?,
                kind: row.get(1)?,
                book_id: row.get(2)?,
                book_title: row.get(3)?,
                excerpt: row.get(4)?,
                note: row.get(5)?,
                created_at: row.get(6)?,
                score: row.get(7)?,
            })
        })?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(row?);
        }
        Ok(hits)
    }

    /// Aggregates reading sessions for a given `YYYY-MM` month for the anti-anxiety glow calendar.
    pub fn monthly_reading_summary(&self, year_month: &str) -> Result<MonthlyReadingSummary> {
        let prefix = format!("{year_month}-%");
        let mut stmt = self.conn.prepare(
            "SELECT date, SUM(minutes)
             FROM reading_sessions
             WHERE date LIKE ?1
             GROUP BY date
             ORDER BY date ASC",
        )?;
        let mut daily = Vec::new();
        let mut total = 0u32;
        for row in stmt.query_map(params![prefix], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })? {
            let (date, mins) = row?;
            if mins > 0 {
                total = total.saturating_add(mins);
                daily.push((date, mins));
            }
        }
        Ok(MonthlyReadingSummary {
            year_month: year_month.to_owned(),
            total_minutes: total,
            active_days: daily.len(),
            daily_minutes: daily,
        })
    }
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x3040..=0x30FF
            | 0xAC00..=0xD7AF
    )
}

/// Inserts spaces around CJK ideographs while preserving ASCII/Latin words so SQLite's
/// built-in `unicode61` FTS5 tokenizer matches Chinese phrases accurately with zero extensions.
pub(crate) fn tokenize_for_fts(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 32);
    let mut last_was_space = true;
    for ch in input.chars() {
        if is_cjk(ch) {
            if !last_was_space {
                out.push(' ');
            }
            out.push(ch);
            out.push(' ');
            last_was_space = true;
        } else if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim().to_owned()
}

fn build_fts_query(raw: &str) -> Option<String> {
    let mut clauses = Vec::new();
    for segment in raw.split_whitespace() {
        let tokenized = tokenize_for_fts(segment);
        let parts: Vec<&str> = tokenized.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        if parts.len() == 1 {
            let clean = parts[0].replace('"', "");
            if !clean.is_empty() {
                clauses.push(format!("\"{clean}\""));
            }
        } else {
            let phrase = parts
                .iter()
                .map(|p| p.replace('"', ""))
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !phrase.is_empty() {
                clauses.push(format!("\"{phrase}\""));
            }
        }
    }
    if clauses.is_empty() {
        None
    } else {
        Some(clauses.join(" AND "))
    }
}

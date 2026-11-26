use crate::model::*;
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub struct Store {
    pub root: PathBuf,
    pub state: State,
}

impl Store {
    pub fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(root.join("books"))?;
        fs::create_dir_all(root.join("themes"))?;
        let path = root.join("state.json");
        let mut state: State = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)
                .context("本地数据无法读取；原文件已保留，请先备份后修复。")?
        } else {
            State::default()
        };
        let is_legacy_forest = state.theme.id == "forest"
            && ((state.theme.background.eq_ignore_ascii_case("#E8EDE6")
                && state.theme.paper.eq_ignore_ascii_case("#FAF7EE")
                && state.theme.accent.eq_ignore_ascii_case("#3B664C"))
                || (state.theme.background.eq_ignore_ascii_case("#EDEEE5")
                    && state.theme.paper.eq_ignore_ascii_case("#FAF6EA")
                    && state.theme.accent.eq_ignore_ascii_case("#536A49")));
        if is_legacy_forest {
            let refreshed = Theme::default();
            state.theme.background = refreshed.background;
            state.theme.paper = refreshed.paper;
            state.theme.text = refreshed.text;
            state.theme.muted = refreshed.muted;
            state.theme.accent = refreshed.accent;
        }
        ensure!(state.version == 1, "不支持的本地数据版本");
        ensure!(
            state
                .soundscape_preset
                .as_deref()
                .is_none_or(crate::model::valid_soundscape_preset),
            "声景频道设置损坏"
        );
        ensure!(
            crate::model::valid_radio_lamp_color(&state.radio.lamp_color)
                && crate::model::valid_radio_lamp_mode(&state.radio.lamp_mode)
                && (0.05..=1.0).contains(&state.radio.volume),
            "深夜电台设置损坏"
        );
        for station in &state.radio.custom_stations {
            ensure!(
                !station.id.trim().is_empty()
                    && station.id.len() <= 120
                    && !station.name.trim().is_empty()
                    && station.name.chars().count() <= 120
                    && (88.0..=108.0).contains(&station.frequency_mhz)
                    && (station.kind != "online"
                        || crate::model::valid_radio_stream_url(&station.stream_url)),
                "自定义电台配置损坏"
            );
        }
        ensure!(
            state
                .reader_font_size
                .is_none_or(|value| (14.0..=36.0).contains(&value))
                && state
                    .reader_line_height
                    .is_none_or(|value| (1.3..=2.6).contains(&value))
                && state
                    .reader_width
                    .is_none_or(|value| (400..=1100).contains(&value)),
            "阅读排版设置损坏"
        );
        crate::theme::validate(&state.theme)?;
        for theme in &state.custom_themes {
            crate::theme::validate(theme)?;
        }
        for book in &state.books {
            ensure!(
                book.id.len() == 64 && book.id.chars().all(|c| c.is_ascii_hexdigit()),
                "书籍标识损坏"
            );
            if let Some(cover) = &book.cover {
                ensure!(crate::library::safe_path(cover), "封面路径损坏");
            }
            ensure!(
                book.progress
                    .is_none_or(|value| (0.0..=1.0).contains(&value)),
                "阅读进度损坏"
            );
            ensure!(
                book.chapter
                    .as_ref()
                    .is_none_or(|chapter| chapter.len() <= 300),
                "阅读章节损坏"
            );
            for highlight in &book.highlights {
                ensure!(
                    !highlight.id.is_empty()
                        && highlight.id.len() <= 80
                        && !highlight.text.trim().is_empty()
                        && highlight.text.len() <= 16_384
                        && crate::model::valid_cfi(&highlight.cfi)
                        && highlight
                            .color
                            .as_deref()
                            .is_none_or(crate::model::valid_highlight_color)
                        && highlight
                            .note
                            .as_ref()
                            .is_none_or(|note| note.chars().count() <= 1000),
                    "划线数据损坏"
                );
            }
        }
        Ok(Self { root, state })
    }

    pub fn default_root() -> Result<PathBuf> {
        if let Some(path) = std::env::var_os("GLOW_DATA_DIR") {
            return Ok(path.into());
        }
        Ok(
            directories::ProjectDirs::from("studio", "GlowReading", "GlowReading")
                .context("无法获取本地应用数据目录")?
                .data_local_dir()
                .to_owned(),
        )
    }

    pub fn save(&self) -> Result<()> {
        atomic_write(
            &self.root.join("state.json"),
            &serde_json::to_vec_pretty(&self.state)?,
        )
    }

    pub fn load_secrets(&self) -> LocalSecrets {
        let path = self.root.join("secrets.json");
        fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<LocalSecrets>(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save_secrets(&self, secrets: &LocalSecrets) -> Result<()> {
        atomic_write(
            &self.root.join("secrets.json"),
            &serde_json::to_vec_pretty(secrets)?,
        )
    }

    pub fn record(
        &mut self,
        kind: &str,
        input: String,
        mood: Option<String>,
        echo: EchoResult,
        location: Option<Location>,
    ) -> Result<()> {
        self.state.journal.insert(
            0,
            JournalEntry {
                id: uuid::Uuid::new_v4().to_string(),
                created_at: now(),
                kind: kind.into(),
                input,
                mood,
                echo,
                location,
                favorite: false,
            },
        );
        if let Err(error) = self.save() {
            self.state.journal.remove(0);
            return Err(error);
        }
        Ok(())
    }

    pub fn remove_book(&mut self, id: &str) -> Result<()> {
        let cached = (id.len() == 64 && id.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| self.root.join("books").join(id));
        let previous_books = self.state.books.clone();
        let previous_last_book = self.state.last_book.clone();
        let previous_deleted = self.state.deleted_book_ids.clone();
        let removed = self.state.books.iter().any(|book| book.id == id);
        self.state.books.retain(|b| b.id != id);
        if self.state.last_book.as_deref() == Some(id) {
            self.state.last_book = None;
        }
        if removed && !self.state.deleted_book_ids.iter().any(|d| d == id) {
            self.state.deleted_book_ids.push(id.to_owned());
            if self.state.deleted_book_ids.len() > 500 {
                let drain = self.state.deleted_book_ids.len() - 500;
                self.state.deleted_book_ids.drain(0..drain);
            }
        }
        if let Err(error) = self.save() {
            self.state.books = previous_books;
            self.state.last_book = previous_last_book;
            self.state.deleted_book_ids = previous_deleted;
            return Err(error);
        }
        if removed
            && let Some(cached) = cached
            && cached.exists()
        {
            fs::remove_dir_all(&cached)
                .with_context(|| "书籍已移出书架，但本地缓存未能清理".to_string())?;
        }
        Ok(())
    }

    pub fn edit_journal(&mut self, edit: impl FnOnce(&mut Vec<JournalEntry>)) -> Result<()> {
        let previous = self.state.journal.clone();
        let previous_deleted = self.state.deleted_journal_ids.clone();
        edit(&mut self.state.journal);
        for old in &previous {
            if !self.state.journal.iter().any(|cur| cur.id == old.id)
                && !self.state.deleted_journal_ids.iter().any(|d| d == &old.id)
            {
                self.state.deleted_journal_ids.push(old.id.clone());
            }
        }
        if self.state.deleted_journal_ids.len() > 500 {
            let drain = self.state.deleted_journal_ids.len() - 500;
            self.state.deleted_journal_ids.drain(0..drain);
        }
        if let Err(error) = self.save() {
            self.state.journal = previous;
            self.state.deleted_journal_ids = previous_deleted;
            return Err(error);
        }
        Ok(())
    }

    pub fn add_highlight(
        &mut self,
        book_id: &str,
        text: String,
        cfi: String,
    ) -> Result<(crate::model::Highlight, bool)> {
        self.add_highlight_styled(book_id, text, cfi, None, None)
    }

    pub fn add_highlight_styled(
        &mut self,
        book_id: &str,
        text: String,
        cfi: String,
        color: Option<String>,
        note: Option<String>,
    ) -> Result<(crate::model::Highlight, bool)> {
        let text = text.trim().to_owned();
        ensure!(!text.is_empty(), "请先选择要划线的文字");
        ensure!(text.chars().count() <= 4000, "划线文字过长");
        ensure!(crate::model::valid_cfi(&cfi), "阅读位置无效");
        let color = color.filter(|c| !c.trim().is_empty());
        if let Some(c) = &color {
            ensure!(crate::model::valid_highlight_color(c), "不支持的划线颜色");
        }
        let note = note.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
        if let Some(n) = &note {
            ensure!(n.chars().count() <= 1000, "批注请控制在 1000 字以内");
        }
        let previous = {
            let book = self
                .state
                .books
                .iter()
                .find(|book| book.id == book_id)
                .context("书籍已不在书架中")?;
            book.highlights.clone()
        };
        let (highlight, inserted) = {
            let book = self
                .state
                .books
                .iter_mut()
                .find(|book| book.id == book_id)
                .context("书籍已不在书架中")?;
            if let Some(existing) = book
                .highlights
                .iter_mut()
                .find(|highlight| highlight.cfi == cfi)
            {
                let mut changed = false;
                if let Some(new_color) = color
                    && existing.color.as_deref() != Some(new_color.as_str())
                {
                    existing.color = Some(new_color);
                    changed = true;
                }
                if let Some(new_note) = note
                    && existing.note.as_deref() != Some(new_note.as_str())
                {
                    existing.note = Some(new_note);
                    changed = true;
                }
                if !changed {
                    return Ok((existing.clone(), false));
                }
                (existing.clone(), false)
            } else {
                let highlight = crate::model::Highlight {
                    id: uuid::Uuid::new_v4().to_string(),
                    text,
                    cfi,
                    created_at: now(),
                    color,
                    note,
                };
                book.highlights.push(highlight.clone());
                (highlight, true)
            }
        };
        if let Err(error) = self.save() {
            if let Some(book) = self.state.books.iter_mut().find(|book| book.id == book_id) {
                book.highlights = previous;
            }
            return Err(error);
        }
        Ok((highlight, inserted))
    }

    pub fn update_highlight(
        &mut self,
        book_id: &str,
        highlight_id: &str,
        color: Option<Option<String>>,
        note: Option<Option<String>>,
    ) -> Result<Option<crate::model::Highlight>> {
        if let Some(Some(ref c)) = color {
            ensure!(crate::model::valid_highlight_color(c), "不支持的划线颜色");
        }
        let normalized_note = match note {
            Some(Some(raw)) => {
                let trimmed = raw.trim().to_owned();
                ensure!(trimmed.chars().count() <= 1000, "批注请控制在 1000 字以内");
                Some(if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                })
            }
            Some(None) => Some(None),
            None => None,
        };
        let previous = {
            let Some(book) = self.state.books.iter().find(|book| book.id == book_id) else {
                return Ok(None);
            };
            book.highlights.clone()
        };
        let updated = {
            let Some(book) = self.state.books.iter_mut().find(|book| book.id == book_id) else {
                return Ok(None);
            };
            let Some(item) = book
                .highlights
                .iter_mut()
                .find(|highlight| highlight.id == highlight_id)
            else {
                return Ok(None);
            };
            if let Some(next_color) = color {
                item.color = next_color;
            }
            if let Some(next_note) = normalized_note {
                item.note = next_note;
            }
            item.clone()
        };
        if let Err(error) = self.save() {
            if let Some(book) = self.state.books.iter_mut().find(|book| book.id == book_id) {
                book.highlights = previous;
            }
            return Err(error);
        }
        Ok(Some(updated))
    }

    pub fn add_reading_minutes(&mut self, minutes: u32, today: &str) -> Result<u32> {
        let prev_mins = self.state.reading_minutes_today;
        let prev_date = self.state.reading_minutes_date.clone();
        if self.state.reading_minutes_date.as_deref() != Some(today) {
            self.state.reading_minutes_today = 0;
            self.state.reading_minutes_date = Some(today.to_owned());
        }
        self.state.reading_minutes_today = self.state.reading_minutes_today.saturating_add(minutes);
        if let Err(error) = self.save() {
            self.state.reading_minutes_today = prev_mins;
            self.state.reading_minutes_date = prev_date;
            return Err(error);
        }
        Ok(self.state.reading_minutes_today)
    }

    pub fn remove_highlight(
        &mut self,
        book_id: &str,
        highlight_id: &str,
    ) -> Result<Option<crate::model::Highlight>> {
        let (index, removed) = {
            let Some(book) = self.state.books.iter_mut().find(|book| book.id == book_id) else {
                return Ok(None);
            };
            let Some(index) = book
                .highlights
                .iter()
                .position(|highlight| highlight.id == highlight_id)
            else {
                return Ok(None);
            };
            (index, book.highlights.remove(index))
        };
        let previous_deleted = self.state.deleted_highlight_ids.clone();
        if !self
            .state
            .deleted_highlight_ids
            .iter()
            .any(|d| d == &removed.id)
        {
            self.state.deleted_highlight_ids.push(removed.id.clone());
            if self.state.deleted_highlight_ids.len() > 500 {
                let drain = self.state.deleted_highlight_ids.len() - 500;
                self.state.deleted_highlight_ids.drain(0..drain);
            }
        }
        if let Err(error) = self.save() {
            if let Some(book) = self.state.books.iter_mut().find(|book| book.id == book_id) {
                book.highlights.insert(index, removed);
            }
            self.state.deleted_highlight_ids = previous_deleted;
            return Err(error);
        }
        Ok(Some(removed))
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("缺少父目录")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_journal_write_does_not_leave_an_unsaved_entry() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().into()).unwrap();
        store.save().unwrap();
        fs::remove_file(dir.path().join("state.json")).unwrap();
        fs::create_dir(dir.path().join("state.json")).unwrap();
        assert!(
            store
                .record(
                    "mood",
                    "我的文字".into(),
                    None,
                    crate::echo::respond("", Some("疲惫")),
                    None
                )
                .is_err()
        );
        assert!(store.state.journal.is_empty());
    }
    #[test]
    fn failed_delete_retains_journal_and_library() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().into()).unwrap();
        store.install_sample().unwrap();
        let book_id = store.state.books[0].id.clone();
        store.state.last_book = Some(book_id.clone());
        store
            .record(
                "mood",
                "保留我的日记".into(),
                None,
                crate::echo::respond("", Some("希望")),
                None,
            )
            .unwrap();
        fs::remove_file(dir.path().join("state.json")).unwrap();
        fs::create_dir(dir.path().join("state.json")).unwrap();
        assert!(store.edit_journal(Vec::clear).is_err());
        assert_eq!(store.state.journal[0].input, "保留我的日记");
        assert!(store.remove_book(&book_id).is_err());
        assert_eq!(store.state.books[0].id, book_id);
        assert_eq!(store.state.last_book.as_deref(), Some(book_id.as_str()));
    }
    #[test]
    fn overwrite_restart_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().into()).unwrap();
        store.save().unwrap();
        store.state.privacy_seen = true;
        store
            .record(
                "mood",
                "今天".into(),
                Some("疲惫".into()),
                EchoResult {
                    quotes: vec![],
                    reply: "慢慢来".into(),
                    demo: true,
                },
                None,
            )
            .unwrap();
        let mut reopened = Store::open(dir.path().into()).unwrap();
        assert!(reopened.state.privacy_seen);
        assert_eq!(reopened.state.journal.len(), 1);
        reopened.state.journal.clear();
        reopened.save().unwrap();
        assert!(
            Store::open(dir.path().into())
                .unwrap()
                .state
                .journal
                .is_empty()
        );
    }
    #[test]
    fn corrupt_state_is_not_silently_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("state.json"), b"invalid").unwrap();
        assert!(Store::open(dir.path().into()).is_err());
        assert_eq!(fs::read(dir.path().join("state.json")).unwrap(), b"invalid");
    }

    #[test]
    fn removing_a_book_clears_cached_files_but_keeps_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("我的书.txt");
        fs::write(&source, "第一章\n这是只应留在原处的文字。").unwrap();
        let mut store = Store::open(dir.path().join("data")).unwrap();
        let id = store.import_book(&source, None).unwrap();
        let cached = store.root.join("books").join(&id);
        assert!(cached.join("book.epub").exists());
        assert!(cached.join("original.txt").exists());
        store.remove_book(&id).unwrap();
        assert!(store.state.books.is_empty());
        assert!(!cached.exists());
        assert!(source.exists());
    }

    #[test]
    fn highlights_are_deduplicated_persisted_and_removable() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("data");
        let mut store = Store::open(root.clone()).unwrap();
        store.install_sample().unwrap();
        let book_id = store.state.books[0].id.clone();
        let cfi = "epubcfi(/6/4!/4/2,/1:0,/1:8)";
        let (highlight, inserted) = store
            .add_highlight(&book_id, "  留住的文字  ".into(), cfi.into())
            .unwrap();
        assert!(inserted);
        assert_eq!(highlight.text, "留住的文字");
        let (same, inserted) = store
            .add_highlight(&book_id, "重复".into(), cfi.into())
            .unwrap();
        assert!(!inserted);
        assert_eq!(same.id, highlight.id);
        assert_eq!(store.state.books[0].highlights.len(), 1);

        let reopened = Store::open(root).unwrap();
        assert_eq!(reopened.state.books[0].highlights[0].text, "留住的文字");
        let removed = store
            .remove_highlight(&book_id, &highlight.id)
            .unwrap()
            .unwrap();
        assert_eq!(removed.cfi, cfi);
        assert!(store.state.books[0].highlights.is_empty());
    }

    #[test]
    fn legacy_state_deserializes_and_migrates_forest_palette_and_supports_colored_notes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("data");
        fs::create_dir_all(&root).unwrap();
        let book_id = "0".repeat(64);
        let legacy_json = serde_json::json!({
            "version": 1,
            "books": [{
                "id": book_id,
                "title": "旧版书",
                "author": "作者",
                "format": "epub",
                "imported_at": "2026-01-01T00:00:00Z",
                "cover": null,
                "location": null,
                "bookmarks": [],
                "highlights": [{
                    "id": "hl-legacy",
                    "text": "旧版无颜色划线",
                    "cfi": "epubcfi(/6/2!/4/2,/1:0,/1:6)",
                    "created_at": "2026-01-01T00:00:00Z"
                }]
            }],
            "journal": [],
            "theme": {
                "version": 1,
                "id": "forest",
                "name": "林间暖光",
                "background": "#E8EDE6",
                "paper": "#FAF7EE",
                "text": "#26332B",
                "muted": "#5A6B60",
                "accent": "#3B664C",
                "font": "serif",
                "font_size": 21.0,
                "line_height": 1.9,
                "width": 720,
                "overlay": 0.86,
                "volume": 0.35,
                "reduced_motion": false
            },
            "custom_themes": [],
            "mode": "scrolled",
            "last_book": null,
            "privacy_seen": true
        });
        fs::write(
            root.join("state.json"),
            serde_json::to_vec_pretty(&legacy_json).unwrap(),
        )
        .unwrap();

        let mut store = Store::open(root.clone()).unwrap();
        assert_eq!(store.state.theme.paper, "#F8FCFF");
        assert_eq!(store.state.theme.accent, "#2573B8");
        assert_eq!(store.state.theme.font_size, 21.0);
        assert_eq!(store.state.books[0].highlights[0].color, None);
        assert_eq!(store.state.books[0].highlights[0].note, None);
        assert!(!store.state.warm_filter);
        assert_eq!(store.state.reading_minutes_today, 0);

        let updated = store
            .update_highlight(
                &book_id,
                "hl-legacy",
                Some(Some("amber".into())),
                Some(Some("  读到这里突然安静下来  ".into())),
            )
            .unwrap()
            .unwrap();
        assert_eq!(updated.color.as_deref(), Some("amber"));
        assert_eq!(updated.note.as_deref(), Some("读到这里突然安静下来"));
        assert!(
            store
                .update_highlight(&book_id, "hl-legacy", Some(Some("neon".into())), None)
                .is_err()
        );
        assert!(
            store
                .update_highlight(&book_id, "hl-legacy", None, Some(Some("字".repeat(1001))))
                .is_err()
        );
        assert_eq!(store.add_reading_minutes(25, "2026-11-25").unwrap(), 25);
        assert_eq!(store.add_reading_minutes(15, "2026-11-25").unwrap(), 40);
        assert_eq!(store.add_reading_minutes(10, "2026-11-26").unwrap(), 10);

        let reopened = Store::open(root).unwrap();
        assert_eq!(
            reopened.state.books[0].highlights[0].color.as_deref(),
            Some("amber")
        );
        assert_eq!(
            reopened.state.books[0].highlights[0].note.as_deref(),
            Some("读到这里突然安静下来")
        );
        assert_eq!(reopened.state.reading_minutes_today, 10);

        // v0.1.5 initial forest palette (#EDEEE5 / #FAF6EA / #536A49) also migrates,
        // while a user-customized forest palette is left untouched.
        let v015_root = dir.path().join("v015");
        fs::create_dir_all(&v015_root).unwrap();
        let mut v015_json = legacy_json.clone();
        v015_json["theme"]["background"] = serde_json::json!("#EDEEE5");
        v015_json["theme"]["paper"] = serde_json::json!("#FAF6EA");
        v015_json["theme"]["accent"] = serde_json::json!("#536A49");
        fs::write(
            v015_root.join("state.json"),
            serde_json::to_vec_pretty(&v015_json).unwrap(),
        )
        .unwrap();
        let migrated_v015 = Store::open(v015_root).unwrap();
        assert_eq!(migrated_v015.state.theme.paper, "#F8FCFF");
        assert_eq!(migrated_v015.state.theme.accent, "#2573B8");

        let custom_root = dir.path().join("custom-forest");
        fs::create_dir_all(&custom_root).unwrap();
        let mut custom_json = legacy_json;
        custom_json["theme"]["accent"] = serde_json::json!("#1F5A44");
        fs::write(
            custom_root.join("state.json"),
            serde_json::to_vec_pretty(&custom_json).unwrap(),
        )
        .unwrap();
        let kept_custom = Store::open(custom_root).unwrap();
        assert_eq!(kept_custom.state.theme.accent, "#1F5A44");
    }

    #[test]
    fn radio_settings_default_on_legacy_state_and_validate_custom_stations() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let mut store = Store::open(root.clone()).unwrap();
        assert_eq!(store.state.radio.current_station_id, "glow-night-talk");
        assert_eq!(store.state.radio.lamp_color, "amber");
        assert_eq!(store.state.radio.lamp_mode, "audio");

        store.state.radio.lamp_color = "starry".into();
        store.state.radio.lamp_mode = "breathe_478".into();
        store.state.radio.custom_stations.push(CustomRadioStation {
            id: "custom-1".into(),
            name: "午夜星海".into(),
            subtitle: "自定义测试流".into(),
            frequency_mhz: 95.6,
            kind: "online".into(),
            stream_url: "https://example.com/stream.mp3".into(),
            tags: vec!["ambient".into()],
        });
        store.save().unwrap();

        let reopened = Store::open(root.clone()).unwrap();
        assert_eq!(reopened.state.radio.lamp_color, "starry");
        assert_eq!(reopened.state.radio.custom_stations.len(), 1);

        // Invalid custom URL must be rejected cleanly on open
        let mut bad = reopened.state.clone();
        bad.radio.custom_stations[0].stream_url = "file:///etc/passwd".into();
        fs::write(
            root.join("state.json"),
            serde_json::to_vec_pretty(&bad).unwrap(),
        )
        .unwrap();
        assert!(Store::open(root).is_err());
    }
}

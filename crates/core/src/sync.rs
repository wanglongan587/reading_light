use crate::model::{
    Book, CustomRadioStation, Highlight, JournalEntry, SoundscapeMix, State, Theme, now,
    valid_radio_stream_url,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Portable, secret-free multi-device synchronization snapshot.
/// Explicitly NEVER includes API keys, GitHub tokens, or local machine paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncSnapshot {
    pub version: u32,
    pub updated_at: String,
    pub device_name: String,
    pub books: Vec<Book>,
    pub journal: Vec<JournalEntry>,
    pub custom_themes: Vec<Theme>,
    pub soundscape_mix: SoundscapeMix,
    pub reading_minutes_today: u32,
    pub reading_minutes_date: Option<String>,
    pub radio_favorites: Vec<String>,
    pub radio_custom_stations: Vec<CustomRadioStation>,
    pub deleted_book_ids: Vec<String>,
    pub deleted_journal_ids: Vec<String>,
    pub deleted_highlight_ids: Vec<String>,
}

impl Default for SyncSnapshot {
    fn default() -> Self {
        Self {
            version: 1,
            updated_at: now(),
            device_name: "GlowReading".into(),
            books: vec![],
            journal: vec![],
            custom_themes: vec![],
            soundscape_mix: SoundscapeMix::default(),
            reading_minutes_today: 0,
            reading_minutes_date: None,
            radio_favorites: vec![],
            radio_custom_stations: vec![],
            deleted_book_ids: vec![],
            deleted_journal_ids: vec![],
            deleted_highlight_ids: vec![],
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeReport {
    pub books_added: usize,
    pub books_updated: usize,
    pub highlights_merged: usize,
    pub journal_added: usize,
    pub themes_added: usize,
    pub changed_local: bool,
    pub changed_remote: bool,
}

impl MergeReport {
    pub fn summary(&self) -> String {
        if !self.changed_local && !self.changed_remote {
            return "本地与云端已完全一致，无需变更。".into();
        }
        let mut parts = Vec::new();
        if self.books_added > 0 {
            parts.push(format!("新增 {} 本书目", self.books_added));
        }
        if self.books_updated > 0 {
            parts.push(format!("合并 {} 本书进度", self.books_updated));
        }
        if self.highlights_merged > 0 {
            parts.push(format!("合并 {} 条划线", self.highlights_merged));
        }
        if self.journal_added > 0 {
            parts.push(format!("新增 {} 篇手账", self.journal_added));
        }
        if self.themes_added > 0 {
            parts.push(format!("同步 {} 套自定义主题", self.themes_added));
        }
        if parts.is_empty() {
            if self.changed_remote {
                "已将本地最新状态推送至云端。".into()
            } else {
                "已拉取云端最新状态。".into()
            }
        } else {
            format!("双向同步完成：{}。", parts.join("、"))
        }
    }
}

impl SyncSnapshot {
    pub fn from_state(state: &State, device_name: String) -> Self {
        Self {
            version: 1,
            updated_at: now(),
            device_name,
            books: state.books.clone(),
            journal: state.journal.clone(),
            custom_themes: state.custom_themes.clone(),
            soundscape_mix: state.soundscape_mix.clamped(),
            reading_minutes_today: state.reading_minutes_today,
            reading_minutes_date: state.reading_minutes_date.clone(),
            radio_favorites: state.radio.favorites.clone(),
            radio_custom_stations: state
                .radio
                .custom_stations
                .iter()
                .filter(|st| st.kind == "online" && valid_radio_stream_url(&st.stream_url))
                .cloned()
                .collect(),
            deleted_book_ids: state.deleted_book_ids.clone(),
            deleted_journal_ids: state.deleted_journal_ids.clone(),
            deleted_highlight_ids: state.deleted_highlight_ids.clone(),
        }
    }
}

fn union_tombstones(a: &[String], b: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for id in a.iter().chain(b.iter()) {
        if seen.insert(id.clone()) {
            out.push(id.clone());
        }
    }
    if out.len() > 500 {
        let drain = out.len() - 500;
        out.drain(0..drain);
    }
    out
}

fn merge_highlights(
    local: &[Highlight],
    remote: &[Highlight],
    deleted_ids: &HashSet<String>,
) -> (Vec<Highlight>, usize, bool) {
    let mut by_cfi: HashMap<String, Highlight> = HashMap::new();
    let mut local_ids: HashSet<String> = HashSet::new();
    for h in local {
        if deleted_ids.contains(&h.id) {
            continue;
        }
        local_ids.insert(h.id.clone());
        by_cfi.insert(h.cfi.clone(), h.clone());
    }
    let mut merged_from_remote = 0usize;
    let mut changed_local = false;
    for rh in remote {
        if deleted_ids.contains(&rh.id) {
            continue;
        }
        if let Some(existing) = by_cfi.get_mut(&rh.cfi) {
            // Enrich missing note or color without losing local note
            if existing.note.is_none() && rh.note.is_some() {
                existing.note = rh.note.clone();
                changed_local = true;
                merged_from_remote += 1;
            }
            if existing.color.is_none() && rh.color.is_some() {
                existing.color = rh.color.clone();
                changed_local = true;
            }
        } else {
            by_cfi.insert(rh.cfi.clone(), rh.clone());
            merged_from_remote += 1;
            changed_local = true;
        }
    }
    let mut result: Vec<Highlight> = by_cfi.into_values().collect();
    result.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
    if result.len() != local.len() {
        changed_local = true;
    }
    (result, merged_from_remote, changed_local)
}

fn remote_book_is_newer(local: &Book, remote: &Book) -> bool {
    match (&local.updated_at, &remote.updated_at) {
        (Some(lt), Some(rt)) if lt != rt => rt > lt,
        (None, Some(_)) => true,
        (Some(_), None) => false,
        _ => remote.progress.unwrap_or(-1.0) > local.progress.unwrap_or(-1.0),
    }
}

fn merge_single_book(
    local: &Book,
    remote: &Book,
    deleted_highlights: &HashSet<String>,
) -> (Book, usize, bool) {
    let mut merged = local.clone();
    let mut changed_local = false;

    if remote_book_is_newer(local, remote) {
        if remote.location.is_some() && remote.location != local.location {
            merged.location = remote.location.clone();
            changed_local = true;
        }
        if remote.progress.is_some() && remote.progress != local.progress {
            merged.progress = remote.progress;
            changed_local = true;
        }
        if remote.chapter.is_some() && remote.chapter != local.chapter {
            merged.chapter = remote.chapter.clone();
            changed_local = true;
        }
        if remote.updated_at.is_some() {
            merged.updated_at = remote.updated_at.clone();
        }
    }

    // Bookmarks union by CFI
    for bm in &remote.bookmarks {
        if !merged
            .bookmarks
            .iter()
            .any(|existing| existing.location.cfi == bm.location.cfi)
        {
            merged.bookmarks.push(bm.clone());
            changed_local = true;
        }
    }

    // Highlights conflict-free union
    let (highlights, hl_added, hl_changed) =
        merge_highlights(&local.highlights, &remote.highlights, deleted_highlights);
    merged.highlights = highlights;
    if hl_changed {
        changed_local = true;
    }

    // Keep fresher AI companion cache (prefer AI over offline preview, or newer timestamp)
    match (&local.companion, &remote.companion) {
        (None, Some(rc)) => {
            merged.companion = Some(rc.clone());
            changed_local = true;
        }
        (Some(lc), Some(rc)) => {
            let prefer_remote = (!lc.is_ai && rc.is_ai)
                || (lc.is_ai == rc.is_ai && rc.generated_at > lc.generated_at);
            if prefer_remote {
                merged.companion = Some(rc.clone());
                changed_local = true;
            }
        }
        _ => {}
    }

    (merged, hl_added, changed_local)
}

/// Deterministically merge local `State` with a remote `SyncSnapshot` without ever losing
/// concurrent highlights, bookmarks, journal entries, or newer reading progress.
pub fn merge_state_with_snapshot(local: &State, remote: &SyncSnapshot) -> (State, MergeReport) {
    let mut merged = local.clone();
    let mut report = MergeReport::default();

    let deleted_books = union_tombstones(&local.deleted_book_ids, &remote.deleted_book_ids);
    let deleted_journals =
        union_tombstones(&local.deleted_journal_ids, &remote.deleted_journal_ids);
    let deleted_highlights =
        union_tombstones(&local.deleted_highlight_ids, &remote.deleted_highlight_ids);

    let del_books_set: HashSet<String> = deleted_books.iter().cloned().collect();
    let del_journals_set: HashSet<String> = deleted_journals.iter().cloned().collect();
    let del_highlights_set: HashSet<String> = deleted_highlights.iter().cloned().collect();

    // 1. Merge books
    let local_books_map: HashMap<String, Book> = local
        .books
        .iter()
        .map(|b| (b.id.clone(), b.clone()))
        .collect();
    let remote_books_map: HashMap<String, Book> = remote
        .books
        .iter()
        .map(|b| (b.id.clone(), b.clone()))
        .collect();

    let mut next_books = Vec::new();
    let mut handled = HashSet::new();

    for lb in &local.books {
        handled.insert(lb.id.clone());
        if del_books_set.contains(&lb.id) && !remote_books_map.contains_key(&lb.id) {
            report.changed_local = true;
            continue;
        }
        if let Some(rb) = remote_books_map.get(&lb.id) {
            let (book, hl_added, local_changed) = merge_single_book(lb, rb, &del_highlights_set);
            if local_changed {
                report.books_updated += 1;
                report.changed_local = true;
            }
            report.highlights_merged += hl_added;
            // Check if local had data remote didn't have
            if book.highlights.len() > rb.highlights.len()
                || book.bookmarks.len() > rb.bookmarks.len()
                || book.location != rb.location
                || book.progress != rb.progress
            {
                report.changed_remote = true;
            }
            next_books.push(book);
        } else {
            // Local-only book → will be pushed to remote
            let mut book = lb.clone();
            book.highlights
                .retain(|h| !del_highlights_set.contains(&h.id));
            report.changed_remote = true;
            next_books.push(book);
        }
    }

    for rb in &remote.books {
        if handled.contains(&rb.id) || del_books_set.contains(&rb.id) {
            continue;
        }
        let mut book = rb.clone();
        book.highlights
            .retain(|h| !del_highlights_set.contains(&h.id));
        report.books_added += 1;
        report.highlights_merged += book.highlights.len();
        report.changed_local = true;
        next_books.push(book);
    }

    if local_books_map.len() != remote_books_map.len() {
        report.changed_remote = true;
    }
    merged.books = next_books;

    // 2. Merge journal entries (union by ID + tombstones, sorted newest-first)
    let mut journal_map: HashMap<String, JournalEntry> = HashMap::new();
    let local_journal_ids: HashSet<String> = local.journal.iter().map(|e| e.id.clone()).collect();
    let remote_journal_ids: HashSet<String> = remote.journal.iter().map(|e| e.id.clone()).collect();

    for entry in &local.journal {
        if !del_journals_set.contains(&entry.id) {
            journal_map.insert(entry.id.clone(), entry.clone());
        } else {
            report.changed_local = true;
        }
    }
    for entry in &remote.journal {
        if del_journals_set.contains(&entry.id) {
            continue;
        }
        if let Some(existing) = journal_map.get_mut(&entry.id) {
            if entry.favorite && !existing.favorite {
                existing.favorite = true;
                report.changed_local = true;
            }
        } else {
            journal_map.insert(entry.id.clone(), entry.clone());
            report.journal_added += 1;
            report.changed_local = true;
        }
    }
    if local_journal_ids.iter().any(|id| !remote_journal_ids.contains(id)) {
        report.changed_remote = true;
    }
    let mut next_journal: Vec<JournalEntry> = journal_map.into_values().collect();
    next_journal.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| a.id.cmp(&b.id)));
    merged.journal = next_journal;

    // 3. Merge custom themes (union by ID)
    let local_theme_ids: HashSet<String> =
        local.custom_themes.iter().map(|t| t.id.clone()).collect();
    let remote_theme_ids: HashSet<String> =
        remote.custom_themes.iter().map(|t| t.id.clone()).collect();
    for rt in &remote.custom_themes {
        if !local_theme_ids.contains(&rt.id) && crate::theme::validate(rt).is_ok() {
            merged.custom_themes.push(rt.clone());
            report.themes_added += 1;
            report.changed_local = true;
        }
    }
    if local_theme_ids.iter().any(|id| !remote_theme_ids.contains(id)) {
        report.changed_remote = true;
    }

    // 4. Merge reading minutes (never lose today's reading time)
    match (&local.reading_minutes_date, &remote.reading_minutes_date) {
        (Some(ld), Some(rd)) if ld == rd => {
            let best = local.reading_minutes_today.max(remote.reading_minutes_today);
            if best != local.reading_minutes_today {
                merged.reading_minutes_today = best;
                report.changed_local = true;
            }
            if best != remote.reading_minutes_today {
                report.changed_remote = true;
            }
        }
        (Some(ld), Some(rd)) if rd > ld => {
            merged.reading_minutes_date = Some(rd.clone());
            merged.reading_minutes_today = remote.reading_minutes_today;
            report.changed_local = true;
        }
        (None, Some(rd)) => {
            merged.reading_minutes_date = Some(rd.clone());
            merged.reading_minutes_today = remote.reading_minutes_today;
            report.changed_local = true;
        }
        (Some(_), _) => {
            report.changed_remote = true;
        }
        _ => {}
    }

    // 5. Merge online custom radio stations & favorite station IDs (never local machine folders)
    let local_favs: HashSet<String> = local.radio.favorites.iter().cloned().collect();
    let remote_favs: HashSet<String> = remote.radio_favorites.iter().cloned().collect();
    for fav in &remote.radio_favorites {
        if !fav.trim().is_empty() && !local_favs.contains(fav) {
            merged.radio.favorites.push(fav.clone());
            report.changed_local = true;
        }
    }
    if local_favs.iter().any(|id| !remote_favs.contains(id)) {
        report.changed_remote = true;
    }

    let local_station_ids: HashSet<String> = local
        .radio
        .custom_stations
        .iter()
        .map(|s| s.id.clone())
        .collect();
    let remote_station_ids: HashSet<String> = remote
        .radio_custom_stations
        .iter()
        .map(|s| s.id.clone())
        .collect();
    for st in &remote.radio_custom_stations {
        if st.kind == "online"
            && !st.id.trim().is_empty()
            && !st.name.trim().is_empty()
            && (88.0..=108.0).contains(&st.frequency_mhz)
            && valid_radio_stream_url(&st.stream_url)
            && !local_station_ids.contains(&st.id)
        {
            merged.radio.custom_stations.push(st.clone());
            report.changed_local = true;
        }
    }
    if local
        .radio
        .custom_stations
        .iter()
        .any(|s| s.kind == "online" && !remote_station_ids.contains(&s.id))
    {
        report.changed_remote = true;
    }

    merged.deleted_book_ids = deleted_books;
    merged.deleted_journal_ids = deleted_journals;
    merged.deleted_highlight_ids = deleted_highlights;

    (merged, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Bookmark, EchoResult, Location};

    fn sample_book(id: &str, progress: f32, updated_at: &str) -> Book {
        Book {
            id: id.into(),
            title: "百年孤独".into(),
            author: "马尔克斯".into(),
            format: "epub".into(),
            imported_at: "2026-11-20T00:00:00Z".into(),
            cover: None,
            location: Some(Location {
                book_id: id.into(),
                cfi: format!("epubcfi(/6/4!/4/{})", (progress * 100.0) as u32),
            }),
            bookmarks: vec![],
            highlights: vec![],
            progress: Some(progress),
            chapter: Some("第二章".into()),
            encoding: None,
            updated_at: Some(updated_at.into()),
            companion: None,
        }
    }

    #[test]
    fn merges_two_devices_without_losing_highlights_journal_or_newer_progress() {
        let book_id = "a".repeat(64);
        let mut local = State::default();
        let mut lb = sample_book(&book_id, 0.25, "2026-11-26T08:00:00Z");
        lb.highlights.push(Highlight {
            id: "hl-local".into(),
            text: "本地划线".into(),
            cfi: "epubcfi(/6/2!/4/2,/1:0,/1:4)".into(),
            created_at: "2026-11-26T08:00:00Z".into(),
            color: Some("amber".into()),
            note: Some("本地笔记".into()),
        });
        local.books.push(lb);
        local.reading_minutes_date = Some("2026-11-26".into());
        local.reading_minutes_today = 25;

        let mut remote_state = State::default();
        let mut rb = sample_book(&book_id, 0.48, "2026-11-26T10:00:00Z");
        rb.chapter = Some("第五章".into());
        rb.highlights.push(Highlight {
            id: "hl-remote".into(),
            text: "另一台设备的划线".into(),
            cfi: "epubcfi(/6/6!/4/2,/1:0,/1:8)".into(),
            created_at: "2026-11-26T09:30:00Z".into(),
            color: Some("sage".into()),
            note: None,
        });
        rb.bookmarks.push(Bookmark {
            label: "书签 1".into(),
            location: Location {
                book_id: book_id.clone(),
                cfi: "epubcfi(/6/6!/4/2/1:0)".into(),
            },
        });
        remote_state.books.push(rb);
        remote_state.journal.push(JournalEntry {
            id: "j-remote".into(),
            created_at: "2026-11-26T09:45:00Z".into(),
            kind: "mood".into(),
            input: "在咖啡馆读了半小时".into(),
            mood: Some("安静".into()),
            echo: EchoResult {
                quotes: vec![],
                reply: "很好。".into(),
                demo: false,
            },
            location: None,
            favorite: true,
        });
        remote_state.reading_minutes_date = Some("2026-11-26".into());
        remote_state.reading_minutes_today = 40;

        let snapshot = SyncSnapshot::from_state(&remote_state, "MacBook".into());
        // Verify secrets never appear in serialized snapshot JSON
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(!serialized.contains("github_token"));
        assert!(!serialized.contains("ai_api_key"));

        let (merged, report) = merge_state_with_snapshot(&local, &snapshot);
        assert_eq!(merged.books.len(), 1);
        assert_eq!(merged.books[0].progress, Some(0.48));
        assert_eq!(merged.books[0].chapter.as_deref(), Some("第五章"));
        assert_eq!(merged.books[0].highlights.len(), 2);
        assert_eq!(merged.books[0].bookmarks.len(), 1);
        assert_eq!(merged.journal.len(), 1);
        assert_eq!(merged.reading_minutes_today, 40);
        assert!(report.changed_local);
        assert!(report.changed_remote);
        assert_eq!(report.highlights_merged, 1);
        assert_eq!(report.journal_added, 1);
    }

    #[test]
    fn tombstones_prevent_deleted_highlights_from_resurrecting() {
        let book_id = "b".repeat(64);
        let mut local = State::default();
        local.books.push(sample_book(&book_id, 0.3, "2026-11-26T09:00:00Z"));
        local.deleted_highlight_ids.push("hl-dead".into());

        let mut remote_state = State::default();
        let mut rb = sample_book(&book_id, 0.3, "2026-11-26T08:00:00Z");
        rb.highlights.push(Highlight {
            id: "hl-dead".into(),
            text: "已删除划线".into(),
            cfi: "epubcfi(/6/2!/4/2,/1:0,/1:5)".into(),
            created_at: "2026-11-26T07:00:00Z".into(),
            color: None,
            note: None,
        });
        remote_state.books.push(rb);

        let snapshot = SyncSnapshot::from_state(&remote_state, "PC2".into());
        let (merged, _) = merge_state_with_snapshot(&local, &snapshot);
        assert!(merged.books[0].highlights.is_empty());
    }

    #[test]
    fn syncs_online_radio_favorites_and_custom_stations_without_local_paths() {
        let mut local = State::default();
        local.radio.local_audio_folder = Some("C:\\Users\\Me\\PrivateMusic".into());

        let mut remote_state = State::default();
        remote_state.radio.local_audio_folder = Some("/Users/remote/Secret".into());
        remote_state.radio.favorites.push("classical-nocturne".into());
        remote_state.radio.custom_stations.push(CustomRadioStation {
            id: "custom-jazz".into(),
            name: "东京深夜爵士".into(),
            subtitle: "温暖萨克斯".into(),
            frequency_mhz: 92.8,
            kind: "online".into(),
            stream_url: "https://example.com/tokyo-jazz.mp3".into(),
            tags: vec!["jazz".into()],
        });

        let snapshot = SyncSnapshot::from_state(&remote_state, "Laptop".into());
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(!serialized.contains("Secret"));
        assert!(!serialized.contains("local_audio_folder"));

        let (merged, report) = merge_state_with_snapshot(&local, &snapshot);
        assert!(report.changed_local);
        assert!(merged.radio.favorites.iter().any(|f| f == "classical-nocturne"));
        assert_eq!(merged.radio.custom_stations.len(), 1);
        assert_eq!(
            merged.radio.local_audio_folder.as_deref(),
            Some("C:\\Users\\Me\\PrivateMusic")
        );
    }
}

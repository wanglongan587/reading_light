use super::*;

impl ReadingApp {
    pub(super) fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fading_audio.retain(|(_, sink)| !sink.empty());
        if self
            .chime_audio
            .as_ref()
            .is_some_and(|(_, sink)| sink.empty())
        {
            self.chime_audio = None;
        }
        if let Some(result) = self.mood_rx.as_ref().map(|rx| rx.try_recv()) {
            match result {
                Ok(result) => {
                    self.mood_rx = None;
                    self.finish_mood(result, window, cx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.mood_rx = None;
                    self.error = Some("回复任务已中断，文字已保留，请重试。".into());
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(result) = self.echo_ai_rx.as_ref().map(|rx| rx.try_recv()) {
            match result {
                Ok(result) => {
                    self.echo_ai_rx = None;
                    self.finish_ai_passage(result, cx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.echo_ai_rx = None;
                    self.error = Some("AI 共读任务已中断，可以稍后重试。".into());
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(started) = self.breathing_started {
            if started.elapsed() >= Duration::from_secs(60) {
                self.breathing_started = None;
                let origin = self.pet_celebration_origin(window);
                self.motion
                    .trigger_pet_celebration(origin, self.theme().reduced_motion);
                self.notice = Some("已完成 1 分钟 4-7-8 呼吸放松，愿此刻慢下来。".into());
            }
            cx.notify();
        }
        if let Some(started) = self.focus_timer_started {
            let elapsed_secs = started.elapsed().as_secs();
            let total_secs = self.focus_timer_total_secs as u64;
            if elapsed_secs >= total_secs {
                self.focus_timer_started = None;
                let mins = (self.focus_timer_total_secs / 60).max(1);
                let today = today_ymd();
                let total_today = self
                    .store
                    .add_reading_minutes(mins, &today)
                    .unwrap_or(self.store.state.reading_minutes_today);
                self.play_chime();
                let origin = self.pet_celebration_origin(window);
                self.motion
                    .trigger_pet_celebration(origin, self.theme().reduced_motion);
                self.notice = Some(format!(
                    "棱镜喵：静读光瓶已蓄满 {mins} 分钟，今天共静心阅读 {total_today} 分钟。"
                ));
                if self.store.state.tts.sleep_with_timer {
                    if self.tts_status.active
                        && let Some(player) = &self.tts_player
                    {
                        player.stop();
                        player.set_volume(1.0);
                    }
                    if self.audio.is_some() {
                        self.stop_audio_smoothly();
                    }
                    self.notice = Some(format!(
                        "棱镜喵：静读光瓶已蓄满 {mins} 分钟，声音已轻声渐弱淡出，晚安。"
                    ));
                }
            } else if self.store.state.tts.sleep_with_timer {
                let remaining = total_secs.saturating_sub(elapsed_secs);
                if remaining <= 60 {
                    let fade = (remaining as f32 / 60.0).clamp(0.08, 1.0);
                    if let Some(player) = &self.tts_player
                        && self.tts_status.active
                    {
                        player.set_volume(fade);
                    }
                    if let Some((_, sink, _)) = &self.audio {
                        let base = self.theme().volume.clamp(0.05, 1.0);
                        sink.set_volume((base * fade).clamp(0.02, 1.0));
                    }
                }
            }
            cx.notify();
        }
        self.tick_features(cx);
        self.tick_radio(cx);
        let messages = self
            .rx
            .as_ref()
            .map(|rx| rx.try_iter().take(64).collect::<Vec<_>>())
            .unwrap_or_default();
        let changed = !messages.is_empty();
        for message in messages {
            if let Ok(event) = glow_core::bridge::parse_event(&message, &self.session) {
                if event.request_id <= self.last_event {
                    continue;
                }
                self.last_event = event.request_id;
                if event.kind == "palette" {
                    self.act(Action::TogglePalette, window, cx);
                    continue;
                }
                if event.kind == "escape" {
                    self.handle_escape(window, cx);
                    continue;
                }
                let open_note = event.kind == "highlight" && event.open_note;
                let celebrated_highlight = event.kind == "highlight";
                self.event(event, cx);
                if celebrated_highlight && self.error.is_none() {
                    let origin = self.pet_celebration_origin(window);
                    self.motion
                        .trigger_pet_celebration(origin, self.theme().reduced_motion);
                }
                if open_note
                    && self.error.is_none()
                    && let Some(target_id) = self.highlight_note_target.clone()
                {
                    let existing = self
                        .store
                        .state
                        .books
                        .iter()
                        .find(|b| Some(&b.id) == self.active_book.as_ref())
                        .and_then(|b| b.highlights.iter().find(|h| h.id == target_id))
                        .and_then(|h| h.note.clone())
                        .unwrap_or_default();
                    self.highlight_note_input
                        .update(cx, |s, cx| s.set_value(existing, window, cx));
                    let focus = self.highlight_note_input.read(cx).focus_handle(cx).clone();
                    window.focus(&focus, cx);
                }
            }
        }
        if self.dirty && self.last_save.elapsed() > Duration::from_millis(800) {
            self.save();
        }
        self.recover_stalled_reader(window, cx);
        if self.smoke {
            self.smoke_tick(window, cx);
        }
        if changed {
            cx.notify();
        }
    }
    pub(super) fn recover_stalled_reader(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let stalled = self
            .reader_load_started
            .is_some_and(|started| started.elapsed() > Duration::from_secs(20));
        if self.page != Page::Reader
            || self.opened
            || self.busy
            || self.confirm.is_some()
            || !stalled
        {
            return;
        }
        self.reader_load_started = None;
        if self.reader_retry_used {
            self.error =
                Some("这本书的书页加载超时。请检查 EPUB 是否完整，或返回书架后重新打开。".into());
            cx.notify();
            return;
        }
        let Some(book_id) = self.active_book.clone() else {
            return;
        };
        self.error = None;
        self.notice = Some("书页连接较慢，正在重新打开一次…".into());
        match self.open(&book_id, window, cx) {
            Ok(()) => {
                self.reader_retry_used = true;
                cx.notify();
            }
            Err(error) => {
                self.reader_load_started = None;
                self.error = Some(format!("重新打开书页失败：{error}"));
                cx.notify();
            }
        }
    }
    pub(super) fn event(&mut self, event: ReaderEvent, cx: &mut Context<Self>) {
        match event.kind.as_str() {
            "ready" => {
                self.reader_load_started = Some(Instant::now());
                let (cfi, highlights) = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                    .map(|book| {
                        (
                            book.location.as_ref().map(|location| location.cfi.clone()),
                            book.highlights
                                .iter()
                                .map(|highlight| {
                                    json!({
                                        "id": highlight.id,
                                        "cfi": highlight.cfi,
                                        "text": highlight.text,
                                        "color": highlight.color,
                                    })
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .unwrap_or_default();
                self.send(json!({"kind":"open","url":format!("{}/book.epub",platform::origin()),"cfi":cfi,"highlights":highlights,"theme":self.theme_json(),"mode":self.store.state.mode}),cx);
            }
            "opened" => {
                self.error = None;
                self.opened = true;
                self.reader_load_started = None;
                self.reader_retry_used = false;
            }
            "toc" => self.toc = event.entries,
            "location" => {
                if let Some(book) = self
                    .store
                    .state
                    .books
                    .iter_mut()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                {
                    book.location = Some(Location {
                        book_id: book.id.clone(),
                        cfi: event.cfi,
                    });
                    if let Some(progress) = event.progress {
                        book.progress = Some(progress.clamp(0.0, 1.0));
                    }
                    if !event.chapter.is_empty() {
                        book.chapter = Some(event.chapter.clone());
                    }
                    book.updated_at = Some(glow_core::model::now());
                    self.dirty = true;
                }
            }
            "echo" => {
                let result = echo::respond(&event.text, None);
                let normalized_input = event.text.trim().to_owned();
                self.echo_memories = self
                    .store
                    .recall_related_memories(&normalized_input, 6)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|hit| hit.excerpt.trim() != normalized_input)
                    .take(3)
                    .collect();
                self.echo_input = event.text;
                self.echo_cfi = event.cfi;
                self.echo = Some(result);
                self.echo_saved = false;
                self.echo_ai_rx = None;
                self.drawer = "echo".into();
                self.spoilers.clear();
            }
            "footnote" => {
                let term = event.text.trim().to_owned();
                self.footnote_dict = dictionary::lookup(&term);
                self.footnote_term = term;
                self.footnote_ai_reply = None;
                self.footnote_ai_rx = None;
                self.drawer = "footnote".into();
            }
            "highlight" => {
                let Some(book_id) = self.active_book.clone() else {
                    return;
                };
                let cfi = event.cfi.clone();
                let previous = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| b.id == book_id)
                    .and_then(|b| b.highlights.iter().find(|h| h.cfi == cfi))
                    .cloned();
                let color = (!event.color.is_empty()).then_some(event.color);
                match self.store.add_highlight_styled(
                    &book_id,
                    event.text,
                    cfi.clone(),
                    color,
                    None,
                ) {
                    Ok((highlight, inserted)) => {
                        self.error = None;
                        self.send(
                            json!({
                                "kind": "highlight",
                                "highlight": {
                                    "id": highlight.id,
                                    "cfi": highlight.cfi,
                                    "text": highlight.text,
                                    "color": highlight.color,
                                }
                            }),
                            cx,
                        );
                        if event.open_note {
                            self.drawer = "highlights".into();
                            self.highlight_note_target = Some(highlight.id);
                            self.notice = Some("棱镜喵：已为你定位划线，写下此刻想法吧。".into());
                        } else if inserted {
                            self.notice = Some("棱镜喵：这句微光已经悄悄划下来啦。".into());
                        } else {
                            self.notice = Some("已更新这条划线的颜色。".into());
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        // Roll back any optimistic highlight rendered by the WebView
                        if let Some(prev) = previous {
                            self.send(
                                json!({
                                    "kind": "highlight",
                                    "highlight": {
                                        "id": prev.id,
                                        "cfi": prev.cfi,
                                        "text": prev.text,
                                        "color": prev.color,
                                    }
                                }),
                                cx,
                            );
                        } else if !cfi.is_empty() {
                            self.send(json!({"kind": "unhighlight", "cfi": cfi}), cx);
                        }
                    }
                }
            }
            "copy" => {
                let title = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                    .map(|b| b.title.as_str())
                    .unwrap_or("微光藏书");
                let excerpt = format!("“{}”\n——《{}》", event.text.trim(), title);
                cx.write_to_clipboard(ClipboardItem::new_string(excerpt));
                self.notice = Some("已复制带书名出处的摘录到剪贴板。".into());
            }
            "edge" => {
                if self.focus_mode && self.page == Page::Reader {
                    match event.text.as_str() {
                        "top" => {
                            self.zen_reveal_top = true;
                            self.zen_reveal_bottom = false;
                            self.zen_revealed_at = Some(Instant::now());
                        }
                        "bottom" => {
                            self.zen_reveal_bottom = true;
                            self.zen_reveal_top = false;
                            self.zen_revealed_at = Some(Instant::now());
                        }
                        _ => {
                            if self
                                .zen_revealed_at
                                .is_none_or(|t| t.elapsed() >= Duration::from_millis(320))
                            {
                                self.zen_reveal_top = false;
                                self.zen_reveal_bottom = false;
                            }
                        }
                    }
                }
            }
            "search" => {
                self.searching = false;
                self.search_results = event.results;
            }
            "escape" => {
                if self.palette_open {
                    self.palette_open = false;
                } else if self.quote_card.is_some() {
                    self.quote_card = None;
                } else if self.highlight_note_target.is_some() {
                    self.highlight_note_target = None;
                } else if !self.drawer.is_empty() {
                    self.drawer.clear();
                } else {
                    self.focus_mode = false;
                    self.zen_reveal_top = false;
                    self.zen_reveal_bottom = false;
                }
            }
            "error" => {
                self.error = Some(event.text);
                self.opened = false;
                self.reader_load_started = None;
            }
            _ => {}
        }
    }
    pub(super) fn act(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.error = None;
        self.notice = None;
        if let Err(e) = self.perform(action, window, cx) {
            self.error = Some(e.to_string());
        }
        cx.notify();
    }
    pub(super) fn perform(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        match action {
            Action::Page(page) => {
                if page == Page::Reader && self.active_book.is_none() {
                    let target = self
                        .store
                        .state
                        .last_book
                        .clone()
                        .filter(|id| self.store.state.books.iter().any(|b| &b.id == id))
                        .or_else(|| self.store.state.books.first().map(|b| b.id.clone()));
                    if let Some(book_id) = target {
                        self.open(&book_id, window, cx)?;
                        return Ok(());
                    }
                    self.page = Page::Shelf;
                    self.shelf_tab = ShelfTab::Catalog;
                    self.notice = Some("书架上还没有书，从微光公版馆挑一本翻开吧。".into());
                    return Ok(());
                }
                let was_previewing = self.preview.is_some();
                let entering_radio = page == Page::Radio && self.page != Page::Radio;
                self.page = page.clone();
                self.preview = None;
                self.draft = None;
                self.radio_dial_dragging = false;
                self.radio_drag_mhz = None;
                self.radio_vol_dragging = false;
                self.install_native_theme(cx);
                if was_previewing && self.opened {
                    self.configure(cx);
                }
                if page == Page::Radio {
                    if entering_radio {
                        self.radio_hide_sidebar = true;
                    }
                    self.radio_dial_display_mhz =
                        if self.radio_status.playing && self.radio_status.frequency_mhz > 0.0 {
                            self.radio_status.frequency_mhz
                        } else {
                            self.current_radio_station().frequency_mhz
                        };
                    self.radio_last_motion_at = Instant::now();
                    self.schedule_motion_frame(window, cx);
                }
                self.save();
            }
            Action::Import => {
                self.hide(cx);
                self.busy = true;
                let dialog = rfd::AsyncFileDialog::new()
                    .set_parent(window)
                    .set_title("选择要带回家的书")
                    .add_filter("电子书", &["epub", "txt"]);
                cx.spawn_in(window, async move |this, cx| {
                    let files = dialog.pick_files().await;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(files) = files {
                            this.begin_import(
                                files.into_iter().map(|f| f.path().to_owned()).collect(),
                                cx,
                            );
                        } else {
                            this.busy = false;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
            Action::Open(id) => self.open(&id, window, cx)?,
            Action::OpenAt(target) => {
                let (id, cfi) = *target;
                if let Some(book) = self.store.state.books.iter_mut().find(|b| b.id == id) {
                    book.location = Some(Location {
                        book_id: id.clone(),
                        cfi,
                    });
                }
                self.open(&id, window, cx)?;
            }
            Action::Remove(id) => self.confirm = Some(format!("book:{id}")),
            Action::ConfirmRemove => {
                if let Some(value) = self.confirm.take()
                    && let Some(id) = value.strip_prefix("book:")
                {
                    self.store.remove_book(id)?;
                    if self.active_book.as_deref() == Some(id) {
                        self.hide(cx);
                        self.webview = None;
                        self.active_book = None;
                    }
                }
            }
            Action::Cancel => self.confirm = None,
            Action::Mode => {
                self.store.state.mode = if self.store.state.mode == "scrolled" {
                    "paginated"
                } else {
                    "scrolled"
                }
                .into();
                self.configure(cx);
                self.save();
            }
            Action::Prev => self.send(json!({"kind":"prev"}), cx),
            Action::Next => self.send(json!({"kind":"next"}), cx),
            Action::Focus => {
                self.focus_mode = !self.focus_mode;
                self.zen_reveal_top = false;
                self.zen_reveal_bottom = false;
                self.zen_revealed_at = None;
            }
            Action::Toc => self.drawer = if self.drawer == "toc" { "" } else { "toc" }.into(),
            Action::Search => {
                if self.drawer == "search" {
                    self.drawer.clear();
                } else {
                    self.drawer = "search".into();
                    let focus = self.search_input.read(cx).focus_handle(cx).clone();
                    window.focus(&focus, cx);
                }
            }
            Action::SubmitSearch => {
                anyhow::ensure!(self.opened, "书页还在整理，请稍后再搜索");
                let query = self.search_input.read(cx).value().trim().to_owned();
                anyhow::ensure!(!query.is_empty(), "请输入要搜索的文字");
                anyhow::ensure!(query.chars().count() <= 100, "搜索文字请控制在 100 字以内");
                self.search_results.clear();
                self.searching = true;
                self.send(json!({"kind":"search","query":query}), cx);
            }
            Action::Highlights => {
                self.highlight_note_target = None;
                self.drawer = if self.drawer == "highlights" {
                    ""
                } else {
                    "highlights"
                }
                .into();
            }
            Action::Typography => {
                self.drawer = if self.drawer == "type" { "" } else { "type" }.into();
            }
            Action::TypeSize(delta) => {
                let current = self
                    .store
                    .state
                    .reader_font_size
                    .unwrap_or_else(|| self.theme().font_size);
                self.store.state.reader_font_size =
                    Some((current + delta as f32).clamp(14.0, 36.0));
                self.configure(cx);
                self.save();
            }
            Action::TypeLeading(delta) => {
                let current = self
                    .store
                    .state
                    .reader_line_height
                    .unwrap_or_else(|| self.theme().line_height);
                let next = (current + delta as f32 * 0.1).clamp(1.3, 2.6);
                self.store.state.reader_line_height = Some((next * 10.0).round() / 10.0);
                self.configure(cx);
                self.save();
            }
            Action::TypeWidth(delta) => {
                let current = self
                    .store
                    .state
                    .reader_width
                    .unwrap_or_else(|| self.theme().width);
                self.store.state.reader_width =
                    Some((current as i32 + delta as i32 * 40).clamp(400, 1100) as u32);
                self.configure(cx);
                self.save();
            }
            Action::ToggleWarmFilter => {
                self.store.state.warm_filter = !self.store.state.warm_filter;
                self.configure(cx);
                self.save();
                self.notice = Some(if self.store.state.warm_filter {
                    "已开启夜间暖光护眼滤镜。".into()
                } else {
                    "已关闭暖光护眼滤镜。".into()
                });
            }
            Action::ResetTypography => {
                self.store.state.reader_font_size = None;
                self.store.state.reader_line_height = None;
                self.store.state.reader_width = None;
                self.store.state.warm_filter = false;
                self.configure(cx);
                self.save();
                self.notice = Some("已恢复当前主题的默认排版。".into());
            }
            Action::Highlight => {
                let book_id = self
                    .active_book
                    .clone()
                    .context("请先打开一本可以划线的书")?;
                anyhow::ensure!(!self.echo_cfi.is_empty(), "请先选择一段文字");
                let (highlight, inserted) = self.store.add_highlight_styled(
                    &book_id,
                    self.echo_input.clone(),
                    self.echo_cfi.clone(),
                    Some("amber".into()),
                    None,
                )?;
                self.send(
                    json!({
                        "kind": "highlight",
                        "highlight": {
                            "id": highlight.id,
                            "cfi": highlight.cfi,
                            "text": highlight.text,
                            "color": highlight.color,
                        }
                    }),
                    cx,
                );
                if inserted {
                    let origin = self.pet_celebration_origin(window);
                    self.motion
                        .trigger_pet_celebration(origin, self.theme().reduced_motion);
                    self.notice = Some("已留下划线，之后可以在「划线」里找回。".into());
                } else {
                    self.notice = Some("这段文字已经留过划线。".into());
                }
            }
            Action::HighlightColor(color) => {
                let book_id = self
                    .active_book
                    .clone()
                    .context("请先打开一本可以划线的书")?;
                anyhow::ensure!(!self.echo_cfi.is_empty(), "请先选择一段文字");
                let (highlight, inserted) = self.store.add_highlight_styled(
                    &book_id,
                    self.echo_input.clone(),
                    self.echo_cfi.clone(),
                    Some(color),
                    None,
                )?;
                self.send(
                    json!({
                        "kind": "highlight",
                        "highlight": {
                            "id": highlight.id,
                            "cfi": highlight.cfi,
                            "text": highlight.text,
                            "color": highlight.color,
                        }
                    }),
                    cx,
                );
                let origin = self.pet_celebration_origin(window);
                self.motion
                    .trigger_pet_celebration(origin, self.theme().reduced_motion);
                self.notice = Some(if inserted {
                    "棱镜喵：已按你挑选的颜色留住这句微光。".into()
                } else {
                    "已更新这条划线的颜色。".into()
                });
            }
            Action::RecolorHighlight(pair) => {
                let (id, color) = *pair;
                let Some(book_id) = self.active_book.clone() else {
                    return Ok(());
                };
                if let Some(highlight) =
                    self.store
                        .update_highlight(&book_id, &id, Some(Some(color)), None)?
                {
                    self.send(
                        json!({
                            "kind": "highlight",
                            "highlight": {
                                "id": highlight.id,
                                "cfi": highlight.cfi,
                                "text": highlight.text,
                                "color": highlight.color,
                            }
                        }),
                        cx,
                    );
                    self.notice = Some("已更新划线颜色。".into());
                }
            }
            Action::EditHighlightNote(id) => {
                let existing = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                    .and_then(|b| b.highlights.iter().find(|h| h.id == id))
                    .and_then(|h| h.note.clone())
                    .unwrap_or_default();
                self.highlight_note_target = Some(id);
                self.highlight_note_input
                    .update(cx, |s, cx| s.set_value(existing, window, cx));
                let focus = self.highlight_note_input.read(cx).focus_handle(cx).clone();
                window.focus(&focus, cx);
            }
            Action::SaveHighlightNote => {
                let Some(book_id) = self.active_book.clone() else {
                    return Ok(());
                };
                let Some(target_id) = self.highlight_note_target.clone() else {
                    return Ok(());
                };
                let note = self.highlight_note_input.read(cx).value().to_string();
                self.store
                    .update_highlight(&book_id, &target_id, None, Some(Some(note)))?;
                self.highlight_note_target = None;
                self.notice = Some("批注已保存。".into());
            }
            Action::CancelHighlightNote => {
                self.highlight_note_target = None;
            }
            Action::DeleteHighlight(id) => {
                let Some(book_id) = self.active_book.clone() else {
                    return Ok(());
                };
                if self.highlight_note_target.as_deref() == Some(id.as_str()) {
                    self.highlight_note_target = None;
                }
                if let Some(highlight) = self.store.remove_highlight(&book_id, &id)? {
                    self.send(json!({"kind":"unhighlight","cfi":highlight.cfi}), cx);
                    self.notice = Some("已删除这条划线，书页内容不受影响。".into());
                }
            }
            Action::SaveEchoToJournal => {
                let echo = self.echo.clone().context("当前没有可保存的回响")?;
                anyhow::ensure!(!self.echo_saved, "这条回响已经存入私人日记了");
                let location = self
                    .active_book
                    .as_ref()
                    .filter(|_| glow_core::model::valid_cfi(&self.echo_cfi))
                    .map(|id| Location {
                        book_id: id.clone(),
                        cfi: self.echo_cfi.clone(),
                    });
                self.store
                    .record("selection", self.echo_input.clone(), None, echo, location)?;
                self.echo_saved = true;
                self.notice = Some("已存入私人微光手账。".into());
            }
            Action::AskAiPassage => {
                if self.echo_ai_rx.is_some() {
                    return Ok(());
                }
                let passage = self.echo_input.trim().to_owned();
                anyhow::ensure!(!passage.is_empty(), "请先在书中选中一段文字");
                let book_id = self.active_book.clone();
                let cfi = self.echo_cfi.clone();
                let book_title = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == book_id.as_ref())
                    .map(|b| b.title.clone())
                    .unwrap_or_else(|| "微光藏书".into());
                let worker = self.echo_ai_worker;
                let use_custom = self.custom_mood_worker_override
                    || (worker as *const () as usize)
                        != (mood::generate_passage as *const () as usize);
                let ai_settings = self.store.state.ai.clone();
                let secrets = self.store.load_secrets();
                let local_memories = self.echo_memories.clone();
                let (sender, receiver) = mpsc::channel();
                self.echo_ai_rx = Some(receiver);
                std::thread::spawn(move || {
                    let generated = if use_custom {
                        worker(&passage, &book_title)
                    } else {
                        mood::generate_passage_with_memories(
                            &ai_settings,
                            &secrets,
                            &passage,
                            &book_title,
                            &local_memories,
                        )
                    };
                    let outcome = generated.map(|result| PassageGenerated {
                        book_id,
                        passage,
                        cfi,
                        result,
                    });
                    let _ = sender.send(outcome);
                });
            }
            Action::CancelAiPassage => {
                self.echo_ai_rx = None;
                self.notice = Some("已停止等待 AI 段落共读。已发出的请求可能仍会计费。".into());
            }
            Action::Bookmarks => {
                self.drawer = if self.drawer == "bookmarks" {
                    ""
                } else {
                    "bookmarks"
                }
                .into()
            }
            Action::Bookmark => {
                if let Some(b) = self
                    .store
                    .state
                    .books
                    .iter_mut()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                    && let Some(location) = &b.location
                    && !b.bookmarks.iter().any(|m| m.location == *location)
                {
                    b.bookmarks.push(Bookmark {
                        label: format!("书签 {}", b.bookmarks.len() + 1),
                        location: location.clone(),
                    });
                    self.save();
                    let origin = self.pet_celebration_origin(window);
                    self.motion
                        .trigger_pet_celebration(origin, self.theme().reduced_motion);
                    self.notice = Some("棱镜喵：已替你折好这一页的书签。".into());
                }
            }
            Action::Locate(target) => {
                self.send(
                    if target.starts_with("epubcfi(") {
                        json!({"kind":"locate","cfi":target})
                    } else {
                        json!({"kind":"locate","href":target})
                    },
                    cx,
                );
                self.drawer.clear();
            }
            Action::CloseEcho => {
                self.highlight_note_target = None;
                self.drawer.clear();
                self.send(json!({"kind":"restore"}), cx);
            }
            Action::Mood(mood) => self.mood = mood,
            Action::CancelMood => {
                self.mood_rx = None;
                self.notice = Some("已停止等待，文字已保留。已发出的请求可能仍会计费。".into());
            }
            Action::SubmitMood => {
                if self.mood_rx.is_some() {
                    return Ok(());
                }
                let text = self.mood_input.read(cx).value().trim().to_string();
                anyhow::ensure!(text.chars().count() <= 1000, "心情文字请控制在 1000 字以内");
                if let Some(generated) = self.mood_unsaved.take()
                    && generated.input == text
                    && generated.mood == self.mood
                {
                    self.finish_mood(Ok(generated), window, cx);
                    return Ok(());
                }
                let selected_mood = self.mood.clone();
                let worker = self.mood_worker;
                let use_custom = self.custom_mood_worker_override
                    || (worker as *const () as usize) != (mood::generate as *const () as usize);
                let ai_settings = self.store.state.ai.clone();
                let secrets = self.store.load_secrets();
                let (sender, receiver) = mpsc::channel();
                self.mood_rx = Some(receiver);
                std::thread::spawn(move || {
                    let generated = if use_custom {
                        worker(&text, &selected_mood)
                    } else {
                        mood::generate_with_settings(&ai_settings, &secrets, &text, &selected_mood)
                    };
                    let result = generated.map(|result| mood::Generated {
                        input: text,
                        mood: selected_mood,
                        result,
                    });
                    let _ = sender.send(result);
                });
            }
            Action::ToggleBreathing => {
                if self.breathing_started.is_some() {
                    self.breathing_started = None;
                    self.notice = Some("已结束呼吸引导，随时可以再坐一会儿。".into());
                } else {
                    self.breathing_started = Some(Instant::now());
                    self.schedule_motion_frame(window, cx);
                    self.notice =
                        Some("已开启 4-7-8 呼吸光环：吸气 4 秒 · 屏息 7 秒 · 呼气 8 秒。".into());
                }
            }
            Action::ToggleTimerPicker => {
                self.focus_timer_picker_open = !self.focus_timer_picker_open;
                if self.focus_timer_picker_open {
                    self.palette_open = false;
                    self.quote_card = None;
                    let cur = self.focus_timer_input.read(cx).value().trim().to_owned();
                    if cur.parse::<u32>().is_err() {
                        let fallback = (self.focus_timer_total_secs / 60).clamp(1, 180);
                        self.focus_timer_input
                            .update(cx, |s, cx| s.set_value(fallback.to_string(), window, cx));
                    }
                    let focus = self.focus_timer_input.read(cx).focus_handle(cx).clone();
                    window.focus(&focus, cx);
                } else {
                    window.focus(&self.focus, cx);
                }
            }
            Action::CloseTimerPicker => {
                self.focus_timer_picker_open = false;
                window.focus(&self.focus, cx);
            }
            Action::StepCustomTimer(delta) => {
                let current = self
                    .focus_timer_input
                    .read(cx)
                    .value()
                    .trim()
                    .parse::<i32>()
                    .unwrap_or((self.focus_timer_total_secs / 60).clamp(1, 180) as i32);
                let next = (current + delta).clamp(1, 180);
                self.focus_timer_input
                    .update(cx, |s, cx| s.set_value(next.to_string(), window, cx));
            }
            Action::ToggleTimerWithAudio => {
                self.focus_timer_with_audio = !self.focus_timer_with_audio;
            }
            Action::StartCustomFocusTimer => {
                let raw = self.focus_timer_input.read(cx).value().trim().to_owned();
                let mins = raw
                    .parse::<u32>()
                    .ok()
                    .filter(|m| (1..=180).contains(m))
                    .ok_or_else(|| anyhow::anyhow!("请输入 1—180 之间的整数分钟数"))?;
                self.perform(Action::StartFocusTimer(mins), window, cx)?;
            }
            Action::StartFocusTimer(minutes) => {
                let mins = minutes.clamp(1, 180);
                self.focus_timer_total_secs = mins * 60;
                self.focus_timer_started = Some(Instant::now());
                self.focus_timer_picker_open = false;
                self.focus_timer_input
                    .update(cx, |s, cx| s.set_value(mins.to_string(), window, cx));
                window.focus(&self.focus, cx);
                let audio_note = if self.focus_timer_with_audio && self.audio.is_none() {
                    match self.start_audio(false) {
                        Ok(label) => format!("，并已同步开启「{label}」声景"),
                        Err(_) => String::new(),
                    }
                } else {
                    String::new()
                };
                self.schedule_motion_frame(window, cx);
                self.notice = Some(format!(
                    "棱镜喵：已开启 {mins} 分钟静读光瓶{audio_note}，不急不躁，慢慢读吧。"
                ));
            }
            Action::StopFocusTimer => {
                let elapsed_mins = self
                    .focus_timer_started
                    .take()
                    .map(|started| (started.elapsed().as_secs() / 60) as u32)
                    .unwrap_or(0);
                self.focus_timer_picker_open = false;
                window.focus(&self.focus, cx);
                if elapsed_mins > 0 {
                    let today = today_ymd();
                    let total_today = self
                        .store
                        .add_reading_minutes(elapsed_mins, &today)
                        .unwrap_or(self.store.state.reading_minutes_today);
                    self.notice = Some(format!(
                        "已温柔收起光瓶，为你记下刚才静读的 {elapsed_mins} 分钟（今日共 {total_today} 分钟）。"
                    ));
                } else {
                    self.notice = Some("已结束本次静读光瓶。".into());
                }
            }
            Action::Favorite(id) => {
                self.store.edit_journal(|entries| {
                    if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
                        entry.favorite = !entry.favorite;
                    }
                })?;
            }
            Action::DeleteEntry(id) => {
                self.store
                    .edit_journal(|entries| entries.retain(|entry| entry.id != id))?;
            }
            Action::ClearJournal => self.confirm = Some("journal".into()),
            Action::ConfirmClear => {
                self.store.edit_journal(Vec::clear)?;
                self.confirm = None;
            }
            Action::ShelfFilter(filter) => self.shelf_filter = filter,
            Action::NextRediscovery => {
                self.rediscovery_offset = self.rediscovery_offset.wrapping_add(1);
            }
            Action::JournalFilter(filter) => self.journal_filter = filter,
            Action::ExportJournalMarkdown => {
                anyhow::ensure!(
                    !self.store.state.journal.is_empty(),
                    "日记还是空的，先留下几条回响吧"
                );
                self.hide(cx);
                let markdown = format_journal_markdown(&self.store.state.journal);
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("导出微光手账 (Markdown)")
                    .set_file_name("微光手账.md")
                    .add_filter("Markdown", &["md"])
                    .save_file()
                {
                    glow_core::store::atomic_write(&path, markdown.as_bytes())?;
                    self.notice = Some("已导出为 Markdown 手账文件。".into());
                }
            }
            Action::OpenQuoteCard(draft) => {
                self.palette_open = false;
                self.quote_card = Some(*draft);
            }
            Action::QuoteCardStyle(style) => {
                if let Some(card) = &mut self.quote_card {
                    card.style = style;
                }
            }
            Action::SaveQuoteCardPng => {
                let draft = self.quote_card.clone().context("请先选择要生成的书摘")?;
                self.hide(cx);
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("保存高清微光书摘卡")
                    .set_file_name("微光书摘卡.png")
                    .add_filter("PNG 图片", &["png"])
                    .save_file()
                {
                    quotecard::save_png(&draft, &path)?;
                    self.notice = Some("微光书摘卡已保存为高清 PNG 图片。".into());
                }
            }
            Action::CopyQuoteCardText => {
                if let Some(card) = &self.quote_card {
                    let mut text =
                        format!("“{}”\n—— {}", card.text.trim(), card.attribution.trim());
                    if let Some(note) = card
                        .note
                        .as_deref()
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                    {
                        text.push_str(&format!("\n批注：{note}"));
                    }
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    self.notice = Some("已复制书摘文字到剪贴板。".into());
                }
            }
            Action::CloseQuoteCard => {
                self.quote_card = None;
                window.focus(&self.focus, cx);
            }
            Action::TogglePalette => {
                self.palette_open = !self.palette_open;
                self.palette_selected = 0;
                if self.palette_open {
                    self.palette_input
                        .update(cx, |s, cx| s.set_value("", window, cx));
                    let focus = self.palette_input.read(cx).focus_handle(cx).clone();
                    window.focus(&focus, cx);
                } else {
                    window.focus(&self.focus, cx);
                }
            }
            Action::ClosePalette => {
                self.palette_open = false;
                window.focus(&self.focus, cx);
            }
            Action::PickTheme(index) => {
                let all = self.all_themes();
                if let Some(t) = all.get(index) {
                    let was_playing = self.audio.is_some();
                    let prev_volume = self.store.state.theme.volume;
                    let prev_reduced_motion = self.store.state.theme.reduced_motion;
                    let mut next = t.clone();
                    if index < themes().len() {
                        next.volume = prev_volume;
                        next.reduced_motion = prev_reduced_motion;
                    }
                    self.store.state.theme = next;
                    self.store.state.soundscape_preset = None;
                    let paired = SoundscapePreset::for_theme(&self.store.state.theme.id);
                    self.store.state.soundscape_mix = SoundscapeMix::from_single(paired.id());
                    self.stop_audio_smoothly();
                    self.preview = None;
                    self.install_native_theme(cx);
                    self.configure(cx);
                    if was_playing {
                        let _ = self.start_audio(false);
                    }
                    self.save();
                }
            }
            Action::EditTheme => {
                let t = self.store.duplicate_theme(&self.store.state.theme)?;
                self.fill_fields(&t, window, cx);
                self.draft = Some(t);
            }
            Action::PreviewTheme => {
                let t = self.read_fields(cx)?;
                glow_core::theme::validate(&t)?;
                self.preview = Some(t);
                self.install_native_theme(cx);
                self.configure(cx);
            }
            Action::SaveTheme => {
                let t = self.read_fields(cx)?;
                self.store.save_theme(t)?;
                self.stop_audio_smoothly();
                self.draft = None;
                self.preview = None;
                self.install_native_theme(cx);
                self.configure(cx);
                self.notice = Some("主题已经保存。".into());
            }
            Action::CancelTheme => {
                self.draft = None;
                self.preview = None;
                self.install_native_theme(cx);
                self.configure(cx);
            }
            Action::ResetTheme => {
                let mut t = ReadingTheme::default();
                if let Some(d) = &self.draft {
                    t.id = d.id.clone();
                }
                self.fill_fields(&t, window, cx);
                self.draft = Some(t);
            }
            Action::ImportTheme => {
                self.hide(cx);
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("选择微光主题包")
                    .add_filter("微光主题", &["glowtheme"])
                    .pick_file()
                {
                    self.perform_feature(Action::InspectThemePackage(path), window, cx)?;
                }
            }
            Action::ExportTheme => {
                self.hide(cx);
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("导出一个微光主题")
                    .set_file_name("我的微光.glowtheme")
                    .save_file()
                {
                    self.store.export_theme(self.theme(), &path)?;
                    self.notice = Some("已导出主题包，不包含书籍和日记。".into());
                }
            }
            Action::Asset(kind) => {
                self.hide(cx);
                let extensions = match kind.as_str() {
                    "background" => vec!["png", "jpg", "jpeg", "webp"],
                    "font" => vec!["ttf", "otf"],
                    _ => vec!["wav", "ogg", "mp3"],
                };
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("选择主题素材")
                    .add_filter("主题素材", &extensions)
                    .pick_file()
                {
                    let mut t = self.read_fields(cx)?;
                    self.store.copy_theme_asset(&mut t, &kind, &path)?;
                    self.fill_fields(&t, window, cx);
                    self.draft = Some(t);
                }
            }
            Action::Audio => {
                if self.audio.is_some() {
                    self.stop_audio_smoothly();
                    self.notice = Some("已轻声关闭环境声景。".into());
                } else {
                    self.stop_radio_if_playing();
                    let label = self.start_audio(false)?;
                    let vol_pct = (self.theme().volume.clamp(0.05, 1.0) * 100.0).round() as u32;
                    self.schedule_motion_frame(window, cx);
                    self.notice = Some(format!("已开启环境声景：{label}（音量 {vol_pct}%）。"));
                }
            }
            Action::Soundscape(preset) => {
                self.stop_radio_if_playing();
                self.store.state.soundscape_preset = Some(preset.id().into());
                self.store.state.soundscape_mix = SoundscapeMix::from_single(preset.id());
                self.save();
                let label = self.start_audio(true)?;
                let vol_pct = (self.theme().volume.clamp(0.05, 1.0) * 100.0).round() as u32;
                self.schedule_motion_frame(window, cx);
                self.notice = Some(format!("声景频道已调频至：{label}（音量 {vol_pct}%）。"));
            }
            Action::AudioVolume(delta) => {
                let steps = (self.store.state.theme.volume * 20.0).round() as i32 + delta as i32;
                let clamped = (steps as f32 / 20.0).clamp(0.05, 1.0);
                self.store.state.theme.volume = clamped;
                if let Some(preview) = &mut self.preview {
                    preview.volume = clamped;
                }
                if let Some((_, sink, _)) = &self.audio {
                    sink.set_volume(clamped);
                }
                self.save();
            }
            Action::ReducedMotion => {
                if let Some(t) = &mut self.draft {
                    t.reduced_motion = !t.reduced_motion;
                }
            }
            Action::ToggleFlow => {
                self.store.state.theme.reduced_motion = !self.store.state.theme.reduced_motion;
                if let Some(preview) = &mut self.preview {
                    preview.reduced_motion = self.store.state.theme.reduced_motion;
                }
                self.configure(cx);
                self.save();
                self.notice = Some(if self.store.state.theme.reduced_motion {
                    "已切换为静谧模式（关闭鼠标流光与动效）。".into()
                } else {
                    "已开启棱镜流光与动态交互。".into()
                });
            }
            Action::PetInteract(origin) => {
                let reduced = self.theme().reduced_motion;
                let is_prism = self.is_prism();
                let whisper = self.motion.trigger_pet(origin, reduced);
                self.motion.on_click(origin, reduced, is_prism);
                self.notice = Some(whisper.into());
                self.schedule_motion_frame(window, cx);
            }
            Action::RevealSpoiler(id) => {
                self.spoilers.insert(id);
            }
            Action::Privacy => {
                self.store.state.privacy_seen = true;
                self.save();
            }
            Action::Encoding(encoding) => {
                if let Some(id) = self.active_book.clone() {
                    let path = self.store.root.join("books").join(&id).join("original.txt");
                    let title = self
                        .store
                        .state
                        .books
                        .iter()
                        .find(|b| b.id == id)
                        .map(|b| b.title.clone());
                    let prepared = glow_core::library::prepare_book(&path, Some(&encoding))?;
                    self.store.commit_book_as(prepared, Some(&id))?;
                    if let Some(title) = title
                        && let Some(b) = self.store.state.books.iter_mut().find(|b| b.id == id)
                    {
                        b.title = title;
                    }
                    self.open(&id, window, cx)?;
                }
            }
            action @ (Action::RadioTogglePlay
            | Action::RadioTuneStation(_)
            | Action::RadioTuneMhz(_)
            | Action::RadioPrevStation
            | Action::RadioNextStation
            | Action::RadioNextSegment
            | Action::RadioToggleFavorite(_)
            | Action::RadioLampColor(_)
            | Action::RadioLampMode(_)
            | Action::RadioVolume(_)
            | Action::RadioSetVolume(_)
            | Action::RadioTogglePureGlow
            | Action::RadioToggleImmersive
            | Action::RadioToggleVisualMode
            | Action::RadioCycleSleepTimer
            | Action::RadioToggleDrawer
            | Action::RadioCloseDrawer
            | Action::RadioDrawerTab(_)
            | Action::RadioToggleFavFilter
            | Action::RadioSearchSubmit
            | Action::RadioSearchTag(_)
            | Action::RadioPreviewHit(_)
            | Action::RadioAddHit(_)
            | Action::RadioAddCustomStream
            | Action::RadioRemoveCustomStation(_)
            | Action::RadioPickLocalFolder
            | Action::RadioSetHostVoice(_)
            | Action::RadioSaveLineToJournal) => self.perform_radio(action, window, cx)?,
            other => self.perform_feature(other, window, cx)?,
        }
        Ok(())
    }
    pub(super) fn finish_ai_passage(
        &mut self,
        result: anyhow::Result<PassageGenerated>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Err(error) => self.error = Some(error.to_string()),
            Ok(mut generated) => {
                let same_passage = self.echo_input.trim() == generated.passage;
                // Keep any matched classical quotes alongside the fresh AI companion reflection
                if same_passage && let Some(existing) = &self.echo {
                    generated.result.quotes = existing.quotes.clone();
                }
                if same_passage {
                    self.echo = Some(generated.result.clone());
                }
                let location = generated
                    .book_id
                    .as_ref()
                    .filter(|id| {
                        glow_core::model::valid_cfi(&generated.cfi)
                            && self.store.state.books.iter().any(|b| &b.id == *id)
                    })
                    .map(|id| Location {
                        book_id: id.clone(),
                        cfi: generated.cfi.clone(),
                    });
                match self.store.record(
                    "selection",
                    generated.passage,
                    None,
                    generated.result,
                    location,
                ) {
                    Ok(()) => {
                        if same_passage {
                            self.echo_saved = true;
                        }
                        self.error = None;
                        self.notice = Some("AI 段落共读已送达，并为你收入私人手账。".into());
                    }
                    Err(error) => {
                        self.error = Some(format!("AI 共读已生成，但日记保存失败：{error}"));
                    }
                }
            }
        }
        cx.notify();
    }
    pub(super) fn finish_mood(
        &mut self,
        result: anyhow::Result<mood::Generated>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Err(error) => self.error = Some(error.to_string()),
            Ok(generated) => {
                match self.store.record(
                    "mood",
                    generated.input.clone(),
                    Some(generated.mood.clone()),
                    generated.result.clone(),
                    None,
                ) {
                    Err(error) => {
                        self.error = Some(format!(
                            "回答已生成，但日记保存失败：{error}。请保留当前文字再次点击，重试保存不会重新生成。"
                        ));
                        self.mood_unsaved = Some(generated);
                    }
                    Ok(()) => {
                        if self.mood_input.read(cx).value().trim() == generated.input {
                            self.mood_input
                                .update(cx, |state, cx| state.set_value("", window, cx));
                        }
                        let input = if generated.input.is_empty() {
                            generated.mood
                        } else {
                            generated.input
                        };
                        self.mood_reply = Some((input, generated.result));
                        self.mood_unsaved = None;
                        self.error = None;
                        cx.on_next_frame(window, |_, _, cx| {
                            cx.notify();
                        });
                    }
                }
            }
        }
        cx.notify();
    }

    pub(super) fn hide(&self, cx: &mut App) {
        if let Some(view) = &self.webview {
            view.update(cx, |v, _| v.hide());
        }
    }
    pub(super) fn begin_import(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.is_empty() {
            self.busy = false;
            return;
        }
        self.busy = true;
        self.notice = Some("正在整理书页，请稍候…".into());
        let task = cx.background_executor().spawn(async move {
            let total = paths.iter().try_fold(0u64, |total, path| {
                fs::metadata(path).map(|m| total.saturating_add(m.len()))
            });
            if total.is_ok_and(|size| size > glow_core::library::MAX_ARCHIVE) {
                return vec![Err(anyhow::anyhow!(
                    "一次导入的文件总量不能超过 100 MB，请分批导入"
                ))];
            }
            paths
                .into_iter()
                .map(|path| glow_core::library::prepare_book(&path, None))
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let prepared = task.await;
            let _ = this.update(cx, |this, cx| {
                let mut errors = Vec::new();
                let mut count = 0;
                for book in prepared {
                    match book.and_then(|b| this.store.commit_book(b, false)) {
                        Ok(_) => count += 1,
                        Err(e) => errors.push(e.to_string()),
                    }
                }
                this.busy = false;
                this.notice = Some(format!("已整理 {count} 本书，随时可以开始。"));
                this.error = if errors.is_empty() {
                    None
                } else {
                    Some(errors.join("；"))
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

use super::*;
use crate::soundscape::MIXER_RECIPES;
use glow_core::book_context;

impl ReadingApp {
    pub(super) fn ensure_tts(&mut self) -> &tts::TtsPlayer {
        self.tts_player.get_or_insert_with(tts::TtsPlayer::new)
    }

    pub(super) fn tick_features(&mut self, cx: &mut Context<Self>) {
        // 1. Poll Spoiler-Free AI Companion generation
        if let Some(res) = self.companion_rx.as_ref().map(|rx| rx.try_recv()) {
            match res {
                Ok(Ok((book_id, cache))) => {
                    self.companion_rx = None;
                    if let Some(book) = self.store.state.books.iter_mut().find(|b| b.id == book_id)
                    {
                        book.companion = Some(cache);
                        book.updated_at = Some(glow_core::model::now());
                    }
                    self.companion_selected_char = 0;
                    self.save();
                    self.notice = Some("已根据当前已读进度生成防剧透前情提要与人物图谱。".into());
                    cx.notify();
                }
                Ok(Err(err)) => {
                    self.companion_rx = None;
                    self.error = Some(err.to_string());
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.companion_rx = None;
                    self.error = Some("AI 共读解析任务已中断，请重试。".into());
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        // 2. Poll AI connection test & model discovery
        if let Some(res) = self.ai_test_rx.as_ref().map(|rx| rx.try_recv()) {
            match res {
                Ok(Ok(check)) => {
                    self.ai_test_rx = None;
                    self.discovered_ai_models = check.models;
                    self.notice = Some(check.summary);
                    cx.notify();
                }
                Ok(Err(err)) => {
                    self.ai_test_rx = None;
                    self.error = Some(err.to_string());
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.ai_test_rx = None;
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        // 3. Poll Sync connection test
        if let Some(res) = self.sync_test_rx.as_ref().map(|rx| rx.try_recv()) {
            match res {
                Ok(Ok(msg)) => {
                    self.sync_test_rx = None;
                    self.notice = Some(msg);
                    cx.notify();
                }
                Ok(Err(err)) => {
                    self.sync_test_rx = None;
                    self.error = Some(err.to_string());
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.sync_test_rx = None;
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        // 4. Poll Multi-Device Sync execution
        if let Some(res) = self.sync_rx.as_ref().map(|rx| rx.try_recv()) {
            match res {
                Ok(Ok(outcome)) => {
                    self.sync_rx = None;
                    self.store.state = outcome.merged_state;
                    self.save();
                    self.notice = Some(outcome.summary);
                    cx.notify();
                }
                Ok(Err(err)) => {
                    self.sync_rx = None;
                    self.error = Some(format!("同步失败：{err}"));
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.sync_rx = None;
                    self.error = Some("同步任务已中断，请稍后重试。".into());
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        // 5. Poll Edge-TTS audiobook status & synchronize background soundscape ducking
        if let Some(player) = &self.tts_player {
            let next_status = player.status();
            let changed = next_status.active != self.tts_status.active
                || next_status.paused != self.tts_status.paused
                || next_status.loading != self.tts_status.loading
                || next_status.segment_index != self.tts_status.segment_index
                || next_status.error != self.tts_status.error
                || next_status.engine_label != self.tts_status.engine_label;
            if let Some(err) = next_status.error.clone()
                && self.tts_status.error.as_deref() != Some(err.as_str())
            {
                self.error = Some(err);
            }
            self.tts_status = next_status;
            let should_duck = self.tts_status.active
                && !self.tts_status.paused
                && self.store.state.tts.duck_soundscape;
            if should_duck != self.tts_ducking_applied {
                self.tts_ducking_applied = should_duck;
                if let Some((_, _, Some(handle))) = &self.audio {
                    handle.set_ducking(should_duck);
                }
            }
            if changed {
                cx.notify();
            }
        }
    }

    pub(super) fn perform_feature(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        match action {
            // ================= Multi-Track Soundscape Mixer =================
            Action::ToggleMixer => {
                self.mixer_open = !self.mixer_open;
                if self.mixer_open {
                    self.palette_open = false;
                    self.focus_timer_picker_open = false;
                    self.quote_card = None;
                } else {
                    window.focus(&self.focus, cx);
                }
            }
            Action::CloseMixer => {
                self.mixer_open = false;
                window.focus(&self.focus, cx);
            }
            Action::StepMixTrack(track, delta) => {
                let cur = track.mix_value(&self.store.state.soundscape_mix);
                let next =
                    (((cur * 10.0).round() as i32 + delta as i32) as f32 / 10.0).clamp(0.0, 1.0);
                track.set_mix_value(&mut self.store.state.soundscape_mix, next);
                self.store.state.soundscape_preset = Some(track.id().into());
                self.save();
                if self.store.state.soundscape_mix.is_silent() {
                    self.stop_audio_smoothly();
                } else {
                    self.stop_radio_if_playing();
                    let label = self.start_audio(true)?;
                    self.schedule_motion_frame(window, cx);
                    self.notice = Some(format!("混音台已更新：{label}"));
                }
            }
            Action::SetMixTrack(track, value) => {
                track.set_mix_value(&mut self.store.state.soundscape_mix, value);
                self.store.state.soundscape_preset = Some(track.id().into());
                self.save();
                if self.store.state.soundscape_mix.is_silent() {
                    self.stop_audio_smoothly();
                } else {
                    self.stop_radio_if_playing();
                    let label = self.start_audio(true)?;
                    self.schedule_motion_frame(window, cx);
                    self.notice = Some(format!("混音台已更新：{label}"));
                }
            }
            Action::ApplyMixRecipe(recipe_id) => {
                self.store.state.soundscape_mix = SoundscapeMix::from_recipe(recipe_id);
                self.save();
                self.stop_radio_if_playing();
                let label = self.start_audio(true)?;
                self.schedule_motion_frame(window, cx);
                let name = MIXER_RECIPES
                    .iter()
                    .find(|(id, _, _)| *id == recipe_id)
                    .map(|(_, n, _)| *n)
                    .unwrap_or("混音配方");
                self.notice = Some(format!("已切换声景配方「{name}」：{label}"));
            }

            // ================= Spoiler-Free AI Companion =================
            Action::ToggleCompanion => {
                if self.drawer == "companion" {
                    self.drawer.clear();
                    self.send(json!({"kind":"restore"}), cx);
                } else {
                    self.drawer = "companion".into();
                    self.ensure_companion_preview()?;
                }
            }
            Action::CompanionTab(tab) => {
                self.companion_tab = tab.min(1);
            }
            Action::SelectCharacter(idx) => {
                self.companion_selected_char = idx;
            }
            Action::RefreshCompanion => {
                if self.companion_rx.is_some() {
                    return Ok(());
                }
                let (book_id, ctx) = self.build_active_book_spoiler_context()?;
                let ai_settings = self.store.state.ai.clone();
                let secrets = self.store.load_secrets();
                let (tx, rx) = mpsc::channel();
                self.companion_rx = Some(rx);
                self.notice = Some(format!(
                    "正在仅依据前 {}% 已读正文生成防剧透提要与人物图谱…",
                    ctx.progress_percent
                ));
                std::thread::spawn(move || {
                    let result = mood::generate_book_companion(&ai_settings, &secrets, &ctx)
                        .map(|cache| (book_id, cache));
                    let _ = tx.send(result);
                });
            }
            Action::CancelCompanion => {
                self.companion_rx = None;
                self.notice = Some("已取消 AI 伴读解析。".into());
            }
            Action::OpenCompanionPoster => {
                let book = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                    .context("请先打开一本书")?;
                let cache = book
                    .companion
                    .as_ref()
                    .context("请先生成前情提要与人物图谱")?;
                let mut body = format!("【{}】\n{}", cache.headline, cache.story_summary);
                let rel_lines: Vec<String> = cache
                    .relations
                    .iter()
                    .take(4)
                    .map(|r| format!("{} —({})— {}", r.from, r.label, r.to))
                    .collect();
                let note = if !rel_lines.is_empty() {
                    Some(format!("人物关系 · {}", rel_lines.join("  /  ")))
                } else if !cache.open_threads.is_empty() {
                    Some(format!("未解悬念 · {}", cache.open_threads[0]))
                } else {
                    None
                };
                if body.chars().count() > 260 {
                    body = format!("{}…", body.chars().take(258).collect::<String>());
                }
                self.palette_open = false;
                self.mixer_open = false;
                self.focus_timer_picker_open = false;
                self.quote_card = Some(QuoteCardDraft {
                    text: body,
                    attribution: format!(
                        "《{}》 · 读至 {}% 防剧透共读手账",
                        book.title, cache.progress_percent
                    ),
                    note,
                    style: QuoteCardStyle::from_theme_id(&self.theme().id),
                });
            }

            // ================= Compliant Dual-Track TTS Audiobook Studio =================
            Action::ToggleTtsDrawer => {
                if self.drawer == "tts" {
                    self.drawer.clear();
                    self.send(json!({"kind":"restore"}), cx);
                } else {
                    self.drawer = "tts".into();
                }
            }
            Action::TtsStart => {
                let book = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                    .context("请先打开一本书再开启听书")?
                    .clone();
                let epub_bytes = fs::read(
                    self.store
                        .root
                        .join("books")
                        .join(&book.id)
                        .join("book.epub"),
                )
                .context("无法读取当前书籍内容")?;
                let (segments, start_idx) = book_context::extract_tts_segments(
                    &epub_bytes,
                    book.location.as_ref().map(|l| l.cfi.as_str()),
                    book.chapter.as_deref(),
                    book.progress,
                )?;
                let voice = self.store.state.tts.voice.clone();
                let rate = self.store.state.tts.rate;
                let voice_label = tts::voice_by_id(&voice).label;
                let tts_cfg = tts::TtsRuntimeConfig::from_store(
                    &self.store.state.tts,
                    &self.store.state.ai,
                    &self.store.load_secrets(),
                    &self.store.root,
                );
                self.stop_radio_if_playing();
                self.ensure_tts()
                    .start(segments, start_idx, voice, rate, 0.92, tts_cfg);
                self.schedule_motion_frame(window, cx);
                self.notice = Some(format!("正在召唤「{voice_label}」为您朗读当前章节…"));
            }
            Action::TtsPauseResume => {
                if self.tts_status.active {
                    if self.tts_status.paused {
                        if let Some(p) = &self.tts_player {
                            p.resume();
                        }
                    } else if let Some(p) = &self.tts_player {
                        p.pause();
                    }
                } else {
                    self.perform_feature(Action::TtsStart, window, cx)?;
                }
            }
            Action::TtsStop => {
                if let Some(p) = &self.tts_player {
                    p.stop();
                }
                self.tts_status.active = false;
                self.tts_status.paused = false;
                self.tts_status.loading = false;
                if let Some((_, _, Some(handle))) = &self.audio {
                    handle.set_ducking(false);
                }
                self.tts_ducking_applied = false;
                self.notice = Some("已停止听书。".into());
            }
            Action::TtsPrev => {
                if self.tts_status.active {
                    let target = self.tts_status.segment_index.saturating_sub(1);
                    if let Some(p) = &self.tts_player {
                        p.jump(target);
                    }
                }
            }
            Action::TtsNext => {
                if self.tts_status.active && self.tts_status.total_segments > 0 {
                    let target =
                        (self.tts_status.segment_index + 1).min(self.tts_status.total_segments - 1);
                    if let Some(p) = &self.tts_player {
                        p.jump(target);
                    }
                }
            }
            Action::TtsVoice(voice_id) => {
                self.store.state.tts.voice = voice_id.clone();
                self.save();
                let label = tts::voice_by_id(&voice_id).label;
                if self.tts_status.active {
                    let resume_at = self.tts_status.segment_index;
                    if let Ok(book) = self
                        .store
                        .state
                        .books
                        .iter()
                        .find(|b| Some(&b.id) == self.active_book.as_ref())
                        .cloned()
                        .context("no book")
                        && let Ok(bytes) = fs::read(
                            self.store
                                .root
                                .join("books")
                                .join(&book.id)
                                .join("book.epub"),
                        )
                        && let Ok((segs, _)) = book_context::extract_tts_segments(
                            &bytes,
                            book.location.as_ref().map(|l| l.cfi.as_str()),
                            book.chapter.as_deref(),
                            book.progress,
                        )
                    {
                        let rate = self.store.state.tts.rate;
                        let tts_cfg = tts::TtsRuntimeConfig::from_store(
                            &self.store.state.tts,
                            &self.store.state.ai,
                            &self.store.load_secrets(),
                            &self.store.root,
                        );
                        self.ensure_tts()
                            .start(segs, resume_at, voice_id, rate, 0.92, tts_cfg);
                    }
                }
                self.notice = Some(format!("听书音色已切换为「{label}」。"));
            }
            Action::TtsRate(rate) => {
                self.store.state.tts.rate = rate.clamp(0.5, 2.0);
                self.save();
                self.notice = Some(format!(
                    "朗读语速已设为 {:.2}x（下一段生效或重新点击开始）。",
                    self.store.state.tts.rate
                ));
            }
            Action::TtsToggleDuck => {
                self.store.state.tts.duck_soundscape = !self.store.state.tts.duck_soundscape;
                self.save();
                let duck = self.tts_status.active
                    && !self.tts_status.paused
                    && self.store.state.tts.duck_soundscape;
                self.tts_ducking_applied = duck;
                if let Some((_, _, Some(handle))) = &self.audio {
                    handle.set_ducking(duck);
                }
            }
            Action::TtsToggleSleepTimer => {
                self.store.state.tts.sleep_with_timer = !self.store.state.tts.sleep_with_timer;
                self.save();
            }

            // ================= Multi-Provider AI Settings =================
            Action::SelectAiPreset(provider) => {
                let preset = AiSettings::for_preset(&provider);
                self.store.state.ai = preset.clone();
                self.discovered_ai_models.clear();
                self.ai_base_url_input
                    .update(cx, |s, cx| s.set_value(preset.base_url, window, cx));
                self.ai_model_input
                    .update(cx, |s, cx| s.set_value(preset.model, window, cx));
                self.save();
                self.notice = Some(format!(
                    "已切换 AI 预设：{}（点击保存或测试连接即可拉取可用模型）。",
                    mood::provider_display_name(&provider)
                ));
            }
            Action::PickDiscoveredModel(model_id) => {
                self.store.state.ai.model = model_id.clone();
                self.ai_model_input
                    .update(cx, |s, cx| s.set_value(model_id.clone(), window, cx));
                self.save();
                self.notice = Some(format!("已选中模型：{model_id}"));
            }
            Action::SaveAiSettings => {
                self.sync_ai_inputs_to_store(cx)?;
                self.notice = Some(format!(
                    "已保存 AI 配置（{} · {}），密钥仅存于本机 secrets.json。",
                    mood::provider_display_name(&self.store.state.ai.provider),
                    self.store.state.ai.model
                ));
            }
            Action::TestAiConnection => {
                if self.ai_test_rx.is_some() {
                    return Ok(());
                }
                self.sync_ai_inputs_to_store(cx)?;
                let settings = self.store.state.ai.clone();
                let secrets = self.store.load_secrets();
                let (tx, rx) = mpsc::channel();
                self.ai_test_rx = Some(rx);
                self.notice = Some(format!(
                    "正在检测 {} 连接状态…",
                    mood::provider_display_name(&settings.provider)
                ));
                std::thread::spawn(move || {
                    let _ = tx.send(mood::test_connection(&settings, &secrets));
                });
            }

            // ================= Multi-Device Sync Settings & Run =================
            Action::SelectSyncMode(mode) => {
                self.store.state.sync.mode = mode;
                self.save();
            }
            Action::PickSyncFolder => {
                self.hide(cx);
                if let Some(dir) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("选择多端同步目录（坚果云 / OneDrive / iCloud 等）")
                    .pick_folder()
                {
                    self.store.state.sync.folder_path = dir.to_string_lossy().into_owned();
                    self.store.state.sync.mode = "folder".into();
                    self.save();
                    self.notice = Some("已设置本地同步文件夹。".into());
                }
            }
            Action::ToggleSyncBooks => {
                self.store.state.sync.sync_books = !self.store.state.sync.sync_books;
                self.save();
            }
            Action::ToggleSyncOnExit => {
                self.store.state.sync.auto_sync_on_exit = !self.store.state.sync.auto_sync_on_exit;
                self.save();
            }
            Action::SaveSyncSettings => {
                self.sync_remote_inputs_to_store(cx)?;
                self.notice = Some("多端同步设置已保存（Token 仅存本机，绝不上传至仓库）。".into());
            }
            Action::TestSyncConnection => {
                if self.sync_test_rx.is_some() {
                    return Ok(());
                }
                self.sync_remote_inputs_to_store(cx)?;
                let settings = self.store.state.sync.clone();
                let token = self.store.load_secrets().github_token;
                let (tx, rx) = mpsc::channel();
                self.sync_test_rx = Some(rx);
                self.notice = Some("正在验证多端同步目标…".into());
                std::thread::spawn(move || {
                    let _ = tx.send(sync_client::test_sync_connection(&settings, &token));
                });
            }
            Action::RunSyncNow => {
                if self.sync_rx.is_some() {
                    return Ok(());
                }
                self.sync_remote_inputs_to_store(cx)?;
                let settings = self.store.state.sync.clone();
                anyhow::ensure!(
                    settings.mode != "off",
                    "请先在「主题与设置 → 多端同步」中选择 GitHub 或同步文件夹"
                );
                let token = self.store.load_secrets().github_token;
                let root = self.store.root.clone();
                let state_snapshot = self.store.state.clone();
                let (tx, rx) = mpsc::channel();
                self.sync_rx = Some(rx);
                self.notice = Some("正在执行多端双向合并同步…".into());
                std::thread::spawn(move || {
                    let _ = tx.send(sync_client::run_sync(
                        &root,
                        &state_snapshot,
                        &settings,
                        &token,
                    ));
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn sync_ai_inputs_to_store(&mut self, cx: &App) -> anyhow::Result<()> {
        let base_url = self.ai_base_url_input.read(cx).value().trim().to_owned();
        let model = self.ai_model_input.read(cx).value().trim().to_owned();
        let api_key = self.ai_key_input.read(cx).value().trim().to_owned();
        if !base_url.is_empty() {
            self.store.state.ai.base_url = base_url;
        }
        if !model.is_empty() {
            self.store.state.ai.model = model;
        }
        let mut secrets: LocalSecrets = self.store.load_secrets();
        secrets.ai_api_key = api_key;
        self.store.save_secrets(&secrets)?;
        self.save();
        Ok(())
    }

    fn sync_remote_inputs_to_store(&mut self, cx: &App) -> anyhow::Result<()> {
        self.store.state.sync.github_owner =
            self.sync_owner_input.read(cx).value().trim().to_owned();
        self.store.state.sync.github_repo = self.sync_repo_input.read(cx).value().trim().to_owned();
        let branch = self.sync_branch_input.read(cx).value().trim().to_owned();
        self.store.state.sync.github_branch = if branch.is_empty() {
            "main".into()
        } else {
            branch
        };
        let api_base = self.sync_api_base_input.read(cx).value().trim().to_owned();
        self.store.state.sync.github_api_base = if api_base.is_empty() {
            "https://api.github.com".into()
        } else {
            api_base
        };
        let mut secrets = self.store.load_secrets();
        secrets.github_token = self.sync_token_input.read(cx).value().trim().to_owned();
        self.store.save_secrets(&secrets)?;
        self.save();
        Ok(())
    }

    fn build_active_book_spoiler_context(
        &self,
    ) -> anyhow::Result<(String, book_context::SpoilerSafeContext)> {
        let book = self
            .store
            .state
            .books
            .iter()
            .find(|b| Some(&b.id) == self.active_book.as_ref())
            .context("请先打开一本书")?;
        let epub_bytes = fs::read(
            self.store
                .root
                .join("books")
                .join(&book.id)
                .join("book.epub"),
        )
        .context("无法读取书籍文件")?;
        let ctx = book_context::build_spoiler_safe_context(
            &epub_bytes,
            &book.title,
            &book.author,
            book.location.as_ref().map(|l| l.cfi.as_str()),
            book.chapter.as_deref(),
            book.progress,
        )?;
        Ok((book.id.clone(), ctx))
    }

    fn ensure_companion_preview(&mut self) -> anyhow::Result<()> {
        let Some(book_id) = self.active_book.clone() else {
            return Ok(());
        };
        let has_cache = self
            .store
            .state
            .books
            .iter()
            .find(|b| b.id == book_id)
            .is_some_and(|b| b.companion.is_some());
        if has_cache {
            return Ok(());
        }
        let (_, ctx) = self.build_active_book_spoiler_context()?;
        let preview = book_context::offline_companion_preview(&ctx);
        if let Some(book) = self.store.state.books.iter_mut().find(|b| b.id == book_id) {
            book.companion = Some(preview);
        }
        self.companion_selected_char = 0;
        Ok(())
    }

    // ================= UI: Spoiler-Free AI Companion Drawer =================
    #[inline(never)]
    pub(super) fn drawer_companion(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let dark = self.is_dark();
        let book = self
            .store
            .state
            .books
            .iter()
            .find(|b| Some(&b.id) == self.active_book.as_ref());
        let cache = book.and_then(|b| b.companion.as_ref());
        let loading = self.companion_rx.is_some();

        let mut panel = v_flex().gap_4();

        // Spoiler-free shield banner
        let shield_text = if let Some(c) = cache {
            format!(
                "严防剧透 · 仅读至「{}」（约 {}%），已隔离后续 {} 章",
                c.cutoff_chapter, c.progress_percent, c.skipped_future_chapters
            )
        } else {
            "严防剧透 · 只分析你当前已读进度之前的章节".into()
        };
        panel = panel.child(
            h_flex()
                .gap_2()
                .items_center()
                .px_3()
                .py_2()
                .rounded_xl()
                .bg(rgb(hex(&t.accent)).opacity(0.12))
                .border_1()
                .border_color(rgb(hex(&t.accent)).opacity(0.32))
                .child(
                    Icon::new(IconName::ShieldCheck)
                        .size(px(14.))
                        .text_color(rgb(hex(&t.accent))),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(10.5))
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.accent)))
                        .child(shield_text),
                ),
        );

        // Segmented tabs: 前情提要 / 人物图谱
        panel = panel.child(
            h_flex()
                .gap_1p5()
                .p_1()
                .rounded_xl()
                .bg(rgb(hex(&t.background)).opacity(if dark { 0.66 } else { 0.72 }))
                .child(
                    div()
                        .id("companion-tab-recap")
                        .debug_selector(|| "companion-tab-recap".into())
                        .cursor_pointer()
                        .flex_1()
                        .py_1p5()
                        .rounded_lg()
                        .flex()
                        .justify_center()
                        .bg(if self.companion_tab == 0 {
                            rgb(hex(&t.accent)).opacity(0.20)
                        } else {
                            rgb(0x000000).opacity(0.0)
                        })
                        .text_size(px(11.5))
                        .font_family(self.ui_font())
                        .text_color(if self.companion_tab == 0 {
                            rgb(hex(&t.accent))
                        } else {
                            rgb(hex(&t.muted))
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                this.act(Action::CompanionTab(0), window, cx);
                            }),
                        )
                        .child("前情提要"),
                )
                .child(
                    div()
                        .id("companion-tab-graph")
                        .debug_selector(|| "companion-tab-graph".into())
                        .cursor_pointer()
                        .flex_1()
                        .py_1p5()
                        .rounded_lg()
                        .flex()
                        .justify_center()
                        .bg(if self.companion_tab == 1 {
                            rgb(hex(&t.accent)).opacity(0.20)
                        } else {
                            rgb(0x000000).opacity(0.0)
                        })
                        .text_size(px(11.5))
                        .font_family(self.ui_font())
                        .text_color(if self.companion_tab == 1 {
                            rgb(hex(&t.accent))
                        } else {
                            rgb(hex(&t.muted))
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                this.act(Action::CompanionTab(1), window, cx);
                            }),
                        )
                        .child(format!(
                            "人物图谱 ({})",
                            cache.map(|c| c.characters.len()).unwrap_or(0)
                        )),
                ),
        );

        // Primary AI refresh + Xiaohongshu poster button
        let mut action_row = h_flex().gap_2().flex_wrap();
        if loading {
            action_row = action_row.child(
                self.button_with_icon(
                    "AI 正在梳理脉络… 点此取消",
                    IconName::X,
                    Action::CancelCompanion,
                    cx,
                )
                .outline(),
            );
        } else {
            action_row = action_row
                .child(
                    self.button_with_icon(
                        "AI 深度解析（防剧透）",
                        IconName::WandSparkles,
                        Action::RefreshCompanion,
                        cx,
                    )
                    .primary(),
                )
                .when(cache.is_some(), |el| {
                    el.child(
                        self.button_with_icon(
                            "出图海报",
                            IconName::ImageDown,
                            Action::OpenCompanionPoster,
                            cx,
                        )
                        .outline(),
                    )
                });
        }
        panel = panel.child(action_row);

        let Some(cache) = cache else {
            return panel
                .child(
                    div()
                        .p_4()
                        .rounded_xl()
                        .bg(rgb(hex(&t.background)).opacity(0.55))
                        .text_size(px(12.5))
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.muted)))
                        .child("点击上方按钮，AI 将只读取你已经读过的章节，为你梳理前情提要与动态人物关系。"),
                )
                .into_any_element();
        };

        if self.companion_tab == 0 {
            // ===== Tab 0: Spoiler-Free Story Recap =====
            panel = panel
                .child(
                    v_flex()
                        .p_4()
                        .gap_2p5()
                        .rounded_2xl()
                        .bg(rgb(hex(&t.background)).opacity(if dark { 0.64 } else { 0.70 }))
                        .border_1()
                        .border_color(self.border_subtle())
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.accent)))
                                        .child(if cache.is_ai {
                                            "PREVIOUSLY ON · AI 文学回顾"
                                        } else {
                                            "LOCAL PREVIEW · 本地快速预览"
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(9.5))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child(format!("{}%", cache.progress_percent)),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(16.5))
                                .font_family(self.display_font())
                                .text_color(rgb(hex(&t.text)))
                                .child(cache.headline.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_family(self.ui_font())
                                .line_height(relative(1.75))
                                .text_color(rgb(hex(&t.text)).opacity(0.92))
                                .child(cache.story_summary.clone()),
                        ),
                )
                .child(
                    v_flex()
                        .p_3p5()
                        .gap_1p5()
                        .rounded_xl()
                        .bg(rgb(hex(&t.accent)).opacity(0.08))
                        .border_1()
                        .border_color(rgb(hex(&t.accent)).opacity(0.24))
                        .child(
                            div()
                                .text_size(px(10.))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.accent)))
                                .child("上次停在这里的瞬间"),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_family(self.display_font())
                                .line_height(relative(1.65))
                                .text_color(rgb(hex(&t.text)))
                                .child(format!("“{}”", cache.resume_moment)),
                        ),
                );

            if !cache.open_threads.is_empty() {
                let mut threads_box = v_flex().gap_2().child(
                    div()
                        .text_size(px(11.))
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.muted)))
                        .child("未解伏笔与悬念（绝不剧透答案）"),
                );
                for (idx, item) in cache.open_threads.iter().enumerate() {
                    threads_box = threads_box.child(
                        h_flex()
                            .gap_2()
                            .items_start()
                            .p_2p5()
                            .rounded_xl()
                            .bg(rgb(hex(&t.background)).opacity(0.54))
                            .border_1()
                            .border_color(self.border_subtle())
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgb(hex(&t.accent)).opacity(0.16))
                                    .text_size(px(9.5))
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(format!("{}", idx + 1)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .font_family(self.ui_font())
                                    .line_height(relative(1.55))
                                    .child(item.clone()),
                            ),
                    );
                }
                panel = panel.child(threads_box);
            }
        } else {
            // ===== Tab 1: Interactive Character Relationship Constellation Graph =====
            if cache.characters.is_empty() {
                panel = panel.child(
                    div()
                        .p_4()
                        .rounded_xl()
                        .bg(rgb(hex(&t.background)).opacity(0.55))
                        .text_size(px(12.5))
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.muted)))
                        .child("当前已读内容暂未提取到足够人物，点击上方「AI 深度解析」可从已读正文构建完整关系图谱。"),
                );
            } else {
                let selected_idx = self.companion_selected_char.min(cache.characters.len() - 1);
                let stage_w = 312.0_f32;
                let stage_h = 224.0_f32;
                let center_x = stage_w * 0.5;
                let center_y = stage_h * 0.5;
                let rx = 104.0_f32;
                let ry = 74.0_f32;
                let count = cache.characters.len();

                let mut coords: Vec<(f32, f32)> = Vec::with_capacity(count);
                for i in 0..count {
                    if count == 1 {
                        coords.push((center_x, center_y));
                    } else {
                        let angle = -std::f32::consts::FRAC_PI_2
                            + (i as f32 / count as f32) * std::f32::consts::TAU;
                        coords.push((center_x + rx * angle.cos(), center_y + ry * angle.sin()));
                    }
                }

                let name_to_idx: std::collections::HashMap<&str, usize> = cache
                    .characters
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (c.name.as_str(), i))
                    .collect();

                // Prepare edges for background constellation canvas
                let edge_data: Vec<((f32, f32), (f32, f32), u32, bool)> = cache
                    .relations
                    .iter()
                    .filter_map(|rel| {
                        let a = *name_to_idx.get(rel.from.as_str())?;
                        let b = *name_to_idx.get(rel.to.as_str())?;
                        let col = relation_tone_color(&rel.tone, hex(&t.accent));
                        let active = a == selected_idx || b == selected_idx;
                        Some((coords[a], coords[b], col, active))
                    })
                    .collect();
                let accent_hex = hex(&t.accent);
                let edges_canvas = canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let ox = f32::from(bounds.origin.x);
                        let oy = f32::from(bounds.origin.y);
                        // Soft subtle constellation orbit ring
                        let ring_steps = 48;
                        for s in 0..ring_steps {
                            let a = (s as f32 / ring_steps as f32) * std::f32::consts::TAU;
                            let x = ox + center_x + rx * a.cos();
                            let y = oy + center_y + ry * a.sin();
                            window.paint_quad(PaintQuad {
                                bounds: Bounds::new(
                                    point(px(x - 1.0), px(y - 1.0)),
                                    size(px(2.0), px(2.0)),
                                ),
                                corner_radii: Corners::all(px(1.0)),
                                background: Hsla::from(rgb(accent_hex).opacity(0.18)).into(),
                                border_widths: Edges::default(),
                                border_color: Hsla::transparent_black(),
                                border_style: BorderStyle::Solid,
                            });
                        }
                        // Dotted luminous relationship connectors
                        for &((x0, y0), (x1, y1), col, active) in &edge_data {
                            let steps = 22usize;
                            let dot = if active { 3.2 } else { 2.0 };
                            let alpha = if active { 0.85 } else { 0.30 };
                            for s in 1..steps {
                                let p = s as f32 / steps as f32;
                                let x = ox + x0 + (x1 - x0) * p;
                                let y = oy + y0 + (y1 - y0) * p;
                                window.paint_quad(PaintQuad {
                                    bounds: Bounds::new(
                                        point(px(x - dot * 0.5), px(y - dot * 0.5)),
                                        size(px(dot), px(dot)),
                                    ),
                                    corner_radii: Corners::all(px(dot * 0.5)),
                                    background: Hsla::from(rgb(col).opacity(alpha)).into(),
                                    border_widths: Edges::default(),
                                    border_color: Hsla::transparent_black(),
                                    border_style: BorderStyle::Solid,
                                });
                            }
                        }
                    },
                )
                .absolute()
                .inset_0();

                let mut graph_stage = div()
                    .relative()
                    .w(px(stage_w))
                    .h(px(stage_h))
                    .rounded_2xl()
                    .overflow_hidden()
                    .bg(rgb(hex(&t.background)).opacity(if dark { 0.72 } else { 0.78 }))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(edges_canvas);

                for (idx, character) in cache.characters.iter().enumerate() {
                    let (nx, ny) = coords[idx];
                    let selected = idx == selected_idx;
                    let short_name: String = character.name.chars().take(6).collect();
                    let pill_w = 78.0_f32;
                    let pill_h = 30.0_f32;
                    graph_stage = graph_stage.child(
                        div()
                            .id(SharedString::from(format!("char-node-{idx}")))
                            .cursor_pointer()
                            .absolute()
                            .left(px((nx - pill_w * 0.5).clamp(6.0, stage_w - pill_w - 6.0)))
                            .top(px((ny - pill_h * 0.5).clamp(6.0, stage_h - pill_h - 6.0)))
                            .w(px(pill_w))
                            .h(px(pill_h))
                            .px_1p5()
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(if selected {
                                rgb(hex(&t.accent))
                            } else {
                                rgb(hex(&t.paper)).opacity(0.95)
                            })
                            .border_1()
                            .border_color(if selected {
                                rgb(hex(&t.accent))
                            } else {
                                rgb(hex(&t.accent)).opacity(0.42)
                            })
                            .shadow_sm()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.act(Action::SelectCharacter(idx), window, cx);
                                }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_ellipsis()
                                    .text_color(if selected {
                                        rgb(0xffffff)
                                    } else {
                                        rgb(hex(&t.text))
                                    })
                                    .child(short_name),
                            ),
                    );
                }

                // Legend strip
                let legend = h_flex().gap_3().flex_wrap().px_1().children(
                    [
                        ("羁绊", hex(&t.accent)),
                        ("亲缘", 0x34d399u32),
                        ("对立", 0xfb7185u32),
                        ("悬念", 0xf59e0bu32),
                    ]
                    .into_iter()
                    .map(|(label, col)| {
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(div().size(px(7.)).rounded_full().bg(rgb(col)))
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(label),
                            )
                    }),
                );

                // Selected character dossier card
                let ch = &cache.characters[selected_idx];
                let mut dossier = v_flex()
                    .p_4()
                    .gap_2()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.background)).opacity(if dark { 0.64 } else { 0.70 }))
                    .border_1()
                    .border_color(rgb(hex(&t.accent)).opacity(0.36))
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(16.5))
                                    .font_family(self.display_font())
                                    .text_color(rgb(hex(&t.text)))
                                    .child(ch.name.clone()),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgb(hex(&t.accent)).opacity(0.14))
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(ch.role.clone()),
                            ),
                    );
                if !ch.aliases.is_empty() {
                    dossier = dossier.child(
                        div()
                            .text_size(px(10.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child(format!("别称：{}", ch.aliases.join(" / "))),
                    );
                }
                if !ch.traits.is_empty() {
                    let mut tags = h_flex().gap_1p5().flex_wrap();
                    for tr in &ch.traits {
                        tags = tags.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(rgb(hex(&t.paper)).opacity(0.82))
                                .text_size(px(10.))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.text)))
                                .child(tr.clone()),
                        );
                    }
                    dossier = dossier.child(tags);
                }
                if !ch.status.is_empty() {
                    dossier = dossier.child(
                        div()
                            .text_size(px(12.))
                            .font_family(self.ui_font())
                            .line_height(relative(1.6))
                            .child(format!("当前状态：{}", ch.status)),
                    );
                }
                let linked: Vec<&glow_core::CharacterRelation> = cache
                    .relations
                    .iter()
                    .filter(|r| r.from == ch.name || r.to == ch.name)
                    .collect();
                if !linked.is_empty() {
                    let mut rel_box = v_flex()
                        .gap_1()
                        .pt_1()
                        .border_t_1()
                        .border_color(self.border_subtle());
                    for rel in linked {
                        let other = if rel.from == ch.name {
                            &rel.to
                        } else {
                            &rel.from
                        };
                        let col = relation_tone_color(&rel.tone, hex(&t.accent));
                        rel_box = rel_box.child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(div().size(px(6.)).rounded_full().bg(rgb(col)))
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.text)))
                                        .child(format!("与 {other}：{}", rel.label)),
                                ),
                        );
                    }
                    dossier = dossier.child(rel_box);
                }

                panel = panel.child(graph_stage).child(legend).child(dossier);
            }
        }

        if !cache.is_ai {
            panel = panel.child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .p_3()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(0.52))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child("支持 DeepSeek / 通义千问 / OpenAI 或本机 Ollama 离线模型"),
                    )
                    .child(self.button("去配置 AI", Action::Page(Page::Themes), cx)),
            );
        }

        panel.into_any_element()
    }

    // ================= UI: Compliant Dual-Track TTS Audiobook Studio Drawer =================
    #[inline(never)]
    pub(super) fn drawer_tts(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let dark = self.is_dark();
        let st = &self.tts_status;
        let cur_voice = tts::voice_by_id(&self.store.state.tts.voice);

        let status_label = if st.loading {
            "正在合成语音…".to_owned()
        } else if st.active && !st.paused {
            format!(
                "正在朗读 · {}",
                if st.engine_label.is_empty() {
                    "微光语音引擎"
                } else {
                    st.engine_label.as_str()
                }
            )
        } else if st.active && st.paused {
            "已暂停听书".to_owned()
        } else {
            format!("就绪 · 音色「{}」", cur_voice.label)
        };

        let mut panel = v_flex().gap_4();

        // Now-playing card
        panel = panel.child(
            v_flex()
                .p_4()
                .gap_3()
                .rounded_2xl()
                .bg(rgb(hex(&t.background)).opacity(if dark { 0.68 } else { 0.74 }))
                .border_1()
                .border_color(if st.active {
                    Hsla::from(rgb(hex(&t.accent)).opacity(0.55))
                } else {
                    self.border_subtle()
                })
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(10.5))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.accent)))
                                .child(status_label),
                        )
                        .child(self.waveform_bars(
                            st.active && !st.paused && !st.loading,
                            &[5., 11., 14., 8., 12., 7., 10.],
                        )),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .font_family(self.display_font())
                        .line_height(relative(1.7))
                        .text_color(rgb(hex(&t.text)))
                        .child(if st.current_text.is_empty() {
                            "点击下方「开始听书」，将优先使用您配置的合规云端或本地神经语音服务（自动缓存分段音频），未配置时自动降级为本机离线系统语音。"
                                .to_owned()
                        } else {
                            st.current_text.clone()
                        }),
                )
                .when(st.total_segments > 0, |el| {
                    el.child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .text_ellipsis()
                                    .child(st.chapter_title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(format!(
                                        "第 {} / {} 段",
                                        st.segment_index + 1,
                                        st.total_segments
                                    )),
                            ),
                    )
                }),
        );

        // Transport controls
        panel = panel.child(
            h_flex()
                .gap_2()
                .items_center()
                .justify_between()
                .child(
                    self.button_with_icon("上句", IconName::ChevronLeft, Action::TtsPrev, cx)
                        .disabled(!st.active),
                )
                .child(
                    self.button_with_icon(
                        if !st.active {
                            "开始听书"
                        } else if st.paused {
                            "继续朗读"
                        } else {
                            "暂停朗读"
                        },
                        IconName::AudioLines,
                        Action::TtsPauseResume,
                        cx,
                    )
                    .primary()
                    .flex_1(),
                )
                .child(
                    self.button_with_icon("下句", IconName::ChevronRight, Action::TtsNext, cx)
                        .disabled(!st.active),
                )
                .when(st.active, |el| {
                    el.child(self.button_with_icon("停止", IconName::X, Action::TtsStop, cx))
                }),
        );

        // Voice selector grid
        let mut voices_grid = h_flex().gap_2().flex_wrap();
        for v in tts::VOICES {
            let selected = self.store.state.tts.voice == v.id;
            let vid = v.id.to_owned();
            voices_grid = voices_grid.child(
                v_flex()
                    .id(SharedString::from(format!("tts-voice-{}", v.id)))
                    .cursor_pointer()
                    .w(px(148.))
                    .p_2p5()
                    .gap_0p5()
                    .rounded_xl()
                    .bg(if selected {
                        rgb(hex(&t.accent)).opacity(0.18)
                    } else {
                        rgb(hex(&t.background)).opacity(0.56)
                    })
                    .border_1()
                    .border_color(if selected {
                        Hsla::from(rgb(hex(&t.accent)).opacity(0.68))
                    } else {
                        self.border_subtle()
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::TtsVoice(vid.clone()), window, cx);
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_family(self.ui_font())
                            .text_color(if selected {
                                rgb(hex(&t.accent))
                            } else {
                                rgb(hex(&t.text))
                            })
                            .child(v.label),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child(v.style),
                    ),
            );
        }
        panel = panel.child(
            v_flex()
                .gap_2()
                .child(
                    div()
                        .text_size(px(11.))
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.muted)))
                        .child("陪伴主播音色（兼容云端与离线）"),
                )
                .child(voices_grid),
        );

        // Speed selector
        let mut speed_row = h_flex().gap_1p5().flex_wrap();
        for &rate in &[0.75_f32, 1.0, 1.15, 1.3, 1.5] {
            let selected = (self.store.state.tts.rate - rate).abs() < 0.03;
            speed_row = speed_row.child(
                div()
                    .id(SharedString::from(format!("tts-rate-{:.2}", rate)))
                    .cursor_pointer()
                    .px_2p5()
                    .py_1()
                    .rounded_lg()
                    .bg(if selected {
                        rgb(hex(&t.accent)).opacity(0.20)
                    } else {
                        rgb(hex(&t.background)).opacity(0.60)
                    })
                    .border_1()
                    .border_color(if selected {
                        Hsla::from(rgb(hex(&t.accent)).opacity(0.65))
                    } else {
                        self.border_subtle()
                    })
                    .text_size(px(11.))
                    .font_family(self.ui_font())
                    .text_color(if selected {
                        rgb(hex(&t.accent))
                    } else {
                        rgb(hex(&t.text))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::TtsRate(rate), window, cx);
                        }),
                    )
                    .child(format!("{rate:.2}x")),
            );
        }
        panel = panel.child(
            v_flex()
                .gap_2()
                .child(
                    div()
                        .text_size(px(11.))
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.muted)))
                        .child("朗读语速"),
                )
                .child(speed_row),
        );

        // Cozy integration toggles
        panel = panel.child(
            v_flex()
                .gap_2()
                .p_3()
                .rounded_xl()
                .bg(rgb(hex(&t.background)).opacity(0.52))
                .border_1()
                .border_color(self.border_subtle())
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_family(self.ui_font())
                                .child("朗读时智能压低环境声景"),
                        )
                        .child(
                            self.button(
                                if self.store.state.tts.duck_soundscape {
                                    "已开启"
                                } else {
                                    "已关闭"
                                },
                                Action::TtsToggleDuck,
                                cx,
                            )
                            .toggled(self.store.state.tts.duck_soundscape),
                        ),
                )
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_family(self.ui_font())
                                .child("静读计时结束自动停播（睡前定时）"),
                        )
                        .child(
                            self.button(
                                if self.store.state.tts.sleep_with_timer {
                                    "已开启"
                                } else {
                                    "已关闭"
                                },
                                Action::TtsToggleSleepTimer,
                                cx,
                            )
                            .toggled(self.store.state.tts.sleep_with_timer),
                        ),
                ),
        );

        panel.into_any_element()
    }

    // ================= UI: 6-Track Soundscape Mixer Modal =================
    #[inline(never)]
    pub(super) fn soundscape_mixer_modal(
        &self,
        snap: MotionSnapshot,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = self.theme();
        let dark = self.is_dark();
        let mix = self.store.state.soundscape_mix.clamped();
        let vol_pct = (t.volume.clamp(0.05, 1.0) * 100.0).round() as u32;

        let mut recipe_row = h_flex().gap_2().flex_wrap();
        for &(rid, name, desc) in MIXER_RECIPES {
            let selected = mix.recipe.as_deref() == Some(rid);
            recipe_row = recipe_row.child(
                v_flex()
                    .id(SharedString::from(format!("mix-recipe-{rid}")))
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .gap_0p5()
                    .rounded_xl()
                    .bg(if selected {
                        rgb(hex(&t.accent)).opacity(0.18)
                    } else {
                        rgb(hex(&t.background)).opacity(if dark { 0.62 } else { 0.72 })
                    })
                    .border_1()
                    .border_color(if selected {
                        Hsla::from(rgb(hex(&t.accent)).opacity(0.72))
                    } else {
                        self.border_subtle()
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::ApplyMixRecipe(rid), window, cx);
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(self.ui_font())
                            .text_color(if selected {
                                rgb(hex(&t.accent))
                            } else {
                                rgb(hex(&t.text))
                            })
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child(desc),
                    ),
            );
        }

        let mut tracks_col = v_flex().gap_2p5();
        for preset in SoundscapePreset::ALL {
            let val = preset.mix_value(&mix);
            let pct = (val * 100.0).round() as u32;
            let active = val >= 0.01;
            let toggle_target = if active { 0.0 } else { 0.65 };
            tracks_col = tracks_col.child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .px_3()
                    .py_2()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(if dark { 0.62 } else { 0.68 }))
                    .border_1()
                    .border_color(if active {
                        Hsla::from(rgb(hex(&t.accent)).opacity(0.45))
                    } else {
                        self.border_subtle()
                    })
                    .child(
                        h_flex()
                            .id(SharedString::from(format!("mix-toggle-{}", preset.id())))
                            .cursor_pointer()
                            .w(px(156.))
                            .gap_2()
                            .items_center()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.act(
                                        Action::SetMixTrack(preset, toggle_target),
                                        window,
                                        cx,
                                    );
                                }),
                            )
                            .child(div().size(px(8.)).rounded_full().bg(if active {
                                rgb(0x36c98e)
                            } else {
                                rgb(hex(&t.muted)).opacity(0.35)
                            }))
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_family(self.ui_font())
                                            .text_color(if active {
                                                rgb(hex(&t.text))
                                            } else {
                                                rgb(hex(&t.muted))
                                            })
                                            .child(preset.label()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(9.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.muted)))
                                            .child(preset.subtitle()),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h(px(8.))
                            .rounded_full()
                            .bg(rgb(hex(&t.text)).opacity(0.10))
                            .child(
                                div()
                                    .w(relative(val))
                                    .h_full()
                                    .rounded_full()
                                    .bg(rgb(hex(&t.accent))),
                            ),
                    )
                    .child(
                        div()
                            .w(px(36.))
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(if active {
                                rgb(hex(&t.accent))
                            } else {
                                rgb(hex(&t.muted))
                            })
                            .child(format!("{pct}%")),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(self.button("−", Action::StepMixTrack(preset, -1), cx))
                            .child(self.button("+", Action::StepMixTrack(preset, 1), cx)),
                    ),
            );
        }

        div()
            .id("soundscape-mixer-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .bg(rgb(0x060910).opacity(0.56))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.act(Action::CloseMixer, window, cx);
                }),
            )
            .child(
                v_flex()
                    .id("soundscape-mixer-dialog")
                    .debug_selector(|| "soundscape-mixer-dialog".into())
                    .relative()
                    .overflow_hidden()
                    .w(px(640.))
                    .max_w_full()
                    .max_h_full()
                    .overflow_y_scroll()
                    .p_6()
                    .gap_4()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(0.98))
                    .border_1()
                    .border_color(self.border_strong())
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .child(prism::prism_beam_horizontal(snap, 2.0, 0.9, false)),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(self.studio_badge(
                                        "SOUNDSCAPE MIXER",
                                        "6-TRACK AMBIENT STUDIO",
                                        true,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(21.))
                                            .font_family(self.display_font())
                                            .child("多轨治愈声景混音台"),
                                    ),
                            )
                            .child(self.button_with_icon(
                                "关闭",
                                IconName::X,
                                Action::CloseMixer,
                                cx,
                            )),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("一键治愈配方（点击即时融合）"),
                            )
                            .child(recipe_row),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.muted)))
                                            .child(
                                                "独立 6 轨自由叠加（点击名称开关 · ± 微调比例）",
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.accent)))
                                            .child(format!("当前配方：{}", mix.summary_label())),
                                    ),
                            )
                            .child(tracks_col),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .pt_2()
                            .border_t_1()
                            .border_color(self.border_subtle())
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.muted)))
                                            .child(format!("总输出音量 {vol_pct}%")),
                                    )
                                    .child(self.button("−", Action::AudioVolume(-1), cx))
                                    .child(self.button("+", Action::AudioVolume(1), cx)),
                            )
                            .child(
                                self.button_with_icon(
                                    if self.audio.is_some() {
                                        "停止声景播放"
                                    } else {
                                        "立即开启混音声景"
                                    },
                                    if self.audio.is_some() {
                                        IconName::VolumeX
                                    } else {
                                        IconName::AudioLines
                                    },
                                    Action::Audio,
                                    cx,
                                )
                                .primary(),
                            ),
                    ),
            )
            .into_any_element()
    }

    // ================= UI: Settings Cards on Page::Themes =================
    #[inline(never)]
    pub(super) fn settings_ai_card(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let is_dark = self.is_dark();
        let cur_provider = self.store.state.ai.provider.as_str();
        let is_local = self.store.state.ai.is_local();

        let presets: &[(&str, &str)] = &[
            ("deepseek", "DeepSeek"),
            ("qwen", "通义千问"),
            ("siliconflow", "硅基流动"),
            ("openai", "OpenAI"),
            ("ollama", "Ollama 离线"),
            ("lmstudio", "LM Studio 离线"),
            ("llamacpp", "llama.cpp 离线"),
            ("custom", "自定义大模型 URL"),
        ];
        let mut preset_pills = h_flex().gap_1p5().flex_wrap();
        for &(pid, label) in presets {
            let selected = cur_provider == pid;
            let pid_owned = pid.to_owned();
            preset_pills = preset_pills.child(
                div()
                    .id(SharedString::from(format!("ai-preset-{pid}")))
                    .cursor_pointer()
                    .px_3()
                    .py_1p5()
                    .rounded_full()
                    .bg(if selected {
                        rgb(hex(&t.accent)).opacity(0.20)
                    } else {
                        rgb(hex(&t.background)).opacity(if is_dark { 0.65 } else { 0.78 })
                    })
                    .border_1()
                    .border_color(if selected {
                        Hsla::from(rgb(hex(&t.accent)).opacity(0.70))
                    } else {
                        self.border_subtle()
                    })
                    .text_size(px(11.5))
                    .font_family(self.ui_font())
                    .text_color(if selected {
                        rgb(hex(&t.accent))
                    } else {
                        rgb(hex(&t.text))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::SelectAiPreset(pid_owned.clone()), window, cx);
                        }),
                    )
                    .child(label),
            );
        }

        v_flex()
            .p_5()
            .gap_4()
            .rounded_2xl()
            .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.90 } else { 0.94 }))
            .border_1()
            .border_color(self.border_subtle())
            .shadow_sm()
            .child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(self.studio_badge("AI ENGINE", "OFFLINE LLM & CUSTOM MODEL URL", true))
                            .child(
                                div()
                                    .text_size(px(19.5))
                                    .font_family(self.display_font())
                                    .child("AI 共读引擎（本地离线大模型 / 任意大模型 URL）"),
                            ),
                    )
                    .child(
                        div()
                            .px_2p5()
                            .py_1()
                            .rounded_full()
                            .bg(rgb(hex(&t.accent)).opacity(0.12))
                            .text_size(px(10.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.accent)))
                            .child(mood::provider_display_name(cur_provider)),
                    ),
            )
            .child(preset_pills)
            .child(
                h_flex()
                    .gap_4()
                    .flex_wrap()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(230.))
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("大模型 URL（支持 Base URL 或完整接口路径）"),
                            )
                            .child(Input::new(&self.ai_base_url_input)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(200.))
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("模型名称（Model ID）"),
                            )
                            .child(Input::new(&self.ai_model_input)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(230.))
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(if is_local || cur_provider == "custom" {
                                        "API Key（离线/免鉴权 URL 可留空 · 仅存本机）"
                                    } else {
                                        "API Key（仅存本机 secrets.json，永不同步）"
                                    }),
                            )
                            .child(Input::new(&self.ai_key_input)),
                    ),
            )
            .when(!self.discovered_ai_models.is_empty(), |card| {
                let mut chips = h_flex().gap_1p5().flex_wrap();
                for model_id in &self.discovered_ai_models {
                    let selected = self.store.state.ai.model == *model_id;
                    let mid = model_id.clone();
                    chips = chips.child(
                        div()
                            .id(SharedString::from(format!("discovered-model-{model_id}")))
                            .cursor_pointer()
                            .px_2p5()
                            .py_1()
                            .rounded_full()
                            .bg(if selected {
                                rgb(hex(&t.accent)).opacity(0.22)
                            } else {
                                rgb(hex(&t.background)).opacity(0.66)
                            })
                            .border_1()
                            .border_color(if selected {
                                Hsla::from(rgb(hex(&t.accent)).opacity(0.72))
                            } else {
                                self.border_subtle()
                            })
                            .text_size(px(10.5))
                            .font_family(self.ui_font())
                            .text_color(if selected {
                                rgb(hex(&t.accent))
                            } else {
                                rgb(hex(&t.text))
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.act(Action::PickDiscoveredModel(mid.clone()), window, cx);
                                }),
                            )
                            .child(model_id.clone()),
                    );
                }
                card.child(
                    v_flex()
                        .gap_1p5()
                        .p_3()
                        .rounded_xl()
                        .bg(rgb(hex(&t.background)).opacity(0.52))
                        .border_1()
                        .border_color(self.border_subtle())
                        .child(
                            div()
                                .text_size(px(10.5))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.accent)))
                                .child("已发现可用模型（点击即可一键选用）："),
                        )
                        .child(chips),
                )
            })
            .child(
                div()
                    .text_size(px(11.))
                    .font_family(self.ui_font())
                    .line_height(relative(1.6))
                    .text_color(rgb(hex(&t.muted)))
                    .child(if is_local {
                        "离线大模型模式：支持本机或局域网 Ollama（11434）、LM Studio（1234）、llama.cpp（8080）与 vLLM。推理全程不出内网、断网可用、零 API 费用；点击下方「测试连接并拉取模型」可自动列出已安装模型。"
                    } else if cur_provider == "custom" {
                        "自定义 URL 模式：兼容任意 OpenAI 协议地址，可直接粘贴根域名、/v1 地址（如 OpenRouter、One-API、New-API、Gemini 兼容端点）或完整 .../chat/completions、.../responses 路径；局域网/免鉴权中转允许留空 Key。"
                    } else {
                        "配置后将同时驱动「防剧透前情提要」「动态人物关系图谱」「段落共读」与「此刻心情」；保留原有 .env.local 自动兼容。"
                    }),
            )
            .child(
                h_flex()
                    .gap_2p5()
                    .items_center()
                    .flex_wrap()
                    .child(
                        self.button_with_icon(
                            "保存 AI 配置",
                            IconName::Check,
                            Action::SaveAiSettings,
                            cx,
                        )
                        .primary(),
                    )
                    .child(
                        self.button_with_icon(
                            if self.ai_test_rx.is_some() {
                                "正在检测并拉取模型…"
                            } else {
                                "测试连接并拉取模型"
                            },
                            IconName::RefreshCw,
                            Action::TestAiConnection,
                            cx,
                        )
                        .outline()
                        .disabled(self.ai_test_rx.is_some()),
                    ),
            )
            .into_any_element()
    }

    #[inline(never)]
    pub(super) fn settings_sync_card(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let is_dark = self.is_dark();
        let sync = &self.store.state.sync;
        let mode = sync.mode.as_str();

        let modes: &[(&str, &str)] = &[
            ("off", "暂不开启"),
            ("github", "GitHub 私有仓库（推荐）"),
            ("folder", "同步盘目录（坚果云 / OneDrive）"),
        ];
        let mut mode_pills = h_flex().gap_2().flex_wrap();
        for &(mid, label) in modes {
            let selected = mode == mid;
            let mid_owned = mid.to_owned();
            mode_pills = mode_pills.child(
                div()
                    .id(SharedString::from(format!("sync-mode-{mid}")))
                    .cursor_pointer()
                    .px_3()
                    .py_1p5()
                    .rounded_full()
                    .bg(if selected {
                        rgb(hex(&t.accent)).opacity(0.20)
                    } else {
                        rgb(hex(&t.background)).opacity(if is_dark { 0.65 } else { 0.78 })
                    })
                    .border_1()
                    .border_color(if selected {
                        Hsla::from(rgb(hex(&t.accent)).opacity(0.70))
                    } else {
                        self.border_subtle()
                    })
                    .text_size(px(11.5))
                    .font_family(self.ui_font())
                    .text_color(if selected {
                        rgb(hex(&t.accent))
                    } else {
                        rgb(hex(&t.text))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::SelectSyncMode(mid_owned.clone()), window, cx);
                        }),
                    )
                    .child(label),
            );
        }

        let mut card = v_flex()
            .p_5()
            .gap_4()
            .rounded_2xl()
            .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.90 } else { 0.94 }))
            .border_1()
            .border_color(self.border_subtle())
            .shadow_sm()
            .child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(self.studio_badge(
                                "MULTI-DEVICE SYNC",
                                "GITHUB PRIVATE REPO & CLOUD FOLDER",
                                true,
                            ))
                            .child(
                                div()
                                    .text_size(px(19.5))
                                    .font_family(self.display_font())
                                    .child("多端双向同步（零服务器 · 冲突无损合并）"),
                            ),
                    )
                    .when_some(sync.last_sync_at.as_ref(), |el, ts| {
                        el.child(
                            div()
                                .px_2p5()
                                .py_1()
                                .rounded_full()
                                .bg(rgb(0x36c98e).opacity(0.14))
                                .text_size(px(10.))
                                .font_family(self.ui_font())
                                .text_color(rgb(0x28996b))
                                .child(format!(
                                    "最近同步：{}",
                                    ts.chars().take(19).collect::<String>().replace('T', " ")
                                )),
                        )
                    }),
            )
            .child(mode_pills);

        if mode == "github" {
            card = card
                .child(
                    h_flex()
                        .gap_4()
                        .flex_wrap()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(180.))
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child("GitHub 用户名（Owner）"),
                                )
                                .child(Input::new(&self.sync_owner_input)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(200.))
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child("私有仓库名（Repo）"),
                                )
                                .child(Input::new(&self.sync_repo_input)),
                        )
                        .child(
                            v_flex()
                                .w(px(130.))
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child("分支（Branch）"),
                                )
                                .child(Input::new(&self.sync_branch_input)),
                        ),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .flex_wrap()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(260.))
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child("Personal Access Token（仅存本机，绝不提交到仓库）"),
                                )
                                .child(Input::new(&self.sync_token_input)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(220.))
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child("GitHub API 地址（可填镜像代理）"),
                                )
                                .child(Input::new(&self.sync_api_base_input)),
                        ),
                )
                .child(
                    div()
                        .p_3()
                        .rounded_xl()
                        .bg(rgb(hex(&t.accent)).opacity(0.08))
                        .text_size(px(11.))
                        .font_family(self.ui_font())
                        .line_height(relative(1.6))
                        .text_color(rgb(hex(&t.muted)))
                        .child("安全建议：请在 GitHub 新建一个 Private 私有仓库（如 glow-reading-sync），创建 Fine-grained Token 且只勾选该仓库的 Contents: Read and write。同步基于无损三向合并，多台电脑同时划线、写手账、更新进度绝不互相覆盖；API Key 与 Token 严格留在各设备本地。"),
                );
        } else if mode == "folder" {
            card = card.child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .flex_wrap()
                    .p_3()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(0.58))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.text)))
                            .child(if sync.folder_path.is_empty() {
                                "尚未选择目录（可指向坚果云、OneDrive、iCloud Drive 或 Syncthing 本地目录）"
                                    .to_owned()
                            } else {
                                format!("当前同步目录：{}", sync.folder_path)
                            }),
                    )
                    .child(self.button_with_icon(
                        "选择目录",
                        IconName::Download,
                        Action::PickSyncFolder,
                        cx,
                    )),
            );
        }

        if mode != "off" {
            card = card
                .child(
                    h_flex()
                        .gap_3()
                        .flex_wrap()
                        .child(
                            self.button(
                                if sync.sync_books {
                                    "同步 ≤15MB 电子书原文：已开"
                                } else {
                                    "同步 ≤15MB 电子书原文：仅同步进度与笔记"
                                },
                                Action::ToggleSyncBooks,
                                cx,
                            )
                            .toggled(sync.sync_books),
                        )
                        .child(
                            self.button(
                                if sync.auto_sync_on_exit {
                                    "退出时自动同步：已开"
                                } else {
                                    "退出时自动同步：已关"
                                },
                                Action::ToggleSyncOnExit,
                                cx,
                            )
                            .toggled(sync.auto_sync_on_exit),
                        ),
                )
                .when_some(sync.last_sync_summary.as_ref(), |el, sum| {
                    el.child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child(format!("上次同步记录：{sum}")),
                    )
                })
                .child(
                    h_flex()
                        .gap_2p5()
                        .flex_wrap()
                        .child(
                            self.button_with_icon(
                                if self.sync_rx.is_some() {
                                    "正在双向同步…"
                                } else {
                                    "立即双向同步"
                                },
                                IconName::RefreshCw,
                                Action::RunSyncNow,
                                cx,
                            )
                            .primary()
                            .disabled(self.sync_rx.is_some()),
                        )
                        .child(
                            self.button_with_icon(
                                if self.sync_test_rx.is_some() {
                                    "正在验证…"
                                } else {
                                    "测试连接"
                                },
                                IconName::ShieldCheck,
                                Action::TestSyncConnection,
                                cx,
                            )
                            .outline()
                            .disabled(self.sync_test_rx.is_some()),
                        )
                        .child(self.button_with_icon(
                            "保存设置",
                            IconName::Check,
                            Action::SaveSyncSettings,
                            cx,
                        )),
                );
        }

        card.into_any_element()
    }
}

fn relation_tone_color(tone: &str, default_accent: u32) -> u32 {
    match tone {
        "family" => 0x34d399,
        "conflict" => 0xfb7185,
        "mystery" => 0xf59e0b,
        _ => default_accent,
    }
}

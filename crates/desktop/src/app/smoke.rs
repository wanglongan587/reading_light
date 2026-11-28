use super::*;

impl ReadingApp {
    pub(super) fn smoke_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.smoke_started.elapsed() > Duration::from_secs(75) {
            eprintln!(
                "GLOW_SMOKE_FAILED timeout step={} error={:?}",
                self.smoke_step, self.error
            );
            cx.quit();
            return;
        }
        match self.smoke_step {
            0 => {
                self.store.state.privacy_seen = true;
                if std::env::var_os("GLOW_SMOKE_FAKE_MOOD").is_some() {
                    self.custom_mood_worker_override = true;
                    self.mood_worker = |_, _| {
                        Ok(EchoResult {
                            quotes: vec![],
                            reply: "这是原生端到端测试生成的模拟回复。书页、书签和日记保存流程均已走通。"
                                .into(),
                            demo: false,
                        })
                    };
                    println!("GLOW_SMOKE mock_mood");
                }
                if let Some(path) = std::env::var_os("GLOW_SMOKE_IMPORT") {
                    match self.store.import_book(&PathBuf::from(path), None) {
                        Ok(id) => {
                            self.active_book = Some(id);
                            println!("GLOW_SMOKE external_import_ok");
                        }
                        Err(error) => {
                            self.error = Some(error.to_string());
                            eprintln!("GLOW_SMOKE external_import_failed: {error}");
                        }
                    }
                }
                self.smoke_step = 10;
                self.smoke_started = Instant::now();
                cx.notify();
            }
            10 if self.smoke_started.elapsed() > Duration::from_secs(2) => {
                #[cfg(feature = "ui-test")]
                match window.render_to_image() {
                    Ok(image) => {
                        let _ = image.save(self.store.root.join("shelf.png"));
                    }
                    Err(e) => eprintln!("Native visual capture: {e}"),
                }
                if let Some(b) = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|book| Some(&book.id) == self.active_book.as_ref())
                    .or_else(|| self.store.state.books.first())
                {
                    let id = b.id.clone();
                    if std::env::var_os("GLOW_SMOKE_DEFER_OPEN").is_some() {
                        cx.defer_in(window, move |this, window, cx| {
                            this.act(Action::Open(id), window, cx);
                        });
                    } else {
                        self.act(Action::Open(id), window, cx);
                    }
                    self.smoke_step = 1;
                }
            }
            1 if self.opened => {
                println!(
                    "GLOW_SMOKE opened toc={} scale={}",
                    self.toc.len(),
                    window.scale_factor()
                );
                if let Some(entry) = self.toc.get(3).or_else(|| self.toc.first()) {
                    self.act(Action::Locate(entry.href.clone()), window, cx);
                }
                self.smoke_step = 12;
                self.smoke_started = Instant::now();
            }
            12 if self.smoke_started.elapsed() > Duration::from_secs(1) => {
                if let Some(view) = &self.webview {
                    let script = r#"(()=>{let tries=0;const timer=setInterval(()=>{if(++tries>100){clearInterval(timer);return;}const docs=Array.from(document.querySelectorAll('iframe')).map(f=>f.contentDocument).filter(d=>d?.documentElement?.dataset.glowBound==='yes');const d=docs.find(d=>Array.from(d.querySelectorAll('p')).some(p=>p.textContent.trim().length>12));if(!d)return;const p=Array.from(d.querySelectorAll('p')).find(p=>p.textContent.trim().length>12);const r=d.createRange();r.selectNodeContents(p);const s=d.getSelection();s.removeAllRanges();s.addRange(r);p.dispatchEvent(new MouseEvent('mouseup',{bubbles:true}));setTimeout(()=>{const button=document.querySelector('#echo');if(!button.hidden){clearInterval(timer);button.click();}},60);},150);})()"#;
                    if let Err(e) = view.read(cx).raw().evaluate_script(script) {
                        self.error = Some(e.to_string());
                    }
                }
                self.smoke_step = 11;
            }
            11 if self.echo.is_some() => {
                self.act(Action::SaveEchoToJournal, window, cx);
                println!("GLOW_SMOKE selected_echo journal_saved");
                self.smoke_step = 37;
                self.smoke_started = Instant::now();
            }
            37 if self.smoke_started.elapsed() > Duration::from_secs(1) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("echo-panel.png"));
                }
                self.act(Action::Highlight, window, cx);
                self.act(Action::Bookmark, window, cx);
                self.act(Action::Mode, window, cx);
                self.act(Action::TypeSize(1), window, cx);
                self.act(Action::Typography, window, cx);
                self.smoke_step = 38;
                self.smoke_started = Instant::now();
            }
            38 if self.smoke_started.elapsed() > Duration::from_secs(1) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("typography.png"));
                }
                self.act(Action::Typography, window, cx);
                self.search_input.update(cx, |state, cx| {
                    state.set_value("孤独", window, cx);
                });
                self.act(Action::Search, window, cx);
                self.act(Action::SubmitSearch, window, cx);
                self.smoke_step = 34;
            }
            34 if !self.searching && !self.search_results.is_empty() => {
                println!("GLOW_SMOKE search_results={}", self.search_results.len());
                self.smoke_step = 36;
                self.smoke_started = Instant::now();
            }
            36 if self.smoke_started.elapsed() > Duration::from_secs(1) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("search.png"));
                }
                self.act(Action::Highlights, window, cx);
                self.smoke_step = 35;
                self.smoke_started = Instant::now();
            }
            35 if self.smoke_started.elapsed() > Duration::from_secs(1) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("highlights.png"));
                }
                self.act(Action::CloseEcho, window, cx);
                window.resize(size(px(1024.), px(720.)));
                self.smoke_step = 39;
                self.smoke_started = Instant::now();
            }
            39 if self.smoke_started.elapsed() > Duration::from_secs(1) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("reader-1024.png"));
                }
                window.resize(size(px(1280.), px(820.)));
                self.act(Action::Page(Page::Shelf), window, cx);
                self.smoke_step = 40;
                self.smoke_started = Instant::now();
            }
            40 if self.smoke_started.elapsed() > Duration::from_millis(600) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("shelf-resume.png"));
                }
                self.smoke_step = 3;
            }
            3 => {
                self.act(Action::Page(Page::Mood), window, cx);
                self.smoke_step = 30;
            }
            30 => {
                self.mood_input.update(cx, |state, cx| {
                    state.set_value("今天读了一会儿书。\n想把这段心情留下。", window, cx);
                });
                self.act(Action::SubmitMood, window, cx);
                self.smoke_step = 32;
                self.smoke_started = Instant::now();
            }
            32 if self.mood_rx.is_none() => {
                self.smoke_step = 33;
                self.smoke_started = Instant::now();
            }
            33 if self.smoke_started.elapsed() > Duration::from_secs(1) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("mood.png"));
                }
                self.act(Action::Page(Page::Journal), window, cx);
                self.smoke_step = 31;
            }
            31 => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("journal.png"));
                }
                self.act(Action::Page(Page::Themes), window, cx);
                self.smoke_step = 4;
                self.smoke_started = Instant::now();
            }
            4 if self.smoke_started.elapsed() > Duration::from_millis(600) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("themes.png"));
                }
                self.act(Action::PickTheme(3), window, cx);
                self.act(Action::Page(Page::Shelf), window, cx);
                self.motion.on_mouse_move(point(px(620.), px(190.)), false);
                self.motion.on_mouse_move(point(px(690.), px(210.)), false);
                self.motion.on_click(point(px(680.), px(230.)), false, true);
                self.motion.step(false, true);
                self.smoke_step = 41;
                self.smoke_started = Instant::now();
            }
            41 if self.smoke_started.elapsed() > Duration::from_millis(600) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("prism-shelf.png"));
                }
                self.act(Action::Page(Page::Mood), window, cx);
                self.motion.on_mouse_move(point(px(470.), px(260.)), false);
                self.motion.on_click(point(px(470.), px(260.)), false, true);
                self.motion.step(false, true);
                self.smoke_step = 42;
                self.smoke_started = Instant::now();
            }
            42 if self.smoke_started.elapsed() > Duration::from_millis(600) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("prism-mood.png"));
                }
                self.motion = MotionEngine::new();
                self.act(Action::PickTheme(4), window, cx);
                self.act(Action::Page(Page::Shelf), window, cx);
                self.smoke_step = 43;
                self.smoke_started = Instant::now();
            }
            43 if self.smoke_started.elapsed() > Duration::from_millis(500) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("totoro-shelf.png"));
                }
                self.act(Action::PickTheme(5), window, cx);
                self.smoke_step = 44;
                self.smoke_started = Instant::now();
            }
            44 if self.smoke_started.elapsed() > Duration::from_millis(500) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("spirited-shelf.png"));
                }
                self.act(Action::PickTheme(8), window, cx);
                self.smoke_step = 45;
                self.smoke_started = Instant::now();
            }
            45 if self.smoke_started.elapsed() > Duration::from_millis(500) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("lantern-shelf.png"));
                }
                self.act(Action::PickTheme(6), window, cx);
                self.smoke_step = 46;
                self.smoke_started = Instant::now();
            }
            46 if self.smoke_started.elapsed() > Duration::from_millis(450) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("howl-shelf.png"));
                }
                self.act(Action::PickTheme(7), window, cx);
                self.smoke_step = 47;
                self.smoke_started = Instant::now();
            }
            47 if self.smoke_started.elapsed() > Duration::from_millis(450) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("kiki-shelf.png"));
                }
                self.act(Action::PickTheme(5), window, cx);
                self.act(Action::ToggleTimerPicker, window, cx);
                self.smoke_step = 48;
                self.smoke_started = Instant::now();
            }
            48 if self.smoke_started.elapsed() > Duration::from_millis(450) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("focus-timer-modal.png"));
                }
                self.act(Action::CloseTimerPicker, window, cx);
                self.act(Action::Page(Page::Radio), window, cx);
                self.act(
                    Action::RadioTuneStation("glow-night-talk".into()),
                    window,
                    cx,
                );
                self.smoke_step = 49;
                self.smoke_started = Instant::now();
            }
            49 if self.smoke_started.elapsed() > Duration::from_millis(900) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("radio.png"));
                }
                self.act(Action::RadioLampColor("starry".into()), window, cx);
                self.act(Action::RadioLampMode("breathe_478".into()), window, cx);
                self.act(Action::RadioTogglePureGlow, window, cx);
                self.smoke_step = 50;
                self.smoke_started = Instant::now();
            }
            50 if self.smoke_started.elapsed() > Duration::from_millis(700) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("radio-pure-glow.png"));
                }
                self.act(Action::RadioTogglePureGlow, window, cx);
                self.act(Action::RadioLampColor("amber".into()), window, cx);
                self.act(Action::RadioLampMode("audio".into()), window, cx);
                self.act(Action::RadioToggleDrawer, window, cx);
                self.smoke_step = 51;
                self.smoke_started = Instant::now();
            }
            51 if self.smoke_started.elapsed() > Duration::from_millis(550) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("radio-drawer.png"));
                }
                self.act(Action::RadioCloseDrawer, window, cx);
                self.act(Action::PickTheme(3), window, cx);
                self.act(Action::Page(Page::Shelf), window, cx);
                self.smoke_step = 52;
                self.smoke_started = Instant::now();
            }
            52 if self.smoke_started.elapsed() > Duration::from_millis(600) => {
                #[cfg(feature = "ui-test")]
                if let Ok(image) = window.render_to_image() {
                    let _ = image.save(self.store.root.join("radio-bg-shelf.png"));
                }
                if self.radio_status.playing {
                    println!(
                        "GLOW_SMOKE radio_ok station={} mhz={:.1}",
                        self.radio_status.station_id, self.radio_status.frequency_mhz
                    );
                }
                self.stop_radio_if_playing();
                self.act(Action::Page(Page::Reader), window, cx);
                self.smoke_step = 5;
                self.smoke_started = Instant::now();
            }
            5 if self.smoke_started.elapsed() > Duration::from_secs(3) => {
                let location = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|book| Some(&book.id) == self.active_book.as_ref())
                    .and_then(|b| b.location.as_ref());
                let bookmarks = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|book| Some(&book.id) == self.active_book.as_ref())
                    .map(|book| book.bookmarks.len())
                    .unwrap_or_default();
                let highlights = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|book| Some(&book.id) == self.active_book.as_ref())
                    .map(|book| book.highlights.len())
                    .unwrap_or_default();
                let progress = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|book| Some(&book.id) == self.active_book.as_ref())
                    .and_then(|book| book.progress);
                if self.error.is_none()
                    && location.is_some()
                    && !self.toc.is_empty()
                    && bookmarks > 0
                    && highlights > 0
                    && self.store.state.reader_font_size.is_some()
                    && !self.search_results.is_empty()
                    && progress.is_some_and(|progress| (0.0..=1.0).contains(&progress))
                    && self
                        .mood_reply
                        .as_ref()
                        .is_some_and(|(_, reply)| !reply.demo && !reply.reply.is_empty())
                    && self.store.state.journal.iter().any(|entry| {
                        entry.kind == "mood"
                            && entry.input == "今天读了一会儿书。\n想把这段心情留下。"
                            && !entry.echo.demo
                    })
                {
                    println!(
                        "GLOW_SMOKE_OK native_window webview epub toc selection echo search highlight typography progress mood_ai journal bookmark mode theme visibility location"
                    );
                } else {
                    eprintln!(
                        "GLOW_SMOKE_FAILED error={:?} has_location={} toc={}",
                        self.error,
                        location.is_some(),
                        self.toc.len()
                    );
                }
                self.save();
                cx.quit();
            }
            _ => {}
        }
    }
}

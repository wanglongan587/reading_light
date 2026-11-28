use super::*;

impl ReadingApp {
    pub(super) fn mood_page(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let is_dark = self.is_dark();
        let snap = self.motion_snapshot();
        let mut moods = h_flex().gap_2().flex_wrap();
        for mood in echo::MOODS {
            moods = moods.child(self.mood_button(mood, self.mood == *mood, cx));
        }
        let mut left_console = v_flex()
            .relative()
            .overflow_hidden()
            .w(px(408.))
            .max_w_full()
            .flex_shrink_0()
            .p_5()
            .gap_4()
            .rounded_2xl()
            .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.90 } else { 0.94 }))
            .border_1()
            .border_color(self.border_strong())
            .shadow_md()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(prism::prism_beam_horizontal(snap.clone(), 2.0, 0.3, false)),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .child(prism::prism_beam_horizontal(snap.clone(), 2.0, 1.9, false)),
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
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(self.tr(
                                        "选一个最接近的情绪波段",
                                        "Pick the mood closest to how you feel",
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(if self.locale().is_english() {
                                        format!(
                                            "Now: {}",
                                            i18n::mood_name(self.locale(), &self.mood)
                                        )
                                    } else {
                                        format!("当前：{}", self.mood)
                                    }),
                            ),
                    )
                    .child(moods),
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
                                    .child(self.tr(
                                        "如果愿意，再写下一点（选填）",
                                        "Write a few lines if you wish (optional)",
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child({
                                        let count =
                                            self.mood_input.read(cx).value().trim().chars().count();
                                        if self.locale().is_english() {
                                            format!("{count} / 1000 chars")
                                        } else {
                                            format!("{count} / 1000 字")
                                        }
                                    }),
                            ),
                    )
                    .child(
                        Textarea::new(&self.mood_input)
                            .aria_label("心情文字")
                            .w_full(),
                    ),
            )
            .child(
                self.button_with_icon(
                    if self.mood_rx.is_some() {
                        self.tr("正在读你的文字…", "Listening to your words…")
                    } else if self.mood_unsaved.as_ref().is_some_and(|pending| {
                        pending.input == self.mood_input.read(cx).value().trim()
                            && pending.mood == self.mood
                    }) {
                        self.tr("重试保存回答", "Retry Saving Reply")
                    } else {
                        self.tr("让文字陪我一会儿", "Sit With Words for a While")
                    },
                    IconName::MessageCircleHeart,
                    Action::SubmitMood,
                    cx,
                )
                .primary()
                .disabled(self.mood_rx.is_some()),
            );
        if self.mood_rx.is_some() {
            left_console = left_console.child(self.button(
                self.tr("停止等待", "Stop Waiting"),
                Action::CancelMood,
                cx,
            ));
        }
        left_console = left_console.child(
            h_flex()
                .gap_2()
                .items_start()
                .pt_1()
                .border_t_1()
                .border_color(self.border_subtle())
                .child(
                    Icon::new(IconName::Feather)
                        .size(px(12.))
                        .text_color(rgb(hex(&t.accent))),
                )
                .child(
                    div()
                        .text_size(px(10.5))
                        .font_family(self.ui_font())
                        .line_height(relative(1.5))
                        .text_color(rgb(hex(&t.muted)))
                        .flex_1()
                        .min_w(px(0.))
                        .child(self.tr(
                            "点击后将本次文字与所选心情发送给已配置的大模型生成回答（支持本地 Ollama 完全离线）；不会发送书库或历史日记，成功后保存在本机。",
                            "Sends only this note and selected mood to your configured LLM (supports offline Ollama). Never uploads your library or past journal.",
                        )),
                ),
        );

        let left_stage = div()
            .relative()
            .w(px(408.))
            .max_w_full()
            .flex_shrink_0()
            .pt(px(22.))
            .child(left_console)
            .child(
                div()
                    .absolute()
                    .top(px(-2.))
                    .right(px(20.))
                    .child(self.pet_dock("mood-pet", cx)),
            );

        let breathing_card = if let Some(started) = self.breathing_started {
            let elapsed = started.elapsed().as_secs_f32();
            let bp = prism::breathing_phase(elapsed);
            let session_left = (60.0 - elapsed).max(0.0).ceil() as u32;
            let session_ratio = (elapsed / 60.0).clamp(0.0, 1.0);
            v_flex()
                .relative()
                .w_full()
                .p_5()
                .gap_3()
                .rounded_2xl()
                .overflow_hidden()
                .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.92 } else { 0.94 }))
                .border_1()
                .border_color(rgb(hex(&t.accent)).opacity(0.45))
                .shadow_md()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .child(prism::prism_beam_horizontal(snap.clone(), 2.0, 2.6, false)),
                )
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(self.studio_badge(
                            "4-7-8",
                            self.tr("棱镜呼吸光环 · 进行中", "PRISM BREATHING · IN PROGRESS"),
                            true,
                        ))
                        .child(self.button_with_icon(
                            self.tr("结束呼吸", "End Breathing"),
                            IconName::X,
                            Action::ToggleBreathing,
                            cx,
                        )),
                )
                .child(
                    h_flex()
                        .gap_5()
                        .items_center()
                        .child(prism::breathing_orb_canvas(snap.clone(), elapsed))
                        .child(
                            v_flex()
                                .flex_1()
                                .gap_1p5()
                                .child(
                                    h_flex()
                                        .gap_2p5()
                                        .items_baseline()
                                        .child(
                                            div()
                                                .text_size(px(24.))
                                                .font_family(self.display_font())
                                                .text_color(rgb(hex(&t.text)))
                                                .child(bp.label),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(16.))
                                                .font_family(self.ui_font())
                                                .text_color(rgb(hex(&t.accent)))
                                                .child(format!("{} s", bp.seconds_left)),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child(bp.cue),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .pt_1()
                                        .child(
                                            div()
                                                .w(px(140.))
                                                .h(px(4.))
                                                .rounded_full()
                                                .bg(rgb(hex(&t.text)).opacity(0.10))
                                                .child(
                                                    div()
                                                        .w(px((140.0 * session_ratio).max(4.0)))
                                                        .h_full()
                                                        .rounded_full()
                                                        .bg(rgb(hex(&t.accent))),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .font_family(self.ui_font())
                                                .text_color(rgb(hex(&t.muted)))
                                                .child(if self.locale().is_english() {
                                                    format!("{session_left}s left")
                                                } else {
                                                    format!("还剩 {session_left} 秒")
                                                }),
                                        ),
                                ),
                        ),
                )
                .into_any_element()
        } else {
            let (hud_hex, _, hud_wave_tint) = self.theme_hud_palette();
            let hud_bg = rgb(hud_hex).opacity(0.72);
            div()
                .relative()
                .w_full()
                .h(px(168.))
                .rounded_2xl()
                .overflow_hidden()
                .border_1()
                .border_color(self.border_strong())
                .shadow_sm()
                .child(
                    img(self.scene_asset())
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .child(prism::prism_beam_horizontal(snap, 1.5, 2.6, false)),
                )
                .child(
                    h_flex()
                        .absolute()
                        .left_4()
                        .right_4()
                        .bottom_4()
                        .p_3p5()
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .rounded_xl()
                        .bg(hud_bg)
                        .border_1()
                        .border_color(rgb(0xffffff).opacity(0.18))
                        .child(
                            v_flex()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_size(px(16.5))
                                        .font_family(self.display_font())
                                        .text_color(rgb(0xfffdf7))
                                        .child(self.tr("先不急着变好。", "No rush to feel better.")),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(0xd8e4dd))
                                        .child(self.tr(
                                            "在这里坐一会儿，让一句话陪你呼吸。",
                                            "Sit here for a quiet minute and breathe with a gentle line.",
                                        )),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_2p5()
                                .items_center()
                                .child(self.waveform_bars_tinted(
                                    true,
                                    &[6., 13., 18., 9., 15., 11., 7.],
                                    hud_wave_tint,
                                ))
                                .child(
                                    self.button_with_icon(
                                        self.tr("陪我呼吸 1 分钟", "Breathe 1 Min"),
                                        IconName::Wind,
                                        Action::ToggleBreathing,
                                        cx,
                                    )
                                    .primary(),
                                ),
                        ),
                )
                .into_any_element()
        };

        let right_stage = v_flex()
            .flex_1()
            .min_w(px(330.))
            .pt(px(22.))
            .gap_4()
            .child(breathing_card)
            .child(if let Some((input, echo)) = &self.mood_reply {
                v_flex()
                    .p_5()
                    .gap_3()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.90 } else { 0.94 }))
                    .border_1()
                    .border_color(self.border_subtle())
                    .shadow_sm()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(self.tr(
                                        "给你的回响 · LIVE RESONANCE",
                                        "LIVE RESONANCE · FOR YOU",
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(self.tr(
                                        "已自动存入本地私人日记",
                                        "Saved automatically to local journal",
                                    )),
                            ),
                    )
                    .child(self.echo_card(echo, input, false, cx))
                    .into_any_element()
            } else {
                v_flex()
                    .p_6()
                    .gap_3()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.78 } else { 0.86 }))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::Sparkles)
                                    .size(px(16.))
                                    .text_color(rgb(hex(&t.accent))),
                            )
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_family(self.display_font())
                                    .child(self.tr(
                                        "情绪波段待机中 · 等待你的声音",
                                        "Quiet Resonance · Listening When You’re Ready",
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(self.ui_font())
                            .line_height(relative(1.65))
                            .text_color(rgb(hex(&t.muted)))
                            .child(self.tr(
                                "在左侧选一种接近此刻的状态，或者写下几句想说的话。生成的微光陪伴会在这里展开，并为你留在本地日记里。",
                                "Choose a mood on the left or write a few words. Your calm reflection will unfold here and stay in your private local journal.",
                            )),
                    )
                    .into_any_element()
            });

        v_flex()
            .id("mood-scroll")
            .track_scroll(&self.mood_scroll)
            .overflow_y_scroll()
            .size_full()
            .px_7()
            .py_6()
            .gap_5()
            .child(self.heading(
                self.tr("A PLACE TO REST · 情绪调频", "A PLACE TO REST · MOOD TUNER"),
                self.tr("此刻，你感觉怎么样？", "How are you feeling right now?"),
                self.tr(
                    "不用把心情整理好再来。选一种接近的感觉就可以。",
                    "No need to sort everything out first — pick whatever feels closest.",
                ),
            ))
            .child(
                h_flex()
                    .gap_5()
                    .items_start()
                    .flex_wrap()
                    .child(left_stage)
                    .child(right_stage),
            )
            .into_any_element()
    }
    pub(super) fn journal(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let is_dark = self.is_dark();
        let fav_count = self
            .store
            .state
            .journal
            .iter()
            .filter(|e| e.favorite)
            .count();
        let query = self.journal_search.read(cx).value().trim().to_lowercase();
        let filtered: Vec<&JournalEntry> = self
            .store
            .state
            .journal
            .iter()
            .filter(|entry| {
                let kind_ok = match self.journal_filter {
                    JournalFilter::All => true,
                    JournalFilter::Favorite => entry.favorite,
                    JournalFilter::Mood => entry.kind == "mood",
                    JournalFilter::Selection => entry.kind == "selection",
                    JournalFilter::Ticket => entry.kind == "ticket",
                };
                if !kind_ok {
                    return false;
                }
                if query.is_empty() {
                    return true;
                }
                entry.input.to_lowercase().contains(&query)
                    || entry.echo.reply.to_lowercase().contains(&query)
                    || entry
                        .mood
                        .as_deref()
                        .is_some_and(|m| m.to_lowercase().contains(&query))
            })
            .collect();

        // Mood Spectrum Bar: last 14 entries rendered purely as soft color temperature dots
        let mut spectrum_dots = h_flex().gap_1p5().items_center();
        for entry in self.store.state.journal.iter().take(14).rev() {
            let dot_hex = mood_spectrum_color(entry.mood.as_deref());
            spectrum_dots = spectrum_dots.child(
                div()
                    .w(px(12.))
                    .h(px(7.))
                    .rounded_full()
                    .bg(rgb(dot_hex).opacity(0.88)),
            );
        }

        let en = self.locale().is_english();
        let mut filter_bar = h_flex().gap_1p5().items_center().flex_wrap();
        for (label, filter) in [
            (self.tr("全部", "All"), JournalFilter::All),
            (self.tr("已收藏", "Starred"), JournalFilter::Favorite),
            (self.tr("心情", "Mood"), JournalFilter::Mood),
            (self.tr("阅读回响", "Echoes"), JournalFilter::Selection),
            (self.tr("完读票根", "Tickets"), JournalFilter::Ticket),
        ] {
            filter_bar = filter_bar.child(self.pill_button(
                label,
                None,
                self.journal_filter == filter,
                Action::JournalFilter(filter),
                cx,
            ));
        }

        // Anti-anxiety Monthly Glow Calendar (zero streaks, zero penalties)
        let (ym, year, month) = current_ym();
        let monthly =
            self.store
                .monthly_reading_summary(&ym)
                .unwrap_or(glow_core::MonthlyReadingSummary {
                    year_month: ym.clone(),
                    total_minutes: 0,
                    active_days: 0,
                    daily_minutes: vec![],
                });
        let total_days = days_in_month(year, month);
        let mut day_cells = h_flex().gap_1p5().flex_wrap().items_center();
        for day in 1..=total_days {
            let key = format!("{year:04}-{month:02}-{day:02}");
            let mins = monthly
                .daily_minutes
                .iter()
                .find(|(d, _)| d == &key)
                .map(|(_, m)| *m)
                .unwrap_or(0);
            let opacity = if mins >= 40 {
                0.92
            } else if mins >= 15 {
                0.62
            } else if mins > 0 {
                0.34
            } else {
                0.08
            };
            day_cells = day_cells.child(
                div()
                    .w(px(22.))
                    .h(px(22.))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(if mins > 0 {
                        rgb(hex(&t.accent)).opacity(opacity)
                    } else {
                        rgb(hex(&t.text)).opacity(0.06)
                    })
                    .border_1()
                    .border_color(if mins > 0 {
                        Hsla::from(rgb(hex(&t.accent)).opacity(0.45))
                    } else {
                        self.border_subtle()
                    })
                    .text_size(px(9.))
                    .font_family(self.ui_font())
                    .text_color(if mins >= 40 {
                        rgb(hex(&t.paper))
                    } else if mins > 0 {
                        rgb(hex(&t.text))
                    } else {
                        rgb(hex(&t.muted)).opacity(0.65)
                    })
                    .child(format!("{day}")),
            );
        }
        let finished_books_count = self
            .store
            .state
            .books
            .iter()
            .filter(|b| b.effective_status() == "finished")
            .count();
        let calendar_card = v_flex()
            .id("monthly-glow-calendar")
            .p_4()
            .gap_3()
            .rounded_2xl()
            .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.86 } else { 0.92 }))
            .border_1()
            .border_color(self.border_subtle())
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .flex_wrap()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_family(self.display_font())
                                    .child(if en {
                                        format!("{year}-{month:02} · Glow Calendar")
                                    } else {
                                        format!("{year} 年 {month} 月 · 微光日历")
                                    }),
                            )
                            .child(
                                div()
                                    .px_2p5()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgb(hex(&t.accent)).opacity(0.14))
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(if en {
                                        format!(
                                            "{} glowing days · {} min · {} finished",
                                            monthly.active_days,
                                            monthly.total_minutes,
                                            finished_books_count
                                        )
                                    } else {
                                        format!(
                                            "{} 个有微光的日子 · 共 {} 分钟 · 已完读 {} 本",
                                            monthly.active_days,
                                            monthly.total_minutes,
                                            finished_books_count
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child(self.tr(
                                "无连续打卡压力 · 留白的日子同样美好",
                                "Zero streak pressure · Quiet days belong here too",
                            )),
                    ),
            )
            .child(day_cells);

        // Compact inline mood composer inside the unified Journal axis
        let mut quick_moods = h_flex().gap_1p5().flex_wrap().items_center();
        for m in echo::MOODS {
            quick_moods = quick_moods.child(self.pill_button(
                i18n::mood_name(self.locale(), m),
                None,
                self.mood == *m,
                Action::Mood((*m).into()),
                cx,
            ));
        }
        let inline_composer = v_flex()
            .p_4()
            .gap_3()
            .rounded_2xl()
            .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.86 } else { 0.92 }))
            .border_1()
            .border_color(self.border_subtle())
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_family(self.display_font())
                            .child(self.tr("随手记下此刻心绪", "Capture a Quiet Thought")),
                    )
                    .child(quick_moods),
            )
            .child(
                Textarea::new(&self.mood_input)
                    .aria_label("手账此刻心绪")
                    .w_full(),
            )
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                self.button_with_icon(
                                    if self.mood_rx.is_some() {
                                        self.tr("正在写回响…", "Writing echo…")
                                    } else {
                                        self.tr("记入手账并收下微光", "Save & Receive Glow")
                                    },
                                    IconName::MessageCircleHeart,
                                    Action::SubmitMood,
                                    cx,
                                )
                                .primary()
                                .disabled(self.mood_rx.is_some()),
                            )
                            .child(self.button_with_icon(
                                self.tr("4-7-8 呼吸室", "4-7-8 Breathing"),
                                IconName::Wind,
                                Action::Page(Page::Mood),
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child(self.tr(
                                "保存在本地 SQLite，随时可开启私密遮罩",
                                "Stored locally in SQLite · Privacy Veil available anytime",
                            )),
                    ),
            );

        let locked = self.store.state.journal_locked;
        let rediscovery_postcard = (!locked).then(|| self.daily_rediscovery()).flatten().map(
            |(quote, source, target, total)| {
                let mut actions = h_flex().gap_2().items_center().flex_wrap();
                if let Some(loc) = target {
                    actions = actions.child(
                        self.button_with_icon(
                            self.tr("回到书中这一段", "Jump to Passage"),
                            IconName::ArrowUpRight,
                            Action::OpenAt(Box::new(loc)),
                            cx,
                        )
                        .primary(),
                    );
                }
                if total > 1 {
                    actions = actions.child(self.button_with_icon(
                        self.tr("换一封", "Another"),
                        IconName::RefreshCw,
                        Action::NextRediscovery,
                        cx,
                    ));
                }
                h_flex()
                    .id("journal-rediscovery-postcard")
                    .justify_between()
                    .items_center()
                    .flex_wrap()
                    .gap_4()
                    .p_4()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.88 } else { 0.94 }))
                    .border_1()
                    .border_color(rgb(hex(&t.accent)).opacity(0.32))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(240.))
                            .gap_1p5()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Icon::new(IconName::Sparkles)
                                            .size(px(12.5))
                                            .text_color(rgb(hex(&t.accent))),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.accent)))
                                            .child(if en {
                                                format!("Postcard from Your Past Self ({source})")
                                            } else {
                                                format!("跨时空重逢 · 过去寄来的明信片（{source}）")
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_family(self.display_font())
                                    .line_height(relative(1.6))
                                    .text_color(rgb(hex(&t.text)))
                                    .child(format!("「{quote}」")),
                            ),
                    )
                    .child(actions)
            },
        );

        let mut content = v_flex()
            .id("journal-scroll")
            .overflow_y_scroll()
            .size_full()
            .px_7()
            .py_6()
            .gap_5()
            .child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .flex_wrap()
                    .gap_4()
                    .child(self.heading(
                        self.tr("LITTLE THINGS KEPT · 微光手账档案", "LITTLE THINGS KEPT · PERSONAL ARCHIVE"),
                        self.tr("那些被文字接住的时刻", "Moments Held by Words"),
                        self.tr(
                            "书摘、心绪与完读票根全部保存在这台电脑上，支持随时检索、私密遮罩与导出 Markdown。",
                            "Quotes, reflections, and completion tickets stay private on this device with local FTS5 search, Privacy Veil, and Obsidian/Notion export.",
                        ),
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .flex_wrap()
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded_full()
                                    .bg(rgb(hex(&t.paper)).opacity(0.84))
                                    .border_1()
                                    .border_color(self.border_subtle())
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(if en {
                                        format!(
                                            "{} entries  ·  {} starred",
                                            self.store.state.journal.len(),
                                            fav_count
                                        )
                                    } else {
                                        format!(
                                            "共 {} 篇  ·  收藏 {} 篇",
                                            self.store.state.journal.len(),
                                            fav_count
                                        )
                                    }),
                            )
                            .child(
                                self.button_with_icon(
                                    if locked {
                                        self.tr("解锁手账", "Unlock")
                                    } else {
                                        self.tr("私密遮罩", "Privacy Lock")
                                    },
                                    IconName::Focus,
                                    Action::ToggleJournalLock,
                                    cx,
                                )
                                .toggled(locked),
                            )
                            .child(self.button_with_icon(
                                self.tr("导出 Markdown", "Export MD"),
                                IconName::Download,
                                Action::ExportJournalMarkdown,
                                cx,
                            ))
                            .child(self.button_with_icon(
                                self.tr("Obsidian 笔记库", "Obsidian Vault"),
                                IconName::ArrowUpRight,
                                Action::ExportObsidianVault,
                                cx,
                            ))
                            .child(self.button_with_icon(
                                self.tr("Notion 书单 CSV", "Notion CSV"),
                                IconName::FileText,
                                Action::ExportNotionCsv,
                                cx,
                            ))
                            .child(self.button_with_icon(
                                self.tr("清空日记", "Clear"),
                                IconName::X,
                                Action::ClearJournal,
                                cx,
                            )),
                    ),
            )
            .child(calendar_card)
            .when_some(rediscovery_postcard, |this, card| this.child(card))
            .child(inline_composer);
        if !self.store.state.journal.is_empty() {
            content = content.child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .flex_wrap()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.86 } else { 0.90 }))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .flex_wrap()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child("近期情绪光谱"),
                            )
                            .child(spectrum_dots),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .flex_wrap()
                            .child(filter_bar)
                            .child(
                                div()
                                    .w(px(220.))
                                    .child(Input::new(&self.journal_search).w_full()),
                            ),
                    ),
            );
        }
        if self.store.state.journal.is_empty() {
            content = content.child(
                v_flex()
                    .w_full()
                    .py_12()
                    .items_center()
                    .gap_4()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(0.76))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        div()
                            .size(px(58.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(hex(&t.accent)).opacity(0.12))
                            .child(
                                Icon::new(IconName::NotebookPen)
                                    .size(px(24.))
                                    .text_color(rgb(hex(&t.accent))),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_family(self.display_font())
                            .child("这里还很安静。"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child("下一次回响或完读票根，会替你记住那个时刻。"),
                    ),
            );
        } else if filtered.is_empty() {
            content = content.child(
                v_flex()
                    .w_full()
                    .py_10()
                    .items_center()
                    .gap_2()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(0.76))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_family(self.display_font())
                            .child("没有符合当前筛选条件的手账记录"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child("试试切换筛选标签或清空关键词。"),
                    ),
            );
        }
        for entry in filtered {
            let display_input = if entry.input.trim().is_empty() {
                entry
                    .mood
                    .as_ref()
                    .map(|mood| format!("此刻的心情：{mood}"))
                    .unwrap_or_else(|| "此刻的心情".into())
            } else {
                entry.input.clone()
            };
            let card_draft = QuoteCardDraft {
                text: display_input.clone(),
                attribution: format!(
                    "{} · {}",
                    entry.mood.as_deref().unwrap_or("阅读回响"),
                    display_time(&entry.created_at)
                ),
                note: Some(entry.echo.reply.clone()),
                style: QuoteCardStyle::from_theme_id(&t.id),
            };
            let badge_bg = mood_spectrum_color(entry.mood.as_deref());
            let badge_text = mood_badge_text_color(entry.mood.as_deref(), is_dark);
            let mut bar = h_flex()
                .gap_2()
                .pb_3()
                .border_b_1()
                .border_color(self.border_subtle())
                .items_center()
                .flex_wrap()
                .child(
                    h_flex()
                        .flex_1()
                        .gap_2p5()
                        .items_center()
                        .child(
                            div()
                                .px_2p5()
                                .py_0p5()
                                .rounded_full()
                                .bg(rgb(badge_bg).opacity(0.16))
                                .text_size(px(10.5))
                                .font_family(self.ui_font())
                                .text_color(rgb(badge_text))
                                .child(if entry.kind == "ticket" {
                                    "完读票根".to_owned()
                                } else {
                                    entry.mood.as_deref().unwrap_or("阅读回响").to_owned()
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.muted)))
                                .child(display_time(&entry.created_at)),
                        ),
                );
            if entry.kind == "ticket" {
                bar = bar.child(self.button_with_icon(
                    "保存票根 PNG",
                    IconName::ImageDown,
                    Action::SaveJournalTicketPng(entry.id.clone()),
                    cx,
                ));
            }
            bar = bar
                .child(self.button_with_icon(
                    "生成书摘卡",
                    IconName::ImageDown,
                    Action::OpenQuoteCard(Box::new(card_draft)),
                    cx,
                ))
                .child(self.button_with_icon(
                    if entry.favorite {
                        "已收藏"
                    } else {
                        "收藏"
                    },
                    IconName::Heart,
                    Action::Favorite(entry.id.clone()),
                    cx,
                ))
                .child(self.button_with_icon(
                    "删除",
                    IconName::X,
                    Action::DeleteEntry(entry.id.clone()),
                    cx,
                ));
            if let Some(loc) = &entry.location
                && self.store.state.books.iter().any(|b| b.id == loc.book_id)
            {
                let loc = loc.clone();
                bar = bar.child(
                    Button::new(SharedString::from(format!("return-{}", entry.id)))
                        .text()
                        .icon(IconName::BookOpen)
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.accent)))
                        .label("回到书中")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(book) = this
                                .store
                                .state
                                .books
                                .iter_mut()
                                .find(|b| b.id == loc.book_id)
                            {
                                book.location = Some(loc.clone());
                            }
                            this.act(Action::Open(loc.book_id.clone()), window, cx);
                        })),
                );
            }
            let entry_body = if locked {
                div()
                    .p_5()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(0.68))
                    .border_1()
                    .border_color(self.border_subtle())
                    .text_size(px(12.5))
                    .font_family(self.ui_font())
                    .text_color(rgb(hex(&t.muted)))
                    .child("手账私密遮罩已开启 · 点击右上角「解锁手账」后阅读内容")
                    .into_any_element()
            } else {
                self.echo_card(&entry.echo, &display_input, entry.kind == "selection", cx)
            };
            content = content.child(
                v_flex()
                    .id(SharedString::from(entry.id.clone()))
                    .p_5()
                    .gap_4()
                    .rounded_2xl()
                    .border_1()
                    .border_color(self.border_subtle())
                    .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.90 } else { 0.94 }))
                    .shadow_sm()
                    .child(bar)
                    .child(entry_body),
            );
        }
        content.into_any_element()
    }
}

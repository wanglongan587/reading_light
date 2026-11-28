use super::*;

impl ReadingApp {
    pub(super) fn shelf(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let is_dark = self.is_dark();
        let (hud_hex, hud_gold, hud_wave_tint) = self.theme_hud_palette();
        let cover_colors = self.theme_cover_palette();
        let total_highlights: usize = self
            .store
            .state
            .books
            .iter()
            .map(|b| b.highlights.len())
            .sum();
        let total_journal = self.store.state.journal.len();
        let query = self.shelf_search.read(cx).value().trim().to_lowercase();
        let filtered_books: Vec<(usize, &glow_core::Book)> = self
            .store
            .state
            .books
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                let status = b.effective_status();
                let status_ok = match self.shelf_filter {
                    ShelfFilter::All => true,
                    ShelfFilter::Finished => status == "finished",
                    ShelfFilter::Reading => status == "reading",
                    ShelfFilter::Unread => status == "want",
                    ShelfFilter::Paused => status == "paused",
                };
                if !status_ok {
                    return false;
                }
                if query.is_empty() {
                    return true;
                }
                b.title.to_lowercase().contains(&query)
                    || b.author.to_lowercase().contains(&query)
                    || b.tags.iter().any(|tag| tag.to_lowercase().contains(&query))
            })
            .collect();
        let mut books = h_flex().flex_wrap().gap_5().items_start();
        for (i, b) in filtered_books.iter().copied() {
            let colors = cover_colors;
            let status = b.effective_status();
            let reading_label = if let Some(no) = b.completion_ticket_no {
                format!("已完读 · NO.{no:04}")
            } else if status == "paused" {
                "已搁置".to_owned()
            } else if let Some(progress) = b.progress {
                format!(
                    "{} · {:.0}%",
                    glow_core::Book::status_label(status),
                    progress * 100.0
                )
            } else {
                glow_core::Book::status_label(status).to_owned()
            };
            let format_badge = b.format.to_uppercase();
            let cover = if let Some(name) = &b.cover {
                img(self.store.root.join("books").join(&b.id).join(name))
                    .w_full()
                    .h(px(184.))
                    .object_fit(ObjectFit::Cover)
                    .into_any_element()
            } else {
                v_flex()
                    .w_full()
                    .h(px(184.))
                    .p_4()
                    .pl_5()
                    .justify_between()
                    .bg(rgb(colors[i % 4]))
                    .text_color(rgb(0xfffaed))
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(9.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(0xf5e2b8))
                                    .child("微光藏书 · EDITION"),
                            )
                            .child(div().w(px(48.))),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .py_2p5()
                            .border_t_1()
                            .border_b_1()
                            .border_color(rgb(0xffffff).opacity(0.16))
                            .child(
                                div()
                                    .text_size(px(19.5))
                                    .font_family(self.display_font())
                                    .line_height(relative(1.32))
                                    .child(b.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(0xe8dec8))
                                    .text_ellipsis()
                                    .child(b.author.clone()),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(div().w(px(36.)))
                            .child(
                                div()
                                    .text_size(px(8.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(0xf5e2b8).opacity(0.72))
                                    .child(format!("NO. {:02}", i + 1)),
                            ),
                    )
                    .into_any_element()
            };
            let progress_val = b
                .progress
                .map(|p| p.clamp(0.0, 1.0))
                .unwrap_or(if b.location.is_some() { 0.06 } else { 0.0 });
            let chapter_label = b
                .chapter
                .as_deref()
                .filter(|chapter| !chapter.trim().is_empty())
                .unwrap_or(if b.location.is_some() {
                    "继续上次停下的页面"
                } else {
                    "等待翻开第一页"
                });
            let snap = self.motion_snapshot();
            let open_book_id = b.id.clone();
            let card = v_flex()
                .relative()
                .overflow_hidden()
                .gap_3()
                .w(px(236.))
                .p_3p5()
                .rounded_2xl()
                .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.90 } else { 0.94 }))
                .border_1()
                .border_color(self.border_subtle())
                .shadow_sm()
                .hover(|style| {
                    style
                        .bg(rgb(hex(&t.paper)))
                        .border_color(rgb(hex(&t.accent)).opacity(0.56))
                        .shadow_md()
                })
                .child(div().absolute().top_0().left_0().right_0().child(
                    prism::prism_beam_horizontal(snap.clone(), 1.5, i as f32 * 0.85, false),
                ))
                .child(
                    v_flex()
                        .id(SharedString::from(format!("book-open-{i}")))
                        .debug_selector(move || format!("book-open-{i}"))
                        .cursor_pointer()
                        .gap_3()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.act(Action::Open(open_book_id.clone()), window, cx);
                            }),
                        )
                        .child(
                            div()
                                .relative()
                                .rounded_xl()
                                .overflow_hidden()
                                .border_1()
                                .border_color(rgb(0xffffff).opacity(0.22))
                                .hover(|style| {
                                    style.border_color(rgb(hex(&t.accent)).opacity(0.65))
                                })
                                .shadow_sm()
                                .child(cover)
                                // 3D Hardcover Spine Crease Overlay
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(0.))
                                        .top(px(0.))
                                        .w(px(9.))
                                        .h_full()
                                        .bg(rgb(0x000000).opacity(0.24)),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(9.))
                                        .top(px(0.))
                                        .w(px(1.5))
                                        .h_full()
                                        .bg(rgb(0xffffff).opacity(0.26)),
                                )
                                // Top-right reading status pill on cover
                                .child(
                                    div()
                                        .absolute()
                                        .right(px(8.))
                                        .top(px(8.))
                                        .px_2()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(rgb(hud_hex).opacity(0.78))
                                        .border_1()
                                        .border_color(rgb(0xffffff).opacity(0.18))
                                        .text_size(px(10.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(0xfff7df))
                                        .child(reading_label.clone()),
                                )
                                // Bottom-left format badge on cover
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(16.))
                                        .bottom(px(8.))
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded(px(5.))
                                        .bg(rgb(hud_hex).opacity(0.72))
                                        .text_size(px(9.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(0xe8efe9))
                                        .child(format_badge),
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(15.5))
                                        .font_family(self.display_font())
                                        .line_height(relative(1.3))
                                        .text_color(rgb(hex(&t.text)))
                                        .text_ellipsis()
                                        .child(b.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .text_ellipsis()
                                        .child(b.author.clone()),
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_1p5()
                                .px_2p5()
                                .py_2()
                                .rounded_xl()
                                .bg(rgb(hex(&t.background)).opacity(if is_dark {
                                    0.58
                                } else {
                                    0.68
                                }))
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_size(px(10.))
                                                .font_family(self.ui_font())
                                                .text_color(if b.location.is_some() {
                                                    rgb(hex(&t.accent))
                                                } else {
                                                    rgb(hex(&t.muted))
                                                })
                                                .text_ellipsis()
                                                .child(chapter_label.to_owned()),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .font_family(self.ui_font())
                                                .text_color(rgb(hex(&t.muted)))
                                                .child(format!("{:.0}%", progress_val * 100.0)),
                                        ),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(3.5))
                                        .rounded_full()
                                        .bg(rgb(hex(&t.text)).opacity(0.10))
                                        .child(
                                            div()
                                                .w(px((188.0 * progress_val).max(
                                                    if b.location.is_some() { 6.0 } else { 0.0 },
                                                )))
                                                .h_full()
                                                .rounded_full()
                                                .bg(rgb(hex(&t.accent))),
                                        ),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .justify_between()
                        .child(
                            self.button_with_icon(
                                if b.location.is_some() {
                                    "继续阅读"
                                } else {
                                    "打开这本书"
                                },
                                IconName::BookOpen,
                                Action::Open(b.id.clone()),
                                cx,
                            )
                            .primary(),
                        )
                        .child(self.button_with_icon(
                            "移出",
                            IconName::X,
                            Action::Remove(b.id.clone()),
                            cx,
                        )),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .justify_between()
                        .pt_1()
                        .border_t_1()
                        .border_color(self.border_subtle())
                        .child(self.button(
                            glow_core::Book::status_label(status),
                            Action::CycleBookStatus(b.id.clone()),
                            cx,
                        ))
                        .child(self.button("编辑", Action::EditBookMeta(b.id.clone()), cx))
                        .child(self.button("完读", Action::OpenCompletionModal(b.id.clone()), cx)),
                )
                .when(!b.tags.is_empty(), |card_el| {
                    let mut tag_row = h_flex().gap_1().flex_wrap();
                    for tag in b.tags.iter().take(3) {
                        tag_row = tag_row.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(rgb(hex(&t.accent)).opacity(0.11))
                                .text_size(px(9.5))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.accent)))
                                .child(format!("#{tag}")),
                        );
                    }
                    card_el.child(tag_row)
                });
            books = books.child(card);
        }
        books = books.child(
            v_flex()
                .id("shelf-add-book-card")
                .cursor_pointer()
                .w(px(236.))
                .h(px(344.))
                .p_5()
                .items_center()
                .justify_center()
                .gap_4()
                .rounded_2xl()
                .border_1()
                .border_color(self.border_strong())
                .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.56 } else { 0.68 }))
                .hover(|style| {
                    style
                        .bg(rgb(hex(&t.paper)).opacity(0.88))
                        .border_color(rgb(hex(&t.accent)).opacity(0.45))
                        .shadow_sm()
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.act(Action::Import, window, cx);
                    }),
                )
                .child(
                    div()
                        .size(px(68.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(hex(&t.accent)).opacity(0.10))
                        .border_1()
                        .border_color(rgb(hex(&t.accent)).opacity(0.26))
                        .child(
                            div()
                                .size(px(48.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgb(hex(&t.accent)).opacity(0.16))
                                .child(
                                    Icon::new(IconName::Feather)
                                        .size(px(22.))
                                        .text_color(rgb(hex(&t.accent))),
                                ),
                        ),
                )
                .child(
                    v_flex()
                        .items_center()
                        .gap_1p5()
                        .child(
                            div()
                                .text_size(px(17.))
                                .font_family(self.display_font())
                                .child("把下一本书带回家"),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.muted)))
                                .child("拖入 EPUB / TXT，或从电脑选择"),
                        ),
                )
                .child(self.waveform_bars(false, &[6., 11., 16., 9., 14., 8., 12., 7.]))
                .child(
                    self.button_with_icon("添加一本书", IconName::Plus, Action::Import, cx)
                        .primary(),
                ),
        );
        let (circ_kicker, circ_title, circ_sub, is_late_night) = circadian_greeting();
        let resume = self
            .store
            .state
            .last_book
            .as_ref()
            .and_then(|id| self.store.state.books.iter().find(|book| &book.id == id))
            .filter(|book| book.location.is_some());
        let (hero_kicker, hero_title, hero_sub) = if let Some(book) = resume {
            let chapter = book
                .chapter
                .as_deref()
                .filter(|chapter| !chapter.trim().is_empty())
                .unwrap_or("上次停下的那一页");
            let progress = book
                .progress
                .map(|progress| format!(" · 已读 {:.0}%", progress * 100.0))
                .unwrap_or_default();
            (
                circ_kicker,
                "回到故事里，\n那盏灯还亮着。".to_owned(),
                format!("《{}》 · {chapter}{progress}", book.title),
            )
        } else {
            (circ_kicker, circ_title.to_owned(), circ_sub.to_owned())
        };
        let rediscovery = self.daily_rediscovery();
        let snap = self.motion_snapshot();
        let mut hero_actions = h_flex()
            .gap_2p5()
            .items_center()
            .flex_wrap()
            .child(
                self.button_with_icon(
                    if resume.is_some() {
                        "继续上次阅读"
                    } else {
                        "导入一本书"
                    },
                    if resume.is_some() {
                        IconName::BookOpen
                    } else {
                        IconName::Plus
                    },
                    if let Some(book) = resume {
                        Action::Open(book.id.clone())
                    } else {
                        Action::Import
                    },
                    cx,
                )
                .primary(),
            )
            .child(self.button_with_icon(
                "此刻的心情",
                IconName::MessageCircleHeart,
                Action::Page(Page::Mood),
                cx,
            ));
        if is_late_night && !is_dark {
            hero_actions = hero_actions
                .child(self.button_with_icon(
                    "雨夜灯笼巷",
                    IconName::Flame,
                    Action::PickTheme(8),
                    cx,
                ))
                .child(self.button_with_icon("静夜台灯", IconName::Moon, Action::PickTheme(2), cx))
                .child(self.button_with_icon(
                    "星空棱镜",
                    IconName::Sparkles,
                    Action::PickTheme(3),
                    cx,
                ));
        }
        let hud_bg = rgb(hud_hex).opacity(0.72);
        let rediscovery_card =
            if let Some((quote_text, source_label, target, pool_len)) = rediscovery {
                let short_quote: String = if quote_text.chars().count() > 42 {
                    format!(
                        "{}…",
                        quote_text.chars().take(42).collect::<String>().trim_end()
                    )
                } else {
                    quote_text
                };
                let mut card = v_flex()
                    .max_w(px(330.))
                    .px_4()
                    .py_2p5()
                    .gap_1()
                    .items_end()
                    .rounded_xl()
                    .bg(hud_bg)
                    .border_1()
                    .border_color(rgb(0xffffff).opacity(0.22))
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .font_family(self.ui_font())
                                    .text_color(hud_gold)
                                    .child(format!("今日重逢 · {source_label}")),
                            )
                            .when(pool_len > 1, |this| {
                                this.child(
                                    div()
                                        .id("next-rediscovery")
                                        .cursor_pointer()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(rgb(0xffffff).opacity(0.14))
                                        .text_size(px(9.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(0xffffff))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, window, cx| {
                                                this.act(Action::NextRediscovery, window, cx);
                                            }),
                                        )
                                        .child("换一句"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(14.5))
                            .font_family(self.display_font())
                            .line_height(relative(1.45))
                            .text_color(rgb(0xfffdf7))
                            .child(format!("“{short_quote}”")),
                    );
                if let Some((book_id, cfi)) = target {
                    card = card.child(
                        div()
                            .id("open-rediscovery")
                            .cursor_pointer()
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(hud_gold)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.act(
                                        Action::OpenAt(Box::new((book_id.clone(), cfi.clone()))),
                                        window,
                                        cx,
                                    );
                                }),
                            )
                            .child("点击回到书中这一段 →"),
                    );
                }
                card
            } else {
                v_flex()
                    .px_4()
                    .py_2p5()
                    .gap_0p5()
                    .items_end()
                    .rounded_xl()
                    .bg(hud_bg)
                    .border_1()
                    .border_color(rgb(0xffffff).opacity(0.20))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(hud_gold)
                            .child(self.theme_shelf_cue()),
                    )
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_family(self.display_font())
                            .text_color(rgb(0xfffdf7))
                            .child("也足够让风停一会儿"),
                    )
            };
        let hero = h_flex()
            .relative()
            .w_full()
            .h(px(264.))
            .flex_shrink_0()
            .rounded_2xl()
            .overflow_hidden()
            .border_1()
            .border_color(self.border_strong())
            .bg(rgb(hex(&t.background)))
            .shadow_md()
            .child(
                img(self.scene_asset())
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .child(
                v_flex()
                    .relative()
                    .w(px(436.))
                    .max_w_full()
                    .h_full()
                    .p_6()
                    .justify_between()
                    .bg(rgb(hex(&t.paper)).opacity(0.88))
                    .border_r_1()
                    .border_color(self.border_subtle())
                    .child(self.heading(hero_kicker, &hero_title, &hero_sub))
                    .child(hero_actions),
            )
            // Right-side Glow Study Floating Telemetry & Daily Rediscovery HUD
            .child(
                v_flex()
                    .absolute()
                    .right_5()
                    .top_4()
                    .bottom_4()
                    .justify_between()
                    .items_end()
                    .child(
                        h_flex()
                            .gap_2p5()
                            .items_center()
                            .px_3p5()
                            .py_2()
                            .rounded_xl()
                            .bg(hud_bg)
                            .border_1()
                            .border_color(rgb(0xffffff).opacity(0.20))
                            .child(self.waveform_bars_tinted(
                                true,
                                &[5., 11., 15., 8., 13., 7.],
                                hud_wave_tint,
                            ))
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_size(px(9.))
                                            .font_family(self.ui_font())
                                            .text_color(hud_gold)
                                            .child(format!(
                                                "{}  ·  {}",
                                                Self::frequency_for_theme(&t.id),
                                                self.active_soundscape_preset().label()
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(0xf4f7f2))
                                            .child(format!(
                                                "藏书 {} 本  ·  划线 {} 处  ·  回响 {} 篇",
                                                self.store.state.books.len(),
                                                total_highlights,
                                                total_journal
                                            )),
                                    ),
                            ),
                    )
                    .child(rediscovery_card),
            )
            // Signature Obsidian Prism mouse-tracking top & bottom edge beams
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(prism::prism_beam_horizontal(snap.clone(), 2.0, 0.0, false)),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .child(prism::prism_beam_horizontal(snap, 2.0, 1.5, false)),
            );
        let hero_stage = div()
            .relative()
            .w_full()
            .flex_shrink_0()
            .pt(px(22.))
            .child(hero)
            .child(
                div()
                    .absolute()
                    .top(px(-2.))
                    .right(px(24.))
                    .child(self.pet_dock("shelf-pet", cx)),
            );
        let mut filter_pills = h_flex().gap_1p5().items_center().flex_wrap();
        for (label, filter) in [
            ("全部", ShelfFilter::All),
            ("在读", ShelfFilter::Reading),
            ("想读", ShelfFilter::Unread),
            ("已完读", ShelfFilter::Finished),
            ("搁置", ShelfFilter::Paused),
        ] {
            let active = self.shelf_filter == filter;
            filter_pills = filter_pills.child(self.pill_button(
                label,
                None,
                active,
                Action::ShelfFilter(filter),
                cx,
            ));
        }
        let mut study_tabs = h_flex().gap_1p5().items_center().flex_wrap();
        for (label, tab, icon) in [
            ("我的书架", ShelfTab::Library, IconName::LibraryBig),
            ("微光公版馆", ShelfTab::Catalog, IconName::BookOpen),
            ("OPDS 书源", ShelfTab::Opds, IconName::Download),
        ] {
            study_tabs = study_tabs.child(self.pill_button(
                label,
                Some(icon),
                self.shelf_tab == tab,
                Action::ShelfTab(tab),
                cx,
            ));
        }
        let today_mins = if self.store.state.reading_minutes_date.as_deref() == Some(&today_ymd()) {
            self.store.state.reading_minutes_today
        } else {
            0
        };
        // Mood Book Picker strip ("此刻想读什么？")
        let mut mood_chips = h_flex().gap_1p5().items_center().flex_wrap();
        for mood in ["睡前", "疲惫", "独处", "迷茫", "希望"] {
            mood_chips = mood_chips.child(self.pill_button(
                mood,
                None,
                self.shelf_mood_pick == mood,
                Action::ShelfMoodPick(mood.into()),
                cx,
            ));
        }
        let mut mood_picks = h_flex().gap_3().flex_wrap().items_stretch();
        for entry in catalog::recommend_for_mood(&self.shelf_mood_pick)
            .into_iter()
            .take(3)
        {
            mood_picks = mood_picks.child(
                v_flex()
                    .w(px(274.))
                    .p_3p5()
                    .gap_1p5()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(if is_dark { 0.62 } else { 0.72 }))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_family(self.display_font())
                                    .child(format!("《{}》", entry.title)),
                            )
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(format!("约 {} 分钟", entry.minutes)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .line_height(relative(1.5))
                            .text_color(rgb(hex(&t.muted)))
                            .child(entry.tagline),
                    )
                    .child(
                        h_flex()
                            .pt_1()
                            .gap_2()
                            .items_center()
                            .child(
                                self.button_with_icon(
                                    "翻开阅读",
                                    IconName::BookOpen,
                                    Action::InstallCatalogBook(entry.id.into(), true),
                                    cx,
                                )
                                .primary(),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(entry.author),
                            ),
                    ),
            );
        }
        let mood_picker_card = v_flex()
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
                            .child(
                                Icon::new(IconName::Sparkles)
                                    .size(px(14.))
                                    .text_color(rgb(hex(&t.accent))),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_family(self.display_font())
                                    .child("按此刻心绪，挑一段公版微光"),
                            ),
                    )
                    .child(mood_chips),
            )
            .child(mood_picks);

        let mut root = v_flex()
            .id("shelf-scroll")
            .overflow_y_scroll()
            .size_full()
            .px_7()
            .pt_3()
            .pb_6()
            .gap_5()
            .child(hero_stage)
            .child(
                h_flex()
                    .justify_between()
                    .items_end()
                    .flex_wrap()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .gap_2p5()
                                    .items_center()
                                    .flex_wrap()
                                    .child(study_tabs)
                                    .when(today_mins > 0, |this| {
                                        this.child(
                                            div()
                                                .px_2p5()
                                                .py_0p5()
                                                .rounded_full()
                                                .bg(rgb(hex(&t.paper)).opacity(0.84))
                                                .border_1()
                                                .border_color(self.border_subtle())
                                                .text_size(px(10.5))
                                                .font_family(self.ui_font())
                                                .text_color(rgb(hex(&t.muted)))
                                                .child(format!("今日静读 {today_mins} 分钟")),
                                        )
                                    }),
                            )
                            .when(self.shelf_tab == ShelfTab::Library, |this| {
                                this.child(
                                    h_flex()
                                        .gap_2p5()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_size(px(20.))
                                                .font_family(self.display_font())
                                                .child("我的藏书"),
                                        )
                                        .child(
                                            div()
                                                .px_2p5()
                                                .py_0p5()
                                                .rounded_full()
                                                .bg(rgb(hex(&t.accent)).opacity(0.14))
                                                .border_1()
                                                .border_color(rgb(hex(&t.accent)).opacity(0.28))
                                                .text_size(px(11.))
                                                .font_family(self.ui_font())
                                                .text_color(rgb(hex(&t.accent)))
                                                .child(format!(
                                                    "{} / {} 本",
                                                    filtered_books.len(),
                                                    self.store.state.books.len()
                                                )),
                                        ),
                                )
                            }),
                    )
                    .when(self.shelf_tab == ShelfTab::Library, |this| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .flex_wrap()
                                .child(filter_pills)
                                .child(
                                    div()
                                        .w(px(220.))
                                        .child(Input::new(&self.shelf_search).w_full()),
                                ),
                        )
                    }),
            );

        // Optional inline metadata editor card
        if self.shelf_tab == ShelfTab::Library && self.editing_book_id.is_some() {
            root = root.child(
                v_flex()
                    .p_4()
                    .gap_3()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(0.96))
                    .border_1()
                    .border_color(rgb(hex(&t.accent)).opacity(0.45))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_family(self.display_font())
                            .child("编辑书目信息与书单标签"),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .flex_wrap()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(200.))
                                    .child(Input::new(&self.book_meta_title_input).w_full()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(160.))
                                    .child(Input::new(&self.book_meta_author_input).w_full()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(220.))
                                    .child(Input::new(&self.book_meta_tags_input).w_full()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(self.button("保存修改", Action::SaveBookMeta, cx).primary())
                            .child(self.button("取消", Action::CancelBookMeta, cx)),
                    ),
            );
        }

        match self.shelf_tab {
            ShelfTab::Library => {
                root = root.child(mood_picker_card).child(books);
            }
            ShelfTab::Catalog => {
                root = root.child(self.shelf_catalog_view(cx));
            }
            ShelfTab::Opds => {
                root = root.child(self.shelf_opds_view(cx));
            }
        }
        root.into_any_element()
    }

    fn shelf_catalog_view(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let is_dark = self.is_dark();
        let mut cat_bar = h_flex().gap_1p5().flex_wrap().items_center();
        for cat in [
            "全部",
            "睡前故事",
            "原创散文",
            "古典诗文",
            "双语原典",
            "10分钟内",
        ] {
            cat_bar = cat_bar.child(self.pill_button(
                cat,
                None,
                self.catalog_category == cat,
                Action::CatalogCategory(cat.into()),
                cx,
            ));
        }

        let entries = catalog::filter_by_category(&self.catalog_category);
        let total_catalog = catalog::all_books().len();
        let mut grid = h_flex().flex_wrap().gap_4().items_stretch();
        for entry in &entries {
            let accent_col = hex(entry.accent_hex);
            let existing_book_id = self
                .store
                .state
                .books
                .iter()
                .find(|b| b.title == entry.title)
                .map(|b| b.id.clone());
            let mut mood_row = h_flex().gap_1().flex_wrap();
            for m in entry.moods {
                mood_row = mood_row.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_full()
                        .bg(rgb(accent_col).opacity(0.14))
                        .text_size(px(9.5))
                        .font_family(self.ui_font())
                        .text_color(rgb(accent_col))
                        .child(*m),
                );
            }
            grid = grid.child(
                v_flex()
                    .w(px(292.))
                    .p_5()
                    .gap_3()
                    .justify_between()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.90 } else { 0.95 }))
                    .border_1()
                    .border_color(if existing_book_id.is_some() {
                        Hsla::from(rgb(hex(&t.accent)).opacity(0.42))
                    } else {
                        self.border_subtle()
                    })
                    .border_t_4()
                    .shadow_sm()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(accent_col))
                                            .child(format!("{} · {}", entry.category, entry.era)),
                                    )
                                    .child(if existing_book_id.is_some() {
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(rgb(hex(&t.accent)).opacity(0.14))
                                            .text_size(px(9.5))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.accent)))
                                            .child("已在书架 ✓")
                                    } else {
                                        div()
                                            .text_size(px(10.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.muted)))
                                            .child(format!("约 {} 分钟", entry.minutes))
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_family(self.display_font())
                                    .child(format!("《{}》", entry.title)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(format!("{} · 约 {} 分钟", entry.author, entry.minutes)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_family(self.ui_font())
                                    .line_height(relative(1.6))
                                    .text_color(rgb(hex(&t.text)).opacity(0.86))
                                    .child(entry.tagline),
                            )
                            .child(mood_row),
                    )
                    .child(if let Some(book_id) = existing_book_id {
                        h_flex().gap_2().pt_2().items_center().child(
                            self.button_with_icon(
                                "继续阅读",
                                IconName::BookOpen,
                                Action::Open(book_id),
                                cx,
                            )
                            .primary()
                            .flex_1(),
                        )
                    } else {
                        h_flex()
                            .gap_2()
                            .pt_2()
                            .child(
                                self.button_with_icon(
                                    "收入并翻开",
                                    IconName::BookOpen,
                                    Action::InstallCatalogBook(entry.id.into(), true),
                                    cx,
                                )
                                .primary(),
                            )
                            .child(self.button_with_icon(
                                "仅收入书架",
                                IconName::Plus,
                                Action::InstallCatalogBook(entry.id.into(), false),
                                cx,
                            ))
                    }),
            );
        }
        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .flex_wrap()
                    .gap_3()
                    .child(cat_bar)
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child(format!(
                                "显示 {} / {} 部内置典藏 · 完全离线精排，支持划线、听书与纸边注脚",
                                entries.len(),
                                total_catalog
                            )),
                    ),
            )
            .child(grid)
            .into_any_element()
    }

    fn shelf_opds_view(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let is_dark = self.is_dark();
        let mut feeds_bar = h_flex().gap_2().flex_wrap().items_center();
        for preset in opds::preset_feeds() {
            feeds_bar = feeds_bar.child(self.button_with_icon(
                &preset.name,
                IconName::Download,
                Action::OpdsFetchFeed(preset.url),
                cx,
            ));
        }
        for saved in &self.store.state.opds_feeds {
            feeds_bar = feeds_bar.child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(0.65))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(self.button(&saved.name, Action::OpdsFetchFeed(saved.url.clone()), cx))
                    .child(self.button_with_icon(
                        "移除",
                        IconName::X,
                        Action::OpdsRemoveFeed(saved.id.clone()),
                        cx,
                    )),
            );
        }

        let mut view = v_flex()
            .gap_4()
            .child(
                v_flex()
                    .p_5()
                    .gap_3()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.88 } else { 0.94 }))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_family(self.display_font())
                            .child("连接 OPDS 1.2 开放书源（Calibre / Komga / Kavita / 公版库）"),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child("输入你的私人书库或开放公版 OPDS 地址，即可直接浏览目录并一键下载 EPUB 至本地书架。"),
                    )
                    .child(feeds_bar)
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .items_center()
                            .child(
                                div()
                                    .w(px(200.))
                                    .child(Input::new(&self.opds_name_input).w_full()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(260.))
                                    .child(Input::new(&self.opds_url_input).w_full()),
                            )
                            .child(
                                self.button_with_icon(
                                    "添加书源",
                                    IconName::Plus,
                                    Action::OpdsAddFeed,
                                    cx,
                                )
                                .primary(),
                            ),
                    ),
            );

        if !self.opds_entries.is_empty() {
            let mut list = v_flex().gap_2p5();
            for (idx, entry) in self.opds_entries.iter().enumerate() {
                let is_book = entry.epub_url.is_some();
                list = list.child(
                    h_flex()
                        .p_4()
                        .gap_3()
                        .justify_between()
                        .items_center()
                        .rounded_xl()
                        .bg(rgb(hex(&t.paper)).opacity(0.92))
                        .border_1()
                        .border_color(self.border_subtle())
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(14.5))
                                        .font_family(self.display_font())
                                        .child(entry.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child(if entry.summary.is_empty() {
                                            entry.author.clone()
                                        } else {
                                            format!("{} · {}", entry.author, entry.summary)
                                        }),
                                ),
                        )
                        .child(
                            self.button_with_icon(
                                if is_book {
                                    "下载收入书架"
                                } else {
                                    "进入分类"
                                },
                                if is_book {
                                    IconName::Download
                                } else {
                                    IconName::ChevronRight
                                },
                                Action::OpdsImportEntry(idx),
                                cx,
                            )
                            .primary()
                            .disabled(
                                self.opds_fetch_rx.is_some() || self.opds_import_rx.is_some(),
                            ),
                        ),
                );
            }
            view = view.child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_family(self.display_font())
                            .child(format!(
                                "当前目录：{}（{} 条）",
                                self.opds_active_title,
                                self.opds_entries.len()
                            )),
                    )
                    .child(list),
            );
        }
        view.into_any_element()
    }
}

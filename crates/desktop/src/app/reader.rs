use super::*;

impl ReadingApp {
    pub(super) fn reader_stage_backdrop(&self) -> Div {
        let t = self.theme();
        let is_dark = self.is_dark();
        let book = self
            .store
            .state
            .books
            .iter()
            .find(|b| Some(&b.id) == self.active_book.as_ref());
        let title = book.map(|b| b.title.as_str()).unwrap_or("在林间，慢慢来");
        let author = book
            .map(|b| b.author.as_str())
            .unwrap_or("微光阅读 · 原创示例");
        let chapter = book
            .and_then(|b| b.chapter.as_deref())
            .filter(|c| !c.trim().is_empty())
            .unwrap_or("正文 · 慢读频道");
        let font_size = self
            .store
            .state
            .reader_font_size
            .unwrap_or(t.font_size)
            .clamp(15.0, 26.0);
        let leading = self
            .store
            .state
            .reader_line_height
            .unwrap_or(t.line_height)
            .clamp(1.4, 2.3);
        let width = self
            .store
            .state
            .reader_width
            .unwrap_or(t.width)
            .clamp(480, 920) as f32;
        let excerpt = if !self.echo_input.trim().is_empty() {
            self.echo_input.trim().to_owned()
        } else if let Some(first_hl) = book.and_then(|b| b.highlights.last()) {
            first_hl.text.clone()
        } else {
            "傍晚的风从树梢滑下来，把白天的喧嚣留在林子外面。屋里那盏暖黄的小灯亮起来的时候，书页上的字句也仿佛有了温和的呼吸。"
                .to_owned()
        };
        v_flex()
            .absolute()
            .size_full()
            .items_center()
            .justify_center()
            .px_6()
            .py_4()
            .bg(rgb(hex(&t.paper)))
            .child(
                v_flex()
                    .w(px(width))
                    .max_w_full()
                    .h_full()
                    .px_8()
                    .py_6()
                    .justify_between()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.background)).opacity(if is_dark { 0.24 } else { 0.32 }))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .pb_3()
                            .border_b_1()
                            .border_color(self.border_subtle())
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Icon::new(IconName::BookOpen)
                                            .size(px(13.))
                                            .text_color(rgb(hex(&t.accent))),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.muted)))
                                            .child(format!("{title}  ·  {chapter}")),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(format!(
                                        "{:.0} pt  ·  行距 {:.1}  ·  {} px",
                                        font_size, leading, width as u32
                                    )),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_4()
                            .py_2()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(23.))
                                            .font_family(self.display_font())
                                            .text_color(rgb(hex(&t.text)))
                                            .child(chapter.to_owned()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.muted)))
                                            .child(author.to_owned()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(font_size))
                                    .font_family(self.display_font())
                                    .line_height(relative(leading))
                                    .text_color(rgb(hex(&t.text)).opacity(0.86))
                                    .child("许多年后，当目光再次落向这一页纸，那些曾在安静夜晚里被仔细读过的句子，依然保留着最初翻开时的温度与微光。"),
                            )
                            .child(
                                v_flex()
                                    .p_4()
                                    .gap_2()
                                    .rounded_xl()
                                    .bg(rgb(hex(&t.accent)).opacity(if is_dark { 0.16 } else { 0.10 }))
                                    .border_l_3()
                                    .border_color(rgb(hex(&t.accent)))
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                Icon::new(IconName::Sparkles)
                                                    .size(px(12.))
                                                    .text_color(rgb(hex(&t.accent))),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(10.))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.accent)))
                                                    .child("当前书页焦点段落 · SELECTED PASSAGE"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(font_size))
                                            .font_family(self.display_font())
                                            .line_height(relative(leading))
                                            .text_color(rgb(hex(&t.text)))
                                            .child(excerpt.chars().take(190).collect::<String>()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(font_size))
                                    .font_family(self.display_font())
                                    .line_height(relative(leading))
                                    .text_color(rgb(hex(&t.text)).opacity(0.82))
                                    .child("窗外的风声很轻，灯下的纸页不催促任何人。在这里停一会儿，或者顺着目录与搜索去往下一段故事。"),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .pt_3()
                            .border_t_1()
                            .border_color(self.border_subtle())
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(if self.opened {
                                        "GLOW READING STAGE · 书页已就绪"
                                    } else {
                                        "GLOW READING STAGE · 正在铺开书页…"
                                    }),
                            )
                            .child(self.waveform_bars(true, &[4., 8., 12., 7., 10., 5.])),
                    ),
            )
    }
    pub(super) fn reader(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let is_dark = self.is_dark();
        let book = self
            .store
            .state
            .books
            .iter()
            .find(|b| Some(&b.id) == self.active_book.as_ref());
        let title = book.map(|b| b.title.as_str()).unwrap_or("还没有打开书籍");
        let author = book
            .map(|b| b.author.as_str())
            .unwrap_or("从书架选择一本书");
        let chapter = book
            .and_then(|b| b.chapter.as_deref())
            .filter(|c| !c.trim().is_empty());
        let progress = book
            .and_then(|book| book.progress)
            .map(|progress| progress.clamp(0.0, 1.0));
        let controls = h_flex()
            .gap_1()
            .p_1()
            .rounded_xl()
            .bg(rgb(hex(&t.background)).opacity(if is_dark { 0.68 } else { 0.74 }))
            .border_1()
            .border_color(self.border_subtle())
            .child(
                self.button_with_icon(
                    self.tr("AI 共读", "Companion"),
                    IconName::WandSparkles,
                    Action::ToggleCompanion,
                    cx,
                )
                .toggled(self.drawer == "companion"),
            )
            .child(
                self.button_with_icon(
                    if self.tts_status.active && !self.tts_status.paused {
                        self.tr("听书中", "Listening")
                    } else {
                        self.tr("听书", "Listen")
                    },
                    IconName::AudioLines,
                    Action::ToggleTtsDrawer,
                    cx,
                )
                .toggled(self.drawer == "tts" || self.tts_status.active),
            )
            .child(
                self.button_with_icon(
                    if self.radio_status.playing && !self.radio_status.paused {
                        self.tr("电台中", "On Air")
                    } else {
                        self.tr("电台", "Radio")
                    },
                    IconName::Moon,
                    Action::Page(Page::Radio),
                    cx,
                )
                .toggled(self.radio_status.playing && !self.radio_status.paused),
            )
            .child(
                self.button_with_icon(self.tr("目录", "TOC"), IconName::ListTree, Action::Toc, cx)
                    .toggled(self.drawer == "toc"),
            )
            .child(
                self.button_with_icon(
                    self.tr("搜索", "Search"),
                    IconName::Search,
                    Action::Search,
                    cx,
                )
                .toggled(self.drawer == "search"),
            )
            .child(
                self.button_with_icon(
                    self.tr("划线", "Highlights"),
                    IconName::Feather,
                    Action::Highlights,
                    cx,
                )
                .toggled(self.drawer == "highlights"),
            )
            .child(
                self.button_with_icon(
                    self.tr("书签", "Marks"),
                    IconName::Bookmark,
                    Action::Bookmarks,
                    cx,
                )
                .toggled(self.drawer == "bookmarks"),
            )
            .child(self.button_with_icon(
                if self.store.state.mode == "scrolled" {
                    self.tr("滚动", "Scroll")
                } else {
                    self.tr("翻页", "Paged")
                },
                if self.store.state.mode == "scrolled" {
                    IconName::ScrollText
                } else {
                    IconName::Columns2
                },
                Action::Mode,
                cx,
            ))
            .child(
                self.button_with_icon(
                    if self.focus_mode {
                        self.tr("退出专注", "Exit Zen")
                    } else {
                        self.tr("专注", "Focus")
                    },
                    if self.focus_mode {
                        IconName::Minimize2
                    } else {
                        IconName::Focus
                    },
                    Action::Focus,
                    cx,
                )
                .toggled(self.focus_mode),
            );
        let snap = self.motion_snapshot();
        let toolbar = h_flex()
            .relative()
            .h(px(62.))
            .px_5()
            .gap_3()
            .items_center()
            .bg(rgb(hex(&t.paper)).opacity(0.96))
            .border_b_1()
            .border_color(self.border_subtle())
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .child(prism::prism_beam_horizontal(snap, 1.5, 0.4, false)),
            )
            .child(self.button_with_icon(
                self.tr("书架", "Study"),
                IconName::ChevronLeft,
                Action::Page(Page::Shelf),
                cx,
            ))
            .child(
                div()
                    .w(px(1.))
                    .h(px(24.))
                    .bg(rgb(hex(&t.text)).opacity(0.12)),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(15.5))
                                    .font_family(self.display_font())
                                    .text_ellipsis()
                                    .child(title.to_owned()),
                            )
                            .when_some(chapter, |this, ch| {
                                this.child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(rgb(hex(&t.accent)).opacity(0.12))
                                        .text_size(px(10.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.accent)))
                                        .child(ch.to_owned()),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .text_ellipsis()
                            .child(author.to_owned()),
                    ),
            )
            .child(div().flex_1())
            .child(controls);
        let narrow = self.is_narrow_reader(window);
        let mut body = h_flex().flex_1().min_h_0().w_full();
        if (self.drawer.is_empty() || !narrow)
            && let Some(view) = &self.webview
        {
            body = body.child(
                div()
                    .relative()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .child(self.reader_stage_backdrop())
                    .child(view.clone()),
            );
        }
        if !self.drawer.is_empty() {
            let panel = div().h_full();
            let panel = if narrow {
                panel
                    .flex_1()
                    .min_w_0()
                    .bg(rgb(hex(&t.paper)).opacity(0.98))
            } else {
                panel
                    .w(px(364.))
                    .flex_shrink_0()
                    .bg(rgb(hex(&t.paper)).opacity(0.98))
                    .border_l_1()
                    .border_color(self.border_subtle())
            };
            body = body.child(panel.child(self.reader_drawer(cx)));
        }
        let mut bottom = h_flex()
            .h(px(54.))
            .px_5()
            .gap_2p5()
            .items_center()
            .bg(rgb(hex(&t.paper)).opacity(0.96))
            .border_t_1()
            .border_color(self.border_subtle())
            .child(
                h_flex()
                    .gap_1()
                    .p_0p5()
                    .rounded_lg()
                    .bg(rgb(hex(&t.background)).opacity(0.62))
                    .child(self.button_with_icon(
                        self.tr("上一页", "Prev"),
                        IconName::ChevronLeft,
                        Action::Prev,
                        cx,
                    ))
                    .child(self.button_with_icon(
                        self.tr("下一页", "Next"),
                        IconName::ChevronRight,
                        Action::Next,
                        cx,
                    )),
            )
            .child(div().flex_1());
        if let Some(progress) = progress {
            bottom = bottom.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .bg(rgb(hex(&t.background)).opacity(0.66))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        div()
                            .w(px(96.))
                            .h(px(3.5))
                            .rounded_full()
                            .bg(rgb(hex(&t.text)).opacity(0.12))
                            .child(
                                div()
                                    .w(px((96.0 * progress).max(3.0)))
                                    .h_full()
                                    .rounded_full()
                                    .bg(rgb(hex(&t.accent))),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.accent)))
                            .child(format!("{:.0}%", progress * 100.0)),
                    ),
            );
        }
        let hint =
            if let Some(message) = self.error.as_ref().or(self.notice.as_ref()) {
                let color = if self.error.is_some() {
                    0xb34f36
                } else {
                    hex(&t.accent)
                };
                h_flex()
                    .gap_2()
                    .items_center()
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .bg(rgb(color).opacity(0.10))
                    .text_color(rgb(color))
                    .child(
                        Icon::new(if self.error.is_some() {
                            IconName::CircleAlert
                        } else {
                            IconName::Sparkles
                        })
                        .size(px(13.)),
                    )
                    .child(
                        div()
                            .max_w(px(460.))
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_ellipsis()
                            .child(message.clone()),
                    )
                    .into_any_element()
            } else {
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .px_2p5()
                    .py_1()
                    .rounded_full()
                    .bg(rgb(hex(&t.accent)).opacity(0.08))
                    .text_color(rgb(hex(&t.muted)))
                    .child(
                        Icon::new(IconName::Sparkles)
                            .size(px(13.))
                            .text_color(rgb(hex(&t.accent))),
                    )
                    .child(div().text_size(px(11.)).font_family(self.ui_font()).child(
                        self.tr("选中文字，遇见回响", "Select text for an echo or footnote"),
                    ))
                    .into_any_element()
            };
        let timer_label = if let Some((_, rem)) = self.focus_timer_progress() {
            if self.locale().is_english() {
                format!("Focus {:02}:{:02}", rem / 60, rem % 60)
            } else {
                format!("静读 {:02}:{:02}", rem / 60, rem % 60)
            }
        } else {
            self.tr("静读计时", "Focus Timer").to_owned()
        };
        bottom = bottom
            .child(hint)
            .child(self.pet_dock("reader-pet", cx))
            .child(
                self.button_with_icon(&timer_label, IconName::Timer, Action::ToggleTimerPicker, cx)
                    .toggled(self.focus_timer_started.is_some() || self.focus_timer_picker_open),
            )
            .child(
                self.button_with_icon(
                    self.tr("排版", "Type"),
                    IconName::AArrowDown,
                    Action::Typography,
                    cx,
                )
                .toggled(self.drawer == "type"),
            )
            .child(self.button_with_icon(
                self.tr("留下书签", "Bookmark"),
                IconName::Bookmark,
                Action::Bookmark,
                cx,
            ));
        if let Some(active_id) = self.active_book.clone() {
            bottom = bottom.child(self.button_with_icon(
                self.tr("完读仪式", "Finish Book"),
                IconName::Sparkles,
                Action::OpenCompletionModal(active_id),
                cx,
            ));
        }
        if book.is_some_and(|b| b.format == "txt") {
            bottom = bottom
                .child(
                    div()
                        .w(px(1.))
                        .h(px(20.))
                        .bg(rgb(hex(&t.text)).opacity(0.12)),
                )
                .child(self.button("UTF-8", Action::Encoding("UTF-8".into()), cx))
                .child(self.button("GB18030", Action::Encoding("GB18030".into()), cx));
        }
        let show_top = !self.focus_mode || self.zen_reveal_top || !self.drawer.is_empty();
        let show_bottom = !self.focus_mode || self.zen_reveal_bottom || !self.drawer.is_empty();
        let mut stage = v_flex().relative().size_full().bg(rgb(hex(&t.paper)));
        if show_top {
            stage = stage.child(toolbar);
        } else {
            // Whisper-quiet Zen top strip: keeps screen pure while offering an effortless hover/click exit
            stage = stage.child(
                h_flex()
                    .h(px(30.))
                    .px_4()
                    .justify_between()
                    .items_center()
                    .bg(rgb(hex(&t.paper)).opacity(0.86))
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)).opacity(0.68))
                            .child(format!(
                                "{title}  ·  禅意专注中（移至边缘显露工具栏 · Esc 退出）"
                            )),
                    )
                    .child(self.button_with_icon(
                        "退出专注",
                        IconName::Minimize2,
                        Action::Focus,
                        cx,
                    )),
            );
        }
        // Spoiler-free reunion welcome strip when returning to a book already in progress
        if show_top
            && !self.reunion_dismissed
            && let Some(active_book) = book
            && active_book.progress.unwrap_or(0.0) >= 0.02
            && active_book.progress.unwrap_or(0.0) < 0.99
        {
            let pct = (active_book.progress.unwrap_or(0.0) * 100.0).round() as u32;
            let reunion_hint = active_book
                .companion
                .as_ref()
                .map(|c| c.resume_moment.clone())
                .filter(|s| !s.trim().is_empty())
                .or_else(|| {
                    active_book.highlights.last().map(|h| {
                        format!(
                            "上次留下的句子：「{}」",
                            h.text.chars().take(38).collect::<String>()
                        )
                    })
                })
                .unwrap_or_else(|| {
                    format!(
                        "上次读至 {}（已读 {pct}%），点击右侧可 3 秒唤醒防剧透前情。",
                        chapter.unwrap_or("上次停下的页面")
                    )
                });
            stage = stage.child(
                h_flex()
                    .id("reunion-banner")
                    .px_5()
                    .py_2()
                    .gap_3()
                    .items_center()
                    .justify_between()
                    .bg(rgb(hex(&t.accent)).opacity(if is_dark { 0.14 } else { 0.09 }))
                    .border_b_1()
                    .border_color(rgb(hex(&t.accent)).opacity(0.24))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgb(hex(&t.accent)).opacity(0.18))
                                    .text_size(px(9.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child("久别重逢 · 防剧透"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(11.5))
                                    .font_family(self.ui_font())
                                    .text_ellipsis()
                                    .text_color(rgb(hex(&t.text)).opacity(0.88))
                                    .child(reunion_hint),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(self.button_with_icon(
                                "前情唤醒",
                                IconName::WandSparkles,
                                Action::ToggleCompanion,
                                cx,
                            ))
                            .child(self.button_with_icon(
                                "收起提示",
                                IconName::X,
                                Action::DismissReunion,
                                cx,
                            )),
                    ),
            );
        }
        stage = stage.child(body);
        if show_bottom {
            stage = stage.child(bottom);
        }
        stage.into_any_element()
    }
    #[inline(never)]
    pub(super) fn reader_drawer(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let title = match self.drawer.as_str() {
            "companion" => self.tr("防剧透 AI 共读", "Spoiler-Free AI Companion"),
            "tts" => self.tr("微光双轨听书", "Dual-Track Audiobook"),
            "footnote" => self.tr("纸边注脚", "Paper-Margin Footnote"),
            "toc" => self.tr("沿着目录走", "Table of Contents"),
            "search" => self.tr("在书里找一句话", "Search in Book"),
            "highlights" => self.tr("划过的地方", "Highlights"),
            "type" => self.tr("把这一页调舒服", "Typography & Comfort"),
            "bookmarks" => self.tr("留过的记号", "Bookmarks"),
            _ => self.tr("文字的回响", "Text Echoes"),
        };
        let kicker = match self.drawer.as_str() {
            "companion" => self.tr(
                "SPOILER-FREE AI · 前情提要与人物图谱",
                "SPOILER-FREE AI · RECAP & CHARACTER GRAPH",
            ),
            "tts" => self.tr(
                "DUAL-TRACK AUDIOBOOK · 双轨朗读伴听",
                "DUAL-TRACK AUDIOBOOK · CLOUD & OFFLINE",
            ),
            "footnote" => self.tr(
                "PAPER-MARGIN FOOTNOTE · 离线词典与防剧透释义",
                "PAPER-MARGIN FOOTNOTE · OFFLINE LEXICON",
            ),
            "toc" => self.tr("IN THIS BOOK · 章节导航", "IN THIS BOOK · CHAPTERS"),
            "search" => self.tr(
                "FIND THE WORDS · 全书检索",
                "FIND THE WORDS · FULL-TEXT SEARCH",
            ),
            "highlights" => self.tr(
                "LINES WORTH KEEPING · 划线",
                "LINES WORTH KEEPING · HIGHLIGHTS",
            ),
            "type" => self.tr("READ YOUR WAY · 排版调音台", "READ YOUR WAY · TYPOGRAPHY"),
            "bookmarks" => self.tr("LITTLE MARKS · 书签记号", "LITTLE MARKS · BOOKMARKS"),
            _ => self.tr("A SMALL ECHO · 文字共鸣", "A SMALL ECHO · RESONANCE"),
        };
        let mut content = v_flex()
            .id("reader-drawer")
            .overflow_y_scroll()
            .size_full()
            .p_5()
            .gap_5()
            .child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .pb_3()
                    .border_b_1()
                    .border_color(self.border_subtle())
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(self.studio_badge("STUDIO", kicker, true))
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_family(self.display_font())
                                    .child(title),
                            ),
                    )
                    .child(self.button_with_icon(
                        self.tr("收起", "Close"),
                        IconName::X,
                        Action::CloseEcho,
                        cx,
                    )),
            );
        match self.drawer.as_str() {
            "companion" => {
                content = content.child(self.drawer_companion(cx));
            }
            "tts" => {
                content = content.child(self.drawer_tts(cx));
            }
            "footnote" => {
                content = content.child(self.drawer_footnote(cx));
            }
            "toc" => {
                if self.toc.is_empty() {
                    content = content.child(
                        v_flex()
                            .p_5()
                            .gap_3()
                            .rounded_xl()
                            .bg(rgb(hex(&t.background)).opacity(0.62))
                            .border_1()
                            .border_color(self.border_subtle())
                            .child(
                                Icon::new(IconName::ListTree)
                                    .size(px(22.))
                                    .text_color(rgb(hex(&t.accent))),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_family(self.ui_font())
                                    .line_height(relative(1.6))
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(if self.opened {
                                        self.tr(
                                            "这本书没有提供可读取的目录，可以继续滚动阅读。",
                                            "This book does not contain a chapter outline. You can keep reading smoothly.",
                                        )
                                    } else {
                                        self.tr(
                                            "目录正在整理，书页就绪后会自动出现在这里。",
                                            "Preparing table of contents… it will appear once the pages finish loading.",
                                        )
                                    }),
                            ),
                    );
                } else {
                    for item in &self.toc {
                        content = content.child(
                            v_flex()
                                .w_full()
                                .rounded_lg()
                                .border_b_1()
                                .border_color(self.border_subtle())
                                .child(self.button_with_icon(
                                    &item.label,
                                    IconName::ChevronRight,
                                    Action::Locate(item.href.clone()),
                                    cx,
                                )),
                        );
                    }
                }
            }
            "search" => {
                content = content.child(
                    v_flex()
                        .p_3p5()
                        .gap_3()
                        .rounded_xl()
                        .bg(rgb(hex(&t.background)).opacity(0.55))
                        .border_1()
                        .border_color(self.border_subtle())
                        .child(Input::new(&self.search_input).w_full())
                        .child(
                            self.button_with_icon(
                                if self.searching {
                                    self.tr("正在翻找…", "Searching…")
                                } else {
                                    self.tr("搜索全书", "Search Book")
                                },
                                IconName::Search,
                                Action::SubmitSearch,
                                cx,
                            )
                            .primary()
                            .disabled(self.searching),
                        ),
                );
                if self.searching {
                    content = content.child(
                        div()
                            .p_5()
                            .rounded_xl()
                            .bg(rgb(hex(&t.background)).opacity(0.62))
                            .text_size(px(13.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child(self.tr(
                                "一页一页找过去，找到会停在这里。",
                                "Scanning pages quietly — matches will appear here.",
                            )),
                    );
                } else if self.search_results.is_empty() {
                    content = content.child(
                        div()
                            .p_5()
                            .rounded_xl()
                            .bg(rgb(hex(&t.background)).opacity(0.62))
                            .text_size(px(13.))
                            .font_family(self.ui_font())
                            .line_height(relative(1.65))
                            .text_color(rgb(hex(&t.muted)))
                            .child(self.tr(
                                "可以搜人物、地点，或只记得的半句话。",
                                "Search for a character, place, or a half-remembered line.",
                            )),
                    );
                } else {
                    content =
                        content.child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .px_2p5()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(rgb(hex(&t.accent)).opacity(0.12))
                                        .text_size(px(11.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.accent)))
                                        .child(if self.locale().is_english() {
                                            format!("{} matches", self.search_results.len())
                                        } else {
                                            format!("找到 {} 处", self.search_results.len())
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child(self.tr(
                                            "点击直接跳转到对应段落",
                                            "Click to jump to passage",
                                        )),
                                ),
                        );
                    for hit in &self.search_results {
                        let chapter = if hit.chapter.is_empty() {
                            self.tr("书中位置", "Passage").to_owned()
                        } else {
                            hit.chapter.clone()
                        };
                        let excerpt = hit.excerpt.chars().take(86).collect::<String>();
                        content = content.child(
                            v_flex()
                                .w_full()
                                .gap_1p5()
                                .p_3()
                                .rounded_xl()
                                .bg(rgb(hex(&t.background)).opacity(0.56))
                                .border_1()
                                .border_color(self.border_subtle())
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.accent)))
                                        .child(chapter),
                                )
                                .child(self.button_with_icon(
                                    &excerpt,
                                    IconName::ChevronRight,
                                    Action::Locate(hit.cfi.clone()),
                                    cx,
                                )),
                        );
                    }
                }
            }
            "highlights" => {
                content = content.child(self.drawer_highlights(cx));
            }
            "type" => {
                content = content.child(self.drawer_type(cx));
            }
            "bookmarks" => {
                if let Some(b) = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                {
                    if b.bookmarks.is_empty() {
                        content = content.child(
                            v_flex()
                                .p_5()
                                .gap_3()
                                .rounded_xl()
                                .bg(rgb(hex(&t.background)).opacity(0.62))
                                .border_1()
                                .border_color(self.border_subtle())
                                .child(
                                    Icon::new(IconName::Bookmark)
                                        .size(px(22.))
                                        .text_color(rgb(hex(&t.accent))),
                                )
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child(self.tr(
                                            "读到喜欢的地方，可以留一个记号。",
                                            "Leave a bookmark whenever a page makes you pause.",
                                        )),
                                ),
                        );
                    }
                    for mark in &b.bookmarks {
                        content = content.child(
                            v_flex()
                                .w_full()
                                .border_b_1()
                                .border_color(self.border_subtle())
                                .child(self.button_with_icon(
                                    &mark.label,
                                    IconName::Bookmark,
                                    Action::Locate(mark.location.cfi.clone()),
                                    cx,
                                )),
                        );
                    }
                }
            }
            _ => {
                content = content.child(self.drawer_echo(cx));
            }
        }
        content.into_any_element()
    }
    #[inline(never)]
    pub(super) fn drawer_highlights(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let mut list = v_flex().gap_4();
        if let Some(book) = self
            .store
            .state
            .books
            .iter()
            .find(|book| Some(&book.id) == self.active_book.as_ref())
        {
            if book.highlights.is_empty() {
                list = list.child(
                    v_flex()
                        .p_5()
                        .gap_3()
                        .rounded_xl()
                        .bg(rgb(hex(&t.background)).opacity(0.62))
                        .border_1()
                        .border_color(self.border_subtle())
                        .child(
                            Icon::new(IconName::Feather)
                                .size(px(22.))
                                .text_color(rgb(hex(&t.accent))),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_family(self.ui_font())
                                .line_height(relative(1.65))
                                .text_color(rgb(hex(&t.muted)))
                                .child("选中喜欢的句子后，在悬浮微光条里点任意色点即可直接划线并写下批注。"),
                        ),
                );
            }
            for highlight in book.highlights.iter().rev() {
                let excerpt = highlight.text.chars().take(180).collect::<String>();
                let hl_hex = highlight_color_hex_for(
                    highlight.color.as_deref(),
                    hex(&t.accent),
                    self.is_dark(),
                );
                let hl_label = highlight_color_name(highlight.color.as_deref());
                let is_editing =
                    self.highlight_note_target.as_deref() == Some(highlight.id.as_str());
                let mut color_dots = h_flex().gap_1p5().items_center();
                for (cid, chex) in [
                    ("amber", 0xf59e0bu32),
                    ("sage", 0x34d399u32),
                    ("rose", 0xfb7185u32),
                    ("prism", 0xa855f7u32),
                ] {
                    let selected = highlight.color.as_deref() == Some(cid);
                    let hid = highlight.id.clone();
                    color_dots = color_dots.child(
                        div()
                            .id(SharedString::from(format!("hl-dot-{}-{cid}", highlight.id)))
                            .cursor_pointer()
                            .p(px(1.5))
                            .rounded_full()
                            .border_1()
                            .border_color(if selected {
                                rgb(chex)
                            } else {
                                rgb(0x000000).opacity(0.0)
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.act(
                                        Action::RecolorHighlight(Box::new((
                                            hid.clone(),
                                            cid.into(),
                                        ))),
                                        window,
                                        cx,
                                    );
                                }),
                            )
                            .child(div().size(px(10.)).rounded_full().bg(rgb(chex))),
                    );
                }
                let card_draft = QuoteCardDraft {
                    text: highlight.text.clone(),
                    attribution: format!("《{}》 · {}", book.title, book.author),
                    note: highlight.note.clone(),
                    style: QuoteCardStyle::from_theme_id(&t.id),
                };
                let mut hl_card = v_flex()
                    .w_full()
                    .p_4()
                    .gap_3()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(0.62))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(rgb(hl_hex).opacity(0.18))
                                            .text_size(px(10.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hl_hex))
                                            .child(hl_label),
                                    )
                                    .child(color_dots),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(display_time(&highlight.created_at)),
                            ),
                    )
                    .child(
                        div()
                            .pl_3()
                            .border_l_3()
                            .border_color(rgb(hl_hex))
                            .text_size(px(15.5))
                            .font_family(self.display_font())
                            .line_height(relative(1.6))
                            .child(excerpt),
                    );
                if is_editing {
                    let note_chars = self
                        .highlight_note_input
                        .read(cx)
                        .value()
                        .trim()
                        .chars()
                        .count();
                    hl_card = hl_card.child(
                        v_flex()
                            .gap_2()
                            .p_2p5()
                            .rounded_lg()
                            .bg(rgb(hex(&t.paper)).opacity(0.92))
                            .border_1()
                            .border_color(rgb(hl_hex).opacity(0.45))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hl_hex))
                                            .child("写下读时批注"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(if note_chars > 1000 {
                                                0xb34f36
                                            } else {
                                                hex(&t.muted)
                                            }))
                                            .child(format!("{note_chars} / 1000 字")),
                                    ),
                            )
                            .child(
                                Textarea::new(&self.highlight_note_input)
                                    .aria_label("划线批注")
                                    .w_full(),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        self.button_with_icon(
                                            "保存批注",
                                            IconName::Check,
                                            Action::SaveHighlightNote,
                                            cx,
                                        )
                                        .primary(),
                                    )
                                    .child(self.button("取消", Action::CancelHighlightNote, cx)),
                            ),
                    );
                } else if let Some(note) =
                    highlight.note.as_deref().filter(|n| !n.trim().is_empty())
                {
                    hl_card = hl_card.child(
                        v_flex()
                            .p_2p5()
                            .gap_1()
                            .rounded_lg()
                            .bg(rgb(hex(&t.paper)).opacity(0.75))
                            .border_1()
                            .border_color(self.border_subtle())
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hl_hex))
                                    .child("读时批注 · NOTE"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_family(self.ui_font())
                                    .line_height(relative(1.6))
                                    .text_color(rgb(hex(&t.text)).opacity(0.9))
                                    .child(note.to_owned()),
                            ),
                    );
                }
                hl_card = hl_card.child(
                    h_flex()
                        .gap_1p5()
                        .flex_wrap()
                        .child(self.button("回到原文", Action::Locate(highlight.cfi.clone()), cx))
                        .child(self.button_with_icon(
                            if highlight.note.is_some() {
                                "编辑批注"
                            } else {
                                "批注"
                            },
                            IconName::PenLine,
                            Action::EditHighlightNote(highlight.id.clone()),
                            cx,
                        ))
                        .child(self.button_with_icon(
                            "书摘卡",
                            IconName::ImageDown,
                            Action::OpenQuoteCard(Box::new(card_draft)),
                            cx,
                        ))
                        .child(self.button(
                            "删除",
                            Action::DeleteHighlight(highlight.id.clone()),
                            cx,
                        )),
                );
                list = list.child(hl_card);
            }
        }
        list.into_any_element()
    }
    #[inline(never)]
    pub(super) fn drawer_type(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let size = self
            .store
            .state
            .reader_font_size
            .unwrap_or_else(|| self.theme().font_size);
        let leading = self
            .store
            .state
            .reader_line_height
            .unwrap_or_else(|| self.theme().line_height);
        let width = self
            .store
            .state
            .reader_width
            .unwrap_or_else(|| self.theme().width);
        let size_ratio = ((size - 14.0) / (36.0 - 14.0)).clamp(0.05, 1.0);
        let leading_ratio = ((leading - 1.3) / (2.6 - 1.3)).clamp(0.05, 1.0);
        let width_ratio = ((width as f32 - 400.0) / (1100.0 - 400.0)).clamp(0.05, 1.0);
        v_flex()
            .gap_4()
            .child(
                v_flex()
                    .p_4()
                    .gap_4()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.background)).opacity(0.62))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        v_flex()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.muted)))
                                                    .child(
                                                        self.tr(
                                                            "正文字号 · FONT SIZE",
                                                            "FONT SIZE",
                                                        ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(20.))
                                                    .font_family(self.display_font())
                                                    .child(format!("{size:.0} pt")),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .child(self.button("A−", Action::TypeSize(-1), cx))
                                            .child(self.button("A+", Action::TypeSize(1), cx)),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(4.))
                                    .rounded_full()
                                    .bg(rgb(hex(&t.text)).opacity(0.10))
                                    .child(
                                        div()
                                            .w(px(280.0 * size_ratio))
                                            .h_full()
                                            .rounded_full()
                                            .bg(rgb(hex(&t.accent))),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        v_flex()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.muted)))
                                                    .child(self.tr(
                                                        "行距呼吸感 · LINE HEIGHT",
                                                        "LINE HEIGHT",
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(19.))
                                                    .font_family(self.display_font())
                                                    .child(format!("{leading:.1}")),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .child(self.button(
                                                self.tr("紧", "Tighter"),
                                                Action::TypeLeading(-1),
                                                cx,
                                            ))
                                            .child(self.button(
                                                self.tr("松", "Airier"),
                                                Action::TypeLeading(1),
                                                cx,
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(4.))
                                    .rounded_full()
                                    .bg(rgb(hex(&t.text)).opacity(0.10))
                                    .child(
                                        div()
                                            .w(px(280.0 * leading_ratio))
                                            .h_full()
                                            .rounded_full()
                                            .bg(rgb(hex(&t.accent))),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        v_flex()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.muted)))
                                                    .child(
                                                        self.tr("版心宽度 · MEASURE", "MEASURE"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(19.))
                                                    .font_family(self.display_font())
                                                    .child(format!("{width} px")),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .child(self.button(
                                                self.tr("窄", "Narrower"),
                                                Action::TypeWidth(-1),
                                                cx,
                                            ))
                                            .child(self.button(
                                                self.tr("宽", "Wider"),
                                                Action::TypeWidth(1),
                                                cx,
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(4.))
                                    .rounded_full()
                                    .bg(rgb(hex(&t.text)).opacity(0.10))
                                    .child(
                                        div()
                                            .w(px(280.0 * width_ratio))
                                            .h_full()
                                            .rounded_full()
                                            .bg(rgb(hex(&t.accent))),
                                    ),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .p_3p5()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(0.62))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_family(self.ui_font())
                                    .child(self.tr("护眼暖光滤镜", "Warm Eye-Care Filter")),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(self.tr(
                                        "柔和滤去屏幕冷蓝光，久读不累",
                                        "Softens harsh blue light for comfortable night reading",
                                    )),
                            ),
                    )
                    .child(self.pill_button(
                        if self.store.state.warm_filter {
                            self.tr("暖光：开", "Warm: On")
                        } else {
                            self.tr("暖光：关", "Warm: Off")
                        },
                        Some(IconName::Sun),
                        self.store.state.warm_filter,
                        Action::ToggleWarmFilter,
                        cx,
                    )),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .font_family(self.ui_font())
                    .line_height(relative(1.65))
                    .text_color(rgb(hex(&t.muted)))
                    .child(self.tr(
                        "调整会立即同步至当前书页，并对之后打开的书保留。",
                        "Changes apply immediately to this page and persist for future books.",
                    )),
            )
            .child(self.button(
                self.tr("恢复主题默认", "Reset to Theme Defaults"),
                Action::ResetTypography,
                cx,
            ))
            .into_any_element()
    }
    #[inline(never)]
    pub(super) fn drawer_echo(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let mut box_el = v_flex().gap_4();
        if let Some(echo) = &self.echo {
            let book_ref = self
                .store
                .state
                .books
                .iter()
                .find(|book| Some(&book.id) == self.active_book.as_ref());
            let already_kept = book_ref.is_some_and(|book| {
                book.highlights
                    .iter()
                    .any(|highlight| highlight.cfi == self.echo_cfi)
            });
            let book_attr = book_ref
                .map(|b| format!("《{}》 · {}", b.title, b.author))
                .unwrap_or_else(|| self.tr("微光阅读", "Glow Reading").to_owned());
            let card_draft = QuoteCardDraft {
                text: self.echo_input.clone(),
                attribution: book_attr,
                note: None,
                style: QuoteCardStyle::from_theme_id(&t.id),
            };
            let mut color_row = h_flex().gap_1p5().items_center();
            for (cid, chex) in [
                ("amber", 0xf59e0bu32),
                ("sage", 0x34d399u32),
                ("rose", 0xfb7185u32),
                ("prism", 0xa855f7u32),
            ] {
                color_row = color_row.child(
                    div()
                        .id(SharedString::from(format!("echo-color-{cid}")))
                        .cursor_pointer()
                        .p(px(2.))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(chex).opacity(0.45))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.act(Action::HighlightColor(cid.into()), window, cx);
                            }),
                        )
                        .child(div().size(px(11.)).rounded_full().bg(rgb(chex))),
                );
            }
            box_el = box_el.child(
                v_flex()
                    .gap_2()
                    .p_2p5()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(0.55))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        h_flex()
                            .gap_1p5()
                            .flex_wrap()
                            .items_center()
                            .child(
                                self.button_with_icon(
                                    if already_kept {
                                        self.tr("已留下划线", "Highlighted")
                                    } else {
                                        self.tr("留下划线", "Highlight")
                                    },
                                    IconName::Feather,
                                    Action::Highlight,
                                    cx,
                                )
                                .disabled(already_kept),
                            )
                            .child(color_row),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .flex_wrap()
                            .child(
                                self.button_with_icon(
                                    if self.echo_saved {
                                        self.tr("已存入日记", "Saved to Journal")
                                    } else {
                                        self.tr("存入日记", "Save to Journal")
                                    },
                                    IconName::NotebookPen,
                                    Action::SaveEchoToJournal,
                                    cx,
                                )
                                .disabled(self.echo_saved),
                            )
                            .child(self.button_with_icon(
                                self.tr("书摘卡", "Quote Card"),
                                IconName::ImageDown,
                                Action::OpenQuoteCard(Box::new(card_draft)),
                                cx,
                            ))
                            .child(self.button_with_icon(
                                self.tr("纸边注脚", "Footnote"),
                                IconName::BookOpen,
                                Action::OpenFootnote(self.echo_input.clone()),
                                cx,
                            ))
                            .child(self.button_with_icon(
                                self.tr("所有划线", "All Highlights"),
                                IconName::Bookmark,
                                Action::Highlights,
                                cx,
                            )),
                    ),
            );
            // Cross-book local FTS5 personal memory resonance (100% offline & zero cost!)
            if !self.echo_memories.is_empty() {
                let mut mem_card = v_flex()
                    .gap_2p5()
                    .p_3p5()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(if self.is_dark() { 0.68 } else { 0.78 }))
                    .border_1()
                    .border_color(rgb(hex(&t.accent)).opacity(0.32))
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(if self.locale().is_english() {
                                        format!(
                                            "Cross-Book Memory Recall · {} echoes",
                                            self.echo_memories.len()
                                        )
                                    } else {
                                        format!(
                                            "跨书记忆重逢 · 本地召回 {} 条",
                                            self.echo_memories.len()
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(self.tr("SQLite FTS5 离线检索", "Offline SQLite FTS5")),
                            ),
                    );
                for hit in &self.echo_memories {
                    let date = hit.created_at.get(..10).unwrap_or(&hit.created_at);
                    let snippet: String = hit.excerpt.chars().take(74).collect();
                    let label = if hit.kind == "journal" {
                        if self.locale().is_english() {
                            format!("Journal · {date}")
                        } else {
                            format!("手账 · {date}")
                        }
                    } else if self.locale().is_english() {
                        format!("{} · {date}", hit.book_title)
                    } else {
                        format!("《{}》 · {date}", hit.book_title)
                    };
                    mem_card = mem_card.child(
                        v_flex()
                            .p_2p5()
                            .gap_1()
                            .rounded_lg()
                            .bg(rgb(hex(&t.paper)).opacity(0.84))
                            .border_l_2()
                            .border_color(rgb(hex(&t.accent)).opacity(0.65))
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_family(self.display_font())
                                    .line_height(relative(1.5))
                                    .text_color(rgb(hex(&t.text)).opacity(0.90))
                                    .child(if self.locale().is_english() {
                                        format!("“{snippet}”")
                                    } else {
                                        format!("「{snippet}」")
                                    }),
                            )
                            .when_some(
                                hit.note.as_deref().filter(|n| !n.trim().is_empty()),
                                |this, note| {
                                    this.child(
                                        div()
                                            .text_size(px(10.5))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.muted)))
                                            .child(if self.locale().is_english() {
                                                format!("Your note: {note}")
                                            } else {
                                                format!("当时批注：{note}")
                                            }),
                                    )
                                },
                            ),
                    );
                }
                box_el = box_el.child(mem_card);
            }
            box_el = box_el
                .child(
                    v_flex()
                        .gap_2()
                        .p_3()
                        .rounded_xl()
                        .bg(rgb(hex(&t.accent)).opacity(if self.is_dark() { 0.14 } else { 0.09 }))
                        .border_1()
                        .border_color(rgb(hex(&t.accent)).opacity(0.28))
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .gap_2()
                                .child(
                                    self.button_with_icon(
                                        if self.echo_ai_rx.is_some() {
                                            self.tr("正在细读这段文字…", "Reading passage closely…")
                                        } else if !self.echo_memories.is_empty() {
                                            self.tr(
                                                "结合过往记忆 · AI 深度共读",
                                                "Deep AI Reflection with Past Echoes",
                                            )
                                        } else {
                                            self.tr(
                                                "问问微光（AI 深度共读）",
                                                "Ask Glow (AI Deep Reading)",
                                            )
                                        },
                                        IconName::WandSparkles,
                                        Action::AskAiPassage,
                                        cx,
                                    )
                                    .primary()
                                    .disabled(self.echo_ai_rx.is_some()),
                                )
                                .when(self.echo_ai_rx.is_some(), |this| {
                                    this.child(self.button(
                                        self.tr("停止", "Stop"),
                                        Action::CancelAiPassage,
                                        cx,
                                    ))
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(10.))
                                .font_family(self.ui_font())
                                .line_height(relative(1.5))
                                .text_color(rgb(hex(&t.muted)))
                                .child(if self.echo_memories.is_empty() {
                                    self.tr(
                                        "手动触发：仅发送本段选文与书名给已配置的 AI 模型，不上传整本书或日记。",
                                        "Manual trigger: sends only this excerpt and title to your configured AI model — never the whole book or journal.",
                                    )
                                } else {
                                    self.tr(
                                        "手动触发：仅发送本段选文与上方召回的少量关联片段，绝不上传整个书库。",
                                        "Manual trigger: sends only this excerpt and the recalled snippets above — never your full library.",
                                    )
                                }),
                        ),
                )
                .child(self.echo_card(echo, &self.echo_input, true, cx));
        }
        box_el.into_any_element()
    }
    #[inline(never)]
    pub(super) fn drawer_footnote(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let is_dark = self.is_dark();
        let mut view = v_flex().gap_4().child(
            v_flex()
                .p_4()
                .gap_2()
                .rounded_xl()
                .bg(rgb(hex(&t.background)).opacity(if is_dark { 0.65 } else { 0.76 }))
                .border_l_3()
                .border_color(rgb(hex(&t.accent)))
                .child(
                    div()
                        .text_size(px(10.))
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.accent)))
                        .child(self.tr(
                            "选中的词句 · MARGIN TERM",
                            "SELECTED TERM · MARGIN FOOTNOTE",
                        )),
                )
                .child(
                    div()
                        .text_size(px(17.))
                        .font_family(self.display_font())
                        .line_height(relative(1.45))
                        .child(if self.footnote_term.is_empty() {
                            self.tr("尚未选中词句", "No term selected").to_owned()
                        } else {
                            self.footnote_term.clone()
                        }),
                ),
        );
        if let Some(entry) = self.footnote_dict {
            view = view.child(
                v_flex()
                    .p_4()
                    .gap_2p5()
                    .rounded_xl()
                    .bg(rgb(hex(&t.paper)).opacity(0.94))
                    .border_1()
                    .border_color(rgb(hex(&t.accent)).opacity(0.32))
                    .child(
                        h_flex()
                            .justify_between()
                            .items_baseline()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(19.))
                                    .font_family(self.display_font())
                                    .child(entry.headword),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(entry.reading),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_family(self.ui_font())
                            .line_height(relative(1.65))
                            .child(entry.brief),
                    )
                    .child(
                        div()
                            .pt_2()
                            .border_t_1()
                            .border_color(self.border_subtle())
                            .text_size(px(11.5))
                            .font_family(self.ui_font())
                            .line_height(relative(1.6))
                            .text_color(rgb(hex(&t.muted)))
                            .child(entry.literary_note),
                    ),
            );
        } else {
            view = view.child(
                div()
                    .p_4()
                    .rounded_xl()
                    .bg(rgb(hex(&t.background)).opacity(0.56))
                    .text_size(px(12.5))
                    .font_family(self.ui_font())
                    .line_height(relative(1.65))
                    .text_color(rgb(hex(&t.muted)))
                    .child(self.tr(
                        "离线文学词典暂未收录该词条，可点击下方按钮请求 AI 给出克制的防剧透纸边释义。",
                        "Not found in the offline literary glossary. Click below for a calm, spoiler-free AI margin note.",
                    )),
            );
        }
        if let Some(reply) = &self.footnote_ai_reply {
            view = view.child(self.reply_card(reply));
        }
        view.child(
            h_flex()
                .gap_2()
                .items_center()
                .flex_wrap()
                .child(
                    self.button_with_icon(
                        if self.footnote_ai_rx.is_some() {
                            self.tr("正在生成注脚…", "Generating Footnote…")
                        } else {
                            self.tr("AI 防剧透释义", "Spoiler-Free AI Note")
                        },
                        IconName::WandSparkles,
                        Action::AskAiFootnote,
                        cx,
                    )
                    .primary()
                    .disabled(self.footnote_ai_rx.is_some() || self.footnote_term.is_empty()),
                )
                .child(self.button_with_icon(
                    self.tr("关闭注脚", "Close Footnote"),
                    IconName::X,
                    Action::CloseFootnote,
                    cx,
                )),
        )
        .into_any_element()
    }
    #[inline(never)]
    pub(super) fn echo_card(
        &self,
        echo: &EchoResult,
        input: &str,
        is_passage: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = self.theme();
        let mut view = v_flex()
            .gap_4()
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .px_2p5()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgb(hex(&t.accent)).opacity(0.14))
                                    .border_1()
                                    .border_color(rgb(hex(&t.accent)).opacity(0.28))
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(if echo.demo {
                                        "示例回响"
                                    } else if is_passage {
                                        "AI 段落共读"
                                    } else {
                                        "AI 心情回复"
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(if echo.demo {
                                        "精选原文 · 预写回复"
                                    } else if is_passage {
                                        "段落深度共读 · OpenAI"
                                    } else {
                                        "根据本次文字生成 · OpenAI"
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_family(self.display_font())
                            .line_height(relative(1.32))
                            .child(if echo.demo {
                                "这段话，也让我想起——"
                            } else if is_passage {
                                "陪你细读这一段"
                            } else {
                                "写给此刻的你"
                            }),
                    ),
            )
            .child(
                v_flex()
                    .p_3p5()
                    .gap_1p5()
                    .rounded_xl()
                    .border_l_3()
                    .border_color(rgb(hex(&t.accent)))
                    .bg(rgb(hex(&t.background)).opacity(0.52))
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.accent)))
                            .child("你留下的文字"),
                    )
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_family(self.ui_font())
                            .line_height(relative(1.62))
                            .text_color(rgb(hex(&t.text)).opacity(0.88))
                            .child(input.chars().take(250).collect::<String>()),
                    ),
            );
        for (index, q) in echo.quotes.iter().enumerate() {
            let mut quote = v_flex()
                .p_4()
                .gap_3()
                .rounded_xl()
                .bg(rgb(hex(&t.background)).opacity(0.68))
                .border_1()
                .border_color(self.border_subtle());
            quote = quote.child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(32.))
                            .line_height(relative(0.75))
                            .font_family(self.display_font())
                            .text_color(rgb(hex(&t.accent)).opacity(0.75))
                            .child("“"),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child("LITERARY RESONANCE · 典籍回声"),
                    ),
            );
            if q.spoiler && !self.spoilers.contains(&q.id) {
                quote = quote.child(self.button_with_icon(
                    "可能涉及剧情 · 展开",
                    IconName::ChevronRight,
                    Action::RevealSpoiler(q.id.clone()),
                    cx,
                ));
            } else {
                quote = quote.child(
                    div()
                        .text_size(px(17.))
                        .font_family(self.display_font())
                        .line_height(relative(1.6))
                        .child(q.text.clone()),
                );
            }
            let source = q.source.clone();
            quote = quote
                .child(div().h(px(1.)).w_full().bg(self.border_subtle()))
                .child(
                    h_flex()
                        .justify_between()
                        .items_end()
                        .gap_2()
                        .child(
                            v_flex()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.text)))
                                        .child(format!("{} ·《{}》", q.author, q.work)),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .font_family(self.ui_font())
                                        .line_height(relative(1.5))
                                        .text_color(rgb(hex(&t.muted)))
                                        .child(format!("{} · {}", q.chapter, q.edition)),
                                ),
                        )
                        .child(
                            Button::new(SharedString::from(format!("source-{}", q.id)))
                                .ghost()
                                .label("核对出处")
                                .text_color(rgb(hex(&t.accent)))
                                .on_click(move |_, _, cx| cx.open_url(&source)),
                        ),
                );
            view = view.child(quote);
            if index == 0 {
                view = view.child(self.reply_card(&echo.reply));
            }
        }
        if echo.quotes.is_empty() {
            view = view.child(self.reply_card(&echo.reply));
        }
        view.into_any_element()
    }
    pub(super) fn reply_card(&self, reply: &str) -> Div {
        let t = self.theme();
        v_flex()
            .gap_2p5()
            .p_4()
            .rounded_xl()
            .bg(rgb(hex(&t.accent)).opacity(if self.is_dark() { 0.16 } else { 0.11 }))
            .border_1()
            .border_color(rgb(hex(&t.accent)).opacity(0.32))
            .border_l_3()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::Sparkles)
                                    .size(px(15.))
                                    .text_color(rgb(hex(&t.accent))),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child("一束微光"),
                            ),
                    )
                    .child(self.waveform_bars(true, &[4., 8., 11., 6., 9., 5.])),
            )
            .child(
                div()
                    .text_size(px(14.5))
                    .font_family(self.ui_font())
                    .line_height(relative(1.72))
                    .child(reply.to_owned()),
            )
    }
}

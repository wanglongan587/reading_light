use super::*;

impl ReadingApp {
    #[inline(never)]
    pub(super) fn render_welcome(&self, t: &ReadingTheme, cx: &Context<Self>) -> AnyElement {
        let step = self.onboarding_step.min(2);
        let mut steps_bar = h_flex().gap_2().items_center();
        for (idx, label_zh, label_en) in [
            (0u8, "1 · 选书房", "1 · Space"),
            (1u8, "2 · 挑一本书", "2 · Pick a Book"),
            (2u8, "3 · 安心启程", "3 · Local-First"),
        ] {
            let active = step == idx;
            steps_bar = steps_bar.child(self.pill_button(
                self.tr(label_zh, label_en),
                None,
                active,
                Action::OnboardingStep(idx),
                cx,
            ));
        }

        let step_body = match step {
            0 => v_flex()
                .gap_4()
                .child(self.heading(
                    self.tr(
                        "STEP 1 OF 3 · 30 秒布置你的微光书房",
                        "STEP 1 OF 3 · SET UP YOUR READING ROOM IN 30 SECONDS",
                    ),
                    self.tr("挑一间今晚想待着的书房。", "Choose a room for tonight’s reading."),
                    self.tr(
                        "每一套都是原创微光空间，带专属配色与纸张氛围，随时可一键切换。",
                        "Each original Glow Space pairs custom paper tones and atmosphere — switch anytime.",
                    ),
                ))
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .flex_wrap()
                        .child(self.pill_button(
                            self.tr("晴空旧书房 · 蓝天流云", "Sunlit Study · Clouds"),
                            Some(IconName::Cloud),
                            t.id == "forest",
                            Action::PickTheme(0),
                            cx,
                        ))
                        .child(self.pill_button(
                            self.tr("夏风樟树下 · 林荫", "Camphor Shade · Breeze"),
                            Some(IconName::Leaf),
                            t.id == "totoro",
                            Action::PickTheme(4),
                            cx,
                        ))
                        .child(self.pill_button(
                            self.tr("浅海晚风号 · 黄昏", "Evening Tide · Dusk"),
                            Some(IconName::Sparkles),
                            t.id == "spirited",
                            Action::PickTheme(5),
                            cx,
                        ))
                        .child(self.pill_button(
                            self.tr("雨夜灯笼巷 · 暖灯", "Lantern Alley · Rain"),
                            Some(IconName::Flame),
                            t.id == "lantern",
                            Action::PickTheme(8),
                            cx,
                        ))
                        .child(self.pill_button(
                            self.tr("星空棱镜 · 暗夜流光", "Starlight Prism · Night"),
                            Some(IconName::Sparkles),
                            t.id == "prism",
                            Action::PickTheme(3),
                            cx,
                        )),
                )
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .flex_wrap()
                        .child(
                            self.button_with_icon(
                                self.tr("下一步：挑一本书", "Next: Pick a Book"),
                                IconName::ChevronRight,
                                Action::OnboardingStep(1),
                                cx,
                            )
                            .primary(),
                        )
                        .child(self.button_with_icon(
                            self.tr("知道了，先坐一会儿", "Skip for Now"),
                            IconName::BookOpen,
                            Action::Privacy,
                            cx,
                        )),
                )
                .into_any_element(),
            1 => {
                let mut picks = h_flex().gap_2p5().flex_wrap();
                for book in catalog::all_books().iter().take(4) {
                    let cid = book.id.to_owned();
                    picks = picks.child(
                        v_flex()
                            .id(SharedString::from(format!("onboard-book-{cid}")))
                            .cursor_pointer()
                            .w(px(266.))
                            .p_3p5()
                            .gap_1p5()
                            .rounded_xl()
                            .bg(rgb(hex(&t.background)).opacity(0.68))
                            .border_1()
                            .border_color(self.border_subtle())
                            .hover(|s| s.border_color(rgb(hex(&t.accent)).opacity(0.65)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.act(
                                        Action::OnboardingPickCatalog(cid.clone()),
                                        window,
                                        cx,
                                    );
                                }),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(14.5))
                                            .font_family(self.display_font())
                                            .child(if self.locale().is_english() {
                                                book.title.to_owned()
                                            } else {
                                                format!("《{}》", book.title)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.accent)))
                                            .child(if self.locale().is_english() {
                                                format!("~{} min", book.minutes)
                                            } else {
                                                format!("约 {} 分钟", book.minutes)
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(format!("{} · {}", book.author, book.era)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .line_height(relative(1.5))
                                    .text_color(rgb(hex(&t.text)).opacity(0.84))
                                    .child(book.tagline),
                            ),
                    );
                }
                v_flex()
                    .gap_4()
                    .child(self.heading(
                        self.tr("STEP 2 OF 3 · 微光公版馆", "STEP 2 OF 3 · GLOW PUBLIC CLASSICS"),
                        self.tr("点一本立即翻开，不必空着书架。", "Open a classic right away — no empty shelf."),
                        self.tr(
                            "全部内置离线公版/原创作品，点击即可直接进入书页开始慢读。",
                            "Built-in offline classics and original essays. Click any book to start reading.",
                        ),
                    ))
                    .child(picks)
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .flex_wrap()
                            .child(self.button(
                                self.tr("上一步", "Back"),
                                Action::OnboardingStep(0),
                                cx,
                            ))
                            .child(
                                self.button_with_icon(
                                    self.tr("下一步：隐私承诺", "Next: Privacy Promise"),
                                    IconName::ChevronRight,
                                    Action::OnboardingStep(2),
                                    cx,
                                )
                                .primary(),
                            )
                            .child(self.button(
                                self.tr("稍后自己导入", "Import My Own Later"),
                                Action::Privacy,
                                cx,
                            )),
                    )
                    .into_any_element()
            }
            _ => v_flex()
                .gap_4()
                .child(self.heading(
                    "STEP 3 OF 3 · LOCAL-FIRST",
                    self.tr("你的书与心事，只留在你手里。", "Your books and thoughts stay strictly yours."),
                    self.tr(
                        "书库、划线、手账与全文检索全部以 SQLite 本地离线存储；无广告、无强制登录、无打卡焦虑。",
                        "Library, highlights, journal, and FTS5 search live locally in SQLite — no ads, no forced accounts, no streaks.",
                    ),
                ))
                .child(
                    div()
                        .p_4()
                        .rounded_xl()
                        .bg(rgb(hex(&t.background)).opacity(0.62))
                        .border_1()
                        .border_color(self.border_subtle())
                        .text_size(px(13.))
                        .font_family(self.ui_font())
                        .line_height(relative(1.7))
                        .text_color(rgb(hex(&t.muted)))
                        .child(self.tr(
                            "仅当你主动触发 AI 共读、纸边释义或完读信润色时，才将当次最小片段发送给你配置的模型（支持本地 Ollama 完全离线）；平时零联网上传。",
                            "Only when you explicitly invoke AI reflection, footnotes, or completion letters is the minimal excerpt sent to your chosen model (supports 100% offline Ollama).",
                        )),
                )
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(self.button(
                            self.tr("上一步", "Back"),
                            Action::OnboardingStep(1),
                            cx,
                        ))
                        .child(
                            self.button_with_icon(
                                self.tr("进入我的微光书房", "Enter My Reading Room"),
                                IconName::BookOpen,
                                Action::Privacy,
                                cx,
                            )
                            .primary(),
                        ),
                )
                .into_any_element(),
        };

        v_flex()
            .size_full()
            .p_8()
            .items_center()
            .justify_center()
            .bg(rgb(hex(&t.background)).opacity(0.45))
            .child(
                v_flex()
                    .w(px(640.))
                    .max_w_full()
                    .p_7()
                    .gap_5()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(0.97))
                    .border_1()
                    .border_color(self.border_strong())
                    .shadow_lg()
                    .child(self.specular_line())
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .flex_wrap()
                            .gap_3()
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(
                                        img("glow-icon.png")
                                            .size(px(48.))
                                            .object_fit(ObjectFit::Contain)
                                            .rounded_2xl(),
                                    )
                                    .child(self.studio_badge("GLOW 2.0", "LOCAL-FIRST", true)),
                            )
                            .child(steps_bar),
                    )
                    .child(step_body),
            )
            .into_any_element()
    }
    #[inline(never)]
    pub(super) fn render_confirm(
        &self,
        t: &ReadingTheme,
        confirm: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        v_flex()
            .size_full()
            .p_8()
            .items_center()
            .justify_center()
            .bg(rgb(hex(&t.background)).opacity(0.55))
            .child(
                v_flex()
                    .w(px(520.))
                    .max_w_full()
                    .p_7()
                    .gap_5()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(0.98))
                    .border_1()
                    .border_color(self.border_strong())
                    .shadow_lg()
                    .child(self.specular_line())
                    .child(self.studio_badge(
                        "CONFIRM",
                        self.tr("请再确认一次", "PLEASE CONFIRM"),
                        true,
                    ))
                    .child(
                        div()
                            .text_size(px(26.))
                            .font_family(self.display_font())
                            .child(if confirm == "journal" {
                                self.tr("清空全部私人日记？", "Clear all journal entries?")
                            } else {
                                self.tr("把这本书移出书架？", "Remove this book from shelf?")
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_family(self.ui_font())
                            .line_height(relative(1.7))
                            .text_color(rgb(hex(&t.muted)))
                            .child(if confirm == "journal" {
                                self.tr(
                                    "清空后无法在应用内恢复。书籍和阅读进度不会删除。",
                                    "Cleared entries cannot be restored inside the app. Your books and reading progress stay safe.",
                                )
                            } else {
                                self.tr(
                                    "用户原文件不会删除，已经留下的日记仍会保留。",
                                    "Your original file on disk will not be deleted, and journal notes remain intact.",
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .child(self.button(self.tr("保留", "Keep"), Action::Cancel, cx))
                            .child(
                                self.button(
                                    self.tr("确认", "Confirm"),
                                    if confirm == "journal" {
                                        Action::ConfirmClear
                                    } else {
                                        Action::ConfirmRemove
                                    },
                                    cx,
                                )
                                .primary(),
                            ),
                    ),
            )
            .into_any_element()
    }
    #[inline(never)]
    pub(super) fn quote_card_modal(
        &self,
        draft: &QuoteCardDraft,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = self.theme();
        let (bg_hex, card_hex, text_hex, muted_hex, accent_hex, tag_text) = match draft.style {
            QuoteCardStyle::Sky => (
                0x8ecbf6u32,
                0xf9fdffu32,
                0x1a324cu32,
                0x4f6e8eu32,
                0x2573b8u32,
                "GLOW · 晴空书房书摘",
            ),
            QuoteCardStyle::Totoro => (
                0xb7d9b8u32,
                0xf9fcf6u32,
                0x1f3626u32,
                0x4e6b56u32,
                0x3d7a52u32,
                "GLOW · 夏风樟树书摘",
            ),
            QuoteCardStyle::Spirited => (
                0xeec4b8u32,
                0xfff9f5u32,
                0x3b2427u32,
                0x745458u32,
                0xb84b42u32,
                "GLOW · 浅海晚风书摘",
            ),
            QuoteCardStyle::Howl => (
                0xc8c2efu32,
                0xfcfbf7u32,
                0x2b2740u32,
                0x5f5a7au32,
                0x63549eu32,
                "GLOW · 秘密花园书摘",
            ),
            QuoteCardStyle::Kiki => (
                0xead4b7u32,
                0xfffdf7u32,
                0x3b2a1eu32,
                0x755d4au32,
                0xae572au32,
                "GLOW · 海港暖阳书摘",
            ),
            QuoteCardStyle::Lantern => (
                0x17141fu32,
                0x201b26u32,
                0xf5e9d6u32,
                0xbca895u32,
                0xe69f52u32,
                "GLOW · 雨夜灯巷书摘",
            ),
            QuoteCardStyle::Prism => (
                0x090c16u32,
                0x121624u32,
                0xeef1fau32,
                0x94a0beu32,
                0xa78bfau32,
                "GLOW · 星空棱镜书摘",
            ),
            QuoteCardStyle::Paper => (
                0xede5d4u32,
                0xfffcf5u32,
                0x2e2923u32,
                0x70665au32,
                0x8a5e3bu32,
                "GLOW · 素纸信笺书摘",
            ),
        };
        let mut style_row = h_flex().gap_2().flex_wrap();
        for style in QuoteCardStyle::ALL {
            style_row = style_row.child(self.pill_button(
                style.label(),
                None,
                draft.style == style,
                Action::QuoteCardStyle(style),
                cx,
            ));
        }
        let preview_card = v_flex()
            .w(px(360.))
            .p_5()
            .rounded_2xl()
            .bg(rgb(bg_hex))
            .border_1()
            .border_color(rgb(accent_hex).opacity(0.35))
            .shadow_md()
            .child(
                v_flex()
                    .w_full()
                    .p_5()
                    .gap_3()
                    .rounded_xl()
                    .bg(rgb(card_hex))
                    .border_1()
                    .border_color(rgb(accent_hex).opacity(0.28))
                    .child(
                        div()
                            .px_2p5()
                            .py_0p5()
                            .rounded_full()
                            .bg(rgb(accent_hex).opacity(0.14))
                            .text_size(px(9.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(accent_hex))
                            .child(tag_text),
                    )
                    .child(
                        div()
                            .text_size(px(28.))
                            .line_height(relative(0.75))
                            .font_family(self.display_font())
                            .text_color(rgb(accent_hex))
                            .child("“"),
                    )
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_family(self.display_font())
                            .line_height(relative(1.65))
                            .text_color(rgb(text_hex))
                            .child(draft.text.chars().take(220).collect::<String>()),
                    )
                    .when_some(
                        draft
                            .note
                            .as_deref()
                            .map(str::trim)
                            .filter(|n| !n.is_empty())
                            .map(str::to_owned),
                        |this, note| {
                            this.child(
                                v_flex()
                                    .p_2p5()
                                    .gap_1()
                                    .rounded_lg()
                                    .bg(rgb(accent_hex).opacity(0.09))
                                    .border_l_2()
                                    .border_color(rgb(accent_hex))
                                    .child(
                                        div()
                                            .text_size(px(9.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(accent_hex))
                                            .child("读时批注"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .font_family(self.ui_font())
                                            .line_height(relative(1.55))
                                            .text_color(rgb(text_hex))
                                            .child(note.chars().take(120).collect::<String>()),
                                    ),
                            )
                        },
                    )
                    .child(
                        div()
                            .pt_2()
                            .border_t_1()
                            .border_color(rgb(accent_hex).opacity(0.22))
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(rgb(muted_hex))
                            .child(format!("—— {}", draft.attribution)),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(muted_hex).opacity(0.78))
                            .child("微光阅读 · GLOW READING"),
                    ),
            );
        div()
            .id("quote-card-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .bg(rgb(0x060910).opacity(0.58))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.act(Action::CloseQuoteCard, window, cx);
                }),
            )
            .child(
                v_flex()
                    .id("quote-card-dialog")
                    .w(px(740.))
                    .max_w_full()
                    .max_h_full()
                    .overflow_y_scroll()
                    .p_6()
                    .gap_5()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(0.98))
                    .border_1()
                    .border_color(self.border_strong())
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(self.studio_badge("QUOTE CARD STUDIO", "微光书摘卡", true))
                                    .child(
                                        div()
                                            .text_size(px(21.))
                                            .font_family(self.display_font())
                                            .child("把这一句做成一张书摘卡"),
                                    ),
                            )
                            .child(self.button_with_icon(
                                "关闭",
                                IconName::X,
                                Action::CloseQuoteCard,
                                cx,
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_6()
                            .items_start()
                            .flex_wrap()
                            .child(preview_card)
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w(px(240.))
                                    .gap_4()
                                    .child(
                                        v_flex()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.muted)))
                                                    .child("选择卡片版式"),
                                            )
                                            .child(style_row),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_family(self.ui_font())
                                            .line_height(relative(1.65))
                                            .text_color(rgb(hex(&t.muted)))
                                            .child("导出为 960×1240 高清 PNG 图片，100% 本地绘制、不上传任何内容。"),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_2()
                                            .child(
                                                self.button_with_icon(
                                                    "保存高清 PNG",
                                                    IconName::ImageDown,
                                                    Action::SaveQuoteCardPng,
                                                    cx,
                                                )
                                                .primary()
                                                .w_full(),
                                            )
                                            .child(
                                                self.button_with_icon(
                                                    "复制文字摘录",
                                                    IconName::Copy,
                                                    Action::CopyQuoteCardText,
                                                    cx,
                                                )
                                                .outline()
                                                .w_full(),
                                            ),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn filtered_palette_commands(
        &self,
        cx: &App,
    ) -> Vec<(&'static str, &'static str, IconName, Action)> {
        let query = self.palette_input.read(cx).value().trim().to_lowercase();
        let mut commands: Vec<(&'static str, &'static str, IconName, Action)> = vec![
            (
                "主题 · 晴空旧书房（蓝天流云）",
                "原创晴空流云与高窗书桌手绘主题",
                IconName::Cloud,
                Action::PickTheme(0),
            ),
            (
                "主题 · 夏风樟树下（林荫晴雨）",
                "原创古樟树荫与林间晴雨 · 护眼治愈绿",
                IconName::Leaf,
                Action::PickTheme(4),
            ),
            (
                "主题 · 浅海晚风号（浅海黄昏）",
                "原创浅海黄昏与远岸灯塔 · 温柔暖杏玫红",
                IconName::Sparkles,
                Action::PickTheme(5),
            ),
            (
                "主题 · 山谷秘密花园（晨雾花海）",
                "原创山谷花海与雪山微风 · 柔和丁香紫",
                IconName::Wind,
                Action::PickTheme(6),
            ),
            (
                "主题 · 海港烘焙屋（海风暖阳）",
                "原创红瓦海港与暖阳窗台 · 温柔陶土金",
                IconName::Sun,
                Action::PickTheme(7),
            ),
            (
                "主题 · 雨夜灯笼巷（暖灯夜读）",
                "原创暖灯雨窗与暮川萤火 · 深色治愈夜读",
                IconName::Flame,
                Action::PickTheme(8),
            ),
            (
                "主题 · 素纸晨光",
                "温润护眼古典纸页",
                IconName::Sun,
                Action::PickTheme(1),
            ),
            (
                "主题 · 静夜台灯",
                "低亮度柔和深色夜读",
                IconName::Moon,
                Action::PickTheme(2),
            ),
            (
                "主题 · 星空棱镜",
                "光谱流光与交互粒子",
                IconName::Sparkles,
                Action::PickTheme(3),
            ),
            (
                if self.audio.is_some() {
                    "声景 · 关闭环境音"
                } else {
                    "声景 · 开启环境音"
                },
                "切换微光原生程序化自然声景",
                if self.audio.is_some() {
                    IconName::VolumeX
                } else {
                    IconName::AudioLines
                },
                Action::Audio,
            ),
            (
                "声景 · 天空细雨",
                "柔润粉红噪声与细微雨滴",
                IconName::CloudRain,
                Action::Soundscape(SoundscapePreset::Rain),
            ),
            (
                "声景 · 壁炉微光",
                "深沉暖流与柔和木柴轻响",
                IconName::Flame,
                Action::Soundscape(SoundscapePreset::Fireplace),
            ),
            (
                "声景 · 静夜微风",
                "缓慢林风与远景夜鸣",
                IconName::Wind,
                Action::Soundscape(SoundscapePreset::Breeze),
            ),
            (
                "声景 · 黑曜星穹",
                "空灵和谐持续音与水晶风铃",
                IconName::Sparkles,
                Action::Soundscape(SoundscapePreset::Cosmos),
            ),
            (
                "声景 · 浅海电车",
                "铁轨轻律与浅海潮汐浪声",
                IconName::CloudRain,
                Action::Soundscape(SoundscapePreset::Train),
            ),
            (
                "声景 · 治愈琴韵",
                "温柔电钢轻缓五声琶音",
                IconName::AudioLines,
                Action::Soundscape(SoundscapePreset::Piano),
            ),
            (
                "声景 · 打开 6 轨多轨混音台",
                "自由叠加细雨、壁炉、浅海电车与治愈琴韵",
                IconName::Settings2,
                Action::ToggleMixer,
            ),
            (
                "AI · 打开防剧透前情提要与人物图谱",
                "仅依据当前已读章节梳理故事脉络与关系星座图",
                IconName::WandSparkles,
                Action::ToggleCompanion,
            ),
            (
                "听书 · 打开双轨合规语音听书台",
                "支持本机离线系统语音与 OpenAI 兼容神经语音服务，自动压低背景声景",
                IconName::AudioLines,
                Action::ToggleTtsDrawer,
            ),
            (
                "同步 · 立即执行多端双向同步",
                "通过 GitHub 私有仓库或同步盘无损合并进度与手账",
                IconName::RefreshCw,
                Action::RunSyncNow,
            ),
            (
                if self.breathing_started.is_some() {
                    "仪式 · 结束 4-7-8 呼吸放松"
                } else {
                    "仪式 · 启动 4-7-8 呼吸放松（1 分钟）"
                },
                "前往心情页体验呼吸引导光环",
                IconName::Wind,
                Action::ToggleBreathing,
            ),
            (
                "仪式 · 设置静读光瓶时长（支持自定义分钟）",
                "打开静读计时面板，自由设定 1—180 分钟",
                IconName::Timer,
                Action::ToggleTimerPicker,
            ),
            (
                "仪式 · 开启 25 分钟静读光瓶",
                "温和沙漏陪伴，结束时轻声风铃",
                IconName::Hourglass,
                Action::StartFocusTimer(25),
            ),
            (
                "阅读 · 切换护眼暖光滤镜",
                "柔和过滤冷蓝光",
                IconName::Sun,
                Action::ToggleWarmFilter,
            ),
            (
                "阅读 · 切换禅意专注模式",
                "隐匿边栏与工具栏，沉浸书页",
                IconName::Focus,
                Action::Focus,
            ),
            (
                "交互 · 切换流光动效 / 静谧模式",
                "全局减少动态开关",
                IconName::Sparkles,
                Action::ToggleFlow,
            ),
            (
                "导航 · 前往我的书架",
                "藏书、检索与今日重逢",
                IconName::LibraryBig,
                Action::Page(Page::Shelf),
            ),
            (
                "导航 · 前往此刻心情",
                "情绪调频与呼吸光环",
                IconName::MessageCircleHeart,
                Action::Page(Page::Mood),
            ),
            (
                "导航 · 前往深夜电台（Glow FM）",
                "暖光呼吸灯 · 复古刻度尺 · 温柔夜话与治愈电台",
                IconName::AudioLines,
                Action::Page(Page::Radio),
            ),
            (
                "电台 · 播放 / 暂停深夜电台",
                "支持切回阅读后台轻声伴听",
                IconName::Moon,
                Action::RadioTogglePlay,
            ),
            (
                "电台 · 切换纯享呼吸灯模式",
                "隐去多余控件，只留床头呼吸暖光",
                IconName::Sparkles,
                Action::RadioTogglePureGlow,
            ),
            (
                "导航 · 前往私人微光手账",
                "情绪光谱、筛选与书摘卡",
                IconName::NotebookPen,
                Action::Page(Page::Journal),
            ),
            (
                "导航 · 前往主题与设置",
                "调色、自定义素材与导入导出",
                IconName::Settings2,
                Action::Page(Page::Themes),
            ),
            (
                "手账 · 导出 Markdown 手账",
                "把本地日记导出为 .md 文件",
                IconName::Download,
                Action::ExportJournalMarkdown,
            ),
            (
                "手账 · 切换手账私密遮罩",
                "一键隐藏或解锁私人手账正文内容",
                IconName::Focus,
                Action::ToggleJournalLock,
            ),
            (
                "书房 · 打开微光公版馆",
                "内置精排公版经典与原创散文，离线一键直读",
                IconName::BookOpen,
                Action::ShelfTab(ShelfTab::Catalog),
            ),
            (
                "书房 · 连接 OPDS 1.2 书源",
                "接入 Calibre / Komga / 公版库开放书源",
                IconName::Download,
                Action::ShelfTab(ShelfTab::Opds),
            ),
            (
                "光影 · 切换昼夜自然光影随动",
                "依本地晨光/晴午/暮色/深夜自动调节暖光护眼",
                IconName::Sun,
                Action::ToggleDaylightAuto,
            ),
        ];
        if let Some(book_id) = self.active_book.clone() {
            commands.insert(
                0,
                (
                    "仪式 · 打开当前书籍的完读仪式",
                    "生成编号完读纪念票根与完读私信",
                    IconName::Sparkles,
                    Action::OpenCompletionModal(book_id),
                ),
            );
            commands.insert(
                0,
                (
                    "导航 · 回到当前阅读",
                    "继续停下的书页",
                    IconName::BookOpen,
                    Action::Page(Page::Reader),
                ),
            );
        }
        if query.is_empty() {
            commands
        } else {
            commands
                .into_iter()
                .filter(|(title, desc, _, _)| {
                    title.to_lowercase().contains(&query) || desc.to_lowercase().contains(&query)
                })
                .collect()
        }
    }
    pub(super) fn run_palette_command(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.palette_open = false;
        self.palette_selected = 0;
        window.focus(&self.focus, cx);
        if matches!(action, Action::ToggleBreathing) && self.page != Page::Mood {
            self.act(Action::Page(Page::Mood), window, cx);
        }
        self.act(action, window, cx);
    }
    pub(super) fn execute_selected_palette_command(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let commands = self.filtered_palette_commands(cx);
        if commands.is_empty() {
            return;
        }
        let idx = self.palette_selected.min(commands.len() - 1);
        let action = commands[idx].3.clone();
        self.run_palette_command(action, window, cx);
    }
    #[inline(never)]
    pub(super) fn focus_timer_modal(&self, snap: MotionSnapshot, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let dark = self.is_dark();
        let focus_status = self.focus_timer_progress();
        let vial_ratio = self.overall_reading_progress();
        let custom_raw = self.focus_timer_input.read(cx).value().trim().to_owned();
        let custom_mins = custom_raw
            .parse::<u32>()
            .ok()
            .filter(|m| (1..=180).contains(m));
        let today_mins =
            if self.store.state.reading_minutes_date.as_deref() == Some(today_ymd().as_str()) {
                self.store.state.reading_minutes_today
            } else {
                0
            };
        let active_preset = self.active_soundscape_preset();
        let mut preset_pills = h_flex().gap_2().flex_wrap();
        for &(mins, label, hint) in &[
            (10u32, "10 分钟", "小憩片刻"),
            (15, "15 分钟", "睡前几页"),
            (25, "25 分钟", "标准静读"),
            (45, "45 分钟", "深度沉浸"),
            (60, "60 分钟", "长卷慢读"),
        ] {
            let selected = custom_mins == Some(mins);
            preset_pills = preset_pills.child(
                v_flex()
                    .id(SharedString::from(format!("timer-preset-{mins}")))
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .gap_0p5()
                    .rounded_xl()
                    .bg(if selected {
                        rgb(hex(&t.accent)).opacity(0.16)
                    } else {
                        rgb(hex(&t.background)).opacity(if dark { 0.62 } else { 0.72 })
                    })
                    .border_1()
                    .border_color(if selected {
                        Hsla::from(rgb(hex(&t.accent)).opacity(0.70))
                    } else {
                        self.border_subtle()
                    })
                    .hover(|s| {
                        s.bg(rgb(hex(&t.accent)).opacity(0.18))
                            .border_color(rgb(hex(&t.accent)).opacity(0.65))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::StartFocusTimer(mins), window, cx);
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_family(self.ui_font())
                            .text_color(if selected {
                                rgb(hex(&t.accent))
                            } else {
                                rgb(hex(&t.text))
                            })
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child(hint),
                    ),
            );
        }
        let cta_label = if let Some(m) = custom_mins {
            if focus_status.is_some() {
                format!("重新设定为 {m} 分钟静读")
            } else {
                format!("开始 {m} 分钟静读")
            }
        } else {
            "开始自定义静读".to_owned()
        };
        div()
            .id("focus-timer-overlay")
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
                    this.act(Action::CloseTimerPicker, window, cx);
                }),
            )
            .child(
                v_flex()
                    .id("focus-timer-dialog")
                    .debug_selector(|| "focus-timer-dialog".into())
                    .w(px(500.))
                    .max_w_full()
                    .p_6()
                    .gap_5()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(0.98))
                    .border_1()
                    .border_color(self.border_strong())
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        h_flex()
                            .justify_between()
                            .items_start()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(self.studio_badge(
                                        "QUIET READING TIMER",
                                        "静读光瓶 · 自定义时长",
                                        true,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(21.))
                                            .font_family(self.display_font())
                                            .child("为这一段阅读，留一点安静时间"),
                                    ),
                            )
                            .child(self.button_with_icon(
                                "关闭",
                                IconName::X,
                                Action::CloseTimerPicker,
                                cx,
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .justify_between()
                            .p_4()
                            .rounded_xl()
                            .bg(rgb(hex(&t.background)).opacity(if dark { 0.65 } else { 0.72 }))
                            .border_1()
                            .border_color(self.border_subtle())
                            .child(
                                h_flex()
                                    .gap_3p5()
                                    .items_center()
                                    .child(prism::light_vial_canvas(snap, vial_ratio))
                                    .child(
                                        v_flex()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_size(px(19.))
                                                    .font_family(self.display_font())
                                                    .text_color(rgb(hex(&t.text)))
                                                    .child(if let Some((_, rem)) = focus_status {
                                                        format!(
                                                            "正在静读 · 剩余 {:02}:{:02}",
                                                            rem / 60,
                                                            rem % 60
                                                        )
                                                    } else if let Some(m) = custom_mins {
                                                        format!("准备静读 · {m} 分钟")
                                                    } else {
                                                        "输入 1—180 分钟".to_owned()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.muted)))
                                                    .child("结束时以轻柔三音风铃提醒，并自动计入今日阅读时长"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_full()
                                    .bg(rgb(hex(&t.accent)).opacity(0.14))
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child(format!("今日已读 {today_mins} 分钟")),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("快捷时长（点击即刻开始）"),
                            )
                            .child(preset_pills),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("自定义时长（支持自由输入 1—180 分钟，按回车直接开始）"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(self.button("−5", Action::StepCustomTimer(-5), cx).outline())
                                    .child(self.button("−1", Action::StepCustomTimer(-1), cx).outline())
                                    .child(div().w(px(110.)).child(Input::new(&self.focus_timer_input)))
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.muted)))
                                            .child("分钟"),
                                    )
                                    .child(self.button("+1", Action::StepCustomTimer(1), cx).outline())
                                    .child(self.button("+5", Action::StepCustomTimer(5), cx).outline()),
                            ),
                    )
                    .when(self.audio.is_none(), |this| {
                        this.child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .child(self.pill_button(
                                    &format!(
                                        "开始静读时同步开启「{}」环境声",
                                        active_preset.label()
                                    ),
                                    Some(IconName::AudioLines),
                                    self.focus_timer_with_audio,
                                    Action::ToggleTimerWithAudio,
                                    cx,
                                )),
                        )
                    })
                    .when_some(self.error.clone(), |this, err| {
                        this.child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(rgb(0xb34f36).opacity(0.12))
                                .text_size(px(11.5))
                                .font_family(self.ui_font())
                                .text_color(rgb(0xb34f36))
                                .child(err),
                        )
                    })
                    .child(
                        h_flex()
                            .gap_2p5()
                            .justify_end()
                            .items_center()
                            .when(focus_status.is_some(), |this| {
                                this.child(
                                    self.button_with_icon(
                                        "结束当前静读",
                                        IconName::X,
                                        Action::StopFocusTimer,
                                        cx,
                                    )
                                    .outline(),
                                )
                            })
                            .child(
                                self.button_with_icon(
                                    &cta_label,
                                    IconName::Hourglass,
                                    Action::StartCustomFocusTimer,
                                    cx,
                                )
                                .primary(),
                            ),
                    ),
            )
            .into_any_element()
    }
    #[inline(never)]
    pub(super) fn command_palette_modal(
        &self,
        snap: MotionSnapshot,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = self.theme();
        let commands = self.filtered_palette_commands(cx);
        let count = commands.len();
        let selected_idx = if count == 0 {
            0
        } else {
            self.palette_selected.min(count - 1)
        };
        let mut list = v_flex()
            .id("palette-list")
            .overflow_y_scroll()
            .max_h(px(360.))
            .gap_1();
        for (idx, (title, desc, icon, action)) in commands.into_iter().enumerate() {
            let is_selected = idx == selected_idx;
            let action_clone = action.clone();
            list = list.child(
                div()
                    .id(SharedString::from(format!("cmd-{}", idx + 1)))
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_xl()
                    .bg(if is_selected {
                        rgb(hex(&t.accent)).opacity(0.16)
                    } else {
                        rgb(0x000000).opacity(0.0)
                    })
                    .border_1()
                    .border_color(if is_selected {
                        rgb(hex(&t.accent)).opacity(0.45)
                    } else {
                        rgb(0x000000).opacity(0.0)
                    })
                    .hover(|s| s.bg(rgb(hex(&t.accent)).opacity(0.18)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.run_palette_command(action_clone.clone(), window, cx);
                        }),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(
                                        Icon::new(icon)
                                            .size(px(15.))
                                            .text_color(rgb(hex(&t.accent))),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.text)))
                                                    .child(title),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(10.5))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.muted)))
                                                    .child(desc),
                                            ),
                                    ),
                            )
                            .when(is_selected, |this| {
                                this.child(
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded(px(5.))
                                        .bg(rgb(hex(&t.accent)).opacity(0.18))
                                        .text_size(px(9.5))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.accent)))
                                        .child("Enter"),
                                )
                            }),
                    ),
            );
        }
        if count == 0 {
            list = list.child(
                div()
                    .p_5()
                    .text_size(px(12.5))
                    .font_family(self.ui_font())
                    .text_color(rgb(hex(&t.muted)))
                    .child("没有匹配的快捷指令，试试搜“主题”、“声景”、“呼吸”或“静读”。"),
            );
        }
        div()
            .id("palette-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(84.))
            .px_6()
            .bg(rgb(0x060910).opacity(0.52))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.act(Action::ClosePalette, window, cx);
                }),
            )
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let len = this.filtered_palette_commands(cx).len();
                if len == 0 {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "down" => {
                        this.palette_selected = (this.palette_selected + 1) % len;
                        cx.stop_propagation();
                        cx.notify();
                    }
                    "up" => {
                        this.palette_selected = if this.palette_selected == 0 {
                            len - 1
                        } else {
                            this.palette_selected - 1
                        };
                        cx.stop_propagation();
                        cx.notify();
                    }
                    _ => {}
                }
            }))
            .child(
                v_flex()
                    .id("palette-dialog")
                    .relative()
                    .overflow_hidden()
                    .w(px(560.))
                    .max_w_full()
                    .p_4()
                    .gap_3()
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
                            .child(prism::prism_beam_horizontal(snap, 2.0, 0.5, false)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::Command)
                                    .size(px(15.))
                                    .text_color(rgb(hex(&t.accent))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.palette_input).w_full()),
                            )
                            .child(self.button_with_icon(
                                "关闭",
                                IconName::X,
                                Action::ClosePalette,
                                cx,
                            )),
                    )
                    .child(list)
                    .child(
                        div()
                            .pt_1()
                            .border_t_1()
                            .border_color(self.border_subtle())
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child("↑ / ↓ 选择 · Enter 执行 · Esc 或点击空白处关闭"),
                    ),
            )
            .into_any_element()
    }

    #[inline(never)]
    pub(super) fn completion_ceremony_modal(
        &self,
        book_id: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = self.theme();
        let is_dark = self.is_dark();
        let book = self.store.state.books.iter().find(|b| b.id == book_id);
        let title = book.map(|b| b.title.as_str()).unwrap_or("微光藏书");
        let author = book.map(|b| b.author.as_str()).unwrap_or("佚名");
        let minutes = book.map(|b| b.total_reading_minutes).unwrap_or(0);
        let ticket_no = book
            .and_then(|b| b.completion_ticket_no)
            .unwrap_or(self.store.state.next_ticket_no.max(1));
        let already_archived = book.and_then(|b| b.completion_ticket_no).is_some();
        let best_quote = book
            .and_then(|b| b.highlights.last())
            .map(|h| h.text.chars().take(84).collect::<String>());
        let date = today_ymd();

        div()
            .id("completion-ceremony-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .bg(rgb(0x0b101b).opacity(0.62))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.act(Action::CloseCompletionModal, window, cx);
                }),
            )
            .child(
                v_flex()
                    .id("completion-ceremony-card")
                    .w(px(580.))
                    .max_w_full()
                    .p_7()
                    .gap_5()
                    .rounded_2xl()
                    .bg(rgb(hex(&t.paper)).opacity(0.98))
                    .border_1()
                    .border_color(rgb(hex(&t.accent)).opacity(0.45))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(self.studio_badge(
                                &format!("TICKET NO.{ticket_no:04}"),
                                self.tr("合上书页 · 完读纪念仪式", "CLOSE THE COVER · COMPLETION RITUAL"),
                                true,
                            ))
                            .child(self.button_with_icon(
                                self.tr("收起", "Close"),
                                IconName::X,
                                Action::CloseCompletionModal,
                                cx,
                            )),
                    )
                    // Vintage ticket stub preview card
                    .child(
                        v_flex()
                            .p_5()
                            .gap_3()
                            .rounded_2xl()
                            .bg(rgb(hex(&t.background)).opacity(if is_dark { 0.72 } else { 0.80 }))
                            .border_1()
                            .border_color(rgb(hex(&t.accent)).opacity(0.32))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_start()
                                    .gap_3()
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(px(23.))
                                                    .font_family(self.display_font())
                                                    .child(if self.locale().is_english() {
                                                        title.to_owned()
                                                    } else {
                                                        format!("《{title}》")
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.muted)))
                                                    .child(if self.locale().is_english() {
                                                        if minutes > 0 {
                                                            format!(
                                                                "{author}  ·  Finished {date}  ·  {minutes} min read"
                                                            )
                                                        } else {
                                                            format!("{author}  ·  Finished {date}")
                                                        }
                                                    } else if minutes > 0 {
                                                        format!(
                                                            "{author}  ·  完读于 {date}  ·  共静读 {minutes} 分钟"
                                                        )
                                                    } else {
                                                        format!("{author}  ·  完读于 {date}")
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_2p5()
                                            .py_1p5()
                                            .rounded_lg()
                                            .bg(rgb(0xb84338).opacity(0.92))
                                            .border_1()
                                            .border_color(rgb(0xfff4ec).opacity(0.55))
                                            .text_size(px(12.))
                                            .font_family(self.display_font())
                                            .text_color(rgb(0xfff8f2))
                                            .child(self.tr("完读", "DONE")),
                                    ),
                            )
                            .when_some(best_quote, |this, q| {
                                this.child(
                                    div()
                                        .p_3()
                                        .rounded_xl()
                                        .bg(rgb(hex(&t.paper)).opacity(0.75))
                                        .border_l_3()
                                        .border_color(rgb(hex(&t.accent)))
                                        .text_size(px(13.))
                                        .font_family(self.display_font())
                                        .line_height(relative(1.55))
                                        .child(if self.locale().is_english() {
                                            format!("“{q}”")
                                        } else {
                                            format!("「{q}」")
                                        }),
                                )
                            })
                            .child(
                                div()
                                    .h(px(1.))
                                    .w_full()
                                    .bg(rgb(hex(&t.accent)).opacity(0.24)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .font_family(self.ui_font())
                                    .line_height(relative(1.75))
                                    .text_color(rgb(hex(&t.text)).opacity(0.92))
                                    .child(self.completion_draft_letter.clone()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .flex_wrap()
                            .child(
                                self.button_with_icon(
                                    if already_archived {
                                        self.tr("更新手账中的完读票根", "Update Archived Ticket")
                                    } else {
                                        self.tr("珍藏票根至手账", "Archive Ticket to Journal")
                                    },
                                    IconName::Bookmark,
                                    Action::ConfirmCompleteBook,
                                    cx,
                                )
                                .primary(),
                            )
                            .child(self.button_with_icon(
                                self.tr("保存高清票根 PNG", "Save Ticket PNG"),
                                IconName::ImageDown,
                                Action::SaveCompletionTicketPng,
                                cx,
                            ))
                            .child(
                                self.button_with_icon(
                                    if self.completion_letter_rx.is_some() {
                                        self.tr("正在写完读信…", "Writing Letter…")
                                    } else {
                                        self.tr("AI 润色完读信", "Polish with AI")
                                    },
                                    IconName::WandSparkles,
                                    Action::GenerateAiCompletionLetter,
                                    cx,
                                )
                                .disabled(self.completion_letter_rx.is_some()),
                            ),
                    ),
            )
            .into_any_element()
    }

    #[inline(never)]
    pub(super) fn theme_preview_modal(
        &self,
        preview: &glow_core::theme::ThemePackagePreview,
        cx: &Context<Self>,
    ) -> AnyElement {
        let active = self.theme();
        let pt = &preview.theme;
        let size_kb = preview.archive_bytes.div_ceil(1024);
        let contrast_grade = if preview.text_contrast >= 7.0 {
            self.tr("WCAG AAA 护眼级", "WCAG AAA Comfort")
        } else {
            self.tr("WCAG AA 达标", "WCAG AA Compliant")
        };

        let mut asset_badges = h_flex().gap_1p5().flex_wrap().items_center();
        if preview.has_image {
            asset_badges = asset_badges.child(
                div()
                    .px_2p5()
                    .py_0p5()
                    .rounded_full()
                    .bg(rgb(hex(&active.accent)).opacity(0.14))
                    .text_size(px(10.))
                    .font_family(self.ui_font())
                    .text_color(rgb(hex(&active.accent)))
                    .child(self.tr("含背景图片", "Includes Backdrop")),
            );
        }
        if preview.has_font {
            asset_badges = asset_badges.child(
                div()
                    .px_2p5()
                    .py_0p5()
                    .rounded_full()
                    .bg(rgb(hex(&active.accent)).opacity(0.14))
                    .text_size(px(10.))
                    .font_family(self.ui_font())
                    .text_color(rgb(hex(&active.accent)))
                    .child(self.tr("含定制字体", "Includes Custom Font")),
            );
        }
        if preview.has_audio {
            asset_badges = asset_badges.child(
                div()
                    .px_2p5()
                    .py_0p5()
                    .rounded_full()
                    .bg(rgb(hex(&active.accent)).opacity(0.14))
                    .text_size(px(10.))
                    .font_family(self.ui_font())
                    .text_color(rgb(hex(&active.accent)))
                    .child(self.tr("含环境音素材", "Includes Soundscape")),
            );
        }
        if !preview.has_image && !preview.has_font && !preview.has_audio {
            asset_badges = asset_badges.child(
                div()
                    .px_2p5()
                    .py_0p5()
                    .rounded_full()
                    .bg(rgb(hex(&active.background)).opacity(0.75))
                    .text_size(px(10.))
                    .font_family(self.ui_font())
                    .text_color(rgb(hex(&active.muted)))
                    .child(self.tr("纯配色与排版主题包", "Palette & Typography Only")),
            );
        }

        let mut swatch_row = h_flex().gap_2().flex_wrap().items_center();
        for (label_zh, label_en, value) in [
            ("底色", "BG", &pt.background),
            ("纸面", "Paper", &pt.paper),
            ("正文", "Ink", &pt.text),
            ("次要", "Muted", &pt.muted),
            ("强调", "Accent", &pt.accent),
        ] {
            let label = self.tr(label_zh, label_en);
            swatch_row = swatch_row.child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_lg()
                    .bg(rgb(hex(&active.background)).opacity(0.66))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        div()
                            .size(px(11.))
                            .rounded_full()
                            .bg(rgb(hex(value)))
                            .border_1()
                            .border_color(self.border_strong()),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&active.muted)))
                            .child(format!("{label} {value}")),
                    ),
            );
        }

        div()
            .id("theme-preview-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .bg(rgb(0x060910).opacity(0.58))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.act(Action::CancelImportThemePackage, window, cx);
                }),
            )
            .child(
                v_flex()
                    .id("theme-preview-dialog")
                    .debug_selector(|| "theme-preview-dialog".into())
                    .w(px(560.))
                    .max_w_full()
                    .p_6()
                    .gap_4()
                    .rounded_2xl()
                    .bg(rgb(hex(&active.paper)).opacity(0.98))
                    .border_1()
                    .border_color(rgb(hex(&active.accent)).opacity(0.45))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(self.studio_badge(
                                "THEME INSPECT",
                                self.tr("主题包导入预检", "THEME PACKAGE PREVIEW"),
                                true,
                            ))
                            .child(self.button_with_icon(
                                self.tr("取消", "Cancel"),
                                IconName::X,
                                Action::CancelImportThemePackage,
                                cx,
                            )),
                    )
                    // Miniature live reading room mockup styled in the incoming theme's exact colors
                    .child(
                        v_flex()
                            .p_4()
                            .gap_3()
                            .rounded_2xl()
                            .bg(rgb(hex(&pt.background)))
                            .border_1()
                            .border_color(rgb(hex(&pt.text)).opacity(0.16))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&pt.muted)))
                                            .child(if self.locale().is_english() {
                                                format!("Preview Space · {}", pt.name)
                                            } else {
                                                format!("预览空间 · {}", pt.name)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(rgb(hex(&pt.accent)))
                                            .text_size(px(9.5))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&pt.paper)))
                                            .child(if self.locale().is_english() {
                                                format!("Size {:.0}px", pt.font_size)
                                            } else {
                                                format!("字号 {:.0}px", pt.font_size)
                                            }),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .p_4()
                                    .gap_2()
                                    .rounded_xl()
                                    .bg(rgb(hex(&pt.paper)))
                                    .border_1()
                                    .border_color(rgb(hex(&pt.text)).opacity(0.10))
                                    .child(div().w(px(36.)).h(px(2.5)).rounded_full().bg(rgb(hex(&pt.accent))))
                                    .child(
                                        div()
                                            .text_size(px(16.))
                                            .font_family(self.display_font())
                                            .line_height(relative(1.65))
                                            .text_color(rgb(hex(&pt.text)))
                                            .child(self.tr(
                                                "山风吹过微黄纸页，把白日的喧嚣轻轻搁在窗外。",
                                                "Evening breeze across warm paper, leaving the noise of the day outside.",
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&pt.muted)))
                                            .child(if self.locale().is_english() {
                                                format!(
                                                    "Font: {}  ·  Line {:.1}  ·  Width {}px",
                                                    pt.font.split(',').next().unwrap_or("Serif").trim(),
                                                    pt.line_height,
                                                    pt.width
                                                )
                                            } else {
                                                format!(
                                                    "字体：{}  ·  行距 {:.1}  ·  正文宽 {}px",
                                                    pt.font.split(',').next().unwrap_or("衬线体").trim(),
                                                    pt.line_height,
                                                    pt.width
                                                )
                                            }),
                                    ),
                            ),
                    )
                    .child(swatch_row)
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .flex_wrap()
                            .gap_2()
                            .p_3()
                            .rounded_xl()
                            .bg(rgb(hex(&active.background)).opacity(0.55))
                            .border_1()
                            .border_color(self.border_subtle())
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Icon::new(IconName::ShieldCheck)
                                            .size(px(14.))
                                            .text_color(rgb(hex(&active.accent))),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&active.text)))
                                            .child(if self.locale().is_english() {
                                                format!(
                                                    "Text contrast {:.1}:1 ({contrast_grade}) · Accent {:.1}:1",
                                                    preview.text_contrast, preview.accent_contrast
                                                )
                                            } else {
                                                format!(
                                                    "正文对比度 {:.1}:1（{contrast_grade}） · 强调对比度 {:.1}:1",
                                                    preview.text_contrast, preview.accent_contrast
                                                )
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&active.muted)))
                                    .child(if self.locale().is_english() {
                                        format!("Package ~{size_kb} KB")
                                    } else {
                                        format!("包体约 {size_kb} KB")
                                    }),
                            ),
                    )
                    .child(asset_badges)
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&active.muted)))
                            .child(self.tr(
                                "已通过安全清单与对比度校验，导入后将保存至「我的私藏主题」，不会影响你的藏书和日记。",
                                "Passed manifest safety & contrast checks. Importing adds it to Personal Themes without altering your books or journal.",
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_2p5()
                            .items_center()
                            .child(
                                self.button_with_icon(
                                    self.tr("确认导入并启用", "Import & Apply"),
                                    IconName::Check,
                                    Action::ConfirmImportThemePackage,
                                    cx,
                                )
                                .primary(),
                            )
                            .child(self.button(
                                self.tr("暂不导入", "Cancel"),
                                Action::CancelImportThemePackage,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }
}

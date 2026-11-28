use super::*;

impl ReadingApp {
    pub(super) fn theme_card(
        &self,
        i: usize,
        t: &ReadingTheme,
        active_theme: &ReadingTheme,
        is_dark: bool,
        is_custom: bool,
        cx: &Context<Self>,
    ) -> Div {
        let active = active_theme.id == t.id;
        let (subtitle, poster_badge, mood_note) = Self::theme_card_meta(&t.id);
        let is_scenic_card = Self::is_scenic_space_id(&t.id);
        let is_prism_card = t.id == "prism";
        let swatches = h_flex()
            .gap_1()
            .items_center()
            .px_2()
            .py_1()
            .rounded_full()
            .bg(rgb(0x0b1512).opacity(0.66))
            .child(
                div()
                    .size(px(9.))
                    .rounded_full()
                    .bg(rgb(hex(&t.background)))
                    .border_1()
                    .border_color(rgb(0xffffff).opacity(0.35)),
            )
            .child(
                div()
                    .size(px(9.))
                    .rounded_full()
                    .bg(rgb(hex(&t.paper)))
                    .border_1()
                    .border_color(rgb(0xffffff).opacity(0.35)),
            )
            .child(
                div()
                    .size(px(9.))
                    .rounded_full()
                    .bg(rgb(hex(&t.accent)))
                    .border_1()
                    .border_color(rgb(0xffffff).opacity(0.35)),
            )
            .child(
                div()
                    .size(px(9.))
                    .rounded_full()
                    .bg(rgb(hex(&t.text)))
                    .border_1()
                    .border_color(rgb(0xffffff).opacity(0.35)),
            );
        let snap = self.motion_snapshot();
        let preview_h = px(152.);
        let preview_inner = if let Some(scene) = Self::scene_asset_for(&t.id) {
            img(scene)
                .w_full()
                .h(preview_h)
                .object_fit(ObjectFit::Cover)
                .into_any_element()
        } else if let Some(image) = &t.image {
            img(self.store.root.join("themes").join(&t.id).join(image))
                .w_full()
                .h(preview_h)
                .object_fit(ObjectFit::Cover)
                .into_any_element()
        } else {
            v_flex()
                .w_full()
                .h(preview_h)
                .p_4()
                .justify_between()
                .bg(rgb(hex(&t.background)))
                .child(
                    h_flex()
                        .gap_1p5()
                        .items_center()
                        .child(
                            Icon::new(IconName::Sun)
                                .size(px(15.))
                                .text_color(rgb(hex(&t.accent))),
                        )
                        .child(
                            div()
                                .text_size(px(9.))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.muted)))
                                .child("EDITORIAL PAPER"),
                        ),
                )
                .child(
                    v_flex()
                        .gap_1p5()
                        .child(div().w(px(34.)).h(px(2.)).bg(rgb(hex(&t.accent))))
                        .child(
                            div()
                                .text_size(px(15.5))
                                .font_family(self.display_font())
                                .line_height(relative(1.48))
                                .text_color(rgb(hex(&t.text)))
                                .child("风吹过微黄纸页，\n把心绪轻轻放下。"),
                        ),
                )
                .into_any_element()
        };
        let badge_bg = if is_prism_card {
            rgb(0x090a0f).opacity(0.82)
        } else if t.id == "lantern" {
            rgb(0x1a1322).opacity(0.82)
        } else {
            rgb(0x132028).opacity(0.72)
        };
        let badge_fg = if is_prism_card {
            rgb(0xe9d5ff)
        } else if t.id == "lantern" {
            rgb(0xfde2a6)
        } else {
            rgb(0xfffdf7)
        };
        let badge_border = if is_prism_card {
            rgb(0xc4b5fd).opacity(0.42)
        } else if is_scenic_card {
            rgb(0xffffff).opacity(0.28)
        } else {
            rgb(0xffffff).opacity(0.20)
        };
        let card = v_flex()
            .relative()
            .overflow_hidden()
            .w(px(292.))
            .p_3p5()
            .gap_3()
            .rounded_2xl()
            .bg(rgb(hex(&active_theme.paper)).opacity(if is_dark { 0.90 } else { 0.94 }))
            .border_1()
            .border_color(if active {
                Hsla::from(rgb(hex(&active_theme.accent)).opacity(0.75))
            } else {
                self.border_subtle()
            })
            .shadow_sm()
            .hover(|style| {
                style
                    .border_color(rgb(hex(&active_theme.accent)).opacity(0.62))
                    .shadow_md()
            })
            .when(is_prism_card || active, |this| {
                this.child(div().absolute().top_0().left_0().right_0().child(
                    prism::prism_beam_horizontal(snap.clone(), 2.0, i as f32 * 0.9, is_prism_card),
                ))
            })
            .child(
                div()
                    .relative()
                    .rounded_xl()
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(hex(&t.text)).opacity(0.12))
                    .child(preview_inner)
                    .child(div().absolute().right(px(8.)).top(px(8.)).child(swatches))
                    .when_some(poster_badge, |this, badge| {
                        this.child(
                            div()
                                .absolute()
                                .left(px(8.))
                                .bottom(px(8.))
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(badge_bg)
                                .border_1()
                                .border_color(badge_border)
                                .text_size(px(9.))
                                .font_family(self.ui_font())
                                .text_color(badge_fg)
                                .child(badge),
                        )
                    })
                    .when(is_prism_card, |this| {
                        this.child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_0()
                                .right_0()
                                .child(prism::prism_beam_horizontal(snap, 2.0, 1.2, true)),
                        )
                    }),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_size(px(16.5))
                                            .font_family(self.display_font())
                                            .text_color(rgb(hex(&active_theme.text)))
                                            .child(t.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(9.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&active_theme.muted)))
                                            .child(subtitle),
                                    ),
                            )
                            .when(active, |style| {
                                style.child(
                                    h_flex()
                                        .gap_1()
                                        .items_center()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(rgb(hex(&active_theme.accent)).opacity(0.14))
                                        .text_size(px(9.5))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&active_theme.accent)))
                                        .child(
                                            Icon::new(IconName::Check)
                                                .size(px(10.))
                                                .text_color(rgb(hex(&active_theme.accent))),
                                        )
                                        .child("使用中"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&active_theme.muted)))
                            .child(mood_note),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(if active {
                        self.button_with_icon("正在使用", IconName::Check, Action::PickTheme(i), cx)
                            .primary()
                            .flex_1()
                    } else {
                        self.button_with_icon(
                            "使用此主题",
                            IconName::Sparkles,
                            Action::PickTheme(i),
                            cx,
                        )
                        .outline()
                        .flex_1()
                    })
                    .when(is_custom, |row| {
                        row.child(self.button_with_icon(
                            "删除",
                            IconName::X,
                            Action::DeleteCustomTheme(t.id.clone()),
                            cx,
                        ))
                    }),
            );
        if active {
            card.border_2()
                .border_color(rgb(hex(&t.accent)))
                .shadow_md()
        } else {
            card
        }
    }
    pub(super) fn theme_page(&self, cx: &Context<Self>) -> AnyElement {
        let active_theme = self.theme();
        let is_dark = self.is_dark();
        let all = self.all_themes();
        let mut view = v_flex()
            .id("themes-scroll")
            .overflow_y_scroll()
            .size_full()
            .px_7()
            .py_6()
            .gap_5()
            .child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .gap_6()
                    .child(self.heading(
                        self.tr(
                            "MAKE YOURSELF AT HOME · 空间与声景调音台",
                            "MAKE YOURSELF AT HOME · SPACE & STUDIO SETTINGS",
                        ),
                        self.tr("把这里，变成喜欢的样子。", "Make This Reading Room Yours."),
                        self.tr(
                            "从原创手绘水彩般的温柔光影，到素纸晨光与星空棱镜；主题包不包含你的书和日记。",
                            "Original hand-painted Glow Spaces, editorial paper, and dark sanctuaries. Theme packages never include your books or journal.",
                        ),
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(self.button_with_icon(
                                self.tr("导入主题包", "Import Theme"),
                                IconName::Plus,
                                Action::ImportTheme,
                                cx,
                            ))
                            .child(self.button_with_icon(
                                self.tr("导出当前主题", "Export Theme"),
                                IconName::ArrowUpRight,
                                Action::ExportTheme,
                                cx,
                            )),
                    ),
            );
        let mut scenic_row = h_flex().gap_4().flex_wrap();
        let mut classic_row = h_flex().gap_4().flex_wrap();
        let mut custom_row = h_flex().gap_4().flex_wrap();
        let mut custom_count = 0usize;
        for (i, t) in all.iter().enumerate() {
            let is_scenic = Self::is_scenic_space_id(&t.id);
            let is_classic = matches!(t.id.as_str(), "paper" | "night" | "prism");
            let is_custom = !is_scenic && !is_classic;
            let card = self.theme_card(i, t, active_theme, is_dark, is_custom, cx);
            if is_scenic {
                scenic_row = scenic_row.child(card);
            } else if is_classic {
                classic_row = classic_row.child(card);
            } else {
                custom_count += 1;
                custom_row = custom_row.child(card);
            }
        }
        view = view
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_end()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&active_theme.accent)))
                                            .child("GLOW SPACE GALLERY"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(18.5))
                                            .font_family(self.display_font())
                                            .text_color(rgb(hex(&active_theme.text)))
                                            .child(self.tr("微光空间画廊", "Glow Space Gallery")),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&active_theme.muted)))
                                    .child(self.tr(
                                        "晴空旧书房、夏风樟树下、浅海晚风号、山谷秘密花园、海港烘焙屋与雨夜灯笼巷 · 均满足高对比度护眼阅读",
                                        "Sky Study, Camphor Shade, Twilight Sea, Meadow Breeze, Harbor Bakery & Lantern Rain · All WCAG AAA calibrated",
                                    )),
                            ),
                    )
                    .child(scenic_row),
            )
            .child(
                v_flex()
                    .gap_3()
                    .pt_1()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&active_theme.accent)))
                                    .child("CLASSIC & AMBIENT"),
                            )
                            .child(
                                div()
                                    .text_size(px(17.5))
                                    .font_family(self.display_font())
                                    .text_color(rgb(hex(&active_theme.text)))
                                    .child(self.tr("经典与光影空间", "Classic & Night Spaces")),
                            ),
                    )
                    .child(classic_row),
            );
        if custom_count > 0 {
            view = view.child(
                v_flex()
                    .gap_3()
                    .pt_1()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&active_theme.accent)))
                                    .child("PERSONAL THEMES"),
                            )
                            .child(
                                div()
                                    .text_size(px(17.5))
                                    .font_family(self.display_font())
                                    .text_color(rgb(hex(&active_theme.text)))
                                    .child(self.tr("我的私藏主题", "My Custom Themes")),
                            ),
                    )
                    .child(custom_row),
            );
        }
        view = view
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .flex_wrap()
                    .gap_3()
                    .p_4()
                    .rounded_2xl()
                    .bg(rgb(hex(&active_theme.paper)).opacity(if is_dark { 0.88 } else { 0.92 }))
                    .border_1()
                    .border_color(self.border_subtle())
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .flex_wrap()
                            .child(
                                self.button_with_icon(
                                    self.tr("自定义 / 另存主题", "Customize Theme"),
                                    IconName::Feather,
                                    Action::EditTheme,
                                    cx,
                                )
                                .primary(),
                            )
                            .child(
                                self.button_with_icon(
                                    if active_theme.reduced_motion {
                                        self.tr("流光交互：已静谧", "Motion: Calm")
                                    } else {
                                        self.tr("流光交互：已开启", "Motion: Ambient")
                                    },
                                    IconName::Sparkles,
                                    Action::ToggleFlow,
                                    cx,
                                )
                                .outline(),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(self.pill_button(
                                        "简体中文",
                                        None,
                                        !self.locale().is_english(),
                                        Action::SetLocale("zh-CN".into()),
                                        cx,
                                    ))
                                    .child(self.pill_button(
                                        "English",
                                        None,
                                        self.locale().is_english(),
                                        Action::SetLocale("en-US".into()),
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&active_theme.muted)))
                                    .child(self.tr(
                                        "支持鼠标流光、棱镜折射、自定义素材与中英双语界面",
                                        "Ambient prism refraction, custom assets, and live ZH / EN interface",
                                    )),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .px_3()
                            .py_1p5()
                            .rounded_full()
                            .bg(rgb(hex(&active_theme.accent)).opacity(0.12))
                            .child(
                                Icon::new(IconName::ShieldCheck)
                                    .size(px(14.))
                                    .text_color(rgb(hex(&active_theme.accent))),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&active_theme.accent)))
                                    .child({
                                        let ratio = glow_core::theme::contrast(
                                            &active_theme.text,
                                            &active_theme.paper,
                                        );
                                        if self.locale().is_english() {
                                            format!("Text contrast {ratio:.1}:1 · Min 4.5:1")
                                        } else {
                                            format!("当前正文对比度 {ratio:.1}:1 · 最低要求 4.5:1")
                                        }
                                    }),
                            ),
                    ),
            )
            .child(self.settings_ai_card(cx))
            .child(self.settings_tts_card(cx))
            .child(self.settings_sync_card(cx));
        if let Some(draft) = &self.draft {
            let pick_hex = |idx: usize, fallback: &str| -> String {
                let raw = self.fields[idx].read(cx).value().trim().to_owned();
                if glow_core::theme::color(&raw).is_some() {
                    raw
                } else {
                    fallback.to_owned()
                }
            };
            let live_name = {
                let raw = self.fields[0].read(cx).value().trim().to_owned();
                if raw.is_empty() {
                    draft.name.clone()
                } else {
                    raw
                }
            };
            let live_bg = pick_hex(1, &draft.background);
            let live_paper = pick_hex(2, &draft.paper);
            let live_text = pick_hex(3, &draft.text);
            let live_muted = pick_hex(4, &draft.muted);
            let live_accent = pick_hex(5, &draft.accent);
            let live_contrast = glow_core::theme::contrast(&live_text, &live_paper);
            let live_pass = live_contrast >= 4.5;

            let labels: &[(&str, &str)] = &[
                ("主题名称", "Theme Name"),
                ("界面底色 #RRGGBB", "Backdrop #RRGGBB"),
                ("纸面颜色 #RRGGBB", "Paper #RRGGBB"),
                ("正文颜色 #RRGGBB", "Ink #RRGGBB"),
                ("次要文字 #RRGGBB", "Muted #RRGGBB"),
                ("强调色 #RRGGBB", "Accent #RRGGBB"),
                ("正文字体", "Body Font"),
                ("字号 14—36", "Font Size 14—36"),
                ("行距 1.3—2.6", "Line Height 1.3—2.6"),
                ("正文宽度 400—1100", "Measure 400—1100"),
                ("背景遮罩 0—1", "Backdrop Scrim 0—1"),
                ("环境音量 0—1", "Ambient Volume 0—1"),
            ];
            let mut fields = h_flex().flex_wrap().gap_4();
            for (i, &(label_zh, label_en)) in labels.iter().enumerate() {
                fields = fields.child(
                    v_flex()
                        .w(px(212.))
                        .gap_1p5()
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&active_theme.muted)))
                                .child(self.tr(label_zh, label_en)),
                        )
                        .child(Input::new(&self.fields[i])),
                );
            }
            view = view.child(
                v_flex()
                    .p_5()
                    .gap_5()
                    .rounded_2xl()
                    .bg(rgb(hex(&active_theme.paper)).opacity(0.94))
                    .border_1()
                    .border_color(self.border_subtle())
                    .shadow_sm()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .flex_wrap()
                            .gap_3()
                            .child(
                                v_flex()
                                    .gap_1p5()
                                    .child(self.studio_badge("DIY CONSOLE", "MAKE IT YOURS", true))
                                    .child(
                                        div()
                                            .text_size(px(21.))
                                            .font_family(self.display_font())
                                            .child(self.tr(
                                                "编辑这一个阅读空间",
                                                "Customize This Reading Space",
                                            )),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_full()
                                    .bg(if live_pass {
                                        rgb(hex(&active_theme.accent)).opacity(0.14)
                                    } else {
                                        rgb(0xb34f36).opacity(0.16)
                                    })
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(if live_pass {
                                        rgb(hex(&active_theme.accent))
                                    } else {
                                        rgb(0xb34f36)
                                    })
                                    .child(if self.locale().is_english() {
                                        if live_contrast >= 7.0 {
                                            format!("Live contrast {live_contrast:.1}:1 · AAA Comfort")
                                        } else if live_pass {
                                            format!("Live contrast {live_contrast:.1}:1 · AA Pass")
                                        } else {
                                            format!("Live contrast {live_contrast:.1}:1 · Below 4.5:1")
                                        }
                                    } else if live_contrast >= 7.0 {
                                        format!("实时对比度 {live_contrast:.1}:1 · AAA 护眼级")
                                    } else if live_pass {
                                        format!("实时对比度 {live_contrast:.1}:1 · AA 达标")
                                    } else {
                                        format!("实时对比度 {live_contrast:.1}:1 · 低于 4.5:1")
                                    }),
                            ),
                    )
                    // Real-time miniature reading preview
                    .child(
                        h_flex()
                            .p_3p5()
                            .gap_4()
                            .items_center()
                            .flex_wrap()
                            .rounded_xl()
                            .bg(rgb(hex(&live_bg)))
                            .border_1()
                            .border_color(rgb(hex(&live_text)).opacity(0.16))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w(px(240.))
                                    .p_3p5()
                                    .gap_1p5()
                                    .rounded_xl()
                                    .bg(rgb(hex(&live_paper)))
                                    .border_1()
                                    .border_color(rgb(hex(&live_text)).opacity(0.10))
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_size(px(10.))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&live_muted)))
                                                    .child(if self.locale().is_english() {
                                                        format!("Live paper preview · {live_name}")
                                                    } else {
                                                        format!("实时纸面预览 · {live_name}")
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_full()
                                                    .bg(rgb(hex(&live_accent)))
                                                    .text_size(px(9.5))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&live_paper)))
                                                    .child(self.tr("微光书签", "Glow Mark")),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.5))
                                            .font_family(self.display_font())
                                            .line_height(relative(1.6))
                                            .text_color(rgb(hex(&live_text)))
                                            .child(self.tr(
                                                "灯光不必照亮整片原野，只要足够照见眼前这一页。",
                                                "A lamp need not light the whole valley — only the page before you.",
                                            )),
                                    ),
                            ),
                    )
                    .child(fields)
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(self.button_with_icon(
                                self.tr("选择背景图片", "Choose Backdrop"),
                                IconName::Image,
                                Action::Asset("background".into()),
                                cx,
                            ))
                            .child(self.button_with_icon(
                                self.tr("选择字体文件", "Choose Font File"),
                                IconName::AArrowDown,
                                Action::Asset("font".into()),
                                cx,
                            ))
                            .child(self.button_with_icon(
                                self.tr("选择环境音", "Choose Soundscape"),
                                IconName::AudioLines,
                                Action::Asset("audio".into()),
                                cx,
                            ))
                            .child(self.button_with_icon(
                                if draft.reduced_motion {
                                    self.tr("减少动态：开", "Reduced Motion: On")
                                } else {
                                    self.tr("减少动态：关", "Reduced Motion: Off")
                                },
                                IconName::Sparkles,
                                Action::ReducedMotion,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&active_theme.muted)))
                            .child(if self.locale().is_english() {
                                format!(
                                    "Backdrop: {}  /  Font: {}  /  Audio: {}",
                                    draft.image.as_deref().unwrap_or("Built-in"),
                                    draft.font_asset.as_deref().unwrap_or("System"),
                                    draft.audio.as_deref().unwrap_or("None")
                                )
                            } else {
                                format!(
                                    "背景：{}  /  字体：{}  /  声音：{}",
                                    draft.image.as_deref().unwrap_or("内置"),
                                    draft.font_asset.as_deref().unwrap_or("系统"),
                                    draft.audio.as_deref().unwrap_or("未选择")
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(self.button_with_icon(
                                self.tr("预览", "Preview"),
                                IconName::Eye,
                                Action::PreviewTheme,
                                cx,
                            ))
                            .child(
                                self.button_with_icon(
                                    self.tr("保存主题", "Save Theme"),
                                    IconName::Check,
                                    Action::SaveTheme,
                                    cx,
                                )
                                .primary(),
                            )
                            .child(self.button(
                                self.tr("恢复默认", "Reset Defaults"),
                                Action::ResetTheme,
                                cx,
                            ))
                            .child(self.button(
                                self.tr("取消", "Cancel"),
                                Action::CancelTheme,
                                cx,
                            )),
                    ),
            );
        }
        view.into_any_element()
    }
}

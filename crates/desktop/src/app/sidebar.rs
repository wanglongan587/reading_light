use super::*;

impl ReadingApp {
    #[inline(never)]
    pub(super) fn render_sidebar(
        &self,
        t: &ReadingTheme,
        snap: MotionSnapshot,
        cx: &Context<Self>,
    ) -> Div {
        let dark = self.is_dark();
        let is_prism = self.is_prism();
        let freq = Self::frequency_for_theme(&t.id);
        let mut quick_swatches = h_flex().gap(px(3.5)).items_center().justify_between();
        for (idx, preset) in themes().iter().enumerate() {
            let selected = t.id == preset.id;
            let dot_color = if preset.id == "prism" {
                0x9b87ff
            } else {
                hex(&preset.accent)
            };
            quick_swatches = quick_swatches.child(
                div()
                    .id(SharedString::from(format!("quick-theme-{idx}")))
                    .cursor_pointer()
                    .p(px(1.5))
                    .rounded_full()
                    .border_1()
                    .border_color(if selected {
                        rgb(dot_color).opacity(0.95)
                    } else {
                        rgb(0xffffff).opacity(0.14)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::PickTheme(idx), window, cx);
                        }),
                    )
                    .child(div().size(px(8.5)).rounded_full().bg(rgb(dot_color))),
            );
        }
        let mut sidebar = v_flex()
            .relative()
            .w(px(224.))
            .h_full()
            .px_4()
            .py_5()
            .gap_1p5()
            .flex_shrink_0()
            .bg(rgb(hex(&t.paper)).opacity(if is_prism {
                0.86
            } else if dark {
                0.90
            } else {
                0.94
            }))
            .border_r_1()
            .border_color(self.border_subtle())
            .child(prism::prism_rail_vertical(snap.clone()))
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .px_2()
                    .pt_1()
                    .pb_5()
                    .child(
                        div()
                            .relative()
                            .p_0p5()
                            .rounded_2xl()
                            .bg(rgb(hex(&t.accent)).opacity(0.14))
                            .border_1()
                            .border_color(rgb(hex(&t.accent)).opacity(0.32))
                            .child(
                                img("glow-icon.png")
                                    .size(px(40.))
                                    .object_fit(ObjectFit::Contain)
                                    .rounded_xl(),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(21.))
                                            .font_family(self.display_font())
                                            .child(self.tr("微光", "Glow")),
                                    )
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(rgb(hex(&t.accent)).opacity(0.16))
                                            .border_1()
                                            .border_color(rgb(hex(&t.accent)).opacity(0.28))
                                            .text_size(px(8.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.accent)))
                                            .child(freq),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(8.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(if is_prism {
                                        "STARLIGHT PRISM STATION"
                                    } else if self.is_scenic_space() {
                                        "GLOW SPACE STATION"
                                    } else {
                                        "AMBIENT READING STATION"
                                    }),
                            ),
                    ),
            );
        sidebar = sidebar.child(
            h_flex()
                .px_3()
                .pb_1p5()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(9.))
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.muted)).opacity(0.84))
                        .child(self.tr("核心三轴", "NAVIGATION")),
                )
                .child(
                    div()
                        .text_size(px(8.))
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.muted)).opacity(0.55))
                        .child("STUDY · PAGES · JOURNAL"),
                ),
        );
        for (label, selector, page, icon) in [
            (
                self.tr("书房", "Study"),
                "我的书架",
                Page::Shelf,
                IconName::LibraryBig,
            ),
            (
                self.tr("书页", "Pages"),
                "回到阅读",
                Page::Reader,
                IconName::BookOpen,
            ),
            (
                self.tr("手账", "Journal"),
                "私人日记",
                Page::Journal,
                IconName::NotebookPen,
            ),
        ] {
            let active = self.page == page;
            sidebar = sidebar.child(self.nav_button_with_selector(
                label,
                selector,
                icon,
                active,
                Action::Page(page),
                cx,
            ));
        }
        // Compact ambient & studio quick row (keeps sidebar focused on the 3 main axes)
        let mut studio_row = h_flex().gap_1().pt_1p5().items_center().justify_between();
        for (label, selector, page, icon) in [
            (
                self.tr("电台", "FM"),
                "深夜电台",
                Page::Radio,
                IconName::AudioLines,
            ),
            (
                self.tr("心绪", "Mood"),
                "此刻心情",
                Page::Mood,
                IconName::MessageCircleHeart,
            ),
            (
                self.tr("空间", "Space"),
                "主题与设置",
                Page::Themes,
                IconName::Settings2,
            ),
        ] {
            let active = self.page == page;
            studio_row = studio_row.child(
                div()
                    .id(SharedString::from(format!("studio-mini-{selector}")))
                    .debug_selector(move || selector.to_owned())
                    .cursor_pointer()
                    .flex_1()
                    .py_1p5()
                    .px_1()
                    .rounded_xl()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .bg(if active {
                        rgb(hex(&t.accent)).opacity(0.20)
                    } else {
                        rgb(hex(&t.background)).opacity(if dark { 0.56 } else { 0.64 })
                    })
                    .border_1()
                    .border_color(if active {
                        Hsla::from(rgb(hex(&t.accent)).opacity(0.62))
                    } else {
                        self.border_subtle()
                    })
                    .hover(|s| s.border_color(rgb(hex(&t.accent)).opacity(0.45)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::Page(page.clone()), window, cx);
                        }),
                    )
                    .child(Icon::new(icon).size(px(11.)).text_color(if active {
                        rgb(hex(&t.accent))
                    } else {
                        rgb(hex(&t.muted))
                    }))
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(if active {
                                rgb(hex(&t.accent))
                            } else {
                                rgb(hex(&t.text))
                            })
                            .child(label),
                    ),
            );
        }
        sidebar = sidebar.child(studio_row);
        sidebar = sidebar.child(
            div()
                .id("palette-trigger")
                .cursor_pointer()
                .mt_1()
                .px_3()
                .py_1p5()
                .rounded_xl()
                .bg(rgb(hex(&t.background)).opacity(if dark { 0.56 } else { 0.64 }))
                .border_1()
                .border_color(self.border_subtle())
                .hover(|s| s.border_color(rgb(hex(&t.accent)).opacity(0.45)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.act(Action::TogglePalette, window, cx);
                    }),
                )
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .child(
                                    Icon::new(IconName::Command)
                                        .size(px(11.5))
                                        .text_color(rgb(hex(&t.accent))),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(hex(&t.muted)))
                                        .child(self.tr("快捷指令", "Commands")),
                                ),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded(px(5.))
                                .bg(rgb(hex(&t.paper)).opacity(0.85))
                                .text_size(px(9.))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.muted)))
                                .child("Ctrl+K"),
                        ),
                ),
        );
        let deck = self.render_soundscape_deck(t, snap, quick_swatches, cx);
        let presence_count = ambient_presence_count();
        let presence_on = self.store.state.show_ambient_presence;
        sidebar.child(div().flex_1()).child(deck).child(
            h_flex()
                .id("ambient-presence-toggle")
                .debug_selector(|| "ambient-presence-toggle".into())
                .cursor_pointer()
                .px_2p5()
                .pt_2()
                .gap_1p5()
                .items_center()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.act(Action::ToggleAmbientPresence, window, cx);
                    }),
                )
                .child(div().size(px(5.5)).rounded_full().bg(if presence_on {
                    rgb(hex(&t.accent)).opacity(0.88)
                } else {
                    rgb(hex(&t.muted)).opacity(0.35)
                }))
                .child(
                    div()
                        .text_size(px(8.5))
                        .font_family(self.ui_font())
                        .line_height(relative(1.45))
                        .text_color(rgb(hex(&t.muted)).opacity(0.86))
                        .child(if presence_on {
                            if self.locale().is_english() {
                                format!("~{presence_count} quiet lamps reading nearby")
                            } else {
                                format!("此刻约 {presence_count} 盏微光正安静阅读")
                            }
                        } else {
                            self.tr(
                                "纯粹独处中 · 一盏灯，一本书",
                                "Pure solitude · One lamp, one book",
                            )
                            .to_owned()
                        }),
                ),
        )
    }
    #[inline(never)]
    pub(super) fn render_soundscape_deck(
        &self,
        t: &ReadingTheme,
        snap: MotionSnapshot,
        quick_swatches: Div,
        cx: &Context<Self>,
    ) -> Div {
        let dark = self.is_dark();
        let is_prism = self.is_prism();
        let vial_ratio = self.overall_reading_progress();
        let active_preset = self.active_soundscape_preset();
        let focus_status = self.focus_timer_progress();
        let mut preset_row = h_flex().gap_1().items_center().justify_between();
        let active_mix = self.active_soundscape_mix();
        for preset in SoundscapePreset::SIDEBAR_QUICK {
            let selected = preset.mix_value(&active_mix) >= 0.01
                || (active_mix.is_silent() && active_preset == preset);
            preset_row = preset_row.child(
                div()
                    .id(SharedString::from(format!("sc-{}", preset.id())))
                    .cursor_pointer()
                    .flex_1()
                    .py_1()
                    .rounded_lg()
                    .flex()
                    .justify_center()
                    .bg(if selected {
                        rgb(hex(&t.accent)).opacity(0.22)
                    } else {
                        rgb(hex(&t.paper)).opacity(if dark { 0.42 } else { 0.62 })
                    })
                    .border_1()
                    .border_color(if selected {
                        rgb(hex(&t.accent)).opacity(0.65)
                    } else {
                        rgb(0x000000).opacity(0.0)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::Soundscape(preset), window, cx);
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_family(self.ui_font())
                            .text_color(if selected {
                                rgb(hex(&t.accent))
                            } else {
                                rgb(hex(&t.muted))
                            })
                            .child(preset.short_label()),
                    ),
            );
        }
        let vol_pct = (t.volume.clamp(0.05, 1.0) * 100.0).round() as u32;
        let today_mins =
            if self.store.state.reading_minutes_date.as_deref() == Some(today_ymd().as_str()) {
                self.store.state.reading_minutes_today
            } else {
                0
            };
        let deck = v_flex()
            .relative()
            .overflow_hidden()
            .p_3()
            .gap_2()
            .rounded_2xl()
            .bg(rgb(hex(&t.background)).opacity(if dark { 0.76 } else { 0.78 }))
            .border_1()
            .border_color(self.border_subtle())
            .when(
                self.radio_status.playing && self.page != Page::Radio,
                |el| el.child(self.render_sidebar_radio_mini(t, cx)),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(prism::prism_beam_horizontal(snap.clone(), 1.6, 0.9, false)),
            )
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(
                                Icon::new(match t.id.as_str() {
                                    "prism" | "spirited" => IconName::Sparkles,
                                    "night" => IconName::Moon,
                                    "forest" => IconName::Cloud,
                                    "totoro" => IconName::Leaf,
                                    "howl" => IconName::Wind,
                                    "lantern" => IconName::Flame,
                                    _ => IconName::Sun,
                                })
                                .size(px(13.))
                                .text_color(rgb(hex(&t.accent))),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.text)))
                                    .child(t.name.clone()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(div().size(px(5.)).rounded_full().bg(rgb(
                                if self.audio.is_some()
                                    || focus_status.is_some()
                                    || (is_prism && !t.reduced_motion)
                                {
                                    0x36c98e
                                } else {
                                    hex(&t.accent)
                                },
                            )))
                            .child(
                                div()
                                    .text_size(px(8.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(if focus_status.is_some() {
                                        "QUIET"
                                    } else if self.audio.is_some() {
                                        "LIVE"
                                    } else if is_prism && !t.reduced_motion {
                                        "FLOW"
                                    } else {
                                        "IDLE"
                                    }),
                            ),
                    ),
            )
            // Full-card interactive Quiet Reading Timer trigger (discoverable label + pill CTA)
            .child(
                h_flex()
                    .id("vial-trigger")
                    .debug_selector(|| "vial-trigger".into())
                    .cursor_pointer()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_2p5()
                    .py_2()
                    .rounded_xl()
                    .bg(rgb(hex(&t.paper)).opacity(if dark { 0.68 } else { 0.86 }))
                    .border_1()
                    .border_color(if focus_status.is_some() || self.focus_timer_picker_open {
                        rgb(hex(&t.accent)).opacity(0.62)
                    } else {
                        rgb(hex(&t.accent)).opacity(0.24)
                    })
                    .hover(|s| {
                        s.bg(rgb(hex(&t.paper)))
                            .border_color(rgb(hex(&t.accent)).opacity(0.75))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.act(Action::ToggleTimerPicker, window, cx);
                        }),
                    )
                    .child(prism::light_vial_canvas(snap, vial_ratio))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(hex(&t.text)))
                                            .child(if let Some((_, rem)) = focus_status {
                                                format!("静读 {:02}:{:02}", rem / 60, rem % 60)
                                            } else {
                                                "静读计时".to_owned()
                                            }),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_0p5()
                                            .items_center()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(rgb(hex(&t.accent)).opacity(0.16))
                                            .border_1()
                                            .border_color(rgb(hex(&t.accent)).opacity(0.32))
                                            .child(
                                                Icon::new(IconName::Timer)
                                                    .size(px(8.5))
                                                    .text_color(rgb(hex(&t.accent))),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(8.))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.accent)))
                                                    .child(if focus_status.is_some() {
                                                        "调整"
                                                    } else {
                                                        "设时长"
                                                    }),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(8.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .text_ellipsis()
                                    .child(if focus_status.is_some() {
                                        format!(
                                            "已蓄光 {:.0}% · 点此结束或重设",
                                            vial_ratio * 100.0
                                        )
                                    } else if today_mins > 0 {
                                        format!("今日已静读 {today_mins} 分钟")
                                    } else {
                                        "专注倒计时 · 可自定义".to_owned()
                                    }),
                            ),
                    ),
            )
            // Dedicated soundscape channel status row + multi-track mixer button + waveform
            .child(
                h_flex()
                    .px_1()
                    .justify_between()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(8.5))
                            .font_family(self.ui_font())
                            .text_ellipsis()
                            .text_color(if self.audio.is_some() {
                                rgb(hex(&t.accent))
                            } else {
                                rgb(hex(&t.muted))
                            })
                            .child(format!("声景 · {}", active_mix.summary_label())),
                    )
                    .child(
                        div()
                            .id("mixer-trigger")
                            .debug_selector(|| "mixer-trigger".into())
                            .cursor_pointer()
                            .px_1p5()
                            .py_0p5()
                            .rounded_full()
                            .bg(rgb(hex(&t.accent)).opacity(if self.mixer_open {
                                0.28
                            } else {
                                0.14
                            }))
                            .border_1()
                            .border_color(rgb(hex(&t.accent)).opacity(0.36))
                            .text_size(px(8.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.accent)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.act(Action::ToggleMixer, window, cx);
                                }),
                            )
                            .child("混音台"),
                    )
                    .child(
                        self.waveform_bars(self.audio.is_some(), &[4., 9., 13., 7., 11., 6., 9.]),
                    ),
            );
        deck.child(preset_row)
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .px_1()
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                Icon::new(IconName::Volume2)
                                    .size(px(11.))
                                    .text_color(rgb(hex(&t.muted))),
                            )
                            .child(
                                div()
                                    .text_size(px(8.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(format!("音量 {vol_pct}%")),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                div()
                                    .id("vol-down")
                                    .cursor_pointer()
                                    .px_1p5()
                                    .rounded(px(5.))
                                    .bg(rgb(hex(&t.paper)).opacity(0.7))
                                    .text_size(px(10.))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.act(Action::AudioVolume(-1), window, cx);
                                        }),
                                    )
                                    .child("−"),
                            )
                            .child(
                                div()
                                    .id("vol-up")
                                    .cursor_pointer()
                                    .px_1p5()
                                    .rounded(px(5.))
                                    .bg(rgb(hex(&t.paper)).opacity(0.7))
                                    .text_size(px(10.))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.act(Action::AudioVolume(1), window, cx);
                                        }),
                                    )
                                    .child("+"),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        h_flex()
                            .px_1()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(8.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("微光空间"),
                            )
                            .child(
                                div()
                                    .id("daylight-auto-pill")
                                    .debug_selector(|| "daylight-auto-pill".into())
                                    .cursor_pointer()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(if self.store.state.daylight_auto {
                                        rgb(hex(&t.accent)).opacity(0.22)
                                    } else {
                                        rgb(hex(&t.paper)).opacity(0.55)
                                    })
                                    .border_1()
                                    .border_color(if self.store.state.daylight_auto {
                                        Hsla::from(rgb(hex(&t.accent)).opacity(0.55))
                                    } else {
                                        self.border_subtle()
                                    })
                                    .text_size(px(8.))
                                    .font_family(self.ui_font())
                                    .text_color(if self.store.state.daylight_auto {
                                        rgb(hex(&t.accent))
                                    } else {
                                        rgb(hex(&t.muted))
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.act(Action::ToggleDaylightAuto, window, cx);
                                        }),
                                    )
                                    .child(format!(
                                        "昼夜·{}{}",
                                        circadian_daylight_stage().0,
                                        if self.store.state.daylight_auto {
                                            " ✓"
                                        } else {
                                            ""
                                        }
                                    )),
                            ),
                    )
                    .child(quick_swatches),
            )
            .child(self.nav_button(
                if self.audio.is_some() {
                    "关闭环境音"
                } else {
                    "开启环境音"
                },
                if self.audio.is_some() {
                    IconName::VolumeX
                } else {
                    IconName::AudioLines
                },
                self.audio.is_some(),
                Action::Audio,
                cx,
            ))
    }
}

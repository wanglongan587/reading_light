use super::*;

impl ReadingApp {
    pub(super) fn all_themes(&self) -> Vec<ReadingTheme> {
        let mut list = themes();
        list.extend(self.store.state.custom_themes.clone());
        list
    }
    pub(super) fn fill_fields(&self, t: &ReadingTheme, window: &mut Window, cx: &mut App) {
        let values = vec![
            t.name.clone(),
            t.background.clone(),
            t.paper.clone(),
            t.text.clone(),
            t.muted.clone(),
            t.accent.clone(),
            t.font.clone(),
            t.font_size.to_string(),
            t.line_height.to_string(),
            t.width.to_string(),
            t.overlay.to_string(),
            t.volume.to_string(),
            String::new(),
        ];
        for (field, value) in self.fields.iter().zip(values) {
            field.update(cx, |s, cx| s.set_value(value, window, cx));
        }
    }
    pub(super) fn read_fields(&self, cx: &App) -> anyhow::Result<ReadingTheme> {
        let mut t = self
            .draft
            .clone()
            .ok_or_else(|| anyhow::anyhow!("请先创建主题"))?;
        let v = self
            .fields
            .iter()
            .map(|f| f.read(cx).value().to_string())
            .collect::<Vec<_>>();
        t.name = v[0].clone();
        t.background = v[1].clone();
        t.paper = v[2].clone();
        t.text = v[3].clone();
        t.muted = v[4].clone();
        t.accent = v[5].clone();
        t.font = v[6].clone();
        t.font_size = v[7].parse()?;
        t.line_height = v[8].parse()?;
        t.width = v[9].parse()?;
        t.overlay = v[10].parse()?;
        t.volume = v[11].parse()?;
        Ok(t)
    }
    pub(super) fn is_dark(&self) -> bool {
        glow_core::theme::color(&self.theme().paper).unwrap_or(0) <= 0x606060
    }
    pub(super) fn is_prism(&self) -> bool {
        self.theme().id == "prism"
    }
    #[cfg(all(test, feature = "ui-test"))]
    pub(super) fn is_sky(&self) -> bool {
        self.theme().id == "forest"
    }
    pub(super) fn is_scenic_space(&self) -> bool {
        Self::is_scenic_space_id(&self.theme().id)
    }
    pub(super) fn is_scenic_space_id(id: &str) -> bool {
        matches!(
            id,
            "forest" | "totoro" | "spirited" | "howl" | "kiki" | "lantern"
        )
    }
    pub(super) fn scene_asset_for(id: &str) -> Option<&'static str> {
        match id {
            "forest" => Some("scene.png"),
            "totoro" => Some("scene-totoro.png"),
            "spirited" => Some("scene-spirited.png"),
            "howl" => Some("scene-howl.png"),
            "kiki" => Some("scene-kiki.png"),
            "lantern" => Some("scene-lantern.png"),
            "night" => Some("scene-night.png"),
            "prism" => Some("scene-prism.png"),
            _ => None,
        }
    }
    pub(super) fn theme_card_meta(id: &str) -> (&'static str, Option<&'static str>, &'static str) {
        match id {
            "forest" => (
                "SKY STUDY · 晴空旧书房 · 88.6 FM",
                Some("✦ 微光空间"),
                "蓝天流云与高窗书桌，清澈舒展",
            ),
            "totoro" => (
                "CAMPHOR SHADE · 夏风樟树下 · 90.2 FM",
                Some("✦ 微光空间"),
                "古樟树荫与林间晴雨，温柔护眼",
            ),
            "spirited" => (
                "TWILIGHT SEA · 浅海晚风号 · 94.8 FM",
                Some("✦ 微光空间"),
                "浅海黄昏与远岸灯塔，卸下疲惫",
            ),
            "howl" => (
                "MEADOW BREEZE · 山谷秘密花园 · 96.5 FM",
                Some("✦ 微光空间"),
                "晨雾花海与雪山微风，轻盈开阔",
            ),
            "kiki" => (
                "HARBOR BAKERY · 海港烘焙屋 · 98.4 FM",
                Some("✦ 微光空间"),
                "红瓦海港与暖阳窗台，惬意日常",
            ),
            "lantern" => (
                "LANTERN RAIN · 雨夜灯笼巷 · 103.6 FM",
                Some("✦ 微光夜读"),
                "暖灯雨窗与静谧暮川，治愈夜读",
            ),
            "paper" => (
                "MORNING PAPER · 92.4 FM",
                None,
                "温润古典素纸，专注文字本身",
            ),
            "night" => ("NIGHT LAMP · 101.7 FM", None, "低亮度青墨夜色，静心深读"),
            "prism" => (
                "STARLIGHT PRISM · 108.0 FM",
                Some("✦ 鼠标流光"),
                "星夜光谱与流光粒子交互",
            ),
            _ => ("PERSONAL SPACE · 96.0 FM", None, "你亲手调制的私人阅读空间"),
        }
    }
    pub(super) fn theme_cover_palette(&self) -> [u32; 4] {
        match self.theme().id.as_str() {
            "forest" => [0x2b6294, 0x3b729c, 0x385e82, 0x4b6b8e],
            "totoro" => [0x365c43, 0x4b6f50, 0x5c6d4c, 0x3d5c52],
            "spirited" => [0x8c4442, 0x6b4c5e, 0x456278, 0x995d4c],
            "howl" => [0x554c82, 0x6e5a88, 0x48667d, 0x7b6078],
            "kiki" => [0x9c5739, 0x7c5a44, 0x3f6478, 0xa16f43],
            "lantern" => [0x6e322e, 0x4d354a, 0x7a5032, 0x384359],
            _ => [0x385043, 0x69503d, 0x364d56, 0x524656],
        }
    }
    pub(super) fn theme_hud_palette(&self) -> (u32, Rgba, Option<u32>) {
        match self.theme().id.as_str() {
            "forest" => (0x133352, rgb(0xbae6fd), Some(0x93c5fd)),
            "totoro" => (0x183022, rgb(0xd9f2d0), Some(0xa7ddb0)),
            "spirited" => (0x361d24, rgb(0xffdfc9), Some(0xf5bfa8)),
            "howl" => (0x241f3d, rgb(0xe4dcff), Some(0xc7bef2)),
            "kiki" => (0x382418, rgb(0xffe6bd), Some(0xf2c89b)),
            "lantern" => (0x1a1322, rgb(0xfde2a6), Some(0xe69f52)),
            _ => (0x0b1512, rgb(0xf5dc9c), Some(0xf5dc9c)),
        }
    }
    pub(super) fn theme_shelf_cue(&self) -> &'static str {
        match self.theme().id.as_str() {
            "forest" => "云端晴空 · 今天的一小段",
            "totoro" => "樟树林荫 · 今天的一小段",
            "spirited" => "浅海黄昏 · 今天的一小段",
            "howl" => "秘密花原 · 今天的一小段",
            "kiki" => "海风晴午 · 今天的一小段",
            "lantern" => "暖灯夜读 · 今天的一小段",
            "prism" => "棱镜星穹 · 今天的一小段",
            _ => "今天的一小段",
        }
    }
    pub(super) fn schedule_motion_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.frame_scheduled || self.theme().reduced_motion || cfg!(test) || self.smoke {
            return;
        }
        self.frame_scheduled = true;
        cx.on_next_frame(window, |this, _, cx| {
            this.frame_scheduled = false;
            cx.notify();
        });
    }
    pub(super) fn scene_asset(&self) -> &'static str {
        Self::scene_asset_for(&self.theme().id).unwrap_or("scene.png")
    }
    pub(super) fn motion_snapshot(&self) -> MotionSnapshot {
        let t = self.theme();
        let active_mood = if self.page == Page::Mood {
            Some(self.mood.clone())
        } else {
            self.store
                .state
                .journal
                .first()
                .and_then(|entry| entry.mood.clone())
        };
        self.motion.snapshot(
            ThemeAtmosphere::from_theme_id(&t.id),
            hex(&t.accent),
            self.is_dark(),
            t.reduced_motion,
            active_mood,
        )
    }
    pub(super) fn focus_timer_progress(&self) -> Option<(f32, u32)> {
        let started = self.focus_timer_started?;
        let elapsed = started.elapsed().as_secs_f32();
        let total = (self.focus_timer_total_secs as f32).max(1.0);
        let ratio = (elapsed / total).clamp(0.0, 1.0);
        let remaining_secs = (self.focus_timer_total_secs as f32 - elapsed)
            .max(0.0)
            .ceil() as u32;
        Some((ratio, remaining_secs))
    }
    pub(super) fn overall_reading_progress(&self) -> f32 {
        if let Some((ratio, _)) = self.focus_timer_progress() {
            return ratio.clamp(0.0, 1.0);
        }
        if let Some(active_id) = self
            .active_book
            .as_ref()
            .or(self.store.state.last_book.as_ref())
            && let Some(book) = self.store.state.books.iter().find(|b| &b.id == active_id)
            && let Some(p) = book.progress
        {
            return p.clamp(0.0, 1.0);
        }
        self.store
            .state
            .books
            .iter()
            .filter_map(|b| b.progress)
            .fold(0.0_f32, f32::max)
            .clamp(0.0, 1.0)
    }
    pub(super) fn daily_rediscovery(&self) -> Option<DailyRediscovery> {
        let mut pool: Vec<RediscoveryItem> = Vec::new();
        for book in &self.store.state.books {
            for hl in &book.highlights {
                pool.push((
                    hl.text.clone(),
                    format!("《{}》", book.title),
                    Some((book.id.clone(), hl.cfi.clone())),
                ));
            }
        }
        if pool.is_empty() {
            for entry in self.store.state.journal.iter().filter(|e| e.favorite) {
                let text = if !entry.input.trim().is_empty() {
                    entry.input.clone()
                } else {
                    entry.echo.reply.clone()
                };
                let target = entry
                    .location
                    .as_ref()
                    .filter(|loc| {
                        glow_core::model::valid_cfi(&loc.cfi)
                            && self.store.state.books.iter().any(|b| b.id == loc.book_id)
                    })
                    .map(|loc| (loc.book_id.clone(), loc.cfi.clone()));
                pool.push((
                    text,
                    entry
                        .mood
                        .as_ref()
                        .map(|m| format!("收藏心情 · {m}"))
                        .unwrap_or_else(|| "收藏回响".into()),
                    target,
                ));
            }
        }
        let total = pool.len();
        if total == 0 {
            return None;
        }
        let day = time::OffsetDateTime::now_local()
            .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
            .ordinal() as usize;
        let idx = (day + self.rediscovery_offset) % total;
        let (text, source, target) = pool.into_iter().nth(idx)?;
        Some((text, source, target, total))
    }
    pub(super) fn pet_dock(&self, id: &'static str, cx: &Context<Self>) -> impl IntoElement {
        let snap = self.motion_snapshot();
        div()
            .id(id)
            .cursor_pointer()
            .rounded_xl()
            .px_1()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.act(Action::PetInteract(event.position), window, cx);
                }),
            )
            .child(prism::prism_companion_canvas(snap))
    }
    pub(super) fn border_subtle(&self) -> Hsla {
        if self.is_prism() {
            rgb(0xa5b4fc).opacity(0.14).into()
        } else if self.is_dark() {
            rgb(0xffffff).opacity(0.11).into()
        } else {
            rgb(hex(&self.theme().text)).opacity(0.10).into()
        }
    }
    pub(super) fn border_strong(&self) -> Hsla {
        if self.is_prism() {
            rgb(0xc4b5fd).opacity(0.24).into()
        } else if self.is_dark() {
            rgb(0xffffff).opacity(0.18).into()
        } else {
            rgb(hex(&self.theme().text)).opacity(0.16).into()
        }
    }
    pub(super) fn specular_line(&self) -> Div {
        div()
            .w_full()
            .h(px(1.))
            .bg(rgb(0xffffff).opacity(if self.is_dark() { 0.14 } else { 0.62 }))
    }
    pub(super) fn frequency_for_theme(id: &str) -> &'static str {
        match id {
            "forest" => "FM 88.6",
            "totoro" => "FM 90.2",
            "paper" => "FM 92.4",
            "spirited" => "FM 94.8",
            "howl" => "FM 96.5",
            "kiki" => "FM 98.4",
            "night" => "FM 101.7",
            "lantern" => "FM 103.6",
            "prism" => "FM 108.0",
            _ => "FM 96.0",
        }
    }
    pub(super) fn waveform_bars(&self, active: bool, heights: &[f32]) -> Div {
        self.waveform_bars_tinted(active, heights, None)
    }
    pub(super) fn waveform_bars_tinted(
        &self,
        active: bool,
        heights: &[f32],
        tint: Option<u32>,
    ) -> Div {
        let t = self.theme();
        let prism = self.is_prism();
        let animate = !t.reduced_motion && (active || prism);
        let mut row = h_flex().gap_1().items_center().h(px(18.));
        for (idx, h) in heights.iter().enumerate() {
            let dyn_h = if animate {
                let wave = (self.motion.phase * 2.6 + idx as f32 * 0.78).sin() * 0.34 + 0.86;
                (*h * wave).clamp(3.5, 17.5)
            } else {
                *h
            };
            let opacity = if active {
                if idx % 2 == 0 { 0.94 } else { 0.68 }
            } else if prism {
                if idx % 2 == 0 { 0.82 } else { 0.56 }
            } else if idx % 2 == 0 {
                0.42
            } else {
                0.24
            };
            let bar_bg: Hsla = if prism && tint.is_none() {
                prism::spectral_color(
                    (idx as f32 / heights.len().max(1) as f32) + self.motion.phase * 0.06,
                    opacity,
                )
            } else {
                rgb(tint.unwrap_or_else(|| hex(&t.accent)))
                    .opacity(opacity)
                    .into()
            };
            row = row.child(div().w(px(2.5)).h(px(dyn_h)).rounded_full().bg(bar_bg));
        }
        row
    }
    pub(super) fn studio_badge(&self, code: &str, label: &str, active: bool) -> Div {
        let t = self.theme();
        let dot_color: Hsla = if active && self.is_prism() {
            prism::spectral_color(self.motion.phase * 0.08 + 0.55, 1.0)
        } else if active {
            rgb(hex(&t.accent)).into()
        } else {
            rgb(hex(&t.muted)).into()
        };
        h_flex()
            .gap_2()
            .items_center()
            .px_2p5()
            .py_1()
            .rounded_full()
            .bg(rgb(hex(&t.accent)).opacity(if self.is_prism() {
                0.15
            } else if self.is_dark() {
                0.18
            } else {
                0.11
            }))
            .border_1()
            .border_color(rgb(hex(&t.accent)).opacity(if self.is_prism() { 0.34 } else { 0.28 }))
            .child(div().size(px(6.)).rounded_full().bg(dot_color))
            .child(
                div()
                    .text_size(px(10.))
                    .font_family(self.ui_font())
                    .text_color(rgb(hex(&t.accent)))
                    .child(format!("{code}  ·  {label}")),
            )
    }
    pub(super) fn button(&self, label: &str, action: Action, cx: &Context<Self>) -> Button {
        Button::new(SharedString::from(format!("{label}-{action:?}")))
            .label(label.to_owned())
            .ghost()
            .font_family(self.ui_font())
            .disabled(self.busy)
            .debug_selector({
                let label = label.to_owned();
                move || label.clone()
            })
            .on_click(cx.listener(move |this, _, window, cx| this.act(action.clone(), window, cx)))
    }
    pub(super) fn button_with_icon(
        &self,
        label: &str,
        icon: IconName,
        action: Action,
        cx: &Context<Self>,
    ) -> Button {
        self.button(label, action, cx).icon(icon)
    }
    pub(super) fn pill_button(
        &self,
        label: &str,
        icon: Option<IconName>,
        active: bool,
        action: Action,
        cx: &Context<Self>,
    ) -> Div {
        let t = self.theme();
        let style = ButtonCustomVariant::new(cx)
            .color(if active {
                rgb(hex(&t.paper)).opacity(0.0).into()
            } else {
                rgb(hex(&t.paper))
                    .opacity(if self.is_dark() { 0.45 } else { 0.72 })
                    .into()
            })
            .foreground(if active {
                rgb(hex(&t.paper)).into()
            } else {
                rgb(hex(&t.text)).into()
            })
            .hover(if active {
                rgb(hex(&t.accent)).opacity(0.90).into()
            } else {
                rgb(hex(&t.accent)).opacity(0.14).into()
            })
            .active(rgb(hex(&t.accent)).opacity(0.24).into());
        let mut button = Button::new(SharedString::from(format!("pill-{label}-{action:?}")))
            .label(label.to_owned())
            .custom(style)
            .rounded(px(18.))
            .h(px(32.))
            .px_3()
            .font_family(self.ui_font())
            .text_size(px(12.))
            .toggled(active)
            .disabled(self.busy)
            .debug_selector({
                let label = label.to_owned();
                move || label.clone()
            })
            .on_click(cx.listener(move |this, _, window, cx| this.act(action.clone(), window, cx)));
        if let Some(icon) = icon {
            button = button.icon(icon);
        }
        div()
            .rounded(px(18.))
            .border_1()
            .border_color(if active {
                Hsla::from(rgb(hex(&t.accent)).opacity(0.92))
            } else {
                self.border_subtle()
            })
            .when(active, |this| this.bg(rgb(hex(&t.accent))).shadow_sm())
            .child(button)
    }
    pub(super) fn effective_warm_filter(&self) -> bool {
        self.store.state.warm_filter
            || (self.store.state.daylight_auto && circadian_daylight_stage().1)
    }
    pub(super) fn nav_button(
        &self,
        label: &str,
        icon: IconName,
        active: bool,
        action: Action,
        cx: &Context<Self>,
    ) -> Div {
        self.nav_button_with_selector(label, label, icon, active, action, cx)
    }
    pub(super) fn nav_button_with_selector(
        &self,
        label: &str,
        selector: &str,
        icon: IconName,
        active: bool,
        action: Action,
        cx: &Context<Self>,
    ) -> Div {
        let t = self.theme();
        let foreground: Hsla = if active {
            rgb(hex(&t.paper)).into()
        } else {
            rgb(hex(&t.text)).into()
        };
        let background: Hsla = rgb(hex(&t.paper)).opacity(0.0).into();
        let hover: Hsla = if active {
            rgb(hex(&t.accent)).opacity(0.90).into()
        } else {
            rgb(hex(&t.accent)).opacity(0.12).into()
        };
        let pressed: Hsla = rgb(hex(&t.accent)).opacity(0.22).into();
        let style = ButtonCustomVariant::new(cx)
            .color(background)
            .foreground(foreground)
            .hover(hover)
            .active(pressed);
        let button = Button::new(SharedString::from(format!("{label}-{action:?}")))
            .label(label.to_owned())
            .icon(icon)
            .custom(style)
            .rounded(px(11.))
            .w_full()
            .h(px(40.))
            .font_family(self.ui_font())
            .text_size(px(13.5))
            .toggled(active)
            .disabled(self.busy)
            .debug_selector({
                let selector = selector.to_owned();
                move || selector.clone()
            })
            .on_click(cx.listener(move |this, _, window, cx| this.act(action.clone(), window, cx)));
        div()
            .relative()
            .w_full()
            .rounded(px(11.))
            .border_1()
            .border_color(if active {
                rgb(hex(&t.accent)).opacity(0.65)
            } else {
                rgb(0x000000).opacity(0.0)
            })
            .when(active, |this| this.bg(rgb(hex(&t.accent))).shadow_sm())
            .child(button)
            .when(active, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(5.))
                        .top(px(11.))
                        .w(px(3.))
                        .h(px(18.))
                        .rounded_full()
                        .bg(rgb(hex(&t.paper)).opacity(0.85)),
                )
            })
    }
    pub(super) fn mood_button(&self, mood: &str, active: bool, cx: &Context<Self>) -> Div {
        let t = self.theme();
        let mood = mood.to_owned();
        let style = ButtonCustomVariant::new(cx)
            .color(if active {
                rgb(hex(&t.paper)).opacity(0.0).into()
            } else {
                rgb(hex(&t.background)).opacity(0.55).into()
            })
            .foreground(if active {
                rgb(hex(&t.paper)).into()
            } else {
                rgb(hex(&t.text)).into()
            })
            .hover(if active {
                rgb(hex(&t.accent)).opacity(0.88).into()
            } else {
                rgb(hex(&t.accent)).opacity(0.14).into()
            })
            .active(rgb(hex(&t.accent)).opacity(0.22).into());
        let button = Button::new(SharedString::from(format!("mood-{mood}")))
            .label(mood.clone())
            .icon(IconName::Heart)
            .custom(style)
            .rounded(px(20.))
            .h(px(36.))
            .px_3p5()
            .font_family(self.ui_font())
            .text_size(px(13.))
            .toggled(active)
            .disabled(self.busy)
            .debug_selector({
                let mood = mood.clone();
                move || mood.clone()
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.act(Action::Mood(mood.clone()), window, cx)
            }));
        div()
            .rounded(px(20.))
            .border_1()
            .border_color(if active {
                Hsla::from(rgb(hex(&t.accent)).opacity(0.95))
            } else {
                self.border_subtle()
            })
            .when(active, |this| this.bg(rgb(hex(&t.accent))).shadow_sm())
            .child(button)
    }
    pub(super) fn heading(&self, kicker: &str, title: &str, sub: &str) -> Div {
        let freq = Self::frequency_for_theme(&self.theme().id);
        v_flex()
            .gap_2()
            .items_start()
            .child(self.studio_badge(freq, kicker, true))
            .child(
                div()
                    .text_size(px(28.))
                    .font_family(self.display_font())
                    .line_height(relative(1.22))
                    .child(title.to_owned()),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .font_family(self.ui_font())
                    .line_height(relative(1.6))
                    .text_color(rgb(hex(&self.theme().muted)))
                    .max_w(px(680.))
                    .child(sub.to_owned()),
            )
    }
}

pub(super) fn hex(s: &str) -> u32 {
    glow_core::theme::color(s).unwrap_or(0x353e35)
}

pub(super) fn highlight_color_hex(color: Option<&str>, fallback: u32) -> u32 {
    match color {
        Some("amber") => 0xb45309,
        Some("sage") => 0x047857,
        Some("rose") => 0xbe123c,
        Some("prism") => 0x7e22ce,
        _ => fallback,
    }
}

pub(super) fn highlight_color_hex_for(color: Option<&str>, fallback: u32, is_dark: bool) -> u32 {
    if is_dark {
        match color {
            Some("amber") => 0xfbbf24,
            Some("sage") => 0x34d399,
            Some("rose") => 0xfb7185,
            Some("prism") => 0xc084fc,
            _ => fallback,
        }
    } else {
        highlight_color_hex(color, fallback)
    }
}

pub(super) fn highlight_color_name(color: Option<&str>) -> &'static str {
    match color {
        Some("amber") => "琥珀暖黄",
        Some("sage") => "苔藓微绿",
        Some("rose") => "樱雾淡粉",
        Some("prism") => "棱镜暮紫",
        _ => "原文摘录",
    }
}

pub(super) fn mood_spectrum_color(mood: Option<&str>) -> u32 {
    match mood {
        Some("孤独") => 0x818cf8,
        Some("疲惫") => 0xf59e0b,
        Some("迷茫") => 0x38bdf8,
        Some("失落") => 0x60a5fa,
        Some("成长") => 0x34d399,
        Some("希望") => 0xfbbf24,
        _ => 0xf472b6,
    }
}

pub(super) fn mood_badge_text_color(mood: Option<&str>, is_dark: bool) -> u32 {
    if is_dark {
        mood_spectrum_color(mood)
    } else {
        match mood {
            Some("孤独") => 0x4f46e5,
            Some("疲惫") => 0xb45309,
            Some("迷茫") => 0x0369a1,
            Some("失落") => 0x1d4ed8,
            Some("成长") => 0x047857,
            Some("希望") => 0xa16207,
            _ => 0xbe185d,
        }
    }
}

pub(super) fn circadian_greeting() -> (&'static str, &'static str, &'static str, bool) {
    let hour = time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
        .hour();
    match hour {
        5..=10 => (
            "MORNING SKY · 晨光慢读",
            "早安，乘着晨光，\n翻开今天的第一页。",
            "云层刚被阳光照亮，不赶时间，按自己的步子读几页吧。",
            false,
        ),
        11..=16 => (
            "AFTERNOON CLOUD · 午后云影",
            "午后的风很轻，\n适合躲进书页里。",
            "这里没有读完的期限。蓝天、白云和故事，都会等你。",
            false,
        ),
        17..=20 => (
            "TWILIGHT GLOW · 暮色微澜",
            "傍晚的光慢慢暗下来，\n让文字陪你歇一会儿。",
            "把白天的喧嚣留在门外，挑一首轻缓声景慢慢读。",
            false,
        ),
        _ => (
            "MIDNIGHT SANCTUARY · 深夜书灯",
            "夜深了，\n这盏小灯还在为你亮着。",
            "晚风安静，若觉得屏幕太亮，可以一键切到护眼的静夜空间。",
            true,
        ),
    }
}

pub(super) fn today_ymd() -> String {
    let dt = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!("{:04}-{:02}-{:02}", dt.year(), dt.month() as u8, dt.day())
}

pub(super) fn current_ym() -> (String, i32, u8) {
    let dt = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let year = dt.year();
    let month = dt.month() as u8;
    (format!("{year:04}-{month:02}"), year, month)
}

pub(super) fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Returns `(stage_label, should_enable_warm_evening_tint)` based on local clock hour.
pub(super) fn circadian_daylight_stage() -> (&'static str, bool) {
    let hour = time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
        .hour();
    match hour {
        5..=10 => ("晨光", false),
        11..=16 => ("晴午", false),
        17..=20 => ("暮色", true),
        _ => ("深夜", true),
    }
}

/// Gentle, zero-tracking circadian quiet-presence number for optional ambient companionship.
pub(super) fn ambient_presence_count() -> u32 {
    let dt = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let hour = dt.hour() as u32;
    let minute_bucket = (dt.minute() as u32) / 5;
    let base = match hour {
        21..=23 | 0..=1 => 46,
        18..=20 => 34,
        13..=17 => 26,
        7..=12 => 22,
        _ => 14,
    };
    base + ((hour * 7 + minute_bucket * 3) % 13)
}

pub(super) fn format_journal_markdown(entries: &[JournalEntry]) -> String {
    let mut out = String::from("# 微光手账 · Glow Reading Journal\n\n");
    for entry in entries {
        let tag = entry.mood.as_deref().unwrap_or("阅读回响");
        let star = if entry.favorite { " ★" } else { "" };
        out.push_str(&format!(
            "## {} · {}{}\n\n",
            display_time(&entry.created_at),
            tag,
            star
        ));
        if !entry.input.trim().is_empty() {
            out.push_str(&format!(
                "> {}\n\n",
                entry.input.trim().replace('\n', "\n> ")
            ));
        }
        for q in &entry.echo.quotes {
            out.push_str(&format!(
                "- 「{}」—— {}《{}》\n",
                q.text.trim(),
                q.author,
                q.work
            ));
        }
        if !entry.echo.quotes.is_empty() {
            out.push('\n');
        }
        out.push_str(entry.echo.reply.trim());
        out.push_str("\n\n---\n\n");
    }
    out
}

pub(super) fn display_time(raw: &str) -> String {
    let Ok(parsed) =
        time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
    else {
        return raw.to_owned();
    };
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    parsed
        .to_offset(offset)
        .format(&time::macros::format_description!(
            "[year].[month].[day] [hour]:[minute]"
        ))
        .unwrap_or_else(|_| raw.to_owned())
}

use super::*;
use crate::{
    prism::{RadioLampColor, RadioLampMode},
    radio::{self, NightSegment, RadioStation, RadioStationKind},
};
use glow_core::{CustomRadioStation, valid_radio_stream_url};

const GENRE_SEARCH_TAGS: &[(&str, &str)] = &[
    ("lofi", "Lofi 书房"),
    ("jazz", "午夜轻爵士"),
    ("piano", "轻柔钢琴"),
    ("classical", "古典夜曲"),
    ("ambient", "深空助眠"),
    ("chillout", "晚风轻音乐"),
];

const SLEEP_PRESETS_MINS: &[u32] = &[0, 15, 30, 45, 60];

impl ReadingApp {
    pub(super) fn ensure_radio(&mut self) -> &radio::RadioPlayer {
        self.radio_player.get_or_insert_with(radio::RadioPlayer::new)
    }

    pub(super) fn all_radio_stations(&self) -> Vec<RadioStation> {
        radio::all_stations(&self.store.state.radio.custom_stations)
    }

    pub(super) fn current_radio_station(&self) -> RadioStation {
        let stations = self.all_radio_stations();
        radio::station_by_id(&stations, &self.store.state.radio.current_station_id)
    }

    pub(super) fn stop_radio_if_playing(&mut self) {
        if let Some(player) = &self.radio_player {
            player.stop();
        }
        self.radio_status.playing = false;
        self.radio_status.paused = false;
        self.radio_status.tuning = false;
        self.radio_status.connecting = false;
        self.radio_status.host_speaking = false;
        self.radio_audio_level = 0.0;
        self.radio_sleep_started = None;
    }

    fn collect_book_echo_segments(&self) -> Vec<NightSegment> {
        let mut segments = Vec::new();
        for book in &self.store.state.books {
            for hl in book.highlights.iter().rev().take(6) {
                let text = hl.text.trim();
                if text.is_empty() {
                    continue;
                }
                let mut script = format!(
                    "书页回声私人台为你读起《{}》里曾触动过你的这一句：{}",
                    book.title, text
                );
                if let Some(note) = hl.note.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
                    script.push_str(&format!("。当时你写下的批注是：{}", note));
                }
                segments.push(NightSegment {
                    title: format!("《{}》划线回声", book.title),
                    script,
                    source_hint: format!("来自你的书架 · 《{}》", book.title),
                });
            }
        }
        for entry in self.store.state.journal.iter().filter(|e| e.favorite).take(6) {
            let reply = entry.echo.reply.trim();
            if !reply.is_empty() {
                segments.push(NightSegment {
                    title: "微光手账珍藏回响".into(),
                    script: format!("翻开你珍藏的一页微光手账：{}", reply),
                    source_hint: "来自你的私人日记收藏".into(),
                });
            }
        }
        if segments.is_empty() {
            radio::default_book_echo_segments()
        } else {
            segments
        }
    }

    pub(super) fn radio_sleep_remaining_and_dim(&self) -> (Option<u32>, f32) {
        let Some(started) = self.radio_sleep_started else {
            return (None, 1.0);
        };
        let elapsed = started.elapsed().as_secs_f32();
        let total = (self.radio_sleep_total_secs as f32).max(1.0);
        let remaining = (total - elapsed).max(0.0);
        // Smoothly dim both breathing light and radio volume during the last 60 seconds
        let dim = if remaining < 60.0 {
            (remaining / 60.0).clamp(0.08, 1.0)
        } else {
            1.0
        };
        (Some(remaining.ceil() as u32), dim)
    }

    fn set_radio_toast(&mut self, msg: impl Into<String>) {
        self.notice = Some(msg.into());
        self.radio_toast_at = Some(Instant::now());
    }

    pub(super) fn step_radio_motion(&mut self, reduced_motion: bool) -> bool {
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.radio_last_motion_at)
            .as_secs_f32()
            .clamp(0.004, 0.042);
        self.radio_last_motion_at = now;

        // Auto-fade transient floating radio notice pill without altering layout
        if self.page == Page::Radio
            && self.error.is_none()
            && self
                .radio_toast_at
                .is_some_and(|t| t.elapsed() >= Duration::from_millis(2800))
        {
            self.radio_toast_at = None;
            self.notice = None;
        }

        let playing_active = self.radio_status.playing && !self.radio_status.paused;

        // 1. Vinyl platter angular inertia (smoothly accelerates & gently coasts down)
        let target_vel = if playing_active && !reduced_motion {
            0.44_f32 // ~14.3 seconds per serene revolution
        } else {
            0.0_f32
        };
        if reduced_motion {
            self.radio_vinyl_vel = 0.0;
        } else {
            self.radio_vinyl_vel += (target_vel - self.radio_vinyl_vel) * (dt * 3.6).min(1.0);
            if self.radio_vinyl_vel.abs() < 0.001 && target_vel == 0.0 {
                self.radio_vinyl_vel = 0.0;
            }
            self.radio_vinyl_angle =
                (self.radio_vinyl_angle + self.radio_vinyl_vel * dt).rem_euclid(std::f32::consts::TAU);
        }

        // 2. Tonearm smooth pivot (0.0 parked off disc -> 1.0 engaged on grooves)
        let target_arm = if self.radio_dial_dragging {
            0.28_f32
        } else if playing_active {
            1.0_f32
        } else {
            0.0_f32
        };
        if reduced_motion || cfg!(test) {
            self.radio_tonearm_pos = target_arm;
        } else {
            self.radio_tonearm_pos += (target_arm - self.radio_tonearm_pos) * (dt * 6.4).min(1.0);
            if (target_arm - self.radio_tonearm_pos).abs() < 0.002 {
                self.radio_tonearm_pos = target_arm;
            }
        }

        // 3. Analog FM dial needle smooth glide
        let target_mhz = self.radio_drag_mhz.unwrap_or_else(|| {
            if self.radio_status.playing && self.radio_status.frequency_mhz > 0.0 {
                self.radio_status.frequency_mhz
            } else {
                self.current_radio_station().frequency_mhz
            }
        });
        if self.radio_dial_dragging || reduced_motion || cfg!(test) || self.radio_dial_display_mhz <= 0.0 {
            self.radio_dial_display_mhz = target_mhz;
        } else {
            self.radio_dial_display_mhz +=
                (target_mhz - self.radio_dial_display_mhz) * (dt * 12.0).min(1.0);
            if (target_mhz - self.radio_dial_display_mhz).abs() < 0.015 {
                self.radio_dial_display_mhz = target_mhz;
            }
        }

        if reduced_motion {
            return false;
        }
        self.page == Page::Radio
            && (playing_active
                || self.radio_vinyl_vel.abs() > 0.002
                || (target_arm - self.radio_tonearm_pos).abs() > 0.004
                || (target_mhz - self.radio_dial_display_mhz).abs() > 0.02
                || self.radio_dial_dragging
                || self.radio_toast_at.is_some())
    }

    fn mhz_from_dial_x(&self, x: Pixels) -> Option<f32> {
        let bounds = self.radio_dial_bounds.lock().ok().and_then(|g| *g)?;
        let pad_x = 20.0_f32;
        let track_w = (f32::from(bounds.size.width) - pad_x * 2.0).max(20.0);
        let frac = ((f32::from(x) - (f32::from(bounds.origin.x) + pad_x)) / track_w).clamp(0.0, 1.0);
        let mhz = 88.0 + frac * 20.0;
        Some(((mhz * 10.0).round() / 10.0).clamp(88.0, 108.0))
    }

    pub(super) fn begin_radio_dial_drag(
        &mut self,
        pos: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mhz = self
            .mhz_from_dial_x(pos.x)
            .unwrap_or_else(|| self.current_radio_station().frequency_mhz);
        self.radio_dial_dragging = true;
        self.radio_drag_mhz = Some(mhz);
        self.radio_dial_display_mhz = mhz;
        self.schedule_motion_frame(window, cx);
        cx.notify();
    }

    pub(super) fn update_radio_dial_drag(
        &mut self,
        pos: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.radio_dial_dragging {
            return;
        }
        if let Some(mhz) = self.mhz_from_dial_x(pos.x) {
            self.radio_drag_mhz = Some(mhz);
            self.radio_dial_display_mhz = mhz;
            self.schedule_motion_frame(window, cx);
        }
    }

    pub(super) fn finish_radio_dial_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.radio_dial_dragging {
            return;
        }
        self.radio_dial_dragging = false;
        let Some(mhz) = self.radio_drag_mhz.take() else {
            cx.notify();
            return;
        };
        let stations = self.all_radio_stations();
        if let Some(st) = radio::nearest_station_for_mhz(&stations, mhz) {
            if self.radio_status.playing && self.radio_status.station_id == st.id {
                // Already tuned to this station: smoothly snap the needle back without interrupting audio
                if cfg!(test) || self.theme().reduced_motion {
                    self.radio_dial_display_mhz = st.frequency_mhz;
                }
                self.schedule_motion_frame(window, cx);
                cx.notify();
            } else {
                self.act(Action::RadioTuneMhz(mhz), window, cx);
            }
        }
    }

    fn volume_from_bar_x(&self, x: Pixels) -> Option<f32> {
        let bounds = self.radio_vol_bounds.lock().ok().and_then(|g| *g)?;
        let pad = 7.0_f32;
        let track_w = (f32::from(bounds.size.width) - pad * 2.0).max(8.0);
        let frac = ((f32::from(x) - (f32::from(bounds.origin.x) + pad)) / track_w).clamp(0.0, 1.0);
        Some((0.05 + frac * 0.95).clamp(0.05, 1.0))
    }

    pub(super) fn begin_radio_vol_drag(
        &mut self,
        pos: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.radio_vol_dragging = true;
        if let Some(vol) = self.volume_from_bar_x(pos.x) {
            self.act(Action::RadioSetVolume(vol), window, cx);
        }
    }

    pub(super) fn update_radio_vol_drag(
        &mut self,
        pos: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if !self.radio_vol_dragging {
            return;
        }
        if let Some(vol) = self.volume_from_bar_x(pos.x) {
            self.store.state.radio.volume = vol;
            if let Some(p) = &self.radio_player {
                p.set_volume(vol);
            }
        }
    }

    pub(super) fn finish_radio_vol_drag(&mut self, cx: &mut Context<Self>) {
        if !self.radio_vol_dragging {
            return;
        }
        self.radio_vol_dragging = false;
        self.save();
        cx.notify();
    }

    pub(super) fn finish_radio_drags(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_radio_dial_drag(window, cx);
        self.finish_radio_vol_drag(cx);
    }

    pub(super) fn on_radio_dial_scroll(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.radio_last_wheel_at.elapsed() < Duration::from_millis(130) {
            return;
        }
        let delta = match event.delta {
            ScrollDelta::Pixels(p) => f32::from(p.y) + f32::from(p.x),
            ScrollDelta::Lines(p) => p.y + p.x,
        };
        if delta.abs() < 0.05 {
            return;
        }
        self.radio_last_wheel_at = Instant::now();
        if delta > 0.0 {
            self.act(Action::RadioPrevStation, window, cx);
        } else {
            self.act(Action::RadioNextStation, window, cx);
        }
    }

    fn tune_to_station(
        &mut self,
        station: RadioStation,
        with_static: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Mutual exclusion with reading ambient soundscape and audiobook TTS
        self.stop_audio_smoothly();
        if self.tts_status.active {
            if let Some(p) = &self.tts_player {
                p.stop();
            }
            self.tts_status.active = false;
            self.tts_status.paused = false;
            self.tts_status.loading = false;
        }

        self.store.state.radio.current_station_id = station.id.clone();
        self.save();

        let echoes = if station.kind == RadioStationKind::BookEchoHost {
            self.collect_book_echo_segments()
        } else {
            vec![]
        };
        let (init_title, init_line, init_hint) = radio::initial_copy_for_station(&station, &echoes);

        // Optimistically sync UI state so the dial needle and copy respond immediately with zero flash
        self.radio_status.playing = true;
        self.radio_status.paused = false;
        self.radio_status.tuning = with_static;
        self.radio_status.connecting = station.kind == RadioStationKind::OnlineStream;
        self.radio_status.fallback_offline = false;
        self.radio_status.station_id = station.id.clone();
        self.radio_status.station_name = station.name.clone();
        self.radio_status.frequency_mhz = station.frequency_mhz;
        self.radio_status.kind_label = station.kind.badge_label().into();
        self.radio_status.now_playing_title = init_title;
        self.radio_status.current_line = init_line;
        self.radio_status.source_hint = init_hint;
        self.radio_status.error = None;

        if cfg!(test) || self.theme().reduced_motion {
            self.radio_dial_display_mhz = station.frequency_mhz;
        }

        if !cfg!(test) {
            let vol = self.store.state.radio.volume.clamp(0.05, 1.0);
            let host_voice = self.store.state.radio.host_voice.clone();
            let local_folder = self
                .store
                .state
                .radio
                .local_audio_folder
                .as_deref()
                .map(PathBuf::from);
            self.ensure_radio()
                .tune(station, vol, host_voice, echoes, local_folder, with_static);
        }
        self.schedule_motion_frame(window, cx);
    }

    pub(super) fn tick_radio(&mut self, cx: &mut Context<Self>) {
        // 1. Poll online Radio-Browser search results
        if let Some(res) = self.radio_search_rx.as_ref().map(|rx| rx.try_recv()) {
            match res {
                Ok(Ok(hits)) => {
                    self.radio_search_rx = None;
                    let count = hits.len();
                    self.radio_search_results = hits;
                    self.set_radio_toast(if count == 0 {
                        "暂未搜到匹配的 MP3 电台，换个关键词或点选治愈标签试试。".into()
                    } else {
                        format!("已为你找到 {count} 个深夜电台，点击即可试听或收藏到刻度尺。")
                    });
                    cx.notify();
                }
                Ok(Err(err)) => {
                    self.radio_search_rx = None;
                    self.error = Some(err.to_string());
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.radio_search_rx = None;
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        // 2. Poll live RadioPlayer status & RMS energy
        if let Some(player) = &self.radio_player {
            let next_status = player.status();
            let next_level = player.audio_level();
            let status_changed = next_status.playing != self.radio_status.playing
                || next_status.paused != self.radio_status.paused
                || next_status.tuning != self.radio_status.tuning
                || next_status.connecting != self.radio_status.connecting
                || next_status.fallback_offline != self.radio_status.fallback_offline
                || next_status.station_id != self.radio_status.station_id
                || next_status.current_line != self.radio_status.current_line
                || next_status.source_hint != self.radio_status.source_hint
                || next_status.host_speaking != self.radio_status.host_speaking
                || next_status.error != self.radio_status.error;
            let level_changed = (next_level - self.radio_audio_level).abs() > 0.015;
            if let Some(err) = next_status.error.clone()
                && self.radio_status.error.as_deref() != Some(err.as_str())
            {
                self.error = Some(err);
            }
            self.radio_status = next_status;
            self.radio_audio_level = next_level;
            if status_changed || level_changed {
                cx.notify();
            }
        }

        // 3. Sleep-timer smooth fade-out (audio + lamp dimming synchronously)
        if let Some(started) = self.radio_sleep_started {
            let elapsed = started.elapsed().as_secs_f32();
            let total = (self.radio_sleep_total_secs as f32).max(1.0);
            if elapsed >= total {
                self.radio_sleep_started = None;
                self.stop_radio_if_playing();
                self.set_radio_toast("深夜电台伴眠定时已结束，呼吸灯已熄灭，晚安。");
                cx.notify();
            } else {
                let (_, dim) = self.radio_sleep_remaining_and_dim();
                if let Some(player) = &self.radio_player {
                    player.set_fade_multiplier(dim);
                }
                cx.notify();
            }
        }
    }

    pub(super) fn perform_radio(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        match action {
            Action::RadioTogglePlay => {
                if self.radio_status.playing {
                    if self.radio_status.paused {
                        if let Some(p) = &self.radio_player {
                            p.resume();
                        }
                        self.radio_status.paused = false;
                        self.schedule_motion_frame(window, cx);
                    } else {
                        if let Some(p) = &self.radio_player {
                            p.pause();
                        }
                        self.radio_status.paused = true;
                        self.radio_audio_level = 0.0;
                        self.schedule_motion_frame(window, cx);
                    }
                } else {
                    let st = self.current_radio_station();
                    self.tune_to_station(st, true, window, cx);
                }
            }
            Action::RadioTuneStation(station_id) => {
                let stations = self.all_radio_stations();
                let st = radio::station_by_id(&stations, &station_id);
                let title = st.name.clone();
                let mhz = st.frequency_mhz;
                self.tune_to_station(st, true, window, cx);
                self.set_radio_toast(format!("已调频至 FM {mhz:.1} · {title}"));
            }
            Action::RadioTuneMhz(mhz) => {
                let stations = self.all_radio_stations();
                if let Some(st) = radio::nearest_station_for_mhz(&stations, mhz) {
                    let title = st.name.clone();
                    let actual = st.frequency_mhz;
                    self.tune_to_station(st, true, window, cx);
                    self.set_radio_toast(format!("已吸附至 FM {actual:.1} · {title}"));
                }
            }
            Action::RadioPrevStation => {
                let stations = self.all_radio_stations();
                if !stations.is_empty() {
                    let cur_idx = stations
                        .iter()
                        .position(|s| s.id == self.store.state.radio.current_station_id)
                        .unwrap_or(0);
                    let prev_idx = if cur_idx == 0 {
                        stations.len() - 1
                    } else {
                        cur_idx - 1
                    };
                    let st = stations[prev_idx].clone();
                    let label = format!("FM {:.1} · {}", st.frequency_mhz, st.name);
                    self.tune_to_station(st, true, window, cx);
                    self.set_radio_toast(format!("已调频至 {label}"));
                }
            }
            Action::RadioNextStation => {
                let stations = self.all_radio_stations();
                if !stations.is_empty() {
                    let cur_idx = stations
                        .iter()
                        .position(|s| s.id == self.store.state.radio.current_station_id)
                        .unwrap_or(0);
                    let next_idx = (cur_idx + 1) % stations.len();
                    let st = stations[next_idx].clone();
                    let label = format!("FM {:.1} · {}", st.frequency_mhz, st.name);
                    self.tune_to_station(st, true, window, cx);
                    self.set_radio_toast(format!("已调频至 {label}"));
                }
            }
            Action::RadioNextSegment => {
                if let Some(p) = &self.radio_player {
                    p.next_segment();
                } else {
                    let st = self.current_radio_station();
                    self.tune_to_station(st, false, window, cx);
                }
            }
            Action::RadioToggleFavorite(id) => {
                if let Some(pos) = self
                    .store
                    .state
                    .radio
                    .favorites
                    .iter()
                    .position(|f| f == &id)
                {
                    self.store.state.radio.favorites.remove(pos);
                    self.set_radio_toast("已从红心频段移出。");
                } else {
                    self.store.state.radio.favorites.push(id);
                    self.set_radio_toast("已点亮红心收藏，刻度尺上会为你保留这颗星点。");
                }
                self.save();
            }
            Action::RadioLampColor(color) => {
                let resolved = RadioLampColor::from_id(&color);
                self.store.state.radio.lamp_color = resolved.id().into();
                self.save();
                self.schedule_motion_frame(window, cx);
                self.set_radio_toast(format!("氛围灯色温已切至「{}」。", resolved.label()));
            }
            Action::RadioLampMode(mode) => {
                let resolved = RadioLampMode::from_id(&mode);
                self.store.state.radio.lamp_mode = resolved.id().into();
                if resolved == RadioLampMode::Breathe478 {
                    self.radio_breathe_started = Instant::now();
                }
                self.save();
                self.schedule_motion_frame(window, cx);
                self.set_radio_toast(format!(
                    "呼吸灯律动已切换为「{}」（{}）。",
                    resolved.label(),
                    resolved.hint()
                ));
            }
            Action::RadioVolume(delta) => {
                let steps =
                    (self.store.state.radio.volume * 20.0).round() as i32 + delta as i32;
                let clamped = (steps as f32 / 20.0).clamp(0.05, 1.0);
                self.store.state.radio.volume = clamped;
                if let Some(p) = &self.radio_player {
                    p.set_volume(clamped);
                }
                self.save();
            }
            Action::RadioSetVolume(vol) => {
                let clamped = vol.clamp(0.05, 1.0);
                self.store.state.radio.volume = clamped;
                if let Some(p) = &self.radio_player {
                    p.set_volume(clamped);
                }
                self.save();
            }
            Action::RadioTogglePureGlow => {
                self.radio_pure_glow = !self.radio_pure_glow;
                if self.radio_pure_glow {
                    self.radio_drawer_open = false;
                }
                self.schedule_motion_frame(window, cx);
            }
            Action::RadioToggleImmersive => {
                self.radio_hide_sidebar = !self.radio_hide_sidebar;
                if !self.radio_hide_sidebar {
                    self.radio_pure_glow = false;
                }
                self.schedule_motion_frame(window, cx);
            }
            Action::RadioToggleVisualMode => {
                self.radio_visual_mode = if self.radio_visual_mode == 0 { 1 } else { 0 };
                self.schedule_motion_frame(window, cx);
                self.set_radio_toast(if self.radio_visual_mode == 0 {
                    "已切至「旋转黑胶唱片」沉浸主视觉。"
                } else {
                    "已切至「治愈呼吸光球」床头氛围主视觉。"
                });
            }
            Action::RadioCycleSleepTimer => {
                let cur_mins = if self.radio_sleep_started.is_some() {
                    self.radio_sleep_total_secs / 60
                } else {
                    0
                };
                let next_idx = SLEEP_PRESETS_MINS
                    .iter()
                    .position(|&m| m == cur_mins)
                    .map(|i| (i + 1) % SLEEP_PRESETS_MINS.len())
                    .unwrap_or(1);
                let next_mins = SLEEP_PRESETS_MINS[next_idx];
                if next_mins == 0 {
                    self.radio_sleep_started = None;
                    if let Some(p) = &self.radio_player {
                        p.set_fade_multiplier(1.0);
                    }
                    self.set_radio_toast("已关闭电台伴眠定时。");
                } else {
                    self.radio_sleep_total_secs = next_mins * 60;
                    self.radio_sleep_started = Some(Instant::now());
                    if let Some(p) = &self.radio_player {
                        p.set_fade_multiplier(1.0);
                    }
                    self.schedule_motion_frame(window, cx);
                    self.set_radio_toast(format!(
                        "已开启 {next_mins} 分钟伴眠定时：最后 1 分钟音量与氛围灯将同步渐暗。"
                    ));
                }
            }
            Action::RadioToggleDrawer => {
                self.radio_drawer_open = !self.radio_drawer_open;
                if self.radio_drawer_open {
                    self.radio_pure_glow = false;
                    self.palette_open = false;
                    self.mixer_open = false;
                    self.focus_timer_picker_open = false;
                } else {
                    window.focus(&self.focus, cx);
                }
            }
            Action::RadioCloseDrawer => {
                self.radio_drawer_open = false;
                window.focus(&self.focus, cx);
            }
            Action::RadioDrawerTab(tab) => {
                self.radio_drawer_tab = tab.min(2);
            }
            Action::RadioToggleFavFilter => {
                self.radio_only_favorites = !self.radio_only_favorites;
            }
            Action::RadioSearchSubmit => {
                if self.radio_search_rx.is_some() {
                    return Ok(());
                }
                let query = self.radio_search_input.read(cx).value().trim().to_owned();
                anyhow::ensure!(
                    !query.is_empty(),
                    "请输入想搜索的电台名称或风格关键词（如 lofi、jazz、piano）"
                );
                let (tx, rx) = mpsc::channel();
                self.radio_search_rx = Some(rx);
                self.set_radio_toast(format!("正在电台库中搜索「{query}」…"));
                std::thread::spawn(move || {
                    let _ = tx.send(radio::search_online_stations(&query, None));
                });
            }
            Action::RadioSearchTag(tag) => {
                if self.radio_search_rx.is_some() {
                    return Ok(());
                }
                let tag_owned = tag.to_owned();
                let (tx, rx) = mpsc::channel();
                self.radio_search_rx = Some(rx);
                self.set_radio_toast(format!("正在寻找「{tag}」治愈频段…"));
                std::thread::spawn(move || {
                    let _ = tx.send(radio::search_online_stations("", Some(&tag_owned)));
                });
            }
            Action::RadioPreviewHit(idx) => {
                if let Some(hit) = self.radio_search_results.get(idx).cloned() {
                    let temp = RadioStation {
                        id: hit.id.clone(),
                        frequency_mhz: hit.suggested_mhz,
                        name: hit.name.clone(),
                        subtitle: if hit.country.is_empty() {
                            format!("在线搜索试听 · {} kbps MP3", hit.bitrate)
                        } else {
                            format!("{} · {} kbps MP3", hit.country, hit.bitrate)
                        },
                        mood_line: format!("正在试听「{}」，如果喜欢可点击收藏固定到刻度尺。", hit.name),
                        kind: RadioStationKind::OnlineStream,
                        stream_urls: vec![hit.stream_url],
                        suggested_lamp: "amber",
                        tags: hit.tags,
                        is_custom: true,
                    };
                    self.tune_to_station(temp, true, window, cx);
                }
            }
            Action::RadioAddHit(idx) => {
                if let Some(hit) = self.radio_search_results.get(idx).cloned() {
                    let exists = self
                        .store
                        .state
                        .radio
                        .custom_stations
                        .iter()
                        .any(|s| s.id == hit.id || s.stream_url == hit.stream_url);
                    if !exists {
                        self.store
                            .state
                            .radio
                            .custom_stations
                            .push(CustomRadioStation {
                                id: hit.id.clone(),
                                name: hit.name.clone(),
                                subtitle: if hit.country.is_empty() {
                                    "在线收藏电台".into()
                                } else {
                                    format!("{} · {} kbps", hit.country, hit.bitrate)
                                },
                                frequency_mhz: hit.suggested_mhz,
                                kind: "online".into(),
                                stream_url: hit.stream_url.clone(),
                                tags: hit.tags.clone(),
                            });
                    }
                    if !self.store.state.radio.favorites.iter().any(|f| f == &hit.id) {
                        self.store.state.radio.favorites.push(hit.id.clone());
                    }
                    self.save();
                    let stations = self.all_radio_stations();
                    let st = radio::station_by_id(&stations, &hit.id);
                    self.tune_to_station(st, true, window, cx);
                    self.set_radio_toast(format!(
                        "已将「{}」收藏至 FM {:.1} 刻度尺。",
                        hit.name, hit.suggested_mhz
                    ));
                }
            }
            Action::RadioAddCustomStream => {
                let name = self
                    .radio_custom_name_input
                    .read(cx)
                    .value()
                    .trim()
                    .to_owned();
                let url = self
                    .radio_custom_url_input
                    .read(cx)
                    .value()
                    .trim()
                    .to_owned();
                anyhow::ensure!(!name.is_empty(), "请为自定义电台起一个名字");
                anyhow::ensure!(
                    valid_radio_stream_url(&url),
                    "请输入合法的 http:// 或 https:// 音频流地址（推荐 MP3 / Ogg）"
                );
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                let id = format!("custom-{stamp}");
                let mhz = radio::deterministic_mhz(&url);
                self.store
                    .state
                    .radio
                    .custom_stations
                    .push(CustomRadioStation {
                        id: id.clone(),
                        name: name.clone(),
                        subtitle: "我的自定义电台流".into(),
                        frequency_mhz: mhz,
                        kind: "online".into(),
                        stream_url: url,
                        tags: vec!["自定义".into()],
                    });
                self.store.state.radio.favorites.push(id.clone());
                self.save();
                self.radio_custom_name_input
                    .update(cx, |s, cx| s.set_value("", window, cx));
                self.radio_custom_url_input
                    .update(cx, |s, cx| s.set_value("", window, cx));
                let stations = self.all_radio_stations();
                let st = radio::station_by_id(&stations, &id);
                self.tune_to_station(st, true, window, cx);
                self.set_radio_toast(format!("已收藏并调频至自定义电台「{name}」(FM {mhz:.1})。"));
            }
            Action::RadioRemoveCustomStation(id) => {
                self.store
                    .state
                    .radio
                    .custom_stations
                    .retain(|s| s.id != id);
                self.store.state.radio.favorites.retain(|f| f != &id);
                if self.store.state.radio.current_station_id == id {
                    self.store.state.radio.current_station_id = "glow-night-talk".into();
                }
                self.save();
                self.set_radio_toast("已移出该自定义电台频段。");
            }
            Action::RadioPickLocalFolder => {
                self.hide(cx);
                if let Some(dir) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("选择私藏本地音乐或播客文件夹（支持 MP3 / WAV / OGG）")
                    .pick_folder()
                {
                    let tracks = radio::scan_local_audio_files(&dir);
                    let count = tracks.len();
                    self.store.state.radio.local_audio_folder =
                        Some(dir.to_string_lossy().into_owned());
                    self.save();
                    let stations = self.all_radio_stations();
                    let st = radio::station_by_id(&stations, "local-tape-fm");
                    self.tune_to_station(st, true, window, cx);
                    self.set_radio_toast(if count > 0 {
                        format!("已载入本地私藏磁带目录，共发现 {count} 首音频，正在温柔连播。")
                    } else {
                        "已保存该目录，但未找到 MP3 / WAV / OGG 文件，暂以雨夜炉火陪伴。".into()
                    });
                }
            }
            Action::RadioSetHostVoice(voice_id) => {
                self.store.state.radio.host_voice = voice_id.clone();
                self.save();
                let label = tts::voice_by_id(&voice_id).label;
                let cur = self.current_radio_station();
                if self.radio_status.playing
                    && matches!(
                        cur.kind,
                        RadioStationKind::NightHost | RadioStationKind::BookEchoHost
                    )
                {
                    self.tune_to_station(cur, false, window, cx);
                }
                self.set_radio_toast(format!("深夜电台主播音色已切为「{label}」。"));
            }
            Action::RadioSaveLineToJournal => {
                let cur = self.current_radio_station();
                let line = if !self.radio_status.current_line.trim().is_empty()
                    && self.radio_status.station_id == cur.id
                {
                    self.radio_status.current_line.trim().to_owned()
                } else {
                    cur.mood_line.clone()
                };
                anyhow::ensure!(!line.is_empty(), "当前没有可收藏的夜话句子");
                let title = if self.radio_status.now_playing_title.is_empty() {
                    cur.name
                } else {
                    self.radio_status.now_playing_title.clone()
                };
                self.store.record(
                    "mood",
                    format!("深夜电台 · {title}"),
                    Some("安静".into()),
                    EchoResult {
                        quotes: vec![],
                        reply: line,
                        demo: false,
                    },
                    None,
                )?;
                self.set_radio_toast("已把这句深夜电台夜话温柔收入私人手账。");
            }
            _ => {}
        }
        Ok(())
    }

    // ================= UI: Sidebar FM ON AIR Mini Companion Strip =================
    #[inline(never)]
    pub(super) fn render_sidebar_radio_mini(
        &self,
        t: &ReadingTheme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let playing_now = self.radio_status.playing && !self.radio_status.paused;
        let lamp_color = RadioLampColor::from_id(&self.store.state.radio.lamp_color);
        let (primary_hex, _, _, _) = lamp_color.palette();
        let st_mhz = if self.radio_status.frequency_mhz > 0.0 {
            self.radio_status.frequency_mhz
        } else {
            self.current_radio_station().frequency_mhz
        };
        let st_name = if self.radio_status.station_name.is_empty() {
            self.current_radio_station().name
        } else {
            self.radio_status.station_name.clone()
        };

        v_flex()
            .id("sidebar-radio-mini")
            .debug_selector(|| "sidebar-radio-mini".into())
            .gap_1()
            .px_2p5()
            .py_2()
            .rounded_xl()
            .bg(rgb(primary_hex).opacity(0.14))
            .border_1()
            .border_color(rgb(primary_hex).opacity(0.42))
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_1p5()
                    .child(
                        h_flex()
                            .id("sidebar-radio-open")
                            .cursor_pointer()
                            .flex_1()
                            .min_w_0()
                            .gap_1p5()
                            .items_center()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.act(Action::Page(Page::Radio), window, cx);
                                }),
                            )
                            .child(
                                div()
                                    .size(px(6.))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .bg(rgb(if playing_now {
                                        primary_hex
                                    } else {
                                        hex(&t.muted)
                                    })),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(9.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(primary_hex))
                                    .text_ellipsis()
                                    .child(format!("FM {st_mhz:.1} · {st_name}")),
                            ),
                    )
                    .child(
                        div()
                            .id("sidebar-radio-toggle")
                            .cursor_pointer()
                            .flex_shrink_0()
                            .px_1p5()
                            .py_0p5()
                            .rounded_lg()
                            .bg(rgb(primary_hex).opacity(0.24))
                            .text_size(px(8.5))
                            .font_family(self.ui_font())
                            .text_color(rgb(primary_hex))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.act(Action::RadioTogglePlay, window, cx);
                                }),
                            )
                            .child(if playing_now { "暂停" } else { "继续" }),
                    ),
            )
            .child(
                h_flex()
                    .id("sidebar-radio-return")
                    .cursor_pointer()
                    .items_center()
                    .justify_between()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.act(Action::Page(Page::Radio), window, cx);
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(8.))
                            .font_family(self.ui_font())
                            .text_color(rgb(hex(&t.muted)))
                            .child(if playing_now {
                                "后台伴听中 · 点此回到电台"
                            } else {
                                "电台已暂停 · 点此回到电台"
                            }),
                    )
                    .child(self.waveform_bars(playing_now, &[4., 9., 6., 11., 5.])),
            )
            .into_any_element()
    }

    // ================= UI: Glow FM Immersive Sanctuary Page =================
    #[inline(never)]
    pub(super) fn radio_page(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let snap = self.motion_snapshot();
        let lamp_color = RadioLampColor::from_id(&self.store.state.radio.lamp_color);
        let lamp_mode = RadioLampMode::from_id(&self.store.state.radio.lamp_mode);
        let (primary_hex, core_hex, _outer_hex, stage_bg_hex) = lamp_color.palette();
        let stations = self.all_radio_stations();
        let current = self.current_radio_station();
        let playing_active = self.radio_status.playing && !self.radio_status.paused;
        let is_fav = self
            .store
            .state
            .radio
            .favorites
            .iter()
            .any(|f| f == &current.id);
        let (sleep_rem, dim_factor) = self.radio_sleep_remaining_and_dim();
        let breathe_elapsed = self.radio_breathe_started.elapsed().as_secs_f32();
        let station_scene = radio::station_scene_asset(&current.id);

        let station_mhz = if self.radio_status.playing && self.radio_status.frequency_mhz > 0.0 {
            self.radio_status.frequency_mhz
        } else {
            current.frequency_mhz
        };
        let dial_mhz = if self.radio_dial_dragging {
            self.radio_drag_mhz.unwrap_or(self.radio_dial_display_mhz)
        } else if self.radio_dial_display_mhz >= 88.0 && self.radio_dial_display_mhz <= 108.0 {
            self.radio_dial_display_mhz
        } else {
            station_mhz
        };
        let drag_nearest = if self.radio_dial_dragging {
            radio::nearest_station_for_mhz(&stations, dial_mhz)
        } else {
            None
        };
        let locked_marker_mhz = drag_nearest
            .as_ref()
            .map(|s| s.frequency_mhz)
            .or(Some(station_mhz));

        let display_mhz = if self.radio_dial_dragging {
            dial_mhz
        } else {
            station_mhz
        };
        let display_name = if self.radio_status.playing && !self.radio_status.station_name.is_empty() {
            self.radio_status.station_name.clone()
        } else {
            current.name.clone()
        };
        let (fallback_title, fallback_line, fallback_hint) =
            radio::initial_copy_for_station(&current, &[]);
        let display_title = if !self.radio_status.now_playing_title.is_empty()
            && self.radio_status.station_id == current.id
        {
            self.radio_status.now_playing_title.clone()
        } else {
            fallback_title
        };
        let display_line = if !self.radio_status.current_line.is_empty()
            && self.radio_status.station_id == current.id
        {
            self.radio_status.current_line.clone()
        } else {
            fallback_line
        };
        let display_hint = if !self.radio_status.source_hint.is_empty()
            && self.radio_status.station_id == current.id
        {
            self.radio_status.source_hint.clone()
        } else {
            fallback_hint
        };

        let text_warm = rgb(0xf6efe6);
        let text_muted = rgb(0xaba197);
        let primary = rgb(primary_hex);
        let core = rgb(core_hex);

        // ---- Pure-Glow Zen Mode (Minimalist Full-Room Bedside Breathing Lamp Only) ----
        if self.radio_pure_glow {
            let bp_cue = if lamp_mode == RadioLampMode::Breathe478 {
                let bp = prism::breathing_phase(breathe_elapsed);
                Some(format!("{} · {}s  ·  {}", bp.label, bp.seconds_left, bp.cue))
            } else {
                None
            };
            return v_flex()
                .id("radio-pure-glow-stage")
                .debug_selector(|| "radio-pure-glow-stage".into())
                .relative()
                .size_full()
                .overflow_hidden()
                .items_center()
                .justify_between()
                .p_8()
                .bg(rgb(stage_bg_hex))
                .child(
                    img(station_scene)
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .opacity(0.11 * dim_factor),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .size_full()
                        .bg(rgb(stage_bg_hex).opacity(0.84)),
                )
                .child(prism::radio_ambient_room_canvas(
                    snap.clone(),
                    lamp_color,
                    lamp_mode,
                    self.radio_audio_level,
                    breathe_elapsed,
                    playing_active,
                    dim_factor,
                    true,
                ))
                .child(
                    h_flex()
                        .relative()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_family(self.ui_font())
                                .text_color(text_muted.opacity(0.55 * dim_factor))
                                .child(if let Some(rem) = sleep_rem {
                                    format!("伴眠渐暗 {:02}:{:02}", rem / 60, rem % 60)
                                } else {
                                    local_clock_hm()
                                }),
                        )
                        .child(
                            div()
                                .id("radio-exit-pure-glow")
                                .debug_selector(|| "radio-exit-pure-glow".into())
                                .cursor_pointer()
                                .px_3p5()
                                .py_1p5()
                                .rounded_full()
                                .bg(rgb(0xffffff).opacity(0.07))
                                .border_1()
                                .border_color(rgb(0xffffff).opacity(0.14))
                                .hover(|s| s.bg(rgb(0xffffff).opacity(0.14)))
                                .text_size(px(11.))
                                .font_family(self.ui_font())
                                .text_color(text_warm.opacity(0.78))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.act(Action::RadioTogglePureGlow, window, cx);
                                    }),
                                )
                                .child("唤醒界面 · Esc"),
                        ),
                )
                .child(
                    v_flex()
                        .relative()
                        .items_center()
                        .gap_5()
                        .child(
                            div()
                                .id("radio-pure-orb-hit")
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                        if event.click_count >= 2 {
                                            this.act(Action::RadioTogglePureGlow, window, cx);
                                        }
                                    }),
                                )
                                .child(prism::radio_breathing_lamp_canvas(
                                    snap,
                                    lamp_color,
                                    lamp_mode,
                                    self.radio_audio_level,
                                    breathe_elapsed,
                                    playing_active,
                                    dim_factor,
                                    368.0,
                                )),
                        )
                        .child(
                            v_flex()
                                .items_center()
                                .gap_2()
                                .max_w(px(560.))
                                .child(
                                    div()
                                        .text_size(px(20.))
                                        .font_family(self.display_font())
                                        .text_color(core.opacity(0.78 * dim_factor))
                                        .child(format!("FM {display_mhz:.1}  ·  {display_name}")),
                                )
                                .child(
                                    div()
                                        .h(px(22.))
                                        .text_size(px(12.))
                                        .font_family(self.ui_font())
                                        .text_color(primary.opacity(0.82 * dim_factor))
                                        .child(bp_cue.unwrap_or_default()),
                                )
                                .child(
                                    div()
                                        .h(px(74.))
                                        .overflow_hidden()
                                        .text_center()
                                        .text_size(px(14.))
                                        .font_family(self.display_font())
                                        .line_height(relative(1.75))
                                        .text_color(text_warm.opacity(0.60 * dim_factor))
                                        .child(display_line),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .relative()
                        .gap_3()
                        .items_center()
                        .px_5()
                        .py_2()
                        .rounded_full()
                        .bg(rgb(0xffffff).opacity(0.06))
                        .border_1()
                        .border_color(rgb(0xffffff).opacity(0.11))
                        .child(
                            div()
                                .id("pure-prev-st")
                                .cursor_pointer()
                                .px_2()
                                .text_size(px(11.5))
                                .font_family(self.ui_font())
                                .text_color(text_muted.opacity(0.78))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.act(Action::RadioPrevStation, window, cx);
                                    }),
                                )
                                .child("上一频段"),
                        )
                        .child(
                            div()
                                .id("pure-play-toggle")
                                .cursor_pointer()
                                .w(px(64.))
                                .flex()
                                .justify_center()
                                .py_1()
                                .rounded_full()
                                .bg(primary.opacity(0.24 * dim_factor))
                                .text_size(px(11.5))
                                .font_family(self.ui_font())
                                .text_color(core.opacity(0.92 * dim_factor))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.act(Action::RadioTogglePlay, window, cx);
                                    }),
                                )
                                .child(if playing_active { "暂停" } else { "开播" }),
                        )
                        .child(
                            div()
                                .id("pure-next-st")
                                .cursor_pointer()
                                .px_2()
                                .text_size(px(11.5))
                                .font_family(self.ui_font())
                                .text_color(text_muted.opacity(0.78))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.act(Action::RadioNextStation, window, cx);
                                    }),
                                )
                                .child("下一频段"),
                        ),
                )
                .into_any_element();
        }

        // ---- Immersive Full-Viewport Glow FM Vinyl × Breathing Lamp Sanctuary ----
        let vp = window.viewport_size();
        let vp_w = f32::from(vp.width).max(640.0);
        let vp_h = f32::from(vp.height).max(520.0);
        let avail_w = if self.radio_hide_sidebar {
            vp_w
        } else {
            (vp_w - 240.0).max(540.0)
        };
        let compact = vp_h < 760.0 || avail_w < 940.0;
        let stage_size = if compact { 324.0_f32 } else { 404.0_f32 };
        let cover_d = (stage_size * 0.375 * 0.47 * 2.0).round();

        let status_badge = if self.radio_dial_dragging {
            "手动调频中"
        } else if self.radio_status.connecting {
            "信号接入中…"
        } else if self.radio_status.tuning {
            "电波锁频中…"
        } else if self.radio_status.fallback_offline {
            "离线微光代播"
        } else if playing_active {
            if self.radio_status.host_speaking {
                "ON AIR · 夜话中"
            } else {
                "ON AIR · 播映中"
            }
        } else if self.radio_status.playing && self.radio_status.paused {
            "PAUSED · 已暂停"
        } else {
            "STANDBY · 待开播"
        };

        let sleep_label = if let Some(rem) = sleep_rem {
            format!("伴眠 {:02}:{:02}", rem / 60, rem % 60)
        } else {
            "伴眠定时".into()
        };

        // 1. Fixed-height floating glass header bar (54px)
        let mut top_right = h_flex().gap_2().items_center().flex_shrink_0();
        top_right = top_right
            .child(
                h_flex()
                    .id("radio-sleep-btn")
                    .debug_selector(|| "radio-sleep-btn".into())
                    .cursor_pointer()
                    .gap_1p5()
                    .items_center()
                    .px_3()
                    .py_1p5()
                    .rounded_full()
                    .bg(if sleep_rem.is_some() {
                        primary.opacity(0.22)
                    } else {
                        rgb(0xffffff).opacity(0.06)
                    })
                    .border_1()
                    .border_color(if sleep_rem.is_some() {
                        primary.opacity(0.55)
                    } else {
                        rgb(0xffffff).opacity(0.11)
                    })
                    .hover(|s| s.border_color(primary.opacity(0.6)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.act(Action::RadioCycleSleepTimer, window, cx);
                        }),
                    )
                    .child(
                        Icon::new(IconName::Timer)
                            .size(px(12.5))
                            .text_color(if sleep_rem.is_some() { core } else { text_muted }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(if sleep_rem.is_some() { core } else { text_warm })
                            .child(sleep_label),
                    ),
            )
            .child(
                h_flex()
                    .id("radio-pure-glow-btn")
                    .debug_selector(|| "radio-pure-glow-btn".into())
                    .cursor_pointer()
                    .gap_1p5()
                    .items_center()
                    .px_3()
                    .py_1p5()
                    .rounded_full()
                    .bg(rgb(0xffffff).opacity(0.06))
                    .border_1()
                    .border_color(rgb(0xffffff).opacity(0.11))
                    .hover(|s| s.bg(primary.opacity(0.18)).border_color(primary.opacity(0.5)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.act(Action::RadioTogglePureGlow, window, cx);
                        }),
                    )
                    .child(Icon::new(IconName::Moon).size(px(12.5)).text_color(core))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(text_warm)
                            .child("纯享微光"),
                    ),
            );
        if self.active_book.is_some() {
            top_right = top_right.child(
                h_flex()
                    .id("radio-back-reader")
                    .cursor_pointer()
                    .gap_1p5()
                    .items_center()
                    .px_3()
                    .py_1p5()
                    .rounded_full()
                    .bg(primary.opacity(0.15))
                    .border_1()
                    .border_color(primary.opacity(0.38))
                    .hover(|s| s.bg(primary.opacity(0.24)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.act(Action::Page(Page::Reader), window, cx);
                        }),
                    )
                    .child(Icon::new(IconName::BookOpen).size(px(12.5)).text_color(core))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(core)
                            .child("回到阅读"),
                    ),
            );
        }

        let top_bar = h_flex()
            .relative()
            .w_full()
            .h(px(54.))
            .flex_shrink_0()
            .px_6()
            .justify_between()
            .items_center()
            .border_b_1()
            .border_color(rgb(0xffffff).opacity(0.07))
            .child(
                h_flex()
                    .gap_2p5()
                    .items_center()
                    .child(
                        h_flex()
                            .id("radio-back-shelf")
                            .debug_selector(|| "radio-back-shelf".into())
                            .cursor_pointer()
                            .gap_1p5()
                            .items_center()
                            .px_3()
                            .py_1p5()
                            .rounded_full()
                            .bg(rgb(0xffffff).opacity(0.06))
                            .border_1()
                            .border_color(rgb(0xffffff).opacity(0.11))
                            .hover(|s| s.bg(rgb(0xffffff).opacity(0.12)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.act(Action::Page(Page::Shelf), window, cx);
                                }),
                            )
                            .child(
                                Icon::new(IconName::ArrowLeft)
                                    .size(px(12.5))
                                    .text_color(text_warm),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(text_warm)
                                    .child("返回书架"),
                            ),
                    )
                    .child(
                        h_flex()
                            .id("radio-immersive-btn")
                            .debug_selector(|| "radio-immersive-btn".into())
                            .cursor_pointer()
                            .gap_1p5()
                            .items_center()
                            .px_2p5()
                            .py_1p5()
                            .rounded_full()
                            .bg(if !self.radio_hide_sidebar {
                                primary.opacity(0.20)
                            } else {
                                rgb(0xffffff).opacity(0.05)
                            })
                            .border_1()
                            .border_color(if !self.radio_hide_sidebar {
                                primary.opacity(0.45)
                            } else {
                                rgb(0xffffff).opacity(0.10)
                            })
                            .hover(|s| s.border_color(primary.opacity(0.48)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.act(Action::RadioToggleImmersive, window, cx);
                                }),
                            )
                            .child(
                                Icon::new(if self.radio_hide_sidebar {
                                    IconName::PanelLeftOpen
                                } else {
                                    IconName::PanelLeftClose
                                })
                                .size(px(12.5))
                                .text_color(text_muted),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(text_muted)
                                    .child(if self.radio_hide_sidebar {
                                        "侧栏"
                                    } else {
                                        "收起侧栏"
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .w(px(122.))
                            .justify_center()
                            .gap_1p5()
                            .items_center()
                            .px_2p5()
                            .py_1()
                            .rounded_full()
                            .bg(primary.opacity(0.15))
                            .border_1()
                            .border_color(primary.opacity(0.36))
                            .child(
                                div()
                                    .size(px(6.))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .bg(if playing_active { primary } else { text_muted }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(core)
                                    .child(status_badge),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .text_color(text_muted.opacity(0.85))
                            .child(format!("GLOW FM · {}", local_clock_hm())),
                    ),
            )
            .child(top_right);

        // 2. Left Hero Visual Stage: Spinning Vinyl Turntable OR Breathing Glow Orb (fixed size!)
        let turntable_visual: AnyElement = if self.radio_visual_mode == 0 {
            div()
                .relative()
                .size(px(stage_size))
                .flex()
                .items_center()
                .justify_center()
                .child(prism::radio_vinyl_underlay_canvas(
                    snap.clone(),
                    lamp_color,
                    lamp_mode,
                    self.radio_audio_level,
                    breathe_elapsed,
                    self.radio_vinyl_angle,
                    playing_active,
                    self.radio_dial_dragging,
                    dim_factor,
                    stage_size,
                ))
                .child(
                    div()
                        .absolute()
                        .size(px(cover_d))
                        .rounded(px(cover_d * 0.5))
                        .overflow_hidden()
                        .child(prism::radio_vinyl_cover_canvas(
                            station_scene,
                            self.radio_vinyl_angle,
                            dim_factor,
                            cover_d,
                            self.theme().reduced_motion,
                            Arc::clone(&self.radio_cover_cache),
                        )),
                )
                .child(prism::radio_vinyl_overlay_canvas(
                    snap.clone(),
                    lamp_color,
                    self.radio_audio_level,
                    self.radio_vinyl_angle,
                    self.radio_tonearm_pos,
                    playing_active,
                    dim_factor,
                    stage_size,
                ))
                .into_any_element()
        } else {
            prism::radio_breathing_lamp_canvas(
                snap.clone(),
                lamp_color,
                lamp_mode,
                self.radio_audio_level,
                breathe_elapsed,
                playing_active,
                dim_factor,
                stage_size,
            )
            .into_any_element()
        };

        let left_hero = v_flex()
            .flex_shrink_0()
            .w(px(stage_size))
            .items_center()
            .gap_1p5()
            .child(
                div()
                    .id("radio-lamp-stage")
                    .debug_selector(|| "radio-lamp-stage".into())
                    .cursor_pointer()
                    .size(px(stage_size))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            if event.click_count >= 2 {
                                this.act(Action::RadioTogglePureGlow, window, cx);
                            }
                        }),
                    )
                    .child(turntable_visual),
            )
            .child(
                div()
                    .h(px(18.))
                    .text_size(px(10.))
                    .font_family(self.ui_font())
                    .text_color(text_muted.opacity(0.62))
                    .child(if self.radio_visual_mode == 0 {
                        "旋转黑胶唱片 · 双击进入纯享微光"
                    } else {
                        "床头呼吸氛围灯 · 双击进入纯享微光"
                    }),
            );

        // 3. Right Editorial Lyric / Night-Talk & Fixed 2x4 Station Preset Stage (ZERO JITTER!)
        let is_host_or_local = matches!(
            current.kind,
            RadioStationKind::NightHost
                | RadioStationKind::BookEchoHost
                | RadioStationKind::LocalFolder
        );
        let subtitle_line = if lamp_mode == RadioLampMode::Breathe478 {
            let bp = prism::breathing_phase(breathe_elapsed);
            format!("4-7-8 助眠呼吸 · {} {}s · {}", bp.label, bp.seconds_left, bp.cue)
        } else {
            display_hint
        };

        let card_h = if compact { 164.0_f32 } else { 196.0_f32 };
        let lyric_card = v_flex()
            .w_full()
            .h(px(card_h))
            .flex_shrink_0()
            .rounded_2xl()
            .bg(rgb(0x0b0c12).opacity(0.48))
            .border_1()
            .border_color(primary.opacity(0.22))
            .overflow_hidden()
            .child(
                v_flex()
                    .id("radio-story-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_5()
                    .py_4()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(primary.opacity(0.88))
                                    .text_ellipsis()
                                    .child(format!("✦  {display_title}")),
                            )
                            .child(self.waveform_bars(playing_active, &[4., 10., 6., 12., 5.])),
                    )
                    .child(
                        div()
                            .text_size(px(if compact { 14.5 } else { 15.5 }))
                            .font_family(self.display_font())
                            .line_height(relative(1.85))
                            .text_color(text_warm.opacity(0.92))
                            .child(display_line),
                    ),
            )
            .child(
                h_flex()
                    .h(px(38.))
                    .flex_shrink_0()
                    .px_4()
                    .items_center()
                    .justify_between()
                    .bg(rgb(0xffffff).opacity(0.025))
                    .border_t_1()
                    .border_color(rgb(0xffffff).opacity(0.07))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(text_muted.opacity(0.75))
                            .text_ellipsis()
                            .child(if self.radio_status.fallback_offline {
                                "离线微光琴韵温柔代播中".to_owned()
                            } else if let Some(meta) = &self.radio_status.stream_meta {
                                meta.clone()
                            } else {
                                "沉浸伴听 · 切回书架仍可在后台轻声播放".to_owned()
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .flex_shrink_0()
                            .when(is_host_or_local, |el| {
                                el.child(
                                    div()
                                        .id("radio-next-segment")
                                        .cursor_pointer()
                                        .px_2p5()
                                        .py_1()
                                        .rounded_full()
                                        .bg(rgb(0xffffff).opacity(0.07))
                                        .border_1()
                                        .border_color(rgb(0xffffff).opacity(0.11))
                                        .hover(|s| s.border_color(primary.opacity(0.55)))
                                        .text_size(px(10.))
                                        .font_family(self.ui_font())
                                        .text_color(text_warm.opacity(0.86))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, window, cx| {
                                                this.act(Action::RadioNextSegment, window, cx);
                                            }),
                                        )
                                        .child(if current.kind == RadioStationKind::LocalFolder {
                                            "下一首"
                                        } else {
                                            "换一句"
                                        }),
                                )
                            })
                            .child(
                                div()
                                    .id("radio-save-journal")
                                    .cursor_pointer()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_full()
                                    .bg(primary.opacity(0.14))
                                    .border_1()
                                    .border_color(primary.opacity(0.32))
                                    .hover(|s| s.bg(primary.opacity(0.24)))
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(core)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.act(Action::RadioSaveLineToJournal, window, cx);
                                        }),
                                    )
                                    .child("收入手账"),
                            ),
                    ),
            );

        // Fixed-height 2×4 symmetrical station preset grid (76px total — NEVER wraps or shifts!)
        let mut preset_slots: Vec<RadioStation> = stations.iter().take(8).cloned().collect();
        if stations.len() > 8
            && !preset_slots.iter().any(|s| s.id == current.id)
            && let Some(last) = preset_slots.last_mut()
        {
            *last = current.clone();
        }
        let render_preset_row = |slice: &[RadioStation]| {
            let mut row = h_flex().w_full().h(px(34.)).gap_1p5().items_center();
            for st in slice {
                let active = st.id == current.id;
                let previewed = drag_nearest.as_ref().is_some_and(|d| d.id == st.id);
                let fav = self
                    .store
                    .state
                    .radio
                    .favorites
                    .iter()
                    .any(|f| f == &st.id);
                let sid = st.id.clone();
                row = row.child(
                    h_flex()
                        .id(SharedString::from(format!("fm-pill-{}", st.id)))
                        .cursor_pointer()
                        .flex_1()
                        .min_w_0()
                        .h(px(34.))
                        .gap_1()
                        .items_center()
                        .px_2p5()
                        .rounded_xl()
                        .bg(if previewed {
                            primary.opacity(0.32)
                        } else if active {
                            primary.opacity(0.22)
                        } else {
                            rgb(0xffffff).opacity(0.045)
                        })
                        .border_1()
                        .border_color(if previewed || active {
                            primary.opacity(0.68)
                        } else {
                            rgb(0xffffff).opacity(0.09)
                        })
                        .hover(|s| s.border_color(primary.opacity(0.55)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.act(Action::RadioTuneStation(sid.clone()), window, cx);
                            }),
                        )
                        .child(
                            div()
                                .size(px(5.5))
                                .flex_shrink_0()
                                .rounded_full()
                                .bg(if active || previewed {
                                    core
                                } else if fav {
                                    primary
                                } else {
                                    text_muted.opacity(0.45)
                                }),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(9.5))
                                .font_family(self.ui_font())
                                .text_color(if active || previewed { core } else { text_muted })
                                .child(format!("{:.1}", st.frequency_mhz)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(10.5))
                                .font_family(self.ui_font())
                                .text_color(if active || previewed {
                                    text_warm
                                } else {
                                    text_muted
                                })
                                .text_ellipsis()
                                .child(st.name.clone()),
                        ),
                );
            }
            row
        };
        let row1_end = preset_slots.len().min(4);
        let preset_grid = v_flex()
            .w_full()
            .h(px(74.))
            .flex_shrink_0()
            .gap_1p5()
            .child(render_preset_row(&preset_slots[..row1_end]))
            .child(render_preset_row(&preset_slots[row1_end..]));

        let right_editorial = v_flex()
            .flex_1()
            .min_w_0()
            .max_w(px(540.))
            .gap_3()
            .child(
                // Fixed-height station headline (42px)
                h_flex()
                    .w_full()
                    .h(px(42.))
                    .flex_shrink_0()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .flex_shrink_0()
                            .w(px(116.))
                            .text_size(px(28.))
                            .font_family(self.display_font())
                            .text_color(if self.radio_dial_dragging { primary } else { core })
                            .child(format!("FM {display_mhz:.1}")),
                    )
                    .child(
                        div()
                            .w(px(1.))
                            .h(px(22.))
                            .flex_shrink_0()
                            .bg(rgb(0xffffff).opacity(0.18)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(21.))
                            .font_family(self.display_font())
                            .text_color(text_warm)
                            .text_ellipsis()
                            .child(
                                drag_nearest
                                    .as_ref()
                                    .map(|s| s.name.clone())
                                    .unwrap_or(display_name),
                            ),
                    )
                    .child({
                        let fav_id = current.id.clone();
                        div()
                            .id("radio-fav-btn")
                            .debug_selector(|| "radio-fav-btn".into())
                            .cursor_pointer()
                            .flex_shrink_0()
                            .size(px(34.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(if is_fav {
                                primary.opacity(0.24)
                            } else {
                                rgb(0xffffff).opacity(0.06)
                            })
                            .border_1()
                            .border_color(if is_fav {
                                primary.opacity(0.65)
                            } else {
                                rgb(0xffffff).opacity(0.12)
                            })
                            .hover(|s| s.border_color(primary.opacity(0.6)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.act(
                                        Action::RadioToggleFavorite(fav_id.clone()),
                                        window,
                                        cx,
                                    );
                                }),
                            )
                            .child(
                                Icon::new(IconName::Heart)
                                    .size(px(14.))
                                    .text_color(if is_fav { primary } else { text_muted }),
                            )
                    }),
            )
            .child(
                // Fixed-height badge + subtitle / 4-7-8 cadence slot (26px)
                h_flex()
                    .w_full()
                    .h(px(26.))
                    .flex_shrink_0()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_2p5()
                            .py_0p5()
                            .rounded_full()
                            .bg(primary.opacity(0.16))
                            .border_1()
                            .border_color(primary.opacity(0.32))
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(core)
                            .child(current.kind.badge_label()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(11.5))
                            .font_family(self.ui_font())
                            .text_color(if lamp_mode == RadioLampMode::Breathe478 {
                                core.opacity(0.88)
                            } else {
                                text_muted
                            })
                            .text_ellipsis()
                            .child(subtitle_line),
                    ),
            )
            .child(lyric_card)
            .child(preset_grid);

        let hero_stage = div()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .px_8()
            .flex()
            .items_center()
            .justify_center()
            .child(
                h_flex()
                    .w_full()
                    .max_w(px(1060.))
                    .items_center()
                    .justify_center()
                    .gap(px(if compact { 24. } else { 44. }))
                    .child(left_hero)
                    .child(right_editorial),
            );

        // 4. Full-Width Draggable Analog 88.0 – 108.0 MHz FM Tuner Deck (fixed 96px)
        let markers: Vec<(f32, bool)> = stations
            .iter()
            .map(|s| {
                (
                    s.frequency_mhz,
                    self.store.state.radio.favorites.iter().any(|f| f == &s.id),
                )
            })
            .collect();
        let tuner_status_text = if let Some(ref target) = drag_nearest {
            format!(
                "拖动调频 FM {:.1}  →  松手吸附 FM {:.1} · {}",
                dial_mhz, target.frequency_mhz, target.name
            )
        } else {
            format!("当前锁频 FM {:.1} · {}", station_mhz, current.name)
        };
        let tuner_deck = div()
            .relative()
            .w_full()
            .flex_shrink_0()
            .px_8()
            .pb_2p5()
            .flex()
            .justify_center()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(1060.))
                    .h(px(96.))
                    .px_4()
                    .py_2()
                    .justify_between()
                    .rounded_2xl()
                    .bg(rgb(0x0b0c12).opacity(0.46))
                    .border_1()
                    .border_color(if self.radio_dial_dragging {
                        primary.opacity(0.48)
                    } else {
                        primary.opacity(0.20)
                    })
                    .child(
                        h_flex()
                            .h(px(16.))
                            .justify_between()
                            .items_center()
                            .px_1()
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(text_muted.opacity(0.76))
                                    .child("88.0 — 108.0 MHz  ·  按住指针左右拖动调频 / 滚轮切台"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(if self.radio_dial_dragging { core } else { primary })
                                    .child(tuner_status_text),
                            ),
                    )
                    .child(
                        h_flex()
                            .h(px(13.))
                            .justify_between()
                            .items_center()
                            .px_3()
                            .children(
                                [
                                    "88", "90", "92", "94", "96", "98", "100", "102", "104",
                                    "106", "108 MHz",
                                ]
                                .into_iter()
                                .map(|lbl| {
                                    div()
                                        .text_size(px(9.))
                                        .font_family(self.ui_font())
                                        .text_color(text_muted.opacity(0.68))
                                        .child(lbl)
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("radio-dial-track")
                            .debug_selector(|| "radio-dial-track".into())
                            .relative()
                            .w_full()
                            .h(px(54.))
                            .cursor(if self.radio_dial_dragging {
                                CursorStyle::ClosedHand
                            } else {
                                CursorStyle::OpenHand
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                    this.begin_radio_dial_drag(event.position, window, cx);
                                    cx.stop_propagation();
                                }),
                            )
                            .on_scroll(cx.listener(
                                |this, event: &ScrollEvent, window, cx| {
                                    this.on_radio_dial_scroll(event, window, cx);
                                },
                            ))
                            .child(prism::fm_tuner_dial_canvas(
                                snap.clone(),
                                dial_mhz,
                                locked_marker_mhz,
                                markers,
                                lamp_color,
                                playing_active,
                                self.radio_dial_dragging,
                                Arc::clone(&self.radio_dial_bounds),
                            )),
                    ),
            );

        // 5. Bottom Immersive Glass Control Dock (fixed 70px, rock-solid 3-zone grid!)
        let mut lamp_swatches = h_flex().gap_1p5().items_center();
        for c in RadioLampColor::ALL {
            let (c_hex, _, _, _) = c.palette();
            let active = c == lamp_color;
            let cid = c.id().to_owned();
            lamp_swatches = lamp_swatches.child(
                div()
                    .id(SharedString::from(format!("lamp-color-{}", c.id())))
                    .debug_selector(move || format!("lamp-color-{}", c.id()))
                    .cursor_pointer()
                    .p(px(2.5))
                    .rounded_full()
                    .border_1()
                    .border_color(if active {
                        rgb(c_hex)
                    } else {
                        rgb(0xffffff).opacity(0.14)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::RadioLampColor(cid.clone()), window, cx);
                        }),
                    )
                    .child(div().size(px(13.)).rounded_full().bg(rgb(c_hex))),
            );
        }

        let mut mode_pills = h_flex().gap_1().items_center();
        for m in RadioLampMode::ALL {
            let active = m == lamp_mode;
            let mid = m.id().to_owned();
            let short_label = match m {
                RadioLampMode::Audio => "声波",
                RadioLampMode::Breathe478 => "4-7-8",
                RadioLampMode::Steady => "暖灯",
            };
            mode_pills = mode_pills.child(
                div()
                    .id(SharedString::from(format!("lamp-mode-{}", m.id())))
                    .debug_selector(move || format!("lamp-mode-{}", m.id()))
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_lg()
                    .bg(if active {
                        primary.opacity(0.24)
                    } else {
                        rgb(0xffffff).opacity(0.05)
                    })
                    .border_1()
                    .border_color(if active {
                        primary.opacity(0.60)
                    } else {
                        rgb(0xffffff).opacity(0.10)
                    })
                    .text_size(px(10.))
                    .font_family(self.ui_font())
                    .text_color(if active { core } else { text_muted })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::RadioLampMode(mid.clone()), window, cx);
                        }),
                    )
                    .child(short_label),
            );
        }

        let vol_pct = (self.store.state.radio.volume.clamp(0.05, 1.0) * 100.0).round() as u32;
        let bottom_dock = h_flex()
            .relative()
            .w_full()
            .h(px(70.))
            .flex_shrink_0()
            .px_6()
            .justify_between()
            .items_center()
            .gap_4()
            .bg(rgb(0x090a0f).opacity(0.76))
            .border_t_1()
            .border_color(primary.opacity(0.18))
            // Left: Breathing Lamp Color + Cadence + Vinyl/Orb visual toggle
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2p5()
                    .items_center()
                    .child(lamp_swatches)
                    .child(div().w(px(1.)).h(px(18.)).bg(rgb(0xffffff).opacity(0.12)))
                    .child(mode_pills)
                    .child(div().w(px(1.)).h(px(18.)).bg(rgb(0xffffff).opacity(0.12)))
                    .child(
                        div()
                            .id("radio-visual-toggle")
                            .debug_selector(|| "radio-visual-toggle".into())
                            .cursor_pointer()
                            .px_2p5()
                            .py_1()
                            .rounded_lg()
                            .bg(rgb(0xffffff).opacity(0.06))
                            .border_1()
                            .border_color(rgb(0xffffff).opacity(0.12))
                            .hover(|s| s.border_color(primary.opacity(0.55)))
                            .text_size(px(10.))
                            .font_family(self.ui_font())
                            .text_color(text_warm.opacity(0.88))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.act(Action::RadioToggleVisualMode, window, cx);
                                }),
                            )
                            .child(if self.radio_visual_mode == 0 {
                                "切光球"
                            } else {
                                "切黑胶"
                            }),
                    ),
            )
            // Center: Rock-solid fixed-width circular Transport Cluster (NEVER shifts!)
            .child(
                h_flex()
                    .w(px(176.))
                    .flex_shrink_0()
                    .justify_center()
                    .gap_4()
                    .items_center()
                    .child(
                        div()
                            .id("radio-prev-btn")
                            .debug_selector(|| "radio-prev-btn".into())
                            .cursor_pointer()
                            .size(px(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(rgb(0xffffff).opacity(0.06))
                            .border_1()
                            .border_color(rgb(0xffffff).opacity(0.12))
                            .hover(|s| s.border_color(primary.opacity(0.6)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.act(Action::RadioPrevStation, window, cx);
                                }),
                            )
                            .child(
                                Icon::new(IconName::ChevronLeft)
                                    .size(px(16.))
                                    .text_color(text_warm),
                            ),
                    )
                    .child(
                        div()
                            .id("radio-play-btn")
                            .debug_selector(|| "radio-play-btn".into())
                            .cursor_pointer()
                            .size(px(48.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(primary)
                            .hover(|s| s.opacity(0.92))
                            .shadow_md()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.act(Action::RadioTogglePlay, window, cx);
                                }),
                            )
                            .child(
                                Icon::new(if playing_active {
                                    IconName::Pause
                                } else {
                                    IconName::Play
                                })
                                .size(px(19.))
                                .text_color(rgb(0x141010)),
                            ),
                    )
                    .child(
                        div()
                            .id("radio-next-btn")
                            .debug_selector(|| "radio-next-btn".into())
                            .cursor_pointer()
                            .size(px(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(rgb(0xffffff).opacity(0.06))
                            .border_1()
                            .border_color(rgb(0xffffff).opacity(0.12))
                            .hover(|s| s.border_color(primary.opacity(0.6)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.act(Action::RadioNextStation, window, cx);
                                }),
                            )
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .size(px(16.))
                                    .text_color(text_warm),
                            ),
                    ),
            )
            // Right: Draggable Smooth Volume Slider + Explore Drawer Trigger
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .justify_end()
                    .gap_2p5()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .px_2p5()
                            .py_1()
                            .rounded_full()
                            .bg(rgb(0xffffff).opacity(0.05))
                            .border_1()
                            .border_color(rgb(0xffffff).opacity(0.10))
                            .child(
                                div()
                                    .id("radio-vol-down")
                                    .cursor_pointer()
                                    .px_1()
                                    .text_size(px(12.))
                                    .text_color(text_warm)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.act(Action::RadioVolume(-1), window, cx);
                                        }),
                                    )
                                    .child("−"),
                            )
                            .child(
                                div()
                                    .id("radio-vol-track")
                                    .debug_selector(|| "radio-vol-track".into())
                                    .cursor(if self.radio_vol_dragging {
                                        CursorStyle::ClosedHand
                                    } else {
                                        CursorStyle::OpenHand
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                            this.begin_radio_vol_drag(event.position, window, cx);
                                            cx.stop_propagation();
                                        }),
                                    )
                                    .child(prism::radio_volume_bar_canvas(
                                        self.store.state.radio.volume,
                                        lamp_color,
                                        self.radio_vol_dragging,
                                        Arc::clone(&self.radio_vol_bounds),
                                    )),
                            )
                            .child(
                                div()
                                    .id("radio-vol-up")
                                    .cursor_pointer()
                                    .px_1()
                                    .text_size(px(12.))
                                    .text_color(text_warm)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.act(Action::RadioVolume(1), window, cx);
                                        }),
                                    )
                                    .child("+"),
                            )
                            .child(
                                div()
                                    .w(px(30.))
                                    .text_right()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(text_muted)
                                    .child(format!("{vol_pct}%")),
                            ),
                    )
                    .child(
                        h_flex()
                            .id("radio-drawer-btn")
                            .debug_selector(|| "radio-drawer-btn".into())
                            .cursor_pointer()
                            .flex_shrink_0()
                            .gap_1p5()
                            .items_center()
                            .px_3p5()
                            .py_2()
                            .rounded_full()
                            .bg(if self.radio_drawer_open {
                                primary.opacity(0.28)
                            } else {
                                primary.opacity(0.14)
                            })
                            .border_1()
                            .border_color(primary.opacity(0.45))
                            .hover(|s| s.bg(primary.opacity(0.24)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.act(Action::RadioToggleDrawer, window, cx);
                                }),
                            )
                            .child(Icon::new(IconName::Search).size(px(12.5)).text_color(core))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_family(self.ui_font())
                                    .text_color(core)
                                    .child("探索频段"),
                            ),
                    ),
            );

        // 6. Floating non-layout-shifting toast pill (absolute overlay -> 0px layout impact!)
        let floating_toast = self.error.as_ref().or(self.notice.as_ref()).map(|msg| {
            let is_err = self.error.is_some();
            div()
                .absolute()
                .top(px(62.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .px_4()
                        .py_1p5()
                        .rounded_full()
                        .bg(rgb(0x131118).opacity(0.90))
                        .border_1()
                        .border_color(if is_err {
                            rgb(0xf48c7a).opacity(0.55)
                        } else {
                            primary.opacity(0.42)
                        })
                        .shadow_md()
                        .child(
                            Icon::new(if is_err {
                                IconName::CircleAlert
                            } else {
                                IconName::Sparkles
                            })
                            .size(px(12.5))
                            .text_color(if is_err { rgb(0xf48c7a) } else { core }),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_family(self.ui_font())
                                .text_color(if is_err { rgb(0xf48c7a) } else { text_warm })
                                .child(msg.clone()),
                        ),
                )
        });

        v_flex()
            .id("radio-page")
            .debug_selector(|| "radio-page".into())
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(rgb(stage_bg_hex))
            .text_color(text_warm)
            .child(
                img(station_scene)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .opacity(0.22 * dim_factor),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .bg(rgb(stage_bg_hex).opacity(0.76)),
            )
            .child(prism::radio_ambient_room_canvas(
                snap,
                lamp_color,
                lamp_mode,
                self.radio_audio_level,
                breathe_elapsed,
                playing_active,
                dim_factor,
                false,
            ))
            .child(top_bar)
            .child(hero_stage)
            .child(tuner_deck)
            .child(bottom_dock)
            .when_some(floating_toast, |el, toast| el.child(toast))
            .into_any_element()
    }

    // ================= UI: Explore / Search / Custom / Local Stations Modal =================
    #[inline(never)]
    pub(super) fn radio_drawer_modal(&self, cx: &Context<Self>) -> AnyElement {
        let lamp_color = RadioLampColor::from_id(&self.store.state.radio.lamp_color);
        let (primary_hex, core_hex, _, stage_bg_hex) = lamp_color.palette();
        let primary = rgb(primary_hex);
        let core = rgb(core_hex);
        let text_warm = rgb(0xf4ebe0);
        let text_muted = rgb(0xa89f96);

        let tabs = [
            (0u8, "频段与主播"),
            (1u8, "在线搜台 · 自定义流"),
            (2u8, "本地私藏磁带"),
        ];
        let mut tab_bar = h_flex()
            .gap_1p5()
            .p_1()
            .rounded_xl()
            .bg(rgb(0xffffff).opacity(0.05));
        for (idx, label) in tabs {
            let active = self.radio_drawer_tab == idx;
            tab_bar = tab_bar.child(
                div()
                    .id(SharedString::from(format!("radio-tab-{idx}")))
                    .cursor_pointer()
                    .flex_1()
                    .py_1p5()
                    .rounded_lg()
                    .flex()
                    .justify_center()
                    .bg(if active {
                        primary.opacity(0.24)
                    } else {
                        rgb(0x000000).opacity(0.0)
                    })
                    .text_size(px(11.5))
                    .font_family(self.ui_font())
                    .text_color(if active { core } else { text_muted })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.act(Action::RadioDrawerTab(idx), window, cx);
                        }),
                    )
                    .child(label),
            );
        }

        let body: AnyElement = match self.radio_drawer_tab {
            0 => {
                let stations = self.all_radio_stations();
                let mut list = v_flex().gap_2();
                let filtered: Vec<&RadioStation> = stations
                    .iter()
                    .filter(|s| {
                        !self.radio_only_favorites
                            || self.store.state.radio.favorites.iter().any(|f| f == &s.id)
                    })
                    .collect();
                for st in filtered {
                    let active = st.id == self.store.state.radio.current_station_id;
                    let fav = self
                        .store
                        .state
                        .radio
                        .favorites
                        .iter()
                        .any(|f| f == &st.id);
                    let sid = st.id.clone();
                    let fav_id = st.id.clone();
                    let rm_id = st.id.clone();
                    list = list.child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .p_3()
                            .rounded_xl()
                            .bg(if active {
                                primary.opacity(0.18)
                            } else {
                                rgb(0xffffff).opacity(0.04)
                            })
                            .border_1()
                            .border_color(if active {
                                primary.opacity(0.60)
                            } else {
                                rgb(0xffffff).opacity(0.09)
                            })
                            .child(
                                h_flex()
                                    .id(SharedString::from(format!("drawer-tune-{}", st.id)))
                                    .cursor_pointer()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_3()
                                    .items_center()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, window, cx| {
                                            this.act(
                                                Action::RadioTuneStation(sid.clone()),
                                                window,
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(
                                        h_flex()
                                            .w(px(62.))
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                div()
                                                    .size(px(6.))
                                                    .rounded_full()
                                                    .bg(rgb(
                                                        RadioLampColor::from_id(st.suggested_lamp)
                                                            .palette()
                                                            .0,
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .font_family(self.display_font())
                                                    .text_color(core)
                                                    .child(format!("{:.1}", st.frequency_mhz)),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap_0p5()
                                            .child(
                                                h_flex()
                                                    .gap_1p5()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .text_size(px(12.5))
                                                            .font_family(self.ui_font())
                                                            .text_color(text_warm)
                                                            .child(st.name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .px_1p5()
                                                            .py_0p5()
                                                            .rounded_full()
                                                            .bg(primary.opacity(0.14))
                                                            .text_size(px(9.))
                                                            .font_family(self.ui_font())
                                                            .text_color(core)
                                                            .child(st.kind.badge_label()),
                                                    )
                                                    .when(!st.tags.is_empty(), |el| {
                                                        el.child(
                                                            div()
                                                                .text_size(px(9.5))
                                                                .font_family(self.ui_font())
                                                                .text_color(text_muted.opacity(0.76))
                                                                .child(
                                                                    st.tags
                                                                        .iter()
                                                                        .take(2)
                                                                        .map(|t| format!("#{t}"))
                                                                        .collect::<Vec<_>>()
                                                                        .join(" "),
                                                                ),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(10.5))
                                                    .font_family(self.ui_font())
                                                    .text_color(text_muted)
                                                    .text_ellipsis()
                                                    .child(st.subtitle.clone()),
                                            ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("drawer-fav-{}", st.id)))
                                            .cursor_pointer()
                                            .p_1p5()
                                            .rounded_lg()
                                            .bg(if fav {
                                                primary.opacity(0.22)
                                            } else {
                                                rgb(0xffffff).opacity(0.05)
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, window, cx| {
                                                    this.act(
                                                        Action::RadioToggleFavorite(fav_id.clone()),
                                                        window,
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .child(
                                                Icon::new(IconName::Heart)
                                                    .size(px(12.5))
                                                    .text_color(if fav { primary } else { text_muted }),
                                            ),
                                    )
                                    .when(st.is_custom, |el| {
                                        el.child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "drawer-rm-{}",
                                                    st.id
                                                )))
                                                .cursor_pointer()
                                                .p_1p5()
                                                .rounded_lg()
                                                .bg(rgb(0xffffff).opacity(0.05))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, window, cx| {
                                                        this.act(
                                                            Action::RadioRemoveCustomStation(
                                                                rm_id.clone(),
                                                            ),
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                                )
                                                .child(
                                                    Icon::new(IconName::X)
                                                        .size(px(12.))
                                                        .text_color(text_muted),
                                                ),
                                        )
                                    }),
                            ),
                    );
                }

                // Night host voice chooser
                let mut voice_row = h_flex().gap_1p5().flex_wrap();
                for v in tts::VOICES.iter().take(4) {
                    let selected = self.store.state.radio.host_voice == v.id;
                    let vid = v.id.to_owned();
                    voice_row = voice_row.child(
                        div()
                            .id(SharedString::from(format!("radio-voice-{}", v.id)))
                            .cursor_pointer()
                            .px_2p5()
                            .py_1()
                            .rounded_lg()
                            .bg(if selected {
                                primary.opacity(0.24)
                            } else {
                                rgb(0xffffff).opacity(0.05)
                            })
                            .border_1()
                            .border_color(if selected {
                                primary.opacity(0.65)
                            } else {
                                rgb(0xffffff).opacity(0.10)
                            })
                            .text_size(px(10.5))
                            .font_family(self.ui_font())
                            .text_color(if selected { core } else { text_muted })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.act(Action::RadioSetHostVoice(vid.clone()), window, cx);
                                }),
                            )
                            .child(format!("{} · {}", v.label, v.style)),
                    );
                }

                v_flex()
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(text_muted)
                                    .child("点击频段即刻调频收听（带老收音机柔和过渡音）"),
                            )
                            .child(
                                div()
                                    .id("radio-fav-filter")
                                    .cursor_pointer()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_full()
                                    .bg(if self.radio_only_favorites {
                                        primary.opacity(0.24)
                                    } else {
                                        rgb(0xffffff).opacity(0.06)
                                    })
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(if self.radio_only_favorites {
                                        core
                                    } else {
                                        text_muted
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.act(Action::RadioToggleFavFilter, window, cx);
                                        }),
                                    )
                                    .child(if self.radio_only_favorites {
                                        "仅看红心收藏"
                                    } else {
                                        "全部频段"
                                    }),
                            ),
                    )
                    .child(list)
                    .child(
                        v_flex()
                            .gap_1p5()
                            .pt_2()
                            .border_t_1()
                            .border_color(rgb(0xffffff).opacity(0.08))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(text_muted)
                                    .child("深夜夜话 / 书摘回声主播音色"),
                            )
                            .child(voice_row),
                    )
                    .into_any_element()
            }
            1 => {
                let searching = self.radio_search_rx.is_some();
                let mut tag_row = h_flex().gap_1p5().flex_wrap();
                for &(tag_id, label) in GENRE_SEARCH_TAGS {
                    tag_row = tag_row.child(
                        div()
                            .id(SharedString::from(format!("radio-tag-{tag_id}")))
                            .cursor_pointer()
                            .px_2p5()
                            .py_1()
                            .rounded_full()
                            .bg(primary.opacity(0.14))
                            .border_1()
                            .border_color(primary.opacity(0.32))
                            .hover(|s| s.bg(primary.opacity(0.24)))
                            .text_size(px(10.5))
                            .font_family(self.ui_font())
                            .text_color(core)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.act(Action::RadioSearchTag(tag_id), window, cx);
                                }),
                            )
                            .child(label),
                    );
                }

                let mut hits_list = v_flex().gap_1p5();
                for (idx, hit) in self.radio_search_results.iter().enumerate() {
                    hits_list = hits_list.child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .p_2p5()
                            .rounded_xl()
                            .bg(rgb(0xffffff).opacity(0.04))
                            .border_1()
                            .border_color(rgb(0xffffff).opacity(0.09))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_family(self.ui_font())
                                            .text_color(text_warm)
                                            .text_ellipsis()
                                            .child(hit.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .font_family(self.ui_font())
                                            .text_color(text_muted)
                                            .text_ellipsis()
                                            .child(format!(
                                                "{} · {} kbps · FM {:.1}",
                                                if hit.country.is_empty() {
                                                    "全球电台"
                                                } else {
                                                    hit.country.as_str()
                                                },
                                                hit.bitrate,
                                                hit.suggested_mhz
                                            )),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("rb-preview-{idx}")))
                                            .cursor_pointer()
                                            .px_2p5()
                                            .py_1()
                                            .rounded_lg()
                                            .bg(rgb(0xffffff).opacity(0.08))
                                            .text_size(px(10.5))
                                            .font_family(self.ui_font())
                                            .text_color(text_warm)
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, window, cx| {
                                                    this.act(
                                                        Action::RadioPreviewHit(idx),
                                                        window,
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .child("试听"),
                                    )
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("rb-add-{idx}")))
                                            .cursor_pointer()
                                            .px_2p5()
                                            .py_1()
                                            .rounded_lg()
                                            .bg(primary.opacity(0.24))
                                            .text_size(px(10.5))
                                            .font_family(self.ui_font())
                                            .text_color(core)
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, window, cx| {
                                                    this.act(Action::RadioAddHit(idx), window, cx);
                                                }),
                                            )
                                            .child("加入刻度尺"),
                                    ),
                            ),
                    );
                }

                v_flex()
                    .gap_3p5()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_family(self.ui_font())
                                    .text_color(text_warm)
                                    .child("搜索全球免费 MP3 网络电台（Radio-Browser）"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().flex_1().child(Input::new(&self.radio_search_input)))
                                    .child(
                                        div()
                                            .id("radio-search-go")
                                            .cursor_pointer()
                                            .px_4()
                                            .py_2()
                                            .rounded_lg()
                                            .bg(primary)
                                            .text_size(px(11.5))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(0x141010))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, window, cx| {
                                                    this.act(Action::RadioSearchSubmit, window, cx);
                                                }),
                                            )
                                            .child(if searching { "搜索中…" } else { "搜索" }),
                                    ),
                            )
                            .child(tag_row),
                    )
                    .when(!self.radio_search_results.is_empty(), |el| el.child(hits_list))
                    .child(
                        v_flex()
                            .gap_2()
                            .pt_3()
                            .border_t_1()
                            .border_color(rgb(0xffffff).opacity(0.08))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_family(self.ui_font())
                                    .text_color(text_warm)
                                    .child("手动添加自定义电台流（MP3 / Ogg URL）"),
                            )
                            .child(Input::new(&self.radio_custom_name_input))
                            .child(Input::new(&self.radio_custom_url_input))
                            .child(
                                h_flex().justify_end().child(
                                    div()
                                        .id("radio-add-custom-btn")
                                        .debug_selector(|| "radio-add-custom-btn".into())
                                        .cursor_pointer()
                                        .px_4()
                                        .py_1p5()
                                        .rounded_lg()
                                        .bg(primary.opacity(0.26))
                                        .border_1()
                                        .border_color(primary.opacity(0.55))
                                        .text_size(px(11.5))
                                        .font_family(self.ui_font())
                                        .text_color(core)
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, window, cx| {
                                                this.act(Action::RadioAddCustomStream, window, cx);
                                            }),
                                        )
                                        .child("添加并收藏到刻度尺"),
                                ),
                            ),
                    )
                    .into_any_element()
            }
            _ => {
                let folder = self.store.state.radio.local_audio_folder.clone();
                let track_count = folder
                    .as_deref()
                    .map(|p| radio::scan_local_audio_files(std::path::Path::new(p)).len())
                    .unwrap_or(0);
                v_flex()
                    .gap_4()
                    .p_4()
                    .rounded_2xl()
                    .bg(rgb(0xffffff).opacity(0.04))
                    .border_1()
                    .border_color(rgb(0xffffff).opacity(0.09))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_family(self.display_font())
                            .text_color(core)
                            .child("FM 107.6 · 本地私藏磁带"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_family(self.ui_font())
                            .line_height(relative(1.65))
                            .text_color(text_muted)
                            .child("选择一个存放 MP3、WAV 或 OGG 音乐与播客的本地文件夹，微光会自动将其变成你的专属离线深夜电台（仅保存本机路径，不复制原文件，也不会上传同步）。"),
                    )
                    .child(
                        div()
                            .p_3()
                            .rounded_xl()
                            .bg(rgb(0x000000).opacity(0.28))
                            .text_size(px(11.5))
                            .font_family(self.ui_font())
                            .text_color(text_warm)
                            .child(match folder {
                                Some(path) => format!("当前目录：{path}\n已发现 {track_count} 首音频"),
                                None => "尚未选择本地音频文件夹".into(),
                            }),
                    )
                    .child(
                        h_flex().gap_2().child(
                            div()
                                .id("radio-pick-folder-btn")
                                .cursor_pointer()
                                .px_4()
                                .py_2()
                                .rounded_lg()
                                .bg(primary)
                                .text_size(px(12.))
                                .font_family(self.ui_font())
                                .text_color(rgb(0x141010))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.act(Action::RadioPickLocalFolder, window, cx);
                                    }),
                                )
                                .child("选择本地音频文件夹"),
                        ),
                    )
                    .into_any_element()
            }
        };

        div()
            .id("radio-drawer-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x05060a).opacity(0.72))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.act(Action::RadioCloseDrawer, window, cx);
                }),
            )
            .child(
                v_flex()
                    .id("radio-drawer-card")
                    .w(px(660.))
                    .max_w_full()
                    .max_h(px(620.))
                    .p_5()
                    .gap_4()
                    .rounded_2xl()
                    .bg(rgb(stage_bg_hex).opacity(0.98))
                    .border_1()
                    .border_color(primary.opacity(0.35))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
                                            .text_color(core)
                                            .child("探索深夜频段 · Glow FM"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .font_family(self.ui_font())
                                            .text_color(text_muted)
                                            .child("精选治愈流 · 在线免费电台搜索 · 本地私藏磁带"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("radio-drawer-close")
                                    .cursor_pointer()
                                    .p_1p5()
                                    .rounded_lg()
                                    .bg(rgb(0xffffff).opacity(0.06))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.act(Action::RadioCloseDrawer, window, cx);
                                        }),
                                    )
                                    .child(
                                        Icon::new(IconName::X)
                                            .size(px(14.))
                                            .text_color(text_muted),
                                    ),
                            ),
                    )
                    .child(tab_bar)
                    .child(
                        v_flex()
                            .id("radio-drawer-body-scroll")
                            .overflow_y_scroll()
                            .flex_1()
                            .min_h_0()
                            .pr_1()
                            .child(body),
                    ),
            )
            .into_any_element()
    }
}

fn local_clock_hm() -> String {
    time::OffsetDateTime::now_local()
        .ok()
        .and_then(|dt| {
            dt.format(&time::macros::format_description!("[hour]:[minute]"))
                .ok()
        })
        .unwrap_or_else(|| "23:00".into())
}

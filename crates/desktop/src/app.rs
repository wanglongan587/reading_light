#[path = "app_features.rs"]
mod features;

use crate::{
    mood, platform,
    prism::{self, MotionEngine, MotionSnapshot, ThemeAtmosphere},
    quotecard::{self, QuoteCardDraft, QuoteCardStyle},
    soundscape::{ChimeSource, SoundscapeHandle, SoundscapePreset, SoundscapeSource},
    sync_client, tts,
};
use anyhow::Context as _;
use glow_core::{
    AiSettings, BookCompanionCache, Bookmark, EchoResult, JournalEntry, LocalSecrets, Location,
    SoundscapeMix, Store, Theme as ReadingTheme,
    bridge::{ReaderEvent, SearchHit, TocEntry},
    echo,
    model::themes,
};
use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Icon, Theme as KitTheme, ThemeMode,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    h_flex,
    input::{Escape as InputEscape, Input, InputEvent, InputState, Textarea, TextareaState},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_webview::WebView;
use rodio::Source;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

actions!(glow, [Escape, Import, Previous, Next, Search, Palette]);

type RediscoveryTarget = (String, String);
type RediscoveryItem = (String, String, Option<RediscoveryTarget>);
type DailyRediscovery = (String, String, Option<RediscoveryTarget>, usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShelfFilter {
    All,
    Reading,
    Unread,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JournalFilter {
    All,
    Favorite,
    Mood,
    Selection,
}

#[derive(Clone, Debug, PartialEq)]
enum Page {
    Shelf,
    Reader,
    Mood,
    Journal,
    Themes,
}
#[derive(Clone, Debug)]
enum Action {
    Page(Page),
    Import,
    Open(String),
    OpenAt(Box<(String, String)>),
    Remove(String),
    ConfirmRemove,
    Cancel,
    Mode,
    Prev,
    Next,
    Focus,
    Toc,
    Search,
    SubmitSearch,
    Highlights,
    Highlight,
    HighlightColor(String),
    RecolorHighlight(Box<(String, String)>),
    EditHighlightNote(String),
    SaveHighlightNote,
    CancelHighlightNote,
    DeleteHighlight(String),
    SaveEchoToJournal,
    AskAiPassage,
    CancelAiPassage,
    Typography,
    TypeSize(i8),
    TypeLeading(i8),
    TypeWidth(i16),
    ToggleWarmFilter,
    ResetTypography,
    Bookmarks,
    Bookmark,
    Locate(String),
    CloseEcho,
    Mood(String),
    SubmitMood,
    CancelMood,
    ToggleBreathing,
    ToggleTimerPicker,
    CloseTimerPicker,
    StepCustomTimer(i32),
    StartCustomFocusTimer,
    ToggleTimerWithAudio,
    StartFocusTimer(u32),
    StopFocusTimer,
    Favorite(String),
    DeleteEntry(String),
    ClearJournal,
    ConfirmClear,
    ShelfFilter(ShelfFilter),
    NextRediscovery,
    JournalFilter(JournalFilter),
    ExportJournalMarkdown,
    OpenQuoteCard(Box<QuoteCardDraft>),
    QuoteCardStyle(QuoteCardStyle),
    SaveQuoteCardPng,
    CopyQuoteCardText,
    CloseQuoteCard,
    TogglePalette,
    ClosePalette,
    PickTheme(usize),
    EditTheme,
    PreviewTheme,
    SaveTheme,
    CancelTheme,
    ResetTheme,
    ImportTheme,
    ExportTheme,
    Asset(String),
    Audio,
    Soundscape(SoundscapePreset),
    AudioVolume(i8),
    ReducedMotion,
    ToggleFlow,
    PetInteract(Point<Pixels>),
    RevealSpoiler(String),
    Privacy,
    Encoding(String),
    // Multi-Track Soundscape Mixer
    ToggleMixer,
    CloseMixer,
    StepMixTrack(SoundscapePreset, i8),
    SetMixTrack(SoundscapePreset, f32),
    ApplyMixRecipe(&'static str),
    // Spoiler-Free AI Companion (Recap & Character Graph)
    ToggleCompanion,
    CompanionTab(u8),
    SelectCharacter(usize),
    RefreshCompanion,
    CancelCompanion,
    OpenCompanionPoster,
    // Edge-TTS Audiobook Studio
    ToggleTtsDrawer,
    TtsStart,
    TtsPauseResume,
    TtsStop,
    TtsPrev,
    TtsNext,
    TtsVoice(String),
    TtsRate(f32),
    TtsToggleDuck,
    TtsToggleSleepTimer,
    // Multi-Provider AI Settings (Cloud + Local Offline Ollama/LM Studio/llama.cpp + Custom URL)
    SelectAiPreset(String),
    PickDiscoveredModel(String),
    SaveAiSettings,
    TestAiConnection,
    // Multi-Device Sync (GitHub Private Repo / Sync Folder)
    SelectSyncMode(String),
    PickSyncFolder,
    ToggleSyncBooks,
    ToggleSyncOnExit,
    SaveSyncSettings,
    TestSyncConnection,
    RunSyncNow,
}

struct PassageGenerated {
    book_id: Option<String>,
    passage: String,
    cfi: String,
    result: EchoResult,
}

pub struct ReadingApp {
    store: Store,
    page: Page,
    webview: Option<Entity<WebView>>,
    session: String,
    active_book: Option<String>,
    request: u64,
    last_event: u64,
    rx: Option<mpsc::Receiver<String>>,
    opened: bool,
    toc: Vec<TocEntry>,
    drawer: String,
    focus_mode: bool,
    zen_reveal_top: bool,
    zen_reveal_bottom: bool,
    zen_revealed_at: Option<Instant>,
    search_input: Entity<InputState>,
    search_results: Vec<SearchHit>,
    searching: bool,
    echo: Option<EchoResult>,
    echo_input: String,
    echo_cfi: String,
    echo_saved: bool,
    echo_ai_rx: Option<mpsc::Receiver<anyhow::Result<PassageGenerated>>>,
    echo_ai_worker: fn(&str, &str) -> anyhow::Result<EchoResult>,
    highlight_note_target: Option<String>,
    highlight_note_input: Entity<TextareaState>,
    mood_reply: Option<(String, EchoResult)>,
    mood_rx: Option<mpsc::Receiver<anyhow::Result<mood::Generated>>>,
    mood_unsaved: Option<mood::Generated>,
    mood_worker: fn(&str, &str) -> anyhow::Result<EchoResult>,
    mood_scroll: ScrollHandle,
    mood: String,
    mood_input: Entity<TextareaState>,
    breathing_started: Option<Instant>,
    focus_timer_started: Option<Instant>,
    focus_timer_total_secs: u32,
    focus_timer_picker_open: bool,
    focus_timer_input: Entity<InputState>,
    focus_timer_with_audio: bool,
    shelf_search: Entity<InputState>,
    shelf_filter: ShelfFilter,
    rediscovery_offset: usize,
    journal_search: Entity<InputState>,
    journal_filter: JournalFilter,
    quote_card: Option<QuoteCardDraft>,
    palette_open: bool,
    palette_selected: usize,
    palette_input: Entity<InputState>,
    error: Option<String>,
    notice: Option<String>,
    confirm: Option<String>,
    focus: FocusHandle,
    draft: Option<ReadingTheme>,
    preview: Option<ReadingTheme>,
    fields: Vec<Entity<InputState>>,
    spoilers: HashSet<String>,
    audio: Option<(rodio::OutputStream, rodio::Sink, Option<SoundscapeHandle>)>,
    fading_audio: Vec<(rodio::OutputStream, rodio::Sink)>,
    chime_audio: Option<(rodio::OutputStream, rodio::Sink)>,
    dirty: bool,
    last_save: Instant,
    reader_load_started: Option<Instant>,
    reader_retry_used: bool,
    smoke: bool,
    smoke_step: u8,
    smoke_started: Instant,
    busy: bool,
    motion: MotionEngine,
    frame_scheduled: bool,
    // Feature pillars state
    mixer_open: bool,
    companion_tab: u8,
    companion_selected_char: usize,
    companion_rx: Option<mpsc::Receiver<anyhow::Result<(String, BookCompanionCache)>>>,
    tts_player: Option<tts::TtsPlayer>,
    tts_status: tts::TtsStatus,
    tts_ducking_applied: bool,
    ai_base_url_input: Entity<InputState>,
    ai_model_input: Entity<InputState>,
    ai_key_input: Entity<InputState>,
    ai_test_rx: Option<mpsc::Receiver<anyhow::Result<mood::ConnectionCheck>>>,
    discovered_ai_models: Vec<String>,
    sync_owner_input: Entity<InputState>,
    sync_repo_input: Entity<InputState>,
    sync_branch_input: Entity<InputState>,
    sync_api_base_input: Entity<InputState>,
    sync_token_input: Entity<InputState>,
    sync_rx: Option<mpsc::Receiver<anyhow::Result<sync_client::SyncOutcome>>>,
    sync_test_rx: Option<mpsc::Receiver<anyhow::Result<String>>>,
    custom_mood_worker_override: bool,
}

impl ReadingApp {
    pub fn new(store: Store, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mood_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("想说的话，可以写在这里（选填）")
                .auto_grow(4, 8)
        });
        cx.observe(&mood_input, |_, _, cx| cx.notify()).detach();
        let highlight_note_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("写下读到这一句时的想法（最多 1000 字）")
                .auto_grow(3, 6)
        });
        cx.observe(&highlight_note_input, |_, _, cx| cx.notify())
            .detach();
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索书名、人物或书中一句话"));
        cx.subscribe_in(
            &search_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) && this.drawer == "search" {
                    this.act(Action::SubmitSearch, window, cx);
                }
            },
        )
        .detach();
        let shelf_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("按书名或作者筛选藏书…"));
        cx.observe(&shelf_search, |_, _, cx| cx.notify()).detach();
        let journal_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("检索日记中的句子、心情或回响…"));
        cx.observe(&journal_search, |_, _, cx| cx.notify()).detach();
        let palette_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入指令：切主题、开声景、呼吸放松、静读、导出…")
        });
        cx.subscribe_in(
            &palette_input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    this.palette_selected = 0;
                    cx.notify();
                }
                InputEvent::PressEnter { .. } if this.palette_open => {
                    this.execute_selected_palette_command(window, cx);
                }
                _ => {}
            },
        )
        .detach();
        cx.observe(&palette_input, |_, _, cx| cx.notify()).detach();
        let focus_timer_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("1—180");
            state.set_value("30", window, cx);
            state
        });
        cx.subscribe_in(
            &focus_timer_input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } if this.focus_timer_picker_open => {
                    this.act(Action::StartCustomFocusTimer, window, cx);
                }
                _ => {}
            },
        )
        .detach();
        cx.observe(&focus_timer_input, |_, _, cx| cx.notify())
            .detach();
        let fields = (0..13)
            .map(|_| cx.new(|cx| InputState::new(window, cx)))
            .collect();
        let secrets = store.load_secrets();
        let ai_base_url_val = store.state.ai.base_url.clone();
        let ai_model_val = store.state.ai.model.clone();
        let ai_key_val = secrets.ai_api_key.clone();
        let ai_base_url_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx)
                .placeholder("Base URL 或完整接口（如 https://your-api/v1 或 http://127.0.0.1:11434/v1）");
            s.set_value(ai_base_url_val, window, cx);
            s
        });
        let ai_model_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("例如 deepseek-chat 或 qwen2.5:7b");
            s.set_value(ai_model_val, window, cx);
            s
        });
        let ai_key_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("API Key（本地 Ollama/LM Studio 无需填写）");
            s.set_value(ai_key_val, window, cx);
            s
        });
        let sync_owner_val = store.state.sync.github_owner.clone();
        let sync_repo_val = store.state.sync.github_repo.clone();
        let sync_branch_val = store.state.sync.github_branch.clone();
        let sync_api_val = store.state.sync.github_api_base.clone();
        let sync_token_val = secrets.github_token.clone();
        let sync_owner_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("你的 GitHub 用户名");
            s.set_value(sync_owner_val, window, cx);
            s
        });
        let sync_repo_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("私有同步仓库名（如 glow-reading-sync）");
            s.set_value(sync_repo_val, window, cx);
            s
        });
        let sync_branch_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("默认 main");
            s.set_value(sync_branch_val, window, cx);
            s
        });
        let sync_api_base_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("默认 https://api.github.com");
            s.set_value(sync_api_val, window, cx);
            s
        });
        let sync_token_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("GitHub Personal Access Token（仅存本机）");
            s.set_value(sync_token_val, window, cx);
            s
        });
        let smoke = std::env::args().any(|a| a == "--smoke-test");
        let app = Self {
            store,
            page: Page::Shelf,
            webview: None,
            session: String::new(),
            active_book: None,
            request: 0,
            last_event: 0,
            rx: None,
            opened: false,
            toc: vec![],
            drawer: String::new(),
            focus_mode: false,
            zen_reveal_top: false,
            zen_reveal_bottom: false,
            zen_revealed_at: None,
            search_input,
            search_results: vec![],
            searching: false,
            echo: None,
            echo_input: String::new(),
            echo_cfi: String::new(),
            echo_saved: false,
            echo_ai_rx: None,
            echo_ai_worker: mood::generate_passage,
            highlight_note_target: None,
            highlight_note_input,
            mood_reply: None,
            mood_rx: None,
            mood_unsaved: None,
            mood_worker: mood::generate,
            mood_scroll: ScrollHandle::new(),
            mood: "疲惫".into(),
            mood_input,
            breathing_started: None,
            focus_timer_started: None,
            focus_timer_total_secs: 25 * 60,
            focus_timer_picker_open: false,
            focus_timer_input,
            focus_timer_with_audio: false,
            shelf_search,
            shelf_filter: ShelfFilter::All,
            rediscovery_offset: 0,
            journal_search,
            journal_filter: JournalFilter::All,
            quote_card: None,
            palette_open: false,
            palette_selected: 0,
            palette_input,
            error: None,
            notice: None,
            confirm: None,
            focus: cx.focus_handle(),
            draft: None,
            preview: None,
            fields,
            spoilers: HashSet::new(),
            audio: None,
            fading_audio: vec![],
            chime_audio: None,
            dirty: false,
            last_save: Instant::now(),
            reader_load_started: None,
            reader_retry_used: false,
            smoke,
            smoke_step: 0,
            smoke_started: Instant::now(),
            busy: false,
            motion: MotionEngine::new(),
            frame_scheduled: false,
            mixer_open: false,
            companion_tab: 0,
            companion_selected_char: 0,
            companion_rx: None,
            tts_player: None,
            tts_status: tts::TtsStatus::default(),
            tts_ducking_applied: false,
            ai_base_url_input,
            ai_model_input,
            ai_key_input,
            ai_test_rx: None,
            discovered_ai_models: vec![],
            sync_owner_input,
            sync_repo_input,
            sync_branch_input,
            sync_api_base_input,
            sync_token_input,
            sync_rx: None,
            sync_test_rx: None,
            custom_mood_worker_override: false,
        };
        app.install_native_theme(cx);
        window.focus(&app.focus, cx);
        if !cfg!(test) {
            let window_handle = window.window_handle();
            let view = cx.weak_entity();
            cx.spawn(async move |_, cx| {
                loop {
                    futures_timer::Delay::new(Duration::from_millis(80)).await;
                    let Some(view) = view.upgrade() else {
                        break;
                    };
                    let _ = window_handle.update(cx, |_, window, cx| {
                        view.update(cx, |app, cx| app.tick(window, cx));
                    });
                }
            })
            .detach();
        }
        app
    }
    fn theme(&self) -> &ReadingTheme {
        self.preview.as_ref().unwrap_or(&self.store.state.theme)
    }
    fn display_font(&self) -> SharedString {
        if self.theme().font_asset.is_some() {
            return self.theme().font.clone().into();
        }
        if !matches!(
            self.theme().id.as_str(),
            "forest" | "paper" | "night" | "prism" | "totoro" | "spirited" | "howl" | "kiki" | "lantern"
        ) {
            return self
                .theme()
                .font
                .split(',')
                .next()
                .unwrap_or("serif")
                .trim()
                .into();
        }
        if cfg!(target_os = "windows") {
            "KaiTi".into()
        } else if cfg!(target_os = "macos") {
            "STKaiti".into()
        } else {
            "Noto Serif CJK SC".into()
        }
    }
    fn ui_font(&self) -> SharedString {
        if cfg!(target_os = "windows") {
            "Microsoft YaHei UI".into()
        } else if cfg!(target_os = "macos") {
            "PingFang SC".into()
        } else {
            "Noto Sans CJK SC".into()
        }
    }
    fn install_native_theme(&self, cx: &mut App) {
        let t = self.theme();
        if let Some(font) = &t.font_asset
            && let Ok(bytes) = fs::read(self.store.root.join("themes").join(&t.id).join(font))
        {
            let _ = cx
                .text_system()
                .add_fonts(vec![std::borrow::Cow::Owned(bytes)]);
        }
        KitTheme::change(
            if glow_core::theme::color(&t.paper).unwrap_or(0) > 0x808080 {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            },
            None,
            cx,
        );
        let kit = KitTheme::global_mut(cx);
        kit.colors.background = rgb(hex(&t.background)).into();
        kit.colors.foreground = rgb(hex(&t.text)).into();
        kit.colors.primary = rgb(hex(&t.accent)).into();
        kit.colors.primary_foreground = rgb(hex(&t.paper)).into();
        kit.colors.button_primary = kit.colors.primary;
        kit.colors.button_primary_foreground = kit.colors.primary_foreground;
        kit.tokens.button_primary = kit.colors.primary.into();
        kit.tokens.button_primary_hover = kit.colors.primary.into();
        kit.tokens.button_primary_active = kit.colors.primary.into();
        kit.colors.button_foreground = rgb(hex(&t.text)).into();
        kit.colors.muted_foreground = rgb(hex(&t.muted)).into();
        kit.colors.accent = rgb(hex(&t.background)).into();
        kit.colors.accent_foreground = rgb(hex(&t.text)).into();
        kit.radius = px(10.);
        kit.font_size = px(15.);
    }
    fn save(&mut self) {
        if let Err(e) = self.store.save() {
            self.error = Some(e.to_string());
        } else {
            self.dirty = false;
            self.last_save = Instant::now();
        }
    }
    fn send(&mut self, mut value: Value, cx: &App) {
        self.request += 1;
        value["version"] = json!(1);
        value["session"] = json!(self.session);
        value["request_id"] = json!(self.request);
        if let Some(view) = &self.webview {
            let script = format!("window.glowReceive({});", value);
            if let Err(e) = view.read(cx).raw().evaluate_script(&script) {
                self.error = Some(format!("阅读消息未送达：{e}"));
            }
        }
    }
    fn theme_json(&self) -> Value {
        let t = self.theme();
        let mut value = serde_json::to_value(t).unwrap();
        if let Some(size) = self.store.state.reader_font_size {
            value["font_size"] = json!(size);
        }
        if let Some(line_height) = self.store.state.reader_line_height {
            value["line_height"] = json!(line_height);
        }
        if let Some(width) = self.store.state.reader_width {
            value["width"] = json!(width);
        }
        if let Some(font) = &t.font_asset {
            value["font_url"] = json!(format!("{}/theme/{}/{}", platform::origin(), t.id, font));
        }
        value["warm_filter"] = json!(self.store.state.warm_filter);
        value
    }
    fn active_soundscape_preset(&self) -> SoundscapePreset {
        self.store
            .state
            .soundscape_preset
            .as_deref()
            .and_then(SoundscapePreset::from_id)
            .unwrap_or_else(|| SoundscapePreset::for_theme(&self.theme().id))
    }
    fn stop_audio_smoothly(&mut self) {
        if let Some((stream, sink, handle)) = self.audio.take() {
            if let Some(handle) = handle {
                handle.trigger_fade_out();
                self.fading_audio.retain(|(_, s)| !s.empty());
                if self.fading_audio.len() >= 3 {
                    self.fading_audio.remove(0);
                }
                self.fading_audio.push((stream, sink));
            } else {
                sink.set_volume(0.0);
                sink.stop();
            }
        }
    }
    fn active_soundscape_mix(&self) -> SoundscapeMix {
        let mix = self.store.state.soundscape_mix.clamped();
        if mix.is_silent() {
            SoundscapeMix::from_single(self.active_soundscape_preset().id())
        } else {
            mix
        }
    }
    fn start_audio(&mut self, prefer_preset: bool) -> anyhow::Result<String> {
        let vol = self.theme().volume.clamp(0.05, 1.0);
        if !prefer_preset && let Some(path) = &self.theme().audio {
            let source = rodio::Decoder::new(std::io::BufReader::new(fs::File::open(
                self.store
                    .root
                    .join("themes")
                    .join(&self.theme().id)
                    .join(path),
            )?))?;
            let (stream, handle) = rodio::OutputStream::try_default()
                .map_err(|_| anyhow::anyhow!("未检测到可用的音频输出设备"))?;
            let sink = rodio::Sink::try_new(&handle)?;
            sink.set_volume(vol);
            sink.append(source.repeat_infinite());
            self.stop_audio_smoothly();
            self.audio = Some((stream, sink, None));
            return Ok("自定义环境音".into());
        }
        let mix = self.active_soundscape_mix();
        let label = mix.summary_label();
        if let Some((_, sink, Some(handle))) = &self.audio {
            handle.apply_mix(&mix);
            handle.set_ducking(
                self.tts_status.active
                    && !self.tts_status.paused
                    && self.store.state.tts.duck_soundscape,
            );
            sink.set_volume(vol);
            return Ok(label);
        }
        let (stream, handle) = rodio::OutputStream::try_default()
            .map_err(|_| anyhow::anyhow!("未检测到可用的音频输出设备"))?;
        let sink = rodio::Sink::try_new(&handle)?;
        sink.set_volume(vol);
        let (src, stop_handle) = SoundscapeSource::with_mix_handle(&mix);
        if self.tts_status.active
            && !self.tts_status.paused
            && self.store.state.tts.duck_soundscape
        {
            stop_handle.set_ducking(true);
        }
        sink.append(src);
        self.stop_audio_smoothly();
        self.audio = Some((stream, sink, Some(stop_handle)));
        Ok(label)
    }
    fn play_chime(&mut self) {
        if let Ok((stream, handle)) = rodio::OutputStream::try_default()
            && let Ok(sink) = rodio::Sink::try_new(&handle)
        {
            sink.set_volume((self.theme().volume * 1.1).clamp(0.32, 0.85));
            sink.append(ChimeSource::new());
            self.chime_audio = Some((stream, sink));
        }
    }
    fn pet_celebration_origin(&self, window: &Window) -> Point<Pixels> {
        let sz = window.viewport_size();
        let w = f32::from(sz.width).max(320.0);
        let h = f32::from(sz.height).max(240.0);
        match self.page {
            Page::Reader => point(px((w - 220.0).max(160.0)), px((h - 32.0).max(80.0))),
            Page::Mood => point(px(580.0_f32.min(w - 80.0)), px(150.0)),
            _ => point(px((w - 92.0).max(160.0)), px(44.0)),
        }
    }
    fn is_narrow_reader(&self, window: &Window) -> bool {
        f32::from(window.viewport_size().width) - if self.focus_mode { 0. } else { 224. } - 364.
            < 480.
    }
    fn handle_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open {
            self.palette_open = false;
            window.focus(&self.focus, cx);
            cx.notify();
        } else if self.mixer_open {
            self.mixer_open = false;
            window.focus(&self.focus, cx);
            cx.notify();
        } else if self.focus_timer_picker_open {
            self.focus_timer_picker_open = false;
            window.focus(&self.focus, cx);
            cx.notify();
        } else if self.quote_card.is_some() {
            self.quote_card = None;
            window.focus(&self.focus, cx);
            cx.notify();
        } else if self.confirm.is_some() {
            self.confirm = None;
            cx.notify();
        } else if self.highlight_note_target.is_some() {
            self.highlight_note_target = None;
            window.focus(&self.focus, cx);
            cx.notify();
        } else if !self.drawer.is_empty() {
            self.act(Action::CloseEcho, window, cx);
            window.focus(&self.focus, cx);
        } else if self.focus_mode {
            self.act(Action::Focus, window, cx);
        } else {
            self.act(Action::CloseEcho, window, cx);
        }
    }
    fn configure(&mut self, cx: &App) {
        self.send(
            json!({"kind":"configure","theme":self.theme_json(),"mode":self.store.state.mode}),
            cx,
        );
    }
    fn open(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.save();
        let book = self
            .store
            .state
            .books
            .iter()
            .find(|b| b.id == id)
            .ok_or_else(|| anyhow::anyhow!("书籍已不在书架中"))?;
        let bytes = fs::read(
            self.store
                .root
                .join("books")
                .join(&book.id)
                .join("book.epub"),
        )?;
        if let Some(view) = self.webview.take() {
            view.update(cx, |v, _| v.hide());
        }
        self.session = format!(
            "{}-{}",
            id,
            glow_core::model::now().replace([':', '.'], "-")
        );
        let (tx, rx) = mpsc::sync_channel(128);
        self.webview = Some(platform::create_reader(
            window,
            cx,
            self.session.clone(),
            bytes,
            self.store.root.clone(),
            tx,
        )?);
        self.rx = Some(rx);
        self.request = 0;
        self.last_event = 0;
        self.opened = false;
        self.reader_load_started = Some(Instant::now());
        self.reader_retry_used = false;
        self.active_book = Some(id.into());
        self.page = Page::Reader;
        self.drawer.clear();
        self.search_results.clear();
        self.searching = false;
        self.echo = None;
        self.echo_input.clear();
        self.echo_cfi.clear();
        self.echo_saved = false;
        self.echo_ai_rx = None;
        self.highlight_note_target = None;
        self.zen_reveal_top = false;
        self.zen_reveal_bottom = false;
        self.zen_revealed_at = None;
        self.toc.clear();
        self.search_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.store.state.last_book = Some(id.into());
        self.save();
        Ok(())
    }
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fading_audio.retain(|(_, sink)| !sink.empty());
        if self
            .chime_audio
            .as_ref()
            .is_some_and(|(_, sink)| sink.empty())
        {
            self.chime_audio = None;
        }
        if let Some(result) = self.mood_rx.as_ref().map(|rx| rx.try_recv()) {
            match result {
                Ok(result) => {
                    self.mood_rx = None;
                    self.finish_mood(result, window, cx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.mood_rx = None;
                    self.error = Some("回复任务已中断，文字已保留，请重试。".into());
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(result) = self.echo_ai_rx.as_ref().map(|rx| rx.try_recv()) {
            match result {
                Ok(result) => {
                    self.echo_ai_rx = None;
                    self.finish_ai_passage(result, cx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.echo_ai_rx = None;
                    self.error = Some("AI 共读任务已中断，可以稍后重试。".into());
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(started) = self.breathing_started {
            if started.elapsed() >= Duration::from_secs(60) {
                self.breathing_started = None;
                let origin = self.pet_celebration_origin(window);
                self.motion
                    .trigger_pet_celebration(origin, self.theme().reduced_motion);
                self.notice = Some("已完成 1 分钟 4-7-8 呼吸放松，愿此刻慢下来。".into());
            }
            cx.notify();
        }
        if let Some(started) = self.focus_timer_started {
            if started.elapsed().as_secs() >= self.focus_timer_total_secs as u64 {
                self.focus_timer_started = None;
                let mins = (self.focus_timer_total_secs / 60).max(1);
                let today = today_ymd();
                let total_today = self
                    .store
                    .add_reading_minutes(mins, &today)
                    .unwrap_or(self.store.state.reading_minutes_today);
                self.play_chime();
                let origin = self.pet_celebration_origin(window);
                self.motion
                    .trigger_pet_celebration(origin, self.theme().reduced_motion);
                self.notice = Some(format!(
                    "棱镜喵：静读光瓶已蓄满 {mins} 分钟，今天共静心阅读 {total_today} 分钟。"
                ));
                if self.store.state.tts.sleep_with_timer && self.tts_status.active {
                    if let Some(player) = &self.tts_player {
                        player.stop();
                    }
                    self.notice = Some(format!(
                        "棱镜喵：静读光瓶已蓄满 {mins} 分钟，并已轻声停止听书，晚安。"
                    ));
                }
            }
            cx.notify();
        }
        self.tick_features(cx);
        let messages = self
            .rx
            .as_ref()
            .map(|rx| rx.try_iter().take(64).collect::<Vec<_>>())
            .unwrap_or_default();
        let changed = !messages.is_empty();
        for message in messages {
            if let Ok(event) = glow_core::bridge::parse_event(&message, &self.session) {
                if event.request_id <= self.last_event {
                    continue;
                }
                self.last_event = event.request_id;
                if event.kind == "palette" {
                    self.act(Action::TogglePalette, window, cx);
                    continue;
                }
                if event.kind == "escape" {
                    self.handle_escape(window, cx);
                    continue;
                }
                let open_note = event.kind == "highlight" && event.open_note;
                let celebrated_highlight = event.kind == "highlight";
                self.event(event, cx);
                if celebrated_highlight && self.error.is_none() {
                    let origin = self.pet_celebration_origin(window);
                    self.motion
                        .trigger_pet_celebration(origin, self.theme().reduced_motion);
                }
                if open_note
                    && self.error.is_none()
                    && let Some(target_id) = self.highlight_note_target.clone()
                {
                    let existing = self
                        .store
                        .state
                        .books
                        .iter()
                        .find(|b| Some(&b.id) == self.active_book.as_ref())
                        .and_then(|b| b.highlights.iter().find(|h| h.id == target_id))
                        .and_then(|h| h.note.clone())
                        .unwrap_or_default();
                    self.highlight_note_input
                        .update(cx, |s, cx| s.set_value(existing, window, cx));
                    let focus = self.highlight_note_input.read(cx).focus_handle(cx).clone();
                    window.focus(&focus, cx);
                }
            }
        }
        if self.dirty && self.last_save.elapsed() > Duration::from_millis(800) {
            self.save();
        }
        self.recover_stalled_reader(window, cx);
        if self.smoke {
            self.smoke_tick(window, cx);
        }
        if changed {
            cx.notify();
        }
    }
    fn recover_stalled_reader(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let stalled = self
            .reader_load_started
            .is_some_and(|started| started.elapsed() > Duration::from_secs(20));
        if self.page != Page::Reader
            || self.opened
            || self.busy
            || self.confirm.is_some()
            || !stalled
        {
            return;
        }
        self.reader_load_started = None;
        if self.reader_retry_used {
            self.error =
                Some("这本书的书页加载超时。请检查 EPUB 是否完整，或返回书架后重新打开。".into());
            cx.notify();
            return;
        }
        let Some(book_id) = self.active_book.clone() else {
            return;
        };
        self.error = None;
        self.notice = Some("书页连接较慢，正在重新打开一次…".into());
        match self.open(&book_id, window, cx) {
            Ok(()) => {
                self.reader_retry_used = true;
                cx.notify();
            }
            Err(error) => {
                self.reader_load_started = None;
                self.error = Some(format!("重新打开书页失败：{error}"));
                cx.notify();
            }
        }
    }
    fn event(&mut self, event: ReaderEvent, cx: &mut Context<Self>) {
        match event.kind.as_str() {
            "ready" => {
                self.reader_load_started = Some(Instant::now());
                let (cfi, highlights) = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                    .map(|book| {
                        (
                            book.location.as_ref().map(|location| location.cfi.clone()),
                            book.highlights
                                .iter()
                                .map(|highlight| {
                                    json!({
                                        "id": highlight.id,
                                        "cfi": highlight.cfi,
                                        "text": highlight.text,
                                        "color": highlight.color,
                                    })
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .unwrap_or_default();
                self.send(json!({"kind":"open","url":format!("{}/book.epub",platform::origin()),"cfi":cfi,"highlights":highlights,"theme":self.theme_json(),"mode":self.store.state.mode}),cx);
            }
            "opened" => {
                self.error = None;
                self.opened = true;
                self.reader_load_started = None;
                self.reader_retry_used = false;
            }
            "toc" => self.toc = event.entries,
            "location" => {
                if let Some(book) = self
                    .store
                    .state
                    .books
                    .iter_mut()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                {
                    book.location = Some(Location {
                        book_id: book.id.clone(),
                        cfi: event.cfi,
                    });
                    if let Some(progress) = event.progress {
                        book.progress = Some(progress.clamp(0.0, 1.0));
                    }
                    if !event.chapter.is_empty() {
                        book.chapter = Some(event.chapter.clone());
                    }
                    book.updated_at = Some(glow_core::model::now());
                    self.dirty = true;
                }
            }
            "echo" => {
                let result = echo::respond(&event.text, None);
                self.echo_input = event.text;
                self.echo_cfi = event.cfi;
                self.echo = Some(result);
                self.echo_saved = false;
                self.echo_ai_rx = None;
                self.drawer = "echo".into();
                self.spoilers.clear();
            }
            "highlight" => {
                let Some(book_id) = self.active_book.clone() else {
                    return;
                };
                let cfi = event.cfi.clone();
                let previous = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| b.id == book_id)
                    .and_then(|b| b.highlights.iter().find(|h| h.cfi == cfi))
                    .cloned();
                let color = (!event.color.is_empty()).then_some(event.color);
                match self.store.add_highlight_styled(
                    &book_id,
                    event.text,
                    cfi.clone(),
                    color,
                    None,
                ) {
                    Ok((highlight, inserted)) => {
                        self.error = None;
                        self.send(
                            json!({
                                "kind": "highlight",
                                "highlight": {
                                    "id": highlight.id,
                                    "cfi": highlight.cfi,
                                    "text": highlight.text,
                                    "color": highlight.color,
                                }
                            }),
                            cx,
                        );
                        if event.open_note {
                            self.drawer = "highlights".into();
                            self.highlight_note_target = Some(highlight.id);
                            self.notice = Some("棱镜喵：已为你定位划线，写下此刻想法吧。".into());
                        } else if inserted {
                            self.notice = Some("棱镜喵：这句微光已经悄悄划下来啦。".into());
                        } else {
                            self.notice = Some("已更新这条划线的颜色。".into());
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        // Roll back any optimistic highlight rendered by the WebView
                        if let Some(prev) = previous {
                            self.send(
                                json!({
                                    "kind": "highlight",
                                    "highlight": {
                                        "id": prev.id,
                                        "cfi": prev.cfi,
                                        "text": prev.text,
                                        "color": prev.color,
                                    }
                                }),
                                cx,
                            );
                        } else if !cfi.is_empty() {
                            self.send(json!({"kind": "unhighlight", "cfi": cfi}), cx);
                        }
                    }
                }
            }
            "copy" => {
                let title = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                    .map(|b| b.title.as_str())
                    .unwrap_or("微光藏书");
                let excerpt = format!("“{}”\n——《{}》", event.text.trim(), title);
                cx.write_to_clipboard(ClipboardItem::new_string(excerpt));
                self.notice = Some("已复制带书名出处的摘录到剪贴板。".into());
            }
            "edge" => {
                if self.focus_mode && self.page == Page::Reader {
                    match event.text.as_str() {
                        "top" => {
                            self.zen_reveal_top = true;
                            self.zen_reveal_bottom = false;
                            self.zen_revealed_at = Some(Instant::now());
                        }
                        "bottom" => {
                            self.zen_reveal_bottom = true;
                            self.zen_reveal_top = false;
                            self.zen_revealed_at = Some(Instant::now());
                        }
                        _ => {
                            if self
                                .zen_revealed_at
                                .is_none_or(|t| t.elapsed() >= Duration::from_millis(320))
                            {
                                self.zen_reveal_top = false;
                                self.zen_reveal_bottom = false;
                            }
                        }
                    }
                }
            }
            "search" => {
                self.searching = false;
                self.search_results = event.results;
            }
            "escape" => {
                if self.palette_open {
                    self.palette_open = false;
                } else if self.quote_card.is_some() {
                    self.quote_card = None;
                } else if self.highlight_note_target.is_some() {
                    self.highlight_note_target = None;
                } else if !self.drawer.is_empty() {
                    self.drawer.clear();
                } else {
                    self.focus_mode = false;
                    self.zen_reveal_top = false;
                    self.zen_reveal_bottom = false;
                }
            }
            "error" => {
                self.error = Some(event.text);
                self.opened = false;
                self.reader_load_started = None;
            }
            _ => {}
        }
    }
    fn act(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.error = None;
        self.notice = None;
        if let Err(e) = self.perform(action, window, cx) {
            self.error = Some(e.to_string());
        }
        cx.notify();
    }
    fn perform(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        match action {
            Action::Page(page) => {
                let was_previewing = self.preview.is_some();
                self.page = page;
                self.preview = None;
                self.draft = None;
                self.install_native_theme(cx);
                if was_previewing && self.opened {
                    self.configure(cx);
                }
                self.save();
            }
            Action::Import => {
                self.hide(cx);
                self.busy = true;
                let dialog = rfd::AsyncFileDialog::new()
                    .set_parent(window)
                    .set_title("选择要带回家的书")
                    .add_filter("电子书", &["epub", "txt"]);
                cx.spawn_in(window, async move |this, cx| {
                    let files = dialog.pick_files().await;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(files) = files {
                            this.begin_import(
                                files.into_iter().map(|f| f.path().to_owned()).collect(),
                                cx,
                            );
                        } else {
                            this.busy = false;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
            Action::Open(id) => self.open(&id, window, cx)?,
            Action::OpenAt(target) => {
                let (id, cfi) = *target;
                if let Some(book) = self.store.state.books.iter_mut().find(|b| b.id == id) {
                    book.location = Some(Location {
                        book_id: id.clone(),
                        cfi,
                    });
                }
                self.open(&id, window, cx)?;
            }
            Action::Remove(id) => self.confirm = Some(format!("book:{id}")),
            Action::ConfirmRemove => {
                if let Some(value) = self.confirm.take()
                    && let Some(id) = value.strip_prefix("book:")
                {
                    self.store.remove_book(id)?;
                    if self.active_book.as_deref() == Some(id) {
                        self.hide(cx);
                        self.webview = None;
                        self.active_book = None;
                    }
                }
            }
            Action::Cancel => self.confirm = None,
            Action::Mode => {
                self.store.state.mode = if self.store.state.mode == "scrolled" {
                    "paginated"
                } else {
                    "scrolled"
                }
                .into();
                self.configure(cx);
                self.save();
            }
            Action::Prev => self.send(json!({"kind":"prev"}), cx),
            Action::Next => self.send(json!({"kind":"next"}), cx),
            Action::Focus => {
                self.focus_mode = !self.focus_mode;
                self.zen_reveal_top = false;
                self.zen_reveal_bottom = false;
                self.zen_revealed_at = None;
            }
            Action::Toc => self.drawer = if self.drawer == "toc" { "" } else { "toc" }.into(),
            Action::Search => {
                if self.drawer == "search" {
                    self.drawer.clear();
                } else {
                    self.drawer = "search".into();
                    let focus = self.search_input.read(cx).focus_handle(cx).clone();
                    window.focus(&focus, cx);
                }
            }
            Action::SubmitSearch => {
                anyhow::ensure!(self.opened, "书页还在整理，请稍后再搜索");
                let query = self.search_input.read(cx).value().trim().to_owned();
                anyhow::ensure!(!query.is_empty(), "请输入要搜索的文字");
                anyhow::ensure!(query.chars().count() <= 100, "搜索文字请控制在 100 字以内");
                self.search_results.clear();
                self.searching = true;
                self.send(json!({"kind":"search","query":query}), cx);
            }
            Action::Highlights => {
                self.highlight_note_target = None;
                self.drawer = if self.drawer == "highlights" {
                    ""
                } else {
                    "highlights"
                }
                .into();
            }
            Action::Typography => {
                self.drawer = if self.drawer == "type" { "" } else { "type" }.into();
            }
            Action::TypeSize(delta) => {
                let current = self
                    .store
                    .state
                    .reader_font_size
                    .unwrap_or_else(|| self.theme().font_size);
                self.store.state.reader_font_size =
                    Some((current + delta as f32).clamp(14.0, 36.0));
                self.configure(cx);
                self.save();
            }
            Action::TypeLeading(delta) => {
                let current = self
                    .store
                    .state
                    .reader_line_height
                    .unwrap_or_else(|| self.theme().line_height);
                let next = (current + delta as f32 * 0.1).clamp(1.3, 2.6);
                self.store.state.reader_line_height = Some((next * 10.0).round() / 10.0);
                self.configure(cx);
                self.save();
            }
            Action::TypeWidth(delta) => {
                let current = self
                    .store
                    .state
                    .reader_width
                    .unwrap_or_else(|| self.theme().width);
                self.store.state.reader_width =
                    Some((current as i32 + delta as i32 * 40).clamp(400, 1100) as u32);
                self.configure(cx);
                self.save();
            }
            Action::ToggleWarmFilter => {
                self.store.state.warm_filter = !self.store.state.warm_filter;
                self.configure(cx);
                self.save();
                self.notice = Some(if self.store.state.warm_filter {
                    "已开启夜间暖光护眼滤镜。".into()
                } else {
                    "已关闭暖光护眼滤镜。".into()
                });
            }
            Action::ResetTypography => {
                self.store.state.reader_font_size = None;
                self.store.state.reader_line_height = None;
                self.store.state.reader_width = None;
                self.store.state.warm_filter = false;
                self.configure(cx);
                self.save();
                self.notice = Some("已恢复当前主题的默认排版。".into());
            }
            Action::Highlight => {
                let book_id = self
                    .active_book
                    .clone()
                    .context("请先打开一本可以划线的书")?;
                anyhow::ensure!(!self.echo_cfi.is_empty(), "请先选择一段文字");
                let (highlight, inserted) = self.store.add_highlight_styled(
                    &book_id,
                    self.echo_input.clone(),
                    self.echo_cfi.clone(),
                    Some("amber".into()),
                    None,
                )?;
                self.send(
                    json!({
                        "kind": "highlight",
                        "highlight": {
                            "id": highlight.id,
                            "cfi": highlight.cfi,
                            "text": highlight.text,
                            "color": highlight.color,
                        }
                    }),
                    cx,
                );
                if inserted {
                    let origin = self.pet_celebration_origin(window);
                    self.motion
                        .trigger_pet_celebration(origin, self.theme().reduced_motion);
                    self.notice = Some("已留下划线，之后可以在「划线」里找回。".into());
                } else {
                    self.notice = Some("这段文字已经留过划线。".into());
                }
            }
            Action::HighlightColor(color) => {
                let book_id = self
                    .active_book
                    .clone()
                    .context("请先打开一本可以划线的书")?;
                anyhow::ensure!(!self.echo_cfi.is_empty(), "请先选择一段文字");
                let (highlight, inserted) = self.store.add_highlight_styled(
                    &book_id,
                    self.echo_input.clone(),
                    self.echo_cfi.clone(),
                    Some(color),
                    None,
                )?;
                self.send(
                    json!({
                        "kind": "highlight",
                        "highlight": {
                            "id": highlight.id,
                            "cfi": highlight.cfi,
                            "text": highlight.text,
                            "color": highlight.color,
                        }
                    }),
                    cx,
                );
                let origin = self.pet_celebration_origin(window);
                self.motion
                    .trigger_pet_celebration(origin, self.theme().reduced_motion);
                self.notice = Some(if inserted {
                    "棱镜喵：已按你挑选的颜色留住这句微光。".into()
                } else {
                    "已更新这条划线的颜色。".into()
                });
            }
            Action::RecolorHighlight(pair) => {
                let (id, color) = *pair;
                let Some(book_id) = self.active_book.clone() else {
                    return Ok(());
                };
                if let Some(highlight) =
                    self.store
                        .update_highlight(&book_id, &id, Some(Some(color)), None)?
                {
                    self.send(
                        json!({
                            "kind": "highlight",
                            "highlight": {
                                "id": highlight.id,
                                "cfi": highlight.cfi,
                                "text": highlight.text,
                                "color": highlight.color,
                            }
                        }),
                        cx,
                    );
                    self.notice = Some("已更新划线颜色。".into());
                }
            }
            Action::EditHighlightNote(id) => {
                let existing = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                    .and_then(|b| b.highlights.iter().find(|h| h.id == id))
                    .and_then(|h| h.note.clone())
                    .unwrap_or_default();
                self.highlight_note_target = Some(id);
                self.highlight_note_input
                    .update(cx, |s, cx| s.set_value(existing, window, cx));
                let focus = self.highlight_note_input.read(cx).focus_handle(cx).clone();
                window.focus(&focus, cx);
            }
            Action::SaveHighlightNote => {
                let Some(book_id) = self.active_book.clone() else {
                    return Ok(());
                };
                let Some(target_id) = self.highlight_note_target.clone() else {
                    return Ok(());
                };
                let note = self.highlight_note_input.read(cx).value().to_string();
                self.store
                    .update_highlight(&book_id, &target_id, None, Some(Some(note)))?;
                self.highlight_note_target = None;
                self.notice = Some("批注已保存。".into());
            }
            Action::CancelHighlightNote => {
                self.highlight_note_target = None;
            }
            Action::DeleteHighlight(id) => {
                let Some(book_id) = self.active_book.clone() else {
                    return Ok(());
                };
                if self.highlight_note_target.as_deref() == Some(id.as_str()) {
                    self.highlight_note_target = None;
                }
                if let Some(highlight) = self.store.remove_highlight(&book_id, &id)? {
                    self.send(json!({"kind":"unhighlight","cfi":highlight.cfi}), cx);
                    self.notice = Some("已删除这条划线，书页内容不受影响。".into());
                }
            }
            Action::SaveEchoToJournal => {
                let echo = self.echo.clone().context("当前没有可保存的回响")?;
                anyhow::ensure!(!self.echo_saved, "这条回响已经存入私人日记了");
                let location = self
                    .active_book
                    .as_ref()
                    .filter(|_| glow_core::model::valid_cfi(&self.echo_cfi))
                    .map(|id| Location {
                        book_id: id.clone(),
                        cfi: self.echo_cfi.clone(),
                    });
                self.store
                    .record("selection", self.echo_input.clone(), None, echo, location)?;
                self.echo_saved = true;
                self.notice = Some("已存入私人微光手账。".into());
            }
            Action::AskAiPassage => {
                if self.echo_ai_rx.is_some() {
                    return Ok(());
                }
                let passage = self.echo_input.trim().to_owned();
                anyhow::ensure!(!passage.is_empty(), "请先在书中选中一段文字");
                let book_id = self.active_book.clone();
                let cfi = self.echo_cfi.clone();
                let book_title = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|b| Some(&b.id) == book_id.as_ref())
                    .map(|b| b.title.clone())
                    .unwrap_or_else(|| "微光藏书".into());
                let worker = self.echo_ai_worker;
                let use_custom = self.custom_mood_worker_override
                    || (worker as *const () as usize)
                        != (mood::generate_passage as *const () as usize);
                let ai_settings = self.store.state.ai.clone();
                let secrets = self.store.load_secrets();
                let (sender, receiver) = mpsc::channel();
                self.echo_ai_rx = Some(receiver);
                std::thread::spawn(move || {
                    let generated = if use_custom {
                        worker(&passage, &book_title)
                    } else {
                        mood::generate_passage_with_settings(
                            &ai_settings,
                            &secrets,
                            &passage,
                            &book_title,
                        )
                    };
                    let outcome = generated.map(|result| PassageGenerated {
                        book_id,
                        passage,
                        cfi,
                        result,
                    });
                    let _ = sender.send(outcome);
                });
            }
            Action::CancelAiPassage => {
                self.echo_ai_rx = None;
                self.notice = Some("已停止等待 AI 段落共读。已发出的请求可能仍会计费。".into());
            }
            Action::Bookmarks => {
                self.drawer = if self.drawer == "bookmarks" {
                    ""
                } else {
                    "bookmarks"
                }
                .into()
            }
            Action::Bookmark => {
                if let Some(b) = self
                    .store
                    .state
                    .books
                    .iter_mut()
                    .find(|b| Some(&b.id) == self.active_book.as_ref())
                    && let Some(location) = &b.location
                    && !b.bookmarks.iter().any(|m| m.location == *location)
                {
                    b.bookmarks.push(Bookmark {
                        label: format!("书签 {}", b.bookmarks.len() + 1),
                        location: location.clone(),
                    });
                    self.save();
                    let origin = self.pet_celebration_origin(window);
                    self.motion
                        .trigger_pet_celebration(origin, self.theme().reduced_motion);
                    self.notice = Some("棱镜喵：已替你折好这一页的书签。".into());
                }
            }
            Action::Locate(target) => {
                self.send(
                    if target.starts_with("epubcfi(") {
                        json!({"kind":"locate","cfi":target})
                    } else {
                        json!({"kind":"locate","href":target})
                    },
                    cx,
                );
                self.drawer.clear();
            }
            Action::CloseEcho => {
                self.highlight_note_target = None;
                self.drawer.clear();
                self.send(json!({"kind":"restore"}), cx);
            }
            Action::Mood(mood) => self.mood = mood,
            Action::CancelMood => {
                self.mood_rx = None;
                self.notice = Some("已停止等待，文字已保留。已发出的请求可能仍会计费。".into());
            }
            Action::SubmitMood => {
                if self.mood_rx.is_some() {
                    return Ok(());
                }
                let text = self.mood_input.read(cx).value().trim().to_string();
                anyhow::ensure!(text.chars().count() <= 1000, "心情文字请控制在 1000 字以内");
                if let Some(generated) = self.mood_unsaved.take()
                    && generated.input == text
                    && generated.mood == self.mood
                {
                    self.finish_mood(Ok(generated), window, cx);
                    return Ok(());
                }
                let selected_mood = self.mood.clone();
                let worker = self.mood_worker;
                let use_custom = self.custom_mood_worker_override
                    || (worker as *const () as usize) != (mood::generate as *const () as usize);
                let ai_settings = self.store.state.ai.clone();
                let secrets = self.store.load_secrets();
                let (sender, receiver) = mpsc::channel();
                self.mood_rx = Some(receiver);
                std::thread::spawn(move || {
                    let generated = if use_custom {
                        worker(&text, &selected_mood)
                    } else {
                        mood::generate_with_settings(&ai_settings, &secrets, &text, &selected_mood)
                    };
                    let result = generated.map(|result| mood::Generated {
                        input: text,
                        mood: selected_mood,
                        result,
                    });
                    let _ = sender.send(result);
                });
            }
            Action::ToggleBreathing => {
                if self.breathing_started.is_some() {
                    self.breathing_started = None;
                    self.notice = Some("已结束呼吸引导，随时可以再坐一会儿。".into());
                } else {
                    self.breathing_started = Some(Instant::now());
                    self.schedule_motion_frame(window, cx);
                    self.notice =
                        Some("已开启 4-7-8 呼吸光环：吸气 4 秒 · 屏息 7 秒 · 呼气 8 秒。".into());
                }
            }
            Action::ToggleTimerPicker => {
                self.focus_timer_picker_open = !self.focus_timer_picker_open;
                if self.focus_timer_picker_open {
                    self.palette_open = false;
                    self.quote_card = None;
                    let cur = self.focus_timer_input.read(cx).value().trim().to_owned();
                    if cur.parse::<u32>().is_err() {
                        let fallback = (self.focus_timer_total_secs / 60).clamp(1, 180);
                        self.focus_timer_input.update(cx, |s, cx| {
                            s.set_value(fallback.to_string(), window, cx)
                        });
                    }
                    let focus = self.focus_timer_input.read(cx).focus_handle(cx).clone();
                    window.focus(&focus, cx);
                } else {
                    window.focus(&self.focus, cx);
                }
            }
            Action::CloseTimerPicker => {
                self.focus_timer_picker_open = false;
                window.focus(&self.focus, cx);
            }
            Action::StepCustomTimer(delta) => {
                let current = self
                    .focus_timer_input
                    .read(cx)
                    .value()
                    .trim()
                    .parse::<i32>()
                    .unwrap_or((self.focus_timer_total_secs / 60).clamp(1, 180) as i32);
                let next = (current + delta).clamp(1, 180);
                self.focus_timer_input
                    .update(cx, |s, cx| s.set_value(next.to_string(), window, cx));
            }
            Action::ToggleTimerWithAudio => {
                self.focus_timer_with_audio = !self.focus_timer_with_audio;
            }
            Action::StartCustomFocusTimer => {
                let raw = self.focus_timer_input.read(cx).value().trim().to_owned();
                let mins = raw
                    .parse::<u32>()
                    .ok()
                    .filter(|m| (1..=180).contains(m))
                    .ok_or_else(|| anyhow::anyhow!("请输入 1—180 之间的整数分钟数"))?;
                self.perform(Action::StartFocusTimer(mins), window, cx)?;
            }
            Action::StartFocusTimer(minutes) => {
                let mins = minutes.clamp(1, 180);
                self.focus_timer_total_secs = mins * 60;
                self.focus_timer_started = Some(Instant::now());
                self.focus_timer_picker_open = false;
                self.focus_timer_input
                    .update(cx, |s, cx| s.set_value(mins.to_string(), window, cx));
                window.focus(&self.focus, cx);
                let audio_note = if self.focus_timer_with_audio && self.audio.is_none() {
                    match self.start_audio(false) {
                        Ok(label) => format!("，并已同步开启「{label}」声景"),
                        Err(_) => String::new(),
                    }
                } else {
                    String::new()
                };
                self.schedule_motion_frame(window, cx);
                self.notice = Some(format!(
                    "棱镜喵：已开启 {mins} 分钟静读光瓶{audio_note}，不急不躁，慢慢读吧。"
                ));
            }
            Action::StopFocusTimer => {
                let elapsed_mins = self
                    .focus_timer_started
                    .take()
                    .map(|started| (started.elapsed().as_secs() / 60) as u32)
                    .unwrap_or(0);
                self.focus_timer_picker_open = false;
                window.focus(&self.focus, cx);
                if elapsed_mins > 0 {
                    let today = today_ymd();
                    let total_today = self
                        .store
                        .add_reading_minutes(elapsed_mins, &today)
                        .unwrap_or(self.store.state.reading_minutes_today);
                    self.notice = Some(format!(
                        "已温柔收起光瓶，为你记下刚才静读的 {elapsed_mins} 分钟（今日共 {total_today} 分钟）。"
                    ));
                } else {
                    self.notice = Some("已结束本次静读光瓶。".into());
                }
            }
            Action::Favorite(id) => {
                self.store.edit_journal(|entries| {
                    if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
                        entry.favorite = !entry.favorite;
                    }
                })?;
            }
            Action::DeleteEntry(id) => {
                self.store
                    .edit_journal(|entries| entries.retain(|entry| entry.id != id))?;
            }
            Action::ClearJournal => self.confirm = Some("journal".into()),
            Action::ConfirmClear => {
                self.store.edit_journal(Vec::clear)?;
                self.confirm = None;
            }
            Action::ShelfFilter(filter) => self.shelf_filter = filter,
            Action::NextRediscovery => {
                self.rediscovery_offset = self.rediscovery_offset.wrapping_add(1);
            }
            Action::JournalFilter(filter) => self.journal_filter = filter,
            Action::ExportJournalMarkdown => {
                anyhow::ensure!(
                    !self.store.state.journal.is_empty(),
                    "日记还是空的，先留下几条回响吧"
                );
                self.hide(cx);
                let markdown = format_journal_markdown(&self.store.state.journal);
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("导出微光手账 (Markdown)")
                    .set_file_name("微光手账.md")
                    .add_filter("Markdown", &["md"])
                    .save_file()
                {
                    glow_core::store::atomic_write(&path, markdown.as_bytes())?;
                    self.notice = Some("已导出为 Markdown 手账文件。".into());
                }
            }
            Action::OpenQuoteCard(draft) => {
                self.palette_open = false;
                self.quote_card = Some(*draft);
            }
            Action::QuoteCardStyle(style) => {
                if let Some(card) = &mut self.quote_card {
                    card.style = style;
                }
            }
            Action::SaveQuoteCardPng => {
                let draft = self.quote_card.clone().context("请先选择要生成的书摘")?;
                self.hide(cx);
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("保存高清微光书摘卡")
                    .set_file_name("微光书摘卡.png")
                    .add_filter("PNG 图片", &["png"])
                    .save_file()
                {
                    quotecard::save_png(&draft, &path)?;
                    self.notice = Some("微光书摘卡已保存为高清 PNG 图片。".into());
                }
            }
            Action::CopyQuoteCardText => {
                if let Some(card) = &self.quote_card {
                    let mut text =
                        format!("“{}”\n—— {}", card.text.trim(), card.attribution.trim());
                    if let Some(note) = card
                        .note
                        .as_deref()
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                    {
                        text.push_str(&format!("\n批注：{note}"));
                    }
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    self.notice = Some("已复制书摘文字到剪贴板。".into());
                }
            }
            Action::CloseQuoteCard => {
                self.quote_card = None;
                window.focus(&self.focus, cx);
            }
            Action::TogglePalette => {
                self.palette_open = !self.palette_open;
                self.palette_selected = 0;
                if self.palette_open {
                    self.palette_input
                        .update(cx, |s, cx| s.set_value("", window, cx));
                    let focus = self.palette_input.read(cx).focus_handle(cx).clone();
                    window.focus(&focus, cx);
                } else {
                    window.focus(&self.focus, cx);
                }
            }
            Action::ClosePalette => {
                self.palette_open = false;
                window.focus(&self.focus, cx);
            }
            Action::PickTheme(index) => {
                let all = self.all_themes();
                if let Some(t) = all.get(index) {
                    let was_playing = self.audio.is_some();
                    let prev_volume = self.store.state.theme.volume;
                    let prev_reduced_motion = self.store.state.theme.reduced_motion;
                    let mut next = t.clone();
                    if index < themes().len() {
                        next.volume = prev_volume;
                        next.reduced_motion = prev_reduced_motion;
                    }
                    self.store.state.theme = next;
                    self.store.state.soundscape_preset = None;
                    let paired = SoundscapePreset::for_theme(&self.store.state.theme.id);
                    self.store.state.soundscape_mix = SoundscapeMix::from_single(paired.id());
                    self.stop_audio_smoothly();
                    self.preview = None;
                    self.install_native_theme(cx);
                    self.configure(cx);
                    if was_playing {
                        let _ = self.start_audio(false);
                    }
                    self.save();
                }
            }
            Action::EditTheme => {
                let t = self.store.duplicate_theme(&self.store.state.theme)?;
                self.fill_fields(&t, window, cx);
                self.draft = Some(t);
            }
            Action::PreviewTheme => {
                let t = self.read_fields(cx)?;
                glow_core::theme::validate(&t)?;
                self.preview = Some(t);
                self.install_native_theme(cx);
                self.configure(cx);
            }
            Action::SaveTheme => {
                let t = self.read_fields(cx)?;
                self.store.save_theme(t)?;
                self.stop_audio_smoothly();
                self.draft = None;
                self.preview = None;
                self.install_native_theme(cx);
                self.configure(cx);
                self.notice = Some("主题已经保存。".into());
            }
            Action::CancelTheme => {
                self.draft = None;
                self.preview = None;
                self.install_native_theme(cx);
                self.configure(cx);
            }
            Action::ResetTheme => {
                let mut t = ReadingTheme::default();
                if let Some(d) = &self.draft {
                    t.id = d.id.clone();
                }
                self.fill_fields(&t, window, cx);
                self.draft = Some(t);
            }
            Action::ImportTheme => {
                self.hide(cx);
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("选择微光主题包")
                    .add_filter("微光主题", &["glowtheme"])
                    .pick_file()
                {
                    self.store.import_theme(&path)?;
                    self.stop_audio_smoothly();
                    self.install_native_theme(cx);
                    self.configure(cx);
                }
            }
            Action::ExportTheme => {
                self.hide(cx);
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("导出一个微光主题")
                    .set_file_name("我的微光.glowtheme")
                    .save_file()
                {
                    self.store.export_theme(self.theme(), &path)?;
                    self.notice = Some("已导出主题包，不包含书籍和日记。".into());
                }
            }
            Action::Asset(kind) => {
                self.hide(cx);
                let extensions = match kind.as_str() {
                    "background" => vec!["png", "jpg", "jpeg", "webp"],
                    "font" => vec!["ttf", "otf"],
                    _ => vec!["wav", "ogg", "mp3"],
                };
                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(window)
                    .set_title("选择主题素材")
                    .add_filter("主题素材", &extensions)
                    .pick_file()
                {
                    let mut t = self.read_fields(cx)?;
                    self.store.copy_theme_asset(&mut t, &kind, &path)?;
                    self.fill_fields(&t, window, cx);
                    self.draft = Some(t);
                }
            }
            Action::Audio => {
                if self.audio.is_some() {
                    self.stop_audio_smoothly();
                    self.notice = Some("已轻声关闭环境声景。".into());
                } else {
                    let label = self.start_audio(false)?;
                    let vol_pct = (self.theme().volume.clamp(0.05, 1.0) * 100.0).round() as u32;
                    self.schedule_motion_frame(window, cx);
                    self.notice = Some(format!("已开启环境声景：{label}（音量 {vol_pct}%）。"));
                }
            }
            Action::Soundscape(preset) => {
                self.store.state.soundscape_preset = Some(preset.id().into());
                self.store.state.soundscape_mix = SoundscapeMix::from_single(preset.id());
                self.save();
                let label = self.start_audio(true)?;
                let vol_pct = (self.theme().volume.clamp(0.05, 1.0) * 100.0).round() as u32;
                self.schedule_motion_frame(window, cx);
                self.notice = Some(format!("声景频道已调频至：{label}（音量 {vol_pct}%）。"));
            }
            Action::AudioVolume(delta) => {
                let steps = (self.store.state.theme.volume * 20.0).round() as i32 + delta as i32;
                let clamped = (steps as f32 / 20.0).clamp(0.05, 1.0);
                self.store.state.theme.volume = clamped;
                if let Some(preview) = &mut self.preview {
                    preview.volume = clamped;
                }
                if let Some((_, sink, _)) = &self.audio {
                    sink.set_volume(clamped);
                }
                self.save();
            }
            Action::ReducedMotion => {
                if let Some(t) = &mut self.draft {
                    t.reduced_motion = !t.reduced_motion;
                }
            }
            Action::ToggleFlow => {
                self.store.state.theme.reduced_motion = !self.store.state.theme.reduced_motion;
                if let Some(preview) = &mut self.preview {
                    preview.reduced_motion = self.store.state.theme.reduced_motion;
                }
                self.configure(cx);
                self.save();
                self.notice = Some(if self.store.state.theme.reduced_motion {
                    "已切换为静谧模式（关闭鼠标流光与动效）。".into()
                } else {
                    "已开启棱镜流光与动态交互。".into()
                });
            }
            Action::PetInteract(origin) => {
                let reduced = self.theme().reduced_motion;
                let is_prism = self.is_prism();
                let whisper = self.motion.trigger_pet(origin, reduced);
                self.motion.on_click(origin, reduced, is_prism);
                self.notice = Some(whisper.into());
                self.schedule_motion_frame(window, cx);
            }
            Action::RevealSpoiler(id) => {
                self.spoilers.insert(id);
            }
            Action::Privacy => {
                self.store.state.privacy_seen = true;
                self.save();
            }
            Action::Encoding(encoding) => {
                if let Some(id) = self.active_book.clone() {
                    let path = self.store.root.join("books").join(&id).join("original.txt");
                    let title = self
                        .store
                        .state
                        .books
                        .iter()
                        .find(|b| b.id == id)
                        .map(|b| b.title.clone());
                    let prepared = glow_core::library::prepare_book(&path, Some(&encoding))?;
                    self.store.commit_book_as(prepared, Some(&id))?;
                    if let Some(title) = title
                        && let Some(b) = self.store.state.books.iter_mut().find(|b| b.id == id)
                    {
                        b.title = title;
                    }
                    self.open(&id, window, cx)?;
                }
            }
            other => self.perform_feature(other, window, cx)?,
        }
        Ok(())
    }
    fn finish_ai_passage(
        &mut self,
        result: anyhow::Result<PassageGenerated>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Err(error) => self.error = Some(error.to_string()),
            Ok(mut generated) => {
                let same_passage = self.echo_input.trim() == generated.passage;
                // Keep any matched classical quotes alongside the fresh AI companion reflection
                if same_passage && let Some(existing) = &self.echo {
                    generated.result.quotes = existing.quotes.clone();
                }
                if same_passage {
                    self.echo = Some(generated.result.clone());
                }
                let location = generated
                    .book_id
                    .as_ref()
                    .filter(|id| {
                        glow_core::model::valid_cfi(&generated.cfi)
                            && self.store.state.books.iter().any(|b| &b.id == *id)
                    })
                    .map(|id| Location {
                        book_id: id.clone(),
                        cfi: generated.cfi.clone(),
                    });
                match self.store.record(
                    "selection",
                    generated.passage,
                    None,
                    generated.result,
                    location,
                ) {
                    Ok(()) => {
                        if same_passage {
                            self.echo_saved = true;
                        }
                        self.error = None;
                        self.notice = Some("AI 段落共读已送达，并为你收入私人手账。".into());
                    }
                    Err(error) => {
                        self.error = Some(format!("AI 共读已生成，但日记保存失败：{error}"));
                    }
                }
            }
        }
        cx.notify();
    }
    fn finish_mood(
        &mut self,
        result: anyhow::Result<mood::Generated>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Err(error) => self.error = Some(error.to_string()),
            Ok(generated) => {
                match self.store.record(
                    "mood",
                    generated.input.clone(),
                    Some(generated.mood.clone()),
                    generated.result.clone(),
                    None,
                ) {
                    Err(error) => {
                        self.error = Some(format!(
                            "回答已生成，但日记保存失败：{error}。请保留当前文字再次点击，重试保存不会重新生成。"
                        ));
                        self.mood_unsaved = Some(generated);
                    }
                    Ok(()) => {
                        if self.mood_input.read(cx).value().trim() == generated.input {
                            self.mood_input
                                .update(cx, |state, cx| state.set_value("", window, cx));
                        }
                        let input = if generated.input.is_empty() {
                            generated.mood
                        } else {
                            generated.input
                        };
                        self.mood_reply = Some((input, generated.result));
                        self.mood_unsaved = None;
                        self.error = None;
                        cx.on_next_frame(window, |_, _, cx| {
                            cx.notify();
                        });
                    }
                }
            }
        }
        cx.notify();
    }

    fn hide(&self, cx: &mut App) {
        if let Some(view) = &self.webview {
            view.update(cx, |v, _| v.hide());
        }
    }
    fn begin_import(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.is_empty() {
            self.busy = false;
            return;
        }
        self.busy = true;
        self.notice = Some("正在整理书页，请稍候…".into());
        let task = cx.background_executor().spawn(async move {
            let total = paths.iter().try_fold(0u64, |total, path| {
                fs::metadata(path).map(|m| total.saturating_add(m.len()))
            });
            if total.is_ok_and(|size| size > glow_core::library::MAX_ARCHIVE) {
                return vec![Err(anyhow::anyhow!(
                    "一次导入的文件总量不能超过 100 MB，请分批导入"
                ))];
            }
            paths
                .into_iter()
                .map(|path| glow_core::library::prepare_book(&path, None))
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let prepared = task.await;
            let _ = this.update(cx, |this, cx| {
                let mut errors = Vec::new();
                let mut count = 0;
                for book in prepared {
                    match book.and_then(|b| this.store.commit_book(b, false)) {
                        Ok(_) => count += 1,
                        Err(e) => errors.push(e.to_string()),
                    }
                }
                this.busy = false;
                this.notice = Some(format!("已整理 {count} 本书，随时可以开始。"));
                this.error = if errors.is_empty() {
                    None
                } else {
                    Some(errors.join("；"))
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn all_themes(&self) -> Vec<ReadingTheme> {
        let mut list = themes();
        list.extend(self.store.state.custom_themes.clone());
        list
    }
    fn fill_fields(&self, t: &ReadingTheme, window: &mut Window, cx: &mut App) {
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
    fn read_fields(&self, cx: &App) -> anyhow::Result<ReadingTheme> {
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
    fn is_dark(&self) -> bool {
        glow_core::theme::color(&self.theme().paper).unwrap_or(0) <= 0x606060
    }
    fn is_prism(&self) -> bool {
        self.theme().id == "prism"
    }
    #[cfg(all(test, feature = "ui-test"))]
    fn is_sky(&self) -> bool {
        self.theme().id == "forest"
    }
    fn is_ghibli(&self) -> bool {
        Self::is_ghibli_id(&self.theme().id)
    }
    fn is_ghibli_id(id: &str) -> bool {
        matches!(
            id,
            "forest" | "totoro" | "spirited" | "howl" | "kiki" | "lantern"
        )
    }
    fn scene_asset_for(id: &str) -> Option<&'static str> {
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
    fn theme_card_meta(id: &str) -> (&'static str, Option<&'static str>, &'static str) {
        match id {
            "forest" => (
                "LAPUTA SKY · 天空之城 · 88.6 FM",
                Some("✦ 天空之城"),
                "蓝天流云与浮岛树屋，清澈舒展",
            ),
            "totoro" => (
                "TOTORO WOODLAND · 龙猫森林 · 90.2 FM",
                Some("✦ 龙猫"),
                "古樟树荫与林间晴雨，温柔护眼",
            ),
            "spirited" => (
                "SPIRITED SEA TRAIN · 海上电车 · 94.8 FM",
                Some("✦ 千与千寻"),
                "浅海黄昏与温柔车厢，卸下疲惫",
            ),
            "howl" => (
                "HOWL'S MEADOW · 秘密花原 · 96.5 FM",
                Some("✦ 哈尔的移动城堡"),
                "微风花海与雪山湖畔，轻盈开阔",
            ),
            "kiki" => (
                "KIKI'S COAST · 海风烘焙镇 · 98.4 FM",
                Some("✦ 魔女宅急便"),
                "红瓦海港与暖阳窗台，惬意日常",
            ),
            "lantern" => (
                "SPIRITED LANTERN · 汤屋灯夜 · 103.6 FM",
                Some("✦ 千与千寻 · 夜灯"),
                "红桥暖灯与静谧暮川，治愈夜读",
            ),
            "paper" => ("QUIET PAPER · 92.4 FM", None, "温润古典素纸，专注文字本身"),
            "night" => ("NIGHT GARDEN · 101.7 FM", None, "低亮度青墨夜色，静心深读"),
            "prism" => (
                "OBSIDIAN PRISM · 108.0 FM",
                Some("✦ 鼠标流光"),
                "黑曜光谱与流光粒子交互",
            ),
            _ => ("PERSONAL THEME · 96.0 FM", None, "你亲手调制的私人阅读空间"),
        }
    }
    fn theme_cover_palette(&self) -> [u32; 4] {
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
    fn theme_hud_palette(&self) -> (u32, Rgba, Option<u32>) {
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
    fn theme_shelf_cue(&self) -> &'static str {
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
    fn schedule_motion_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.frame_scheduled || self.theme().reduced_motion || cfg!(test) || self.smoke {
            return;
        }
        self.frame_scheduled = true;
        cx.on_next_frame(window, |this, _, cx| {
            this.frame_scheduled = false;
            cx.notify();
        });
    }
    fn scene_asset(&self) -> &'static str {
        Self::scene_asset_for(&self.theme().id).unwrap_or("scene.png")
    }
    fn motion_snapshot(&self) -> MotionSnapshot {
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
    fn focus_timer_progress(&self) -> Option<(f32, u32)> {
        let started = self.focus_timer_started?;
        let elapsed = started.elapsed().as_secs_f32();
        let total = (self.focus_timer_total_secs as f32).max(1.0);
        let ratio = (elapsed / total).clamp(0.0, 1.0);
        let remaining_secs = (self.focus_timer_total_secs as f32 - elapsed)
            .max(0.0)
            .ceil() as u32;
        Some((ratio, remaining_secs))
    }
    fn overall_reading_progress(&self) -> f32 {
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
    fn daily_rediscovery(&self) -> Option<DailyRediscovery> {
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
    fn pet_dock(&self, id: &'static str, cx: &Context<Self>) -> impl IntoElement {
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
    fn border_subtle(&self) -> Hsla {
        if self.is_prism() {
            rgb(0xa5b4fc).opacity(0.14).into()
        } else if self.is_dark() {
            rgb(0xffffff).opacity(0.11).into()
        } else {
            rgb(hex(&self.theme().text)).opacity(0.10).into()
        }
    }
    fn border_strong(&self) -> Hsla {
        if self.is_prism() {
            rgb(0xc4b5fd).opacity(0.24).into()
        } else if self.is_dark() {
            rgb(0xffffff).opacity(0.18).into()
        } else {
            rgb(hex(&self.theme().text)).opacity(0.16).into()
        }
    }
    fn specular_line(&self) -> Div {
        div()
            .w_full()
            .h(px(1.))
            .bg(rgb(0xffffff).opacity(if self.is_dark() { 0.14 } else { 0.62 }))
    }
    fn frequency_for_theme(id: &str) -> &'static str {
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
    fn waveform_bars(&self, active: bool, heights: &[f32]) -> Div {
        self.waveform_bars_tinted(active, heights, None)
    }
    fn waveform_bars_tinted(&self, active: bool, heights: &[f32], tint: Option<u32>) -> Div {
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
    fn studio_badge(&self, code: &str, label: &str, active: bool) -> Div {
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
    fn button(&self, label: &str, action: Action, cx: &Context<Self>) -> Button {
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
    fn button_with_icon(
        &self,
        label: &str,
        icon: IconName,
        action: Action,
        cx: &Context<Self>,
    ) -> Button {
        self.button(label, action, cx).icon(icon)
    }
    fn pill_button(
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
    fn nav_button(
        &self,
        label: &str,
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
                let label = label.to_owned();
                move || label.clone()
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
    fn mood_button(&self, mood: &str, active: bool, cx: &Context<Self>) -> Div {
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
    fn heading(&self, kicker: &str, title: &str, sub: &str) -> Div {
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
    fn shelf(&self, cx: &Context<Self>) -> AnyElement {
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
                let status_ok = match self.shelf_filter {
                    ShelfFilter::All => true,
                    ShelfFilter::Finished => b.progress.is_some_and(|p| p >= 0.95),
                    ShelfFilter::Reading => {
                        b.location.is_some() && !b.progress.is_some_and(|p| p >= 0.95)
                    }
                    ShelfFilter::Unread => b.location.is_none(),
                };
                if !status_ok {
                    return false;
                }
                if query.is_empty() {
                    return true;
                }
                b.title.to_lowercase().contains(&query) || b.author.to_lowercase().contains(&query)
            })
            .collect();
        let mut books = h_flex().flex_wrap().gap_5().items_start();
        for (i, b) in filtered_books.iter().copied() {
            let colors = cover_colors;
            let reading_label = if let Some(progress) = b.progress {
                format!("已读 {:.0}%", progress * 100.0)
            } else if b.location.is_some() {
                "阅读中".to_owned()
            } else {
                "未开始".to_owned()
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
                                .hover(|style| style.border_color(rgb(hex(&t.accent)).opacity(0.65)))
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
                                .bg(rgb(hex(&t.background)).opacity(if is_dark { 0.58 } else { 0.68 }))
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
                                                .w(px((188.0 * progress_val)
                                                    .max(if b.location.is_some() { 6.0 } else { 0.0 })))
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
                );
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
                .child(self.button_with_icon("汤屋灯夜", IconName::Flame, Action::PickTheme(8), cx))
                .child(self.button_with_icon("切到静夜", IconName::Moon, Action::PickTheme(2), cx))
                .child(self.button_with_icon(
                    "黑曜棱镜",
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
            // Right-side MineRadio Floating Telemetry & Daily Rediscovery HUD
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
            ("未开始", ShelfFilter::Unread),
            ("已读完", ShelfFilter::Finished),
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
        let today_mins = if self.store.state.reading_minutes_date.as_deref() == Some(&today_ymd()) {
            self.store.state.reading_minutes_today
        } else {
            0
        };
        v_flex()
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
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.accent)))
                                    .child("YOUR QUIET SHELF · 私人藏书架"),
                            )
                            .child(
                                h_flex()
                                    .gap_2p5()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(22.))
                                            .font_family(self.display_font())
                                            .child("我的书架"),
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
                                    )
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
                            ),
                    )
                    .child(
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
                    ),
            )
            .child(books)
            .into_any_element()
    }
    fn reader_stage_backdrop(&self) -> Div {
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
    fn reader(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
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
                self.button_with_icon("AI 共读", IconName::WandSparkles, Action::ToggleCompanion, cx)
                    .toggled(self.drawer == "companion"),
            )
            .child(
                self.button_with_icon(
                    if self.tts_status.active && !self.tts_status.paused {
                        "听书中"
                    } else {
                        "听书"
                    },
                    IconName::AudioLines,
                    Action::ToggleTtsDrawer,
                    cx,
                )
                .toggled(self.drawer == "tts" || self.tts_status.active),
            )
            .child(
                self.button_with_icon("目录", IconName::ListTree, Action::Toc, cx)
                    .toggled(self.drawer == "toc"),
            )
            .child(
                self.button_with_icon("搜索", IconName::Search, Action::Search, cx)
                    .toggled(self.drawer == "search"),
            )
            .child(
                self.button_with_icon("划线", IconName::Feather, Action::Highlights, cx)
                    .toggled(self.drawer == "highlights"),
            )
            .child(
                self.button_with_icon("书签", IconName::Bookmark, Action::Bookmarks, cx)
                    .toggled(self.drawer == "bookmarks"),
            )
            .child(self.button_with_icon(
                if self.store.state.mode == "scrolled" {
                    "滚动"
                } else {
                    "翻页"
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
                        "退出专注"
                    } else {
                        "专注"
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
                "书架",
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
                    .child(self.button_with_icon("上一页", IconName::ChevronLeft, Action::Prev, cx))
                    .child(self.button_with_icon(
                        "下一页",
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
        let hint = if let Some(message) = self.error.as_ref().or(self.notice.as_ref()) {
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
                .child(
                    div()
                        .text_size(px(11.))
                        .font_family(self.ui_font())
                        .child("选中文字，遇见回响"),
                )
                .into_any_element()
        };
        let timer_label = if let Some((_, rem)) = self.focus_timer_progress() {
            format!("静读 {:02}:{:02}", rem / 60, rem % 60)
        } else {
            "静读计时".to_owned()
        };
        bottom = bottom
            .child(hint)
            .child(self.pet_dock("reader-pet", cx))
            .child(
                self.button_with_icon(
                    &timer_label,
                    IconName::Timer,
                    Action::ToggleTimerPicker,
                    cx,
                )
                .toggled(self.focus_timer_started.is_some() || self.focus_timer_picker_open),
            )
            .child(
                self.button_with_icon("排版", IconName::AArrowDown, Action::Typography, cx)
                    .toggled(self.drawer == "type"),
            )
            .child(self.button_with_icon("留下书签", IconName::Bookmark, Action::Bookmark, cx));
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
        stage = stage.child(body);
        if show_bottom {
            stage = stage.child(bottom);
        }
        stage.into_any_element()
    }
    #[inline(never)]
    fn reader_drawer(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let title = match self.drawer.as_str() {
            "companion" => "防剧透 AI 共读",
            "tts" => "微光神经听书",
            "toc" => "沿着目录走",
            "search" => "在书里找一句话",
            "highlights" => "划过的地方",
            "type" => "把这一页调舒服",
            "bookmarks" => "留过的记号",
            _ => "文字的回响",
        };
        let kicker = match self.drawer.as_str() {
            "companion" => "SPOILER-FREE AI · 前情提要与人物图谱",
            "tts" => "EDGE NEURAL AUDIOBOOK · 朗读伴听",
            "toc" => "IN THIS BOOK · 章节导航",
            "search" => "FIND THE WORDS · 全书检索",
            "highlights" => "LINES WORTH KEEPING · 划线",
            "type" => "READ YOUR WAY · 排版调音台",
            "bookmarks" => "LITTLE MARKS · 书签记号",
            _ => "A SMALL ECHO · 文字共鸣",
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
                    .child(self.button_with_icon("收起", IconName::X, Action::CloseEcho, cx)),
            );
        match self.drawer.as_str() {
            "companion" => {
                content = content.child(self.drawer_companion(cx));
            }
            "tts" => {
                content = content.child(self.drawer_tts(cx));
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
                                        "这本书没有提供可读取的目录，可以继续滚动阅读。"
                                    } else {
                                        "目录正在整理，书页就绪后会自动出现在这里。"
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
                                    "正在翻找…"
                                } else {
                                    "搜索全书"
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
                            .child("一页一页找过去，找到会停在这里。"),
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
                            .child("可以搜人物、地点，或只记得的半句话。"),
                    );
                } else {
                    content = content.child(
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
                                    .child(format!("找到 {} 处", self.search_results.len())),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("点击直接跳转到对应段落"),
                            ),
                    );
                    for hit in &self.search_results {
                        let chapter = if hit.chapter.is_empty() {
                            "书中位置".to_owned()
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
                                        .child("读到喜欢的地方，可以留一个记号。"),
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
    fn drawer_highlights(&self, cx: &Context<Self>) -> AnyElement {
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
    fn drawer_type(&self, cx: &Context<Self>) -> AnyElement {
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
                                                    .child("正文字号 · FONT SIZE"),
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
                                                    .child("行距呼吸感 · LINE HEIGHT"),
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
                                            .child(self.button("紧", Action::TypeLeading(-1), cx))
                                            .child(self.button("松", Action::TypeLeading(1), cx)),
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
                                                    .child("版心宽度 · MEASURE"),
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
                                            .child(self.button("窄", Action::TypeWidth(-1), cx))
                                            .child(self.button("宽", Action::TypeWidth(1), cx)),
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
                                    .child("护眼暖光滤镜"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("柔和滤去屏幕冷蓝光，久读不累"),
                            ),
                    )
                    .child(self.pill_button(
                        if self.store.state.warm_filter {
                            "暖光：开"
                        } else {
                            "暖光：关"
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
                    .child("调整会立即同步至当前书页，并对之后打开的书保留。"),
            )
            .child(self.button("恢复主题默认", Action::ResetTypography, cx))
            .into_any_element()
    }
    #[inline(never)]
    fn drawer_echo(&self, cx: &Context<Self>) -> AnyElement {
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
                .unwrap_or_else(|| "微光阅读".into());
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
            box_el = box_el
                .child(
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
                                            "已留下划线"
                                        } else {
                                            "留下划线"
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
                                            "已存入日记"
                                        } else {
                                            "存入日记"
                                        },
                                        IconName::NotebookPen,
                                        Action::SaveEchoToJournal,
                                        cx,
                                    )
                                    .disabled(self.echo_saved),
                                )
                                .child(self.button_with_icon(
                                    "书摘卡",
                                    IconName::ImageDown,
                                    Action::OpenQuoteCard(Box::new(card_draft)),
                                    cx,
                                ))
                                .child(self.button_with_icon(
                                    "所有划线",
                                    IconName::Bookmark,
                                    Action::Highlights,
                                    cx,
                                )),
                        ),
                )
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
                                            "正在细读这段文字…"
                                        } else {
                                            "问问微光（AI 深度共读）"
                                        },
                                        IconName::WandSparkles,
                                        Action::AskAiPassage,
                                        cx,
                                    )
                                    .primary()
                                    .disabled(self.echo_ai_rx.is_some()),
                                )
                                .when(self.echo_ai_rx.is_some(), |this| {
                                    this.child(self.button("停止", Action::CancelAiPassage, cx))
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(10.))
                                .font_family(self.ui_font())
                                .line_height(relative(1.5))
                                .text_color(rgb(hex(&t.muted)))
                                .child(
                                    "手动触发：仅发送本段选文与书名给 OpenAI，不上传整本书或日记。",
                                ),
                        ),
                )
                .child(self.echo_card(echo, &self.echo_input, true, cx));
        }
        box_el.into_any_element()
    }
    #[inline(never)]
    fn echo_card(
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
    fn reply_card(&self, reply: &str) -> Div {
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

    fn mood_page(&self, cx: &Context<Self>) -> AnyElement {
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
                                    .child("选一个最接近的情绪波段"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(format!("当前：{}", self.mood)),
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
                                    .child("如果愿意，再写下一点（选填）"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child(format!(
                                        "{} / 1000 字",
                                        self.mood_input.read(cx).value().trim().chars().count()
                                    )),
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
                        "正在读你的文字…"
                    } else if self.mood_unsaved.as_ref().is_some_and(|pending| {
                        pending.input == self.mood_input.read(cx).value().trim()
                            && pending.mood == self.mood
                    }) {
                        "重试保存回答"
                    } else {
                        "让文字陪我一会儿"
                    },
                    IconName::MessageCircleHeart,
                    Action::SubmitMood,
                    cx,
                )
                .primary()
                .disabled(self.mood_rx.is_some()),
            );
        if self.mood_rx.is_some() {
            left_console = left_console.child(self.button("停止等待", Action::CancelMood, cx));
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
                        .child("点击后将本次文字与所选心情发送给 OpenAI 生成回答，可能产生 API 费用。不会发送书库或历史日记；成功后保存在本机。"),
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
                        .child(self.studio_badge("4-7-8", "棱镜呼吸光环 · 进行中", true))
                        .child(self.button_with_icon(
                            "结束呼吸",
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
                                                .child(format!("还剩 {session_left} 秒")),
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
                                        .child("先不急着变好。"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_family(self.ui_font())
                                        .text_color(rgb(0xd8e4dd))
                                        .child("在这里坐一会儿，让一句话陪你呼吸。"),
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
                                        "陪我呼吸 1 分钟",
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
                                    .child("给你的回响 · LIVE RESONANCE"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("已自动存入本地私人日记"),
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
                                    .child("情绪波段待机中 · 等待你的声音"),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(self.ui_font())
                            .line_height(relative(1.65))
                            .text_color(rgb(hex(&t.muted)))
                            .child("在左侧选一种接近此刻的状态，或者写下几句想说的话。生成的微光陪伴会在这里展开，并为你留在本地日记里。"),
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
                "A PLACE TO REST · 情绪调频",
                "此刻，你感觉怎么样？",
                "不用把心情整理好再来。选一种接近的感觉就可以。",
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
    fn journal(&self, cx: &Context<Self>) -> AnyElement {
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

        let mut filter_bar = h_flex().gap_1p5().items_center().flex_wrap();
        for (label, filter) in [
            ("全部", JournalFilter::All),
            ("已收藏", JournalFilter::Favorite),
            ("心情", JournalFilter::Mood),
            ("阅读回响", JournalFilter::Selection),
        ] {
            filter_bar = filter_bar.child(self.pill_button(
                label,
                None,
                self.journal_filter == filter,
                Action::JournalFilter(filter),
                cx,
            ));
        }

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
                        "LITTLE THINGS KEPT · 微光手账档案",
                        "那些被文字接住的时刻",
                        "手账保存在这台电脑上，支持随时检索、导出 Markdown 或生成微光书摘卡。",
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
                                    .child(format!(
                                        "共 {} 篇  ·  收藏 {} 篇",
                                        self.store.state.journal.len(),
                                        fav_count
                                    )),
                            )
                            .child(self.button_with_icon(
                                "导出 Markdown",
                                IconName::Download,
                                Action::ExportJournalMarkdown,
                                cx,
                            ))
                            .child(self.button_with_icon(
                                "清空日记",
                                IconName::X,
                                Action::ClearJournal,
                                cx,
                            )),
                    ),
            );
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
                            .child("下一次回响，会替你记住那个时刻。"),
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
                                .child(entry.mood.as_deref().unwrap_or("阅读回响").to_owned()),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.muted)))
                                .child(display_time(&entry.created_at)),
                        ),
                )
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
                    .child(self.echo_card(
                        &entry.echo,
                        &display_input,
                        entry.kind == "selection",
                        cx,
                    )),
            );
        }
        content.into_any_element()
    }
    fn theme_card(
        &self,
        i: usize,
        t: &ReadingTheme,
        active_theme: &ReadingTheme,
        is_dark: bool,
        cx: &Context<Self>,
    ) -> Div {
        let active = active_theme.id == t.id;
        let (subtitle, poster_badge, mood_note) = Self::theme_card_meta(&t.id);
        let is_ghibli_card = Self::is_ghibli_id(&t.id);
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
        } else if is_ghibli_card {
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
                    prism::prism_beam_horizontal(
                        snap.clone(),
                        2.0,
                        i as f32 * 0.9,
                        is_prism_card,
                    ),
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
            .child(if active {
                self.button_with_icon("正在使用", IconName::Check, Action::PickTheme(i), cx)
                    .primary()
                    .w_full()
            } else {
                self.button_with_icon("使用此主题", IconName::Sparkles, Action::PickTheme(i), cx)
                    .outline()
                    .w_full()
            });
        if active {
            card.border_2()
                .border_color(rgb(hex(&t.accent)))
                .shadow_md()
        } else {
            card
        }
    }
    fn theme_page(&self, cx: &Context<Self>) -> AnyElement {
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
                        "MAKE YOURSELF AT HOME · 空间与声景调音台",
                        "把这里，变成喜欢的样子。",
                        "从宫崎骏水彩海报般的温柔光影，到素纸静夜与流光棱镜；主题包不包含你的书和日记。",
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(self.button_with_icon(
                                "导入主题包",
                                IconName::Plus,
                                Action::ImportTheme,
                                cx,
                            ))
                            .child(self.button_with_icon(
                                "导出当前主题",
                                IconName::ArrowUpRight,
                                Action::ExportTheme,
                                cx,
                            )),
                    ),
            );
        let mut ghibli_row = h_flex().gap_4().flex_wrap();
        let mut classic_row = h_flex().gap_4().flex_wrap();
        let mut custom_row = h_flex().gap_4().flex_wrap();
        let mut custom_count = 0usize;
        for (i, t) in all.iter().enumerate() {
            let card = self.theme_card(i, t, active_theme, is_dark, cx);
            if Self::is_ghibli_id(&t.id) {
                ghibli_row = ghibli_row.child(card);
            } else if matches!(t.id.as_str(), "paper" | "night" | "prism") {
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
                                            .child("GHIBLI WATERCOLOR GALLERY · 宫崎骏手绘海报"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(18.5))
                                            .font_family(self.display_font())
                                            .text_color(rgb(hex(&active_theme.text)))
                                            .child("温柔治愈画廊"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&active_theme.muted)))
                                    .child("龙猫、千与千寻、天空之城、哈尔花原与魔女海镇 · 均满足高对比度护眼阅读"),
                            ),
                    )
                    .child(ghibli_row),
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
                                    .child("CLASSIC & AMBIENT · 经典与夜读"),
                            )
                            .child(
                                div()
                                    .text_size(px(17.5))
                                    .font_family(self.display_font())
                                    .text_color(rgb(hex(&active_theme.text)))
                                    .child("经典与光影空间"),
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
                                    .child("PERSONAL THEMES · 自定义"),
                            )
                            .child(
                                div()
                                    .text_size(px(17.5))
                                    .font_family(self.display_font())
                                    .text_color(rgb(hex(&active_theme.text)))
                                    .child("我的私藏主题"),
                            ),
                    )
                    .child(custom_row),
            );
        }
        view = view.child(
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
                                "自定义 / 另存主题",
                                IconName::Feather,
                                Action::EditTheme,
                                cx,
                            )
                            .primary(),
                        )
                        .child(
                            self.button_with_icon(
                                if active_theme.reduced_motion {
                                    "流光交互：已静谧"
                                } else {
                                    "流光交互：已开启"
                                },
                                IconName::Sparkles,
                                Action::ToggleFlow,
                                cx,
                            )
                            .outline(),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&active_theme.muted)))
                                .child("支持鼠标流光、棱镜边缘折射与自定义素材"),
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
                                .child(format!(
                                    "当前正文对比度 {:.1}:1 · 最低要求 4.5:1",
                                    glow_core::theme::contrast(
                                        &active_theme.text,
                                        &active_theme.paper
                                    )
                                )),
                        ),
                ),
        )
        .child(self.settings_ai_card(cx))
        .child(self.settings_sync_card(cx));
        if let Some(draft) = &self.draft {
            let labels = [
                "主题名称",
                "界面底色 #RRGGBB",
                "纸面颜色 #RRGGBB",
                "正文颜色 #RRGGBB",
                "次要文字 #RRGGBB",
                "强调色 #RRGGBB",
                "正文字体",
                "字号 14—36",
                "行距 1.3—2.6",
                "正文宽度 400—1100",
                "背景遮罩 0—1",
                "环境音量 0—1",
            ];
            let mut fields = h_flex().flex_wrap().gap_4();
            for (i, label) in labels.iter().enumerate() {
                fields = fields.child(
                    v_flex()
                        .w(px(212.))
                        .gap_1p5()
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&active_theme.muted)))
                                .child((*label).to_owned()),
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
                        v_flex()
                            .gap_1p5()
                            .child(self.studio_badge("DIY CONSOLE", "MAKE IT YOURS", true))
                            .child(
                                div()
                                    .text_size(px(21.))
                                    .font_family(self.display_font())
                                    .child("编辑这一个阅读空间"),
                            ),
                    )
                    .child(fields)
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(self.button_with_icon(
                                "选择背景图片",
                                IconName::Image,
                                Action::Asset("background".into()),
                                cx,
                            ))
                            .child(self.button_with_icon(
                                "选择字体文件",
                                IconName::AArrowDown,
                                Action::Asset("font".into()),
                                cx,
                            ))
                            .child(self.button_with_icon(
                                "选择环境音",
                                IconName::AudioLines,
                                Action::Asset("audio".into()),
                                cx,
                            ))
                            .child(self.button_with_icon(
                                if draft.reduced_motion {
                                    "减少动态：开"
                                } else {
                                    "减少动态：关"
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
                            .child(format!(
                                "背景：{}  /  字体：{}  /  声音：{}",
                                draft.image.as_deref().unwrap_or("内置"),
                                draft.font_asset.as_deref().unwrap_or("系统"),
                                draft.audio.as_deref().unwrap_or("未选择")
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(self.button_with_icon(
                                "预览",
                                IconName::Eye,
                                Action::PreviewTheme,
                                cx,
                            ))
                            .child(
                                self.button_with_icon(
                                    "保存主题",
                                    IconName::Check,
                                    Action::SaveTheme,
                                    cx,
                                )
                                .primary(),
                            )
                            .child(self.button("恢复默认", Action::ResetTheme, cx))
                            .child(self.button("取消", Action::CancelTheme, cx)),
                    ),
            );
        }
        view.into_any_element()
    }
    fn smoke_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                self.act(Action::PickTheme(3), window, cx);
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
    #[inline(never)]
    fn render_sidebar(&self, t: &ReadingTheme, snap: MotionSnapshot, cx: &Context<Self>) -> Div {
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
                                            .child("微光"),
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
                                        "OBSIDIAN PRISM STATION"
                                    } else if self.is_ghibli() {
                                        "GHIBLI READING STATION"
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
                        .child("在这里"),
                )
                .child(
                    div()
                        .text_size(px(8.))
                        .font_family(self.ui_font())
                        .text_color(rgb(hex(&t.muted)).opacity(0.55))
                        .child("CHANNELS"),
                ),
        );
        for (label, page, icon) in [
            ("我的书架", Page::Shelf, IconName::LibraryBig),
            ("此刻心情", Page::Mood, IconName::MessageCircleHeart),
            ("私人日记", Page::Journal, IconName::NotebookPen),
            ("主题与设置", Page::Themes, IconName::Settings2),
        ] {
            let active = self.page == page;
            sidebar = sidebar.child(self.nav_button(label, icon, active, Action::Page(page), cx));
        }
        if self.active_book.is_some() {
            sidebar = sidebar.child(self.nav_button(
                "回到阅读",
                IconName::BookOpen,
                self.page == Page::Reader,
                Action::Page(Page::Reader),
                cx,
            ));
        }
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
                                        .child("快捷指令"),
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
        sidebar.child(div().flex_1()).child(deck).child(
            div()
                .px_3()
                .pt_1p5()
                .text_size(px(9.))
                .font_family(self.ui_font())
                .line_height(relative(1.5))
                .text_color(rgb(hex(&t.muted)).opacity(0.84))
                .child("一盏灯，一本书 · 慢慢读"),
        )
    }
    #[inline(never)]
    fn render_soundscape_deck(
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
        let today_mins = if self.store.state.reading_minutes_date.as_deref()
            == Some(today_ymd().as_str())
        {
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
                            .bg(rgb(hex(&t.accent)).opacity(if self.mixer_open { 0.28 } else { 0.14 }))
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
                    .child(self.waveform_bars(
                        self.audio.is_some(),
                        &[4., 9., 13., 7., 11., 6., 9.],
                    )),
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
                                    .child("快速切换主题"),
                            )
                            .child(
                                div()
                                    .text_size(px(8.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)).opacity(0.76))
                                    .child(format!("{} 套", themes().len())),
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
    #[inline(never)]
    fn render_welcome(&self, t: &ReadingTheme, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .size_full()
            .p_8()
            .items_center()
            .justify_center()
            .bg(rgb(hex(&t.background)).opacity(0.45))
            .child(
                v_flex()
                    .w(px(620.))
                    .max_w_full()
                    .p_8()
                    .gap_6()
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
                            .child(
                                img("glow-icon.png")
                                    .size(px(56.))
                                    .object_fit(ObjectFit::Contain)
                                    .rounded_2xl(),
                            )
                            .child(self.studio_badge("LOCAL-FIRST", "PRIVATE STORAGE", true)),
                    )
                    .child(self.heading(
                        "WELCOME TO GLOW STATION",
                        "欢迎来到微光。",
                        "书库、划线和日记保存在这台电脑上；心情回复可连接 OpenAI。",
                    ))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_family(self.ui_font())
                            .line_height(relative(1.75))
                            .text_color(rgb(hex(&t.muted)))
                            .child("书库、多色划线、批注、原生声景与书摘卡均 100% 在本机运行。仅当你主动提交心情或点击「问问微光（AI 深度共读）」时，才将当次单段文字发给 OpenAI 生成回复。"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .flex_wrap()
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("选择初始空间："),
                            )
                            .child(self.pill_button(
                                "林间暖光 · 天空之城",
                                Some(IconName::Cloud),
                                t.id == "forest",
                                Action::PickTheme(0),
                                cx,
                            ))
                            .child(self.pill_button(
                                "龙猫林荫 · 森林",
                                Some(IconName::Leaf),
                                t.id == "totoro",
                                Action::PickTheme(4),
                                cx,
                            ))
                            .child(self.pill_button(
                                "千寻海轨 · 海上电车",
                                Some(IconName::Sparkles),
                                t.id == "spirited",
                                Action::PickTheme(5),
                                cx,
                            ))
                            .child(self.pill_button(
                                "汤屋灯夜 · 暖灯",
                                Some(IconName::Flame),
                                t.id == "lantern",
                                Action::PickTheme(8),
                                cx,
                            ))
                            .child(self.pill_button(
                                "黑曜棱镜 · 暗夜流光",
                                Some(IconName::Sparkles),
                                t.id == "prism",
                                Action::PickTheme(3),
                                cx,
                            )),
                    )
                    .child(
                        self.button_with_icon(
                            "知道了，先坐一会儿",
                            IconName::BookOpen,
                            Action::Privacy,
                            cx,
                        )
                        .primary(),
                    ),
            )
            .into_any_element()
    }
    #[inline(never)]
    fn render_confirm(&self, t: &ReadingTheme, confirm: &str, cx: &Context<Self>) -> AnyElement {
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
                    .child(self.studio_badge("CONFIRM", "请再确认一次", true))
                    .child(
                        div()
                            .text_size(px(26.))
                            .font_family(self.display_font())
                            .child(if confirm == "journal" {
                                "清空全部私人日记？"
                            } else {
                                "把这本书移出书架？"
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_family(self.ui_font())
                            .line_height(relative(1.7))
                            .text_color(rgb(hex(&t.muted)))
                            .child(if confirm == "journal" {
                                "清空后无法在应用内恢复。书籍和阅读进度不会删除。"
                            } else {
                                "用户原文件不会删除，已经留下的日记仍会保留。"
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .child(self.button("保留", Action::Cancel, cx))
                            .child(
                                self.button(
                                    "确认",
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
    fn quote_card_modal(&self, draft: &QuoteCardDraft, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let (bg_hex, card_hex, text_hex, muted_hex, accent_hex, tag_text) = match draft.style {
            QuoteCardStyle::Sky => (
                0x8ecbf6u32,
                0xf9fdffu32,
                0x1a324cu32,
                0x4f6e8eu32,
                0x2573b8u32,
                "GLOW · 天空云境书摘",
            ),
            QuoteCardStyle::Totoro => (
                0xb7d9b8u32,
                0xf9fcf6u32,
                0x1f3626u32,
                0x4e6b56u32,
                0x3d7a52u32,
                "GLOW · 龙猫林荫书摘",
            ),
            QuoteCardStyle::Spirited => (
                0xeec4b8u32,
                0xfff9f5u32,
                0x3b2427u32,
                0x745458u32,
                0xb84b42u32,
                "GLOW · 海上电车书摘",
            ),
            QuoteCardStyle::Howl => (
                0xc8c2efu32,
                0xfcfbf7u32,
                0x2b2740u32,
                0x5f5a7au32,
                0x63549eu32,
                "GLOW · 秘密花原书摘",
            ),
            QuoteCardStyle::Kiki => (
                0xead4b7u32,
                0xfffdf7u32,
                0x3b2a1eu32,
                0x755d4au32,
                0xae572au32,
                "GLOW · 魔女海镇书摘",
            ),
            QuoteCardStyle::Lantern => (
                0x17141fu32,
                0x201b26u32,
                0xf5e9d6u32,
                0xbca895u32,
                0xe69f52u32,
                "GLOW · 汤屋灯夜书摘",
            ),
            QuoteCardStyle::Prism => (
                0x090c16u32,
                0x121624u32,
                0xeef1fau32,
                0x94a0beu32,
                0xa78bfau32,
                "GLOW · 黑曜棱镜书摘",
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
    fn filtered_palette_commands(
        &self,
        cx: &App,
    ) -> Vec<(&'static str, &'static str, IconName, Action)> {
        let query = self.palette_input.read(cx).value().trim().to_lowercase();
        let mut commands: Vec<(&'static str, &'static str, IconName, Action)> = vec![
            (
                "主题 · 林间暖光（天空之城蓝白）",
                "宫崎骏天空之城蓝天白云手绘主题",
                IconName::Cloud,
                Action::PickTheme(0),
            ),
            (
                "主题 · 龙猫林荫（龙猫森林）",
                "宫崎骏龙猫古樟树荫与林间晴雨 · 护眼治愈绿",
                IconName::Leaf,
                Action::PickTheme(4),
            ),
            (
                "主题 · 千寻海轨（千与千寻海上电车）",
                "宫崎骏千与千寻浅海黄昏电车 · 温柔暖杏玫红",
                IconName::Sparkles,
                Action::PickTheme(5),
            ),
            (
                "主题 · 哈尔花原（哈尔的移动城堡）",
                "宫崎骏哈尔秘密花海与雪山微风 · 柔和丁香紫",
                IconName::Wind,
                Action::PickTheme(6),
            ),
            (
                "主题 · 魔女海镇（魔女宅急便）",
                "宫崎骏魔女宅急便红瓦海港与暖阳窗台 · 温柔陶土金",
                IconName::Sun,
                Action::PickTheme(7),
            ),
            (
                "主题 · 汤屋灯夜（千与千寻夜灯）",
                "宫崎骏千与千寻红桥暖灯与暮川萤火 · 深色治愈夜读",
                IconName::Flame,
                Action::PickTheme(8),
            ),
            (
                "主题 · 素纸",
                "温润护眼古典纸页",
                IconName::Sun,
                Action::PickTheme(1),
            ),
            (
                "主题 · 静夜",
                "低亮度柔和深色夜读",
                IconName::Moon,
                Action::PickTheme(2),
            ),
            (
                "主题 · 黑曜棱镜",
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
                "切换 MineRadio 原生合成自然声景",
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
                "久石让式温柔电钢五声琶音",
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
                "听书 · 打开 Edge 神经语音听书台",
                "晓晓/云希等多音色朗读，自动压低背景声景",
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
        ];
        if self.active_book.is_some() {
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
    fn run_palette_command(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = false;
        self.palette_selected = 0;
        window.focus(&self.focus, cx);
        if matches!(action, Action::ToggleBreathing) && self.page != Page::Mood {
            self.act(Action::Page(Page::Mood), window, cx);
        }
        self.act(action, window, cx);
    }
    fn execute_selected_palette_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let commands = self.filtered_palette_commands(cx);
        if commands.is_empty() {
            return;
        }
        let idx = self.palette_selected.min(commands.len() - 1);
        let action = commands[idx].3.clone();
        self.run_palette_command(action, window, cx);
    }
    #[inline(never)]
    fn focus_timer_modal(&self, snap: MotionSnapshot, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let dark = self.is_dark();
        let focus_status = self.focus_timer_progress();
        let vial_ratio = self.overall_reading_progress();
        let custom_raw = self.focus_timer_input.read(cx).value().trim().to_owned();
        let custom_mins = custom_raw
            .parse::<u32>()
            .ok()
            .filter(|m| (1..=180).contains(m));
        let today_mins = if self.store.state.reading_minutes_date.as_deref()
            == Some(today_ymd().as_str())
        {
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
    fn command_palette_modal(&self, snap: MotionSnapshot, cx: &Context<Self>) -> AnyElement {
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
}

fn hex(s: &str) -> u32 {
    glow_core::theme::color(s).unwrap_or(0x353e35)
}

fn highlight_color_hex(color: Option<&str>, fallback: u32) -> u32 {
    match color {
        Some("amber") => 0xb45309,
        Some("sage") => 0x047857,
        Some("rose") => 0xbe123c,
        Some("prism") => 0x7e22ce,
        _ => fallback,
    }
}

fn highlight_color_hex_for(color: Option<&str>, fallback: u32, is_dark: bool) -> u32 {
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

fn highlight_color_name(color: Option<&str>) -> &'static str {
    match color {
        Some("amber") => "琥珀暖黄",
        Some("sage") => "苔藓微绿",
        Some("rose") => "樱雾淡粉",
        Some("prism") => "棱镜暮紫",
        _ => "原文摘录",
    }
}

fn mood_spectrum_color(mood: Option<&str>) -> u32 {
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

fn mood_badge_text_color(mood: Option<&str>, is_dark: bool) -> u32 {
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

fn circadian_greeting() -> (&'static str, &'static str, &'static str, bool) {
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

fn today_ymd() -> String {
    let dt = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!("{:04}-{:02}-{:02}", dt.year(), dt.month() as u8, dt.day())
}

fn format_journal_markdown(entries: &[JournalEntry]) -> String {
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

fn display_time(raw: &str) -> String {
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

impl Render for ReadingApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme().clone();
        let dark = self.is_dark();
        let is_prism = self.is_prism();
        let continuous_flow = !t.reduced_motion
            && (is_prism
                || self.audio.is_some()
                || self.tts_status.active
                || self.breathing_started.is_some()
                || self.focus_timer_started.is_some());
        let needs_frame = self.motion.step(t.reduced_motion, continuous_flow);
        if needs_frame {
            self.schedule_motion_frame(window, cx);
        }
        let snap = self.motion_snapshot();
        let narrow = self.is_narrow_reader(window);
        let visible = self.page == Page::Reader
            && !self.busy
            && self.confirm.is_none()
            && self.quote_card.is_none()
            && !self.focus_timer_picker_open
            && !self.mixer_open
            && !self.palette_open
            && self.store.state.privacy_seen
            && (self.drawer.is_empty() || !narrow);
        if let Some(view) = &self.webview {
            if visible {
                view.update(cx, |v, _| v.show());
            } else {
                view.update(cx, |v, _| v.hide());
            }
        }
        let page = if !self.store.state.privacy_seen {
            self.render_welcome(&t, cx)
        } else if let Some(confirm) = &self.confirm {
            self.render_confirm(&t, confirm, cx)
        } else {
            match self.page {
                Page::Shelf => self.shelf(cx),
                Page::Reader => self.reader(window, cx),
                Page::Mood => self.mood_page(cx),
                Page::Journal => self.journal(cx),
                Page::Themes => self.theme_page(cx),
            }
        };
        let mut main = v_flex().flex_1().h_full().min_w_0();
        if self.page != Page::Reader
            && let Some(message) = self.error.as_ref().or(self.notice.as_ref())
        {
            main = main.child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .px_5()
                    .py_2p5()
                    .bg(rgb(hex(&t.paper)).opacity(0.96))
                    .border_b_1()
                    .border_color(self.border_subtle())
                    .text_color(rgb(if self.error.is_some() {
                        0xb34f36
                    } else {
                        hex(&t.accent)
                    }))
                    .child(
                        Icon::new(if self.error.is_some() {
                            IconName::CircleAlert
                        } else {
                            IconName::Sparkles
                        })
                        .size(px(15.)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_family(self.ui_font())
                            .child(message.clone()),
                    ),
            );
        }
        main = main.child(div().flex_1().min_h_0().child(page));
        let mut root = h_flex()
            .relative()
            .id("glow-app")
            .key_context("GlowReading")
            .track_focus(&self.focus)
            .size_full()
            .bg(rgb(hex(&t.background)))
            .text_color(rgb(hex(&t.text)))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                let reduced = this.theme().reduced_motion;
                this.motion.on_mouse_move(event.position, reduced);
                if this.focus_mode && this.page == Page::Reader {
                    let y = f32::from(event.position.y);
                    let h = f32::from(window.viewport_size().height).max(1.0);
                    // Hysteresis covers the full 62px top toolbar and 54px bottom toolbar once revealed
                    let next_top = if this.zen_reveal_top {
                        y <= 78.0
                    } else {
                        y <= 44.0
                    };
                    let next_bottom = if this.zen_reveal_bottom {
                        y >= h - 70.0
                    } else {
                        y >= h - 44.0
                    };
                    if (next_top && !this.zen_reveal_top)
                        || (next_bottom && !this.zen_reveal_bottom)
                    {
                        this.zen_revealed_at = Some(Instant::now());
                    }
                    this.zen_reveal_top = next_top;
                    this.zen_reveal_bottom = next_bottom;
                }
                if !reduced {
                    this.schedule_motion_frame(window, cx);
                }
                cx.notify();
            }))
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                let reduced = this.theme().reduced_motion;
                let is_prism = this.is_prism();
                this.motion.on_click(event.position, reduced, is_prism);
                if !reduced {
                    this.schedule_motion_frame(window, cx);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Escape, window, cx| {
                this.handle_escape(window, cx);
            }))
            .on_action(cx.listener(|this, _: &InputEscape, window, cx| {
                this.handle_escape(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Palette, window, cx| {
                this.act(Action::TogglePalette, window, cx);
            }))
            .on_action(
                cx.listener(|this, _: &Import, window, cx| this.act(Action::Import, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &Previous, window, cx| this.act(Action::Prev, window, cx)),
            )
            .on_action(cx.listener(|this, _: &Next, window, cx| this.act(Action::Next, window, cx)))
            .on_action(
                cx.listener(|this, _: &Search, window, cx| this.act(Action::Search, window, cx)),
            )
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                if !this.busy {
                    this.begin_import(paths.paths().to_vec(), cx);
                }
            }));
        if let Some(image) = &t.image {
            root = root
                .child(
                    img(self.store.root.join("themes").join(&t.id).join(image))
                        .absolute()
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                )
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .bg(rgb(hex(&t.background)).opacity(t.overlay)),
                );
        }
        // Preload builtin scenic textures so theme switching has zero first-frame blanking
        for preload in [
            "scene.png",
            "scene-totoro.png",
            "scene-spirited.png",
            "scene-howl.png",
            "scene-kiki.png",
            "scene-lantern.png",
            "scene-night.png",
            "scene-prism.png",
        ] {
            root = root.child(
                img(preload)
                    .absolute()
                    .size(px(1.))
                    .opacity(0.001),
            );
        }
        if let Some(scene) = Self::scene_asset_for(&t.id) {
            root = root.child(
                img(scene)
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .opacity(if self.page == Page::Reader {
                        0.03
                    } else if is_prism {
                        0.16
                    } else if dark {
                        0.11
                    } else {
                        0.075
                    }),
            );
        }
        // Ambient GPU optical stage: parallax aurora, dot-matrix, and mouse-following chromatic spotlight
        root = root.child(prism::ambient_stage_canvas(
            snap.clone(),
            self.page == Page::Reader,
        ));
        if !self.focus_mode || self.page != Page::Reader {
            root = root.child(self.render_sidebar(&t, snap.clone(), cx).relative());
        }
        root = root.child(main.relative());
        if let Some(draft) = &self.quote_card {
            root = root.child(self.quote_card_modal(draft, cx));
        }
        if self.focus_timer_picker_open {
            root = root.child(self.focus_timer_modal(snap.clone(), cx));
        }
        if self.mixer_open {
            root = root.child(self.soundscape_mixer_modal(snap.clone(), cx));
        }
        if self.palette_open {
            root = root.child(self.command_palette_modal(snap.clone(), cx));
        }
        root.child(prism::cursor_fx_overlay_canvas(snap))
    }
}
impl Drop for ReadingApp {
    fn drop(&mut self) {
        if let Some(player) = self.tts_player.take() {
            player.stop();
        }
        let _ = self.store.save();
        if !cfg!(test)
            && self.store.state.sync.auto_sync_on_exit
            && self.store.state.sync.mode != "off"
        {
            let token = self.store.load_secrets().github_token;
            if let Ok(outcome) = sync_client::run_sync(
                &self.store.root,
                &self.store.state,
                &self.store.state.sync,
                &token,
            ) {
                self.store.state = outcome.merged_state;
                let _ = self.store.save();
            }
        }
    }
}

pub fn bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("escape", Escape, Some("GlowReading")),
        KeyBinding::new("ctrl-k", Palette, Some("GlowReading")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-k", Palette, Some("GlowReading")),
        KeyBinding::new("ctrl-o", Import, Some("GlowReading")),
        KeyBinding::new("ctrl-f", Search, Some("GlowReading")),
        KeyBinding::new("pageup", Previous, Some("GlowReading")),
        KeyBinding::new("pagedown", Next, Some("GlowReading")),
    ]
}

#[cfg(all(test, feature = "ui-test"))]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn test_app(store: Store, window: &mut Window, cx: &mut Context<ReadingApp>) -> ReadingApp {
        let mut app = ReadingApp::new(store, window, cx);
        app.custom_mood_worker_override = true;
        app.mood_worker = |text, _| {
            Ok(EchoResult {
                quotes: vec![],
                reply: format!("测试回答：{text}"),
                demo: false,
            })
        };
        app
    }

    fn finish_test_request(
        app: &mut ReadingApp,
        window: &mut Window,
        cx: &mut Context<ReadingApp>,
    ) {
        let result = app
            .mood_rx
            .take()
            .unwrap()
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        app.finish_mood(result, window, cx);
    }
    #[gpui::test]
    fn mood_reply_is_separate_from_reader_echo_and_survives_navigation(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().into()).unwrap();
        store.state.privacy_seen = true;
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, window, cx| {
            app.echo = Some(echo::respond("阅读选文", Some("希望")));
            app.echo_input = "阅读选文".into();
            app.act(Action::Page(Page::Mood), window, cx);
            assert!(app.mood_reply.is_none());
            app.mood_input
                .update(cx, |state, cx| state.set_value("  今天累了  ", window, cx));
            app.act(Action::SubmitMood, window, cx);
            finish_test_request(app, window, cx);
            assert_eq!(app.mood_reply.as_ref().unwrap().0, "今天累了");
            assert_eq!(app.echo_input, "阅读选文");
            assert_eq!(app.store.state.journal[0].input, "今天累了");
            app.act(Action::Page(Page::Journal), window, cx);
            app.act(Action::Page(Page::Mood), window, cx);
            assert_eq!(app.mood_reply.as_ref().unwrap().0, "今天累了");
        });
        assert_eq!(
            Store::open(dir.path().into()).unwrap().state.journal[0].input,
            "今天累了"
        );
    }

    #[gpui::test]
    fn failed_mood_save_keeps_draft_and_does_not_show_success(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().into()).unwrap();
        fs::create_dir(dir.path().join("state.json")).unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, window, cx| {
            app.mood_input.update(cx, |state, cx| {
                state.set_value("需要留下的文字", window, cx)
            });
            app.act(Action::SubmitMood, window, cx);
            finish_test_request(app, window, cx);
            assert!(app.error.is_some());
            assert!(app.mood_reply.is_none());
            assert!(app.store.state.journal.is_empty());
            assert_eq!(app.mood_input.read(cx).value().as_str(), "需要留下的文字");
            assert!(app.mood_unsaved.is_some());
            fs::remove_dir(app.store.root.join("state.json")).unwrap();
            app.mood_worker = |_, _| panic!("saving again must not call the model");
            app.act(Action::SubmitMood, window, cx);
            assert!(app.error.is_none());
            assert!(app.mood_rx.is_none());
            assert_eq!(app.store.state.journal.len(), 1);
            assert!(app.mood_input.read(cx).value().is_empty());
        });
    }

    #[gpui::test]
    fn pending_mood_preserves_new_drafts_and_cancel_discards_late_result(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().into()).unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, window, cx| {
            let (sender, receiver) = mpsc::channel();
            app.mood_rx = Some(receiver);
            app.mood_worker = |_, _| panic!("duplicate submit must not call the model");
            app.mood_input
                .update(cx, |state, cx| state.set_value("新写的草稿", window, cx));
            app.act(Action::SubmitMood, window, cx);
            sender
                .send(Ok(mood::Generated {
                    input: "已提交的旧文字".into(),
                    mood: "希望".into(),
                    result: EchoResult {
                        quotes: vec![],
                        reply: "对应旧文字的回答".into(),
                        demo: false,
                    },
                }))
                .unwrap();
            finish_test_request(app, window, cx);
            assert_eq!(app.mood_input.read(cx).value().as_str(), "新写的草稿");
            assert_eq!(app.store.state.journal[0].input, "已提交的旧文字");
            let (sender, receiver) = mpsc::channel();
            app.mood_rx = Some(receiver);
            app.act(Action::CancelMood, window, cx);
            assert!(sender.send(Err(anyhow::anyhow!("late response"))).is_err());
            assert_eq!(app.store.state.journal.len(), 1);
        });
    }
    #[gpui::test]
    fn navigation_mood_and_journal_use_real_controls(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().into()).unwrap();
        store.state.privacy_seen = true;
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx
            .debug_bounds("此刻心情")
            .expect("navigation button laid out");
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| assert_eq!(app.page, Page::Mood));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx
            .debug_bounds("让文字陪我一会儿")
            .expect("mood button laid out");
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.update_in(cx, finish_test_request);
        app.read_with(cx, |app, _| assert_eq!(app.store.state.journal.len(), 1));
        let bounds = cx.debug_bounds("私人日记").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| assert_eq!(app.page, Page::Journal));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx.debug_bounds("清空日记").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| {
            assert_eq!(app.confirm.as_deref(), Some("journal"))
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx.debug_bounds("保留").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| assert_eq!(app.store.state.journal.len(), 1));
    }
    #[gpui::test]
    fn escape_closes_drawer_and_theme_preview_is_reversible(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.bind_keys(bindings());
        });
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().into()).unwrap();
        store.state.privacy_seen = true;
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, window, cx| {
            app.drawer = "echo".into();
            window.focus(&app.focus, cx);
            cx.notify();
        });
        cx.simulate_keystrokes("escape");
        app.read_with(cx, |app, _| assert!(app.drawer.is_empty()));
        app.update_in(cx, |app, window, cx| {
            app.act(Action::Page(Page::Themes), window, cx);
            app.act(Action::EditTheme, window, cx);
            app.fields[7].update(cx, |s, cx| s.set_value("26", window, cx));
            app.act(Action::PreviewTheme, window, cx);
            assert_eq!(app.theme().font_size, 26.);
            assert_eq!(app.store.state.theme.font_size, 20.);
            app.act(Action::CancelTheme, window, cx);
            assert_eq!(app.theme().font_size, 20.);
        });
    }

    #[gpui::test]
    fn bookmark_and_journal_controls_update_persistent_state(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("data")).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let book_id = store.state.books[0].id.clone();
        store.state.books[0].location = Some(Location {
            book_id: book_id.clone(),
            cfi: "epubcfi(/6/2!/4/2/1:0)".into(),
        });
        store
            .record(
                "mood",
                String::new(),
                Some("疲惫".into()),
                echo::respond("", Some("疲惫")),
                None,
            )
            .unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, _, cx| {
            app.active_book = Some(book_id.clone());
            app.page = Page::Reader;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx.debug_bounds("留下书签").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| {
            assert_eq!(app.store.state.books[0].bookmarks.len(), 1)
        });

        app.update_in(cx, |app, _, cx| {
            app.page = Page::Journal;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx.debug_bounds("收藏").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| assert!(app.store.state.journal[0].favorite));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx.debug_bounds("删除").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| assert!(app.store.state.journal.is_empty()));
        let persisted = Store::open(dir.path().join("data")).unwrap();
        assert_eq!(persisted.state.books[0].bookmarks.len(), 1);
        assert!(persisted.state.journal.is_empty());
    }

    #[gpui::test]
    fn multi_quote_echo_drawer_renders_with_stable_controls(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().into()).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let book_id = store.state.books[0].id.clone();
        let result = echo::respond("窗边有一盏小灯。", Some("疲惫"));
        assert_eq!(result.quotes.len(), 3);
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, _, cx| {
            app.active_book = Some(book_id);
            app.page = Page::Reader;
            app.drawer = "echo".into();
            app.echo_input = "窗边有一盏小灯。".into();
            app.echo = Some(result);
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("收起").is_some());
    }

    #[gpui::test]
    fn custom_theme_save_persists_and_restores(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let mut store = Store::open(data.clone()).unwrap();
        store.state.privacy_seen = true;
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, window, cx| {
            app.act(Action::Page(Page::Themes), window, cx);
            app.act(Action::EditTheme, window, cx);
            app.fields[0].update(cx, |state, cx| state.set_value("林间夜读", window, cx));
            app.fields[7].update(cx, |state, cx| state.set_value("23", window, cx));
            app.act(Action::ReducedMotion, window, cx);
            app.act(Action::SaveTheme, window, cx);
            assert_eq!(app.theme().name, "林间夜读");
            assert_eq!(app.theme().font_size, 23.0);
            assert!(app.theme().reduced_motion);
            assert_eq!(app.store.state.custom_themes.len(), 1);
        });
        let reopened = Store::open(data).unwrap();
        assert_eq!(reopened.state.theme.name, "林间夜读");
        assert_eq!(reopened.state.theme.font_size, 23.0);
        assert!(reopened.state.theme.reduced_motion);
        assert_eq!(reopened.state.custom_themes.len(), 1);
    }

    #[gpui::test]
    fn shelf_removal_confirm_clears_cached_files(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("data")).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let book_id = store.state.books[0].id.clone();
        store.state.last_book = Some(book_id.clone());
        let cached = store.root.join("books").join(&book_id);
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, _, cx| {
            app.active_book = Some(book_id.clone());
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx.debug_bounds("移出").expect("remove button laid out");
        cx.simulate_click(bounds.center(), Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx.debug_bounds("确认").expect("confirmation laid out");
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| {
            assert!(app.store.state.books.is_empty());
            assert!(app.active_book.is_none());
            assert!(app.store.state.last_book.is_none());
        });
        assert!(!cached.exists());
    }

    #[gpui::test]
    fn reader_can_keep_and_remove_a_highlight_from_the_echo_panel(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let mut store = Store::open(data.clone()).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let book_id = store.state.books[0].id.clone();
        let cfi = "epubcfi(/6/4!/4/2,/1:0,/1:8)";
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, _, cx| {
            app.active_book = Some(book_id.clone());
            app.page = Page::Reader;
            app.opened = true;
            app.echo_input = "想留住的句子".into();
            app.echo_cfi = cfi.into();
            app.echo = Some(echo::respond("想留住的句子", None));
            app.drawer = "echo".into();
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx
            .debug_bounds("留下划线")
            .expect("highlight action laid out");
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| {
            assert_eq!(app.store.state.books[0].highlights.len(), 1);
            assert_eq!(app.store.state.books[0].highlights[0].cfi, cfi);
        });
        app.update_in(cx, |app, window, cx| {
            app.act(Action::Highlight, window, cx);
            assert_eq!(app.store.state.books[0].highlights.len(), 1);
            let highlight_id = app.store.state.books[0].highlights[0].id.clone();
            app.act(Action::DeleteHighlight(highlight_id), window, cx);
            assert!(app.store.state.books[0].highlights.is_empty());
        });
        assert!(
            Store::open(data).unwrap().state.books[0]
                .highlights
                .is_empty()
        );
    }

    #[gpui::test]
    fn search_results_and_progress_update_native_reader_state(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("data")).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let book_id = store.state.books[0].id.clone();
        store.state.last_book = Some(book_id.clone());
        store.state.books[0].location = Some(Location {
            book_id: book_id.clone(),
            cfi: "epubcfi(/6/4!/4/2,/1:0,/1:8)".into(),
        });
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, _, cx| {
            app.active_book = Some(book_id);
            app.page = Page::Reader;
            app.opened = true;
            app.drawer = "search".into();
            app.search_results = vec![SearchHit {
                cfi: "epubcfi(/6/4!/4/4,/1:0,/1:8)".into(),
                excerpt: "风从窗边吹过，书页停在这一行。".into(),
                chapter: "第二章 风".into(),
            }];
            app.event(
                ReaderEvent {
                    version: 1,
                    session: app.session.clone(),
                    request_id: 1,
                    kind: "location".into(),
                    text: String::new(),
                    cfi: "epubcfi(/6/4!/4/4,/1:0,/1:8)".into(),
                    color: String::new(),
                    open_note: false,
                    entries: vec![],
                    progress: Some(0.42),
                    chapter: "第二章 风".into(),
                    results: vec![],
                },
                cx,
            );
            assert_eq!(app.store.state.books[0].progress, Some(0.42));
            assert_eq!(
                app.store.state.books[0].chapter.as_deref(),
                Some("第二章 风")
            );
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("收起").is_some());
        assert!(cx.debug_bounds("风从窗边吹过，书页停在这一行。").is_some());
        app.update_in(cx, |app, _, cx| {
            app.page = Page::Shelf;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("继续上次阅读").is_some());
    }

    #[gpui::test]
    fn reader_typography_overrides_apply_and_persist(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let mut store = Store::open(data.clone()).unwrap();
        store.state.privacy_seen = true;
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, window, cx| {
            app.act(Action::TypeSize(2), window, cx);
            app.act(Action::TypeLeading(1), window, cx);
            app.act(Action::TypeWidth(1), window, cx);
            assert_eq!(app.store.state.reader_font_size, Some(22.0));
            assert_eq!(app.store.state.reader_line_height, Some(2.0));
            assert_eq!(app.store.state.reader_width, Some(760));
            let json = app.theme_json();
            assert_eq!(json["font_size"], 22.0);
            assert_eq!(json["line_height"], 2.0);
            assert_eq!(json["width"], 760);
        });
        let reopened = Store::open(data).unwrap();
        assert_eq!(reopened.state.reader_font_size, Some(22.0));
        assert_eq!(reopened.state.reader_line_height, Some(2.0));
        assert_eq!(reopened.state.reader_width, Some(760));
    }

    #[gpui::test]
    fn prism_theme_mouse_move_click_and_pet_interactions_do_not_panic(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().into()).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, window, cx| {
            app.act(Action::PickTheme(3), window, cx);
            assert_eq!(app.theme().id, "prism");
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_mouse_move(point(px(320.), px(220.)), None, Modifiers::default());
        cx.simulate_mouse_move(point(px(540.), px(260.)), None, Modifiers::default());
        cx.simulate_click(point(px(540.), px(260.)), Modifiers::default());
        app.update_in(cx, |app, window, cx| {
            app.act(Action::PetInteract(point(px(900.), px(40.))), window, cx);
            assert!(app.notice.as_deref().is_some_and(|n| n.contains("棱镜喵")));
            app.act(Action::Page(Page::Mood), window, cx);
            app.act(Action::ToggleFlow, window, cx);
            assert!(app.theme().reduced_motion);
            app.act(Action::ToggleFlow, window, cx);
            assert!(!app.theme().reduced_motion);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_mouse_move(point(px(420.), px(300.)), None, Modifiers::default());
        cx.simulate_click(point(px(420.), px(300.)), Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    #[gpui::test]
    fn direct_colored_highlights_notes_echo_and_ai_passage_work_without_journal_spam(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let mut store = Store::open(data.clone()).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let book_id = store.state.books[0].id.clone();
        let (app, cx) = cx.add_window_view(|window, cx| {
            let mut app = test_app(store, window, cx);
            app.echo_ai_worker = |passage, title| {
                Ok(EchoResult {
                    quotes: vec![],
                    reply: format!("与你共读《{title}》里的这句：{passage}"),
                    demo: false,
                })
            };
            app
        });
        let cfi = "epubcfi(/6/4!/4/2,/1:0,/1:12)";
        app.update_in(cx, |app, window, cx| {
            app.active_book = Some(book_id.clone());
            app.page = Page::Reader;
            app.opened = true;
            // 1. Direct color highlight event does NOT pollute journal
            app.event(
                ReaderEvent {
                    version: 1,
                    session: app.session.clone(),
                    request_id: 1,
                    kind: "highlight".into(),
                    text: "蓝天下的风轻轻吹过。".into(),
                    cfi: cfi.into(),
                    color: "rose".into(),
                    open_note: true,
                    entries: vec![],
                    progress: None,
                    chapter: String::new(),
                    results: vec![],
                },
                cx,
            );
            assert!(app.store.state.journal.is_empty());
            assert_eq!(app.drawer, "highlights");
            assert_eq!(
                app.store.state.books[0].highlights[0].color.as_deref(),
                Some("rose")
            );
            let hl_id = app.store.state.books[0].highlights[0].id.clone();
            // 2. Edit & save note + recolor
            app.act(Action::EditHighlightNote(hl_id.clone()), window, cx);
            app.highlight_note_input
                .update(cx, |s, cx| s.set_value("像天空之城里的云朵。", window, cx));
            app.act(Action::SaveHighlightNote, window, cx);
            app.act(
                Action::RecolorHighlight(Box::new((hl_id, "prism".into()))),
                window,
                cx,
            );
            assert_eq!(
                app.store.state.books[0].highlights[0].note.as_deref(),
                Some("像天空之城里的云朵。")
            );
            assert_eq!(
                app.store.state.books[0].highlights[0].color.as_deref(),
                Some("prism")
            );
            // 3. Casual echo does NOT pollute journal until explicitly saved or AI companion completes
            app.event(
                ReaderEvent {
                    version: 1,
                    session: app.session.clone(),
                    request_id: 2,
                    kind: "echo".into(),
                    text: "傍晚的风吹过林间小路。".into(),
                    cfi: cfi.into(),
                    color: String::new(),
                    open_note: false,
                    entries: vec![],
                    progress: None,
                    chapter: String::new(),
                    results: vec![],
                },
                cx,
            );
            assert!(app.store.state.journal.is_empty());
            assert!(app.echo.as_ref().is_some_and(|e| !e.quotes.is_empty()));
            // 4. Trigger manual AI 段落共读
            app.act(Action::AskAiPassage, window, cx);
            let result = app
                .echo_ai_rx
                .take()
                .unwrap()
                .recv_timeout(Duration::from_secs(3))
                .unwrap();
            app.finish_ai_passage(result, cx);
            assert_eq!(app.store.state.journal.len(), 1);
            assert!(!app.store.state.journal[0].echo.demo);
            assert!(app.store.state.journal[0].echo.reply.contains("共读"));
            assert!(app.echo_saved);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let reopened = Store::open(data).unwrap();
        assert_eq!(
            reopened.state.books[0].highlights[0].note.as_deref(),
            Some("像天空之城里的云朵。")
        );
        assert_eq!(reopened.state.journal.len(), 1);
    }

    #[gpui::test]
    fn breathing_focus_timer_zen_warm_filter_palette_and_quote_card_render_cleanly(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.bind_keys(bindings());
        });
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let mut store = Store::open(data.clone()).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let book_id = store.state.books[0].id.clone();
        store
            .add_highlight_styled(
                &book_id,
                "云端的光从窗户洒下来。".into(),
                "epubcfi(/6/4!/4/2,/1:0,/1:10)".into(),
                Some("amber".into()),
                Some("安静的一页".into()),
            )
            .unwrap();
        store
            .record(
                "mood",
                "今天天空很蓝".into(),
                Some("希望".into()),
                echo::respond("", Some("希望")),
                None,
            )
            .unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
        app.update_in(cx, |app, window, cx| {
            // Default theme is Laputa Sky blue-white "林间暖光"
            assert_eq!(app.theme().id, "forest");
            assert_eq!(app.theme().paper, "#F8FCFF");
            assert!(app.is_sky());
            // Unread book shows true 0% progress (not fake clamped 34%)
            assert_eq!(app.overall_reading_progress(), 0.0);
            // Symmetric 5% volume stepping from default 0.55 -> 0.60 -> 0.50
            app.act(Action::AudioVolume(1), window, cx);
            assert!((app.theme().volume - 0.60).abs() < 0.001);
            app.act(Action::AudioVolume(-2), window, cx);
            assert!((app.theme().volume - 0.50).abs() < 0.001);
            // Switching built-in themes preserves user-adjusted volume
            app.act(Action::PickTheme(4), window, cx);
            assert_eq!(app.theme().id, "totoro");
            assert!((app.theme().volume - 0.50).abs() < 0.001);
            app.act(Action::PickTheme(0), window, cx);
            assert_eq!(app.theme().id, "forest");
            assert!((app.theme().volume - 0.50).abs() < 0.001);
            // Daily rediscovery picks up the saved highlight
            assert!(app.daily_rediscovery().is_some());
            // Shelf filters
            app.act(Action::ShelfFilter(ShelfFilter::Unread), window, cx);
            assert_eq!(app.shelf_filter, ShelfFilter::Unread);
            app.act(Action::ShelfFilter(ShelfFilter::All), window, cx);
            // Warm filter persists
            app.act(Action::ToggleWarmFilter, window, cx);
            assert!(app.store.state.warm_filter);
            assert_eq!(app.theme_json()["warm_filter"], true);
            // 4-7-8 Breathing Ritual on Mood page
            app.act(Action::Page(Page::Mood), window, cx);
            app.act(Action::ToggleBreathing, window, cx);
            assert!(app.breathing_started.is_some());
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("结束呼吸").is_some());

        app.update_in(cx, |app, window, cx| {
            app.act(Action::ToggleBreathing, window, cx);
            assert!(app.breathing_started.is_none());
            // Reading Light Vial Focus Timer modal supports custom duration + steppers + Escape
            app.act(Action::ToggleTimerPicker, window, cx);
            assert!(app.focus_timer_picker_open);
            app.act(Action::StepCustomTimer(5), window, cx);
            assert_eq!(app.focus_timer_input.read(cx).value().as_str(), "35");
            app.focus_timer_input
                .update(cx, |s, cx| s.set_value("0", window, cx));
            app.act(Action::StartCustomFocusTimer, window, cx);
            assert!(app.error.is_some());
            assert!(app.focus_timer_picker_open);
            app.focus_timer_input
                .update(cx, |s, cx| s.set_value("38", window, cx));
            app.act(Action::StartCustomFocusTimer, window, cx);
            assert!(app.error.is_none());
            assert!(app.focus_timer_started.is_some());
            assert_eq!(app.focus_timer_total_secs, 38 * 60);
            assert!(!app.focus_timer_picker_open);
            assert!(app.overall_reading_progress() < 0.05);
            app.act(Action::StartFocusTimer(25), window, cx);
            assert_eq!(app.focus_timer_total_secs, 25 * 60);
            // Zen Reading Mode & edge reveal hysteresis
            app.active_book = Some(book_id.clone());
            app.page = Page::Reader;
            app.act(Action::Focus, window, cx);
            assert!(app.focus_mode);
            app.event(
                ReaderEvent {
                    version: 1,
                    session: app.session.clone(),
                    request_id: 1,
                    kind: "edge".into(),
                    text: "top".into(),
                    cfi: String::new(),
                    color: String::new(),
                    open_note: false,
                    entries: vec![],
                    progress: None,
                    chapter: String::new(),
                    results: vec![],
                },
                cx,
            );
            assert!(app.zen_reveal_top);
            // Immediate stale "none" from WebView mouseleave must NOT collapse revealed bar
            app.event(
                ReaderEvent {
                    version: 1,
                    session: app.session.clone(),
                    request_id: 2,
                    kind: "edge".into(),
                    text: "none".into(),
                    cfi: String::new(),
                    color: String::new(),
                    open_note: false,
                    entries: vec![],
                    progress: None,
                    chapter: String::new(),
                    results: vec![],
                },
                cx,
            );
            assert!(app.zen_reveal_top);
        });
        // Moving pointer across the 62px top toolbar (y = 60) keeps toolbar revealed via hysteresis
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_mouse_move(point(px(400.), px(60.)), None, Modifiers::default());
        app.read_with(cx, |app, _| assert!(app.zen_reveal_top));
        // Moving into center of reading area hides top toolbar
        cx.simulate_mouse_move(point(px(400.), px(260.)), None, Modifiers::default());
        app.read_with(cx, |app, _| assert!(!app.zen_reveal_top));

        app.update_in(cx, |app, window, cx| {
            // Journal filter & Markdown formatter
            app.act(Action::Page(Page::Journal), window, cx);
            app.act(Action::JournalFilter(JournalFilter::Mood), window, cx);
            let md = format_journal_markdown(&app.store.state.journal);
            assert!(md.contains("微光手账"));
            assert!(md.contains("今天天空很蓝"));
            // Quote Card Studio modal
            app.act(
                Action::OpenQuoteCard(Box::new(QuoteCardDraft {
                    text: "云端的光从窗户洒下来。".into(),
                    attribution: "《在林间，慢慢来》".into(),
                    note: Some("安静的一页".into()),
                    style: QuoteCardStyle::Sky,
                })),
                window,
                cx,
            );
            assert!(app.quote_card.is_some());
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("保存高清 PNG").is_some());

        // Ctrl+K Command Palette: Escape MUST close even while palette_input holds focus!
        app.update_in(cx, |app, window, cx| {
            app.act(Action::CloseQuoteCard, window, cx);
            app.act(Action::TogglePalette, window, cx);
            assert!(app.palette_open);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_keystrokes("escape");
        app.read_with(cx, |app, _| assert!(!app.palette_open));

        // Command Palette keyboard filtering + Enter execution works end-to-end
        app.update_in(cx, |app, window, cx| {
            app.act(Action::TogglePalette, window, cx);
            app.palette_input
                .update(cx, |s, cx| s.set_value("黑曜棱镜", window, cx));
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_keystrokes("enter");
        app.read_with(cx, |app, _| {
            assert!(!app.palette_open);
            assert_eq!(app.theme().id, "prism");
        });

        // Removed book guard in daily_rediscovery: favorited entry pointing to removed book has target=None
        app.update_in(cx, |app, window, cx| {
            app.store.state.journal[0].favorite = true;
            app.store.state.journal[0].location = Some(Location {
                book_id: book_id.clone(),
                cfi: "epubcfi(/6/4!/4/2,/1:0,/1:10)".into(),
            });
            app.store.remove_book(&book_id).unwrap();
            let red = app.daily_rediscovery().unwrap();
            assert!(red.2.is_none());
            // Palette breathing command transitions cleanly to Mood page and starts breathing
            app.run_palette_command(Action::ToggleBreathing, window, cx);
            assert_eq!(app.page, Page::Mood);
            assert!(app.breathing_started.is_some());
        });
    }

    #[gpui::test]
    fn stale_ai_passage_and_failed_note_event_preserve_active_state(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let mut store = Store::open(data).unwrap();
        store.install_sample().unwrap();
        let book_id = store.state.books[0].id.clone();
        let (hl, _) = store
            .add_highlight_styled(
                &book_id,
                "桌上的枝条在水里长出了细小的根。".into(),
                "epubcfi(/6/4!/4/2,/1:0,/1:12)".into(),
                Some("amber".into()),
                Some("旧批注".into()),
            )
            .unwrap();
        let (app, cx) = cx.add_window_view(|window, cx| {
            let mut app = test_app(store, window, cx);
            app.active_book = Some(book_id.clone());
            app.page = Page::Reader;
            app.session = "s1".into();
            app
        });
        app.update_in(cx, |app, window, cx| {
            // 1. User switches selection to passage B before a stale AI result for passage A finishes
            app.echo_input = "新的选文段落 B".into();
            app.echo_cfi = "epubcfi(/6/4!/4/4,/1:0,/1:10)".into();
            app.echo = Some(EchoResult {
                quotes: vec![],
                reply: "段落 B 的本地回响".into(),
                demo: true,
            });
            app.echo_saved = false;
            app.finish_ai_passage(
                Ok(PassageGenerated {
                    book_id: Some(book_id.clone()),
                    passage: "旧的选文段落 A".into(),
                    cfi: "epubcfi(/6/4!/4/2,/1:0,/1:10)".into(),
                    result: EchoResult {
                        quotes: vec![],
                        reply: "段落 A 的 AI 共读".into(),
                        demo: false,
                    },
                }),
                cx,
            );
            assert_eq!(app.store.state.journal.len(), 1);
            assert_eq!(app.echo.as_ref().unwrap().reply, "段落 B 的本地回响");
            assert!(
                !app.echo_saved,
                "stale AI completion for passage A must not mark passage B as saved"
            );

            // 2. Active note draft is preserved when an incoming open_note highlight event fails
            app.act(Action::EditHighlightNote(hl.id.clone()), window, cx);
            app.highlight_note_input
                .update(cx, |s, cx| s.set_value("正在编辑中的新批注", window, cx));
            let (tx, rx) = mpsc::sync_channel(4);
            app.rx = Some(rx);
            tx.send(
                json!({
                    "version": 1,
                    "session": "s1",
                    "request_id": 1,
                    "kind": "highlight",
                    "text": "   ",
                    "cfi": "epubcfi(/6/4!/4/6,/1:0,/1:6)",
                    "color": "amber",
                    "open_note": true
                })
                .to_string(),
            )
            .unwrap();
            app.tick(window, cx);
            assert!(app.error.is_some());
            assert_eq!(app.highlight_note_target.as_deref(), Some(hl.id.as_str()));
            assert_eq!(
                app.highlight_note_input.read(cx).value().as_str(),
                "正在编辑中的新批注"
            );
        });
    }

    #[gpui::test]
    fn miyazaki_themes_switch_render_pair_soundscapes_and_filter_in_palette(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.bind_keys(bindings());
        });
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let mut store = Store::open(data.clone()).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));

        let specs: &[(usize, &str, bool, &str, SoundscapePreset, QuoteCardStyle)] = &[
            (
                4,
                "totoro",
                false,
                "scene-totoro.png",
                SoundscapePreset::Rain,
                QuoteCardStyle::Totoro,
            ),
            (
                5,
                "spirited",
                false,
                "scene-spirited.png",
                SoundscapePreset::Train,
                QuoteCardStyle::Spirited,
            ),
            (
                6,
                "howl",
                false,
                "scene-howl.png",
                SoundscapePreset::Breeze,
                QuoteCardStyle::Howl,
            ),
            (
                7,
                "kiki",
                false,
                "scene-kiki.png",
                SoundscapePreset::Fireplace,
                QuoteCardStyle::Kiki,
            ),
            (
                8,
                "lantern",
                true,
                "scene-lantern.png",
                SoundscapePreset::Fireplace,
                QuoteCardStyle::Lantern,
            ),
        ];

        for &(idx, id, dark, asset, preset, card_style) in specs {
            let bytes = platform::Assets
                .load(asset)
                .unwrap()
                .expect("embedded scene PNG exists");
            assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
            app.update_in(cx, |app, window, cx| {
                app.act(Action::PickTheme(idx), window, cx);
                assert_eq!(app.theme().id, id);
                assert!(app.is_ghibli());
                assert_eq!(app.is_dark(), dark);
                assert_eq!(app.scene_asset(), asset);
                assert_eq!(app.active_soundscape_preset(), preset);
                assert_eq!(QuoteCardStyle::from_theme_id(id), card_style);
                app.act(Action::Page(Page::Shelf), window, cx);
            });
            cx.update(|window, cx| window.draw(cx).clear(cx));
            cx.simulate_mouse_move(point(px(460.), px(240.)), None, Modifiers::default());
            cx.simulate_click(point(px(460.), px(240.)), Modifiers::default());
            app.update_in(cx, |app, window, cx| {
                app.act(Action::Page(Page::Mood), window, cx);
            });
            cx.update(|window, cx| window.draw(cx).clear(cx));
            app.update_in(cx, |app, window, cx| {
                app.act(Action::Page(Page::Themes), window, cx);
            });
            cx.update(|window, cx| window.draw(cx).clear(cx));
        }

        // Command palette search resolves every new Miyazaki theme accurately
        for &(query, expected_id) in &[
            ("龙猫", "totoro"),
            ("海上电车", "spirited"),
            ("哈尔", "howl"),
            ("魔女", "kiki"),
            ("汤屋", "lantern"),
        ] {
            app.update_in(cx, |app, window, cx| {
                app.act(Action::TogglePalette, window, cx);
                app.palette_input
                    .update(cx, |s, cx| s.set_value(query, window, cx));
                cx.notify();
            });
            cx.update(|window, cx| window.draw(cx).clear(cx));
            cx.simulate_keystrokes("enter");
            app.read_with(cx, |app, _| {
                assert!(!app.palette_open);
                assert_eq!(app.theme().id, expected_id);
            });
        }

        // Persisted state reloads the selected Miyazaki theme cleanly
        let reopened = Store::open(data).unwrap();
        assert_eq!(reopened.state.theme.id, "lantern");
    }

    #[gpui::test]
    fn focus_timer_modal_renders_and_closes_on_escape_and_book_cover_opens(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.bind_keys(bindings());
        });
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().into()).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));

        // Clicking the sidebar Quiet Reading Timer card opens the custom duration modal
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let timer_bounds = cx
            .debug_bounds("vial-trigger")
            .expect("sidebar quiet timer trigger laid out");
        cx.simulate_click(timer_bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| assert!(app.focus_timer_picker_open));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("focus-timer-dialog").is_some());

        // Escape closes the timer modal even while the custom minutes input holds focus
        cx.simulate_keystrokes("escape");
        app.read_with(cx, |app, _| assert!(!app.focus_timer_picker_open));

        // Clicking directly on the book cover/body area triggers Action::Open
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let book_bounds = cx
            .debug_bounds("book-open-0")
            .expect("clickable book cover body laid out");
        assert!(book_bounds.size.height > px(180.));
    }

    #[gpui::test]
    fn mixer_companion_tts_and_github_sync_ui_render_and_keep_secrets_out_of_state(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.bind_keys(bindings());
        });
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let mut store = Store::open(data.clone()).unwrap();
        store.install_sample().unwrap();
        store.state.privacy_seen = true;
        let book_id = store.state.books[0].id.clone();
        let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));

        // 1. Multi-track soundscape mixer modal opens from sidebar, applies recipe, closes on Esc
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let mixer_btn = cx
            .debug_bounds("mixer-trigger")
            .expect("sidebar mixer trigger laid out");
        cx.simulate_click(mixer_btn.center(), Modifiers::default());
        app.read_with(cx, |app, _| assert!(app.mixer_open));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("soundscape-mixer-dialog").is_some());
        app.update_in(cx, |app, window, cx| {
            app.act(Action::ApplyMixRecipe("sea_train"), window, cx);
            assert!(app.store.state.soundscape_mix.train > 0.5);
            assert!(app.store.state.soundscape_mix.piano > 0.3);
            app.stop_audio_smoothly();
        });
        cx.simulate_keystrokes("escape");
        app.read_with(cx, |app, _| assert!(!app.mixer_open));

        // 2. Spoiler-free AI companion drawer (Recap + Interactive Character Graph + Poster)
        app.update_in(cx, |app, window, cx| {
            app.active_book = Some(book_id.clone());
            app.page = Page::Reader;
            app.act(Action::ToggleCompanion, window, cx);
            assert_eq!(app.drawer, "companion");
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        app.update_in(cx, |app, window, cx| {
            app.act(Action::CompanionTab(1), window, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        app.update_in(cx, |app, window, cx| {
            app.act(Action::OpenCompanionPoster, window, cx);
            assert!(app.quote_card.is_some());
            app.act(Action::CloseQuoteCard, window, cx);
        });

        // 3. Edge-TTS Audiobook Studio drawer renders and updates voice/rate settings
        app.update_in(cx, |app, window, cx| {
            app.act(Action::ToggleTtsDrawer, window, cx);
            assert_eq!(app.drawer, "tts");
            app.act(Action::TtsVoice("zh-CN-YunxiNeural".into()), window, cx);
            app.act(Action::TtsRate(1.15), window, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));

        // 4. Multi-Provider AI & GitHub Sync Settings save secrets strictly outside state.json
        app.update_in(cx, |app, window, cx| {
            app.act(Action::Page(Page::Themes), window, cx);
            app.act(Action::SelectAiPreset("llamacpp".into()), window, cx);
            assert_eq!(app.store.state.ai.base_url, "http://127.0.0.1:8080/v1");
            app.discovered_ai_models = vec!["qwen2.5:7b".into(), "deepseek-r1:7b".into()];
            app.act(Action::PickDiscoveredModel("qwen2.5:7b".into()), window, cx);
            assert_eq!(app.store.state.ai.model, "qwen2.5:7b");

            app.act(Action::SelectAiPreset("custom".into()), window, cx);
            app.ai_base_url_input.update(cx, |s, cx| {
                s.set_value("https://my-gateway.example/v1/chat/completions", window, cx)
            });
            app.ai_key_input
                .update(cx, |s, cx| s.set_value("sk-secret-ai-never-sync", window, cx));
            app.act(Action::SaveAiSettings, window, cx);

            app.act(Action::SelectSyncMode("github".into()), window, cx);
            app.sync_owner_input
                .update(cx, |s, cx| s.set_value("demo-user", window, cx));
            app.sync_token_input
                .update(cx, |s, cx| s.set_value("ghp_secret_never_sync", window, cx));
            app.act(Action::SaveSyncSettings, window, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));

        let state_text = fs::read_to_string(data.join("state.json")).unwrap();
        assert!(!state_text.contains("sk-secret-ai-never-sync"));
        assert!(!state_text.contains("ghp_secret_never_sync"));
        let secrets_text = fs::read_to_string(data.join("secrets.json")).unwrap();
        assert!(secrets_text.contains("sk-secret-ai-never-sync"));
        assert!(secrets_text.contains("ghp_secret_never_sync"));
    }
}

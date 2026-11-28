mod actions;
mod features;
mod modals;
mod mood_journal;
mod radio_view;
mod reader;
mod shelf;
mod sidebar;
mod smoke;
mod theme_meta;
mod themes_view;

#[cfg(all(test, feature = "ui-test"))]
mod tests;

use theme_meta::*;

use crate::{
    mood, platform,
    prism::{self, MotionEngine, MotionSnapshot, ThemeAtmosphere},
    quotecard::{self, CompletionTicketDraft, QuoteCardDraft, QuoteCardStyle},
    radio,
    soundscape::{ChimeSource, SoundscapeHandle, SoundscapePreset, SoundscapeSource},
    sync_client, tts,
};
use anyhow::Context as _;
use glow_core::{
    AiSettings, BookCompanionCache, Bookmark, EchoResult, JournalEntry, LocalSecrets, Location,
    MemoryHit, SoundscapeMix, Store, Theme as ReadingTheme,
    bridge::{ReaderEvent, SearchHit, TocEntry},
    catalog, dictionary, echo,
    model::themes,
    opds,
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
    sync::{Arc, Mutex, mpsc},
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
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShelfTab {
    Library,
    Catalog,
    Opds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JournalFilter {
    All,
    Favorite,
    Mood,
    Selection,
    Ticket,
}

#[derive(Clone, Debug, PartialEq)]
enum Page {
    Shelf,
    Reader,
    Mood,
    Radio,
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
    // Dual-Track Compliant TTS Audiobook Studio
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
    // Glow FM Late-Night Radio
    RadioTogglePlay,
    RadioTuneStation(String),
    RadioTuneMhz(f32),
    RadioPrevStation,
    RadioNextStation,
    RadioNextSegment,
    RadioToggleFavorite(String),
    RadioLampColor(String),
    RadioLampMode(String),
    RadioVolume(i8),
    RadioSetVolume(f32),
    RadioTogglePureGlow,
    RadioToggleImmersive,
    RadioToggleVisualMode,
    RadioCycleSleepTimer,
    RadioToggleDrawer,
    RadioCloseDrawer,
    RadioDrawerTab(u8),
    RadioToggleFavFilter,
    RadioSearchSubmit,
    RadioSearchTag(&'static str),
    RadioPreviewHit(usize),
    RadioAddHit(usize),
    RadioAddCustomStream,
    RadioRemoveCustomStation(String),
    RadioPickLocalFolder,
    RadioSetHostVoice(String),
    RadioSaveLineToJournal,
    // Phase 1 V1.0 Core Experience
    OnboardingStep(u8),
    OnboardingPickCatalog(String),
    ShelfTab(ShelfTab),
    ShelfMoodPick(String),
    InstallCatalogBook(String, bool),
    CycleBookStatus(String),
    EditBookMeta(String),
    SaveBookMeta,
    CancelBookMeta,
    OpdsAddFeed,
    OpdsRemoveFeed(String),
    OpdsFetchFeed(String),
    OpdsImportEntry(usize),
    DismissReunion,
    OpenFootnote(String),
    AskAiFootnote,
    CloseFootnote,
    OpenCompletionModal(String),
    GenerateAiCompletionLetter,
    ConfirmCompleteBook,
    SaveCompletionTicketPng,
    SaveJournalTicketPng(String),
    CloseCompletionModal,
    ToggleJournalLock,
    ToggleDaylightAuto,
    ToggleAmbientPresence,
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
    echo_memories: Vec<MemoryHit>,
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
    shelf_tab: ShelfTab,
    shelf_mood_pick: String,
    editing_book_id: Option<String>,
    book_meta_title_input: Entity<InputState>,
    book_meta_author_input: Entity<InputState>,
    book_meta_tags_input: Entity<InputState>,
    opds_name_input: Entity<InputState>,
    opds_url_input: Entity<InputState>,
    opds_active_title: String,
    opds_active_url: String,
    opds_entries: Vec<opds::OpdsEntry>,
    opds_fetch_rx: Option<mpsc::Receiver<anyhow::Result<(String, opds::OpdsFeed)>>>,
    opds_import_rx: Option<mpsc::Receiver<anyhow::Result<Vec<u8>>>>,
    onboarding_step: u8,
    reunion_dismissed: bool,
    footnote_term: String,
    footnote_dict: Option<dictionary::DictEntry>,
    footnote_ai_reply: Option<String>,
    footnote_ai_rx: Option<mpsc::Receiver<anyhow::Result<String>>>,
    completion_book_id: Option<String>,
    completion_draft_letter: String,
    completion_letter_rx: Option<mpsc::Receiver<anyhow::Result<String>>>,
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
    tts_last_synced_segment: Option<usize>,
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
    // Glow FM Late-Night Radio state
    radio_player: Option<radio::RadioPlayer>,
    radio_status: radio::RadioStatus,
    radio_audio_level: f32,
    radio_breathe_started: Instant,
    radio_pure_glow: bool,
    radio_hide_sidebar: bool,
    radio_visual_mode: u8,
    radio_vinyl_angle: f32,
    radio_vinyl_vel: f32,
    radio_tonearm_pos: f32,
    radio_dial_display_mhz: f32,
    radio_dial_dragging: bool,
    radio_drag_mhz: Option<f32>,
    radio_dial_bounds: Arc<Mutex<Option<Bounds<Pixels>>>>,
    radio_vol_dragging: bool,
    radio_vol_bounds: Arc<Mutex<Option<Bounds<Pixels>>>>,
    radio_cover_cache: Arc<Mutex<prism::VinylCoverCache>>,
    radio_last_motion_at: Instant,
    radio_last_wheel_at: Instant,
    radio_toast_at: Option<Instant>,
    radio_drawer_open: bool,
    radio_drawer_tab: u8,
    radio_only_favorites: bool,
    radio_sleep_started: Option<Instant>,
    radio_sleep_total_secs: u32,
    radio_search_input: Entity<InputState>,
    radio_search_rx: Option<mpsc::Receiver<anyhow::Result<Vec<radio::OnlineStationHit>>>>,
    radio_search_results: Vec<radio::OnlineStationHit>,
    radio_custom_name_input: Entity<InputState>,
    radio_custom_url_input: Entity<InputState>,
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
            let mut s = InputState::new(window, cx).placeholder(
                "Base URL 或完整接口（如 https://your-api/v1 或 http://127.0.0.1:11434/v1）",
            );
            s.set_value(ai_base_url_val, window, cx);
            s
        });
        let ai_model_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("例如 deepseek-chat 或 qwen2.5:7b");
            s.set_value(ai_model_val, window, cx);
            s
        });
        let ai_key_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx)
                .placeholder("API Key（本地 Ollama/LM Studio 无需填写）");
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
            let mut s =
                InputState::new(window, cx).placeholder("私有同步仓库名（如 glow-reading-sync）");
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
            let mut s =
                InputState::new(window, cx).placeholder("GitHub Personal Access Token（仅存本机）");
            s.set_value(sync_token_val, window, cx);
            s
        });
        let radio_search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入电台名或关键词，如 lofi、jazz、piano、sleep…")
        });
        cx.subscribe_in(
            &radio_search_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) && this.radio_drawer_open {
                    this.act(Action::RadioSearchSubmit, window, cx);
                }
            },
        )
        .detach();
        let radio_custom_name_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("自定义电台名称（如：雨夜东京爵士）")
        });
        let radio_custom_url_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("音频流地址（https://...mp3 或 .ogg）")
        });
        let book_meta_title_input = cx.new(|cx| InputState::new(window, cx).placeholder("书名"));
        let book_meta_author_input = cx.new(|cx| InputState::new(window, cx).placeholder("作者"));
        let book_meta_tags_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("书单标签（逗号分隔，如：睡前, 散文）")
        });
        let opds_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("书源名称（如：Calibre 书房）"));
        let opds_url_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("OPDS 1.2 地址（https://.../opds）"));
        cx.subscribe_in(
            &radio_custom_url_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) && this.radio_drawer_open {
                    this.act(Action::RadioAddCustomStream, window, cx);
                }
            },
        )
        .detach();
        let initial_radio_mhz = {
            let stations = radio::all_stations(&store.state.radio.custom_stations);
            radio::station_by_id(&stations, &store.state.radio.current_station_id).frequency_mhz
        };
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
            echo_memories: vec![],
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
            shelf_tab: ShelfTab::Library,
            shelf_mood_pick: "睡前".into(),
            editing_book_id: None,
            book_meta_title_input,
            book_meta_author_input,
            book_meta_tags_input,
            opds_name_input,
            opds_url_input,
            opds_active_title: String::new(),
            opds_active_url: String::new(),
            opds_entries: vec![],
            opds_fetch_rx: None,
            opds_import_rx: None,
            onboarding_step: 0,
            reunion_dismissed: false,
            footnote_term: String::new(),
            footnote_dict: None,
            footnote_ai_reply: None,
            footnote_ai_rx: None,
            completion_book_id: None,
            completion_draft_letter: String::new(),
            completion_letter_rx: None,
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
            tts_last_synced_segment: None,
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
            radio_player: None,
            radio_status: radio::RadioStatus::default(),
            radio_audio_level: 0.0,
            radio_breathe_started: Instant::now(),
            radio_pure_glow: false,
            radio_hide_sidebar: true,
            radio_visual_mode: 0,
            radio_vinyl_angle: 0.0,
            radio_vinyl_vel: 0.0,
            radio_tonearm_pos: 0.0,
            radio_dial_display_mhz: initial_radio_mhz,
            radio_dial_dragging: false,
            radio_drag_mhz: None,
            radio_dial_bounds: Arc::new(Mutex::new(None)),
            radio_vol_dragging: false,
            radio_vol_bounds: Arc::new(Mutex::new(None)),
            radio_cover_cache: Arc::new(Mutex::new(prism::VinylCoverCache::new())),
            radio_last_motion_at: Instant::now(),
            radio_last_wheel_at: Instant::now() - Duration::from_secs(1),
            radio_toast_at: None,
            radio_drawer_open: false,
            radio_drawer_tab: 0,
            radio_only_favorites: false,
            radio_sleep_started: None,
            radio_sleep_total_secs: 30 * 60,
            radio_search_input,
            radio_search_rx: None,
            radio_search_results: vec![],
            radio_custom_name_input,
            radio_custom_url_input,
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
            "forest"
                | "paper"
                | "night"
                | "prism"
                | "totoro"
                | "spirited"
                | "howl"
                | "kiki"
                | "lantern"
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
        value["warm_filter"] = json!(self.effective_warm_filter());
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
            let (stream, handle) = crate::soundscape::try_default_output_stream()
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
        let (stream, handle) = crate::soundscape::try_default_output_stream()
            .map_err(|_| anyhow::anyhow!("未检测到可用的音频输出设备"))?;
        let sink = rodio::Sink::try_new(&handle)?;
        sink.set_volume(vol);
        let (src, stop_handle) = SoundscapeSource::with_mix_handle(&mix);
        if self.tts_status.active && !self.tts_status.paused && self.store.state.tts.duck_soundscape
        {
            stop_handle.set_ducking(true);
        }
        sink.append(src);
        self.stop_audio_smoothly();
        self.audio = Some((stream, sink, Some(stop_handle)));
        Ok(label)
    }
    fn play_chime(&mut self) {
        if let Ok((stream, handle)) = crate::soundscape::try_default_output_stream()
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
        } else if self.radio_drawer_open {
            self.radio_drawer_open = false;
            window.focus(&self.focus, cx);
            cx.notify();
        } else if self.page == Page::Radio && self.radio_pure_glow {
            self.radio_pure_glow = false;
            cx.notify();
        } else if self.page == Page::Radio && self.radio_hide_sidebar {
            self.radio_hide_sidebar = false;
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
        } else if self.completion_book_id.is_some() {
            self.completion_book_id = None;
            self.completion_letter_rx = None;
            window.focus(&self.focus, cx);
            cx.notify();
        } else if self.editing_book_id.is_some() {
            self.editing_book_id = None;
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
        self.echo_memories.clear();
        self.reunion_dismissed = false;
        self.footnote_term.clear();
        self.footnote_dict = None;
        self.footnote_ai_reply = None;
        self.footnote_ai_rx = None;
        self.tts_last_synced_segment = None;
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
}

impl Render for ReadingApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme().clone();
        let dark = self.is_dark();
        let is_prism = self.is_prism();
        let continuous_flow = !t.reduced_motion
            && (is_prism
                || self.page == Page::Radio
                || (self.radio_status.playing && !self.radio_status.paused)
                || self.audio.is_some()
                || self.tts_status.active
                || self.breathing_started.is_some()
                || self.focus_timer_started.is_some());
        let needs_frame = self.motion.step(t.reduced_motion, continuous_flow);
        let radio_needs_frame = self.step_radio_motion(t.reduced_motion);
        if needs_frame || radio_needs_frame {
            self.schedule_motion_frame(window, cx);
        }
        let snap = self.motion_snapshot();
        let narrow = self.is_narrow_reader(window);
        let visible = self.page == Page::Reader
            && !self.busy
            && self.confirm.is_none()
            && self.quote_card.is_none()
            && self.completion_book_id.is_none()
            && !self.focus_timer_picker_open
            && !self.mixer_open
            && !self.radio_drawer_open
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
                Page::Radio => self.radio_page(window, cx),
                Page::Journal => self.journal(cx),
                Page::Themes => self.theme_page(cx),
            }
        };
        let mut main = v_flex().flex_1().h_full().min_w_0();
        let show_top_banner = self.page != Page::Reader && self.page != Page::Radio;
        if show_top_banner && let Some(message) = self.error.as_ref().or(self.notice.as_ref()) {
            main = main.child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .px_5()
                    .py_2p5()
                    .bg(rgb(hex(&t.paper)).opacity(0.96))
                    .border_b_1()
                    .border_color(self.border_subtle())
                    .text_color(if self.error.is_some() {
                        rgb(0xb34f36)
                    } else {
                        rgb(hex(&t.accent))
                    })
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
                if this.radio_dial_dragging {
                    if event.pressed_button == Some(MouseButton::Left) {
                        this.update_radio_dial_drag(event.position, window, cx);
                    } else {
                        this.finish_radio_dial_drag(window, cx);
                    }
                }
                if this.radio_vol_dragging {
                    if event.pressed_button == Some(MouseButton::Left) {
                        this.update_radio_vol_drag(event.position, window, cx);
                    } else {
                        this.finish_radio_vol_drag(cx);
                    }
                }
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
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.finish_radio_drags(window, cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.finish_radio_drags(window, cx);
                }),
            )
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
            root = root.child(img(preload).absolute().size(px(1.)).opacity(0.001));
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
        let hide_sidebar = (self.focus_mode && self.page == Page::Reader)
            || (self.page == Page::Radio && (self.radio_pure_glow || self.radio_hide_sidebar));
        if !hide_sidebar {
            root = root.child(self.render_sidebar(&t, snap.clone(), cx).relative());
        }
        root = root.child(main.relative());
        if let Some(draft) = &self.quote_card {
            root = root.child(self.quote_card_modal(draft, cx));
        }
        if let Some(book_id) = &self.completion_book_id {
            root = root.child(self.completion_ceremony_modal(book_id, cx));
        }
        if self.focus_timer_picker_open {
            root = root.child(self.focus_timer_modal(snap.clone(), cx));
        }
        if self.mixer_open {
            root = root.child(self.soundscape_mixer_modal(snap.clone(), cx));
        }
        if self.radio_drawer_open {
            root = root.child(self.radio_drawer_modal(cx));
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
        if let Some(radio) = self.radio_player.take() {
            radio.stop();
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

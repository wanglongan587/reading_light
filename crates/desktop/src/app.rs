use crate::{
    mood, platform,
    prism::{self, MotionEngine, MotionSnapshot},
};
use anyhow::Context as _;
use glow_core::{
    Bookmark, EchoResult, Location, Store, Theme as ReadingTheme,
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
    input::{Input, InputState, Textarea, TextareaState},
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

actions!(glow, [Escape, Import, Previous, Next, Search]);

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
    DeleteHighlight(String),
    Typography,
    TypeSize(i8),
    TypeLeading(i8),
    TypeWidth(i16),
    ResetTypography,
    Bookmarks,
    Bookmark,
    Locate(String),
    CloseEcho,
    Mood(String),
    SubmitMood,
    CancelMood,
    Favorite(String),
    DeleteEntry(String),
    ClearJournal,
    ConfirmClear,
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
    ReducedMotion,
    ToggleFlow,
    PetInteract(Point<Pixels>),
    RevealSpoiler(String),
    Privacy,
    Encoding(String),
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
    search_input: Entity<InputState>,
    search_results: Vec<SearchHit>,
    searching: bool,
    echo: Option<EchoResult>,
    echo_input: String,
    echo_cfi: String,
    mood_reply: Option<(String, EchoResult)>,
    mood_rx: Option<mpsc::Receiver<anyhow::Result<mood::Generated>>>,
    mood_unsaved: Option<mood::Generated>,
    mood_worker: fn(&str, &str) -> anyhow::Result<EchoResult>,
    mood_scroll: ScrollHandle,
    mood: String,
    mood_input: Entity<TextareaState>,
    error: Option<String>,
    notice: Option<String>,
    confirm: Option<String>,
    focus: FocusHandle,
    draft: Option<ReadingTheme>,
    preview: Option<ReadingTheme>,
    fields: Vec<Entity<InputState>>,
    spoilers: HashSet<String>,
    audio: Option<(rodio::OutputStream, rodio::Sink)>,
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
}

impl ReadingApp {
    pub fn new(store: Store, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mood_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("想说的话，可以写在这里（选填）")
                .auto_grow(4, 8)
        });
        cx.observe(&mood_input, |_, _, cx| cx.notify()).detach();
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索书名、人物或书中一句话"));
        let fields = (0..13)
            .map(|_| cx.new(|cx| InputState::new(window, cx)))
            .collect();
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
            search_input,
            search_results: vec![],
            searching: false,
            echo: None,
            echo_input: String::new(),
            echo_cfi: String::new(),
            mood_reply: None,
            mood_rx: None,
            mood_unsaved: None,
            mood_worker: mood::generate,
            mood_scroll: ScrollHandle::new(),
            mood: "疲惫".into(),
            mood_input,
            error: None,
            notice: None,
            confirm: None,
            focus: cx.focus_handle(),
            draft: None,
            preview: None,
            fields,
            spoilers: HashSet::new(),
            audio: None,
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
            "forest" | "paper" | "night" | "prism"
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
        value
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
        self.echo_cfi.clear();
        self.toc.clear();
        self.search_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.store.state.last_book = Some(id.into());
        self.save();
        Ok(())
    }
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                self.event(event, cx);
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
                                    json!({"id":highlight.id,"cfi":highlight.cfi,"text":highlight.text})
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
                    self.dirty = true;
                }
            }
            "echo" => {
                let result = echo::respond(&event.text, None);
                self.echo_input = event.text.clone();
                self.echo_cfi = event.cfi.clone();
                self.echo = Some(result.clone());
                self.drawer = "echo".into();
                self.spoilers.clear();
                let location = self.active_book.as_ref().map(|id| Location {
                    book_id: id.clone(),
                    cfi: event.cfi,
                });
                if let Err(e) = self
                    .store
                    .record("selection", event.text, None, result, location)
                {
                    self.error = Some(e.to_string());
                }
            }
            "search" => {
                self.searching = false;
                self.search_results = event.results;
            }
            "escape" => {
                self.drawer.clear();
                self.focus_mode = false;
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
            Action::Focus => self.focus_mode = !self.focus_mode,
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
            Action::ResetTypography => {
                self.store.state.reader_font_size = None;
                self.store.state.reader_line_height = None;
                self.store.state.reader_width = None;
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
                let (highlight, inserted) = self.store.add_highlight(
                    &book_id,
                    self.echo_input.clone(),
                    self.echo_cfi.clone(),
                )?;
                if inserted {
                    self.send(
                        json!({"kind":"highlight","highlight":{"id":highlight.id,"cfi":highlight.cfi,"text":highlight.text}}),
                        cx,
                    );
                    self.notice = Some("已留下划线，之后可以在「划线」里找回。".into());
                } else {
                    self.notice = Some("这段文字已经留过划线。".into());
                }
            }
            Action::DeleteHighlight(id) => {
                let Some(book_id) = self.active_book.clone() else {
                    return Ok(());
                };
                if let Some(highlight) = self.store.remove_highlight(&book_id, &id)? {
                    self.send(json!({"kind":"unhighlight","cfi":highlight.cfi}), cx);
                    self.notice = Some("已删除这条划线，书页内容不受影响。".into());
                }
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
                    self.notice = Some("已留下书签。".into());
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
                let (sender, receiver) = mpsc::channel();
                self.mood_rx = Some(receiver);
                std::thread::spawn(move || {
                    let result = worker(&text, &selected_mood).map(|result| mood::Generated {
                        input: text,
                        mood: selected_mood,
                        result,
                    });
                    let _ = sender.send(result);
                });
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
            Action::PickTheme(index) => {
                let all = self.all_themes();
                if let Some(t) = all.get(index) {
                    self.store.state.theme = t.clone();
                    self.audio = None;
                    self.preview = None;
                    self.install_native_theme(cx);
                    self.configure(cx);
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
                self.audio = None;
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
                    self.audio = None;
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
                    self.audio = None;
                } else if let Some(path) = &self.theme().audio {
                    let source = rodio::Decoder::new(std::io::BufReader::new(fs::File::open(
                        self.store
                            .root
                            .join("themes")
                            .join(&self.theme().id)
                            .join(path),
                    )?))?;
                    let (stream, handle) = rodio::OutputStream::try_default()?;
                    let sink = rodio::Sink::try_new(&handle)?;
                    sink.set_volume(self.theme().volume);
                    sink.append(source.repeat_infinite());
                    self.audio = Some((stream, sink));
                } else {
                    self.notice = Some("可在主题编辑器中添加自己的环境音。".into());
                }
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
        }
        Ok(())
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
        match self.theme().id.as_str() {
            "prism" => "scene-prism.png",
            "night" => "scene-night.png",
            _ => "scene.png",
        }
    }
    fn motion_snapshot(&self) -> MotionSnapshot {
        let t = self.theme();
        self.motion.snapshot(
            self.is_prism(),
            hex(&t.accent),
            self.is_dark(),
            t.reduced_motion,
        )
    }
    fn overall_reading_progress(&self) -> f32 {
        if let Some(active_id) = self
            .active_book
            .as_ref()
            .or(self.store.state.last_book.as_ref())
            && let Some(book) = self.store.state.books.iter().find(|b| &b.id == active_id)
            && let Some(p) = book.progress
        {
            return p.clamp(0.18, 0.94);
        }
        let max_p = self
            .store
            .state
            .books
            .iter()
            .filter_map(|b| b.progress)
            .fold(0.0_f32, f32::max);
        if max_p > 0.01 {
            max_p.clamp(0.18, 0.94)
        } else {
            0.34
        }
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
            "paper" => "FM 92.4",
            "night" => "FM 101.7",
            "prism" => "FM 108.0",
            _ => "FM 96.0",
        }
    }
    fn waveform_bars(&self, active: bool, heights: &[f32]) -> Div {
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
            let bar_bg: Hsla = if prism {
                prism::spectral_color(
                    (idx as f32 / heights.len().max(1) as f32) + self.motion.phase * 0.06,
                    opacity,
                )
            } else {
                rgb(hex(&t.accent)).opacity(opacity).into()
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
        let total_highlights: usize = self
            .store
            .state
            .books
            .iter()
            .map(|b| b.highlights.len())
            .sum();
        let total_journal = self.store.state.journal.len();
        let mut books = h_flex().flex_wrap().gap_5().items_start();
        for (i, b) in self.store.state.books.iter().enumerate() {
            let colors = [0x385043, 0x69503d, 0x364d56, 0x524656];
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
                    div()
                        .relative()
                        .rounded_xl()
                        .overflow_hidden()
                        .border_1()
                        .border_color(rgb(0xffffff).opacity(0.22))
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
                                .bg(rgb(0x0c1613).opacity(0.76))
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
                                .bg(rgb(0x0c1613).opacity(0.68))
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
                "CONTINUE YOUR STORY · 续读波段",
                "回到故事里，\n那盏灯还亮着。".to_owned(),
                format!("《{}》 · {chapter}{progress}", book.title),
            )
        } else {
            (
                "A LITTLE LIGHT · 慢读时刻",
                "让心慢下来，\n读几页吧。".to_owned(),
                "这里没有读完的期限。故事和微光，都会等你。".to_owned(),
            )
        };
        let snap = self.motion_snapshot();
        let hero = h_flex()
            .relative()
            .w_full()
            .h(px(252.))
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
                    .bg(rgb(hex(&t.paper)).opacity(if is_dark { 0.88 } else { 0.90 }))
                    .border_r_1()
                    .border_color(self.border_subtle())
                    .child(self.heading(hero_kicker, &hero_title, &hero_sub))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
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
                            )),
                    ),
            )
            // Right-side MineRadio Floating Telemetry & Poetic Stage HUD
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
                            .bg(rgb(0x0b1512).opacity(0.68))
                            .border_1()
                            .border_color(rgb(0xffffff).opacity(0.16))
                            .child(self.waveform_bars(true, &[5., 11., 15., 8., 13., 7.]))
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_size(px(9.))
                                            .font_family(self.ui_font())
                                            .text_color(rgb(0xf5dc9c))
                                            .child(format!(
                                                "{}  ·  AMBIENT STAGE",
                                                Self::frequency_for_theme(&t.id)
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
                    .child(
                        v_flex()
                            .px_4()
                            .py_2p5()
                            .gap_0p5()
                            .items_end()
                            .rounded_xl()
                            .bg(rgb(0x0b1512).opacity(0.68))
                            .border_1()
                            .border_color(rgb(0xffffff).opacity(0.16))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(0xf5dc9c))
                                    .child("今天的一小段"),
                            )
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_family(self.display_font())
                                    .text_color(rgb(0xfffdf7))
                                    .child("也足够让风停一会儿"),
                            ),
                    ),
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
                                            .child(format!("{} 本", self.store.state.books.len())),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .px_3()
                            .py_1()
                            .rounded_full()
                            .bg(rgb(hex(&t.paper)).opacity(0.76))
                            .border_1()
                            .border_color(self.border_subtle())
                            .child(
                                Icon::new(IconName::Leaf)
                                    .size(px(12.))
                                    .text_color(rgb(hex(&t.accent))),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("支持将 EPUB / TXT 拖入窗口直接阅读"),
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
        let narrow = f32::from(window.viewport_size().width)
            - if self.focus_mode { 0. } else { 224. }
            - 364.
            < 480.;
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
        bottom = bottom
            .child(hint)
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
        v_flex()
            .size_full()
            .bg(rgb(hex(&t.paper)))
            .child(toolbar)
            .child(body)
            .child(bottom)
            .into_any_element()
    }
    fn reader_drawer(&self, cx: &Context<Self>) -> AnyElement {
        let t = self.theme();
        let title = match self.drawer.as_str() {
            "toc" => "沿着目录走",
            "search" => "在书里找一句话",
            "highlights" => "划过的地方",
            "type" => "把这一页调舒服",
            "bookmarks" => "留过的记号",
            _ => "文字的回响",
        };
        let kicker = match self.drawer.as_str() {
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
                if let Some(book) = self
                    .store
                    .state
                    .books
                    .iter()
                    .find(|book| Some(&book.id) == self.active_book.as_ref())
                {
                    if book.highlights.is_empty() {
                        content = content.child(
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
                                        .child("选中喜欢的句子并打开回响，就可以留下第一条划线。"),
                                ),
                        );
                    }
                    for highlight in book.highlights.iter().rev() {
                        let excerpt = highlight.text.chars().take(180).collect::<String>();
                        content = content.child(
                            v_flex()
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
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_full()
                                                .bg(rgb(hex(&t.accent)).opacity(0.12))
                                                .text_size(px(10.))
                                                .font_family(self.ui_font())
                                                .text_color(rgb(hex(&t.accent)))
                                                .child("HIGHLIGHT · 原文摘录"),
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
                                        .border_l_2()
                                        .border_color(rgb(hex(&t.accent)))
                                        .text_size(px(15.5))
                                        .font_family(self.display_font())
                                        .line_height(relative(1.6))
                                        .child(excerpt),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(self.button(
                                            "回到原文",
                                            Action::Locate(highlight.cfi.clone()),
                                            cx,
                                        ))
                                        .child(self.button(
                                            "删除",
                                            Action::DeleteHighlight(highlight.id.clone()),
                                            cx,
                                        )),
                                ),
                        );
                    }
                }
            }
            "type" => {
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
                content = content.child(
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
                                                .child(self.button(
                                                    "紧",
                                                    Action::TypeLeading(-1),
                                                    cx,
                                                ))
                                                .child(self.button(
                                                    "松",
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
                );
                content = content
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_family(self.ui_font())
                            .line_height(relative(1.65))
                            .text_color(rgb(hex(&t.muted)))
                            .child("调整会立即同步至当前书页，并对之后打开的书保留。"),
                    )
                    .child(self.button("恢复主题默认", Action::ResetTypography, cx));
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
                if let Some(echo) = &self.echo {
                    let already_kept = self
                        .store
                        .state
                        .books
                        .iter()
                        .find(|book| Some(&book.id) == self.active_book.as_ref())
                        .is_some_and(|book| {
                            book.highlights
                                .iter()
                                .any(|highlight| highlight.cfi == self.echo_cfi)
                        });
                    content = content.child(
                        h_flex()
                            .gap_2()
                            .p_1p5()
                            .rounded_xl()
                            .bg(rgb(hex(&t.background)).opacity(0.55))
                            .border_1()
                            .border_color(self.border_subtle())
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
                            .child(self.button_with_icon(
                                "所有划线",
                                IconName::Bookmark,
                                Action::Highlights,
                                cx,
                            )),
                    );
                    content = content.child(self.echo_card(echo, &self.echo_input, cx));
                }
            }
        }
        content.into_any_element()
    }
    fn echo_card(&self, echo: &EchoResult, input: &str, cx: &Context<Self>) -> AnyElement {
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

        let right_stage = v_flex()
            .flex_1()
            .min_w(px(330.))
            .pt(px(22.))
            .gap_4()
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(152.))
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
                            .rounded_xl()
                            .bg(rgb(0x0b1512).opacity(0.72))
                            .border_1()
                            .border_color(rgb(0xffffff).opacity(0.16))
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
                            .child(self.waveform_bars(true, &[6., 13., 18., 9., 15., 11., 7.])),
                    ),
            )
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
                    .child(self.echo_card(echo, input, cx))
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
                    .gap_6()
                    .child(self.heading(
                        "LITTLE THINGS KEPT · 私人回响档案",
                        "那些被文字接住的时刻",
                        "日记保存在这台电脑上，尚未加密。你可以随时删除。",
                    ))
                    .child(
                        h_flex()
                            .gap_2p5()
                            .items_center()
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
                                "清空日记",
                                IconName::X,
                                Action::ClearJournal,
                                cx,
                            )),
                    ),
            );
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
        }
        for entry in &self.store.state.journal {
            let display_input = if entry.input.trim().is_empty() {
                entry
                    .mood
                    .as_ref()
                    .map(|mood| format!("此刻的心情：{mood}"))
                    .unwrap_or_else(|| "此刻的心情".into())
            } else {
                entry.input.clone()
            };
            let mut bar = h_flex()
                .gap_2()
                .pb_3()
                .border_b_1()
                .border_color(self.border_subtle())
                .items_center()
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
                                .bg(rgb(hex(&t.accent)).opacity(0.14))
                                .text_size(px(10.5))
                                .font_family(self.ui_font())
                                .text_color(rgb(hex(&t.accent)))
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
            if let Some(loc) = &entry.location {
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
                    .child(self.echo_card(&entry.echo, &display_input, cx)),
            );
        }
        content.into_any_element()
    }
    fn theme_page(&self, cx: &Context<Self>) -> AnyElement {
        let active_theme = self.theme();
        let is_dark = self.is_dark();
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
                        "从一张纸、一种颜色，到窗外的风景。主题包不包含你的书和日记。",
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
        let mut choices = h_flex().gap_4().flex_wrap();
        for (i, t) in self.all_themes().iter().enumerate() {
            let active = active_theme.id == t.id;
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
            let preview_inner = if matches!(t.id.as_str(), "forest" | "night" | "prism") {
                img(match t.id.as_str() {
                    "prism" => "scene-prism.png",
                    "night" => "scene-night.png",
                    _ => "scene.png",
                })
                .w_full()
                .h(px(132.))
                .object_fit(ObjectFit::Cover)
                .into_any_element()
            } else if let Some(image) = &t.image {
                img(self.store.root.join("themes").join(&t.id).join(image))
                    .w_full()
                    .h(px(132.))
                    .object_fit(ObjectFit::Cover)
                    .into_any_element()
            } else {
                v_flex()
                    .w_full()
                    .h(px(132.))
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
            let is_prism_card = t.id == "prism";
            let card = v_flex()
                .relative()
                .overflow_hidden()
                .w(px(228.))
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
                        .when(is_prism_card, |this| {
                            this.child(
                                div()
                                    .absolute()
                                    .left(px(8.))
                                    .bottom(px(8.))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgb(0x090a0f).opacity(0.82))
                                    .border_1()
                                    .border_color(rgb(0xc4b5fd).opacity(0.42))
                                    .text_size(px(9.))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(0xe9d5ff))
                                    .child("✦ 鼠标流光"),
                            )
                            .child(
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
                                        .child(match t.id.as_str() {
                                            "forest" => "FOREST LIGHT · 88.6 FM",
                                            "paper" => "QUIET PAPER · 92.4 FM",
                                            "night" => "NIGHT GARDEN · 101.7 FM",
                                            "prism" => "OBSIDIAN PRISM · 108.0 FM",
                                            _ => "PERSONAL THEME · 96.0 FM",
                                        }),
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
                .child(if active {
                    self.button_with_icon("正在使用", IconName::Check, Action::PickTheme(i), cx)
                        .primary()
                        .w_full()
                } else {
                    self.button_with_icon(
                        "使用此主题",
                        IconName::Sparkles,
                        Action::PickTheme(i),
                        cx,
                    )
                    .outline()
                    .w_full()
                });
            let card = if active {
                card.border_2()
                    .border_color(rgb(hex(&t.accent)))
                    .shadow_md()
            } else {
                card
            };
            choices = choices.child(card);
        }
        view = view.child(choices).child(
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
        );
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
            11 if self.echo.is_some() && !self.store.state.journal.is_empty() => {
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
}

fn hex(s: &str) -> u32 {
    glow_core::theme::color(s).unwrap_or(0x353e35)
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
        let continuous_flow = !t.reduced_motion && (is_prism || self.audio.is_some());
        let needs_frame = self.motion.step(t.reduced_motion, continuous_flow);
        if needs_frame {
            self.schedule_motion_frame(window, cx);
        }
        let snap = self.motion_snapshot();
        let narrow = f32::from(window.viewport_size().width)
            - if self.focus_mode { 0. } else { 224. }
            - 368.
            < 480.;
        let visible = self.page == Page::Reader
            && !self.busy
            && self.confirm.is_none()
            && self.store.state.privacy_seen
            && (self.drawer.is_empty() || !narrow);
        if let Some(view) = &self.webview {
            if visible {
                view.update(cx, |v, _| v.show());
            } else {
                view.update(cx, |v, _| v.hide());
            }
        }
        let freq = Self::frequency_for_theme(&t.id);
        let mut quick_swatches = h_flex().gap_1p5().items_center();
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
                    .p(px(2.))
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
                    .child(div().size(px(10.)).rounded_full().bg(rgb(dot_color))),
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
        let vial_ratio = self.overall_reading_progress();
        sidebar = sidebar
            .child(div().flex_1())
            .child(
                v_flex()
                    .relative()
                    .overflow_hidden()
                    .p_3()
                    .gap_2p5()
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
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Icon::new(if is_prism {
                                            IconName::Sparkles
                                        } else if t.id == "night" {
                                            IconName::Moon
                                        } else {
                                            IconName::Sun
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
                                        if self.audio.is_some() || (is_prism && !t.reduced_motion) {
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
                                            .child(if self.audio.is_some() {
                                                "LIVE"
                                            } else if is_prism && !t.reduced_motion {
                                                "FLOW"
                                            } else {
                                                "IDLE"
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .px_2p5()
                            .py_2()
                            .rounded_xl()
                            .bg(rgb(hex(&t.paper)).opacity(if dark { 0.62 } else { 0.78 }))
                            .child(
                                div()
                                    .id("vial-trigger")
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.act(Action::ToggleFlow, window, cx);
                                        }),
                                    )
                                    .child(prism::light_vial_canvas(snap.clone(), vial_ratio)),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_size(px(8.5))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.accent)))
                                                    .child(freq),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(8.5))
                                                    .font_family(self.ui_font())
                                                    .text_color(rgb(hex(&t.muted)))
                                                    .child(format!("{:.0}%", vial_ratio * 100.0)),
                                            ),
                                    )
                                    .child(self.waveform_bars(
                                        self.audio.is_some(),
                                        &[5., 11., 15., 8., 13., 9., 6., 12., 8.],
                                    )),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .px_1()
                            .child(
                                div()
                                    .text_size(px(8.5))
                                    .font_family(self.ui_font())
                                    .text_color(rgb(hex(&t.muted)))
                                    .child("快速切换主题"),
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
                    )),
            )
            .child(
                div()
                    .px_3()
                    .pt_1p5()
                    .text_size(px(9.))
                    .font_family(self.ui_font())
                    .line_height(relative(1.5))
                    .text_color(rgb(hex(&t.muted)).opacity(0.84))
                    .child("一盏灯，一本书 · 按自己的步子来"),
            );
        let page = if !self.store.state.privacy_seen {
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
                                .child("日记与划线尚未加密，你可以随时删除。心情提交会将本次文字与所选心情发送给 OpenAI 生成回复，可能产生 API 费用；成功后记入本机日记。阅读选文回响仍为本地精选示例。"),
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
        } else if let Some(confirm) = &self.confirm {
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
                this.confirm = None;
                this.act(Action::CloseEcho, window, cx);
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
        root = root
            .child(
                img("scene-night.png")
                    .absolute()
                    .size(px(1.))
                    .opacity(0.001),
            )
            .child(
                img("scene-prism.png")
                    .absolute()
                    .size(px(1.))
                    .opacity(0.001),
            );
        if matches!(t.id.as_str(), "forest" | "night" | "prism") {
            root = root.child(
                img(self.scene_asset())
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .opacity(if self.page == Page::Reader {
                        0.03
                    } else if is_prism {
                        0.16
                    } else if dark {
                        0.10
                    } else {
                        0.07
                    }),
            );
        }
        // Ambient GPU optical stage: parallax aurora, dot-matrix, and mouse-following chromatic spotlight
        root = root.child(prism::ambient_stage_canvas(
            snap.clone(),
            self.page == Page::Reader,
        ));
        if !self.focus_mode || self.page != Page::Reader {
            root = root.child(sidebar.relative());
        }
        root.child(main.relative())
            .child(prism::cursor_fx_overlay_canvas(snap))
    }
}
impl Drop for ReadingApp {
    fn drop(&mut self) {
        let _ = self.store.save();
    }
}

pub fn bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("escape", Escape, Some("GlowReading")),
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
}

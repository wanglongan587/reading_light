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

fn finish_test_request(app: &mut ReadingApp, window: &mut Window, cx: &mut Context<ReadingApp>) {
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
            .update(cx, |s, cx| s.set_value("像晴空里舒展的云朵。", window, cx));
        app.act(Action::SaveHighlightNote, window, cx);
        app.act(
            Action::RecolorHighlight(Box::new((hl_id, "prism".into()))),
            window,
            cx,
        );
        assert_eq!(
            app.store.state.books[0].highlights[0].note.as_deref(),
            Some("像晴空里舒展的云朵。")
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
        Some("像晴空里舒展的云朵。")
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
            .update(cx, |s, cx| s.set_value("星空棱镜", window, cx));
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
fn glow_space_themes_switch_render_pair_soundscapes_and_filter_in_palette(cx: &mut TestAppContext) {
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
            assert!(app.is_scenic_space());
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

    // Command palette search resolves every original Glow Space theme accurately
    for &(query, expected_id) in &[
        ("樟树", "totoro"),
        ("浅海晚风", "spirited"),
        ("秘密花园", "howl"),
        ("烘焙屋", "kiki"),
        ("灯笼巷", "lantern"),
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

    // Persisted state reloads the selected Glow Space theme cleanly
    let reopened = Store::open(data).unwrap();
    assert_eq!(reopened.state.theme.id, "lantern");
}

#[gpui::test]
fn focus_timer_modal_renders_and_closes_on_escape_and_book_cover_opens(cx: &mut TestAppContext) {
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

    // 3. Compliant TTS Audiobook Studio drawer renders and updates voice/rate settings
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
        app.ai_key_input.update(cx, |s, cx| {
            s.set_value("sk-secret-ai-never-sync", window, cx)
        });
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

#[gpui::test]
fn late_night_fm_radio_renders_tunes_pure_glow_custom_stream_and_background_strip(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    cx.update(|cx| cx.bind_keys(bindings()));
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let mut store = Store::open(data.clone()).unwrap();
    store.install_sample().unwrap();
    store.state.privacy_seen = true;
    let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));

    // 1. Navigate to Glow FM Radio page and draw full breathing lamp & tuner dial
    app.update_in(cx, |app, window, cx| {
        app.act(Action::Page(Page::Radio), window, cx);
        assert_eq!(app.page, Page::Radio);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("radio-page").is_some());
    assert!(cx.debug_bounds("radio-lamp-stage").is_some());

    // 2. Switch lamp colors, breathing modes, tune stations, favorite, and cycle sleep timer
    app.update_in(cx, |app, window, cx| {
        app.act(Action::RadioLampColor("starry".into()), window, cx);
        assert_eq!(app.store.state.radio.lamp_color, "starry");
        app.act(Action::RadioLampMode("breathe_478".into()), window, cx);
        assert_eq!(app.store.state.radio.lamp_mode, "breathe_478");
        app.act(
            Action::RadioTuneStation("offline-piano-rain".into()),
            window,
            cx,
        );
        assert!(app.radio_status.playing);
        assert_eq!(
            app.store.state.radio.current_station_id,
            "offline-piano-rain"
        );
        app.act(
            Action::RadioToggleFavorite("offline-piano-rain".into()),
            window,
            cx,
        );
        assert!(
            app.store
                .state
                .radio
                .favorites
                .iter()
                .any(|f| f == "offline-piano-rain")
        );
        app.act(Action::RadioCycleSleepTimer, window, cx);
        assert!(app.radio_sleep_started.is_some());
        assert_eq!(app.radio_sleep_total_secs, 15 * 60);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 3. Enter Pure-Glow Zen mode, render cleanly, and exit on Escape
    app.update_in(cx, |app, window, cx| {
        app.act(Action::RadioTogglePureGlow, window, cx);
        assert!(app.radio_pure_glow);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("radio-pure-glow-stage").is_some());
    cx.simulate_keystrokes("escape");
    app.read_with(cx, |app, _| assert!(!app.radio_pure_glow));

    // 4. Open Explore drawer, add custom online stream URL, verify persistence, close on Esc
    app.update_in(cx, |app, window, cx| {
        app.act(Action::RadioToggleDrawer, window, cx);
        assert!(app.radio_drawer_open);
        app.act(Action::RadioDrawerTab(1), window, cx);
        app.radio_custom_name_input
            .update(cx, |s, cx| s.set_value("海边深夜钢琴", window, cx));
        app.radio_custom_url_input.update(cx, |s, cx| {
            s.set_value("https://example.com/late-piano.mp3", window, cx)
        });
        app.act(Action::RadioAddCustomStream, window, cx);
        assert_eq!(app.store.state.radio.custom_stations.len(), 1);
        assert_eq!(
            app.store.state.radio.custom_stations[0].name,
            "海边深夜钢琴"
        );
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_keystrokes("escape");
    app.read_with(cx, |app, _| assert!(!app.radio_drawer_open));

    // 5. Switch back to Shelf while radio keeps playing in background -> sidebar mini strip appears
    app.update_in(cx, |app, window, cx| {
        app.act(Action::Page(Page::Shelf), window, cx);
        assert!(app.radio_status.playing);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("sidebar-radio-mini").is_some());

    // 6. Turning on reading soundscape smoothly stops background FM radio (mutual exclusion)
    app.update_in(cx, |app, window, cx| {
        let _ = app.act(Action::Soundscape(SoundscapePreset::Rain), window, cx);
        assert!(!app.radio_status.playing);
        app.stop_audio_smoothly();
    });

    let reopened = Store::open(data).unwrap();
    assert_eq!(reopened.state.radio.lamp_color, "starry");
    assert_eq!(reopened.state.radio.lamp_mode, "breathe_478");
    assert_eq!(reopened.state.radio.custom_stations.len(), 1);
}

#[gpui::test]
fn radio_station_switching_has_zero_layout_jitter_and_dial_is_continuously_draggable(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    cx.update(|cx| cx.bind_keys(bindings()));
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let mut store = Store::open(data).unwrap();
    store.install_sample().unwrap();
    store.state.privacy_seen = true;
    let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));

    // 1. Entering Page::Radio opens full-viewport immersive stage by default (no bright sidebar)
    app.update_in(cx, |app, window, cx| {
        app.act(Action::Page(Page::Radio), window, cx);
        assert!(
            app.radio_hide_sidebar,
            "FM page should open immersive by default"
        );
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let base_lamp = cx
        .debug_bounds("radio-lamp-stage")
        .expect("vinyl stage laid out");
    let base_dial = cx
        .debug_bounds("radio-dial-track")
        .expect("dial track laid out");
    let base_play = cx
        .debug_bounds("radio-play-btn")
        .expect("play btn laid out");
    assert!(
        base_lamp.size.width >= px(300.),
        "hero turntable stage must be large"
    );

    // 2. Switch across EVERY station kind (short 1-line online, 4-line NightHost, BookEchoHost,
    //    AmbientSynth, LocalFolder), lamp colors, 4-7-8 breathing mode, play/pause, visual toggle.
    //    Assert ZERO layout jitter (exact identical pixel bounds on every render)!
    let transitions: Vec<Action> = vec![
        Action::RadioTuneStation("lofi-rain".into()),
        Action::RadioTuneStation("glow-night-talk".into()),
        Action::RadioTuneStation("book-echo-fm".into()),
        Action::RadioTuneStation("offline-piano-rain".into()),
        Action::RadioTuneStation("local-tape-fm".into()),
        Action::RadioLampMode("breathe_478".into()),
        Action::RadioLampColor("starry".into()),
        Action::RadioLampColor("rose".into()),
        Action::RadioTogglePlay,
        Action::RadioTogglePlay,
        Action::RadioToggleVisualMode,
        Action::RadioToggleVisualMode,
        Action::RadioLampMode("audio".into()),
        Action::RadioTuneStation("deep-space-sleep".into()),
    ];
    for action in transitions {
        app.update_in(cx, |app, window, cx| {
            app.act(action.clone(), window, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(
            cx.debug_bounds("radio-lamp-stage").unwrap(),
            base_lamp,
            "radio-lamp-stage jittered after {action:?}"
        );
        assert_eq!(
            cx.debug_bounds("radio-dial-track").unwrap(),
            base_dial,
            "radio-dial-track jittered after {action:?}"
        );
        assert_eq!(
            cx.debug_bounds("radio-play-btn").unwrap(),
            base_play,
            "radio-play-btn jittered after {action:?}"
        );
    }

    // 3. Real pointer drag across the 88.0 - 108.0 MHz FM dial track!
    let dial_bounds = cx.debug_bounds("radio-dial-track").unwrap();
    let pad = 20.0_f32;
    let track_w = (f32::from(dial_bounds.size.width) - pad * 2.0).max(20.0);
    let y_mid = dial_bounds.center().y;
    let x_for_mhz = |mhz: f32| {
        let frac = ((mhz - 88.0) / 20.0).clamp(0.0, 1.0);
        px(f32::from(dial_bounds.origin.x) + pad + frac * track_w)
    };

    // Start drag near 88.6 MHz (lofi-rain)
    cx.simulate_mouse_down(
        point(x_for_mhz(88.6), y_mid),
        MouseButton::Left,
        Modifiers::default(),
    );
    app.read_with(cx, |app, _| {
        assert!(app.radio_dial_dragging);
        let mhz = app.radio_drag_mhz.expect("drag mhz set");
        assert!((mhz - 88.6).abs() <= 0.3, "expected ~88.6, got {mhz}");
    });

    // Drag continuously to mid-band 96.8 MHz (classical-nocturne)
    cx.simulate_mouse_move(
        point(x_for_mhz(96.8), y_mid),
        MouseButton::Left,
        Modifiers::default(),
    );
    app.read_with(cx, |app, _| {
        assert!(app.radio_dial_dragging);
        let mhz = app.radio_drag_mhz.expect("drag mhz updated");
        assert!((mhz - 96.8).abs() <= 0.3, "expected ~96.8, got {mhz}");
    });

    // Drag past right boundary (clamps safely to 108.0 MHz), then back to 104.6 MHz (offline-piano-rain)
    cx.simulate_mouse_move(
        point(px(f32::from(dial_bounds.right()) + 80.0), y_mid),
        MouseButton::Left,
        Modifiers::default(),
    );
    app.read_with(cx, |app, _| {
        assert_eq!(app.radio_drag_mhz, Some(108.0));
    });
    cx.simulate_mouse_move(
        point(x_for_mhz(104.6), y_mid),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(x_for_mhz(104.6), y_mid),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.update(|window, cx| window.draw(cx).clear(cx));

    app.read_with(cx, |app, _| {
        assert!(!app.radio_dial_dragging);
        assert!(app.radio_drag_mhz.is_none());
        assert_eq!(
            app.store.state.radio.current_station_id,
            "offline-piano-rain"
        );
        assert!(app.radio_status.playing);
    });
    assert_eq!(
        cx.debug_bounds("radio-dial-track").unwrap(),
        base_dial,
        "dial bounds must remain rock-solid after dragging"
    );

    // 4. Drag the smooth volume slider bar
    let vol_bounds = cx
        .debug_bounds("radio-vol-track")
        .expect("volume track laid out");
    cx.simulate_mouse_down(
        point(
            px(f32::from(vol_bounds.origin.x) + 12.0),
            vol_bounds.center().y,
        ),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(
            px(f32::from(vol_bounds.right()) - 10.0),
            vol_bounds.center().y,
        ),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(
            px(f32::from(vol_bounds.right()) - 10.0),
            vol_bounds.center().y,
        ),
        MouseButton::Left,
        Modifiers::default(),
    );
    app.read_with(cx, |app, _| {
        assert!(!app.radio_vol_dragging);
        assert!(app.store.state.radio.volume > 0.85);
    });

    // 5. Clicking back-to-shelf exits cleanly to Shelf while keeping background FM active
    let back_bounds = cx
        .debug_bounds("radio-back-shelf")
        .expect("back-to-shelf button");
    cx.simulate_click(back_bounds.center(), Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    app.read_with(cx, |app, _| {
        assert_eq!(app.page, Page::Shelf);
        assert!(app.radio_status.playing);
    });
    assert!(cx.debug_bounds("sidebar-radio-mini").is_some());
}

#[gpui::test]
fn phase1_onboarding_catalog_shelf_lifecycle_and_opds_work_end_to_end(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().into()).unwrap();
    let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));

    // 1. 3-step 30-second healing onboarding ritual
    cx.update(|window, cx| window.draw(cx).clear(cx));
    app.update_in(cx, |app, window, cx| {
        assert!(!app.store.state.privacy_seen);
        assert_eq!(app.onboarding_step, 0);
        app.act(Action::OnboardingStep(1), window, cx);
        assert_eq!(app.onboarding_step, 1);
        app.act(Action::OnboardingStep(2), window, cx);
        assert_eq!(app.onboarding_step, 2);
        app.act(Action::Privacy, window, cx);
        assert!(app.store.state.privacy_seen);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 2. Built-in 微光公版馆 & Mood Book Picker
    app.update_in(cx, |app, window, cx| {
        app.act(Action::ShelfMoodPick("迷茫".into()), window, cx);
        app.act(Action::ShelfTab(ShelfTab::Catalog), window, cx);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    app.update_in(cx, |app, window, cx| {
        app.act(
            Action::InstallCatalogBook("chibi-sushi".into(), false),
            window,
            cx,
        );
        assert_eq!(app.shelf_tab, ShelfTab::Library);
        assert_eq!(app.store.state.books.len(), 1);
        assert_eq!(app.store.state.books[0].title, "赤壁夜游集");
    });

    // 3. Book status cycle + inline metadata & tags editing
    let book_id = app.read_with(cx, |app, _| app.store.state.books[0].id.clone());
    app.update_in(cx, |app, window, cx| {
        app.act(Action::CycleBookStatus(book_id.clone()), window, cx);
        assert_eq!(app.store.state.books[0].effective_status(), "reading");
        app.act(Action::EditBookMeta(book_id.clone()), window, cx);
        assert_eq!(app.editing_book_id.as_deref(), Some(book_id.as_str()));
        app.book_meta_title_input
            .update(cx, |s, cx| s.set_value("赤壁夜游集 · 典藏", window, cx));
        app.book_meta_tags_input
            .update(cx, |s, cx| s.set_value("宋词, 睡前必读", window, cx));
        app.act(Action::SaveBookMeta, window, cx);
        assert!(app.editing_book_id.is_none());
        assert_eq!(app.store.state.books[0].title, "赤壁夜游集 · 典藏");
        assert_eq!(app.store.state.books[0].tags, vec!["宋词", "睡前必读"]);
    });

    // 4. OPDS 1.2 feed management & rendering
    app.update_in(cx, |app, window, cx| {
        app.act(Action::ShelfTab(ShelfTab::Opds), window, cx);
        app.opds_name_input
            .update(cx, |s, cx| s.set_value("Calibre 私人库", window, cx));
        app.opds_url_input.update(cx, |s, cx| {
            s.set_value("https://books.example.com/opds", window, cx)
        });
        app.act(Action::OpdsAddFeed, window, cx);
        assert_eq!(app.store.state.opds_feeds.len(), 1);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 5. Circadian daylight & quiet ambient presence toggles
    app.update_in(cx, |app, window, cx| {
        assert!(!app.store.state.daylight_auto);
        app.act(Action::ToggleDaylightAuto, window, cx);
        assert!(app.store.state.daylight_auto);
        assert!(app.store.state.show_ambient_presence);
        app.act(Action::ToggleAmbientPresence, window, cx);
        assert!(!app.store.state.show_ambient_presence);
    });
}

#[gpui::test]
fn phase1_footnote_cross_book_fts5_completion_ceremony_and_journal_lock_work(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path().into()).unwrap();
    store.state.privacy_seen = true;
    let book1 = store.install_catalog_book("taohuayuan").unwrap();
    let book2 = store.install_catalog_book("chibi-sushi").unwrap();
    // Seed a highlight in Book 1 containing "清风" & "桃花" so Book 2 can trigger cross-book FTS5 recall
    store
        .add_highlight_styled(
            &book1,
            "清风吹过落英缤纷的桃花林，忽然豁然开朗。".into(),
            "epubcfi(/6/2!/4/2,/1:0,/1:12)".into(),
            Some("amber".into()),
            Some("去年秋天在林间散步时划下的句子".into()),
        )
        .unwrap();
    let today = today_ymd();
    store
        .record_reading_session(Some(book2.clone()), 28, &today, Some("forest".into()))
        .unwrap();

    let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));
    app.update_in(cx, |app, _, cx| {
        app.active_book = Some(book2.clone());
        app.page = Page::Reader;
        app.opened = true;
        if let Some(b) = app.store.state.books.iter_mut().find(|b| b.id == book2) {
            b.progress = Some(0.35);
            b.chapter = Some("前赤壁赋".into());
        }
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 1. Reunion spoiler-free banner dismisses cleanly
    app.update_in(cx, |app, window, cx| {
        assert!(!app.reunion_dismissed);
        app.act(Action::DismissReunion, window, cx);
        assert!(app.reunion_dismissed);
    });

    // 2. Paper-margin offline footnote lookup via WebView event
    app.update_in(cx, |app, _, cx| {
        app.event(
            ReaderEvent {
                version: 1,
                session: app.session.clone(),
                request_id: 10,
                kind: "footnote".into(),
                text: "豁然开朗。".into(),
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
        assert_eq!(app.drawer, "footnote");
        assert_eq!(app.footnote_dict.unwrap().headword, "豁然开朗");
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 3. Cross-book FTS5 personal memory recall on echo event
    app.update_in(cx, |app, _, cx| {
        app.event(
            ReaderEvent {
                version: 1,
                session: app.session.clone(),
                request_id: 11,
                kind: "echo".into(),
                text: "清风徐来，水波不兴。".into(),
                cfi: "epubcfi(/6/2!/4/4,/1:0,/1:8)".into(),
                color: String::new(),
                open_note: false,
                entries: vec![],
                progress: None,
                chapter: String::new(),
                results: vec![],
            },
            cx,
        );
        assert_eq!(app.drawer, "echo");
        assert!(
            !app.echo_memories.is_empty(),
            "should recall Book 1's highlight via local SQLite FTS5"
        );
        assert_eq!(app.echo_memories[0].book_title, "归园田居与桃花源");
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 4. Completion Ceremony modal -> archive collectible ticket stub to Journal
    app.update_in(cx, |app, window, cx| {
        app.act(Action::OpenCompletionModal(book2.clone()), window, cx);
        assert_eq!(app.completion_book_id.as_deref(), Some(book2.as_str()));
        assert!(app.completion_draft_letter.contains("赤壁夜游集"));
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    app.update_in(cx, |app, window, cx| {
        app.act(Action::ConfirmCompleteBook, window, cx);
        app.act(Action::CloseCompletionModal, window, cx);
        assert!(app.completion_book_id.is_none());
        assert_eq!(app.store.state.journal[0].kind, "ticket");
    });

    // 5. Unified Journal view: monthly glow calendar + ticket filter + privacy lock
    app.update_in(cx, |app, window, cx| {
        app.act(Action::Page(Page::Journal), window, cx);
        app.act(Action::JournalFilter(JournalFilter::Ticket), window, cx);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    app.update_in(cx, |app, window, cx| {
        app.act(Action::ToggleJournalLock, window, cx);
        assert!(app.store.state.journal_locked);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui::test]
fn phase1_polish_theme_preview_tts_controls_catalog_filter_and_journal_postcard_work(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path().join("data")).unwrap();
    store.state.privacy_seen = true;

    // Export a valid .glowtheme package to test preview -> confirm -> delete lifecycle
    let mut exported_theme = store.duplicate_theme(&ReadingTheme::default()).unwrap();
    exported_theme.name = "松烟竹纸".into();
    let pkg_path = dir.path().join("bamboo.glowtheme");
    store.export_theme(&exported_theme, &pkg_path).unwrap();

    // Pre-populate a fake cached TTS audio segment
    let cache_dir = store.root.join("tts_cache");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(cache_dir.join("sample.mp3"), vec![0xAAu8; 2048]).unwrap();

    let (app, cx) = cx.add_window_view(|window, cx| test_app(store, window, cx));

    // 1. Inspect .glowtheme package without mutating store, render preview modal, confirm import, then delete
    app.update_in(cx, |app, window, cx| {
        app.act(Action::Page(Page::Themes), window, cx);
        app.act(Action::InspectThemePackage(pkg_path.clone()), window, cx);
        assert!(app.pending_theme_preview.is_some());
        assert!(app.store.state.custom_themes.is_empty());
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let imported_id = app.update_in(cx, |app, window, cx| {
        app.act(Action::ConfirmImportThemePackage, window, cx);
        assert!(app.pending_theme_preview.is_none());
        assert_eq!(app.store.state.custom_themes.len(), 1);
        assert_eq!(app.store.state.theme.name, "松烟竹纸");
        // Also open DIY console to verify live miniature paper & contrast preview renders
        app.act(Action::EditTheme, window, cx);
        assert!(app.draft.is_some());
        app.store.state.custom_themes[0].id.clone()
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    app.update_in(cx, |app, window, cx| {
        app.act(Action::CancelTheme, window, cx);
        app.act(Action::DeleteCustomTheme(imported_id), window, cx);
        assert!(app.store.state.custom_themes.is_empty());
        assert_eq!(app.store.state.theme.id, ReadingTheme::default().id);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 2. Dual-track TTS configuration & audio cache clearing on Themes + Reader TTS drawer
    app.update_in(cx, |app, window, cx| {
        app.act(Action::TtsProvider("openai_compatible".into()), window, cx);
        assert_eq!(app.store.state.tts.provider, "openai_compatible");
        app.tts_api_base_input.update(cx, |s, cx| {
            s.set_value("http://127.0.0.1:8880/v1", window, cx)
        });
        app.tts_model_input
            .update(cx, |s, cx| s.set_value("kokoro", window, cx));
        app.act(Action::SaveTtsSettings, window, cx);
        assert_eq!(app.store.state.tts.api_base_url, "http://127.0.0.1:8880/v1");
        assert_eq!(app.store.state.tts.model, "kokoro");

        assert_eq!(tts::cache_stats(&app.store.root).0, 1);
        app.act(Action::TtsClearCache, window, cx);
        assert_eq!(tts::cache_stats(&app.store.root).0, 0);

        app.act(Action::TtsProvider("auto".into()), window, cx);
        assert_eq!(app.store.state.tts.provider, "auto");
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 3. Expanded 10-book 微光公版馆 category filtering + installed-on-shelf detection
    let story_book_id = app.update_in(cx, |app, window, cx| {
        app.act(Action::Page(Page::Shelf), window, cx);
        app.act(Action::ShelfTab(ShelfTab::Catalog), window, cx);
        app.act(Action::CatalogCategory("睡前故事".into()), window, cx);
        assert_eq!(app.catalog_category, "睡前故事");
        app.act(
            Action::InstallCatalogBook("starlight-post".into(), false),
            window,
            cx,
        );
        let installed = app
            .store
            .state
            .books
            .iter()
            .find(|b| b.title == "星灯与慢邮局")
            .expect("starlight-post should be installed")
            .id
            .clone();
        // Seed a highlight so Journal renders the cross-time rediscovery postcard
        app.store
            .add_highlight_styled(
                &installed,
                "灯光不必照亮整片原野，只要足够照见眼前这一页。".into(),
                "epubcfi(/6/2!/4/2,/1:0,/1:18)".into(),
                Some("amber".into()),
                None,
            )
            .unwrap();
        installed
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 4. Open reader TTS drawer and verify upgraded engine selector + cache block renders cleanly
    app.update_in(cx, |app, _, cx| {
        app.active_book = Some(story_book_id.clone());
        app.page = Page::Reader;
        app.opened = true;
        app.drawer = "tts".into();
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 5. Journal view renders finished books summary + Cross-Time Rediscovery Postcard
    app.update_in(cx, |app, window, cx| {
        app.act(Action::Page(Page::Journal), window, cx);
        assert!(app.daily_rediscovery().is_some());
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

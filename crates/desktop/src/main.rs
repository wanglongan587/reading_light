#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
mod app;
mod mood;
mod platform;
mod prism;
#[cfg(all(test, feature = "ui-test"))]
extern crate gpui_kit as gpui;
use gpui_kit::*;

fn main() {
    if let Err(error) = run() {
        eprintln!("微光阅读启动失败：{error:#}");
        if !std::env::args().any(|a| a == "--smoke-test" || a == "--check-mood") {
            rfd::MessageDialog::new()
                .set_title("微光阅读")
                .set_description(format!("启动失败：{error:#}"))
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
        std::process::exit(1);
    }
}
fn run() -> anyhow::Result<()> {
    if std::env::args().any(|arg| arg == "--check-mood") {
        let config = mood::Config::load()?;
        for text in [
            "今天终于升职了，想和家人庆祝，但又怕新岗位做不好。",
            "我养了十年的猫去世了，回家看到空猫窝就忍不住哭。",
        ] {
            let result = config.generate(text, "疲惫")?;
            println!("INPUT: {text}\nREPLY: {}\n", result.reply);
        }
        return Ok(());
    }
    let root = glow_core::Store::default_root()?;
    std::fs::create_dir_all(&root)?;
    let instance_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("instance.lock"))?;
    instance_lock
        .try_lock()
        .map_err(|_| anyhow::anyhow!("这个书库已由另一个微光阅读窗口打开，请先关闭该窗口。"))?;
    let mut store = glow_core::Store::open(root)?;
    store.install_sample()?;
    #[cfg(target_os = "windows")]
    unsafe {
        std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "true");
    }
    #[cfg(target_os = "linux")]
    let application = gpui_kit::platform::linux(WindowingModes::X11);
    #[cfg(not(target_os = "linux"))]
    let application = gpui_kit::application();
    application.with_assets(platform::Assets).run(move |cx| {
        gpui_kit::init(cx);
        cx.bind_keys(app::bindings());
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.), px(820.)),
                cx,
            ))),
            window_min_size: Some(size(px(1024.), px(720.))),
            titlebar: Some(TitlebarOptions {
                title: Some("微光阅读".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        if let Err(e) = gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| app::ReadingApp::new(store, window, cx))
        }) {
            eprintln!("无法创建窗口：{e}");
            cx.quit();
        }
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
    Ok(())
}

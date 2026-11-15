use gpui_kit::*;
use gpui_webview::WebView;
use std::{
    borrow::Cow,
    path::PathBuf,
    sync::{Arc, mpsc::SyncSender},
};

gpui_kit::assets::icon_assets!(
    ExtraIcons,
    [
        AArrowDown,
        ArrowUpRight,
        AudioLines,
        CircleAlert,
        Columns2,
        Eye,
        Feather,
        Focus,
        Image,
        Leaf,
        LibraryBig,
        ListTree,
        MessageCircleHeart,
        Minimize2,
        NotebookPen,
        ScrollText,
        Settings2,
        ShieldCheck,
        Sparkles,
        VolumeX,
        X
    ]
);

pub fn origin() -> &'static str {
    if cfg!(target_os = "windows") {
        "http://glow.localhost"
    } else {
        "glow://localhost"
    }
}

pub fn create_reader(
    window: &mut Window,
    cx: &mut App,
    session: String,
    book: Vec<u8>,
    root: PathBuf,
    tx: SyncSender<String>,
) -> anyhow::Result<Entity<WebView>> {
    let book = Arc::new(book);
    let url = format!("{}/index.html?session={session}", origin());
    let expected = url.clone();
    let ipc_url = url.clone();
    let webview_data = root.join("webview");
    std::fs::create_dir_all(&webview_data)?;
    let mut web_context = wry::WebContext::new(Some(webview_data));
    let builder = wry::WebViewBuilder::new_with_web_context(&mut web_context)
        .with_custom_protocol("glow".into(), move |_, request| {
            let path = request.uri().path().trim_start_matches('/');
            let (mime, data): (&str, Option<Vec<u8>>) = match path {
                "index.html" => (
                    "text/html; charset=utf-8",
                    Some(include_bytes!("../../../web/dist/index.html").to_vec()),
                ),
                "reader.js" => (
                    "text/javascript; charset=utf-8",
                    Some(include_bytes!("../../../web/dist/reader.js").to_vec()),
                ),
                "reader.css" => (
                    "text/css; charset=utf-8",
                    Some(include_bytes!("../../../web/dist/reader.css").to_vec()),
                ),
                "book.epub" => ("application/epub+zip", Some(book.as_ref().clone())),
                _ if path.starts_with("theme/") && glow_core::library::safe_path(path) => {
                    let local = root.join("themes").join(&path[6..]);
                    let allowed = local
                        .canonicalize()
                        .ok()
                        .zip(root.join("themes").canonicalize().ok())
                        .is_some_and(|(p, r)| p.starts_with(r));
                    let ext = local.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let mime = match ext {
                        "ttf" => "font/ttf",
                        "otf" => "font/otf",
                        _ => "application/octet-stream",
                    };
                    (
                        mime,
                        if allowed && ["ttf", "otf"].contains(&ext) {
                            std::fs::read(local).ok()
                        } else {
                            None
                        },
                    )
                }
                _ => ("text/plain", None),
            };
            wry::http::Response::builder()
                .status(if data.is_some() { 200 } else { 404 })
                .header("Content-Type", mime)
                .header("Access-Control-Allow-Origin", origin())
                .header("Cache-Control", "no-store")
                .header("X-Content-Type-Options", "nosniff")
                .body(Cow::Owned(data.unwrap_or_default()))
                .expect("constant headers")
        })
        .with_ipc_handler(move |request| {
            if request.uri().to_string() == ipc_url && request.body().len() <= 128 * 1024 {
                let _ = tx.try_send(request.body().clone());
            }
        })
        .with_navigation_handler(move |next| next == expected || next.starts_with("blob:"))
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_url(url);
    let mut outcome = None;
    let entity = cx.new(|cx| {
        let result = WebView::build(builder, window, cx);
        match result {
            Ok(view) => Some(view),
            Err(e) => {
                outcome = Some(e);
                None
            }
        }
    });
    // Construct the public Entity only after initialization has succeeded.
    let view = entity.update(cx, |slot, _| slot.take());
    match view {
        Some(view) => Ok(cx.new(|_| view)),
        None => Err(anyhow::anyhow!(
            "阅读组件启动失败：{}。Windows 请确认已安装 Microsoft Edge WebView2 Runtime。",
            outcome.map(|e| e.to_string()).unwrap_or_default()
        )),
    }
}

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        match path {
            "scene.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/scene.svg"
            )))),
            "scene-night.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/scene-night.svg"
            )))),
            "scene.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/scene.png"
            )))),
            "scene-night.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/scene-night.png"
            )))),
            "glow-icon.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/glow-icon.png"
            )))),
            _ => {
                if let Some(bytes) = ExtraIcons.load(path)? {
                    return Ok(Some(bytes));
                }
                gpui_kit::assets::Assets.load(path)
            }
        }
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

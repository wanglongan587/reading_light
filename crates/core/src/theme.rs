use crate::{
    Store, Theme,
    library::{pack, safe_path, unpack},
    store::atomic_write,
};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, fs, path::Path};

fn valid_image(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || (bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
}

fn font_family(bytes: &[u8]) -> Result<String> {
    let face = ttf_parser::Face::parse(bytes, 0)
        .map_err(|_| anyhow::anyhow!("无法读取字体，请选择有效 TTF 或 OTF"))?;
    face.names()
        .into_iter()
        .find(|name| name.name_id == ttf_parser::name_id::FAMILY && name.is_unicode())
        .and_then(|name| name.to_string())
        .context("字体缺少家族名称")
}

fn valid_audio(bytes: &[u8]) -> bool {
    (bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE")
        || bytes.starts_with(b"OggS")
        || bytes.starts_with(b"ID3")
        || (bytes.len() >= 2 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0)
}

pub fn color(value: &str) -> Option<u32> {
    if value.len() != 7 || !value.starts_with('#') {
        return None;
    }
    u32::from_str_radix(&value[1..], 16).ok()
}
pub fn contrast(a: &str, b: &str) -> f32 {
    fn luminance(c: u32) -> f32 {
        let channel = |v: u32| {
            let f = v as f32 / 255.;
            if f <= 0.04045 {
                f / 12.92
            } else {
                ((f + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel((c >> 16) & 255)
            + 0.7152 * channel((c >> 8) & 255)
            + 0.0722 * channel(c & 255)
    }
    let a = luminance(color(a).unwrap_or(0));
    let b = luminance(color(b).unwrap_or(0));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
pub fn validate(theme: &Theme) -> Result<()> {
    ensure!(theme.version == 1, "不支持的主题包版本");
    ensure!(
        !theme.name.trim().is_empty() && theme.name.len() <= 120,
        "主题名称需为 1—40 个汉字"
    );
    ensure!(
        !theme.id.is_empty()
            && theme.id.len() <= 80
            && theme
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "主题标识无效"
    );
    for c in [
        &theme.background,
        &theme.paper,
        &theme.text,
        &theme.muted,
        &theme.accent,
    ] {
        ensure!(color(c).is_some(), "颜色必须使用 #RRGGBB 格式");
    }
    ensure!(
        (14.0..=36.0).contains(&theme.font_size)
            && (1.3..=2.6).contains(&theme.line_height)
            && (400..=1100).contains(&theme.width),
        "阅读排版参数超出范围"
    );
    ensure!(
        (0.0..=1.0).contains(&theme.overlay) && (0.0..=1.0).contains(&theme.volume),
        "遮罩或音量超出范围"
    );
    ensure!(
        theme.font.len() <= 300 && !theme.font.contains([';', '{', '}', '\n', '"', '\\']),
        "字体名称无效"
    );
    for (asset, extensions) in [
        (&theme.image, &["png", "jpg", "jpeg", "webp"][..]),
        (&theme.font_asset, &["ttf", "otf"][..]),
        (&theme.audio, &["wav", "ogg", "mp3"][..]),
    ] {
        if let Some(name) = asset {
            ensure!(safe_path(name), "主题素材路径无效");
            ensure!(
                extensions.contains(
                    &Path::new(name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                ),
                "不支持的素材类型"
            );
        }
    }
    ensure!(
        contrast(&theme.text, &theme.paper) >= 4.5,
        "正文对比度不足 4.5:1，请调整文字或纸面颜色"
    );
    Ok(())
}

impl Store {
    pub fn save_theme(&mut self, theme: Theme) -> Result<()> {
        validate(&theme)?;
        self.state.custom_themes.retain(|t| t.id != theme.id);
        self.state.custom_themes.push(theme.clone());
        self.state.theme = theme;
        self.save()
    }
    pub fn import_theme(&mut self, path: &Path) -> Result<Theme> {
        ensure!(
            fs::metadata(path)?.len() <= crate::library::MAX_ARCHIVE,
            "主题包过大"
        );
        let files = unpack(&fs::read(path)?)?;
        let mut theme: Theme =
            serde_json::from_slice(files.get("theme.json").context("主题包缺少 theme.json")?)?;
        validate(&theme)?;
        for name in [&theme.image, &theme.font_asset, &theme.audio]
            .into_iter()
            .flatten()
        {
            ensure!(files.contains_key(name), "主题包缺少引用的素材");
        }
        if let Some(name) = &theme.image {
            ensure!(valid_image(&files[name]), "主题背景图片已损坏或格式不符");
        }
        if let Some(name) = &theme.font_asset {
            theme.font = font_family(&files[name])?;
        }
        if let Some(name) = &theme.audio {
            ensure!(valid_audio(&files[name]), "主题环境音已损坏或格式不符");
        }
        for name in files.keys() {
            if name != "theme.json" {
                ensure!(
                    [&theme.image, &theme.font_asset, &theme.audio]
                        .into_iter()
                        .flatten()
                        .any(|n| n == name),
                    "主题包包含未声明文件"
                );
            }
        }
        theme.id = format!("custom-{}", uuid::Uuid::new_v4());
        let dir = self.root.join("themes").join(&theme.id);
        for (name, bytes) in &files {
            atomic_write(&dir.join(name), bytes)?;
        }
        self.save_theme(theme.clone())?;
        Ok(theme)
    }
    pub fn export_theme(&self, theme: &Theme, path: &Path) -> Result<()> {
        validate(theme)?;
        let mut files = BTreeMap::new();
        files.insert("theme.json".into(), serde_json::to_vec_pretty(theme)?);
        for name in [&theme.image, &theme.font_asset, &theme.audio]
            .into_iter()
            .flatten()
        {
            files.insert(
                name.clone(),
                fs::read(self.root.join("themes").join(&theme.id).join(name))?,
            );
        }
        atomic_write(path, &pack(&files)?)
    }
    pub fn copy_theme_asset(&self, theme: &mut Theme, kind: &str, path: &Path) -> Result<()> {
        ensure!(
            fs::metadata(path)?.len() <= 25 * 1024 * 1024,
            "单个素材不能超过 25 MB"
        );
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let name = format!("{kind}.{ext}");
        let mut next = theme.clone();
        match kind {
            "background" => {
                ensure!(valid_image(&fs::read(path)?), "背景图片已损坏或格式不符");
                next.image = Some(name.clone());
            }
            "font" => {
                let bytes = fs::read(path)?;
                next.font = font_family(&bytes)?;
                next.font_asset = Some(name.clone());
            }
            "audio" => {
                ensure!(valid_audio(&fs::read(path)?), "环境音已损坏或格式不符");
                next.audio = Some(name.clone());
            }
            _ => anyhow::bail!("未知素材类型"),
        }
        validate(&next)?;
        atomic_write(
            &self.root.join("themes").join(&theme.id).join(&name),
            &fs::read(path)?,
        )?;
        *theme = next;
        Ok(())
    }
    pub fn duplicate_theme(&self, original: &Theme) -> Result<Theme> {
        let mut theme = original.clone();
        theme.id = format!("custom-{}", uuid::Uuid::new_v4());
        for asset in [&theme.image, &theme.font_asset, &theme.audio]
            .into_iter()
            .flatten()
        {
            let bytes = fs::read(self.root.join("themes").join(&original.id).join(asset))?;
            atomic_write(
                &self.root.join("themes").join(&theme.id).join(asset),
                &bytes,
            )?;
        }
        Ok(theme)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_readable() {
        let all = crate::model::themes();
        let ids: Vec<&str> = all.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "forest", "paper", "night", "prism", "totoro", "spirited", "howl", "kiki",
                "lantern"
            ]
        );
        for theme in all {
            validate(&theme).unwrap();
            assert!(
                contrast(&theme.text, &theme.paper) >= 7.0,
                "{} text contrast should be comfortable for long reading",
                theme.id
            );
            assert!(
                contrast(&theme.accent, &theme.paper) >= 4.5,
                "{} accent button contrast should pass WCAG AA",
                theme.id
            );
        }
    }
    #[test]
    fn package_roundtrip_and_rejection() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::open(dir.path().join("data")).unwrap();
        let p = dir.path().join("a.glowtheme");
        let t = Theme::default();
        s.export_theme(&t, &p).unwrap();
        let got = s.import_theme(&p).unwrap();
        assert_eq!(got.name, t.name);
        assert_eq!(got.paper, t.paper);
        let before = s.state.theme.clone();
        fs::write(&p, b"broken").unwrap();
        assert!(s.import_theme(&p).is_err());
        assert_eq!(s.state.theme, before);
        let mut t = Theme {
            image: Some("../x.png".into()),
            ..Theme::default()
        };
        assert!(validate(&t).is_err());
        t.image = None;
        t.text = t.paper.clone();
        assert!(validate(&t).is_err());
    }

    #[test]
    fn packaged_background_asset_round_trips_and_bad_assets_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("data")).unwrap();
        let mut theme = store.duplicate_theme(&Theme::default()).unwrap();
        let image = dir.path().join("background.png");
        fs::write(&image, include_bytes!("../../../assets/scene.png")).unwrap();
        store
            .copy_theme_asset(&mut theme, "background", &image)
            .unwrap();
        let package = dir.path().join("scene.glowtheme");
        store.export_theme(&theme, &package).unwrap();
        let imported = store.import_theme(&package).unwrap();
        assert!(imported.image.is_some());
        assert!(
            store
                .root
                .join("themes")
                .join(imported.id)
                .join(imported.image.unwrap())
                .exists()
        );

        let broken = dir.path().join("background.png");
        fs::write(&broken, b"not an image").unwrap();
        assert!(
            store
                .copy_theme_asset(&mut theme, "background", &broken)
                .is_err()
        );
        let broken_font = dir.path().join("font.ttf");
        fs::write(&broken_font, b"not a font").unwrap();
        assert!(
            store
                .copy_theme_asset(&mut theme, "font", &broken_font)
                .is_err()
        );
    }
}

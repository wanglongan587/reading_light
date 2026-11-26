use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Location {
    pub book_id: String,
    pub cfi: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: String,
    pub format: String,
    pub imported_at: String,
    pub cover: Option<String>,
    pub location: Option<Location>,
    pub bookmarks: Vec<Bookmark>,
    #[serde(default)]
    pub highlights: Vec<Highlight>,
    #[serde(default)]
    pub progress: Option<f32>,
    #[serde(default)]
    pub chapter: Option<String>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub companion: Option<BookCompanionCache>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CharacterNode {
    pub name: String,
    pub aliases: Vec<String>,
    pub role: String,
    pub traits: Vec<String>,
    pub status: String,
    pub first_seen: String,
}

impl Default for CharacterNode {
    fn default() -> Self {
        Self {
            name: String::new(),
            aliases: vec![],
            role: String::new(),
            traits: vec![],
            status: String::new(),
            first_seen: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CharacterRelation {
    pub from: String,
    pub to: String,
    pub label: String,
    /// One of: "bond" | "family" | "conflict" | "mystery"
    pub tone: String,
}

impl Default for CharacterRelation {
    fn default() -> Self {
        Self {
            from: String::new(),
            to: String::new(),
            label: String::new(),
            tone: "bond".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BookCompanionCache {
    pub generated_at: String,
    pub progress_percent: u8,
    pub cutoff_chapter: String,
    pub skipped_future_chapters: usize,
    pub is_ai: bool,
    pub headline: String,
    pub story_summary: String,
    pub open_threads: Vec<String>,
    pub resume_moment: String,
    pub characters: Vec<CharacterNode>,
    pub relations: Vec<CharacterRelation>,
}

impl Default for BookCompanionCache {
    fn default() -> Self {
        Self {
            generated_at: String::new(),
            progress_percent: 0,
            cutoff_chapter: String::new(),
            skipped_future_chapters: 0,
            is_ai: false,
            headline: String::new(),
            story_summary: String::new(),
            open_threads: vec![],
            resume_moment: String::new(),
            characters: vec![],
            relations: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiSettings {
    /// "openai" | "deepseek" | "qwen" | "siliconflow" | "ollama" | "lmstudio" | "llamacpp" | "custom"
    pub provider: String,
    pub base_url: String,
    pub model: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4.1-mini-2025-04-14".into(),
        }
    }
}

impl AiSettings {
    pub fn for_preset(provider: &str) -> Self {
        match provider {
            "deepseek" => Self {
                provider: "deepseek".into(),
                base_url: "https://api.deepseek.com/v1".into(),
                model: "deepseek-chat".into(),
            },
            "qwen" => Self {
                provider: "qwen".into(),
                base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
                model: "qwen-plus".into(),
            },
            "siliconflow" => Self {
                provider: "siliconflow".into(),
                base_url: "https://api.siliconflow.cn/v1".into(),
                model: "Qwen/Qwen2.5-7B-Instruct".into(),
            },
            "ollama" => Self {
                provider: "ollama".into(),
                base_url: "http://127.0.0.1:11434/v1".into(),
                model: "qwen2.5:7b".into(),
            },
            "lmstudio" => Self {
                provider: "lmstudio".into(),
                base_url: "http://127.0.0.1:1234/v1".into(),
                model: "local-model".into(),
            },
            "llamacpp" => Self {
                provider: "llamacpp".into(),
                base_url: "http://127.0.0.1:8080/v1".into(),
                model: "local-model".into(),
            },
            "custom" => Self {
                provider: "custom".into(),
                base_url: "https://api.openai.com/v1".into(),
                model: "gpt-4o-mini".into(),
            },
            _ => Self::default(),
        }
    }

    pub fn is_local(&self) -> bool {
        if matches!(self.provider.as_str(), "ollama" | "lmstudio" | "llamacpp") {
            return true;
        }
        let lower = self.base_url.trim().to_ascii_lowercase();
        let host = lower
            .strip_prefix("http://")
            .or_else(|| lower.strip_prefix("https://"))
            .unwrap_or(&lower)
            .split(['/', '?'])
            .next()
            .unwrap_or("");
        let host_no_port = if host.starts_with('[') {
            host.split(']').next().unwrap_or(host).trim_start_matches('[')
        } else {
            host.split(':').next().unwrap_or(host)
        };
        matches!(host_no_port, "localhost" | "127.0.0.1" | "::1" | "0.0.0.0")
            || host_no_port.starts_with("192.168.")
            || host_no_port.starts_with("10.")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SoundscapeMix {
    pub rain: f32,
    pub fireplace: f32,
    pub breeze: f32,
    pub cosmos: f32,
    pub train: f32,
    pub piano: f32,
    pub recipe: Option<String>,
}

impl Default for SoundscapeMix {
    fn default() -> Self {
        Self {
            rain: 0.72,
            fireplace: 0.0,
            breeze: 0.0,
            cosmos: 0.0,
            train: 0.0,
            piano: 0.0,
            recipe: None,
        }
    }
}

impl SoundscapeMix {
    pub fn clamped(&self) -> Self {
        Self {
            rain: self.rain.clamp(0.0, 1.0),
            fireplace: self.fireplace.clamp(0.0, 1.0),
            breeze: self.breeze.clamp(0.0, 1.0),
            cosmos: self.cosmos.clamp(0.0, 1.0),
            train: self.train.clamp(0.0, 1.0),
            piano: self.piano.clamp(0.0, 1.0),
            recipe: self.recipe.clone(),
        }
    }

    pub fn is_silent(&self) -> bool {
        self.rain < 0.01
            && self.fireplace < 0.01
            && self.breeze < 0.01
            && self.cosmos < 0.01
            && self.train < 0.01
            && self.piano < 0.01
    }

    pub fn active_count(&self) -> usize {
        [
            self.rain,
            self.fireplace,
            self.breeze,
            self.cosmos,
            self.train,
            self.piano,
        ]
        .into_iter()
        .filter(|&v| v >= 0.01)
        .count()
    }

    pub fn from_single(preset: &str) -> Self {
        let mut mix = Self {
            rain: 0.0,
            fireplace: 0.0,
            breeze: 0.0,
            cosmos: 0.0,
            train: 0.0,
            piano: 0.0,
            recipe: None,
        };
        match preset {
            "fireplace" => mix.fireplace = 0.80,
            "breeze" => mix.breeze = 0.80,
            "cosmos" => mix.cosmos = 0.80,
            "train" => mix.train = 0.80,
            "piano" => mix.piano = 0.78,
            _ => mix.rain = 0.80,
        }
        mix
    }

    pub fn from_recipe(id: &str) -> Self {
        match id {
            "rain_hearth" => Self {
                rain: 0.65,
                fireplace: 0.55,
                breeze: 0.0,
                cosmos: 0.0,
                train: 0.0,
                piano: 0.30,
                recipe: Some("rain_hearth".into()),
            },
            "sea_train" => Self {
                rain: 0.0,
                fireplace: 0.0,
                breeze: 0.35,
                cosmos: 0.0,
                train: 0.68,
                piano: 0.42,
                recipe: Some("sea_train".into()),
            },
            "camphor_rain" => Self {
                rain: 0.55,
                fireplace: 0.0,
                breeze: 0.60,
                cosmos: 0.0,
                train: 0.0,
                piano: 0.22,
                recipe: Some("camphor_rain".into()),
            },
            "midnight_study" => Self {
                rain: 0.32,
                fireplace: 0.42,
                breeze: 0.0,
                cosmos: 0.52,
                train: 0.0,
                piano: 0.0,
                recipe: Some("midnight_study".into()),
            },
            "music_box" => Self {
                rain: 0.0,
                fireplace: 0.0,
                breeze: 0.40,
                cosmos: 0.25,
                train: 0.0,
                piano: 0.76,
                recipe: Some("music_box".into()),
            },
            _ => Self::default(),
        }
    }

    pub fn summary_label(&self) -> String {
        let mut names = Vec::new();
        if self.rain >= 0.01 {
            names.push("细雨");
        }
        if self.fireplace >= 0.01 {
            names.push("壁炉");
        }
        if self.breeze >= 0.01 {
            names.push("微风");
        }
        if self.cosmos >= 0.01 {
            names.push("星穹");
        }
        if self.train >= 0.01 {
            names.push("海轨");
        }
        if self.piano >= 0.01 {
            names.push("琴韵");
        }
        if names.is_empty() {
            "静音配方".into()
        } else {
            names.join(" + ")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TtsSettings {
    pub voice: String,
    pub rate: f32,
    pub duck_soundscape: bool,
    pub sleep_with_timer: bool,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            voice: "zh-CN-XiaoxiaoNeural".into(),
            rate: 1.0,
            duck_soundscape: true,
            sleep_with_timer: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SyncSettings {
    /// "off" | "github" | "folder"
    pub mode: String,
    pub github_owner: String,
    pub github_repo: String,
    pub github_branch: String,
    pub github_api_base: String,
    pub folder_path: String,
    pub sync_books: bool,
    pub auto_sync_on_exit: bool,
    pub last_sync_at: Option<String>,
    pub last_sync_summary: Option<String>,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            mode: "off".into(),
            github_owner: String::new(),
            github_repo: "glow-reading-sync".into(),
            github_branch: "main".into(),
            github_api_base: "https://api.github.com".into(),
            folder_path: String::new(),
            sync_books: false,
            auto_sync_on_exit: false,
            last_sync_at: None,
            last_sync_summary: None,
        }
    }
}

/// Stored ONLY in local `secrets.json` — never written to `state.json` or sync snapshots!
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LocalSecrets {
    pub ai_api_key: String,
    pub github_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub label: String,
    pub location: Location,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Highlight {
    pub id: String,
    pub text: String,
    pub cfi: String,
    pub created_at: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

pub fn valid_highlight_color(value: &str) -> bool {
    matches!(value, "amber" | "sage" | "rose" | "prism")
}

pub fn valid_soundscape_preset(value: &str) -> bool {
    matches!(
        value,
        "rain" | "fireplace" | "breeze" | "cosmos" | "train" | "piano"
    )
}

pub fn valid_cfi(value: &str) -> bool {
    value.starts_with("epubcfi(")
        && value.ends_with(')')
        && value.len() <= 4096
        && !value.chars().any(char::is_control)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Theme {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub background: String,
    pub paper: String,
    pub text: String,
    pub muted: String,
    pub accent: String,
    pub font: String,
    pub font_size: f32,
    pub line_height: f32,
    pub width: u32,
    pub overlay: f32,
    pub image: Option<String>,
    pub font_asset: Option<String>,
    pub audio: Option<String>,
    pub volume: f32,
    pub reduced_motion: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            version: 1,
            id: "forest".into(),
            name: "林间暖光".into(),
            background: "#DCEBF8".into(),
            paper: "#F8FCFF".into(),
            text: "#1A324C".into(),
            muted: "#4F6E8E".into(),
            accent: "#2573B8".into(),
            font: "SimSun, Songti SC, Noto Serif CJK SC, serif".into(),
            font_size: 20.0,
            line_height: 1.9,
            width: 720,
            overlay: 0.86,
            image: None,
            font_asset: None,
            audio: None,
            volume: 0.55,
            reduced_motion: false,
        }
    }
}

pub fn themes() -> Vec<Theme> {
    vec![
        Theme::default(),
        Theme {
            id: "paper".into(),
            name: "素纸".into(),
            background: "#ECEAE4".into(),
            paper: "#FFFCF6".into(),
            text: "#2E2B27".into(),
            muted: "#6B645B".into(),
            accent: "#8A5E3B".into(),
            ..Theme::default()
        },
        Theme {
            id: "night".into(),
            name: "静夜".into(),
            background: "#111C1A".into(),
            paper: "#192825".into(),
            text: "#E8EFEA".into(),
            muted: "#95ADA4".into(),
            accent: "#7CD4B0".into(),
            ..Theme::default()
        },
        Theme {
            id: "prism".into(),
            name: "黑曜棱镜".into(),
            background: "#090A0F".into(),
            paper: "#121520".into(),
            text: "#EEF1FA".into(),
            muted: "#8C96B0".into(),
            accent: "#9B87FF".into(),
            ..Theme::default()
        },
        Theme {
            id: "totoro".into(),
            name: "龙猫林荫".into(),
            background: "#E1ECDF".into(),
            paper: "#F9FCF6".into(),
            text: "#1F3626".into(),
            muted: "#4E6B56".into(),
            accent: "#3D7A52".into(),
            ..Theme::default()
        },
        Theme {
            id: "spirited".into(),
            name: "千寻海轨".into(),
            background: "#F3E4DE".into(),
            paper: "#FFF9F5".into(),
            text: "#3B2427".into(),
            muted: "#745458".into(),
            accent: "#B84B42".into(),
            ..Theme::default()
        },
        Theme {
            id: "howl".into(),
            name: "哈尔花原".into(),
            background: "#EAE6F5".into(),
            paper: "#FCFBF7".into(),
            text: "#2B2740".into(),
            muted: "#5F5A7A".into(),
            accent: "#63549E".into(),
            ..Theme::default()
        },
        Theme {
            id: "kiki".into(),
            name: "魔女海镇".into(),
            background: "#F4E9D8".into(),
            paper: "#FFFDF7".into(),
            text: "#3B2A1E".into(),
            muted: "#755D4A".into(),
            accent: "#AE572A".into(),
            ..Theme::default()
        },
        Theme {
            id: "lantern".into(),
            name: "汤屋灯夜".into(),
            background: "#15121D".into(),
            paper: "#201B26".into(),
            text: "#F5E9D6".into(),
            muted: "#BCA895".into(),
            accent: "#E69F52".into(),
            ..Theme::default()
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub id: String,
    pub text: String,
    pub author: String,
    pub work: String,
    pub edition: String,
    pub chapter: String,
    pub source: String,
    pub tags: Vec<String>,
    pub spoiler: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EchoResult {
    pub quotes: Vec<Quote>,
    pub reply: String,
    pub demo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub created_at: String,
    pub kind: String,
    pub input: String,
    pub mood: Option<String>,
    pub echo: EchoResult,
    pub location: Option<Location>,
    pub favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub version: u32,
    pub books: Vec<Book>,
    pub journal: Vec<JournalEntry>,
    pub theme: Theme,
    pub custom_themes: Vec<Theme>,
    pub mode: String,
    pub last_book: Option<String>,
    pub privacy_seen: bool,
    #[serde(default)]
    pub reader_font_size: Option<f32>,
    #[serde(default)]
    pub reader_line_height: Option<f32>,
    #[serde(default)]
    pub reader_width: Option<u32>,
    #[serde(default)]
    pub soundscape_preset: Option<String>,
    #[serde(default)]
    pub warm_filter: bool,
    #[serde(default)]
    pub reading_minutes_today: u32,
    #[serde(default)]
    pub reading_minutes_date: Option<String>,
    #[serde(default)]
    pub ai: AiSettings,
    #[serde(default)]
    pub soundscape_mix: SoundscapeMix,
    #[serde(default)]
    pub tts: TtsSettings,
    #[serde(default)]
    pub sync: SyncSettings,
    #[serde(default)]
    pub deleted_book_ids: Vec<String>,
    #[serde(default)]
    pub deleted_journal_ids: Vec<String>,
    #[serde(default)]
    pub deleted_highlight_ids: Vec<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            books: vec![],
            journal: vec![],
            theme: Theme::default(),
            custom_themes: vec![],
            mode: "scrolled".into(),
            last_book: None,
            privacy_seen: false,
            reader_font_size: None,
            reader_line_height: None,
            reader_width: None,
            soundscape_preset: None,
            warm_filter: false,
            reading_minutes_today: 0,
            reading_minutes_date: None,
            ai: AiSettings::default(),
            soundscape_mix: SoundscapeMix::default(),
            tts: TtsSettings::default(),
            sync: SyncSettings::default(),
            deleted_book_ids: vec![],
            deleted_journal_ids: vec![],
            deleted_highlight_ids: vec![],
        }
    }
}

pub fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

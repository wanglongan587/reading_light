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
    matches!(value, "rain" | "fireplace" | "breeze" | "cosmos")
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
            volume: 0.35,
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
        }
    }
}

pub fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

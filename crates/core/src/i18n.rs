/// Lightweight bilingual localization table (ZH-CN primary + EN-US ready for global launch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    ZhCn,
    EnUs,
}

impl Locale {
    pub fn from_code(code: &str) -> Self {
        if code.trim().to_ascii_lowercase().starts_with("en") {
            Self::EnUs
        } else {
            Self::ZhCn
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::EnUs => "en-US",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ZhCn => "简体中文",
            Self::EnUs => "English",
        }
    }

    pub fn is_english(self) -> bool {
        matches!(self, Self::EnUs)
    }
}

/// Returns the localized UI copy for the given locale.
pub fn pick<'a>(locale: Locale, zh: &'a str, en: &'a str) -> &'a str {
    match locale {
        Locale::ZhCn => zh,
        Locale::EnUs => en,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_locale_codes_and_picks_strings() {
        assert_eq!(Locale::from_code("zh-CN"), Locale::ZhCn);
        assert_eq!(Locale::from_code("en-US"), Locale::EnUs);
        assert_eq!(Locale::from_code("EN"), Locale::EnUs);
        assert_eq!(Locale::from_code("unknown"), Locale::ZhCn);
        assert_eq!(pick(Locale::ZhCn, "书房", "Study"), "书房");
        assert_eq!(pick(Locale::EnUs, "书房", "Study"), "Study");
    }
}

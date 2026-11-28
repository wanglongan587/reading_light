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

pub fn book_status(locale: Locale, status: &str) -> &'static str {
    match (locale, status) {
        (Locale::ZhCn, "want") => "想读",
        (Locale::ZhCn, "reading") => "在读",
        (Locale::ZhCn, "finished") => "已完读",
        (Locale::ZhCn, "paused") => "搁置",
        (Locale::ZhCn, _) => "在读",
        (Locale::EnUs, "want") => "Want",
        (Locale::EnUs, "reading") => "Reading",
        (Locale::EnUs, "finished") => "Finished",
        (Locale::EnUs, "paused") => "Paused",
        (Locale::EnUs, _) => "Reading",
    }
}

pub fn mood_name<'a>(locale: Locale, mood: &'a str) -> &'a str {
    if matches!(locale, Locale::ZhCn) {
        return mood;
    }
    match mood {
        "疲惫" => "Tired",
        "迷茫" => "Lost",
        "平静" => "Calm",
        "难过" => "Heavy",
        "想慢慢来" => "Unhurried",
        "焦虑" => "Anxious",
        "睡前" => "Bedtime",
        "独处" => "Solitude",
        "希望" => "Hope",
        "开心" => "Bright",
        other => other,
    }
}

pub fn catalog_category<'a>(locale: Locale, category: &'a str) -> &'a str {
    if matches!(locale, Locale::ZhCn) {
        return category;
    }
    match category {
        "全部" => "All",
        "睡前故事" => "Bedtime Stories",
        "原创散文" => "Original Essays",
        "古典诗文" => "Classical Prose",
        "双语原典" => "Bilingual Classics",
        "10分钟内" => "Under 10 min",
        other => other,
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
        assert_eq!(book_status(Locale::EnUs, "finished"), "Finished");
        assert_eq!(mood_name(Locale::EnUs, "睡前"), "Bedtime");
        assert_eq!(catalog_category(Locale::EnUs, "10分钟内"), "Under 10 min");
    }
}

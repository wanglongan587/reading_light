use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReaderEvent {
    pub version: u32,
    pub session: String,
    pub request_id: u64,
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub cfi: String,
    #[serde(default)]
    pub entries: Vec<TocEntry>,
    #[serde(default)]
    pub progress: Option<f32>,
    #[serde(default)]
    pub chapter: String,
    #[serde(default)]
    pub results: Vec<SearchHit>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TocEntry {
    pub label: String,
    pub href: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchHit {
    pub cfi: String,
    pub excerpt: String,
    pub chapter: String,
}

pub fn parse_event(input: &str, session: &str) -> Result<ReaderEvent> {
    ensure!(input.len() <= 128 * 1024, "消息过长");
    let event: ReaderEvent = serde_json::from_str(input)?;
    ensure!(
        event.version == 1 && event.session == session,
        "过期的阅读会话"
    );
    ensure!(
        [
            "ready",
            "opened",
            "selection",
            "echo",
            "location",
            "toc",
            "search",
            "error",
            "escape"
        ]
        .contains(&event.kind.as_str()),
        "未知消息类型"
    );
    ensure!(
        event.text.len() <= 16_384 && event.cfi.len() <= 4096,
        "内容过长"
    );
    ensure!(
        event.cfi.is_empty() || crate::model::valid_cfi(&event.cfi),
        "无效阅读位置"
    );
    ensure!(
        event
            .progress
            .is_none_or(|value| (0.0..=1.0).contains(&value)),
        "无效阅读进度"
    );
    ensure!(event.chapter.len() <= 300, "章节名称过长");
    ensure!(event.entries.len() <= 2000, "目录过长");
    for item in &event.entries {
        ensure!(
            item.label.len() <= 1024
                && item.href.len() <= 2048
                && !item.href.contains(':')
                && !item.href.starts_with('/'),
            "无效目录链接"
        );
    }
    ensure!(event.results.len() <= 100, "搜索结果过多");
    for hit in &event.results {
        ensure!(
            crate::model::valid_cfi(&hit.cfi)
                && hit.excerpt.len() <= 1024
                && hit.chapter.len() <= 300,
            "无效搜索结果"
        );
    }
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_and_privileged_messages_are_rejected() {
        assert!(
            parse_event(
                r#"{"version":1,"session":"old","request_id":1,"kind":"ready"}"#,
                "new"
            )
            .is_err()
        );
        assert!(
            parse_event(
                r#"{"version":1,"session":"s","request_id":1,"kind":"delete_file"}"#,
                "s"
            )
            .is_err()
        );
        assert!(parse_event(r#"{"version":1,"session":"s","request_id":1,"kind":"location","cfi":"file:///etc/passwd"}"#, "s").is_err());
        assert!(
            parse_event(
                r#"{"version":1,"session":"s","request_id":1,"kind":"location","cfi":"epubcfi(/6/2)","progress":1.2}"#,
                "s"
            )
            .is_err()
        );
        assert!(
            parse_event(
                r#"{"version":1,"session":"s","request_id":1,"kind":"search","results":[{"cfi":"epubcfi(/6/2!/4/2)","excerpt":"文字","chapter":"第一章"}]}"#,
                "s"
            )
            .is_ok()
        );
        assert!(
            parse_event(
                r#"{"version":1,"session":"s","request_id":1,"kind":"ready"}"#,
                "s"
            )
            .is_ok()
        );
    }
}

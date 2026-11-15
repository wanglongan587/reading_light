use crate::{EchoResult, Quote};

pub const MOODS: &[&str] = &["孤独", "疲惫", "迷茫", "失落", "成长", "希望"];

pub fn quotes() -> Vec<Quote> {
    serde_json::from_str(include_str!("../../../assets/quotes.json"))
        .expect("bundled quote corpus must be validated")
}

pub fn respond(text: &str, mood: Option<&str>) -> EchoResult {
    let categories = [
        ("孤独", &["孤独", "寂寞", "独自", "一个人", "想念"][..]),
        ("疲惫", &["疲惫", "疲倦", "累", "休息", "忙碌"][..]),
        ("迷茫", &["迷茫", "方向", "岔路", "不知道", "困惑"][..]),
        ("失落", &["失落", "悲伤", "难过", "离别", "眼泪"][..]),
        ("成长", &["成长", "勇敢", "坚持", "长大", "改变"][..]),
        ("希望", &["希望", "明天", "阳光", "种子", "出发"][..]),
    ];
    let tag = mood.filter(|m| MOODS.contains(m)).or_else(|| {
        categories
            .iter()
            .find(|(_, words)| words.iter().any(|w| text.contains(w)))
            .map(|(tag, _)| *tag)
    });
    let Some(tag) = tag else {
        return EchoResult {
            quotes: vec![],
            reply: "暂时没有找到合适的回响。这段文字可以先留在这里。".into(),
            demo: true,
        };
    };
    let quotes = quotes()
        .into_iter()
        .filter(|q| q.tags.iter().any(|t| t == tag))
        .take(3)
        .collect();
    let reply = match tag {
        "孤独" => "有些话，一时找不到可以说给谁听。你可以先把它放在这里，不必急着解释清楚。",
        "疲惫" => "今天走到这里，也许已经用了很多力气。先歇一会儿吧，这一页会等你。",
        "迷茫" => "还看不清方向的时候，不必马上给自己一个答案。我们可以先停在这段文字旁边。",
        "失落" => "难过的感觉，不需要立刻被赶走。给它一点位置，也给自己一点时间。",
        "成长" => "有些变化很轻，只有你自己知道它有多不容易。可以慢一点，按自己的步子走。",
        _ => "不必替明天做保证。此刻有一点让你愿意抬头看的微光，就已经很好。",
    };
    EchoResult {
        quotes,
        reply: reply.into(),
        demo: true,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corpus_and_matching() {
        let all = quotes();
        assert!(all.len() >= 24);
        let ids: std::collections::HashSet<_> = all.iter().map(|q| &q.id).collect();
        assert_eq!(ids.len(), all.len());
        assert!(all.iter().all(|q| !q.text.is_empty()
            && !q.author.is_empty()
            && q.source.starts_with("https://")));
        for mood in MOODS {
            assert_eq!(respond("", Some(mood)).quotes.len(), 3);
        }
        assert!(respond("三角形的内角和", None).quotes.is_empty());
        assert!(!respond("今天很疲惫", None).quotes.is_empty());
    }
}

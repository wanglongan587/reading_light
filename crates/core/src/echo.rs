use crate::{EchoResult, Quote};

pub const MOODS: &[&str] = &["孤独", "疲惫", "迷茫", "失落", "成长", "希望"];

pub fn quotes() -> Vec<Quote> {
    serde_json::from_str(include_str!("../../../assets/quotes.json"))
        .expect("bundled quote corpus must be validated")
}

pub fn respond(text: &str, mood: Option<&str>) -> EchoResult {
    let categories: &[(&str, &[&str])] = &[
        (
            "孤独",
            &["孤独", "寂寞", "独自", "一个人", "想念", "孤单", "独处"],
        ),
        (
            "疲惫",
            &["疲惫", "疲倦", "累", "休息", "忙碌", "喘口气", "发呆"],
        ),
        (
            "迷茫",
            &["迷茫", "方向", "岔路", "不知道", "困惑", "犹豫", "选择"],
        ),
        (
            "失落",
            &["失落", "悲伤", "难过", "离别", "眼泪", "遗憾", "叹息"],
        ),
        (
            "成长",
            &["成长", "勇敢", "坚持", "长大", "改变", "前行", "跨越"],
        ),
        (
            "希望",
            &["希望", "明天", "阳光", "种子", "出发", "晴朗", "破晓"],
        ),
        (
            "时间",
            &[
                "时间", "回忆", "岁月", "往事", "旧", "流年", "曾经", "时光", "多年",
            ],
        ),
        (
            "海风",
            &[
                "海", "风", "云", "天空", "蓝天", "飞", "帆", "潮", "雨", "雾", "岛",
            ],
        ),
        (
            "夜晚",
            &[
                "夜", "月", "星", "灯", "微光", "黄昏", "暮", "梦", "醒", "晚",
            ],
        ),
        (
            "阅读",
            &[
                "书", "字", "读", "信", "纸", "诗", "故事", "笔", "墨", "句", "页",
            ],
        ),
        (
            "爱别",
            &[
                "爱", "告别", "再见", "相遇", "重逢", "温柔", "等待", "牵", "心",
            ],
        ),
        (
            "旅途",
            &[
                "路", "远方", "旅", "街", "窗", "桥", "归", "门", "走", "城", "回",
            ],
        ),
        (
            "自然",
            &[
                "花", "树", "林", "春", "夏", "秋", "冬", "山", "水", "草", "鸟", "雪", "叶",
            ],
        ),
    ];
    let tag = mood.filter(|m| MOODS.contains(m)).or_else(|| {
        let mut best_tag: Option<&str> = None;
        let mut best_score = 0usize;
        for &(cat, words) in categories {
            let score = words.iter().filter(|&&w| text.contains(w)).count();
            if score > best_score {
                best_score = score;
                best_tag = Some(cat);
            }
        }
        best_tag
    });
    let Some(tag) = tag else {
        return EchoResult {
            quotes: vec![],
            reply: "这段文字安静地停在纸面上，暂时没有匹配到古典诗文回响。你可以把它留作划线，或点击下方「问问微光」一起细读。".into(),
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
        "希望" => "不必替明天做保证。此刻有一点让你愿意抬头看的微光，就已经很好。",
        "时间" => {
            "时间带走喧嚣，却把真正触动过你的片段悄悄折进了纸页里。回头看时，它们仍然温热。"
        }
        "海风" => "风与云从不赶路，只是顺着天空慢慢漂移。读到这一段时，不妨也让自己松一口气。",
        "夜晚" => "安静的夜色很适合与文字并肩坐着。灯还亮着，星光与书页都愿意听你慢慢说完。",
        "阅读" => {
            "真正读进心里的句子，往往会陪人走过很长一段路。把这一句轻轻收进你的私人手账吧。"
        }
        "爱别" => "相遇和告别都是长河里的波纹。心里留住的那份温柔，并不会因为距离而变淡。",
        "旅途" => {
            "路途有时漫长，幸好窗外的云和手边的书都能成为暂时的落脚处。慢慢走，不急着抵达。"
        }
        _ => "四季流转，草木自有节气。把目光借给自然片刻，心里的皱褶也会慢慢舒展开。",
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
        assert!(all.len() >= 45);
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
        for (sample, expected_tag) in [
            ("岁月在回忆里缓慢流动", "时间"),
            ("海风吹过蓝天与白云", "海风"),
            ("深夜窗边的一盏微光", "夜晚"),
            ("翻开这页书信与诗句", "阅读"),
            ("温柔的相遇与告别", "爱别"),
            ("沿着远方的街道走去", "旅途"),
            ("春风吹过林间的花树", "自然"),
        ] {
            let res = respond(sample, None);
            assert!(
                !res.quotes.is_empty(),
                "expected literary resonance for: {sample}"
            );
            assert!(
                res.quotes
                    .iter()
                    .all(|q| q.tags.iter().any(|t| t == expected_tag)),
                "expected tag {expected_tag} for {sample:?}, got {:?}",
                res.quotes.iter().map(|q| &q.tags).collect::<Vec<_>>()
            );
            assert!(!res.reply.is_empty());
        }
    }
}

use crate::{
    soundscape::{SoundscapeHandle, SoundscapeSource},
    tts,
};
use anyhow::{Result, anyhow, ensure};
use glow_core::{CustomRadioStation, SoundscapeMix, valid_radio_stream_url};
use rodio::Source;
use serde_json::Value;
use std::{
    collections::VecDeque,
    fs,
    io::{self, BufReader, Cursor, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioStationKind {
    OnlineStream,
    NightHost,
    BookEchoHost,
    AmbientSynth,
    LocalFolder,
}

impl RadioStationKind {
    pub fn badge_label(self) -> &'static str {
        match self {
            Self::OnlineStream => "在线电台流",
            Self::NightHost => "深夜主播夜话",
            Self::BookEchoHost => "书摘回声台",
            Self::AmbientSynth => "离线微光琴房",
            Self::LocalFolder => "本地私藏磁带",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RadioStation {
    pub id: String,
    pub frequency_mhz: f32,
    pub name: String,
    pub subtitle: String,
    pub mood_line: String,
    pub kind: RadioStationKind,
    pub stream_urls: Vec<String>,
    pub suggested_lamp: &'static str,
    pub tags: Vec<String>,
    pub is_custom: bool,
}

#[derive(Clone, Debug)]
pub struct NightSegment {
    pub title: String,
    pub script: String,
    pub source_hint: String,
}

#[derive(Clone, Debug)]
pub struct OnlineStationHit {
    pub id: String,
    pub name: String,
    pub country: String,
    pub tags: Vec<String>,
    pub stream_url: String,
    pub bitrate: u32,
    pub suggested_mhz: f32,
}

#[derive(Clone, Debug, Default)]
pub struct RadioStatus {
    pub playing: bool,
    pub paused: bool,
    pub tuning: bool,
    pub connecting: bool,
    pub fallback_offline: bool,
    pub station_id: String,
    pub station_name: String,
    pub frequency_mhz: f32,
    pub kind_label: String,
    pub now_playing_title: String,
    pub current_line: String,
    pub source_hint: String,
    pub stream_meta: Option<String>,
    pub host_speaking: bool,
    pub local_track_count: usize,
    pub local_track_index: usize,
    pub error: Option<String>,
}

/// Curated, ad-free healing FM stations spanning 88.6 – 107.6 MHz.
/// Online stations use reliable MP3 streams natively decodable by rodio.
pub fn builtin_stations() -> Vec<RadioStation> {
    vec![
        RadioStation {
            id: "lofi-rain".into(),
            frequency_mhz: 88.6,
            name: "雨夜 Lofi 书房".into(),
            subtitle: "Downtempo · 柔和慢拍与温暖合成器".into(),
            mood_line: "窗外落着细雨，慢一点也没关系，今晚的节拍只陪你放空。".into(),
            kind: RadioStationKind::OnlineStream,
            stream_urls: vec![
                "https://ice2.somafm.com/groovesalad-128-mp3".into(),
                "https://ice4.somafm.com/groovesalad-128-mp3".into(),
            ],
            suggested_lamp: "amber",
            tags: vec!["lofi".into(), "chill".into(), "rain".into()],
            is_custom: false,
        },
        RadioStation {
            id: "midnight-jazz".into(),
            frequency_mhz: 91.2,
            name: "午夜微醺轻爵士".into(),
            subtitle: "Mellow Vocals & Midnight Lounge".into(),
            mood_line: "昏黄台灯下，像旧电影里的午夜爵士小馆，慢慢把紧绷放下。".into(),
            kind: RadioStationKind::OnlineStream,
            stream_urls: vec![
                "https://ice2.somafm.com/lush-128-mp3".into(),
                "https://ice4.somafm.com/secretagent-128-mp3".into(),
            ],
            suggested_lamp: "ember",
            tags: vec!["jazz".into(), "lounge".into(), "night".into()],
            is_custom: false,
        },
        RadioStation {
            id: "glow-night-talk".into(),
            frequency_mhz: 94.2,
            name: "微光深夜夜话".into(),
            subtitle: "温柔神经主播 · 雨夜电钢伴读夜话".into(),
            mood_line: "看书累了就闭上眼吧，今晚我在这里，陪你听听风声和几句轻柔的话。".into(),
            kind: RadioStationKind::NightHost,
            stream_urls: vec![],
            suggested_lamp: "amber",
            tags: vec!["夜话".into(), "治愈".into(), "陪伴".into()],
            is_custom: false,
        },
        RadioStation {
            id: "classical-nocturne".into(),
            frequency_mhz: 96.8,
            name: "古典夜曲与钢琴".into(),
            subtitle: "Radio Swiss Classic · 无广告静谧室内乐".into(),
            mood_line: "月光落在黑白琴键上，让肖邦与德彪西的余韵替你安抚疲惫的双眼。".into(),
            kind: RadioStationKind::OnlineStream,
            stream_urls: vec![
                "https://stream.srg-ssr.ch/m/rsc_de/mp3_128".into(),
                "https://ice2.somafm.com/thistle-128-mp3".into(),
            ],
            suggested_lamp: "rose",
            tags: vec!["classical".into(), "piano".into(), "nocturne".into()],
            is_custom: false,
        },
        RadioStation {
            id: "book-echo-fm".into(),
            frequency_mhz: 99.9,
            name: "书页回声私人台".into(),
            subtitle: "你的划线摘录与日记微光 · 深夜轻念".into(),
            mood_line: "那些曾经打动过你的句子，今夜化成电波，轻轻回到耳边。".into(),
            kind: RadioStationKind::BookEchoHost,
            stream_urls: vec![],
            suggested_lamp: "rose",
            tags: vec!["书摘".into(), "划线".into(), "夜读".into()],
            is_custom: false,
        },
        RadioStation {
            id: "deep-space-sleep".into(),
            frequency_mhz: 101.4,
            name: "深空星眠声景".into(),
            subtitle: "Drone Zone · 漂浮星尘与助眠和弦".into(),
            mood_line: "没有急促的鼓点，只有缓慢浮动的星尘和弦，像漂在安静的夜海里。".into(),
            kind: RadioStationKind::OnlineStream,
            stream_urls: vec![
                "https://ice2.somafm.com/dronezone-128-mp3".into(),
                "https://ice4.somafm.com/deepspaceone-128-mp3".into(),
            ],
            suggested_lamp: "starry",
            tags: vec!["ambient".into(), "sleep".into(), "space".into()],
            is_custom: false,
        },
        RadioStation {
            id: "offline-piano-rain".into(),
            frequency_mhz: 104.6,
            name: "汤屋雨夜琴房".into(),
            subtitle: "100% 离线治愈合成台 · 细雨 · 炉火 · 轻琴".into(),
            mood_line: "即使没有网络也不必担心，这间雨夜小琴房永远为你留着一盏暖灯。".into(),
            kind: RadioStationKind::AmbientSynth,
            stream_urls: vec![],
            suggested_lamp: "ember",
            tags: vec!["离线".into(), "钢琴".into(), "雨声".into()],
            is_custom: false,
        },
        RadioStation {
            id: "local-tape-fm".into(),
            frequency_mhz: 107.6,
            name: "本地私藏磁带".into(),
            subtitle: "本机音频目录漫游 · MP3 / WAV / OGG".into(),
            mood_line: "把你珍藏在电脑里的旧歌与播客，变成只属于今晚的私人磁带电台。".into(),
            kind: RadioStationKind::LocalFolder,
            stream_urls: vec![],
            suggested_lamp: "amber",
            tags: vec!["本地".into(), "磁带".into(), "私藏".into()],
            is_custom: false,
        },
    ]
}

pub fn all_stations(custom: &[CustomRadioStation]) -> Vec<RadioStation> {
    let mut list = builtin_stations();
    for st in custom {
        if st.kind == "online" && valid_radio_stream_url(&st.stream_url) {
            list.push(RadioStation {
                id: st.id.clone(),
                frequency_mhz: st.frequency_mhz.clamp(88.0, 108.0),
                name: st.name.clone(),
                subtitle: if st.subtitle.trim().is_empty() {
                    "自定义在线电台流".into()
                } else {
                    st.subtitle.clone()
                },
                mood_line: format!(
                    "正在收听你收藏的深夜频段「{}」，愿此刻被温柔接住。",
                    st.name
                ),
                kind: RadioStationKind::OnlineStream,
                stream_urls: vec![st.stream_url.clone()],
                suggested_lamp: "amber",
                tags: st.tags.clone(),
                is_custom: true,
            });
        }
    }
    list.sort_by(|a, b| {
        a.frequency_mhz
            .total_cmp(&b.frequency_mhz)
            .then_with(|| a.id.cmp(&b.id))
    });
    list
}

pub fn station_by_id(stations: &[RadioStation], id: &str) -> RadioStation {
    stations
        .iter()
        .find(|s| s.id == id)
        .or_else(|| stations.iter().find(|s| s.id == "glow-night-talk"))
        .or_else(|| stations.first())
        .cloned()
        .unwrap_or_else(|| builtin_stations().remove(2))
}

pub fn nearest_station_for_mhz(stations: &[RadioStation], mhz: f32) -> Option<RadioStation> {
    let target = mhz.clamp(88.0, 108.0);
    stations
        .iter()
        .min_by(|a, b| {
            (a.frequency_mhz - target)
                .abs()
                .total_cmp(&(b.frequency_mhz - target).abs())
        })
        .cloned()
}

/// Curated atmospheric cover & backdrop artwork per FM station.
/// Custom stations deterministically map onto one of the 8 bundled scenic illustrations.
pub fn station_scene_asset(station_id: &str) -> &'static str {
    match station_id {
        "lofi-rain" => "scene-night.png",
        "midnight-jazz" => "scene-lantern.png",
        "glow-night-talk" => "scene-spirited.png",
        "classical-nocturne" => "scene-howl.png",
        "book-echo-fm" => "scene-kiki.png",
        "deep-space-sleep" => "scene-prism.png",
        "offline-piano-rain" => "scene-totoro.png",
        "local-tape-fm" => "scene.png",
        other => {
            const POOL: &[&str] = &[
                "scene-lantern.png",
                "scene-spirited.png",
                "scene-night.png",
                "scene-howl.png",
                "scene-totoro.png",
                "scene-kiki.png",
                "scene-prism.png",
                "scene.png",
            ];
            let mut hash: u64 = 14695981039346656037;
            for b in other.bytes() {
                hash ^= b as u64;
                hash = hash.wrapping_mul(1099511628211);
            }
            POOL[(hash as usize) % POOL.len()]
        }
    }
}

/// Computes the exact initial `(now_playing_title, current_line, source_hint)` for a station
/// synchronously so UI and audio worker agree from frame 0 without post-tune text jumps.
pub fn initial_copy_for_station(
    station: &RadioStation,
    book_echoes: &[NightSegment],
) -> (String, String, String) {
    match station.kind {
        RadioStationKind::NightHost => {
            if let Some(first) = curated_night_talk_segments().into_iter().next() {
                return (first.title, first.script, first.source_hint);
            }
        }
        RadioStationKind::BookEchoHost => {
            if let Some(first) = book_echoes
                .first()
                .cloned()
                .or_else(|| default_book_echo_segments().into_iter().next())
            {
                return (first.title, first.script, first.source_hint);
            }
        }
        _ => {}
    }
    (
        station.name.clone(),
        station.mood_line.clone(),
        station.subtitle.clone(),
    )
}

pub fn curated_night_talk_segments() -> Vec<NightSegment> {
    const ITEMS: &[(&str, &str, &str)] = &[
        (
            "致看书眼睛酸涩的你",
            "欢迎来到微光深夜电台。如果你刚刚合上书页，觉得眼睛有些发酸，现在可以把视线从文字上移开，只看着屏幕里这盏慢慢呼吸的暖灯。今晚不需要再读完哪一章，也不需要记住任何道理，只要安安静静待一会儿就好。",
            "微光深夜夜话 · 第 01 封来信",
        ),
        (
            "关于允许自己慢下来",
            "白天总是被各种提醒和进度催着往前走，连看书有时都会不小心变成一场任务。可在深夜的电波里，时间是软的。允许自己什么也不做，听着窗外若有若无的雨声，让呼吸比刚才慢半拍，这就已经足够温柔了。",
            "微光深夜夜话 · 第 02 封来信",
        ),
        (
            "深夜窗台的一盏微光",
            "有人说，夜晚最动人的地方，并不是万籁俱寂，而是整座城市暗下去之后，依然有几扇窗户亮着橘黄色的光。现在你桌前的这束微光，也是夜色里很安心的一点点暖意。今天辛苦了。",
            "微光深夜夜话 · 第 03 封来信",
        ),
        (
            "把今天没说完的话交给风",
            "也许今天发生了一些让人有点累的小事，或者只是心里空落落的，说不上为什么。不用急着把情绪整理得井井有条，就像翻书时夹进去的一枚银杏叶，先轻轻放着，明天的阳光会替你重新照亮它。",
            "微光深夜夜话 · 第 04 封来信",
        ),
        (
            "月下独酌的轻柔陪伴",
            "李白曾写过：举杯邀明月，对影成三人。孤独并不总是冷清的，有时它只是留给自己的安静角落。在这个角落里，有一首轻柔的钢琴曲，有一盏会呼吸的台灯，还有正在收听这段电波的你。",
            "古典微光 · 李白《月下独酌》",
        ),
        (
            "给肩膀松一口气",
            "趁着下一段旋律响起之前，试着把微微耸起的肩膀往下放一厘米，松开咬紧的牙关，长长地呼出一口气。你看，身体其实一直都在等你对它说一句：好了，可以休息了。",
            "微光深夜夜话 · 第 06 封来信",
        ),
        (
            "山气日夕佳，飞鸟相与还",
            "陶渊明在傍晚看着南山的雾气和归巢的飞鸟，写下：山气日夕佳，飞鸟相与还。万物到了夜晚都要归巢，思绪也一样。把白天四散奔跑的念头轻轻唤回来吧，今晚安心靠岸。",
            "古典微光 · 陶渊明《饮酒》",
        ),
        (
            "睡前不必复盘今天",
            "如果此刻脑海里又开始重播白天的某个瞬间，忍不住想自己哪里做得不够好，不妨把那段画面轻轻关成静音。深夜不是用来审判自己的法庭，它是用来盖好被子、休养生息的避风港。",
            "微光深夜夜话 · 第 08 封来信",
        ),
        (
            "电波里的微弱沙沙声",
            "老式收音机在两个频段之间，总有一小段温柔的沙沙声。生活里也有很多这样的过渡时刻，暂时还不确定下一站去哪里，但只要还在慢慢调频，总会在不经意间听见清晰而温暖的声音。",
            "微光深夜夜话 · 第 09 封来信",
        ),
        (
            "一首没有终点的夜曲",
            "接下来的一小段时间，我先安静一会儿，把麦克风交给细雨和轻柔的电钢。你可以闭上眼睛，跟着光晕吸气四秒、屏息七秒、呼气八秒。晚安，祝你今晚做个柔软的好梦。",
            "微光深夜夜话 · 晚安寄语",
        ),
    ];
    ITEMS
        .iter()
        .map(|(title, script, hint)| NightSegment {
            title: (*title).into(),
            script: (*script).into(),
            source_hint: (*hint).into(),
        })
        .collect()
}

pub fn default_book_echo_segments() -> Vec<NightSegment> {
    vec![
        NightSegment {
            title: "在林间，慢慢来".into(),
            script: "书页回声私人台为你轻念：风穿过树梢的时候从不赶路，看书也是。当你觉得累了，停在这一页就好，文字会像旧朋友一样在原地等你回来。".into(),
            source_hint: "内置示例书 · 《在林间，慢慢来》".into(),
        },
        NightSegment {
            title: "心远地自偏".into(),
            script: "结庐在人境，而无车马喧。问君何能尔？心远地自偏。在喧闹的一天结束之后，愿你心里始终留着一小片安静的林荫。".into(),
            source_hint: "微光书摘 · 陶渊明《饮酒》".into(),
        },
        NightSegment {
            title: "山重水复疑无路".into(),
            script: "山重水复疑无路，柳暗花明又一村。暂时看不清前路的时候，先歇一歇脚，转过下一个弯，也许灯火就在不远处亮着。".into(),
            source_hint: "微光书摘 · 陆游《游山西村》".into(),
        },
    ]
}

/// Search free global internet radio stations via Radio-Browser public API.
/// Explicitly restricts to `codec=MP3&hidebroken=true` so every returned station decodes cleanly in rodio.
pub fn search_online_stations(query: &str, tag: Option<&str>) -> Result<Vec<OnlineStationHit>> {
    let q = query.trim();
    let t = tag.map(str::trim).filter(|s| !s.is_empty());
    ensure!(
        !q.is_empty() || t.is_some(),
        "请输入电台关键词或选择一个治愈标签"
    );

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(7))
        .timeout(Duration::from_secs(12))
        .user_agent("GlowReadingFM/0.1.5")
        .build()
        .map_err(|e| anyhow!("无法初始化电台搜索连接：{e}"))?;

    let mirrors = [
        "https://de1.api.radio-browser.info",
        "https://nl1.api.radio-browser.info",
        "https://at1.api.radio-browser.info",
    ];

    let mut params: Vec<(&str, String)> = vec![
        ("codec", "MP3".into()),
        ("hidebroken", "true".into()),
        ("order", "clickcount".into()),
        ("reverse", "true".into()),
        ("limit", "18".into()),
    ];
    if !q.is_empty() {
        params.push(("name", q.to_owned()));
    }
    if let Some(tag_val) = t {
        params.push(("tag", tag_val.to_owned()));
    }

    let mut last_err: Option<anyhow::Error> = None;
    for base in mirrors {
        let url = format!("{base}/json/stations/search");
        match client.get(&url).query(&params).send() {
            Ok(resp) if resp.status().is_success() => {
                let raw: Value = resp.json().map_err(|e| anyhow!("电台目录解析失败：{e}"))?;
                let items = raw.as_array().cloned().unwrap_or_default();
                let mut seen_urls = std::collections::HashSet::new();
                let mut hits = Vec::new();
                for item in items {
                    let stream_url = item["url_resolved"]
                        .as_str()
                        .filter(|s| valid_radio_stream_url(s))
                        .or_else(|| item["url"].as_str().filter(|s| valid_radio_stream_url(s)))
                        .unwrap_or("")
                        .trim()
                        .to_owned();
                    if stream_url.is_empty() || !seen_urls.insert(stream_url.clone()) {
                        continue;
                    }
                    let name = item["name"]
                        .as_str()
                        .unwrap_or("未命名电台")
                        .trim()
                        .to_owned();
                    if name.is_empty() {
                        continue;
                    }
                    let uuid = item["stationuuid"]
                        .as_str()
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| format!("rb-{}", &s[..s.len().min(16)]))
                        .unwrap_or_else(|| format!("rb-{}", hits.len() + 1));
                    let country = item["country"].as_str().unwrap_or("").trim().to_owned();
                    let tags: Vec<String> = item["tags"]
                        .as_str()
                        .unwrap_or("")
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .take(4)
                        .map(str::to_owned)
                        .collect();
                    let bitrate = item["bitrate"].as_u64().unwrap_or(128) as u32;
                    let suggested_mhz = deterministic_mhz(&uuid);
                    hits.push(OnlineStationHit {
                        id: uuid,
                        name,
                        country,
                        tags,
                        stream_url,
                        bitrate,
                        suggested_mhz,
                    });
                }
                return Ok(hits);
            }
            Ok(resp) => {
                last_err = Some(anyhow!("电台服务返回状态 {}", resp.status().as_u16()));
            }
            Err(err) => {
                last_err = Some(anyhow!("无法连接到电台目录：{err}"));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("在线电台搜索暂时不可达，请稍后再试")))
}

pub fn deterministic_mhz(seed: &str) -> f32 {
    let mut hash: u32 = 2166136261;
    for b in seed.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(16777619);
    }
    // Spread across 88.4 ..= 107.2 in 0.2 MHz steps
    let step = (hash % 95) as f32;
    ((88.4 + step * 0.2) * 10.0).round() / 10.0
}

pub fn scan_local_audio_files(folder: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_audio_files_depth(folder, 0, &mut out);
    out.sort();
    out
}

fn collect_audio_files_depth(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 2 || out.len() >= 400 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= 400 {
            break;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_audio_files_depth(&path, depth + 1, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let lower = ext.to_ascii_lowercase();
            if matches!(lower.as_str(), "mp3" | "wav" | "ogg") {
                out.push(path);
            }
        }
    }
}

/// Sliding-window `Read + Seek + Send + Sync` adapter over an infinite HTTP audio stream.
/// Pre-buffers the first probe window so `rodio::Decoder` (symphonia MP3 / lewton Vorbis) can
/// inspect headers and seek freely inside the live window while streaming unbounded audio.
pub struct SeekableStreamReader {
    inner: Mutex<Box<dyn Read + Send>>,
    buf: VecDeque<u8>,
    window_start: u64,
    pos: u64,
    eof: bool,
    stop_flag: Arc<AtomicBool>,
    max_window: usize,
}

impl SeekableStreamReader {
    pub fn new(inner: Box<dyn Read + Send>, stop_flag: Arc<AtomicBool>, max_window: usize) -> Self {
        Self {
            inner: Mutex::new(inner),
            buf: VecDeque::with_capacity(max_window.min(262_144)),
            window_start: 0,
            pos: 0,
            eof: false,
            stop_flag,
            max_window: max_window.max(32_768),
        }
    }

    /// Pre-buffer an initial slice for format header detection before returning `pos` to 0.
    pub fn prebuffer(&mut self, target_bytes: usize) -> io::Result<usize> {
        let cap = target_bytes.min(self.max_window);
        let mut tmp = [0u8; 4096];
        while self.buf.len() < cap && !self.eof {
            if self.stop_flag.load(Ordering::Relaxed) {
                break;
            }
            let want = (cap - self.buf.len()).min(tmp.len());
            let n = {
                let mut guard = self
                    .inner
                    .lock()
                    .map_err(|_| io::Error::other("stream lock poisoned"))?;
                guard.read(&mut tmp[..want])?
            };
            if n == 0 {
                self.eof = true;
                break;
            }
            self.buf.extend(&tmp[..n]);
        }
        self.pos = 0;
        Ok(self.buf.len())
    }

    pub fn peek_magic(&self) -> [u8; 4] {
        let mut out = [0u8; 4];
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(&b) = self.buf.get(i) {
                *slot = b;
            }
        }
        out
    }

    fn pull_more(&mut self, min_want: usize) -> io::Result<usize> {
        if self.eof || self.stop_flag.load(Ordering::Relaxed) {
            return Ok(0);
        }
        let mut tmp = [0u8; 8192];
        let want = min_want.clamp(1024, tmp.len());
        let n = {
            let mut guard = self
                .inner
                .lock()
                .map_err(|_| io::Error::other("stream lock poisoned"))?;
            guard.read(&mut tmp[..want])?
        };
        if n == 0 {
            self.eof = true;
            return Ok(0);
        }
        self.buf.extend(&tmp[..n]);
        if self.buf.len() > self.max_window {
            let excess = self.buf.len() - self.max_window;
            // Never trim past the current reader position if possible
            let safe_excess = excess.min(self.pos.saturating_sub(self.window_start) as usize);
            if safe_excess > 0 {
                self.buf.drain(..safe_excess);
                self.window_start += safe_excess as u64;
            }
        }
        Ok(n)
    }
}

impl Read for SeekableStreamReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.stop_flag.load(Ordering::Relaxed) {
            return Ok(0);
        }
        let available_end = self.window_start + self.buf.len() as u64;
        if self.pos >= available_end {
            let pulled = self.pull_more(out.len())?;
            if pulled == 0 {
                return Ok(0);
            }
        }
        if self.pos < self.window_start {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "live stream cursor fell behind sliding window",
            ));
        }
        let offset = (self.pos - self.window_start) as usize;
        let avail = self.buf.len().saturating_sub(offset);
        if avail == 0 {
            return Ok(0);
        }
        let take = avail.min(out.len());
        let (first, second) = self.buf.as_slices();
        if offset < first.len() {
            let n1 = (first.len() - offset).min(take);
            out[..n1].copy_from_slice(&first[offset..offset + n1]);
            if take > n1 {
                let n2 = take - n1;
                out[n1..take].copy_from_slice(&second[..n2]);
            }
        } else {
            let s_off = offset - first.len();
            out[..take].copy_from_slice(&second[s_off..s_off + take]);
        }
        self.pos += take as u64;
        Ok(take)
    }
}

impl Seek for SeekableStreamReader {
    fn seek(&mut self, style: SeekFrom) -> io::Result<u64> {
        let target: i128 = match style {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
            SeekFrom::End(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "live radio stream has unknown end",
                ));
            }
        };
        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "negative seek position",
            ));
        }
        let target_u64 = target as u64;
        if target_u64 < self.window_start {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot seek before sliding window",
            ));
        }
        let mut guard_budget = 256 * 1024usize;
        while target_u64 > self.window_start + self.buf.len() as u64
            && !self.eof
            && guard_budget > 0
            && !self.stop_flag.load(Ordering::Relaxed)
        {
            let pulled = self.pull_more(4096)?;
            if pulled == 0 {
                break;
            }
            guard_budget = guard_budget.saturating_sub(pulled);
        }
        let max_pos = self.window_start + self.buf.len() as u64;
        self.pos = target_u64.min(max_pos);
        Ok(self.pos)
    }
}

/// Gentle ~360ms vintage FM tuning static transition (warm band-passed pink noise + soft sweep).
pub struct TuningStaticSource {
    frame: u64,
    total_frames: u64,
    emit_right: bool,
    pending_right: f32,
    rng: u64,
    lp_l: f32,
    lp_r: f32,
    hp_prev_l: f32,
    hp_prev_r: f32,
}

impl TuningStaticSource {
    pub fn new(duration_ms: u32) -> Self {
        let total = ((SAMPLE_RATE as u64) * duration_ms.clamp(120, 900) as u64) / 1000;
        Self {
            frame: 0,
            total_frames: total,
            emit_right: false,
            pending_right: 0.0,
            rng: 0x9E37_79B9_7F4A_7C15,
            lp_l: 0.0,
            lp_r: 0.0,
            hp_prev_l: 0.0,
            hp_prev_r: 0.0,
        }
    }

    fn next_signed(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        let u = (x >> 40) as f32 / ((1u32 << 24) as f32);
        u * 2.0 - 1.0
    }
}

impl Iterator for TuningStaticSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.emit_right {
            self.emit_right = false;
            return Some(self.pending_right);
        }
        if self.frame >= self.total_frames {
            return None;
        }
        let p = self.frame as f32 / self.total_frames.max(1) as f32;
        let t = self.frame as f32 / SAMPLE_RATE as f32;
        self.frame += 1;

        // Smooth raised-cosine bell envelope (soft attack, gentle decay)
        let attack = (p / 0.16).clamp(0.0, 1.0);
        let release = ((1.0 - p) / 0.42).clamp(0.0, 1.0);
        let env = (0.5 - 0.5 * (attack * std::f32::consts::PI).cos())
            * (0.5 - 0.5 * (release * std::f32::consts::PI).cos());

        let wl = self.next_signed();
        let wr = self.next_signed();
        // Warm radio band-pass: low-pass at ~2.2kHz + gentle high-pass to remove sub-rumble
        self.lp_l += (wl - self.lp_l) * 0.22;
        self.lp_r += (wr - self.lp_r) * 0.22;
        let bp_l = self.lp_l - self.hp_prev_l * 0.72;
        let bp_r = self.lp_r - self.hp_prev_r * 0.72;
        self.hp_prev_l = self.lp_l;
        self.hp_prev_r = self.lp_r;

        // Ultra-subtle vintage analog tuning carrier hum sweep
        let sweep_hz = 640.0 + 260.0 * (p * std::f32::consts::PI).sin();
        let carrier = (t * std::f32::consts::TAU * sweep_hz).sin() * 0.018;

        let l = ((bp_l * 0.24 + carrier) * env).clamp(-0.32, 0.32);
        let r = ((bp_r * 0.24 + carrier * 0.92) * env).clamp(-0.32, 0.32);
        self.pending_right = r;
        self.emit_right = true;
        Some(l)
    }
}

impl Source for TuningStaticSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(1024)
    }

    fn channels(&self) -> u16 {
        CHANNELS
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.total_frames as f64 / SAMPLE_RATE as f64,
        ))
    }
}

/// Real-time audio wrapper that publishes smoothed RMS energy for the breathing lamp,
/// honors sleep-timer fade multipliers, and performs click-free fade-out on stop.
pub struct MeteredRadioSource<S> {
    inner: S,
    audio_level: Arc<AtomicU32>,
    fade_multiplier: Arc<AtomicU32>,
    stop_flag: Arc<AtomicBool>,
    smoothed_fade: f32,
    stop_fade: Option<u32>,
    accum_sq: f32,
    accum_count: u32,
    smoothed_rms: f32,
}

impl<S: Source<Item = f32>> MeteredRadioSource<S> {
    pub fn new(
        inner: S,
        audio_level: Arc<AtomicU32>,
        fade_multiplier: Arc<AtomicU32>,
        stop_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            inner,
            audio_level,
            fade_multiplier,
            stop_flag,
            smoothed_fade: 1.0,
            stop_fade: None,
            accum_sq: 0.0,
            accum_count: 0,
            smoothed_rms: 0.0,
        }
    }
}

impl<S: Source<Item = f32>> Iterator for MeteredRadioSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let stop_samples =
            (self.inner.sample_rate().max(8_000) * self.inner.channels().max(1) as u32) / 6; // ~166ms smooth stop fade
        if self.stop_fade.is_none() && self.stop_flag.load(Ordering::Relaxed) {
            self.stop_fade = Some(0);
        }
        let stop_gain = if let Some(ref mut step) = self.stop_fade {
            if *step >= stop_samples {
                self.audio_level.store(0.0_f32.to_bits(), Ordering::Relaxed);
                return None;
            }
            let p = *step as f32 / stop_samples.max(1) as f32;
            *step += 1;
            0.5 + 0.5 * (p * std::f32::consts::PI).cos()
        } else {
            1.0
        };

        let Some(raw) = self.inner.next() else {
            self.audio_level.store(0.0_f32.to_bits(), Ordering::Relaxed);
            return None;
        };

        let target_fade =
            f32::from_bits(self.fade_multiplier.load(Ordering::Relaxed)).clamp(0.0, 1.0);
        self.smoothed_fade += (target_fade - self.smoothed_fade) * 0.002;
        let out = (raw * self.smoothed_fade * stop_gain).clamp(-0.98, 0.98);

        self.accum_sq += out * out;
        self.accum_count += 1;
        if self.accum_count >= 512 {
            let rms = (self.accum_sq / self.accum_count as f32).sqrt();
            // Gentle perceptual boost for lamp visual responsiveness
            let norm = (rms * 3.1).clamp(0.0, 1.0);
            self.smoothed_rms += (norm - self.smoothed_rms) * 0.24;
            self.audio_level
                .store(self.smoothed_rms.to_bits(), Ordering::Relaxed);
            self.accum_sq = 0.0;
            self.accum_count = 0;
        }
        Some(out)
    }
}

impl<S: Source<Item = f32>> Source for MeteredRadioSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

enum RadioCommand {
    Tune {
        cmd_gen: u64,
        station: Box<RadioStation>,
        volume: f32,
        host_voice: String,
        book_echoes: Vec<NightSegment>,
        local_folder: Option<PathBuf>,
        with_static: bool,
    },
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    NextSegment,
}

pub struct RadioPlayer {
    cmd_tx: mpsc::Sender<RadioCommand>,
    status: Arc<Mutex<RadioStatus>>,
    audio_level: Arc<AtomicU32>,
    fade_multiplier: Arc<AtomicU32>,
    generation: Arc<AtomicU64>,
    stream_stop: Arc<AtomicBool>,
}

impl RadioPlayer {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<RadioCommand>();
        let status = Arc::new(Mutex::new(RadioStatus::default()));
        let audio_level = Arc::new(AtomicU32::new(0.0_f32.to_bits()));
        let fade_multiplier = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let generation = Arc::new(AtomicU64::new(0));
        let stream_stop = Arc::new(AtomicBool::new(false));

        let st_bg = Arc::clone(&status);
        let lvl_bg = Arc::clone(&audio_level);
        let fade_bg = Arc::clone(&fade_multiplier);
        let gen_bg = Arc::clone(&generation);
        let stop_bg = Arc::clone(&stream_stop);

        std::thread::Builder::new()
            .name("glow-fm-radio".into())
            .spawn(move || {
                radio_worker_loop(cmd_rx, st_bg, lvl_bg, fade_bg, gen_bg, stop_bg);
            })
            .expect("spawn glow-fm-radio thread");

        Self {
            cmd_tx,
            status,
            audio_level,
            fade_multiplier,
            generation,
            stream_stop,
        }
    }

    pub fn status(&self) -> RadioStatus {
        self.status.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn audio_level(&self) -> f32 {
        f32::from_bits(self.audio_level.load(Ordering::Relaxed)).clamp(0.0, 1.0)
    }

    pub fn set_fade_multiplier(&self, factor: f32) {
        self.fade_multiplier
            .store(factor.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn tune(
        &self,
        station: RadioStation,
        volume: f32,
        host_voice: String,
        book_echoes: Vec<NightSegment>,
        local_folder: Option<PathBuf>,
        with_static: bool,
    ) {
        let cmd_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.stream_stop.store(true, Ordering::SeqCst);
        self.set_fade_multiplier(1.0);
        // Update shared status synchronously so UI ticks never revert back to the previous station
        // while the audio worker finishes tearing down or connecting.
        let (title, line, hint) = initial_copy_for_station(&station, &book_echoes);
        update_status(&self.status, |s| {
            s.playing = true;
            s.paused = false;
            s.tuning = with_static;
            s.connecting = station.kind == RadioStationKind::OnlineStream;
            s.fallback_offline = false;
            s.station_id = station.id.clone();
            s.station_name = station.name.clone();
            s.frequency_mhz = station.frequency_mhz;
            s.kind_label = station.kind.badge_label().into();
            s.now_playing_title = title;
            s.current_line = line;
            s.source_hint = hint;
            s.stream_meta = None;
            s.host_speaking = false;
            s.error = None;
        });
        let _ = self.cmd_tx.send(RadioCommand::Tune {
            cmd_gen,
            station: Box::new(station),
            volume: volume.clamp(0.05, 1.0),
            host_voice,
            book_echoes,
            local_folder,
            with_static,
        });
    }

    pub fn pause(&self) {
        self.audio_level.store(0.0_f32.to_bits(), Ordering::Relaxed);
        update_status(&self.status, |s| {
            if s.playing {
                s.paused = true;
            }
        });
        let _ = self.cmd_tx.send(RadioCommand::Pause);
    }

    pub fn resume(&self) {
        update_status(&self.status, |s| {
            if s.playing {
                s.paused = false;
            }
        });
        let _ = self.cmd_tx.send(RadioCommand::Resume);
    }

    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.stream_stop.store(true, Ordering::SeqCst);
        self.audio_level.store(0.0_f32.to_bits(), Ordering::Relaxed);
        update_status(&self.status, |s| {
            s.playing = false;
            s.paused = false;
            s.tuning = false;
            s.connecting = false;
            s.host_speaking = false;
        });
        let _ = self.cmd_tx.send(RadioCommand::Stop);
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self
            .cmd_tx
            .send(RadioCommand::SetVolume(volume.clamp(0.05, 1.0)));
    }

    pub fn next_segment(&self) {
        let _ = self.cmd_tx.send(RadioCommand::NextSegment);
    }
}

impl Drop for RadioPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn update_status(status: &Arc<Mutex<RadioStatus>>, edit: impl FnOnce(&mut RadioStatus)) {
    if let Ok(mut g) = status.lock() {
        edit(&mut g);
    }
}

fn update_status_if_current(
    status: &Arc<Mutex<RadioStatus>>,
    generation: &Arc<AtomicU64>,
    expected_gen: u64,
    edit: impl FnOnce(&mut RadioStatus),
) {
    if generation.load(Ordering::SeqCst) == expected_gen {
        update_status(status, edit);
    }
}

struct ActivePlayback {
    _stream: rodio::OutputStream,
    primary_sink: rodio::Sink,
    voice_sink: Option<rodio::Sink>,
    bed_handle: Option<SoundscapeHandle>,
    source_stop: Arc<AtomicBool>,
}

impl ActivePlayback {
    fn pause(&self) {
        self.primary_sink.pause();
        if let Some(vs) = &self.voice_sink {
            vs.pause();
        }
    }

    fn play(&self) {
        self.primary_sink.play();
        if let Some(vs) = &self.voice_sink {
            vs.play();
        }
    }

    fn set_volume(&self, vol: f32) {
        self.primary_sink.set_volume(vol);
        if let Some(vs) = &self.voice_sink {
            vs.set_volume((vol * 1.12).clamp(0.08, 1.0));
        }
    }

    fn stop_all(self) {
        self.source_stop.store(true, Ordering::Relaxed);
        if let Some(h) = &self.bed_handle {
            h.trigger_fade_out();
        }
        self.primary_sink.stop();
        if let Some(vs) = self.voice_sink {
            vs.stop();
        }
    }
}

fn open_online_stream(
    urls: &[String],
    stop_flag: Arc<AtomicBool>,
) -> Result<(SeekableStreamReader, Option<String>, Option<String>)> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(9))
        .redirect(reqwest::redirect::Policy::limited(6))
        .user_agent("GlowReadingFM/0.1.5")
        .build()
        .map_err(|e| anyhow!("无法创建电台连接：{e}"))?;

    let mut last_err: Option<anyhow::Error> = None;
    for url in urls {
        if stop_flag.load(Ordering::Relaxed) {
            return Err(anyhow!("已切换频段"));
        }
        if !valid_radio_stream_url(url) {
            continue;
        }
        match client.get(url).send() {
            Ok(resp) if resp.status().is_success() => {
                let content_type = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                let icy_name = resp
                    .headers()
                    .get("icy-name")
                    .or_else(|| resp.headers().get("icy-description"))
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty());
                let mut reader =
                    SeekableStreamReader::new(Box::new(resp), Arc::clone(&stop_flag), 256 * 1024);
                let buffered = reader.prebuffer(20 * 1024)?;
                if buffered >= 1024 {
                    return Ok((reader, content_type, icy_name));
                }
                last_err = Some(anyhow!("电台流暂时没有音频数据"));
            }
            Ok(resp) => {
                last_err = Some(anyhow!("电台节点返回状态 {}", resp.status().as_u16()));
            }
            Err(err) => {
                last_err = Some(anyhow!("无法连接到电台信号：{err}"));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("当前无法连接到在线电台")))
}

fn start_bed_playback(
    mix: &SoundscapeMix,
    volume: f32,
    audio_level: &Arc<AtomicU32>,
    fade_multiplier: &Arc<AtomicU32>,
    with_static: bool,
) -> Result<ActivePlayback> {
    let (stream, handle) = crate::soundscape::try_default_output_stream()
        .map_err(|_| anyhow!("未检测到可用的音频输出设备"))?;
    let primary_sink = rodio::Sink::try_new(&handle)?;
    let voice_sink = rodio::Sink::try_new(&handle)?;
    primary_sink.set_volume(volume);
    voice_sink.set_volume((volume * 1.12).clamp(0.08, 1.0));

    if with_static {
        primary_sink.append(TuningStaticSource::new(340));
    }

    let source_stop = Arc::new(AtomicBool::new(false));
    let (bed_src, bed_handle) = SoundscapeSource::with_mix_handle(mix);
    let metered = MeteredRadioSource::new(
        bed_src,
        Arc::clone(audio_level),
        Arc::clone(fade_multiplier),
        Arc::clone(&source_stop),
    );
    primary_sink.append(metered);

    Ok(ActivePlayback {
        _stream: stream,
        primary_sink,
        voice_sink: Some(voice_sink),
        bed_handle: Some(bed_handle),
        source_stop,
    })
}

fn radio_worker_loop(
    cmd_rx: mpsc::Receiver<RadioCommand>,
    status: Arc<Mutex<RadioStatus>>,
    audio_level: Arc<AtomicU32>,
    fade_multiplier: Arc<AtomicU32>,
    generation: Arc<AtomicU64>,
    stream_stop: Arc<AtomicBool>,
) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok();

    let mut active_playback: Option<ActivePlayback> = None;
    let mut current_station: Option<RadioStation> = None;
    let mut volume = 0.72_f32;
    let mut paused = false;
    let mut host_voice = tts::VOICES[0].id.to_owned();
    let mut night_playlist: Vec<NightSegment> = vec![];
    let mut night_idx = 0usize;
    let mut next_host_speak_at: Option<Instant> = None;
    let mut host_speaking = false;
    let mut local_tracks: Vec<PathBuf> = vec![];
    let mut local_idx = 0usize;

    loop {
        let playing_active = active_playback.is_some() && !paused;
        let recv = if playing_active {
            cmd_rx.recv_timeout(Duration::from_millis(90))
        } else {
            cmd_rx
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };

        match recv {
            Ok(cmd) => match cmd {
                RadioCommand::Stop => {
                    if let Some(pb) = active_playback.take() {
                        pb.stop_all();
                    }
                    paused = false;
                    host_speaking = false;
                    next_host_speak_at = None;
                    audio_level.store(0.0_f32.to_bits(), Ordering::Relaxed);
                    update_status(&status, |s| {
                        s.playing = false;
                        s.paused = false;
                        s.tuning = false;
                        s.connecting = false;
                        s.host_speaking = false;
                    });
                }
                RadioCommand::Pause => {
                    paused = true;
                    if let Some(pb) = &active_playback {
                        pb.pause();
                    }
                    audio_level.store(0.0_f32.to_bits(), Ordering::Relaxed);
                    update_status(&status, |s| {
                        s.paused = true;
                    });
                }
                RadioCommand::Resume => {
                    paused = false;
                    if let Some(pb) = &active_playback {
                        pb.play();
                    }
                    update_status(&status, |s| {
                        s.paused = false;
                    });
                }
                RadioCommand::SetVolume(vol) => {
                    volume = vol;
                    if let Some(pb) = &active_playback {
                        pb.set_volume(volume);
                    }
                }
                RadioCommand::NextSegment => {
                    if let Some(st) = &current_station {
                        match st.kind {
                            RadioStationKind::NightHost | RadioStationKind::BookEchoHost => {
                                if !night_playlist.is_empty() {
                                    night_idx = (night_idx + 1) % night_playlist.len();
                                    if let Some(pb) = &active_playback
                                        && let Some(vs) = &pb.voice_sink
                                    {
                                        vs.clear();
                                        vs.play();
                                    }
                                    host_speaking = false;
                                    next_host_speak_at = Some(Instant::now());
                                }
                            }
                            RadioStationKind::LocalFolder => {
                                if !local_tracks.is_empty() {
                                    local_idx = (local_idx + 1) % local_tracks.len();
                                    if let Some(pb) = active_playback.take() {
                                        pb.stop_all();
                                    }
                                    active_playback = start_local_track_playback(
                                        &local_tracks,
                                        local_idx,
                                        volume,
                                        &audio_level,
                                        &fade_multiplier,
                                        false,
                                        &status,
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
                RadioCommand::Tune {
                    cmd_gen,
                    station,
                    volume: next_vol,
                    host_voice: next_voice,
                    book_echoes,
                    local_folder,
                    with_static,
                } => {
                    if generation.load(Ordering::SeqCst) != cmd_gen {
                        continue;
                    }
                    if let Some(pb) = active_playback.take() {
                        pb.stop_all();
                    }
                    let my_gen = cmd_gen;
                    if generation.load(Ordering::SeqCst) != my_gen {
                        continue;
                    }
                    stream_stop.store(false, Ordering::SeqCst);
                    volume = next_vol;
                    host_voice = next_voice;
                    paused = false;
                    host_speaking = false;
                    next_host_speak_at = None;
                    let station = *station;
                    current_station = Some(station.clone());

                    let (init_title, init_line, init_hint) =
                        initial_copy_for_station(&station, &book_echoes);
                    update_status_if_current(&status, &generation, my_gen, |s| {
                        s.playing = true;
                        s.paused = false;
                        s.tuning = with_static;
                        s.connecting = station.kind == RadioStationKind::OnlineStream;
                        s.fallback_offline = false;
                        s.station_id = station.id.clone();
                        s.station_name = station.name.clone();
                        s.frequency_mhz = station.frequency_mhz;
                        s.kind_label = station.kind.badge_label().into();
                        s.now_playing_title = init_title;
                        s.current_line = init_line;
                        s.source_hint = init_hint;
                        s.stream_meta = None;
                        s.host_speaking = false;
                        s.error = None;
                    });

                    match station.kind {
                        RadioStationKind::OnlineStream => {
                            // Play gentle tuning static immediately while connecting so tuning feels alive
                            let static_stream = if with_static {
                                crate::soundscape::try_default_output_stream()
                                    .ok()
                                    .and_then(|(s, h)| {
                                        rodio::Sink::try_new(&h).ok().map(|sink| {
                                            sink.set_volume((volume * 0.82).clamp(0.08, 0.75));
                                            sink.append(TuningStaticSource::new(420));
                                            (s, sink)
                                        })
                                    })
                            } else {
                                None
                            };

                            let per_stream_stop = Arc::new(AtomicBool::new(false));
                            let open_res =
                                open_online_stream(&station.stream_urls, Arc::clone(&stream_stop));
                            drop(static_stream);

                            if generation.load(Ordering::SeqCst) != my_gen {
                                continue;
                            }

                            match open_res {
                                Ok((reader, content_type, icy_name)) => {
                                    let magic = reader.peek_magic();
                                    let is_ogg = magic == *b"OggS"
                                        || content_type.as_deref().is_some_and(|ct| {
                                            ct.to_ascii_lowercase().contains("ogg")
                                        });
                                    let is_wav = magic == *b"RIFF";

                                    let decoded: Result<Box<dyn Source<Item = f32> + Send>> =
                                        if is_ogg {
                                            rodio::Decoder::new_vorbis(reader)
                                                .map(|d| Box::new(d.convert_samples::<f32>()) as _)
                                                .map_err(anyhow::Error::from)
                                        } else if is_wav {
                                            rodio::Decoder::new_wav(reader)
                                                .map(|d| Box::new(d.convert_samples::<f32>()) as _)
                                                .map_err(anyhow::Error::from)
                                        } else {
                                            rodio::Decoder::new_mp3(reader)
                                                .map(|d| Box::new(d.convert_samples::<f32>()) as _)
                                                .map_err(anyhow::Error::from)
                                        };

                                    match decoded.and_then(|src| {
                                        let (stream, handle) =
                                            crate::soundscape::try_default_output_stream()
                                                .map_err(|_| {
                                                    anyhow!("未检测到可用的音频输出设备")
                                                })?;
                                        let sink = rodio::Sink::try_new(&handle)?;
                                        sink.set_volume(volume);
                                        let metered = MeteredRadioSource::new(
                                            src,
                                            Arc::clone(&audio_level),
                                            Arc::clone(&fade_multiplier),
                                            Arc::clone(&per_stream_stop),
                                        );
                                        sink.append(metered);
                                        Ok(ActivePlayback {
                                            _stream: stream,
                                            primary_sink: sink,
                                            voice_sink: None,
                                            bed_handle: None,
                                            source_stop: per_stream_stop,
                                        })
                                    }) {
                                        Ok(pb) => {
                                            if generation.load(Ordering::SeqCst) != my_gen {
                                                pb.stop_all();
                                                continue;
                                            }
                                            active_playback = Some(pb);
                                            update_status_if_current(
                                                &status,
                                                &generation,
                                                my_gen,
                                                |s| {
                                                    s.tuning = false;
                                                    s.connecting = false;
                                                    s.fallback_offline = false;
                                                    s.stream_meta = icy_name.clone();
                                                    if let Some(meta) = icy_name {
                                                        s.source_hint = meta;
                                                    }
                                                },
                                            );
                                        }
                                        Err(_) => {
                                            if generation.load(Ordering::SeqCst) != my_gen {
                                                continue;
                                            }
                                            // Gentle offline fallback bed instead of harsh silence
                                            let fallback_mix =
                                                fallback_mix_for_station(&station.id);
                                            if let Ok(pb) = start_bed_playback(
                                                &fallback_mix,
                                                volume,
                                                &audio_level,
                                                &fade_multiplier,
                                                false,
                                            ) {
                                                active_playback = Some(pb);
                                            }
                                            update_status_if_current(
                                                &status,
                                                &generation,
                                                my_gen,
                                                |s| {
                                                    s.tuning = false;
                                                    s.connecting = false;
                                                    s.fallback_offline = true;
                                                    s.source_hint =
                                                    "夜色信号穿过云层 · 暂以本地微光旋律温柔代播"
                                                        .into();
                                                },
                                            );
                                        }
                                    }
                                }
                                Err(_) => {
                                    if generation.load(Ordering::SeqCst) != my_gen {
                                        continue;
                                    }
                                    let fallback_mix = fallback_mix_for_station(&station.id);
                                    if let Ok(pb) = start_bed_playback(
                                        &fallback_mix,
                                        volume,
                                        &audio_level,
                                        &fade_multiplier,
                                        false,
                                    ) {
                                        active_playback = Some(pb);
                                    }
                                    update_status_if_current(&status, &generation, my_gen, |s| {
                                        s.tuning = false;
                                        s.connecting = false;
                                        s.fallback_offline = true;
                                        s.source_hint =
                                            "当前网络暂不可达 · 已为你点亮本地微光琴韵代播".into();
                                    });
                                }
                            }
                        }
                        RadioStationKind::NightHost | RadioStationKind::BookEchoHost => {
                            night_playlist = if station.kind == RadioStationKind::BookEchoHost
                                && !book_echoes.is_empty()
                            {
                                book_echoes
                            } else if station.kind == RadioStationKind::BookEchoHost {
                                default_book_echo_segments()
                            } else {
                                curated_night_talk_segments()
                            };
                            night_idx = 0;
                            let mix = if station.kind == RadioStationKind::BookEchoHost {
                                SoundscapeMix {
                                    rain: 0.28,
                                    fireplace: 0.0,
                                    breeze: 0.36,
                                    cosmos: 0.28,
                                    train: 0.0,
                                    piano: 0.72,
                                    recipe: None,
                                }
                            } else {
                                SoundscapeMix {
                                    rain: 0.44,
                                    fireplace: 0.32,
                                    breeze: 0.0,
                                    cosmos: 0.14,
                                    train: 0.0,
                                    piano: 0.66,
                                    recipe: None,
                                }
                            };
                            match start_bed_playback(
                                &mix,
                                volume,
                                &audio_level,
                                &fade_multiplier,
                                with_static,
                            ) {
                                Ok(pb) => {
                                    if generation.load(Ordering::SeqCst) != my_gen {
                                        pb.stop_all();
                                        continue;
                                    }
                                    active_playback = Some(pb);
                                    next_host_speak_at =
                                        Some(Instant::now() + Duration::from_millis(850));
                                    if let Some(first) = night_playlist.first() {
                                        update_status_if_current(
                                            &status,
                                            &generation,
                                            my_gen,
                                            |s| {
                                                s.tuning = false;
                                                s.connecting = false;
                                                s.now_playing_title = first.title.clone();
                                                s.current_line = first.script.clone();
                                                s.source_hint = first.source_hint.clone();
                                            },
                                        );
                                    }
                                }
                                Err(err) => {
                                    update_status_if_current(&status, &generation, my_gen, |s| {
                                        s.playing = false;
                                        s.tuning = false;
                                        s.error = Some(err.to_string());
                                    });
                                }
                            }
                        }
                        RadioStationKind::AmbientSynth => {
                            let mix = SoundscapeMix {
                                rain: 0.56,
                                fireplace: 0.42,
                                breeze: 0.18,
                                cosmos: 0.22,
                                train: 0.0,
                                piano: 0.74,
                                recipe: None,
                            };
                            match start_bed_playback(
                                &mix,
                                volume,
                                &audio_level,
                                &fade_multiplier,
                                with_static,
                            ) {
                                Ok(pb) => {
                                    if generation.load(Ordering::SeqCst) != my_gen {
                                        pb.stop_all();
                                        continue;
                                    }
                                    active_playback = Some(pb);
                                    update_status_if_current(&status, &generation, my_gen, |s| {
                                        s.tuning = false;
                                        s.connecting = false;
                                    });
                                }
                                Err(err) => {
                                    update_status_if_current(&status, &generation, my_gen, |s| {
                                        s.playing = false;
                                        s.tuning = false;
                                        s.error = Some(err.to_string());
                                    });
                                }
                            }
                        }
                        RadioStationKind::LocalFolder => {
                            local_tracks = local_folder
                                .as_deref()
                                .map(scan_local_audio_files)
                                .unwrap_or_default();
                            local_idx = 0;
                            if local_tracks.is_empty() {
                                let warm_idle = SoundscapeMix::from_recipe("rain_hearth");
                                if let Ok(pb) = start_bed_playback(
                                    &warm_idle,
                                    volume,
                                    &audio_level,
                                    &fade_multiplier,
                                    with_static,
                                ) {
                                    if generation.load(Ordering::SeqCst) != my_gen {
                                        pb.stop_all();
                                        continue;
                                    }
                                    active_playback = Some(pb);
                                }
                                update_status_if_current(&status, &generation, my_gen, |s| {
                                    s.tuning = false;
                                    s.connecting = false;
                                    s.local_track_count = 0;
                                    s.source_hint =
                                        "点击下方「探索频段 → 本地磁带」选择你的音频文件夹，当前暂以雨夜炉火陪伴"
                                            .into();
                                });
                            } else if generation.load(Ordering::SeqCst) == my_gen {
                                active_playback = start_local_track_playback(
                                    &local_tracks,
                                    local_idx,
                                    volume,
                                    &audio_level,
                                    &fade_multiplier,
                                    with_static,
                                    &status,
                                );
                            }
                        }
                    }
                }
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        if paused {
            continue;
        }

        // Advance Night-Host / Book-Echo DJ cadence or LocalFolder next track
        if let Some(st) = &current_station {
            match st.kind {
                RadioStationKind::NightHost | RadioStationKind::BookEchoHost => {
                    if let Some(pb) = &active_playback {
                        if host_speaking {
                            let finished = pb.voice_sink.as_ref().is_none_or(|vs| vs.empty());
                            if finished {
                                host_speaking = false;
                                if let Some(h) = &pb.bed_handle {
                                    h.set_ducking(false);
                                }
                                let my_gen = generation.load(Ordering::SeqCst);
                                update_status_if_current(&status, &generation, my_gen, |s| {
                                    s.host_speaking = false
                                });
                                if !night_playlist.is_empty() {
                                    night_idx = (night_idx + 1) % night_playlist.len();
                                }
                                // Peaceful 9-second musical interlude between monologues
                                next_host_speak_at = Some(Instant::now() + Duration::from_secs(9));
                            }
                        } else if let Some(due) = next_host_speak_at
                            && Instant::now() >= due
                            && !night_playlist.is_empty()
                        {
                            next_host_speak_at = None;
                            let seg = night_playlist[night_idx % night_playlist.len()].clone();
                            let my_gen = generation.load(Ordering::SeqCst);
                            update_status_if_current(&status, &generation, my_gen, |s| {
                                s.now_playing_title = seg.title.clone();
                                s.current_line = seg.script.clone();
                                s.source_hint = seg.source_hint.clone();
                            });
                            if let Some(runtime) = &rt
                                && let Ok((audio, engine_label)) = tts::synthesize_for_radio(
                                    runtime,
                                    &seg.script,
                                    &host_voice,
                                    0.90,
                                )
                                && generation.load(Ordering::SeqCst) == my_gen
                                && !paused
                                && let Some(pb) = &active_playback
                                && let Some(vs) = &pb.voice_sink
                            {
                                let decoded_ok = match audio {
                                    tts::SynthAudio::Mp3(bytes) => {
                                        rodio::Decoder::new_mp3(Cursor::new(bytes))
                                            .map(|src| vs.append(src))
                                            .is_ok()
                                    }
                                    tts::SynthAudio::Wav(bytes) => {
                                        rodio::Decoder::new_wav(Cursor::new(bytes))
                                            .map(|src| vs.append(src))
                                            .is_ok()
                                    }
                                };
                                if decoded_ok {
                                    if let Some(h) = &pb.bed_handle {
                                        h.set_ducking(true);
                                    }
                                    host_speaking = true;
                                    update_status_if_current(&status, &generation, my_gen, |s| {
                                        s.host_speaking = true;
                                        s.stream_meta = Some(engine_label.into());
                                    });
                                } else {
                                    next_host_speak_at =
                                        Some(Instant::now() + Duration::from_secs(16));
                                }
                            } else {
                                // Voice synthesis unavailable (e.g. offline without SAPI): keep gentle music + rotate text cards
                                if !night_playlist.is_empty() {
                                    night_idx = (night_idx + 1) % night_playlist.len();
                                }
                                next_host_speak_at = Some(Instant::now() + Duration::from_secs(16));
                            }
                        }
                    }
                }
                RadioStationKind::LocalFolder => {
                    if !local_tracks.is_empty()
                        && active_playback
                            .as_ref()
                            .is_some_and(|pb| pb.primary_sink.empty())
                    {
                        local_idx = (local_idx + 1) % local_tracks.len();
                        if let Some(pb) = active_playback.take() {
                            pb.stop_all();
                        }
                        active_playback = start_local_track_playback(
                            &local_tracks,
                            local_idx,
                            volume,
                            &audio_level,
                            &fade_multiplier,
                            false,
                            &status,
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

fn start_local_track_playback(
    tracks: &[PathBuf],
    idx: usize,
    volume: f32,
    audio_level: &Arc<AtomicU32>,
    fade_multiplier: &Arc<AtomicU32>,
    with_static: bool,
    status: &Arc<Mutex<RadioStatus>>,
) -> Option<ActivePlayback> {
    let path = tracks.get(idx)?;
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("私藏音轨")
        .to_owned();
    let file = fs::File::open(path).ok()?;
    let decoder = rodio::Decoder::new(BufReader::new(file)).ok()?;
    let (stream, handle) = crate::soundscape::try_default_output_stream().ok()?;
    let sink = rodio::Sink::try_new(&handle).ok()?;
    sink.set_volume(volume);
    if with_static {
        sink.append(TuningStaticSource::new(300));
    }
    let source_stop = Arc::new(AtomicBool::new(false));
    let metered = MeteredRadioSource::new(
        decoder.convert_samples::<f32>(),
        Arc::clone(audio_level),
        Arc::clone(fade_multiplier),
        Arc::clone(&source_stop),
    );
    sink.append(metered);
    update_status(status, |s| {
        s.tuning = false;
        s.connecting = false;
        s.local_track_count = tracks.len();
        s.local_track_index = idx;
        s.now_playing_title = title.clone();
        s.current_line = format!("正在播放本地私藏磁带 ·《{title}》");
        s.source_hint = format!("第 {} / {} 首 · 本地无损陪伴", idx + 1, tracks.len());
    });
    Some(ActivePlayback {
        _stream: stream,
        primary_sink: sink,
        voice_sink: None,
        bed_handle: None,
        source_stop,
    })
}

fn fallback_mix_for_station(station_id: &str) -> SoundscapeMix {
    match station_id {
        "deep-space-sleep" => SoundscapeMix {
            rain: 0.0,
            fireplace: 0.0,
            breeze: 0.28,
            cosmos: 0.78,
            train: 0.0,
            piano: 0.34,
            recipe: None,
        },
        "midnight-jazz" => SoundscapeMix::from_recipe("rain_hearth"),
        "classical-nocturne" => SoundscapeMix::from_recipe("music_box"),
        _ => SoundscapeMix::from_recipe("camphor_rain"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_and_custom_stations_sort_and_snap_by_frequency() {
        let custom = vec![CustomRadioStation {
            id: "custom-93".into(),
            name: "雨夜爵士".into(),
            subtitle: "自选流".into(),
            frequency_mhz: 93.0,
            kind: "online".into(),
            stream_url: "https://example.com/jazz.mp3".into(),
            tags: vec!["jazz".into()],
        }];
        let stations = all_stations(&custom);
        assert_eq!(stations.len(), builtin_stations().len() + 1);
        for pair in stations.windows(2) {
            assert!(pair[0].frequency_mhz <= pair[1].frequency_mhz);
        }
        let snapped = nearest_station_for_mhz(&stations, 93.1).unwrap();
        assert_eq!(snapped.id, "custom-93");
        let snapped_night = nearest_station_for_mhz(&stations, 94.4).unwrap();
        assert_eq!(snapped_night.id, "glow-night-talk");
    }

    #[test]
    fn tuning_static_source_produces_gentle_finite_stereo_audio() {
        let samples: Vec<f32> = TuningStaticSource::new(240).collect();
        assert!(samples.len() > 16_000);
        assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 0.35));
        let peak = samples.iter().fold(0.0_f32, |acc, &s| acc.max(s.abs()));
        assert!(peak > 0.02, "static should be softly audible");
    }

    #[test]
    fn seekable_stream_reader_supports_probe_seeks_and_sliding_streaming() {
        let payload: Vec<u8> = (0..120_000usize).map(|i| (i % 251) as u8).collect();
        let stop = Arc::new(AtomicBool::new(false));
        let mut reader = SeekableStreamReader::new(
            Box::new(Cursor::new(payload.clone())),
            Arc::clone(&stop),
            65_536,
        );
        let pre = reader.prebuffer(16_384).unwrap();
        assert_eq!(pre, 16_384);

        // Read first 100 bytes, seek back to 10, verify exact match
        let mut head = [0u8; 100];
        reader.read_exact(&mut head).unwrap();
        assert_eq!(&head[..], &payload[..100]);
        assert_eq!(reader.seek(SeekFrom::Start(10)).unwrap(), 10);
        let mut slice = [0u8; 32];
        reader.read_exact(&mut slice).unwrap();
        assert_eq!(&slice[..], &payload[10..42]);

        // Rewind to 0 and drain entire stream past window limit
        assert_eq!(reader.seek(SeekFrom::Start(0)).unwrap(), 0);
        let mut all = Vec::new();
        reader.read_to_end(&mut all).unwrap();
        assert_eq!(all, payload);
    }

    #[test]
    fn metered_radio_source_updates_energy_and_fades_out_cleanly() {
        let stop = Arc::new(AtomicBool::new(false));
        let level = Arc::new(AtomicU32::new(0.0_f32.to_bits()));
        let fade = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let (bed, _handle) =
            SoundscapeSource::with_mix_handle(&SoundscapeMix::from_recipe("music_box"));
        let mut metered = MeteredRadioSource::new(
            bed,
            Arc::clone(&level),
            Arc::clone(&fade),
            Arc::clone(&stop),
        );
        for _ in 0..8_192 {
            assert!(metered.next().is_some());
        }
        let energy = f32::from_bits(level.load(Ordering::Relaxed));
        assert!(energy > 0.01 && energy <= 1.0);

        stop.store(true, Ordering::Relaxed);
        let remaining: Vec<f32> = metered.take(40_000).collect();
        assert!(
            remaining.len() < 40_000,
            "metered source must end after fade-out"
        );
        assert_eq!(f32::from_bits(level.load(Ordering::Relaxed)), 0.0);
    }

    #[test]
    fn station_scene_asset_and_initial_copy_are_deterministic_and_non_empty() {
        for st in builtin_stations() {
            let asset = station_scene_asset(&st.id);
            assert!(asset.ends_with(".png"), "expected png scene for {}", st.id);
            let (title, line, hint) = initial_copy_for_station(&st, &[]);
            assert!(!title.is_empty());
            assert!(!line.is_empty());
            assert!(!hint.is_empty());
        }
        assert_eq!(
            station_scene_asset("custom-my-jazz"),
            station_scene_asset("custom-my-jazz")
        );
        // NightHost resolves directly to its first monologue letter synchronously
        let night = station_by_id(&builtin_stations(), "glow-night-talk");
        let (title, line, _) = initial_copy_for_station(&night, &[]);
        let first = curated_night_talk_segments().remove(0);
        assert_eq!(title, first.title);
        assert_eq!(line, first.script);
    }

    #[test]
    fn radio_player_tune_pause_stop_update_status_synchronously_without_stale_revert() {
        let player = RadioPlayer::new();
        let stations = builtin_stations();
        let synth = station_by_id(&stations, "offline-piano-rain");
        let night = station_by_id(&stations, "glow-night-talk");

        player.tune(
            synth.clone(),
            0.5,
            tts::VOICES[0].id.into(),
            vec![],
            None,
            false,
        );
        assert_eq!(player.status().station_id, "offline-piano-rain");
        assert!(player.status().playing);

        // Rapidly switch to another station: status must immediately reflect the new station
        player.tune(
            night.clone(),
            0.5,
            tts::VOICES[0].id.into(),
            vec![],
            None,
            false,
        );
        let st = player.status();
        assert_eq!(st.station_id, "glow-night-talk");
        assert!((st.frequency_mhz - 94.2).abs() < 0.05);
        assert!(st.playing);
        assert!(!st.paused);

        player.pause();
        assert!(player.status().paused);
        player.resume();
        assert!(!player.status().paused);
        player.stop();
        assert!(!player.status().playing);
    }
}

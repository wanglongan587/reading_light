use anyhow::{Result, anyhow};
use glow_core::book_context::TtsSegment;
use kothok_edge_tts::{EdgeTts, Engine as _, TtsEvent};
use std::{
    io::Cursor,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
pub struct VoiceOption {
    pub id: &'static str,
    pub label: &'static str,
    pub style: &'static str,
    pub lang: &'static str,
}

pub const VOICES: &[VoiceOption] = &[
    VoiceOption {
        id: "zh-CN-XiaoxiaoNeural",
        label: "晓晓",
        style: "温柔治愈女声",
        lang: "zh-CN",
    },
    VoiceOption {
        id: "zh-CN-YunxiNeural",
        label: "云希",
        style: "清朗少年男声",
        lang: "zh-CN",
    },
    VoiceOption {
        id: "zh-CN-XiaoyiNeural",
        label: "晓伊",
        style: "细腻娓娓女声",
        lang: "zh-CN",
    },
    VoiceOption {
        id: "zh-CN-YunjianNeural",
        label: "云健",
        style: "沉稳说书男声",
        lang: "zh-CN",
    },
    VoiceOption {
        id: "zh-CN-liaoning-XiaobeiNeural",
        label: "晓北",
        style: "亲切自然女声",
        lang: "zh-CN",
    },
    VoiceOption {
        id: "en-US-EmmaMultilingualNeural",
        label: "Emma",
        style: "多语种自然女声",
        lang: "en-US",
    },
];

pub fn voice_by_id(id: &str) -> VoiceOption {
    VOICES
        .iter()
        .copied()
        .find(|v| v.id == id)
        .unwrap_or(VOICES[0])
}

pub fn format_edge_rate(rate: f32) -> String {
    let pct = ((rate.clamp(0.5, 2.0) - 1.0) * 100.0).round() as i32;
    if pct >= 0 {
        format!("+{pct}%")
    } else {
        format!("{pct}%")
    }
}

#[derive(Clone, Debug, Default)]
pub struct TtsStatus {
    pub active: bool,
    pub paused: bool,
    pub loading: bool,
    pub segment_index: usize,
    pub total_segments: usize,
    pub chapter_title: String,
    pub current_text: String,
    pub engine_label: String,
    pub error: Option<String>,
}

pub(crate) enum SynthAudio {
    Mp3(Vec<u8>),
    Wav(Vec<u8>),
}

pub(crate) fn synthesize_for_radio(
    rt: &tokio::runtime::Runtime,
    text: &str,
    voice: &str,
    rate: f32,
) -> Result<(SynthAudio, &'static str)> {
    synthesize_with_fallback(rt, text, voice, rate)
}

enum TtsCommand {
    Start {
        segments: Vec<TtsSegment>,
        start_index: usize,
        voice: String,
        rate: f32,
        volume: f32,
    },
    Pause,
    Resume,
    Stop,
    Jump(usize),
    #[allow(dead_code)]
    SetVolume(f32),
}

pub struct TtsPlayer {
    cmd_tx: mpsc::Sender<TtsCommand>,
    status: Arc<Mutex<TtsStatus>>,
    generation: Arc<AtomicU64>,
    stop_signal: Arc<AtomicBool>,
}

impl TtsPlayer {
    pub fn new() -> Self {
        kothok_edge_tts::init_tls();
        let (cmd_tx, cmd_rx) = mpsc::channel::<TtsCommand>();
        let status = Arc::new(Mutex::new(TtsStatus::default()));
        let generation = Arc::new(AtomicU64::new(0));
        let stop_signal = Arc::new(AtomicBool::new(false));

        let status_bg = Arc::clone(&status);
        let gen_bg = Arc::clone(&generation);
        let stop_bg = Arc::clone(&stop_signal);

        std::thread::Builder::new()
            .name("glow-tts".into())
            .spawn(move || {
                tts_worker_loop(cmd_rx, status_bg, gen_bg, stop_bg);
            })
            .expect("spawn glow-tts thread");

        Self {
            cmd_tx,
            status,
            generation,
            stop_signal,
        }
    }

    pub fn status(&self) -> TtsStatus {
        self.status
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn start(
        &self,
        segments: Vec<TtsSegment>,
        start_index: usize,
        voice: String,
        rate: f32,
        volume: f32,
    ) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.stop_signal.store(false, Ordering::SeqCst);
        let _ = self.cmd_tx.send(TtsCommand::Start {
            segments,
            start_index,
            voice,
            rate,
            volume,
        });
    }

    pub fn pause(&self) {
        let _ = self.cmd_tx.send(TtsCommand::Pause);
    }

    pub fn resume(&self) {
        let _ = self.cmd_tx.send(TtsCommand::Resume);
    }

    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.stop_signal.store(true, Ordering::SeqCst);
        let _ = self.cmd_tx.send(TtsCommand::Stop);
    }

    pub fn jump(&self, index: usize) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.stop_signal.store(false, Ordering::SeqCst);
        let _ = self.cmd_tx.send(TtsCommand::Jump(index));
    }

    #[allow(dead_code)]
    pub fn set_volume(&self, volume: f32) {
        let _ = self.cmd_tx.send(TtsCommand::SetVolume(volume.clamp(0.05, 1.0)));
    }
}

impl Drop for TtsPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn update_status(status: &Arc<Mutex<TtsStatus>>, edit: impl FnOnce(&mut TtsStatus)) {
    if let Ok(mut guard) = status.lock() {
        edit(&mut guard);
    }
}

fn synthesize_edge_sync(
    rt: &tokio::runtime::Runtime,
    text: &str,
    voice: &str,
    rate: f32,
) -> Result<SynthAudio> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let opt = voice_by_id(voice);
    let rate_str = format_edge_rate(rate);
    let events = rt
        .block_on(async {
            tokio::time::timeout(
                Duration::from_secs(18),
                EdgeTts.synthesize(text, opt.id, &rate_str, opt.lang),
            )
            .await
        })
        .map_err(|_| anyhow!("Edge-TTS 合成超时"))?
        .map_err(|e| anyhow!("Edge-TTS 连接失败：{e}"))?;

    let mut mp3 = Vec::new();
    for event in events {
        if let TtsEvent::Audio(chunk) = event {
            mp3.extend_from_slice(&chunk);
        }
    }
    if mp3.is_empty() {
        return Err(anyhow!("Edge-TTS 未返回音频数据"));
    }
    Ok(SynthAudio::Mp3(mp3))
}

#[cfg(target_os = "windows")]
fn synthesize_windows_sapi_fallback(text: &str, rate: f32) -> Result<SynthAudio> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let wav_path =
        std::env::temp_dir().join(format!("glow-tts-{}-{stamp}.wav", std::process::id()));
    let sapi_rate = (((rate - 1.0) * 6.0).round() as i32).clamp(-6, 6);
    let script = r#"
Add-Type -AssemblyName System.Speech
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
$synth.Rate = [int]$env:GLOW_TTS_RATE
$synth.SetOutputToWaveFile($env:GLOW_TTS_WAV)
$synth.Speak($env:GLOW_TTS_TEXT)
$synth.Dispose()
"#;
    let status = std::process::Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("GLOW_TTS_RATE", sapi_rate.to_string())
        .env("GLOW_TTS_WAV", &wav_path)
        .env("GLOW_TTS_TEXT", text)
        .status();
    let bytes_res = status
        .map_err(anyhow::Error::from)
        .and_then(|st| {
            if !st.success() {
                Err(anyhow!("Windows 本地语音合成不可用"))
            } else {
                std::fs::read(&wav_path).map_err(anyhow::Error::from)
            }
        });
    let _ = std::fs::remove_file(&wav_path);
    let bytes = bytes_res?;
    if bytes.len() < 64 {
        return Err(anyhow!("本地语音未生成音频"));
    }
    Ok(SynthAudio::Wav(bytes))
}

#[cfg(not(target_os = "windows"))]
fn synthesize_windows_sapi_fallback(_text: &str, _rate: f32) -> Result<SynthAudio> {
    Err(anyhow!("当前平台暂未配置离线语音引擎"))
}

fn synthesize_with_fallback(
    rt: &tokio::runtime::Runtime,
    text: &str,
    voice: &str,
    rate: f32,
) -> Result<(SynthAudio, &'static str)> {
    match synthesize_edge_sync(rt, text, voice, rate) {
        Ok(audio) => Ok((audio, "Edge 神经语音")),
        Err(edge_err) => match synthesize_windows_sapi_fallback(text, rate) {
            Ok(wav) => Ok((wav, "系统离线语音（Edge 暂不可达）")),
            Err(_) => Err(edge_err),
        },
    }
}

fn tts_worker_loop(
    cmd_rx: mpsc::Receiver<TtsCommand>,
    status: Arc<Mutex<TtsStatus>>,
    generation: Arc<AtomicU64>,
    stop_signal: Arc<AtomicBool>,
) {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        update_status(&status, |s| {
            s.error = Some("无法初始化听书运行时".into());
        });
        return;
    };

    let mut segments: Vec<TtsSegment> = Vec::new();
    let mut voice = VOICES[0].id.to_owned();
    let mut rate = 1.0_f32;
    let mut volume = 0.9_f32;
    let mut current_idx = 0usize;
    let mut active = false;
    let mut paused = false;
    let mut active_sink: Option<(rodio::OutputStream, rodio::Sink)> = None;
    let mut prefetched: Option<(usize, Result<(SynthAudio, &'static str)>)> = None;

    loop {
        // Drain pending commands first
        let recv_result = if active && !paused {
            cmd_rx.recv_timeout(Duration::from_millis(60))
        } else {
            cmd_rx
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };

        match recv_result {
            Ok(cmd) => match cmd {
                TtsCommand::Start {
                    segments: next_segs,
                    start_index,
                    voice: next_voice,
                    rate: next_rate,
                    volume: next_vol,
                } => {
                    if let Some((_, sink)) = active_sink.take() {
                        sink.stop();
                    }
                    segments = next_segs;
                    voice = next_voice;
                    rate = next_rate.clamp(0.5, 2.0);
                    volume = next_vol.clamp(0.05, 1.0);
                    current_idx = start_index.min(segments.len().saturating_sub(1));
                    active = !segments.is_empty();
                    paused = false;
                    prefetched = None;
                    update_status(&status, |s| {
                        s.active = active;
                        s.paused = false;
                        s.loading = active;
                        s.segment_index = current_idx;
                        s.total_segments = segments.len();
                        s.error = None;
                        if let Some(seg) = segments.get(current_idx) {
                            s.chapter_title = seg.chapter_title.clone();
                            s.current_text = seg.text.clone();
                        }
                    });
                }
                TtsCommand::Pause => {
                    paused = true;
                    if let Some((_, sink)) = &active_sink {
                        sink.pause();
                    }
                    update_status(&status, |s| s.paused = true);
                }
                TtsCommand::Resume => {
                    paused = false;
                    if let Some((_, sink)) = &active_sink {
                        sink.play();
                    }
                    update_status(&status, |s| s.paused = false);
                }
                TtsCommand::Stop => {
                    active = false;
                    paused = false;
                    prefetched = None;
                    if let Some((_, sink)) = active_sink.take() {
                        sink.stop();
                    }
                    update_status(&status, |s| {
                        s.active = false;
                        s.paused = false;
                        s.loading = false;
                    });
                }
                TtsCommand::Jump(idx) => {
                    if !segments.is_empty() {
                        if let Some((_, sink)) = active_sink.take() {
                            sink.stop();
                        }
                        current_idx = idx.min(segments.len() - 1);
                        active = true;
                        paused = false;
                        if prefetched.as_ref().is_some_and(|(i, _)| *i != current_idx) {
                            prefetched = None;
                        }
                        update_status(&status, |s| {
                            s.active = true;
                            s.paused = false;
                            s.loading = true;
                            s.segment_index = current_idx;
                            s.error = None;
                            if let Some(seg) = segments.get(current_idx) {
                                s.chapter_title = seg.chapter_title.clone();
                                s.current_text = seg.text.clone();
                            }
                        });
                    }
                }
                TtsCommand::SetVolume(vol) => {
                    volume = vol;
                    if let Some((_, sink)) = &active_sink {
                        sink.set_volume(volume);
                    }
                }
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        if !active || paused {
            continue;
        }

        // If a segment is currently playing, check whether it finished
        if let Some((_, sink)) = &active_sink {
            if !sink.empty() {
                // Opportunistically prefetch next segment on a helper thread? Keep simple & deterministic:
                continue;
            }
            active_sink = None;
            current_idx += 1;
            if current_idx >= segments.len() {
                active = false;
                update_status(&status, |s| {
                    s.active = false;
                    s.paused = false;
                    s.loading = false;
                });
                continue;
            }
        }

        // Start playing `current_idx`
        let Some(seg) = segments.get(current_idx).cloned() else {
            active = false;
            continue;
        };
        let my_gen = generation.load(Ordering::SeqCst);
        update_status(&status, |s| {
            s.active = true;
            s.loading = true;
            s.segment_index = current_idx;
            s.total_segments = segments.len();
            s.chapter_title = seg.chapter_title.clone();
            s.current_text = seg.text.clone();
            s.error = None;
        });

        let synth_outcome = if let Some((idx, res)) = prefetched.take()
            && idx == current_idx
        {
            res
        } else {
            synthesize_with_fallback(&rt, &seg.text, &voice, rate)
        };

        // Abort cleanly if user stopped or jumped while synthesizing
        if stop_signal.load(Ordering::SeqCst) || generation.load(Ordering::SeqCst) != my_gen {
            continue;
        }

        match synth_outcome {
            Ok((audio, engine_label)) => {
                match rodio::OutputStream::try_default()
                    .map_err(anyhow::Error::from)
                    .and_then(|(stream, handle)| {
                        rodio::Sink::try_new(&handle)
                            .map_err(anyhow::Error::from)
                            .map(|sink| (stream, sink))
                    })
                {
                    Ok((stream, sink)) => {
                        let decode_res = match audio {
                            SynthAudio::Mp3(bytes) => {
                                rodio::Decoder::new_mp3(Cursor::new(bytes))
                                    .map(|src| sink.append(src))
                            }
                            SynthAudio::Wav(bytes) => {
                                rodio::Decoder::new_wav(Cursor::new(bytes))
                                    .map(|src| sink.append(src))
                            }
                        };
                        match decode_res {
                            Ok(()) => {
                                sink.set_volume(volume);
                                active_sink = Some((stream, sink));
                                update_status(&status, |s| {
                                    s.loading = false;
                                    s.engine_label = engine_label.into();
                                    s.error = None;
                                });
                            }
                            Err(e) => {
                                active = false;
                                update_status(&status, |s| {
                                    s.active = false;
                                    s.loading = false;
                                    s.error = Some(format!("音频解码失败：{e}"));
                                });
                            }
                        }
                    }
                    Err(_) => {
                        active = false;
                        update_status(&status, |s| {
                            s.active = false;
                            s.loading = false;
                            s.error = Some("未检测到可用的音频输出设备".into());
                        });
                    }
                }
            }
            Err(err) => {
                active = false;
                update_status(&status, |s| {
                    s.active = false;
                    s.loading = false;
                    s.error = Some(err.to_string());
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_string_and_voice_lookup_are_well_formed() {
        assert_eq!(format_edge_rate(1.0), "+0%");
        assert_eq!(format_edge_rate(1.25), "+25%");
        assert_eq!(format_edge_rate(0.75), "-25%");
        assert_eq!(voice_by_id("zh-CN-YunxiNeural").label, "云希");
        assert_eq!(voice_by_id("unknown-voice").id, "zh-CN-XiaoxiaoNeural");
    }
}

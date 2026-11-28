use anyhow::{Result, anyhow, ensure};
use glow_core::{AiSettings, LocalSecrets, TtsSettings, book_context::TtsSegment};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
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
    pub openai_voice: &'static str,
    pub label: &'static str,
    pub style: &'static str,
    #[allow(dead_code)]
    pub lang: &'static str,
}

pub const VOICES: &[VoiceOption] = &[
    VoiceOption {
        id: "zh-CN-XiaoxiaoNeural",
        openai_voice: "nova",
        label: "晓晓",
        style: "温柔治愈女声",
        lang: "zh-CN",
    },
    VoiceOption {
        id: "zh-CN-YunxiNeural",
        openai_voice: "onyx",
        label: "云希",
        style: "清朗少年男声",
        lang: "zh-CN",
    },
    VoiceOption {
        id: "zh-CN-XiaoyiNeural",
        openai_voice: "shimmer",
        label: "晓伊",
        style: "细腻娓娓女声",
        lang: "zh-CN",
    },
    VoiceOption {
        id: "zh-CN-YunjianNeural",
        openai_voice: "echo",
        label: "云健",
        style: "沉稳说书男声",
        lang: "zh-CN",
    },
    VoiceOption {
        id: "zh-CN-liaoning-XiaobeiNeural",
        openai_voice: "alloy",
        label: "晓北",
        style: "亲切自然女声",
        lang: "zh-CN",
    },
    VoiceOption {
        id: "en-US-EmmaMultilingualNeural",
        openai_voice: "fable",
        label: "Emma",
        style: "多语种自然女声",
        lang: "en-US",
    },
];

pub fn voice_by_id(id: &str) -> VoiceOption {
    VOICES
        .iter()
        .copied()
        .find(|v| v.id == id || v.openai_voice == id)
        .unwrap_or(VOICES[0])
}

pub fn format_rate_percent(rate: f32) -> String {
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

#[derive(Clone, Debug)]
pub struct TtsRuntimeConfig {
    pub provider: String,
    pub api_base_url: String,
    pub model: String,
    pub api_key: String,
    pub cache_dir: Option<PathBuf>,
}

impl Default for TtsRuntimeConfig {
    fn default() -> Self {
        Self {
            provider: "auto".into(),
            api_base_url: String::new(),
            model: "tts-1".into(),
            api_key: String::new(),
            cache_dir: None,
        }
    }
}

impl TtsRuntimeConfig {
    pub fn from_store(
        tts: &TtsSettings,
        ai: &AiSettings,
        secrets: &LocalSecrets,
        data_root: &Path,
    ) -> Self {
        let base_url = if !tts.api_base_url.trim().is_empty() {
            tts.api_base_url.trim().to_owned()
        } else if !secrets.ai_api_key.trim().is_empty() || ai.is_local() {
            ai.base_url.trim().to_owned()
        } else {
            String::new()
        };
        let model = if tts.model.trim().is_empty() {
            "tts-1".to_owned()
        } else {
            tts.model.trim().to_owned()
        };
        Self {
            provider: tts.provider.trim().to_ascii_lowercase(),
            api_base_url: base_url,
            model,
            api_key: secrets.ai_api_key.trim().to_owned(),
            cache_dir: Some(data_root.join("tts_cache")),
        }
    }

    fn resolved_speech_endpoint(&self) -> Option<String> {
        let trimmed = self.api_base_url.trim().trim_end_matches('/');
        if trimmed.is_empty()
            || !(trimmed.starts_with("http://") || trimmed.starts_with("https://"))
            || trimmed.chars().any(char::is_whitespace)
        {
            return None;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.ends_with("/audio/speech") {
            return Some(trimmed.to_owned());
        }
        if lower.ends_with("/chat/completions") {
            let base = &trimmed[..trimmed.len() - "/chat/completions".len()];
            return Some(format!("{base}/audio/speech"));
        }
        if lower.ends_with("/responses") {
            let base = &trimmed[..trimmed.len() - "/responses".len()];
            return Some(format!("{base}/audio/speech"));
        }
        if lower.ends_with("/v1") {
            Some(format!("{trimmed}/audio/speech"))
        } else {
            Some(format!("{trimmed}/v1/audio/speech"))
        }
    }

    fn is_local_endpoint(&self) -> bool {
        let lower = self.api_base_url.trim().to_ascii_lowercase();
        let host = lower
            .strip_prefix("http://")
            .or_else(|| lower.strip_prefix("https://"))
            .unwrap_or(&lower)
            .split(['/', '?'])
            .next()
            .unwrap_or("");
        let host_no_port = if host.starts_with('[') {
            host.split(']')
                .next()
                .unwrap_or(host)
                .trim_start_matches('[')
        } else {
            host.split(':').next().unwrap_or(host)
        };
        matches!(host_no_port, "localhost" | "127.0.0.1" | "::1" | "0.0.0.0")
            || host_no_port.starts_with("192.168.")
            || host_no_port.starts_with("10.")
    }

    fn can_use_api(&self) -> bool {
        self.resolved_speech_endpoint().is_some()
            && (!self.api_key.is_empty() || self.is_local_endpoint())
    }
}

#[derive(Clone, Debug)]
pub struct TtsRequest<'a> {
    pub text: &'a str,
    pub voice: &'a str,
    pub rate: f32,
}

#[derive(Clone, Debug)]
pub(crate) enum SynthAudio {
    Mp3(Vec<u8>),
    Wav(Vec<u8>),
}

/// Pluggable compliant speech synthesis provider trait.
/// Replaces unofficial reverse-engineered TTS libraries with explicit offline + licensed API backends.
pub(crate) trait TtsProvider: Send + Sync {
    fn label(&self) -> &'static str;
    fn synthesize(&self, req: &TtsRequest<'_>) -> Result<SynthAudio>;
}

/// Compliant OpenAI-compatible `/audio/speech` provider with persistent SHA-256 disk caching.
/// Works seamlessly with licensed cloud providers (OpenAI, SiliconFlow, Volcano/Aliyun gateways)
/// as well as local offline neural servers (Kokoro-FastAPI, Sherpa-ONNX HTTP).
pub(crate) struct OpenAiCompatibleTtsProvider {
    endpoint: String,
    model: String,
    api_key: String,
    is_local: bool,
    cache_dir: Option<PathBuf>,
}

impl OpenAiCompatibleTtsProvider {
    pub(crate) fn from_config(cfg: &TtsRuntimeConfig) -> Option<Self> {
        if !cfg.can_use_api() {
            return None;
        }
        Some(Self {
            endpoint: cfg.resolved_speech_endpoint()?,
            model: cfg.model.clone(),
            api_key: cfg.api_key.clone(),
            is_local: cfg.is_local_endpoint(),
            cache_dir: cfg.cache_dir.clone(),
        })
    }

    fn cache_key(&self, req: &TtsRequest<'_>, voice: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.model.as_bytes());
        hasher.update(b"|");
        hasher.update(voice.as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:.2}", req.rate.clamp(0.5, 2.0)).as_bytes());
        hasher.update(b"|");
        hasher.update(req.text.trim().as_bytes());
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for b in digest {
            use std::fmt::Write as _;
            let _ = write!(&mut hex, "{b:02x}");
        }
        hex
    }
}

impl TtsProvider for OpenAiCompatibleTtsProvider {
    fn label(&self) -> &'static str {
        if self.is_local {
            "本地神经语音服务"
        } else {
            "合规云端神经语音"
        }
    }

    fn synthesize(&self, req: &TtsRequest<'_>) -> Result<SynthAudio> {
        let text = req.text.trim();
        ensure!(!text.is_empty(), "待朗读文本为空");
        let opt = voice_by_id(req.voice);
        let speed = req.rate.clamp(0.5, 2.0);
        let cache_path = self
            .cache_dir
            .as_ref()
            .map(|dir| dir.join(format!("{}.mp3", self.cache_key(req, opt.openai_voice))));

        if let Some(path) = &cache_path
            && let Ok(cached) = fs::read(path)
            && cached.len() > 128
        {
            return Ok(SynthAudio::Mp3(cached));
        }

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(24))
            .user_agent("GlowReading/2.0")
            .build()
            .map_err(|e| anyhow!("无法初始化语音客户端：{e}"))?;

        let payload = json!({
            "model": self.model,
            "input": text,
            "voice": opt.openai_voice,
            "speed": speed,
            "response_format": "mp3"
        });
        let mut builder = client.post(&self.endpoint).json(&payload);
        if !self.api_key.is_empty() {
            builder = builder.bearer_auth(&self.api_key);
        }
        let resp = builder
            .send()
            .map_err(|e| anyhow!("合规语音接口连接失败：{e}"))?;
        if !resp.status().is_success() {
            return Err(anyhow!("语音服务返回状态码 {}", resp.status().as_u16()));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| anyhow!("读取语音音频流失败：{e}"))?
            .to_vec();
        ensure!(bytes.len() > 128, "语音接口未返回有效音频数据");

        if let Some(path) = &cache_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(path, &bytes);
        }
        Ok(SynthAudio::Mp3(bytes))
    }
}

/// 100% offline zero-cost native OS speech synthesis fallback.
pub(crate) struct SystemNativeTtsProvider;

impl TtsProvider for SystemNativeTtsProvider {
    fn label(&self) -> &'static str {
        "本机离线系统语音"
    }

    fn synthesize(&self, req: &TtsRequest<'_>) -> Result<SynthAudio> {
        synthesize_system_native(req.text, req.rate)
    }
}

fn synthesize_with_providers(
    req: &TtsRequest<'_>,
    primary: Option<&dyn TtsProvider>,
    fallback: &dyn TtsProvider,
) -> Result<(SynthAudio, &'static str)> {
    if let Some(p) = primary {
        match p.synthesize(req) {
            Ok(audio) => return Ok((audio, p.label())),
            Err(primary_err) => {
                if let Ok(audio) = fallback.synthesize(req) {
                    return Ok((audio, fallback.label()));
                }
                return Err(primary_err);
            }
        }
    }
    let audio = fallback.synthesize(req)?;
    Ok((audio, fallback.label()))
}

fn synthesize_with_config(
    cfg: &TtsRuntimeConfig,
    text: &str,
    voice: &str,
    rate: f32,
) -> Result<(SynthAudio, &'static str)> {
    let req = TtsRequest { text, voice, rate };
    let system = SystemNativeTtsProvider;
    if cfg.provider == "system" {
        return synthesize_with_providers(&req, None, &system);
    }
    let api_provider = OpenAiCompatibleTtsProvider::from_config(cfg);
    if cfg.provider == "openai_compatible" {
        let Some(api) = api_provider else {
            return Err(anyhow!(
                "尚未配置合规语音接口（请在「主题与设置」填写本地或云端语音服务地址）"
            ));
        };
        return api.synthesize(&req).map(|a| (a, api.label()));
    }
    synthesize_with_providers(
        &req,
        api_provider.as_ref().map(|p| p as &dyn TtsProvider),
        &system,
    )
}

pub(crate) fn synthesize_for_radio(
    cfg: &TtsRuntimeConfig,
    text: &str,
    voice: &str,
    rate: f32,
) -> Result<(SynthAudio, &'static str)> {
    synthesize_with_config(cfg, text, voice, rate)
}

#[cfg(target_os = "windows")]
fn synthesize_system_native(text: &str, rate: f32) -> Result<SynthAudio> {
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
    let bytes_res = status.map_err(anyhow::Error::from).and_then(|st| {
        if !st.success() {
            Err(anyhow!("Windows 本机离线语音合成不可用"))
        } else {
            fs::read(&wav_path).map_err(anyhow::Error::from)
        }
    });
    let _ = fs::remove_file(&wav_path);
    let bytes = bytes_res?;
    ensure!(bytes.len() >= 64, "本机离线语音未生成有效音频");
    Ok(SynthAudio::Wav(bytes))
}

#[cfg(target_os = "macos")]
fn synthesize_system_native(text: &str, rate: f32) -> Result<SynthAudio> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let aiff_path =
        std::env::temp_dir().join(format!("glow-tts-{}-{stamp}.aiff", std::process::id()));
    let wav_path =
        std::env::temp_dir().join(format!("glow-tts-{}-{stamp}.wav", std::process::id()));
    let wpm = (180.0 * rate.clamp(0.5, 2.0)).round() as u32;
    let say_ok = std::process::Command::new("say")
        .args(["-r", &wpm.to_string(), "-o"])
        .arg(&aiff_path)
        .arg(text)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let convert_ok = say_ok
        && std::process::Command::new("afconvert")
            .args(["-f", "WAVE", "-d", "LEI16"])
            .arg(&aiff_path)
            .arg(&wav_path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    let bytes = if convert_ok {
        fs::read(&wav_path).ok()
    } else {
        None
    };
    let _ = fs::remove_file(&aiff_path);
    let _ = fs::remove_file(&wav_path);
    let bytes = bytes.ok_or_else(|| anyhow!("macOS 本机离线语音合成不可用"))?;
    ensure!(bytes.len() >= 64, "本机离线语音未生成有效音频");
    Ok(SynthAudio::Wav(bytes))
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn synthesize_system_native(text: &str, rate: f32) -> Result<SynthAudio> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let wav_path =
        std::env::temp_dir().join(format!("glow-tts-{}-{stamp}.wav", std::process::id()));
    let wpm = (165.0 * rate.clamp(0.5, 2.0)).round() as u32;
    let mut synthesized = false;
    for bin in ["espeak-ng", "espeak"] {
        if let Ok(st) = std::process::Command::new(bin)
            .args(["-s", &wpm.to_string(), "-w"])
            .arg(&wav_path)
            .arg(text)
            .status()
            && st.success()
        {
            synthesized = true;
            break;
        }
    }
    let bytes = if synthesized {
        fs::read(&wav_path).ok()
    } else {
        None
    };
    let _ = fs::remove_file(&wav_path);
    let bytes = bytes.ok_or_else(|| {
        anyhow!("未检测到本机离线语音引擎（Linux 可安装 espeak-ng，或在设置中配置合规云端/本地语音服务）")
    })?;
    ensure!(bytes.len() >= 64, "本机离线语音未生成有效音频");
    Ok(SynthAudio::Wav(bytes))
}

enum TtsCommand {
    Start {
        segments: Vec<TtsSegment>,
        start_index: usize,
        voice: String,
        rate: f32,
        volume: f32,
        config: TtsRuntimeConfig,
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
        let _ = rustls::crypto::ring::default_provider().install_default();
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
        config: TtsRuntimeConfig,
    ) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.stop_signal.store(false, Ordering::SeqCst);
        let _ = self.cmd_tx.send(TtsCommand::Start {
            segments,
            start_index,
            voice,
            rate,
            volume,
            config,
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
        let _ = self
            .cmd_tx
            .send(TtsCommand::SetVolume(volume.clamp(0.05, 1.0)));
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

fn tts_worker_loop(
    cmd_rx: mpsc::Receiver<TtsCommand>,
    status: Arc<Mutex<TtsStatus>>,
    generation: Arc<AtomicU64>,
    stop_signal: Arc<AtomicBool>,
) {
    let mut segments: Vec<TtsSegment> = Vec::new();
    let mut voice = VOICES[0].id.to_owned();
    let mut rate = 1.0_f32;
    let mut volume = 0.9_f32;
    let mut config = TtsRuntimeConfig::default();
    let mut current_idx = 0usize;
    let mut active = false;
    let mut paused = false;
    let mut active_sink: Option<(rodio::OutputStream, rodio::Sink)> = None;

    loop {
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
                    config: next_config,
                } => {
                    if let Some((_, sink)) = active_sink.take() {
                        sink.stop();
                    }
                    segments = next_segs;
                    voice = next_voice;
                    rate = next_rate.clamp(0.5, 2.0);
                    volume = next_vol.clamp(0.05, 1.0);
                    config = next_config;
                    current_idx = start_index.min(segments.len().saturating_sub(1));
                    active = !segments.is_empty();
                    paused = false;
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

        if let Some((_, sink)) = &active_sink {
            if !sink.empty() {
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

        let synth_outcome = synthesize_with_config(&config, &seg.text, &voice, rate);

        if stop_signal.load(Ordering::SeqCst) || generation.load(Ordering::SeqCst) != my_gen {
            continue;
        }

        match synth_outcome {
            Ok((audio, engine_label)) => {
                match crate::soundscape::try_default_output_stream()
                    .map_err(anyhow::Error::from)
                    .and_then(|(stream, handle)| {
                        rodio::Sink::try_new(&handle)
                            .map_err(anyhow::Error::from)
                            .map(|sink| (stream, sink))
                    }) {
                    Ok((stream, sink)) => {
                        let decode_res = match audio {
                            SynthAudio::Mp3(bytes) => rodio::Decoder::new_mp3(Cursor::new(bytes))
                                .map(|src| sink.append(src)),
                            SynthAudio::Wav(bytes) => rodio::Decoder::new_wav(Cursor::new(bytes))
                                .map(|src| sink.append(src)),
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

    struct FailingProvider;
    impl TtsProvider for FailingProvider {
        fn label(&self) -> &'static str {
            "failing-cloud"
        }
        fn synthesize(&self, _req: &TtsRequest<'_>) -> Result<SynthAudio> {
            Err(anyhow!("network offline"))
        }
    }

    struct StubOfflineProvider;
    impl TtsProvider for StubOfflineProvider {
        fn label(&self) -> &'static str {
            "offline-stub"
        }
        fn synthesize(&self, _req: &TtsRequest<'_>) -> Result<SynthAudio> {
            Ok(SynthAudio::Wav(vec![0u8; 128]))
        }
    }

    #[test]
    fn rate_string_and_voice_lookup_are_well_formed() {
        assert_eq!(format_rate_percent(1.0), "+0%");
        assert_eq!(format_rate_percent(1.25), "+25%");
        assert_eq!(format_rate_percent(0.75), "-25%");
        assert_eq!(voice_by_id("zh-CN-YunxiNeural").label, "云希");
        assert_eq!(voice_by_id("onyx").label, "云希");
        assert_eq!(voice_by_id("unknown-voice").id, "zh-CN-XiaoxiaoNeural");
    }

    #[test]
    fn provider_fallback_and_endpoint_resolution_and_disk_cache_work() {
        let req = TtsRequest {
            text: "今晚慢慢来，不必着急。",
            voice: "zh-CN-XiaoxiaoNeural",
            rate: 1.0,
        };
        let (audio, label) =
            synthesize_with_providers(&req, Some(&FailingProvider), &StubOfflineProvider).unwrap();
        assert_eq!(label, "offline-stub");
        assert!(matches!(audio, SynthAudio::Wav(_)));

        let dir = tempfile::tempdir().unwrap();
        let cfg = TtsRuntimeConfig {
            provider: "auto".into(),
            api_base_url: "http://127.0.0.1:8880/v1".into(),
            model: "kokoro".into(),
            api_key: String::new(),
            cache_dir: Some(dir.path().to_path_buf()),
        };
        assert_eq!(
            cfg.resolved_speech_endpoint().as_deref(),
            Some("http://127.0.0.1:8880/v1/audio/speech")
        );
        let provider = OpenAiCompatibleTtsProvider::from_config(&cfg).unwrap();
        assert_eq!(provider.label(), "本地神经语音服务");

        // Pre-populate disk cache and verify cache hit returns without hitting network
        let key = provider.cache_key(&req, voice_by_id(req.voice).openai_voice);
        let fake_mp3 = vec![0xFFu8; 256];
        fs::write(dir.path().join(format!("{key}.mp3")), &fake_mp3).unwrap();
        let cached = provider.synthesize(&req).unwrap();
        match cached {
            SynthAudio::Mp3(bytes) => assert_eq!(bytes, fake_mp3),
            SynthAudio::Wav(_) => panic!("expected cached mp3"),
        }
    }
}

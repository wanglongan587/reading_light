use glow_core::SoundscapeMix;
use rodio::Source;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};

const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
const FADE_IN_FRAMES: f64 = (SAMPLE_RATE as f64) * 0.35;
const FADE_OUT_FRAMES: u32 = (SAMPLE_RATE * 260) / 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundscapePreset {
    Rain,
    Fireplace,
    Breeze,
    Cosmos,
    Train,
    Piano,
}

impl SoundscapePreset {
    pub const ALL: [Self; 6] = [
        Self::Rain,
        Self::Fireplace,
        Self::Breeze,
        Self::Cosmos,
        Self::Train,
        Self::Piano,
    ];

    /// Compact 4-channel bar shown directly in the sidebar quick strip.
    pub const SIDEBAR_QUICK: [Self; 4] = [Self::Rain, Self::Fireplace, Self::Breeze, Self::Cosmos];

    #[allow(dead_code)]
    pub fn index(self) -> usize {
        match self {
            Self::Rain => 0,
            Self::Fireplace => 1,
            Self::Breeze => 2,
            Self::Cosmos => 3,
            Self::Train => 4,
            Self::Piano => 5,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Rain => "rain",
            Self::Fireplace => "fireplace",
            Self::Breeze => "breeze",
            Self::Cosmos => "cosmos",
            Self::Train => "train",
            Self::Piano => "piano",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "rain" => Some(Self::Rain),
            "fireplace" => Some(Self::Fireplace),
            "breeze" => Some(Self::Breeze),
            "cosmos" => Some(Self::Cosmos),
            "train" => Some(Self::Train),
            "piano" => Some(Self::Piano),
            _ => None,
        }
    }

    pub fn for_theme(theme_id: &str) -> Self {
        match theme_id {
            "paper" | "kiki" | "lantern" => Self::Fireplace,
            "night" | "howl" => Self::Breeze,
            "prism" => Self::Cosmos,
            "spirited" => Self::Train,
            _ => Self::Rain,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Rain => "天空细雨",
            Self::Fireplace => "壁炉微光",
            Self::Breeze => "静夜微风",
            Self::Cosmos => "黑曜星穹",
            Self::Train => "浅海电车",
            Self::Piano => "治愈琴韵",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Rain => "细雨",
            Self::Fireplace => "壁炉",
            Self::Breeze => "微风",
            Self::Cosmos => "星穹",
            Self::Train => "海轨",
            Self::Piano => "琴韵",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Rain => "粉红雨幕 · 檐下水滴",
            Self::Fireplace => "深沉暖流 · 木柴轻响",
            Self::Breeze => "双频林风 · 远景夜虫",
            Self::Cosmos => "大九和弦 · 水晶风铃",
            Self::Train => "铁轨轻律 · 浅海潮汐",
            Self::Piano => "温柔电钢 · 久石让五声琶音",
        }
    }

    pub fn mix_value(self, mix: &SoundscapeMix) -> f32 {
        match self {
            Self::Rain => mix.rain,
            Self::Fireplace => mix.fireplace,
            Self::Breeze => mix.breeze,
            Self::Cosmos => mix.cosmos,
            Self::Train => mix.train,
            Self::Piano => mix.piano,
        }
    }

    pub fn set_mix_value(self, mix: &mut SoundscapeMix, value: f32) {
        let clamped = value.clamp(0.0, 1.0);
        match self {
            Self::Rain => mix.rain = clamped,
            Self::Fireplace => mix.fireplace = clamped,
            Self::Breeze => mix.breeze = clamped,
            Self::Cosmos => mix.cosmos = clamped,
            Self::Train => mix.train = clamped,
            Self::Piano => mix.piano = clamped,
        }
        mix.recipe = None;
    }
}

pub const MIXER_RECIPES: &[(&str, &str, &str)] = &[
    ("rain_hearth", "雨夜炉火", "细雨 + 壁炉 + 轻琴"),
    ("sea_train", "浅海列车", "海轨电车 + 微风 + 琴韵"),
    ("camphor_rain", "古樟晴雨", "林间细雨 + 微风 + 轻琴"),
    ("midnight_study", "深夜自习", "星穹 + 壁炉 + 细雨"),
    ("music_box", "林间琴房", "治愈琴韵 + 微风 + 星穹"),
];

/// Thread-safe handle allowing live per-track gain adjustments, TTS ducking,
/// and smooth click-free cosine fade-out on a running multi-track soundscape.
#[derive(Clone)]
pub struct SoundscapeHandle {
    stop_flag: Arc<AtomicBool>,
    track_gains: Arc<[AtomicU32; 6]>,
    duck_target: Arc<AtomicU32>,
}

impl SoundscapeHandle {
    pub fn trigger_fade_out(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    pub fn apply_mix(&self, mix: &SoundscapeMix) {
        let m = mix.clamped();
        let vals = [m.rain, m.fireplace, m.breeze, m.cosmos, m.train, m.piano];
        for (slot, v) in self.track_gains.iter().zip(vals) {
            slot.store(v.to_bits(), Ordering::Relaxed);
        }
    }

    #[allow(dead_code)]
    pub fn set_track_gain(&self, track: SoundscapePreset, gain: f32) {
        self.track_gains[track.index()].store(gain.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    /// Smoothly duck background soundscape volume while TTS audiobook playback is active.
    pub fn set_ducking(&self, active: bool) {
        let target: f32 = if active { 0.24 } else { 1.0 };
        self.duck_target.store(target.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Clone)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn signed(&mut self) -> f32 {
        let v = (self.next_u64() >> 40) as f32 / ((1u32 << 24) as f32);
        v * 2.0 - 1.0
    }

    fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / ((1u32 << 24) as f32)
    }
}

#[derive(Default, Clone)]
struct PinkFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    b3: f32,
    b4: f32,
    b5: f32,
    b6: f32,
}

impl PinkFilter {
    fn process(&mut self, white: f32) -> f32 {
        self.b0 = 0.99886 * self.b0 + white * 0.0555179;
        self.b1 = 0.99332 * self.b1 + white * 0.0750759;
        self.b2 = 0.96900 * self.b2 + white * 0.153_852;
        self.b3 = 0.86650 * self.b3 + white * 0.3104856;
        self.b4 = 0.55000 * self.b4 + white * 0.5329522;
        self.b5 = -0.7616 * self.b5 - white * 0.0168980;
        let out =
            self.b0 + self.b1 + self.b2 + self.b3 + self.b4 + self.b5 + self.b6 + white * 0.5362;
        self.b6 = white * 0.115926;
        out * 0.11
    }
}

#[derive(Default, Clone)]
struct BiquadBp {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadBp {
    fn process(&mut self, x: f32, freq: f32, q: f32) -> f32 {
        let w0 = std::f32::consts::TAU * (freq / SAMPLE_RATE as f32).clamp(0.001, 0.45);
        let alpha = w0.sin() / (2.0 * q.max(0.4));
        let cos_w0 = w0.cos();
        let a0 = 1.0 + alpha;
        let b0 = alpha / a0;
        let b2 = -alpha / a0;
        let a1 = (-2.0 * cos_w0) / a0;
        let a2 = (1.0 - alpha) / a0;
        let y = b0 * x + b2 * self.x2 - a1 * self.y1 - a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

#[derive(Default, Clone)]
struct VoicePing {
    phase: f32,
    freq: f32,
    amp: f32,
    decay: f32,
    pan: f32,
    age: u32,
}

struct TrackEngine {
    rng: Rng,
    pink_l: PinkFilter,
    pink_r: PinkFilter,
    lp_l: f32,
    lp_r: f32,
    brown_l: f32,
    brown_r: f32,
    bp_l: BiquadBp,
    bp_r: BiquadBp,
    bp_hi_l: BiquadBp,
    bp_hi_r: BiquadBp,
    pings: [VoicePing; 4],
    crackle_env: f32,
    crackle_pan: f32,
    crackle_freq: f32,
    next_event_frame: u64,
    piano_step: usize,
}

impl TrackEngine {
    fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            pink_l: PinkFilter::default(),
            pink_r: PinkFilter::default(),
            lp_l: 0.0,
            lp_r: 0.0,
            brown_l: 0.0,
            brown_r: 0.0,
            bp_l: BiquadBp::default(),
            bp_r: BiquadBp::default(),
            bp_hi_l: BiquadBp::default(),
            bp_hi_r: BiquadBp::default(),
            pings: Default::default(),
            crackle_env: 0.0,
            crackle_pan: 0.5,
            crackle_freq: 1850.0,
            next_event_frame: 3_200,
            piano_step: 0,
        }
    }

    #[inline]
    fn sine_hz(t: f64, freq: f64) -> f32 {
        let phase = (t * freq).fract();
        (phase * std::f64::consts::TAU).sin() as f32
    }

    fn quietest_ping(&mut self) -> &mut VoicePing {
        let mut best = 0usize;
        for i in 1..self.pings.len() {
            if self.pings[i].amp < self.pings[best].amp {
                best = i;
            }
        }
        &mut self.pings[best]
    }

    fn advance_pings(&mut self, warm_overtone: bool) -> (f32, f32) {
        let mut l = 0.0_f32;
        let mut r = 0.0_f32;
        for slot in &mut self.pings {
            if slot.amp > 0.0002 {
                let attack = if warm_overtone {
                    (slot.age as f32 / 900.0).min(1.0)
                } else {
                    1.0
                };
                let fund = (slot.phase * std::f32::consts::TAU).sin();
                let s = if warm_overtone {
                    let h2 = (slot.phase * 2.0 * std::f32::consts::TAU).sin() * 0.24;
                    let h3 = (slot.phase * 3.0 * std::f32::consts::TAU).sin() * 0.06;
                    (fund + h2 + h3) * slot.amp * attack
                } else {
                    fund * slot.amp
                };
                slot.phase = (slot.phase + slot.freq / SAMPLE_RATE as f32) % 1.0;
                slot.amp *= slot.decay;
                slot.age = slot.age.saturating_add(1);
                l += s * (1.0 - slot.pan);
                r += s * slot.pan;
            } else {
                slot.amp = 0.0;
            }
        }
        (l, r)
    }

    fn step_rain(&mut self, frame: u64, t: f64) -> (f32, f32) {
        let w_l = self.rng.signed();
        let w_r = self.rng.signed();
        let p_l = self.pink_l.process(w_l);
        let p_r = self.pink_r.process(w_r);
        self.lp_l += (p_l - self.lp_l) * 0.30;
        self.lp_r += (p_r - self.lp_r) * 0.30;
        let air_l = self.bp_hi_l.process(w_l * 0.32, 2450.0, 0.9);
        let air_r = self.bp_hi_r.process(w_r * 0.32, 2550.0, 0.9);
        let swell = 0.84 + 0.16 * (t * 0.42).sin() as f32;
        let bed_l = ((self.lp_l * 0.85 + self.lp_r * 0.15) * 1.15 + air_l * 0.28) * swell;
        let bed_r = ((self.lp_r * 0.85 + self.lp_l * 0.15) * 1.15 + air_r * 0.28) * swell;

        if frame >= self.next_event_frame {
            self.next_event_frame = frame + 3_200 + (self.rng.next_u64() % 9_800);
            let freq = 740.0 + self.rng.unit() * 1_180.0;
            let amp = 0.14 + self.rng.unit() * 0.12;
            let pan = 0.12 + self.rng.unit() * 0.76;
            let slot = self.quietest_ping();
            slot.phase = 0.0;
            slot.freq = freq;
            slot.amp = amp;
            slot.decay = 0.9985;
            slot.pan = pan;
            slot.age = 0;
        }
        let (dl, dr) = self.advance_pings(false);
        (bed_l + dl, bed_r + dr)
    }

    fn step_fireplace(&mut self, frame: u64, t: f64) -> (f32, f32) {
        let w_l = self.rng.signed();
        let w_r = self.rng.signed();
        let p_l = self.pink_l.process(w_l);
        let p_r = self.pink_r.process(w_r);
        self.brown_l = (self.brown_l + w_l * 0.036) * 0.993;
        self.brown_r = (self.brown_r + w_r * 0.036) * 0.993;
        self.lp_l += (self.brown_l - self.lp_l) * 0.10;
        self.lp_r += (self.brown_r - self.lp_r) * 0.10;
        let glow_l = self.bp_l.process(p_l, 420.0, 0.95) * 1.25;
        let glow_r = self.bp_r.process(p_r, 450.0, 0.95) * 1.25;
        let flutter = 0.80 + 0.14 * (t * 2.1).sin() as f32 + 0.06 * (t * 4.7).cos() as f32;
        let hearth_l = ((self.lp_l * 0.8 + self.lp_r * 0.2) * 1.35 + glow_l) * flutter;
        let hearth_r = ((self.lp_r * 0.8 + self.lp_l * 0.2) * 1.35 + glow_r) * flutter;

        if frame >= self.next_event_frame {
            self.next_event_frame = frame + 3_400 + (self.rng.next_u64() % 13_500);
            self.crackle_env = 0.42 + self.rng.unit() * 0.46;
            self.crackle_pan = 0.16 + self.rng.unit() * 0.68;
            self.crackle_freq = 1400.0 + self.rng.unit() * 1400.0;
        }
        let crackle = if self.crackle_env > 0.001 {
            let pop =
                self.bp_hi_l.process(self.rng.signed(), self.crackle_freq, 1.8) * self.crackle_env;
            self.crackle_env *= 0.990;
            pop
        } else {
            self.crackle_env = 0.0;
            0.0
        };
        let cl = crackle * (1.0 - self.crackle_pan);
        let cr = crackle * self.crackle_pan;
        (hearth_l + cl, hearth_r + cr)
    }

    fn step_breeze(&mut self, t: f64) -> (f32, f32) {
        let p_l = self.pink_l.process(self.rng.signed());
        let p_r = self.pink_r.process(self.rng.signed());
        let freq_l = 360.0 + 150.0 * (t * 0.34).sin() as f32 + 55.0 * (t * 0.17 + 1.1).cos() as f32;
        let freq_r = 390.0 + 145.0 * (t * 0.29 + 0.8).sin() as f32 + 50.0 * (t * 0.21).cos() as f32;
        let body_l = self.bp_l.process(p_l, freq_l, 1.2) * 2.55;
        let body_r = self.bp_r.process(p_r, freq_r, 1.2) * 2.55;

        let leaf_freq_l = 1280.0 + 360.0 * (t * 0.23 + 0.4).sin() as f32;
        let leaf_freq_r = 1340.0 + 340.0 * (t * 0.27 + 1.9).cos() as f32;
        let leaf_swell_l = 0.65 + 0.35 * (t * 0.31).sin() as f32;
        let leaf_swell_r = 0.65 + 0.35 * (t * 0.26 + 1.2).cos() as f32;
        let leaves_l = self.bp_hi_l.process(p_l, leaf_freq_l, 1.1) * 1.45 * leaf_swell_l;
        let leaves_r = self.bp_hi_r.process(p_r, leaf_freq_r, 1.1) * 1.45 * leaf_swell_r;

        let cycle = (0.5 + 0.5 * (t * 0.65).sin()).powi(6) as f32;
        let pulse = (0.5 + 0.5 * Self::sine_hz(t, 28.0)) * cycle * 0.022;
        let chirp = (Self::sine_hz(t, 2680.0) * 0.7 + Self::sine_hz(t, 3120.0) * 0.3) * pulse;
        (
            body_l + leaves_l + chirp * 0.38,
            body_r + leaves_r + chirp * 0.62,
        )
    }

    fn step_cosmos(&mut self, frame: u64, t: f64) -> (f32, f32) {
        let breath1 = 0.72 + 0.28 * (t * 0.26).sin() as f32;
        let breath2 = 0.72 + 0.28 * (t * 0.19 + 1.9).cos() as f32;
        let pad_l = Self::sine_hz(t, 130.81) * 0.24 * breath1
            + Self::sine_hz(t, 196.00) * 0.19 * breath2
            + Self::sine_hz(t, 293.66) * 0.12 * breath1
            + Self::sine_hz(t, 329.63) * 0.10 * breath2
            + Self::sine_hz(t, 392.00) * 0.07 * breath1;
        let pad_r = Self::sine_hz(t, 131.05) * 0.24 * breath2
            + Self::sine_hz(t, 195.70) * 0.19 * breath1
            + Self::sine_hz(t, 294.00) * 0.12 * breath2
            + Self::sine_hz(t, 329.95) * 0.09 * breath1
            + Self::sine_hz(t, 493.88) * 0.065 * breath2;

        if frame >= self.next_event_frame {
            self.next_event_frame = frame + 28_000 + (self.rng.next_u64() % 44_000);
            const PENTATONIC: &[f32] = &[523.25, 587.33, 659.25, 783.99, 880.0, 1046.5];
            let freq = PENTATONIC[(self.rng.next_u64() as usize) % PENTATONIC.len()];
            let pan = 0.18 + self.rng.unit() * 0.64;
            let slot = self.quietest_ping();
            slot.phase = 0.0;
            slot.freq = freq;
            slot.amp = 0.17;
            slot.decay = 0.99991;
            slot.pan = pan;
            slot.age = 0;
        }
        let (cl, cr) = self.advance_pings(false);
        (pad_l + cl, pad_r + cr)
    }

    /// Gentle coastal rail rhythm (soft double-beat track pulse) + shallow ocean tide wash.
    fn step_train(&mut self, t: f64) -> (f32, f32) {
        let w_l = self.rng.signed();
        let w_r = self.rng.signed();
        let p_l = self.pink_l.process(w_l);
        let p_r = self.pink_r.process(w_r);

        // Cabin warm rolling bed
        self.brown_l = (self.brown_l + w_l * 0.032) * 0.992;
        self.brown_r = (self.brown_r + w_r * 0.032) * 0.992;
        let roll_l = self.bp_l.process(self.brown_l + p_l * 0.45, 220.0, 0.85) * 1.45;
        let roll_r = self.bp_r.process(self.brown_r + p_r * 0.45, 235.0, 0.85) * 1.45;

        // Shallow sea tide wash breathing every ~7 seconds
        let tide = (0.5 + 0.5 * (t * 0.90).sin()).powi(2) as f32;
        let sea_l = self.bp_hi_l.process(p_l, 780.0 + 320.0 * tide, 0.8) * (0.65 + 0.95 * tide);
        let sea_r = self.bp_hi_r.process(p_r, 820.0 + 300.0 * tide, 0.8) * (0.65 + 0.95 * tide);

        // Iconic soft rail joint double-pulse ("da-dum ... da-dum") every 1.45s
        let cycle = t % 1.45;
        let pulse_a = (-((cycle - 0.12) / 0.045).powi(2)).exp() as f32;
        let pulse_b = (-((cycle - 0.31) / 0.052).powi(2)).exp() as f32 * 0.82;
        let clack_env = pulse_a + pulse_b;
        let clack = (Self::sine_hz(t, 92.0) * 0.55 + Self::sine_hz(t, 148.0) * 0.25 + p_l * 0.45)
            * clack_env
            * 0.26;

        (
            (roll_l * 0.72 + sea_l * 0.58 + clack * 0.92) * 1.32,
            (roll_r * 0.72 + sea_r * 0.58 + clack * 1.08) * 1.32,
        )
    }

    /// Warm Hisaishi-inspired pentatonic / maj7 soft Rhodes piano & celesta gentle arpeggio.
    fn step_piano(&mut self, frame: u64, t: f64) -> (f32, f32) {
        // Gentle Fmaj7 -> G6 -> Em7 -> Am7 progression frequencies (Hz)
        const CHORDS: &[[f32; 4]] = &[
            [174.61, 220.00, 261.63, 329.63], // Fmaj7
            [196.00, 246.94, 293.66, 329.63], // G6
            [164.81, 246.94, 293.66, 392.00], // Em7
            [220.00, 261.63, 329.63, 392.00], // Am7
        ];
        const MELODY: &[f32] = &[
            523.25, 587.33, 659.25, 783.99, 659.25, 587.33, 523.25, 440.00,
        ];

        if frame >= self.next_event_frame {
            // Unhurried ~0.85s - 1.35s between soft keys
            let gap = 36_000 + (self.rng.next_u64() % 22_000);
            self.next_event_frame = frame + gap;
            let chord = CHORDS[(self.piano_step / 4) % CHORDS.len()];
            let note_idx = self.piano_step % 4;
            let freq = if note_idx == 3 && self.rng.unit() > 0.35 {
                MELODY[self.piano_step % MELODY.len()]
            } else {
                chord[note_idx]
            };
            self.piano_step = self.piano_step.wrapping_add(1);
            let pan = 0.24 + self.rng.unit() * 0.52;
            let amp = if freq > 450.0 { 0.14 } else { 0.20 };
            let slot = self.quietest_ping();
            slot.phase = 0.0;
            slot.freq = freq;
            slot.amp = amp;
            slot.decay = 0.999935;
            slot.pan = pan;
            slot.age = 0;
        }

        // Subtle warm felt-piano air pad underneath so loudness stays cozy and continuous
        let pad_env = 0.76 + 0.24 * (t * 0.22).sin() as f32;
        let bed_l =
            (Self::sine_hz(t, 174.61) * 0.12 + Self::sine_hz(t, 261.63) * 0.09) * pad_env;
        let bed_r =
            (Self::sine_hz(t, 220.00) * 0.11 + Self::sine_hz(t, 329.63) * 0.08) * pad_env;

        let (nl, nr) = self.advance_pings(true);
        let raw_l = bed_l + nl * 1.45;
        let raw_r = bed_r + nr * 1.45;
        // Soft low-pass warmth so keys sound felt-damped
        self.lp_l += (raw_l - self.lp_l) * 0.28;
        self.lp_r += (raw_r - self.lp_r) * 0.28;
        (self.lp_l * 1.65, self.lp_r * 1.65)
    }
}

/// Real-time 6-track stereo ambient mixer source with per-track atomic gains,
/// click-free smoothing, and automatic TTS audio ducking.
pub struct SoundscapeSource {
    frame: u64,
    emit_right: bool,
    pending_right: f32,
    stop_flag: Arc<AtomicBool>,
    fade_out_frame: Option<u32>,
    track_gains: Arc<[AtomicU32; 6]>,
    smoothed_gains: [f32; 6],
    duck_target: Arc<AtomicU32>,
    smoothed_duck: f32,
    engines: [TrackEngine; 6],
}

impl SoundscapeSource {
    #[cfg(test)]
    pub fn new(preset: SoundscapePreset) -> Self {
        Self::with_handle(preset).0
    }

    #[allow(dead_code)]
    pub fn with_handle(preset: SoundscapePreset) -> (Self, SoundscapeHandle) {
        let mut mix = SoundscapeMix {
            rain: 0.0,
            fireplace: 0.0,
            breeze: 0.0,
            cosmos: 0.0,
            train: 0.0,
            piano: 0.0,
            recipe: None,
        };
        preset.set_mix_value(&mut mix, 1.0);
        Self::with_mix_handle(&mix)
    }

    pub fn with_mix_handle(mix: &SoundscapeMix) -> (Self, SoundscapeHandle) {
        let m = mix.clamped();
        let initial = [m.rain, m.fireplace, m.breeze, m.cosmos, m.train, m.piano];
        let track_gains = Arc::new([
            AtomicU32::new(initial[0].to_bits()),
            AtomicU32::new(initial[1].to_bits()),
            AtomicU32::new(initial[2].to_bits()),
            AtomicU32::new(initial[3].to_bits()),
            AtomicU32::new(initial[4].to_bits()),
            AtomicU32::new(initial[5].to_bits()),
        ]);
        let duck_target = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let handle = SoundscapeHandle {
            stop_flag: Arc::clone(&stop_flag),
            track_gains: Arc::clone(&track_gains),
            duck_target: Arc::clone(&duck_target),
        };
        let engines = [
            TrackEngine::new(0x51A7_9E23_8841_C0D1),
            TrackEngine::new(0x7F31_4B12_1905_AA37),
            TrackEngine::new(0x2C94_D718_553E_91B3),
            TrackEngine::new(0x9E48_22F1_67C4_3319),
            TrackEngine::new(0x4A11_C8E7_3109_F42D),
            TrackEngine::new(0xB390_77A2_54C1_826F),
        ];
        let source = Self {
            frame: 0,
            emit_right: false,
            pending_right: 0.0,
            stop_flag,
            fade_out_frame: None,
            track_gains,
            smoothed_gains: initial,
            duck_target,
            smoothed_duck: 1.0,
            engines,
        };
        (source, handle)
    }

    fn synthesize_frame(&mut self) -> Option<(f32, f32)> {
        if self.fade_out_frame.is_none() && self.stop_flag.load(Ordering::Relaxed) {
            self.fade_out_frame = Some(0);
        }
        let fade_out = if let Some(ref mut step) = self.fade_out_frame {
            if *step >= FADE_OUT_FRAMES {
                return None;
            }
            let p = *step as f64 / FADE_OUT_FRAMES as f64;
            *step += 1;
            (0.5 + 0.5 * (p * std::f64::consts::PI).cos()) as f32
        } else {
            1.0
        };

        let t = self.frame as f64 / SAMPLE_RATE as f64;
        let fade_in = if (self.frame as f64) < FADE_IN_FRAMES {
            let p = self.frame as f64 / FADE_IN_FRAMES;
            (0.5 - 0.5 * (p * std::f64::consts::PI).cos()) as f32
        } else {
            1.0
        };

        let target_duck = f32::from_bits(self.duck_target.load(Ordering::Relaxed)).clamp(0.1, 1.0);
        self.smoothed_duck += (target_duck - self.smoothed_duck) * 0.0012;
        let fade = fade_in * fade_out * self.smoothed_duck;

        let mut mix_l = 0.0_f32;
        let mut mix_r = 0.0_f32;
        let mut energy_weight = 0.0_f32;

        for i in 0..6 {
            let target = f32::from_bits(self.track_gains[i].load(Ordering::Relaxed)).clamp(0.0, 1.0);
            self.smoothed_gains[i] += (target - self.smoothed_gains[i]) * 0.0025;
            let g = self.smoothed_gains[i];
            if g < 0.001 {
                continue;
            }
            let (tl, tr) = match i {
                0 => self.engines[0].step_rain(self.frame, t),
                1 => self.engines[1].step_fireplace(self.frame, t),
                2 => self.engines[2].step_breeze(t),
                3 => self.engines[3].step_cosmos(self.frame, t),
                4 => self.engines[4].step_train(t),
                _ => self.engines[5].step_piano(self.frame, t),
            };
            mix_l += tl * g;
            mix_r += tr * g;
            energy_weight += g * g;
        }

        // Gentle RMS compensation when multiple tracks are active simultaneously
        let norm = if energy_weight > 1.0 {
            1.0 / energy_weight.sqrt()
        } else {
            1.0
        };

        self.frame = self.frame.wrapping_add(1);
        let l = (mix_l * norm * fade * 1.38).tanh() * 0.86;
        let r = (mix_r * norm * fade * 1.38).tanh() * 0.86;
        Some((l, r))
    }
}

impl Iterator for SoundscapeSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.emit_right {
            self.emit_right = false;
            return Some(self.pending_right);
        }
        let (l, r) = self.synthesize_frame()?;
        self.pending_right = r;
        self.emit_right = true;
        Some(l)
    }
}

impl Source for SoundscapeSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(2048)
    }

    fn channels(&self) -> u16 {
        CHANNELS
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Soft 3-note windchime arpeggio played gently when a Reading Light Vial focus session completes.
pub struct ChimeSource {
    frame: u64,
    total_frames: u64,
    emit_right: bool,
    pending_right: f32,
}

impl ChimeSource {
    pub fn new() -> Self {
        Self {
            frame: 0,
            total_frames: (SAMPLE_RATE as u64) * 3,
            emit_right: false,
            pending_right: 0.0,
        }
    }
}

impl Iterator for ChimeSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.emit_right {
            self.emit_right = false;
            return Some(self.pending_right);
        }
        if self.frame >= self.total_frames {
            return None;
        }
        let t = self.frame as f32 / SAMPLE_RATE as f32;
        self.frame += 1;
        let notes: &[(f32, f32, f32)] = &[
            (0.00, 523.25, 0.35), // C5
            (0.28, 659.25, 0.50), // E5
            (0.56, 783.99, 0.65), // G5
        ];
        let mut l = 0.0_f32;
        let mut r = 0.0_f32;
        for &(start, freq, pan) in notes {
            if t >= start {
                let dt = t - start;
                let attack = (dt / 0.025).min(1.0);
                let env = attack * (-dt * 1.9).exp() * 0.22;
                let tone = (dt * std::f32::consts::TAU * freq).sin()
                    + (dt * std::f32::consts::TAU * freq * 2.0).sin() * 0.18;
                let s = tone * env;
                l += s * (1.0 - pan);
                r += s * pan;
            }
        }
        self.pending_right = r.clamp(-0.85, 0.85);
        self.emit_right = true;
        Some(l.clamp(-0.85, 0.85))
    }
}

impl Source for ChimeSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(
            ((self.total_frames - self.frame) as usize) * CHANNELS as usize
                + usize::from(self.emit_right),
        )
    }

    fn channels(&self) -> u16 {
        CHANNELS
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs(3))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_soundscape_presets_produce_finite_non_clipping_audible_samples() {
        for preset in SoundscapePreset::ALL {
            let mut src = SoundscapeSource::new(preset);
            assert_eq!(src.channels(), 2);
            assert_eq!(src.sample_rate(), 44_100);
            let first_l = src.next().unwrap();
            let first_r = src.next().unwrap();
            assert!(
                first_l.abs() < 0.01 && first_r.abs() < 0.01,
                "{preset:?} should fade in smoothly without initial click"
            );
            let mut max_abs = 0.0_f32;
            let mut energy = 0.0_f64;
            let warmup = 44_100;
            for _ in 0..warmup {
                let _ = src.next().unwrap();
            }
            let count = 44_100 * 4;
            for _ in 0..count {
                let sample = src.next().unwrap();
                assert!(sample.is_finite(), "{preset:?} emitted non-finite sample");
                max_abs = max_abs.max(sample.abs());
                energy += (sample as f64) * (sample as f64);
            }
            assert!(max_abs <= 0.88, "{preset:?} clipped: max_abs={max_abs}");
            let rms = (energy / count as f64).sqrt();
            eprintln!(
                "soundscape {preset:?}: rms={rms:.4} ({:.1} dBFS), peak={max_abs:.4}",
                20.0 * rms.log10()
            );
            assert!(
                rms >= 0.14 && rms <= 0.38,
                "{preset:?} loudness outside comfortable audible target: rms={rms}"
            );
        }
    }

    #[test]
    fn multi_track_recipe_and_tts_ducking_scale_cleanly() {
        let mix = SoundscapeMix::from_recipe("sea_train");
        let (mut src, handle) = SoundscapeSource::with_mix_handle(&mix);
        for _ in 0..44_100 {
            let _ = src.next().unwrap();
        }
        let mut normal_e = 0.0_f64;
        for _ in 0..44_100 {
            let s = src.next().unwrap();
            assert!(s.abs() <= 0.88);
            normal_e += (s as f64) * (s as f64);
        }
        let normal_rms = (normal_e / 44_100.0).sqrt();

        handle.set_ducking(true);
        for _ in 0..22_050 {
            let _ = src.next().unwrap();
        }
        let mut ducked_e = 0.0_f64;
        for _ in 0..44_100 {
            let s = src.next().unwrap();
            ducked_e += (s as f64) * (s as f64);
        }
        let ducked_rms = (ducked_e / 44_100.0).sqrt();
        assert!(
            ducked_rms < normal_rms * 0.45,
            "TTS ducking should smoothly lower ambient loudness: normal={normal_rms}, ducked={ducked_rms}"
        );
    }

    #[test]
    fn rodio_default_output_pulls_real_frames_from_soundscape() {
        use std::sync::atomic::AtomicUsize;

        struct CountingSource {
            inner: SoundscapeSource,
            pulled: Arc<AtomicUsize>,
        }
        impl Iterator for CountingSource {
            type Item = f32;
            fn next(&mut self) -> Option<f32> {
                let s = self.inner.next()?;
                self.pulled.fetch_add(1, Ordering::Relaxed);
                Some(s)
            }
        }
        impl Source for CountingSource {
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

        if let Ok((_stream, handle)) = rodio::OutputStream::try_default() {
            let sink = rodio::Sink::try_new(&handle).unwrap();
            sink.set_volume(0.0);
            let pulled = Arc::new(AtomicUsize::new(0));
            let (src, stop_handle) = SoundscapeSource::with_handle(SoundscapePreset::Breeze);
            sink.append(CountingSource {
                inner: src,
                pulled: Arc::clone(&pulled),
            });
            std::thread::sleep(Duration::from_millis(180));
            stop_handle.trigger_fade_out();
            let count = pulled.load(Ordering::Relaxed);
            assert!(count > 2_000, "pulled={count}");
        }
    }

    #[test]
    fn chime_source_terminates_cleanly_without_clipping() {
        let mut chime = ChimeSource::new();
        let total = 44_100 * 3 * 2;
        assert_eq!(chime.current_frame_len(), Some(total));
        let first_l = chime.next().unwrap();
        let first_r = chime.next().unwrap();
        let mut count = 2usize;
        let mut max_abs = first_l.abs().max(first_r.abs());
        for sample in chime {
            assert!(sample.is_finite());
            max_abs = max_abs.max(sample.abs());
            count += 1;
        }
        assert_eq!(count, total);
        assert!(max_abs > 0.05 && max_abs <= 0.85);
    }

    #[test]
    fn long_running_cosmos_keeps_smooth_waveform_after_one_hour_and_fades_out_cleanly() {
        let (mut src, handle) = SoundscapeSource::with_handle(SoundscapePreset::Cosmos);
        src.frame = (SAMPLE_RATE as u64) * 3600;
        src.engines[3].next_event_frame = src.frame + 100_000;
        let mut prev_l = src.next().unwrap();
        let _prev_r = src.next().unwrap();
        let mut identical_runs = 0usize;
        let mut max_step = 0.0_f32;
        for _ in 0..2048 {
            let l = src.next().unwrap();
            let _r = src.next().unwrap();
            assert!(l.is_finite());
            let delta = (l - prev_l).abs();
            if delta < 1e-7 {
                identical_runs += 1;
            }
            max_step = max_step.max(delta);
            prev_l = l;
        }
        assert!(identical_runs < 10, "identical_runs={identical_runs}");
        assert!(max_step < 0.05, "max_step={max_step}");

        handle.trigger_fade_out();
        let mut remaining = Vec::new();
        for s in src {
            remaining.push(s);
            assert!(remaining.len() <= (FADE_OUT_FRAMES as usize + 4) * 2);
        }
        assert_eq!(remaining.len(), (FADE_OUT_FRAMES as usize) * 2);
        let tail_max = remaining[remaining.len() - 64..]
            .iter()
            .fold(0.0_f32, |m, &s| m.max(s.abs()));
        assert!(tail_max < 0.005, "tail_max={tail_max}");
    }
}

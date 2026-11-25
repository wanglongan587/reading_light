use rodio::Source;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
const FADE_IN_FRAMES: f64 = (SAMPLE_RATE as f64) * 1.2;
const FADE_OUT_FRAMES: u32 = (SAMPLE_RATE * 260) / 1000;

/// Thread-safe handle allowing smooth click-free cosine fade-out before stopping a live soundscape.
#[derive(Clone)]
pub struct SoundscapeHandle {
    stop_flag: Arc<AtomicBool>,
}

impl SoundscapeHandle {
    pub fn trigger_fade_out(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundscapePreset {
    Rain,
    Fireplace,
    Breeze,
    Cosmos,
}

impl SoundscapePreset {
    pub const ALL: [Self; 4] = [Self::Rain, Self::Fireplace, Self::Breeze, Self::Cosmos];

    pub fn id(self) -> &'static str {
        match self {
            Self::Rain => "rain",
            Self::Fireplace => "fireplace",
            Self::Breeze => "breeze",
            Self::Cosmos => "cosmos",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "rain" => Some(Self::Rain),
            "fireplace" => Some(Self::Fireplace),
            "breeze" => Some(Self::Breeze),
            "cosmos" => Some(Self::Cosmos),
            _ => None,
        }
    }

    pub fn for_theme(theme_id: &str) -> Self {
        match theme_id {
            "paper" | "kiki" | "lantern" => Self::Fireplace,
            "night" | "howl" => Self::Breeze,
            "prism" | "spirited" => Self::Cosmos,
            _ => Self::Rain,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Rain => "天空细雨",
            Self::Fireplace => "壁炉微光",
            Self::Breeze => "静夜微风",
            Self::Cosmos => "黑曜星穹",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Rain => "细雨",
            Self::Fireplace => "壁炉",
            Self::Breeze => "微风",
            Self::Cosmos => "星穹",
        }
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

    /// Uniform float in [-1.0, 1.0].
    fn signed(&mut self) -> f32 {
        let v = (self.next_u64() >> 40) as f32 / ((1u32 << 24) as f32);
        v * 2.0 - 1.0
    }

    /// Uniform float in [0.0, 1.0).
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
}

pub struct SoundscapeSource {
    preset: SoundscapePreset,
    frame: u64,
    emit_right: bool,
    pending_right: f32,
    stop_flag: Arc<AtomicBool>,
    fade_out_frame: Option<u32>,
    rng: Rng,
    pink_l: PinkFilter,
    pink_r: PinkFilter,
    lp_l: f32,
    lp_r: f32,
    brown_l: f32,
    brown_r: f32,
    bp_l: BiquadBp,
    bp_r: BiquadBp,
    ping_a: VoicePing,
    ping_b: VoicePing,
    crackle_env: f32,
    crackle_pan: f32,
    next_event_frame: u64,
}

impl SoundscapeSource {
    #[cfg(test)]
    pub fn new(preset: SoundscapePreset) -> Self {
        Self::with_handle(preset).0
    }

    pub fn with_handle(preset: SoundscapePreset) -> (Self, SoundscapeHandle) {
        let seed = match preset {
            SoundscapePreset::Rain => 0x51A7_9E23_8841_C0D1,
            SoundscapePreset::Fireplace => 0x7F31_4B12_1905_AA37,
            SoundscapePreset::Breeze => 0x2C94_D718_553E_91B3,
            SoundscapePreset::Cosmos => 0x9E48_22F1_67C4_3319,
        };
        let stop_flag = Arc::new(AtomicBool::new(false));
        let handle = SoundscapeHandle {
            stop_flag: Arc::clone(&stop_flag),
        };
        let source = Self {
            preset,
            frame: 0,
            emit_right: false,
            pending_right: 0.0,
            stop_flag,
            fade_out_frame: None,
            rng: Rng::new(seed),
            pink_l: PinkFilter::default(),
            pink_r: PinkFilter::default(),
            lp_l: 0.0,
            lp_r: 0.0,
            brown_l: 0.0,
            brown_r: 0.0,
            bp_l: BiquadBp::default(),
            bp_r: BiquadBp::default(),
            ping_a: VoicePing::default(),
            ping_b: VoicePing::default(),
            crackle_env: 0.0,
            crackle_pan: 0.5,
            next_event_frame: 8_000,
        };
        (source, handle)
    }

    #[inline]
    fn sine_hz(t: f64, freq: f64) -> f32 {
        let phase = (t * freq).fract();
        (phase * std::f64::consts::TAU).sin() as f32
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
        let fade = fade_in * fade_out;

        let (raw_l, raw_r) = match self.preset {
            SoundscapePreset::Rain => self.step_rain(t),
            SoundscapePreset::Fireplace => self.step_fireplace(t),
            SoundscapePreset::Breeze => self.step_breeze(t),
            SoundscapePreset::Cosmos => self.step_cosmos(t),
        };

        self.frame = self.frame.wrapping_add(1);
        let l = (raw_l * fade * 0.85).tanh() * 0.82;
        let r = (raw_r * fade * 0.85).tanh() * 0.82;
        Some((l, r))
    }

    fn step_rain(&mut self, t: f64) -> (f32, f32) {
        let w_l = self.rng.signed();
        let w_r = self.rng.signed();
        let p_l = self.pink_l.process(w_l);
        let p_r = self.pink_r.process(w_r);
        // Gentle low-pass rainfall wash with subtle cross-bleed for natural stereo width
        self.lp_l += (p_l - self.lp_l) * 0.14;
        self.lp_r += (p_r - self.lp_r) * 0.14;
        let swell = 0.82 + 0.18 * (t * 0.45).sin() as f32;
        let bed_l = (self.lp_l * 0.85 + self.lp_r * 0.15) * 0.55 * swell;
        let bed_r = (self.lp_r * 0.85 + self.lp_l * 0.15) * 0.55 * swell;

        if self.frame >= self.next_event_frame {
            self.next_event_frame = self.frame + 4_500 + (self.rng.next_u64() % 13_500);
            let slot = if self.ping_a.amp < self.ping_b.amp {
                &mut self.ping_a
            } else {
                &mut self.ping_b
            };
            slot.phase = 0.0;
            slot.freq = 680.0 + self.rng.unit() * 920.0;
            slot.amp = 0.045 + self.rng.unit() * 0.055;
            slot.decay = 0.9986;
            slot.pan = 0.15 + self.rng.unit() * 0.70;
        }
        let (dl, dr) = Self::advance_pings(&mut self.ping_a, &mut self.ping_b);
        (bed_l + dl, bed_r + dr)
    }

    fn step_fireplace(&mut self, t: f64) -> (f32, f32) {
        let w_l = self.rng.signed();
        let w_r = self.rng.signed();
        self.brown_l = (self.brown_l + w_l * 0.028) * 0.995;
        self.brown_r = (self.brown_r + w_r * 0.028) * 0.995;
        self.lp_l += (self.brown_l - self.lp_l) * 0.055;
        self.lp_r += (self.brown_r - self.lp_r) * 0.055;
        let flutter = 0.80 + 0.14 * (t * 2.1).sin() as f32 + 0.06 * (t * 4.7).cos() as f32;
        let hearth_l = (self.lp_l * 0.8 + self.lp_r * 0.2) * 1.15 * flutter;
        let hearth_r = (self.lp_r * 0.8 + self.lp_l * 0.2) * 1.15 * flutter;

        if self.frame >= self.next_event_frame {
            self.next_event_frame = self.frame + 6_000 + (self.rng.next_u64() % 26_000);
            self.crackle_env = 0.14 + self.rng.unit() * 0.16;
            self.crackle_pan = 0.2 + self.rng.unit() * 0.6;
        }
        let crackle = if self.crackle_env > 0.0008 {
            let pop = self.bp_l.process(self.rng.signed(), 1450.0, 2.2) * self.crackle_env;
            self.crackle_env *= 0.992;
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
        let freq_l = 320.0 + 140.0 * (t * 0.34).sin() as f32 + 55.0 * (t * 0.17 + 1.1).cos() as f32;
        let freq_r = 340.0 + 135.0 * (t * 0.29 + 0.8).sin() as f32 + 50.0 * (t * 0.21).cos() as f32;
        let gust_l = self.bp_l.process(p_l, freq_l, 2.4) * 0.72;
        let gust_r = self.bp_r.process(p_r, freq_r, 2.4) * 0.72;
        // Soft, organic distant night-cricket flutter (AM-pulsed rather than a constant sine)
        let cycle = (0.5 + 0.5 * (t * 0.65).sin()).powi(6) as f32;
        let pulse = (0.5 + 0.5 * Self::sine_hz(t, 28.0)) * cycle * 0.009;
        let chirp = (Self::sine_hz(t, 2680.0) * 0.7 + Self::sine_hz(t, 3120.0) * 0.3) * pulse;
        (gust_l + chirp * 0.38, gust_r + chirp * 0.62)
    }

    fn step_cosmos(&mut self, t: f64) -> (f32, f32) {
        // Warm fifth / major-9th luminous pad (C3, G3, D4, E4, B4) with f64 phase precision
        let breath1 = 0.68 + 0.32 * (t * 0.26).sin() as f32;
        let breath2 = 0.68 + 0.32 * (t * 0.19 + 1.9).cos() as f32;
        let pad_l = Self::sine_hz(t, 130.81) * 0.12 * breath1
            + Self::sine_hz(t, 196.00) * 0.09 * breath2
            + Self::sine_hz(t, 293.66) * 0.055 * breath1
            + Self::sine_hz(t, 329.95) * 0.045 * breath2;
        let pad_r = Self::sine_hz(t, 131.05) * 0.12 * breath2
            + Self::sine_hz(t, 195.70) * 0.09 * breath1
            + Self::sine_hz(t, 294.00) * 0.055 * breath2
            + Self::sine_hz(t, 493.88) * 0.032 * breath1;

        if self.frame >= self.next_event_frame {
            self.next_event_frame = self.frame + 42_000 + (self.rng.next_u64() % 68_000);
            const PENTATONIC: &[f32] = &[523.25, 587.33, 659.25, 783.99, 880.0, 1046.5];
            let freq = PENTATONIC[(self.rng.next_u64() as usize) % PENTATONIC.len()];
            let slot = if self.ping_a.amp < self.ping_b.amp {
                &mut self.ping_a
            } else {
                &mut self.ping_b
            };
            slot.phase = 0.0;
            slot.freq = freq;
            slot.amp = 0.065;
            slot.decay = 0.99992;
            slot.pan = 0.18 + self.rng.unit() * 0.64;
        }
        let (cl, cr) = Self::advance_pings(&mut self.ping_a, &mut self.ping_b);
        (pad_l + cl, pad_r + cr)
    }

    fn advance_pings(a: &mut VoicePing, b: &mut VoicePing) -> (f32, f32) {
        let mut l = 0.0;
        let mut r = 0.0;
        for slot in [a, b] {
            if slot.amp > 0.0002 {
                let s = (slot.phase * std::f32::consts::TAU).sin() * slot.amp;
                slot.phase = (slot.phase + slot.freq / SAMPLE_RATE as f32) % 1.0;
                slot.amp *= slot.decay;
                l += s * (1.0 - slot.pan);
                r += s * slot.pan;
            } else {
                slot.amp = 0.0;
            }
        }
        (l, r)
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
        None
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
            let count = 44_100 * 2;
            for _ in 0..count {
                let sample = src.next().unwrap();
                assert!(sample.is_finite(), "{preset:?} emitted non-finite sample");
                max_abs = max_abs.max(sample.abs());
                energy += (sample as f64) * (sample as f64);
            }
            assert!(max_abs <= 0.85, "{preset:?} clipped: max_abs={max_abs}");
            let rms = (energy / count as f64).sqrt();
            assert!(
                rms > 0.003,
                "{preset:?} produced near-silent output: rms={rms}"
            );
        }
    }

    #[test]
    fn chime_source_terminates_cleanly_without_clipping() {
        let mut chime = ChimeSource::new();
        let total = 44_100 * 3 * 2;
        assert_eq!(chime.current_frame_len(), Some(total));
        let first_l = chime.next().unwrap();
        assert!(first_l.is_finite());
        assert_eq!(chime.current_frame_len(), Some(total - 1));
        let first_r = chime.next().unwrap();
        assert!(first_r.is_finite());
        assert_eq!(chime.current_frame_len(), Some(total - 2));

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
        // Simulate 1 full hour of playback (frame = 158,760,000, well past f32 2^24 exact integer limit)
        src.frame = (SAMPLE_RATE as u64) * 3600;
        src.next_event_frame = src.frame + 100_000;
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
        assert!(
            identical_runs < 10,
            "waveform suffered sample-and-hold stairstepping after 1h: identical_runs={identical_runs}"
        );
        assert!(
            max_step < 0.05,
            "unexpected discontinuity in Cosmos pad after 1h: max_step={max_step}"
        );

        // Trigger cosine fade-out and confirm smooth decay to silence and clean termination
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
        assert!(
            tail_max < 0.005,
            "fade-out tail did not decay smoothly to zero: tail_max={tail_max}"
        );
    }
}

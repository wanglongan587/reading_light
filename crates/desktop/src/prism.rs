use gpui_kit::*;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const MAX_TRAIL: usize = 16;
const MAX_RIPPLES: usize = 8;

const PET_WHISPERS: &[&str] = &[
    "棱镜喵：这页的光刚刚好，慢慢读呀",
    "棱镜喵：风从文字间穿过，不催任何人",
    "棱镜喵：把触动你的那句话划下来吧",
    "棱镜喵：书页合上时，星光还会替你记着",
    "棱镜喵：累了就去「此刻心情」坐一会儿",
    "棱镜喵：每一束棱镜光，都在陪你翻下一页",
];

#[derive(Clone, Copy)]
pub struct TrailSpark {
    pub pos: Point<Pixels>,
    pub age: f32,
    pub duration: f32,
    pub hue: f32,
    pub size: f32,
}

#[derive(Clone, Copy)]
pub struct PrismRipple {
    pub origin: Point<Pixels>,
    pub age: f32,
    pub duration: f32,
    pub hue_offset: f32,
    pub is_prism: bool,
}

#[derive(Clone, Copy)]
pub struct BurstSpark {
    pub pos: Point<Pixels>,
    pub vel: (f32, f32),
    pub age: f32,
    pub duration: f32,
    pub hue: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeAtmosphere {
    Sky,
    Totoro,
    Spirited,
    Howl,
    Kiki,
    Lantern,
    Prism,
    Classic,
}

impl ThemeAtmosphere {
    pub fn from_theme_id(id: &str) -> Self {
        match id {
            "forest" => Self::Sky,
            "totoro" => Self::Totoro,
            "spirited" => Self::Spirited,
            "howl" => Self::Howl,
            "kiki" => Self::Kiki,
            "lantern" => Self::Lantern,
            "prism" => Self::Prism,
            _ => Self::Classic,
        }
    }
}

#[derive(Clone)]
pub struct MotionSnapshot {
    pub pointer_smooth: Point<Pixels>,
    pub pointer_halo: Point<Pixels>,
    pub pointer_active: bool,
    pub pointer_speed: f32,
    pub idle_fade: f32,
    pub phase: f32,
    pub atmosphere: ThemeAtmosphere,
    pub is_prism: bool,
    pub is_sky: bool,
    pub is_dark: bool,
    pub accent: u32,
    pub reduced_motion: bool,
    pub trail: Vec<TrailSpark>,
    pub ripples: Vec<PrismRipple>,
    pub pet_jump: f32,
    pub pet_sparks: Vec<BurstSpark>,
    pub pet_happy: bool,
    pub pet_mood: Option<String>,
}

pub struct MotionEngine {
    pub pointer_target: Point<Pixels>,
    pub pointer_smooth: Point<Pixels>,
    pointer_vel: (f32, f32),
    pub pointer_halo: Point<Pixels>,
    pointer_halo_vel: (f32, f32),
    pub pointer_active: bool,
    last_pointer_move: Instant,
    last_spark_pos: Option<Point<Pixels>>,
    pub phase: f32,
    last_frame: Instant,
    trail: VecDeque<TrailSpark>,
    ripples: Vec<PrismRipple>,
    pet_jump: f32,
    pet_vel: f32,
    pet_sparks: Vec<BurstSpark>,
    pet_clicks: usize,
    pet_happy_until: Option<Instant>,
}

impl MotionEngine {
    pub fn new() -> Self {
        let initial = point(px(680.), px(210.));
        Self {
            pointer_target: initial,
            pointer_smooth: initial,
            pointer_vel: (0.0, 0.0),
            pointer_halo: initial,
            pointer_halo_vel: (0.0, 0.0),
            pointer_active: false,
            last_pointer_move: Instant::now() - Duration::from_secs(5),
            last_spark_pos: None,
            phase: 0.4,
            last_frame: Instant::now(),
            trail: VecDeque::with_capacity(MAX_TRAIL),
            ripples: Vec::with_capacity(MAX_RIPPLES),
            pet_jump: 0.0,
            pet_vel: 0.0,
            pet_sparks: Vec::new(),
            pet_clicks: 0,
            pet_happy_until: None,
        }
    }

    pub fn on_mouse_move(&mut self, pos: Point<Pixels>, reduced_motion: bool) {
        self.pointer_target = pos;
        if !self.pointer_active {
            self.pointer_smooth = pos;
            self.pointer_halo = pos;
            self.pointer_active = true;
        }
        self.last_pointer_move = Instant::now();
        if reduced_motion {
            self.pointer_smooth = pos;
            self.pointer_halo = pos;
            return;
        }
        let emit = match self.last_spark_pos {
            Some(prev) => {
                let dx = f32::from(pos.x) - f32::from(prev.x);
                let dy = f32::from(pos.y) - f32::from(prev.y);
                (dx * dx + dy * dy).sqrt() >= 18.0
            }
            None => true,
        };
        if emit {
            self.last_spark_pos = Some(pos);
            if self.trail.len() >= MAX_TRAIL {
                self.trail.pop_front();
            }
            let hue = ((f32::from(pos.x) * 0.0014) + self.phase * 0.16).rem_euclid(1.0);
            self.trail.push_back(TrailSpark {
                pos,
                age: 0.0,
                duration: 0.56,
                hue,
                size: 4.5,
            });
        }
    }

    pub fn on_click(&mut self, pos: Point<Pixels>, reduced_motion: bool, is_prism: bool) {
        self.pointer_target = pos;
        self.last_pointer_move = Instant::now();
        if reduced_motion {
            return;
        }
        if self.ripples.len() >= MAX_RIPPLES {
            self.ripples.remove(0);
        }
        self.ripples.push(PrismRipple {
            origin: pos,
            age: 0.0,
            duration: 0.76,
            hue_offset: (self.phase * 0.22).rem_euclid(1.0),
            is_prism,
        });
    }

    pub fn trigger_pet(&mut self, origin: Point<Pixels>, reduced_motion: bool) -> &'static str {
        let whisper = PET_WHISPERS[self.pet_clicks % PET_WHISPERS.len()];
        self.pet_clicks = self.pet_clicks.wrapping_add(1);
        self.trigger_pet_celebration(origin, reduced_motion);
        whisper
    }

    pub fn trigger_pet_celebration(&mut self, origin: Point<Pixels>, reduced_motion: bool) {
        self.pet_happy_until = Some(Instant::now() + Duration::from_millis(2800));
        if !reduced_motion {
            self.pet_vel = -185.0;
            self.pet_jump = -1.5;
            for i in 0..10 {
                let angle = -std::f32::consts::PI * (0.12 + 0.76 * (i as f32 / 9.0));
                let speed = 58.0 + ((i * 29) % 52) as f32;
                self.pet_sparks.push(BurstSpark {
                    pos: origin,
                    vel: (angle.cos() * speed, angle.sin() * speed - 20.0),
                    age: 0.0,
                    duration: 0.68 + (i % 3) as f32 * 0.12,
                    hue: (i as f32 / 10.0 + self.phase * 0.1).rem_euclid(1.0),
                });
            }
            if self.pet_sparks.len() > 28 {
                let excess = self.pet_sparks.len() - 28;
                self.pet_sparks.drain(0..excess);
            }
        }
    }

    /// Advances spring physics, ripples, trail sparks, and ambient phase.
    /// Returns `true` when another animation frame is needed; drops cleanly to 0 FPS when idle.
    pub fn step(&mut self, reduced_motion: bool, continuous_flow: bool) -> bool {
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.last_frame)
            .as_secs_f32()
            .clamp(0.004, 0.042);
        self.last_frame = now;

        if reduced_motion {
            self.pointer_smooth = self.pointer_target;
            self.pointer_halo = self.pointer_target;
            self.pointer_vel = (0.0, 0.0);
            self.pointer_halo_vel = (0.0, 0.0);
            self.trail.clear();
            self.ripples.clear();
            self.pet_sparks.clear();
            self.pet_jump = 0.0;
            self.pet_vel = 0.0;
            return false;
        }

        // Advance continuous wave phase
        self.phase = (self.phase + dt * 1.42) % (std::f32::consts::TAU * 64.0);

        // Tier-1 fast damped spring for primary cursor spotlight
        let tx = f32::from(self.pointer_target.x);
        let ty = f32::from(self.pointer_target.y);
        let mut sx = f32::from(self.pointer_smooth.x);
        let mut sy = f32::from(self.pointer_smooth.y);
        let (mut vx, mut vy) = self.pointer_vel;
        let stiffness_1 = 175.0_f32;
        let damping_1 = 21.0_f32;
        let ax = (tx - sx) * stiffness_1 - vx * damping_1;
        let ay = (ty - sy) * stiffness_1 - vy * damping_1;
        vx += ax * dt;
        vy += ay * dt;
        sx += vx * dt;
        sy += vy * dt;
        self.pointer_smooth = point(px(sx), px(sy));
        self.pointer_vel = (vx, vy);

        // Tier-2 slower damped spring for chromatic dispersion lag halo
        let mut hx = f32::from(self.pointer_halo.x);
        let mut hy = f32::from(self.pointer_halo.y);
        let (mut hvx, mut hvy) = self.pointer_halo_vel;
        let stiffness_2 = 46.0_f32;
        let damping_2 = 12.5_f32;
        let hax = (tx - hx) * stiffness_2 - hvx * damping_2;
        let hay = (ty - hy) * stiffness_2 - hvy * damping_2;
        hvx += hax * dt;
        hvy += hay * dt;
        hx += hvx * dt;
        hy += hvy * dt;
        self.pointer_halo = point(px(hx), px(hy));
        self.pointer_halo_vel = (hvx, hvy);

        // Advance cursor trail sparks
        for spark in &mut self.trail {
            spark.age += dt;
        }
        self.trail.retain(|s| s.age < s.duration);

        // Advance click ripples
        for ripple in &mut self.ripples {
            ripple.age += dt;
        }
        self.ripples.retain(|r| r.age < r.duration);

        // Advance pet jump physics
        if self.pet_jump < 0.0 || self.pet_vel.abs() > 0.4 {
            self.pet_vel += 620.0 * dt;
            self.pet_jump += self.pet_vel * dt;
            if self.pet_jump >= 0.0 {
                self.pet_jump = 0.0;
                if self.pet_vel.abs() > 38.0 {
                    self.pet_vel = -self.pet_vel * 0.34;
                } else {
                    self.pet_vel = 0.0;
                }
            }
        }

        // Advance pet burst sparks
        for spark in &mut self.pet_sparks {
            spark.age += dt;
            spark.vel.1 += 180.0 * dt;
            spark.pos = point(
                px(f32::from(spark.pos.x) + spark.vel.0 * dt),
                px(f32::from(spark.pos.y) + spark.vel.1 * dt),
            );
        }
        self.pet_sparks.retain(|s| s.age < s.duration);

        let spring_moving = (tx - sx).abs() > 0.45
            || (ty - sy).abs() > 0.45
            || (tx - hx).abs() > 0.6
            || (ty - hy).abs() > 0.6
            || vx.abs() > 2.5
            || vy.abs() > 2.5;
        let fx_active = !self.trail.is_empty()
            || !self.ripples.is_empty()
            || !self.pet_sparks.is_empty()
            || self.pet_jump < -0.1
            || self.pet_vel.abs() > 0.4;
        let recently_moved = self.last_pointer_move.elapsed() < Duration::from_millis(1600);

        continuous_flow || spring_moving || fx_active || recently_moved
    }

    pub fn snapshot(
        &self,
        atmosphere: ThemeAtmosphere,
        accent: u32,
        is_dark: bool,
        reduced_motion: bool,
        pet_mood: Option<String>,
    ) -> MotionSnapshot {
        let is_prism = atmosphere == ThemeAtmosphere::Prism;
        let is_sky = atmosphere == ThemeAtmosphere::Sky;
        let speed = (self.pointer_vel.0 * self.pointer_vel.0
            + self.pointer_vel.1 * self.pointer_vel.1)
            .sqrt()
            .clamp(0.0, 900.0);
        let idle_secs = self.last_pointer_move.elapsed().as_secs_f32();
        let idle_fade = if !self.pointer_active {
            if is_prism { 0.65 } else { 0.0 }
        } else if idle_secs < 1.8 {
            1.0
        } else {
            (1.0 - (idle_secs - 1.8) / 2.4).clamp(if is_prism { 0.48 } else { 0.14 }, 1.0)
        };
        let pet_happy = self
            .pet_happy_until
            .is_some_and(|until| Instant::now() < until);
        MotionSnapshot {
            pointer_smooth: self.pointer_smooth,
            pointer_halo: self.pointer_halo,
            pointer_active: self.pointer_active,
            pointer_speed: speed,
            idle_fade,
            phase: self.phase,
            atmosphere,
            is_prism,
            is_sky,
            is_dark,
            accent,
            reduced_motion,
            trail: self.trail.iter().copied().collect(),
            ripples: self.ripples.clone(),
            pet_jump: self.pet_jump,
            pet_sparks: self.pet_sparks.clone(),
            pet_happy,
            pet_mood,
        }
    }
}

/// Smoothly samples the signature Obsidian Prism spectrum:
/// Champagne Gold -> Coral Rose -> Amethyst Violet -> Electric Cyan -> Aurora Mint -> Gold
pub fn spectral_color(t: f32, alpha: f32) -> Hsla {
    const STOPS: &[(f32, [f32; 3])] = &[
        (0.00, [0.984, 0.749, 0.141]), // #FBBF24 Gold
        (0.22, [0.984, 0.443, 0.522]), // #FB7185 Rose
        (0.46, [0.659, 0.333, 0.969]), // #A855F7 Violet
        (0.70, [0.220, 0.741, 0.973]), // #38BDF8 Cyan
        (0.88, [0.204, 0.827, 0.600]), // #34D399 Mint
        (1.00, [0.984, 0.749, 0.141]), // #FBBF24 Gold
    ];
    let p = t.rem_euclid(1.0);
    for w in STOPS.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if p >= t0 && p <= t1 {
            let local = ((p - t0) / (t1 - t0).max(0.0001)).clamp(0.0, 1.0);
            let r = c0[0] + (c1[0] - c0[0]) * local;
            let g = c0[1] + (c1[1] - c0[1]) * local;
            let b = c0[2] + (c1[2] - c0[2]) * local;
            return Hsla::from(Rgba {
                r,
                g,
                b,
                a: alpha.clamp(0.0, 1.0),
            });
        }
    }
    Hsla::from(Rgba {
        r: 0.659,
        g: 0.333,
        b: 0.969,
        a: alpha.clamp(0.0, 1.0),
    })
}

fn paint_glow_orb(
    window: &mut Window,
    center: Point<Pixels>,
    core_radius: f32,
    blur_radius: f32,
    color: Hsla,
) {
    if color.a <= 0.002
        || core_radius <= 0.0
        || blur_radius <= 0.0
        || !f32::from(center.x).is_finite()
        || !f32::from(center.y).is_finite()
    {
        return;
    }
    let d = core_radius * 2.0;
    let bounds = Bounds::new(
        point(
            px(f32::from(center.x) - core_radius),
            px(f32::from(center.y) - core_radius),
        ),
        size(px(d), px(d)),
    );
    window.paint_drop_shadows(
        bounds,
        Corners::all(px(core_radius)),
        &[BoxShadow {
            color,
            offset: point(px(0.), px(0.)),
            blur_radius: px(blur_radius),
            spread_radius: px(core_radius * 0.35),
            inset: false,
        }],
    );
}

/// Full-window ambient background canvas:
/// - In Obsidian Prism: floating parallax aurora orbs, corner optical dot-matrix, drifting stardust,
///   and 3-layer dispersed chromatic cursor spotlight (cyan leading, violet lagging, gold glint).
/// - In Forest / Paper / Night: subtle theme-tinted reading lamplight aura following the cursor.
pub fn ambient_stage_canvas(snap: MotionSnapshot, is_reader: bool) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let w = f32::from(bounds.size.width).max(1.0);
            let h = f32::from(bounds.size.height).max(1.0);
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let px_norm = ((f32::from(snap.pointer_smooth.x) - ox) / w - 0.5).clamp(-0.6, 0.6);
            let py_norm = ((f32::from(snap.pointer_smooth.y) - oy) / h - 0.5).clamp(-0.6, 0.6);

            // 1. Obsidian Prism atmospheric parallax nebulae & dot-matrix
            if snap.is_prism && !is_reader {
                let p = snap.phase;
                // Subtle optical dot-matrix on bottom-left & top-right (echoing obsidian-prism UI)
                let spacing = 22.0_f32;
                let cols = 10_usize;
                let rows = 8_usize;
                for row in 0..rows {
                    for col in 0..cols {
                        let gx = ox + 242.0 + col as f32 * spacing;
                        let gy = oy + h - 24.0 - row as f32 * spacing;
                        let edge_fade =
                            (1.0 - col as f32 / cols as f32) * (1.0 - row as f32 / rows as f32);
                        let dx = gx - f32::from(snap.pointer_smooth.x);
                        let dy = gy - f32::from(snap.pointer_smooth.y);
                        let near = (1.0 - (dx * dx + dy * dy).sqrt() / 220.0).clamp(0.0, 1.0);
                        let pulse =
                            0.72 + 0.28 * (p * 1.1 + col as f32 * 0.45 + row as f32 * 0.35).sin();
                        let alpha =
                            (0.05 + 0.14 * edge_fade * pulse + 0.28 * near).clamp(0.0, 0.45);
                        let dot_color = if near > 0.15 {
                            spectral_color(0.45 + near * 0.3 + p * 0.04, alpha)
                        } else {
                            Hsla::from(rgb(0x8fa8ff).opacity(alpha))
                        };
                        let r = 1.1 + near * 0.9;
                        window.paint_quad(PaintQuad {
                            bounds: Bounds::new(
                                point(px(gx - r), px(gy - r)),
                                size(px(r * 2.0), px(r * 2.0)),
                            ),
                            corner_radii: Corners::all(px(r)),
                            background: dot_color.into(),
                            border_widths: Edges::default(),
                            border_color: transparent_black(),
                            border_style: BorderStyle::default(),
                        });
                    }
                }

                // Flowing parallax aurora clouds
                let orb1 = point(
                    px(ox + w * (0.34 + 0.07 * (p * 0.32).sin()) - px_norm * 36.0),
                    px(oy + h * (0.20 + 0.05 * (p * 0.27).cos()) - py_norm * 24.0),
                );
                let orb2 = point(
                    px(ox + w * (0.76 + 0.06 * (p * 0.25 + 1.8).cos()) - px_norm * 44.0),
                    px(oy + h * (0.30 + 0.06 * (p * 0.31 + 0.9).sin()) - py_norm * 28.0),
                );
                let orb3 = point(
                    px(ox + w * (0.62 + 0.08 * (p * 0.22 + 3.4).sin()) + px_norm * 26.0),
                    px(oy + h * (0.78 + 0.05 * (p * 0.29).cos()) + py_norm * 20.0),
                );
                paint_glow_orb(
                    window,
                    orb1,
                    82.0,
                    185.0,
                    Hsla::from(rgb(0x7c3aed).opacity(0.18)),
                );
                paint_glow_orb(
                    window,
                    orb2,
                    76.0,
                    175.0,
                    Hsla::from(rgb(0x0ea5e9).opacity(0.16)),
                );
                paint_glow_orb(
                    window,
                    orb3,
                    68.0,
                    165.0,
                    Hsla::from(rgb(0xf59e0b).opacity(0.11)),
                );

                // Floating ambient micro-starlight motes
                for i in 0..14_usize {
                    let seed = i as f32 * 1.618_034;
                    let mx = ox
                        + w * (0.20 + ((seed * 3.17).sin() * 0.5 + 0.5) * 0.76)
                        + (p * 0.5 + seed).sin() * 14.0
                        - px_norm * 16.0;
                    let travel = ((p * 0.032 + seed * 0.37).rem_euclid(1.0)) * h;
                    let my = oy + h - travel;
                    let twinkle = 0.35 + 0.65 * ((p * 1.6 + seed * 2.3).sin() * 0.5 + 0.5);
                    let r = 1.2 + (i % 3) as f32 * 0.45;
                    let color = spectral_color((i as f32 / 14.0) + p * 0.03, 0.28 * twinkle);
                    window.paint_quad(PaintQuad {
                        bounds: Bounds::new(
                            point(px(mx - r), px(my - r)),
                            size(px(r * 2.0), px(r * 2.0)),
                        ),
                        corner_radii: Corners::all(px(r)),
                        background: color.into(),
                        border_widths: Edges::default(),
                        border_color: transparent_black(),
                        border_style: BorderStyle::default(),
                    });
                }
            } else if !is_reader {
                // Original hand-painted atmospheric lighting & gentle floating motes
                let p = if snap.reduced_motion { 0.0 } else { snap.phase };
                match snap.atmosphere {
                    ThemeAtmosphere::Sky => {
                        let cloud1 = point(
                            px(ox + w * (0.28 + 0.04 * (p * 0.16).sin()) - px_norm * 24.0),
                            px(oy + h * 0.16 - py_norm * 14.0),
                        );
                        let cloud2 = point(
                            px(ox + w * (0.78 + 0.05 * (p * 0.14 + 1.5).cos()) - px_norm * 32.0),
                            px(oy + h * 0.24 - py_norm * 18.0),
                        );
                        let cloud3 = point(
                            px(ox + w * (0.64 + 0.04 * (p * 0.12 + 3.0).sin()) + px_norm * 20.0),
                            px(oy + h * 0.84 + py_norm * 12.0),
                        );
                        paint_glow_orb(
                            window,
                            cloud1,
                            96.0,
                            180.0,
                            Hsla::from(rgb(0xffffff).opacity(0.42)),
                        );
                        paint_glow_orb(
                            window,
                            cloud2,
                            108.0,
                            195.0,
                            Hsla::from(rgb(0xffffff).opacity(0.38)),
                        );
                        paint_glow_orb(
                            window,
                            cloud3,
                            92.0,
                            170.0,
                            Hsla::from(rgb(0x93c5fd).opacity(0.24)),
                        );
                    }
                    ThemeAtmosphere::Totoro => {
                        // Dappled camphor forest sunbeams & gentle drifting woodland spores
                        let sun1 = point(
                            px(ox + w * (0.30 + 0.03 * (p * 0.14).sin()) - px_norm * 22.0),
                            px(oy + h * 0.18 - py_norm * 14.0),
                        );
                        let canopy2 = point(
                            px(ox + w * (0.76 + 0.04 * (p * 0.12 + 1.4).cos()) - px_norm * 28.0),
                            px(oy + h * 0.28 - py_norm * 16.0),
                        );
                        let moss3 = point(
                            px(ox + w * (0.62 + 0.03 * (p * 0.10 + 2.6).sin()) + px_norm * 18.0),
                            px(oy + h * 0.82 + py_norm * 10.0),
                        );
                        paint_glow_orb(
                            window,
                            sun1,
                            92.0,
                            185.0,
                            Hsla::from(rgb(0xfff5cc).opacity(0.36)),
                        );
                        paint_glow_orb(
                            window,
                            canopy2,
                            102.0,
                            190.0,
                            Hsla::from(rgb(0xa7ddb0).opacity(0.26)),
                        );
                        paint_glow_orb(
                            window,
                            moss3,
                            88.0,
                            165.0,
                            Hsla::from(rgb(0xdff5d8).opacity(0.32)),
                        );
                        if !snap.reduced_motion {
                            for i in 0..10_usize {
                                let seed = i as f32 * 1.618_034;
                                let mx = ox
                                    + w * (0.22 + ((seed * 2.91).sin() * 0.5 + 0.5) * 0.72)
                                    + (p * 0.42 + seed).sin() * 14.0
                                    - px_norm * 14.0;
                                let travel = ((p * 0.022 + seed * 0.31).rem_euclid(1.0)) * h;
                                let my = oy + h - travel;
                                let pulse =
                                    0.40 + 0.60 * ((p * 1.3 + seed * 2.1).sin() * 0.5 + 0.5);
                                let r = 1.6 + (i % 3) as f32 * 0.55;
                                let mote = if i % 2 == 0 {
                                    Hsla::from(rgb(0xfff1a8).opacity(0.45 * pulse))
                                } else {
                                    Hsla::from(rgb(0x7cc48b).opacity(0.32 * pulse))
                                };
                                window.paint_quad(PaintQuad {
                                    bounds: Bounds::new(
                                        point(px(mx - r), px(my - r)),
                                        size(px(r * 2.0), px(r * 2.0)),
                                    ),
                                    corner_radii: Corners::all(px(r)),
                                    background: mote.into(),
                                    border_widths: Edges::default(),
                                    border_color: transparent_black(),
                                    border_style: BorderStyle::default(),
                                });
                            }
                        }
                    }
                    ThemeAtmosphere::Spirited => {
                        // Soft apricot-rose twilight sky blooms & subtle mirror-water glints
                        let glow1 = point(
                            px(ox + w * (0.28 + 0.04 * (p * 0.13).sin()) - px_norm * 22.0),
                            px(oy + h * 0.22 - py_norm * 14.0),
                        );
                        let glow2 = point(
                            px(ox + w * (0.76 + 0.04 * (p * 0.11 + 1.6).cos()) - px_norm * 30.0),
                            px(oy + h * 0.26 - py_norm * 16.0),
                        );
                        let glow3 = point(
                            px(ox + w * (0.58 + 0.05 * (p * 0.10 + 3.1).sin()) + px_norm * 18.0),
                            px(oy + h * 0.80 + py_norm * 12.0),
                        );
                        paint_glow_orb(
                            window,
                            glow1,
                            98.0,
                            190.0,
                            Hsla::from(rgb(0xffd7c4).opacity(0.38)),
                        );
                        paint_glow_orb(
                            window,
                            glow2,
                            102.0,
                            190.0,
                            Hsla::from(rgb(0xf6bcc5).opacity(0.30)),
                        );
                        paint_glow_orb(
                            window,
                            glow3,
                            94.0,
                            175.0,
                            Hsla::from(rgb(0xb2c9eb).opacity(0.24)),
                        );
                        if !snap.reduced_motion {
                            for i in 0..8_usize {
                                let seed = i as f32 * 2.13;
                                let rx = ox
                                    + w * (0.24 + ((seed * 1.9).sin() * 0.5 + 0.5) * 0.70)
                                    + (p * 0.30 + seed).sin() * 18.0;
                                let ry = oy + h * (0.62 + (i as f32 / 8.0) * 0.30);
                                let shimmer = 0.25 + 0.75 * ((p * 1.2 + seed).sin() * 0.5 + 0.5);
                                let rw = 16.0 + (i % 3) as f32 * 10.0;
                                window.paint_quad(PaintQuad {
                                    bounds: Bounds::new(
                                        point(px(rx), px(ry)),
                                        size(px(rw), px(2.0)),
                                    ),
                                    corner_radii: Corners::all(px(1.0)),
                                    background: Hsla::from(rgb(0xfff0d9).opacity(0.32 * shimmer))
                                        .into(),
                                    border_widths: Edges::default(),
                                    border_color: transparent_black(),
                                    border_style: BorderStyle::default(),
                                });
                            }
                        }
                    }
                    ThemeAtmosphere::Howl => {
                        // Luminous alpine spring sky & gently drifting pastel wildflowers
                        let cloud1 = point(
                            px(ox + w * (0.30 + 0.04 * (p * 0.15).sin()) - px_norm * 24.0),
                            px(oy + h * 0.18 - py_norm * 14.0),
                        );
                        let cloud2 = point(
                            px(ox + w * (0.78 + 0.04 * (p * 0.13 + 1.3).cos()) - px_norm * 28.0),
                            px(oy + h * 0.26 - py_norm * 16.0),
                        );
                        let bloom3 = point(
                            px(ox + w * (0.64 + 0.04 * (p * 0.11 + 2.8).sin()) + px_norm * 18.0),
                            px(oy + h * 0.84 + py_norm * 12.0),
                        );
                        paint_glow_orb(
                            window,
                            cloud1,
                            96.0,
                            185.0,
                            Hsla::from(rgb(0xd4cbf7).opacity(0.32)),
                        );
                        paint_glow_orb(
                            window,
                            cloud2,
                            102.0,
                            190.0,
                            Hsla::from(rgb(0xffffff).opacity(0.40)),
                        );
                        paint_glow_orb(
                            window,
                            bloom3,
                            90.0,
                            170.0,
                            Hsla::from(rgb(0xfff1ba).opacity(0.28)),
                        );
                        if !snap.reduced_motion {
                            for i in 0..10_usize {
                                let seed = i as f32 * 1.732;
                                let drift = (p * 0.026 + seed * 0.29).rem_euclid(1.0);
                                let mx = ox + w * (0.18 + drift * 0.78);
                                let my = oy
                                    + h * (0.12 + ((seed * 2.4).sin() * 0.5 + 0.5) * 0.76)
                                    + (p * 0.8 + seed).sin() * 14.0;
                                let petal = match i % 3 {
                                    0 => Hsla::from(rgb(0xf9c8d6).opacity(0.46)),
                                    1 => Hsla::from(rgb(0xffffff).opacity(0.55)),
                                    _ => Hsla::from(rgb(0xd8cdfa).opacity(0.44)),
                                };
                                window.paint_quad(PaintQuad {
                                    bounds: Bounds::new(
                                        point(px(mx), px(my)),
                                        size(px(5.0), px(3.0)),
                                    ),
                                    corner_radii: Corners::all(px(1.5)),
                                    background: petal.into(),
                                    border_widths: Edges::default(),
                                    border_color: transparent_black(),
                                    border_style: BorderStyle::default(),
                                });
                            }
                        }
                    }
                    ThemeAtmosphere::Kiki => {
                        // Warm sunlit Mediterranean afternoon coastal glow & sea breeze glints
                        let sun1 = point(
                            px(ox + w * (0.30 + 0.04 * (p * 0.14).sin()) - px_norm * 22.0),
                            px(oy + h * 0.18 - py_norm * 14.0),
                        );
                        let coast2 = point(
                            px(ox + w * (0.76 + 0.04 * (p * 0.12 + 1.5).cos()) - px_norm * 28.0),
                            px(oy + h * 0.26 - py_norm * 16.0),
                        );
                        let sea3 = point(
                            px(ox + w * (0.60 + 0.04 * (p * 0.10 + 3.0).sin()) + px_norm * 18.0),
                            px(oy + h * 0.82 + py_norm * 12.0),
                        );
                        paint_glow_orb(
                            window,
                            sun1,
                            96.0,
                            185.0,
                            Hsla::from(rgb(0xffe7ba).opacity(0.40)),
                        );
                        paint_glow_orb(
                            window,
                            coast2,
                            98.0,
                            185.0,
                            Hsla::from(rgb(0xf5c4a2).opacity(0.28)),
                        );
                        paint_glow_orb(
                            window,
                            sea3,
                            92.0,
                            175.0,
                            Hsla::from(rgb(0xbae6fd).opacity(0.24)),
                        );
                    }
                    ThemeAtmosphere::Lantern => {
                        // Cozy night lantern glow & gently rising warm fireflies
                        let lamp1 = point(
                            px(ox + w * (0.32 + 0.04 * (p * 0.14).sin()) - px_norm * 24.0),
                            px(oy + h * 0.22 - py_norm * 14.0),
                        );
                        let lamp2 = point(
                            px(ox + w * (0.76 + 0.05 * (p * 0.12 + 1.8).cos()) - px_norm * 32.0),
                            px(oy + h * 0.30 - py_norm * 18.0),
                        );
                        let river3 = point(
                            px(ox + w * (0.64 + 0.04 * (p * 0.11 + 3.2).sin()) + px_norm * 20.0),
                            px(oy + h * 0.80 + py_norm * 14.0),
                        );
                        paint_glow_orb(
                            window,
                            lamp1,
                            84.0,
                            180.0,
                            Hsla::from(rgb(0x6d4c8c).opacity(0.16)),
                        );
                        paint_glow_orb(
                            window,
                            lamp2,
                            86.0,
                            185.0,
                            Hsla::from(rgb(0xf5b962).opacity(0.16)),
                        );
                        paint_glow_orb(
                            window,
                            river3,
                            74.0,
                            165.0,
                            Hsla::from(rgb(0xd96b4e).opacity(0.12)),
                        );
                        if !snap.reduced_motion {
                            for i in 0..12_usize {
                                let seed = i as f32 * 1.618_034;
                                let mx = ox
                                    + w * (0.20 + ((seed * 3.17).sin() * 0.5 + 0.5) * 0.76)
                                    + (p * 0.45 + seed).sin() * 14.0
                                    - px_norm * 16.0;
                                let travel = ((p * 0.025 + seed * 0.37).rem_euclid(1.0)) * h;
                                let my = oy + h - travel;
                                let twinkle =
                                    0.35 + 0.65 * ((p * 1.4 + seed * 2.3).sin() * 0.5 + 0.5);
                                let r = 1.4 + (i % 3) as f32 * 0.5;
                                let col = if i % 3 == 0 {
                                    Hsla::from(rgb(0xf7b267).opacity(0.42 * twinkle))
                                } else {
                                    Hsla::from(rgb(0xfde4a6).opacity(0.46 * twinkle))
                                };
                                window.paint_quad(PaintQuad {
                                    bounds: Bounds::new(
                                        point(px(mx - r), px(my - r)),
                                        size(px(r * 2.0), px(r * 2.0)),
                                    ),
                                    corner_radii: Corners::all(px(r)),
                                    background: col.into(),
                                    border_widths: Edges::default(),
                                    border_color: transparent_black(),
                                    border_style: BorderStyle::default(),
                                });
                            }
                        }
                    }
                    ThemeAtmosphere::Prism | ThemeAtmosphere::Classic => {}
                }
            }

            // 2. Mouse-following optical spotlight & chromatic dispersion aura
            if !snap.reduced_motion && snap.idle_fade > 0.02 {
                let speed_boost = (snap.pointer_speed / 550.0).clamp(0.0, 0.45);
                let reader_scale = if is_reader { 0.42 } else { 1.0 };
                let intensity = snap.idle_fade * reader_scale;

                if snap.is_prism {
                    let hue_shift = (snap.phase * 0.06
                        + f32::from(snap.pointer_smooth.x) * 0.00055)
                        .rem_euclid(1.0);
                    // Lagging Violet-Rose dispersion halo
                    paint_glow_orb(
                        window,
                        snap.pointer_halo,
                        46.0,
                        145.0,
                        spectral_color(hue_shift + 0.44, (0.22 + speed_boost * 0.12) * intensity),
                    );
                    // Leading Cyan-Sapphire spotlight
                    paint_glow_orb(
                        window,
                        snap.pointer_smooth,
                        34.0,
                        108.0,
                        spectral_color(hue_shift + 0.70, (0.20 + speed_boost * 0.14) * intensity),
                    );
                    // Warm Gold-Amber optical core glint
                    let mid = point(
                        px(
                            (f32::from(snap.pointer_smooth.x) + f32::from(snap.pointer_halo.x))
                                * 0.5,
                        ),
                        px(
                            (f32::from(snap.pointer_smooth.y) + f32::from(snap.pointer_halo.y))
                                * 0.5,
                        ),
                    );
                    paint_glow_orb(
                        window,
                        mid,
                        18.0,
                        58.0,
                        spectral_color(hue_shift, (0.17 + speed_boost * 0.12) * intensity),
                    );
                } else if snap.is_sky {
                    // Sunlit sky-blue & cloud-white cursor aura for Laputa theme
                    paint_glow_orb(
                        window,
                        snap.pointer_smooth,
                        36.0,
                        118.0,
                        Hsla::from(rgb(0x38bdf8).opacity(0.11 * intensity)),
                    );
                    paint_glow_orb(
                        window,
                        snap.pointer_halo,
                        28.0,
                        94.0,
                        Hsla::from(rgb(0xffffff).opacity(0.26 * intensity)),
                    );
                } else if snap.atmosphere == ThemeAtmosphere::Totoro {
                    paint_glow_orb(
                        window,
                        snap.pointer_smooth,
                        36.0,
                        116.0,
                        Hsla::from(rgb(0x5fa370).opacity(0.11 * intensity)),
                    );
                    paint_glow_orb(
                        window,
                        snap.pointer_halo,
                        28.0,
                        92.0,
                        Hsla::from(rgb(0xfff4c2).opacity(0.24 * intensity)),
                    );
                } else if snap.atmosphere == ThemeAtmosphere::Spirited {
                    paint_glow_orb(
                        window,
                        snap.pointer_smooth,
                        36.0,
                        116.0,
                        Hsla::from(rgb(0xd97365).opacity(0.11 * intensity)),
                    );
                    paint_glow_orb(
                        window,
                        snap.pointer_halo,
                        28.0,
                        92.0,
                        Hsla::from(rgb(0xffebd6).opacity(0.25 * intensity)),
                    );
                } else if snap.atmosphere == ThemeAtmosphere::Howl {
                    paint_glow_orb(
                        window,
                        snap.pointer_smooth,
                        36.0,
                        116.0,
                        Hsla::from(rgb(0x8b7acc).opacity(0.11 * intensity)),
                    );
                    paint_glow_orb(
                        window,
                        snap.pointer_halo,
                        28.0,
                        92.0,
                        Hsla::from(rgb(0xfff5cc).opacity(0.24 * intensity)),
                    );
                } else if snap.atmosphere == ThemeAtmosphere::Kiki {
                    paint_glow_orb(
                        window,
                        snap.pointer_smooth,
                        36.0,
                        116.0,
                        Hsla::from(rgb(0xd98752).opacity(0.11 * intensity)),
                    );
                    paint_glow_orb(
                        window,
                        snap.pointer_halo,
                        28.0,
                        92.0,
                        Hsla::from(rgb(0xfff4d9).opacity(0.25 * intensity)),
                    );
                } else if snap.atmosphere == ThemeAtmosphere::Lantern {
                    paint_glow_orb(
                        window,
                        snap.pointer_smooth,
                        36.0,
                        120.0,
                        Hsla::from(rgb(0xe69f52).opacity(0.17 * intensity)),
                    );
                    paint_glow_orb(
                        window,
                        snap.pointer_halo,
                        24.0,
                        88.0,
                        Hsla::from(rgb(0xfde6a8).opacity(0.13 * intensity)),
                    );
                } else {
                    // Subtle warm reading lamplight following the cursor in Paper/Night
                    let aura_alpha = if snap.is_dark { 0.15 } else { 0.09 } * intensity;
                    paint_glow_orb(
                        window,
                        snap.pointer_smooth,
                        32.0,
                        112.0,
                        Hsla::from(rgb(snap.accent).opacity(aura_alpha)),
                    );
                    paint_glow_orb(
                        window,
                        snap.pointer_halo,
                        22.0,
                        84.0,
                        Hsla::from(rgb(0xf4c97a).opacity(aura_alpha * 0.55)),
                    );
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Foreground non-blocking optical canvas for click ripples, fast-sweep stardust trail, and pet burst sparks.
pub fn cursor_fx_overlay_canvas(snap: MotionSnapshot) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_bounds, _, window, _| {
            if snap.reduced_motion {
                return;
            }

            // 1. Cursor stardust wake sparks
            for spark in &snap.trail {
                let progress = (spark.age / spark.duration).clamp(0.0, 1.0);
                let fade = (1.0 - progress).powi(2);
                let r = spark.size * (0.45 + 0.55 * (1.0 - progress));
                let drift_y = -progress * 10.0;
                let center = point(spark.pos.x, px(f32::from(spark.pos.y) + drift_y));
                let color = if snap.is_prism {
                    spectral_color(spark.hue, fade * 0.68)
                } else {
                    Hsla::from(rgb(snap.accent).opacity(fade * 0.34))
                };
                if snap.is_prism {
                    paint_glow_orb(window, center, r * 0.7, 12.0, color.opacity(0.65));
                }
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(
                            px(f32::from(center.x) - r * 0.5),
                            px(f32::from(center.y) - r * 0.5),
                        ),
                        size(px(r), px(r)),
                    ),
                    corner_radii: Corners::all(px(r * 0.5)),
                    background: color.into(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }

            // 2. Click optical prism ripples
            for ripple in &snap.ripples {
                let t = (ripple.age / ripple.duration).clamp(0.0, 1.0);
                let eased = 1.0 - (1.0 - t).powi(3);
                let fade = (1.0 - t).powi(2);
                let outer_r = 10.0 + eased * if ripple.is_prism { 86.0 } else { 64.0 };
                let ring_color = if ripple.is_prism {
                    spectral_color(ripple.hue_offset + t * 0.45, fade * 0.72)
                } else {
                    Hsla::from(rgb(snap.accent).opacity(fade * 0.42))
                };
                let outer_bounds = Bounds::new(
                    point(
                        px(f32::from(ripple.origin.x) - outer_r),
                        px(f32::from(ripple.origin.y) - outer_r),
                    ),
                    size(px(outer_r * 2.0), px(outer_r * 2.0)),
                );
                window.paint_quad(PaintQuad {
                    bounds: outer_bounds,
                    corner_radii: Corners::all(px(outer_r)),
                    background: transparent_black().into(),
                    border_widths: Edges::all(px(if ripple.is_prism { 1.6 } else { 1.2 })),
                    border_color: ring_color,
                    border_style: BorderStyle::Solid,
                });

                // Inner chromatic dispersion ring in Prism mode
                if ripple.is_prism {
                    let inner_r = outer_r * 0.66;
                    let inner_color =
                        spectral_color(ripple.hue_offset + 0.52 + t * 0.35, fade * 0.52);
                    let inner_bounds = Bounds::new(
                        point(
                            px(f32::from(ripple.origin.x) - inner_r),
                            px(f32::from(ripple.origin.y) - inner_r),
                        ),
                        size(px(inner_r * 2.0), px(inner_r * 2.0)),
                    );
                    window.paint_quad(PaintQuad {
                        bounds: inner_bounds,
                        corner_radii: Corners::all(px(inner_r)),
                        background: transparent_black().into(),
                        border_widths: Edges::all(px(1.1)),
                        border_color: inner_color,
                        border_style: BorderStyle::Solid,
                    });
                }
            }

            // 3. Pet interaction pixel burst sparks
            for spark in &snap.pet_sparks {
                let t = (spark.age / spark.duration).clamp(0.0, 1.0);
                let fade = (1.0 - t).powi(2);
                let sz = 4.0 * (0.5 + 0.5 * (1.0 - t));
                let color = spectral_color(spark.hue, fade * 0.92);
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(
                            px(f32::from(spark.pos.x) - sz * 0.5),
                            px(f32::from(spark.pos.y) - sz * 0.5),
                        ),
                        size(px(sz), px(sz)),
                    ),
                    corner_radii: Corners::all(px(1.0)),
                    background: color.into(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Signature Obsidian Prism flowing iridescent border beam (horizontal):
/// Tracks the user's cursor X coordinate across the panel AND flows gently over time,
/// recreating the razor-sharp spectrum edge glow from the Obsidian Prism reference.
pub fn prism_beam_horizontal(
    snap: MotionSnapshot,
    thickness: f32,
    phase_offset: f32,
    always_rainbow: bool,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let w = f32::from(bounds.size.width);
            if !w.is_finite() || w < 4.0 {
                return;
            }
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let h = f32::from(bounds.size.height).max(thickness);

            // Compute mouse focus projected onto this bar + gentle ambient drift
            let drift = if snap.reduced_motion {
                0.5
            } else {
                0.5 + 0.22 * (snap.phase * 0.58 + phase_offset).sin()
            };
            let mx_local = (f32::from(snap.pointer_smooth.x) - ox) / w;
            let dy = (f32::from(snap.pointer_smooth.y) - oy).abs();
            let proximity = if snap.pointer_active {
                (1.0 - dy / 360.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let focal = if snap.pointer_active && (-0.2..=1.2).contains(&mx_local) {
                let clamped = mx_local.clamp(0.08, 0.92);
                drift * (1.0 - proximity * 0.72) + clamped * (proximity * 0.72)
            } else {
                drift
            };

            let rainbow = snap.is_prism || always_rainbow;
            let segments: usize = 8;
            let seg_w = w / segments as f32;
            let boost = 0.72 + proximity * 0.28;

            for i in 0..segments {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let env = |t: f32| -> f32 {
                    let edge_fade = (t * 6.0).min((1.0 - t) * 6.0).clamp(0.0, 1.0);
                    let dist = ((t - focal) / 0.42).abs();
                    let bell = (-dist * dist * 1.45).exp();
                    edge_fade * (0.24 + 0.76 * bell) * boost
                };
                let a0 = env(t0);
                let a1 = env(t1);
                let (c0, c1) = if rainbow {
                    let shift = if snap.reduced_motion {
                        phase_offset * 0.18
                    } else {
                        snap.phase * 0.08 + phase_offset * 0.18
                    };
                    (
                        spectral_color(t0 * 0.92 + shift, a0 * 0.96),
                        spectral_color(t1 * 0.92 + shift, a1 * 0.96),
                    )
                } else {
                    let base = rgb(snap.accent);
                    let warm = rgb(0xf3d38b);
                    let mix = |t: f32, a: f32| {
                        let glow = ((t - focal).abs() < 0.18) as u8;
                        if glow == 1 {
                            Hsla::from(warm.opacity(a * 0.58))
                        } else {
                            Hsla::from(base.opacity(a * 0.46))
                        }
                    };
                    (mix(t0, a0), mix(t1, a1))
                };

                let seg_bounds = Bounds::new(
                    point(px(ox + i as f32 * seg_w), px(oy)),
                    size(px(seg_w + 0.6), px(h)),
                );
                let grad =
                    linear_gradient(90.0, linear_color_stop(c0, 0.0), linear_color_stop(c1, 1.0));
                window.paint_quad(fill(seg_bounds, grad));
            }

            // Soft optical bloom directly around the mouse-tracked focal segment
            let max_glow = (w - 8.0).max(8.0);
            let glow_w = (w * 0.34).min(280.0).min(max_glow).max(8.0);
            let min_x = ox + 4.0;
            let max_x = (ox + w - glow_w - 4.0).max(min_x);
            let glow_x = (ox + focal * w - glow_w * 0.5).clamp(min_x, max_x);
            let glow_color = if rainbow {
                spectral_color(
                    focal * 0.9 + snap.phase * 0.08 + phase_offset * 0.18,
                    0.40 * boost,
                )
            } else {
                Hsla::from(rgb(snap.accent).opacity(0.24 * boost))
            };
            window.paint_drop_shadows(
                Bounds::new(point(px(glow_x), px(oy)), size(px(glow_w), px(h))),
                Corners::all(px(h)),
                &[BoxShadow {
                    color: glow_color,
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(if rainbow { 14. } else { 10. }),
                    spread_radius: px(1.5),
                    inset: false,
                }],
            );
        },
    )
    .w_full()
    .h(px(thickness))
}

/// Vertical mouse-reactive prismatic shimmer along the sidebar border edge.
pub fn prism_rail_vertical(snap: MotionSnapshot) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let h = f32::from(bounds.size.height).max(1.0);
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let w = f32::from(bounds.size.width).max(1.5);

            let drift = if snap.reduced_motion {
                0.35
            } else {
                0.45 + 0.28 * (snap.phase * 0.45).sin()
            };
            let my_local = (f32::from(snap.pointer_smooth.y) - oy) / h;
            let dx = (f32::from(snap.pointer_smooth.x) - ox).abs();
            let proximity = if snap.pointer_active {
                (1.0 - dx / 280.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let focal = if snap.pointer_active {
                drift * (1.0 - proximity * 0.78) + my_local.clamp(0.06, 0.94) * (proximity * 0.78)
            } else {
                drift
            };

            let segments: usize = 8;
            let seg_h = h / segments as f32;
            for i in 0..segments {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let env = |t: f32| -> f32 {
                    let dist = ((t - focal) / 0.28).abs();
                    (-dist * dist * 1.8).exp() * (0.35 + proximity * 0.65)
                };
                let (c0, c1) = if snap.is_prism {
                    let shift = snap.phase * 0.07;
                    (
                        spectral_color(t0 * 0.85 + shift, env(t0) * 0.85),
                        spectral_color(t1 * 0.85 + shift, env(t1) * 0.85),
                    )
                } else {
                    (
                        Hsla::from(rgb(snap.accent).opacity(env(t0) * 0.36)),
                        Hsla::from(rgb(snap.accent).opacity(env(t1) * 0.36)),
                    )
                };
                let seg_bounds = Bounds::new(
                    point(px(ox), px(oy + i as f32 * seg_h)),
                    size(px(w), px(seg_h + 0.6)),
                );
                window.paint_quad(fill(
                    seg_bounds,
                    linear_gradient(
                        180.0,
                        linear_color_stop(c0, 0.0),
                        linear_color_stop(c1, 1.0),
                    ),
                ));
            }
        },
    )
    .absolute()
    .right_0()
    .top_0()
    .w(px(2.))
    .h_full()
}

/// Interactive Pixel Prism Cat & Warm Lamp Spirit perched above cards
/// (directly inspired by Obsidian Prism's Clawd mascot & the user's screenshot).
/// Their glowing eyes follow the user's mouse cursor in real time and bounce happily on click.
pub fn prism_companion_canvas(snap: MotionSnapshot) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let p = 3.0_f32; // crisp 3px pixel grid
            let bob = if snap.reduced_motion {
                0.0
            } else {
                (snap.phase * 2.2).sin() * 1.4
            };
            let jump = snap.pet_jump;

            // Compute eye gaze direction toward user's mouse cursor
            let cat_cx = ox + 22.0;
            let cat_cy = oy + 20.0 + jump;
            let dx = f32::from(snap.pointer_smooth.x) - cat_cx;
            let dy = f32::from(snap.pointer_smooth.y) - cat_cy;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let gaze_x = (dx / dist) * 1.4;
            let gaze_y = (dy / dist) * 1.1;

            // Mood-reactive companion aura halo (painted before px_block borrows window)
            let mood_tint = match snap.pet_mood.as_deref() {
                Some("疲惫") => Some(0xf59e0b),
                Some("孤独") => Some(0xc084fc),
                Some("迷茫") => Some(0x38bdf8),
                Some("失落") => Some(0x60a5fa),
                Some("成长") => Some(0x34d399),
                Some("希望") => Some(0xfbbf24),
                _ => None,
            };
            if let Some(tint) = mood_tint {
                paint_glow_orb(
                    window,
                    point(px(ox + 42.0), px(oy + 22.0 + jump * 0.5)),
                    14.0,
                    24.0,
                    Hsla::from(rgb(tint).opacity(if snap.pet_happy { 0.34 } else { 0.22 })),
                );
            }

            let mut px_block = |gx: f32, gy: f32, gw: f32, gh: f32, color: Hsla| {
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(ox + gx * p), px(oy + gy * p + jump)),
                        size(px(gw * p), px(gh * p)),
                    ),
                    corner_radii: Corners::all(px(0.5)),
                    background: color.into(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            };

            // --- Mascot 1: Theme-Harmonized Pixel Cat Companion ---
            let (cat_main_hex, cat_hi_hex, cat_dark_hex, default_eye_hex) = match snap.atmosphere {
                ThemeAtmosphere::Prism => (0x9b87ff, 0xc4b5fd, 0x4c3f91, 0x7dd3fc),
                ThemeAtmosphere::Sky => (0x3b82c4, 0xbae6fd, 0x1e4e79, 0x7dd3fc),
                ThemeAtmosphere::Totoro => (0x6e8575, 0xcde3ca, 0x3b4f41, 0xfde68a),
                ThemeAtmosphere::Spirited => (0xc46b66, 0xfad8cf, 0x6b3234, 0xfff0c2),
                ThemeAtmosphere::Howl => (0x7a6cb5, 0xdcd5fb, 0x453b73, 0xfef08a),
                ThemeAtmosphere::Kiki => (0x282532, 0x565169, 0x121018, 0xfbbf24),
                ThemeAtmosphere::Lantern => (0x50436b, 0x9b89ba, 0x261e36, 0xfde29f),
                ThemeAtmosphere::Classic => (0x8a79d8, 0xc4b5fd, 0x4c3f91, 0x7dd3fc),
            };
            let cat_main = Hsla::from(rgb(cat_main_hex));
            let cat_hi = Hsla::from(rgb(cat_hi_hex));
            let cat_dark = Hsla::from(rgb(cat_dark_hex));
            let eye_bg = Hsla::from(rgb(0x0b0e18));
            let eye_glow = if snap.pet_happy {
                spectral_color(snap.phase * 0.4, 1.0)
            } else if let Some(tint) = mood_tint {
                Hsla::from(rgb(tint))
            } else {
                Hsla::from(rgb(default_eye_hex))
            };

            // Ears (subtle twitch)
            let ear_lift = if !snap.reduced_motion && (snap.phase * 1.7).sin() > 0.88 {
                -0.35
            } else {
                0.0
            };
            px_block(2.0, 2.0 + ear_lift, 2.0, 2.0, cat_main);
            px_block(9.0, 2.0, 2.0, 2.0, cat_main);
            px_block(2.5, 2.6 + ear_lift, 1.0, 1.0, cat_hi);
            px_block(9.5, 2.6, 1.0, 1.0, cat_hi);

            // Body & head block
            px_block(2.0, 4.0, 9.0, 5.0, cat_main);
            px_block(3.0, 4.0, 7.0, 1.0, cat_hi); // top specular rim
            px_block(2.0, 8.0, 9.0, 1.0, cat_dark); // belly shadow
            if snap.atmosphere == ThemeAtmosphere::Kiki {
                // Tiny crimson ribbon accent for the harbor bakery companion cat
                px_block(4.5, 7.8, 4.0, 0.9, Hsla::from(rgb(0xd9534f)));
            } else if snap.atmosphere == ThemeAtmosphere::Totoro {
                // Soft cream woodland belly patch
                px_block(4.0, 7.2, 5.0, 1.4, Hsla::from(rgb(0xf2f0df)));
            }

            // Cute pixel legs
            px_block(2.5, 9.0, 1.3, 2.0, cat_dark);
            px_block(4.5, 9.0, 1.3, 2.0, cat_main);
            px_block(7.5, 9.0, 1.3, 2.0, cat_main);
            px_block(9.2, 9.0, 1.3, 2.0, cat_dark);

            // Wagging tail
            let wag = if snap.reduced_motion {
                0.0
            } else {
                (snap.phase * 3.6).sin() * 0.7
            };
            px_block(11.0, 6.4 + wag * 0.4, 1.5, 1.1, cat_main);
            px_block(12.2, 5.2 + wag, 1.1, 1.6, cat_hi);

            // Eyes following mouse cursor!
            px_block(4.0, 5.5, 1.6, 1.6, eye_bg);
            px_block(7.4, 5.5, 1.6, 1.6, eye_bg);
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(ox + 4.35 * p + gaze_x), px(oy + 5.8 * p + jump + gaze_y)),
                    size(px(2.4), px(2.4)),
                ),
                corner_radii: Corners::all(px(1.2)),
                background: eye_glow.into(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(ox + 7.75 * p + gaze_x), px(oy + 5.8 * p + jump + gaze_y)),
                    size(px(2.4), px(2.4)),
                ),
                corner_radii: Corners::all(px(1.2)),
                background: eye_glow.into(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });

            // --- Mascot 2: Warm Amber Reading Buddy + Floating Prism Crystal ---
            let buddy_main = Hsla::from(rgb(0xd97745));
            let buddy_hi = Hsla::from(rgb(0xf4a26e));
            let buddy_dark = Hsla::from(rgb(0x8c4422));
            let b_off = 16.5_f32;
            let b_jump = jump * 0.72;

            // Floating crystal prism diamond glow above buddy's head (Laputa levitation crystal in sky theme)
            let crystal_y = 0.8 + bob * 0.32;
            let crystal_color = if snap.is_prism || snap.pet_happy {
                spectral_color(snap.phase * 0.16 + 0.65, 0.95)
            } else if let Some(tint) = mood_tint {
                Hsla::from(rgb(tint).opacity(0.95))
            } else {
                match snap.atmosphere {
                    ThemeAtmosphere::Sky => Hsla::from(rgb(0x38bdf8).opacity(0.96)),
                    ThemeAtmosphere::Totoro => Hsla::from(rgb(0x74c686).opacity(0.96)),
                    ThemeAtmosphere::Spirited => Hsla::from(rgb(0xf2a68d).opacity(0.96)),
                    ThemeAtmosphere::Howl => Hsla::from(rgb(0xb8a8f5).opacity(0.96)),
                    ThemeAtmosphere::Kiki => Hsla::from(rgb(0xf5b762).opacity(0.96)),
                    ThemeAtmosphere::Lantern => Hsla::from(rgb(0xf5b962).opacity(0.98)),
                    ThemeAtmosphere::Prism | ThemeAtmosphere::Classic => {
                        Hsla::from(rgb(0xb8d4f4).opacity(0.92))
                    }
                }
            };
            paint_glow_orb(
                window,
                point(
                    px(ox + (b_off + 4.5) * p),
                    px(oy + (crystal_y + 1.1) * p + b_jump),
                ),
                3.5,
                10.0,
                crystal_color.opacity(0.45),
            );

            let mut bx_block = |gx: f32, gy: f32, gw: f32, gh: f32, color: Hsla| {
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(ox + (b_off + gx) * p), px(oy + gy * p + b_jump)),
                        size(px(gw * p), px(gh * p)),
                    ),
                    corner_radii: Corners::all(px(0.5)),
                    background: color.into(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            };
            bx_block(4.0, crystal_y, 1.0, 2.4, crystal_color);
            bx_block(3.3, crystal_y + 0.7, 2.4, 1.0, crystal_color);

            // Buddy body
            bx_block(1.0, 4.4, 7.0, 4.6, buddy_main);
            bx_block(1.5, 4.4, 6.0, 1.0, buddy_hi);
            bx_block(0.2, 5.6, 0.9, 1.8, buddy_main); // left arm
            bx_block(8.0, 5.6, 0.9, 1.8, buddy_main); // right arm holding tiny lantern
            bx_block(8.4, 4.6, 1.6, 1.6, Hsla::from(rgb(0xfde68a))); // lantern glow
            bx_block(1.0, 8.2, 7.0, 0.8, buddy_dark);
            bx_block(2.0, 9.0, 1.2, 2.0, buddy_dark);
            bx_block(5.8, 9.0, 1.2, 2.0, buddy_dark);

            // Buddy eyes tracking cursor
            bx_block(2.6, 5.8, 1.2, 1.3, eye_bg);
            bx_block(5.2, 5.8, 1.2, 1.3, eye_bg);
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(
                        px(ox + (b_off + 2.85) * p + gaze_x * 0.7),
                        px(oy + 6.05 * p + b_jump + gaze_y * 0.7),
                    ),
                    size(px(1.8), px(1.8)),
                ),
                corner_radii: Corners::all(px(0.9)),
                background: Hsla::from(rgb(0xfef08a)).into(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(
                        px(ox + (b_off + 5.45) * p + gaze_x * 0.7),
                        px(oy + 6.05 * p + b_jump + gaze_y * 0.7),
                    ),
                    size(px(1.8), px(1.8)),
                ),
                corner_radii: Corners::all(px(0.9)),
                background: Hsla::from(rgb(0xfef08a)).into(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });
        },
    )
    .w(px(86.))
    .h(px(36.))
}

/// Interactive Borosilicate Reading Light Vial (inspired by Obsidian Prism's water vial).
/// Displays live reading progress level with an animated wave meniscus & glowing micro-bubbles.
pub fn light_vial_canvas(snap: MotionSnapshot, fill_ratio: f32) -> impl IntoElement {
    let ratio = fill_ratio.clamp(0.18, 0.92);
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            if !w.is_finite() || !h.is_finite() || w < 6.0 || h < 10.0 {
                return;
            }

            // Outer borosilicate capsule glass shell
            let shell_bg = if snap.is_dark {
                Hsla::from(rgb(0x0b0e18).opacity(0.85))
            } else {
                Hsla::from(rgb(0xffffff).opacity(0.65))
            };
            let shell_border = if snap.is_prism {
                spectral_color(snap.phase * 0.08 + 0.65, 0.48)
            } else {
                Hsla::from(rgb(snap.accent).opacity(0.38))
            };
            window.paint_quad(PaintQuad {
                bounds,
                corner_radii: Corners::all(px(w * 0.46)),
                background: shell_bg.into(),
                border_widths: Edges::all(px(1.2)),
                border_color: shell_border,
                border_style: BorderStyle::Solid,
            });

            // Liquid columns creating a smooth animated wave meniscus
            let inset = 2.5_f32;
            let inner_w = (w - inset * 2.0).max(4.0);
            let inner_h = (h - inset * 2.0).max(8.0);
            let cols = 6_usize;
            let col_w = inner_w / cols as f32;
            for c in 0..cols {
                let frac = c as f32 / (cols - 1).max(1) as f32;
                let wave = if snap.reduced_motion {
                    0.0
                } else {
                    (snap.phase * 2.4 + frac * 4.2).sin() * 2.0
                };
                let max_liq = (inner_h - 1.0).max(5.0);
                let liq_h = (inner_h * ratio + wave).clamp(5.0, max_liq);
                let lx = ox + inset + c as f32 * col_w;
                let ly = oy + h - inset - liq_h;
                let top_col = if snap.is_prism {
                    spectral_color(0.55 + frac * 0.25 + snap.phase * 0.05, 0.88)
                } else {
                    Hsla::from(rgb(snap.accent).opacity(0.86))
                };
                let bot_col = if snap.is_prism {
                    spectral_color(0.35 + frac * 0.22 + snap.phase * 0.05, 0.62)
                } else {
                    Hsla::from(rgb(snap.accent).opacity(0.42))
                };
                let grad = linear_gradient(
                    180.0,
                    linear_color_stop(top_col, 0.0),
                    linear_color_stop(bot_col, 1.0),
                );
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(point(px(lx), px(ly)), size(px(col_w + 0.5), px(liq_h))),
                    corner_radii: Corners {
                        top_left: px(1.5),
                        top_right: px(1.5),
                        bottom_left: px(if c == 0 { 6.0 } else { 0.0 }),
                        bottom_right: px(if c + 1 == cols { 6.0 } else { 0.0 }),
                    },
                    background: grad,
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }

            // Rising micro-bubble inside liquid
            if !snap.reduced_motion {
                let b_prog = (snap.phase * 0.35).rem_euclid(1.0);
                let bx = ox + w * (0.42 + 0.14 * (snap.phase * 1.9).sin());
                let by = oy + h - inset - 4.0 - b_prog * (inner_h * ratio - 6.0).max(2.0);
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(point(px(bx), px(by)), size(px(2.4), px(2.4))),
                    corner_radii: Corners::all(px(1.2)),
                    background: Hsla::from(rgb(0xffffff).opacity(0.72 * (1.0 - b_prog))).into(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }

            // Crisp vertical borosilicate glass specular highlight
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(ox + 4.2), px(oy + 6.0)),
                    size(px(1.6), px((h - 14.0).max(6.0))),
                ),
                corner_radii: Corners::all(px(1.0)),
                background: Hsla::from(rgb(0xffffff).opacity(0.36)).into(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });
        },
    )
    .w(px(22.))
    .h(px(40.))
}

#[derive(Clone, Copy, Debug)]
pub struct BreathingPhase {
    pub label: &'static str,
    pub cue: &'static str,
    pub seconds_left: u32,
    pub radius_factor: f32,
    pub color_t: f32,
}

/// Computes the 4-7-8 breathing ritual phase from elapsed seconds.
pub fn breathing_phase(elapsed_secs: f32) -> BreathingPhase {
    let cycle = elapsed_secs.max(0.0) % 19.0;
    if cycle < 4.0 {
        let p = (cycle / 4.0).clamp(0.0, 1.0);
        let eased = 0.5 - 0.5 * (p * std::f32::consts::PI).cos();
        BreathingPhase {
            label: "慢慢吸气",
            cue: "INHALE · 4 秒 · 感受微光充满胸腔",
            seconds_left: (4.0 - cycle).ceil().clamp(1.0, 4.0) as u32,
            radius_factor: 0.42 + 0.58 * eased,
            color_t: 0.68,
        }
    } else if cycle < 11.0 {
        let p = ((cycle - 4.0) / 7.0).clamp(0.0, 1.0);
        let pulse = 0.03 * (p * std::f32::consts::TAU * 2.0).sin();
        BreathingPhase {
            label: "轻轻屏息",
            cue: "HOLD · 7 秒 · 让思绪停在云端",
            seconds_left: (11.0 - cycle).ceil().clamp(1.0, 7.0) as u32,
            radius_factor: (1.0 + pulse).clamp(0.94, 1.02),
            color_t: 0.46,
        }
    } else {
        let p = ((cycle - 11.0) / 8.0).clamp(0.0, 1.0);
        let eased = 0.5 - 0.5 * (p * std::f32::consts::PI).cos();
        BreathingPhase {
            label: "缓缓呼出",
            cue: "EXHALE · 8 秒 · 把疲惫交给风",
            seconds_left: (19.0 - cycle).ceil().clamp(1.0, 8.0) as u32,
            radius_factor: 1.0 - 0.58 * eased,
            color_t: 0.85,
        }
    }
}

/// Serene 4-7-8 GPU Optical Breathing Orb rendered inside the Mood page sanctuary card.
pub fn breathing_orb_canvas(snap: MotionSnapshot, elapsed_secs: f32) -> impl IntoElement {
    let bp = breathing_phase(elapsed_secs);
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let w = f32::from(bounds.size.width).max(1.0);
            let h = f32::from(bounds.size.height).max(1.0);
            let center = point(px(ox + w * 0.5), px(oy + h * 0.5));
            let max_r = (w.min(h) * 0.44).max(18.0);
            let factor = if snap.reduced_motion {
                0.76
            } else {
                bp.radius_factor
            };
            let core_r = max_r * factor;

            let primary = if snap.is_prism {
                spectral_color(bp.color_t + snap.phase * 0.02, 0.85)
            } else {
                match snap.atmosphere {
                    ThemeAtmosphere::Sky => Hsla::from(rgb(0x38bdf8).opacity(0.84)),
                    ThemeAtmosphere::Totoro => Hsla::from(rgb(0x4d9464).opacity(0.84)),
                    ThemeAtmosphere::Spirited => Hsla::from(rgb(0xd46a5e).opacity(0.84)),
                    ThemeAtmosphere::Howl => Hsla::from(rgb(0x7c6cc4).opacity(0.84)),
                    ThemeAtmosphere::Kiki => Hsla::from(rgb(0xc9733e).opacity(0.84)),
                    ThemeAtmosphere::Lantern => Hsla::from(rgb(0xe69f52).opacity(0.86)),
                    ThemeAtmosphere::Prism | ThemeAtmosphere::Classic => {
                        Hsla::from(rgb(snap.accent).opacity(0.82))
                    }
                }
            };
            let secondary = if snap.is_prism {
                spectral_color(bp.color_t + 0.25 + snap.phase * 0.02, 0.42)
            } else {
                match snap.atmosphere {
                    ThemeAtmosphere::Sky => Hsla::from(rgb(0xffffff).opacity(0.62)),
                    ThemeAtmosphere::Totoro => Hsla::from(rgb(0xfff4c2).opacity(0.62)),
                    ThemeAtmosphere::Spirited => Hsla::from(rgb(0xffe6d4).opacity(0.64)),
                    ThemeAtmosphere::Howl => Hsla::from(rgb(0xf6f0ff).opacity(0.64)),
                    ThemeAtmosphere::Kiki => Hsla::from(rgb(0xfff1cf).opacity(0.64)),
                    ThemeAtmosphere::Lantern => Hsla::from(rgb(0xffe3a8).opacity(0.52)),
                    ThemeAtmosphere::Prism | ThemeAtmosphere::Classic => {
                        Hsla::from(rgb(0xfde68a).opacity(0.40))
                    }
                }
            };

            // Soft outer optical bloom
            paint_glow_orb(window, center, core_r * 0.78, 42.0, primary.opacity(0.36));

            // Concentric guide rings
            for (scale, alpha) in [(1.0_f32, 0.20_f32), (0.74, 0.28)] {
                let r = max_r * scale;
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(ox + w * 0.5 - r), px(oy + h * 0.5 - r)),
                        size(px(r * 2.0), px(r * 2.0)),
                    ),
                    corner_radii: Corners::all(px(r)),
                    background: transparent_black().into(),
                    border_widths: Edges::all(px(1.2)),
                    border_color: primary.opacity(alpha),
                    border_style: BorderStyle::Solid,
                });
            }

            // Breathing orb core
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(ox + w * 0.5 - core_r), px(oy + h * 0.5 - core_r)),
                    size(px(core_r * 2.0), px(core_r * 2.0)),
                ),
                corner_radii: Corners::all(px(core_r)),
                background: linear_gradient(
                    135.0,
                    linear_color_stop(secondary, 0.0),
                    linear_color_stop(primary, 1.0),
                ),
                border_widths: Edges::all(px(1.5)),
                border_color: Hsla::from(rgb(0xffffff).opacity(0.55)),
                border_style: BorderStyle::Solid,
            });
        },
    )
    .size(px(108.))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioLampColor {
    Amber,
    Ember,
    Starry,
    Rose,
}

impl RadioLampColor {
    pub const ALL: [Self; 4] = [Self::Amber, Self::Ember, Self::Starry, Self::Rose];

    pub fn from_id(id: &str) -> Self {
        match id {
            "ember" => Self::Ember,
            "starry" => Self::Starry,
            "rose" => Self::Rose,
            _ => Self::Amber,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Amber => "amber",
            Self::Ember => "ember",
            Self::Starry => "starry",
            Self::Rose => "rose",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Amber => "琥珀台灯",
            Self::Ember => "壁炉余烬",
            Self::Starry => "星夜深海",
            Self::Rose => "樱粉微光",
        }
    }

    /// Returns `(primary_glow, inner_core, outer_aura, night_stage_bg)` hex RGB colors.
    pub fn palette(self) -> (u32, u32, u32, u32) {
        match self {
            Self::Amber => (0xf4a261, 0xfff1d0, 0xd97745, 0x110e12),
            Self::Ember => (0xe06d53, 0xffd29d, 0xa83e2d, 0x130d10),
            Self::Starry => (0x5a96bd, 0xd8f3ff, 0x2b4c7e, 0x0b1019),
            Self::Rose => (0xe5989b, 0xffe5ec, 0x9c6684, 0x130f16),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioLampMode {
    Audio,
    Breathe478,
    Steady,
}

impl RadioLampMode {
    pub const ALL: [Self; 3] = [Self::Audio, Self::Breathe478, Self::Steady];

    pub fn from_id(id: &str) -> Self {
        match id {
            "breathe_478" => Self::Breathe478,
            "steady" => Self::Steady,
            _ => Self::Audio,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Breathe478 => "breathe_478",
            Self::Steady => "steady",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Audio => "声波轻漾",
            Self::Breathe478 => "4-7-8 助眠呼吸",
            Self::Steady => "恒定暖灯",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Audio => "随电台人声与旋律温柔起伏",
            Self::Breathe478 => "吸气 4s · 屏息 7s · 呼气 8s",
            Self::Steady => "安静常亮的床头微光",
        }
    }
}

fn radio_breath_factor(
    reduced_motion: bool,
    phase: f32,
    lamp_mode: RadioLampMode,
    level: f32,
    breathe_elapsed: f32,
) -> f32 {
    if reduced_motion {
        0.78 + level * 0.06
    } else {
        match lamp_mode {
            RadioLampMode::Audio => {
                let natural = 0.72 + 0.11 * (phase * 0.68).sin();
                (natural + level * 0.22).clamp(0.54, 1.08)
            }
            RadioLampMode::Breathe478 => {
                let bp = breathing_phase(breathe_elapsed);
                (bp.radius_factor + level * 0.05).clamp(0.42, 1.08)
            }
            RadioLampMode::Steady => 0.82 + 0.025 * (phase * 0.42).sin(),
        }
    }
}

/// Full-viewport GPU breathing ambient room atmosphere behind the Glow FM immersive stage.
/// Paints volumetric multi-zone warm halos synced to audio/4-7-8 breath cadence, drifting firefly
/// bokeh dust across the room, and soft cinema vignette framing.
#[allow(clippy::too_many_arguments)]
pub fn radio_ambient_room_canvas(
    snap: MotionSnapshot,
    lamp_color: RadioLampColor,
    lamp_mode: RadioLampMode,
    audio_level: f32,
    breathe_elapsed: f32,
    playing: bool,
    dim_factor: f32,
    pure_glow: bool,
) -> impl IntoElement {
    let (primary_hex, core_hex, outer_hex, stage_bg_hex) = lamp_color.palette();
    let dim = dim_factor.clamp(0.08, 1.0);
    let level = if playing {
        audio_level.clamp(0.0, 1.0)
    } else {
        0.0
    };

    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let w = f32::from(bounds.size.width).max(1.0);
            let h = f32::from(bounds.size.height).max(1.0);
            let p = if snap.reduced_motion { 0.0 } else { snap.phase };
            let breath =
                radio_breath_factor(snap.reduced_motion, p, lamp_mode, level, breathe_elapsed);

            let primary = Hsla::from(rgb(primary_hex));
            let core = Hsla::from(rgb(core_hex));
            let outer = Hsla::from(rgb(outer_hex));
            let stage_bg = Hsla::from(rgb(stage_bg_hex));

            // 1. Primary volumetric breathing lamp wash behind hero turntable / orb
            let main_x = if pure_glow {
                ox + w * 0.50
            } else {
                ox + w * 0.30
            };
            let main_y = if pure_glow {
                oy + h * 0.48
            } else {
                oy + h * 0.45
            };
            let drift_x = if snap.reduced_motion {
                0.0
            } else {
                (p * 0.24).sin() * 14.0
            };
            let drift_y = if snap.reduced_motion {
                0.0
            } else {
                (p * 0.19).cos() * 10.0
            };
            let base_r = (w.min(h) * if pure_glow { 0.30 } else { 0.28 }).max(110.0) * breath;

            paint_glow_orb(
                window,
                point(px(main_x + drift_x), px(main_y + drift_y)),
                base_r * 0.96,
                210.0,
                outer.opacity((0.24 + level * 0.14) * dim),
            );
            paint_glow_orb(
                window,
                point(px(main_x - drift_x * 0.4), px(main_y - drift_y * 0.4)),
                base_r * 0.62,
                138.0,
                primary.opacity((0.22 + level * 0.12) * dim),
            );

            // 2. Counterweight atmospheric blooms across opposite corners for cinema depth
            if !pure_glow {
                paint_glow_orb(
                    window,
                    point(
                        px(ox + w * 0.80 - drift_x * 0.8),
                        px(oy + h * 0.25 + drift_y),
                    ),
                    (w.min(h) * 0.18).max(70.0) * (0.85 + 0.15 * breath),
                    175.0,
                    outer.opacity((0.15 + level * 0.08) * dim),
                );
                paint_glow_orb(
                    window,
                    point(px(ox + w * 0.55), px(oy + h * 0.86)),
                    (w.min(h) * 0.16).max(60.0),
                    150.0,
                    primary.opacity((0.12 + level * 0.07) * dim),
                );
            }

            // 3. Soft drifting night fireflies & warm starlight bokeh across the room
            if !snap.reduced_motion {
                let mote_count = if pure_glow { 26usize } else { 22usize };
                for i in 0..mote_count {
                    let seed = i as f32 * 1.618_034;
                    let fx = ((seed * 0.73 + p * 0.008 * (1.0 + (i % 3) as f32 * 0.35))
                        .rem_euclid(1.0))
                        * w
                        + ox;
                    let fy_base = ((seed * 1.37).rem_euclid(1.0)) * h + oy;
                    let fy =
                        fy_base + (p * (0.28 + (i % 4) as f32 * 0.07) + seed * 4.0).sin() * 14.0;
                    let twinkle = 0.20 + 0.80 * ((p * 0.95 + seed * 3.1).sin() * 0.5 + 0.5);
                    let r = 1.2 + (i % 4) as f32 * 0.55;
                    let col = if i % 3 == 0 {
                        core.opacity(0.36 * twinkle * dim)
                    } else {
                        primary.opacity(0.28 * twinkle * dim)
                    };
                    if i % 5 == 0 {
                        paint_glow_orb(
                            window,
                            point(px(fx), px(fy)),
                            r * 1.6,
                            12.0,
                            col.opacity(0.45),
                        );
                    }
                    window.paint_quad(PaintQuad {
                        bounds: Bounds::new(
                            point(px(fx - r), px(fy - r)),
                            size(px(r * 2.0), px(r * 2.0)),
                        ),
                        corner_radii: Corners::all(px(r)),
                        background: col.into(),
                        border_widths: Edges::default(),
                        border_color: transparent_black(),
                        border_style: BorderStyle::default(),
                    });
                }
            }

            // 4. Subtle top & bottom gradient scrim so controls float cleanly over the ambient glow
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(point(px(ox), px(oy)), size(px(w), px(74.0))),
                corner_radii: Corners::default(),
                background: linear_gradient(
                    180.0,
                    linear_color_stop(stage_bg.opacity(0.76), 0.0),
                    linear_color_stop(stage_bg.opacity(0.0), 1.0),
                ),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(ox), px(oy + (h - 92.0).max(0.0))),
                    size(px(w), px(92.0)),
                ),
                corner_radii: Corners::default(),
                background: linear_gradient(
                    180.0,
                    linear_color_stop(stage_bg.opacity(0.0), 0.0),
                    linear_color_stop(stage_bg.opacity(0.84), 1.0),
                ),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });
        },
    )
    .absolute()
    .inset_0()
    .size_full()
}

/// Paints a smooth curved wedge fan between `r_inner` and `r_outer` spanning `start_angle..end_angle`.
#[allow(clippy::too_many_arguments)]
fn paint_radial_wedge(
    window: &mut Window,
    cx: f32,
    cy: f32,
    r_inner: f32,
    r_outer: f32,
    start_angle: f32,
    end_angle: f32,
    steps: usize,
    color: Hsla,
) {
    if color.a <= 0.002 || r_outer <= r_inner || steps < 2 {
        return;
    }
    let mut pb = PathBuilder::fill();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let a = start_angle + (end_angle - start_angle) * t;
        let pt = point(px(cx + a.cos() * r_outer), px(cy + a.sin() * r_outer));
        if i == 0 {
            pb.move_to(pt);
        } else {
            pb.line_to(pt);
        }
    }
    for i in (0..=steps).rev() {
        let t = i as f32 / steps as f32;
        let a = start_angle + (end_angle - start_angle) * t;
        pb.line_to(point(
            px(cx + a.cos() * r_inner),
            px(cy + a.sin() * r_inner),
        ));
    }
    pb.close();
    if let Ok(path) = pb.build() {
        window.paint_path(path, color);
    }
}

/// Paints a smooth circular arc stroke around `(cx, cy)` with radius `r`.
#[allow(clippy::too_many_arguments)]
fn paint_arc_stroke(
    window: &mut Window,
    cx: f32,
    cy: f32,
    r: f32,
    start_angle: f32,
    end_angle: f32,
    width: f32,
    steps: usize,
    color: Hsla,
) {
    if color.a <= 0.002 || r <= 1.0 || width <= 0.1 || steps < 2 {
        return;
    }
    let mut pb = PathBuilder::stroke(px(width));
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let a = start_angle + (end_angle - start_angle) * t;
        let pt = point(px(cx + a.cos() * r), px(cy + a.sin() * r));
        if i == 0 {
            pb.move_to(pt);
        } else {
            pb.line_to(pt);
        }
    }
    if let Ok(path) = pb.build() {
        window.paint_path(path, color);
    }
}

/// Underlay GPU canvas for the Glow FM spinning vinyl turntable:
/// breathing ambient aura, 64-bar radial audio spectrum corona, frosted glass rim, matte obsidian
/// platter, micro-grooves, rotating anisotropic specular reflections, and strobe rim dots.
#[allow(clippy::too_many_arguments)]
pub fn radio_vinyl_underlay_canvas(
    snap: MotionSnapshot,
    lamp_color: RadioLampColor,
    lamp_mode: RadioLampMode,
    audio_level: f32,
    breathe_elapsed: f32,
    vinyl_angle: f32,
    playing: bool,
    dragging_dial: bool,
    dim_factor: f32,
    stage_size: f32,
) -> impl IntoElement {
    let (primary_hex, core_hex, outer_hex, _) = lamp_color.palette();
    let dim = dim_factor.clamp(0.08, 1.0);
    let level = if playing {
        audio_level.clamp(0.0, 1.0)
    } else {
        0.0
    };

    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let w = f32::from(bounds.size.width).max(1.0);
            let h = f32::from(bounds.size.height).max(1.0);
            let cx = ox + w * 0.5;
            let cy = oy + h * 0.5;
            let center = point(px(cx), px(cy));
            let disc_r = (w.min(h) * 0.375).max(88.0);
            let label_r = disc_r * 0.47;
            let p = if snap.reduced_motion { 0.0 } else { snap.phase };
            let spin = if snap.reduced_motion {
                0.0
            } else {
                vinyl_angle
            };
            let breath =
                radio_breath_factor(snap.reduced_motion, p, lamp_mode, level, breathe_elapsed);

            let primary = Hsla::from(rgb(primary_hex));
            let core = Hsla::from(rgb(core_hex));
            let outer = Hsla::from(rgb(outer_hex));

            // 1. Volumetric breathing lamp aura glowing right behind the vinyl platter
            paint_glow_orb(
                window,
                center,
                disc_r * (0.90 + 0.16 * breath),
                115.0,
                outer.opacity((0.30 + level * 0.18) * dim),
            );
            paint_glow_orb(
                window,
                center,
                disc_r * (0.74 + 0.12 * breath),
                66.0,
                primary.opacity((0.36 + level * 0.22) * dim),
            );

            // 2. Concentric soft breathing / soundwave ripples
            for (idx, (scale, base_alpha)) in
                [(1.25_f32, 0.10_f32), (1.13, 0.17)].into_iter().enumerate()
            {
                let wave = if snap.reduced_motion {
                    0.0
                } else {
                    (p * 0.80 + idx as f32 * 1.5).sin() * 3.0
                        + (breath - 0.78) * 18.0
                        + level * (8.0 + idx as f32 * 3.0)
                };
                let r = (disc_r * scale + wave).max(disc_r + 10.0);
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(cx - r), px(cy - r)),
                        size(px(r * 2.0), px(r * 2.0)),
                    ),
                    corner_radii: Corners::all(px(r)),
                    background: transparent_black().into(),
                    border_widths: Edges::all(px(1.0)),
                    border_color: primary.opacity((base_alpha + level * 0.14) * dim),
                    border_style: BorderStyle::Solid,
                });
            }

            // 3. 64-bar radial audio spectrum corona orbiting around the vinyl platter
            let bars = 64usize;
            let bar_r_in = disc_r + 8.5;
            for i in 0..bars {
                let frac = i as f32 / bars as f32;
                let a = frac * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                let h1 = (a * 3.0 + p * 1.9).sin() * 0.5 + 0.5;
                let h2 = (a * 7.0 - p * 2.7 + 1.1).sin() * 0.5 + 0.5;
                let h3 = (a * 11.0 + p * 3.4).sin() * 0.5 + 0.5;
                let energy = if dragging_dial {
                    0.28 + 0.52 * h3
                } else if playing {
                    let active_lvl = (level * 1.35 + 0.22).clamp(0.22, 1.0);
                    (0.18 + 0.82 * (h1 * 0.45 + h2 * 0.35 + h3 * 0.20)) * active_lvl
                } else {
                    0.08 + 0.08 * h1
                };
                let bar_len = if snap.reduced_motion {
                    4.0 + level * 8.0
                } else {
                    3.0 + energy * 24.0
                };
                let r_out = bar_r_in + bar_len;
                let mut pb = PathBuilder::stroke(px(2.1));
                pb.move_to(point(
                    px(cx + a.cos() * bar_r_in),
                    px(cy + a.sin() * bar_r_in),
                ));
                pb.line_to(point(px(cx + a.cos() * r_out), px(cy + a.sin() * r_out)));
                if let Ok(path) = pb.build() {
                    let bar_col = if i % 4 == 0 {
                        core.opacity((0.22 + energy * 0.62) * dim)
                    } else {
                        primary.opacity((0.18 + energy * 0.56) * dim)
                    };
                    window.paint_path(path, bar_col);
                }
            }

            // 4. Frosted smoked-glass platter bezel ring (classic NetEase Cloud Music ring)
            let bezel_r = disc_r + 5.0;
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(cx - bezel_r), px(cy - bezel_r)),
                    size(px(bezel_r * 2.0), px(bezel_r * 2.0)),
                ),
                corner_radii: Corners::all(px(bezel_r)),
                background: Hsla::from(rgb(0xffffff).opacity(0.055 * dim)).into(),
                border_widths: Edges::all(px(1.2)),
                border_color: core.opacity(0.18 * dim),
                border_style: BorderStyle::Solid,
            });

            // 5. Matte black vinyl platter disc
            paint_glow_orb(
                window,
                point(px(cx), px(cy + 6.0)),
                disc_r * 0.94,
                32.0,
                Hsla::from(rgb(0x000000).opacity(0.55)),
            );
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(cx - disc_r), px(cy - disc_r)),
                    size(px(disc_r * 2.0), px(disc_r * 2.0)),
                ),
                corner_radii: Corners::all(px(disc_r)),
                background: linear_gradient(
                    135.0,
                    linear_color_stop(Hsla::from(rgb(0x1f222d)), 0.0),
                    linear_color_stop(Hsla::from(rgb(0x0a0b10)), 1.0),
                ),
                border_widths: Edges::all(px(1.4)),
                border_color: Hsla::from(rgb(0xffffff).opacity(0.14 * dim)),
                border_style: BorderStyle::Solid,
            });

            // 6. Rotating anisotropic specular light-sheen fans across the black vinyl grooves
            let groove_in = label_r + 6.0;
            let groove_out = disc_r - 3.5;
            for offset in [0.0_f32, std::f32::consts::PI] {
                paint_radial_wedge(
                    window,
                    cx,
                    cy,
                    groove_in,
                    groove_out,
                    spin + offset + 0.42,
                    spin + offset + 1.08,
                    12,
                    Hsla::from(rgb(0xffffff).opacity(0.075 * dim)),
                );
                paint_radial_wedge(
                    window,
                    cx,
                    cy,
                    groove_in,
                    groove_out,
                    spin + offset + 0.62,
                    spin + offset + 0.88,
                    8,
                    core.opacity(0.070 * dim),
                );
                paint_radial_wedge(
                    window,
                    cx,
                    cy,
                    groove_in,
                    groove_out,
                    spin + offset - 0.88,
                    spin + offset - 0.44,
                    10,
                    primary.opacity(0.095 * dim),
                );
            }

            // 7. 14 Precision concentric vinyl micro-grooves
            let groove_count = 14usize;
            for i in 0..groove_count {
                let frac = (i as f32 + 0.5) / groove_count as f32;
                let r = groove_in + frac * (groove_out - groove_in);
                let is_track_break = i == 4 || i == 9;
                let alpha = if is_track_break { 0.14 } else { 0.055 };
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(cx - r), px(cy - r)),
                        size(px(r * 2.0), px(r * 2.0)),
                    ),
                    corner_radii: Corners::all(px(r)),
                    background: transparent_black().into(),
                    border_widths: Edges::all(px(if is_track_break { 1.1 } else { 0.7 })),
                    border_color: Hsla::from(rgb(0xffffff).opacity(alpha * dim)),
                    border_style: BorderStyle::Solid,
                });
            }

            // 8. Rotating luminous groove glint arcs & Technics-style strobe rim calibration dots
            for (idx, r_frac) in [0.28_f32, 0.58, 0.82].into_iter().enumerate() {
                let r = groove_in + r_frac * (groove_out - groove_in);
                let a0 = spin + idx as f32 * 2.09;
                paint_arc_stroke(
                    window,
                    cx,
                    cy,
                    r,
                    a0,
                    a0 + 0.78,
                    1.2,
                    14,
                    core.opacity(0.28 * dim),
                );
            }
            let strobe_r = disc_r - 3.2;
            for i in 0..24usize {
                let a = spin + (i as f32 / 24.0) * std::f32::consts::TAU;
                let sx = cx + a.cos() * strobe_r;
                let sy = cy + a.sin() * strobe_r;
                let dot_r = if i % 6 == 0 { 1.7 } else { 1.05 };
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(sx - dot_r), px(sy - dot_r)),
                        size(px(dot_r * 2.0), px(dot_r * 2.0)),
                    ),
                    corner_radii: Corners::all(px(dot_r)),
                    background: if i % 6 == 0 {
                        core.opacity(0.62 * dim)
                    } else {
                        primary.opacity(0.32 * dim)
                    }
                    .into(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }
        },
    )
    .size(px(stage_size))
}

/// Overlay GPU canvas painted directly on top of the circular center album artwork:
/// rotating orbital label ring, spinning constellation/starlight ticks, brushed brass center
/// spindle cap, and an animated NetEase Cloud Music style metallic stylus tonearm!
#[allow(clippy::too_many_arguments)]
pub fn radio_vinyl_overlay_canvas(
    snap: MotionSnapshot,
    lamp_color: RadioLampColor,
    audio_level: f32,
    vinyl_angle: f32,
    tonearm_pos: f32,
    playing: bool,
    dim_factor: f32,
    stage_size: f32,
) -> impl IntoElement {
    let (primary_hex, core_hex, _, _) = lamp_color.palette();
    let dim = dim_factor.clamp(0.08, 1.0);
    let level = if playing {
        audio_level.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let arm_t = tonearm_pos.clamp(0.0, 1.0);

    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let w = f32::from(bounds.size.width).max(1.0);
            let h = f32::from(bounds.size.height).max(1.0);
            let cx = ox + w * 0.5;
            let cy = oy + h * 0.5;
            let disc_r = (w.min(h) * 0.375).max(88.0);
            let label_r = disc_r * 0.47;
            let spin = if snap.reduced_motion {
                0.0
            } else {
                vinyl_angle
            };

            let primary = Hsla::from(rgb(primary_hex));
            let core = Hsla::from(rgb(core_hex));

            // 1. Rotating glass glint wedge over circular album cover so the medallion visibly spins
            paint_radial_wedge(
                window,
                cx,
                cy,
                16.0,
                label_r - 1.5,
                spin + 0.30,
                spin + 1.15,
                10,
                Hsla::from(rgb(0xffffff).opacity(0.11 * dim)),
            );
            paint_radial_wedge(
                window,
                cx,
                cy,
                16.0,
                label_r - 1.5,
                spin + std::f32::consts::PI + 0.30,
                spin + std::f32::consts::PI + 1.15,
                10,
                primary.opacity(0.09 * dim),
            );

            // 2. Engraved metallic label collar & rotating 24-tick orbital compass ring
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(cx - label_r), px(cy - label_r)),
                    size(px(label_r * 2.0), px(label_r * 2.0)),
                ),
                corner_radii: Corners::all(px(label_r)),
                background: transparent_black().into(),
                border_widths: Edges::all(px(2.0)),
                border_color: core.opacity(0.55 * dim),
                border_style: BorderStyle::Solid,
            });
            let inner_ring_r = label_r - 7.5;
            paint_arc_stroke(
                window,
                cx,
                cy,
                inner_ring_r,
                spin + 0.2,
                spin + 2.6,
                1.2,
                20,
                core.opacity(0.68 * dim),
            );
            paint_arc_stroke(
                window,
                cx,
                cy,
                inner_ring_r,
                spin + std::f32::consts::PI + 0.2,
                spin + std::f32::consts::PI + 2.6,
                1.2,
                20,
                primary.opacity(0.62 * dim),
            );

            // Rotating orbital ticks & 4 bright cardinal star markers around the album cover
            for i in 0..24usize {
                let a = spin + (i as f32 / 24.0) * std::f32::consts::TAU;
                let is_cardinal = i % 6 == 0;
                if is_cardinal {
                    let sx = cx + a.cos() * (label_r - 7.5);
                    let sy = cy + a.sin() * (label_r - 7.5);
                    let r = 2.6;
                    window.paint_quad(PaintQuad {
                        bounds: Bounds::new(
                            point(px(sx - r), px(sy - r)),
                            size(px(r * 2.0), px(r * 2.0)),
                        ),
                        corner_radii: Corners::all(px(r)),
                        background: core.opacity(0.92 * dim).into(),
                        border_widths: Edges::default(),
                        border_color: transparent_black(),
                        border_style: BorderStyle::default(),
                    });
                } else {
                    let r0 = label_r - 5.2;
                    let r1 = label_r - 2.0;
                    let mut pb = PathBuilder::stroke(px(1.0));
                    pb.move_to(point(px(cx + a.cos() * r0), px(cy + a.sin() * r0)));
                    pb.line_to(point(px(cx + a.cos() * r1), px(cy + a.sin() * r1)));
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, core.opacity(0.38 * dim));
                    }
                }
            }

            // 3. Brushed obsidian & warm brass center spindle cap
            let collar_r = 16.0_f32;
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(cx - collar_r), px(cy - collar_r)),
                    size(px(collar_r * 2.0), px(collar_r * 2.0)),
                ),
                corner_radii: Corners::all(px(collar_r)),
                background: Hsla::from(rgb(0x0c0d13).opacity(0.88)).into(),
                border_widths: Edges::all(px(1.4)),
                border_color: core.opacity(0.45 * dim),
                border_style: BorderStyle::Solid,
            });
            let pin_r = 5.6_f32;
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(cx - pin_r), px(cy - pin_r)),
                    size(px(pin_r * 2.0), px(pin_r * 2.0)),
                ),
                corner_radii: Corners::all(px(pin_r)),
                background: linear_gradient(
                    135.0,
                    linear_color_stop(core.opacity(0.95), 0.0),
                    linear_color_stop(primary.opacity(0.90), 1.0),
                ),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });

            // 4. Animated brushed-champagne metallic stylus tonearm (top-right gimbal mount)
            let pivot_x = cx + disc_r * 0.92;
            let pivot_y = cy - disc_r * 0.92;
            // Smooth interpolation from parked (-14 deg, outside vinyl) to playing (+12 deg, mid-grooves)
            let theta = (-14.0_f32 + arm_t * 26.0_f32).to_radians();
            let arm_len = disc_r * 0.78;
            let head_len = disc_r * 0.195;
            let back_len = disc_r * 0.165;

            let ux = -theta.sin();
            let uy = theta.cos();
            let back_x = pivot_x - ux * back_len;
            let back_y = pivot_y - uy * back_len;
            let elbow_x = pivot_x + ux * arm_len;
            let elbow_y = pivot_y + uy * arm_len;

            let head_theta = theta + 22.0_f32.to_radians();
            let hx = -head_theta.sin();
            let hy = head_theta.cos();
            let tip_x = elbow_x + hx * head_len;
            let tip_y = elbow_y + hy * head_len;

            // Subtle drop shadow underneath the tonearm
            {
                let mut shadow_pb = PathBuilder::stroke(px(5.2));
                shadow_pb.move_to(point(px(back_x + 2.0), px(back_y + 4.0)));
                shadow_pb.line_to(point(px(elbow_x + 2.0), px(elbow_y + 4.0)));
                shadow_pb.line_to(point(px(tip_x + 2.0), px(tip_y + 4.0)));
                if let Ok(path) = shadow_pb.build() {
                    window.paint_path(path, Hsla::from(rgb(0x000000).opacity(0.42)));
                }
            }

            // Counterweight cylinder behind gimbal pivot
            {
                let mut cw_pb = PathBuilder::stroke(px(8.5));
                cw_pb.move_to(point(
                    px(pivot_x - ux * (back_len * 0.45)),
                    px(pivot_y - uy * (back_len * 0.45)),
                ));
                cw_pb.line_to(point(px(back_x), px(back_y)));
                if let Ok(path) = cw_pb.build() {
                    window.paint_path(path, Hsla::from(rgb(0x2a2d3a).opacity(0.95)));
                }
            }

            // Brushed metallic tubular arm shaft
            {
                let mut arm_pb = PathBuilder::stroke(px(3.6));
                arm_pb.move_to(point(px(back_x), px(back_y)));
                arm_pb.line_to(point(px(elbow_x), px(elbow_y)));
                arm_pb.line_to(point(px(tip_x), px(tip_y)));
                if let Ok(path) = arm_pb.build() {
                    window.paint_path(
                        path,
                        linear_gradient(
                            135.0,
                            linear_color_stop(core.opacity(0.92 * dim), 0.0),
                            linear_color_stop(Hsla::from(rgb(0x9b958e).opacity(0.88 * dim)), 1.0),
                        ),
                    );
                }
            }

            // Matte obsidian headshell cartridge at the stylus end
            {
                let shell_start_x = elbow_x + hx * (head_len * 0.25);
                let shell_start_y = elbow_y + hy * (head_len * 0.25);
                let mut head_pb = PathBuilder::stroke(px(8.0));
                head_pb.move_to(point(px(shell_start_x), px(shell_start_y)));
                head_pb.line_to(point(px(tip_x), px(tip_y)));
                if let Ok(path) = head_pb.build() {
                    window.paint_path(path, Hsla::from(rgb(0x1b1d26).opacity(0.98)));
                }
                let mut accent_pb = PathBuilder::stroke(px(2.0));
                accent_pb.move_to(point(px(shell_start_x), px(shell_start_y)));
                accent_pb.line_to(point(px(tip_x), px(tip_y)));
                if let Ok(path) = accent_pb.build() {
                    window.paint_path(path, core.opacity(0.75 * dim));
                }
            }

            // Tiny luminous ON AIR stylus tip jewel when engaged
            if arm_t > 0.65 && playing {
                paint_glow_orb(
                    window,
                    point(px(tip_x), px(tip_y)),
                    3.2,
                    12.0,
                    primary.opacity((0.62 + level * 0.30) * dim),
                );
                let r = 2.4;
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(tip_x - r), px(tip_y - r)),
                        size(px(r * 2.0), px(r * 2.0)),
                    ),
                    corner_radii: Corners::all(px(r)),
                    background: core.opacity(0.95 * dim).into(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }

            // Brushed gimbal pivot assembly on top
            for (r, bg, border) in [
                (
                    17.5_f32,
                    Hsla::from(rgb(0x1d202a).opacity(0.96)),
                    core.opacity(0.35 * dim),
                ),
                (
                    11.0_f32,
                    Hsla::from(rgb(0x2f3342).opacity(0.96)),
                    core.opacity(0.50 * dim),
                ),
                (4.6_f32, core.opacity(0.92 * dim), transparent_black()),
            ] {
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(pivot_x - r), px(pivot_y - r)),
                        size(px(r * 2.0), px(r * 2.0)),
                    ),
                    corner_radii: Corners::all(px(r)),
                    background: bg.into(),
                    border_widths: if border.a > 0.01 {
                        Edges::all(px(1.2))
                    } else {
                        Edges::default()
                    },
                    border_color: border,
                    border_style: BorderStyle::Solid,
                });
            }
        },
    )
    .absolute()
    .inset_0()
    .size(px(stage_size))
}

/// Multi-layer GPU healing breathing lamp centerpiece for Glow FM orb mode & Pure-Glow Zen mode.
/// Supports 4 warm color temperatures, 3 breathing cadences (audio-reactive / 4-7-8 / steady),
/// live RMS resonance ripples, drifting night fireflies, and sleep-timer dimming.
#[allow(clippy::too_many_arguments)]
pub fn radio_breathing_lamp_canvas(
    snap: MotionSnapshot,
    lamp_color: RadioLampColor,
    lamp_mode: RadioLampMode,
    audio_level: f32,
    breathe_elapsed: f32,
    playing: bool,
    dim_factor: f32,
    stage_size: f32,
) -> impl IntoElement {
    let (primary_hex, core_hex, outer_hex, _) = lamp_color.palette();
    let dim = dim_factor.clamp(0.08, 1.0);
    let level = if playing {
        audio_level.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let pure_glow = stage_size >= 340.0;

    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let w = f32::from(bounds.size.width).max(1.0);
            let h = f32::from(bounds.size.height).max(1.0);
            let cx = ox + w * 0.5;
            let cy = oy + h * 0.5;
            let center = point(px(cx), px(cy));
            let max_r = (w.min(h) * 0.34).max(40.0);
            let p = if snap.reduced_motion { 0.0 } else { snap.phase };

            let breath_factor =
                radio_breath_factor(snap.reduced_motion, p, lamp_mode, level, breathe_elapsed);

            let core_r = max_r * breath_factor;
            let primary = Hsla::from(rgb(primary_hex));
            let core = Hsla::from(rgb(core_hex));
            let outer = Hsla::from(rgb(outer_hex));

            // 1. Wide ambient room-warmth bloom
            paint_glow_orb(
                window,
                center,
                core_r * 1.12,
                if pure_glow { 148.0 } else { 116.0 },
                outer.opacity((0.28 + level * 0.16) * dim),
            );
            // 2. Mid soothing breathing glow
            paint_glow_orb(
                window,
                center,
                core_r * 0.84,
                if pure_glow { 88.0 } else { 70.0 },
                primary.opacity((0.42 + level * 0.18) * dim),
            );

            // 3. Delicate concentric sound/breath ripples
            for (idx, (scale, base_alpha)) in [(1.32_f32, 0.10_f32), (1.10, 0.18), (0.88, 0.26)]
                .into_iter()
                .enumerate()
            {
                let ripple_wave = if snap.reduced_motion {
                    0.0
                } else {
                    (p * 0.85 + idx as f32 * 1.4).sin() * 3.8 + level * (7.0 + idx as f32 * 3.5)
                };
                let r = (max_r * scale + ripple_wave).max(18.0);
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(cx - r), px(cy - r)),
                        size(px(r * 2.0), px(r * 2.0)),
                    ),
                    corner_radii: Corners::all(px(r)),
                    background: transparent_black().into(),
                    border_widths: Edges::all(px(if idx == 2 { 1.3 } else { 1.0 })),
                    border_color: primary.opacity((base_alpha + level * 0.14) * dim),
                    border_style: BorderStyle::Solid,
                });
            }

            // 4. Translucent glass-lantern breathing core sphere
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(cx - core_r), px(cy - core_r)),
                    size(px(core_r * 2.0), px(core_r * 2.0)),
                ),
                corner_radii: Corners::all(px(core_r)),
                background: linear_gradient(
                    138.0,
                    linear_color_stop(core.opacity(0.95 * dim), 0.0),
                    linear_color_stop(primary.opacity(0.88 * dim), 1.0),
                ),
                border_widths: Edges::all(px(1.5)),
                border_color: core.opacity(0.46 * dim),
                border_style: BorderStyle::Solid,
            });

            // 5. Inner warm vacuum-tube heart highlight
            let heart_r = (core_r * 0.44).max(12.0);
            let heart_center = point(px(cx - core_r * 0.14), px(cy - core_r * 0.16));
            paint_glow_orb(
                window,
                heart_center,
                heart_r * 0.72,
                28.0,
                core.opacity(0.58 * dim),
            );
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(
                        px(f32::from(heart_center.x) - heart_r * 0.52),
                        px(f32::from(heart_center.y) - heart_r * 0.52),
                    ),
                    size(px(heart_r * 1.04), px(heart_r * 1.04)),
                ),
                corner_radii: Corners::all(px(heart_r * 0.52)),
                background: Hsla::from(rgb(0xffffff).opacity(0.30 * dim)).into(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });

            // 6. Slow drifting warm firefly motes orbiting the lamp
            if !snap.reduced_motion {
                let mote_count = if pure_glow { 16usize } else { 12usize };
                for i in 0..mote_count {
                    let seed = i as f32 * 1.618_034;
                    let angle = p * (0.14 + (i % 3) as f32 * 0.04) + seed * 3.1;
                    let orbit =
                        max_r * (0.98 + 0.34 * ((seed * 2.1).sin() * 0.5 + 0.5)) + level * 12.0;
                    let mx = cx + angle.cos() * orbit;
                    let my = cy + angle.sin() * orbit * 0.86 - ((p * 0.3 + seed).sin() * 6.0);
                    let twinkle = 0.28 + 0.72 * ((p * 1.2 + seed * 2.5).sin() * 0.5 + 0.5);
                    let r = 1.5 + (i % 3) as f32 * 0.55;
                    let col = if i % 2 == 0 {
                        core.opacity(0.65 * twinkle * dim)
                    } else {
                        primary.opacity(0.55 * twinkle * dim)
                    };
                    window.paint_quad(PaintQuad {
                        bounds: Bounds::new(
                            point(px(mx - r), px(my - r)),
                            size(px(r * 2.0), px(r * 2.0)),
                        ),
                        corner_radii: Corners::all(px(r)),
                        background: col.into(),
                        border_widths: Edges::default(),
                        border_color: transparent_black(),
                        border_style: BorderStyle::default(),
                    });
                }
            }
        },
    )
    .size(px(stage_size))
}

/// Retro-modern draggable 88.0 – 108.0 MHz analog FM tuner glass scale.
/// Records exact layout bounds for smooth continuous mouse dragging, illuminates ticks around
/// the active needle, highlights magnetic target station beacons, and enlarges the tactile glass
/// drag handle while scrubbing frequencies.
#[allow(clippy::too_many_arguments)]
pub fn fm_tuner_dial_canvas(
    snap: MotionSnapshot,
    display_mhz: f32,
    nearest_station_mhz: Option<f32>,
    station_markers: Vec<(f32, bool)>,
    lamp_color: RadioLampColor,
    playing: bool,
    dragging: bool,
    bounds_slot: Arc<Mutex<Option<Bounds<Pixels>>>>,
) -> impl IntoElement {
    let (primary_hex, core_hex, _, _) = lamp_color.palette();
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            if let Ok(mut slot) = bounds_slot.lock() {
                *slot = Some(bounds);
            }
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let w = f32::from(bounds.size.width).max(1.0);
            let h = f32::from(bounds.size.height).max(1.0);
            let pad_x = 20.0_f32;
            let track_w = (w - pad_x * 2.0).max(20.0);
            let primary = Hsla::from(rgb(primary_hex));
            let core = Hsla::from(rgb(core_hex));

            // 1. Recessed smoked-glass vintage radio dial window
            window.paint_quad(PaintQuad {
                bounds,
                corner_radii: Corners::all(px(14.0)),
                background: linear_gradient(
                    180.0,
                    linear_color_stop(Hsla::from(rgb(0x090b12).opacity(0.84)), 0.0),
                    linear_color_stop(Hsla::from(rgb(0x12141e).opacity(0.76)), 1.0),
                ),
                border_widths: Edges::all(px(1.0)),
                border_color: if dragging {
                    primary.opacity(0.55)
                } else {
                    primary.opacity(0.24)
                },
                border_style: BorderStyle::Solid,
            });
            // Subtle glass top highlight strip
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(ox + 10.0), px(oy + 1.5)),
                    size(px((w - 20.0).max(4.0)), px(1.0)),
                ),
                corner_radii: Corners::all(px(0.5)),
                background: Hsla::from(rgb(0xffffff).opacity(0.11)).into(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });

            // 2. Horizontal brass-tinted tuner rail
            let rail_y = oy + h * 0.62;
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(ox + pad_x), px(rail_y)),
                    size(px(track_w), px(1.5)),
                ),
                corner_radii: Corners::all(px(0.75)),
                background: primary.opacity(0.24).into(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });

            let cur_frac = ((display_mhz - 88.0) / 20.0).clamp(0.0, 1.0);
            let nx = ox + pad_x + cur_frac * track_w;

            // Active backlit fill along the rail from 88 MHz up to the tuning needle
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(ox + pad_x), px(rail_y - 0.5)),
                    size(px((cur_frac * track_w).max(2.0)), px(2.5)),
                ),
                corner_radii: Corners::all(px(1.25)),
                background: linear_gradient(
                    90.0,
                    linear_color_stop(primary.opacity(0.15), 0.0),
                    linear_color_stop(primary.opacity(0.68), 1.0),
                ),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });

            // 3. Precision 88.0 to 108.0 MHz scale ticks (backlit warmth near needle!)
            for step in 0..=100usize {
                let frac = step as f32 / 100.0;
                let tx = ox + pad_x + frac * track_w;
                let dist_mhz = ((88.0 + frac * 20.0) - display_mhz).abs();
                let proximity = (1.0 - dist_mhz / 2.2).clamp(0.0, 1.0);
                let is_major = step % 10 == 0;
                let is_mid = step % 5 == 0;
                let tick_h = if is_major {
                    18.0 + proximity * 4.0
                } else if is_mid {
                    11.5 + proximity * 3.0
                } else {
                    6.0 + proximity * 2.5
                };
                let tick_w = if is_major { 1.6 } else { 1.0 };
                let alpha = (if is_major {
                    0.56
                } else if is_mid {
                    0.34
                } else {
                    0.16
                } + proximity * 0.36)
                    .clamp(0.0, 0.96);
                let col = if proximity > 0.25 {
                    core.opacity(alpha)
                } else {
                    core.opacity(alpha * 0.85)
                };
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(tx - tick_w * 0.5), px(rail_y - tick_h * 0.45)),
                        size(px(tick_w), px(tick_h)),
                    ),
                    corner_radii: Corners::all(px(0.6)),
                    background: col.into(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }

            // 4. Starlight station beacons along upper rail (magnetic lock halo on nearest station!)
            for &(mhz, is_fav) in &station_markers {
                let frac = ((mhz - 88.0) / 20.0).clamp(0.0, 1.0);
                let sx = ox + pad_x + frac * track_w;
                let sy = oy + 12.5;
                let is_locked = nearest_station_mhz.is_some_and(|m| (m - mhz).abs() < 0.08);
                let r = if is_locked {
                    3.8
                } else if is_fav {
                    3.0
                } else {
                    2.2
                };
                if is_locked {
                    paint_glow_orb(
                        window,
                        point(px(sx), px(sy)),
                        5.0,
                        14.0,
                        primary.opacity(0.75),
                    );
                    let ring_r = 6.5;
                    window.paint_quad(PaintQuad {
                        bounds: Bounds::new(
                            point(px(sx - ring_r), px(sy - ring_r)),
                            size(px(ring_r * 2.0), px(ring_r * 2.0)),
                        ),
                        corner_radii: Corners::all(px(ring_r)),
                        background: transparent_black().into(),
                        border_widths: Edges::all(px(1.1)),
                        border_color: core.opacity(0.75),
                        border_style: BorderStyle::Solid,
                    });
                } else if is_fav {
                    paint_glow_orb(
                        window,
                        point(px(sx), px(sy)),
                        3.0,
                        9.0,
                        primary.opacity(0.48),
                    );
                }
                window.paint_quad(PaintQuad {
                    bounds: Bounds::new(
                        point(px(sx - r), px(sy - r)),
                        size(px(r * 2.0), px(r * 2.0)),
                    ),
                    corner_radii: Corners::all(px(r)),
                    background: if is_locked || is_fav {
                        core.opacity(0.96)
                    } else {
                        primary.opacity(0.62)
                    }
                    .into(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }

            // 5. Tactile draggable luminous tuning needle & glass-brass thumb handle
            let pulse = if dragging {
                1.0
            } else if playing && !snap.reduced_motion {
                0.80 + 0.20 * (snap.phase * 1.7).sin()
            } else {
                0.74
            };
            paint_glow_orb(
                window,
                point(px(nx), px(oy + h * 0.50)),
                if dragging { 8.5 } else { 5.8 },
                if dragging { 28.0 } else { 20.0 },
                primary.opacity((if dragging { 0.78 } else { 0.55 }) * pulse),
            );
            // Vertical luminous needle beam (starts cleanly below upper starlight beacons)
            let beam_w = if dragging { 3.0 } else { 2.4 };
            let beam_top = oy + 19.5;
            let beam_h = (h - 24.5).max(8.0);
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(nx - beam_w * 0.5), px(beam_top)),
                    size(px(beam_w), px(beam_h)),
                ),
                corner_radii: Corners::all(px(beam_w * 0.5)),
                background: linear_gradient(
                    180.0,
                    linear_color_stop(core.opacity(0.98), 0.0),
                    linear_color_stop(primary.opacity(0.94), 1.0),
                ),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });
            // Center tactile glass-brass slider knob directly on the rail (signals drag affordance!)
            let knob_r = if dragging { 8.0 } else { 6.0 };
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(nx - knob_r), px(rail_y - knob_r)),
                    size(px(knob_r * 2.0), px(knob_r * 2.0)),
                ),
                corner_radii: Corners::all(px(knob_r)),
                background: core.opacity(if dragging { 0.98 } else { 0.90 }).into(),
                border_widths: Edges::all(px(1.8)),
                border_color: primary.opacity(0.95),
                border_style: BorderStyle::Solid,
            });
        },
    )
    .w_full()
    .h(px(54.))
}

/// Smooth draggable glowing volume rail canvas for the Glow FM bottom dock.
pub fn radio_volume_bar_canvas(
    volume: f32,
    lamp_color: RadioLampColor,
    dragging: bool,
    bounds_slot: Arc<Mutex<Option<Bounds<Pixels>>>>,
) -> impl IntoElement {
    let (primary_hex, core_hex, _, _) = lamp_color.palette();
    let frac = volume.clamp(0.05, 1.0);
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            if let Ok(mut slot) = bounds_slot.lock() {
                *slot = Some(bounds);
            }
            let ox = f32::from(bounds.origin.x);
            let oy = f32::from(bounds.origin.y);
            let w = f32::from(bounds.size.width).max(1.0);
            let h = f32::from(bounds.size.height).max(1.0);
            let pad = 7.0_f32;
            let track_w = (w - pad * 2.0).max(8.0);
            let cy = oy + h * 0.5;
            let primary = Hsla::from(rgb(primary_hex));
            let core = Hsla::from(rgb(core_hex));

            // Base rail
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(ox + pad), px(cy - 2.0)),
                    size(px(track_w), px(4.0)),
                ),
                corner_radii: Corners::all(px(2.0)),
                background: Hsla::from(rgb(0xffffff).opacity(0.14)).into(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });

            // Filled volume portion
            let fill_w = (track_w * frac).max(3.0);
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(point(px(ox + pad), px(cy - 2.0)), size(px(fill_w), px(4.0))),
                corner_radii: Corners::all(px(2.0)),
                background: linear_gradient(
                    90.0,
                    linear_color_stop(primary.opacity(0.65), 0.0),
                    linear_color_stop(core.opacity(0.95), 1.0),
                ),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });

            // Glowing thumb knob
            let kx = ox + pad + track_w * frac;
            let kr = if dragging { 6.2 } else { 4.8 };
            if dragging {
                paint_glow_orb(
                    window,
                    point(px(kx), px(cy)),
                    5.5,
                    14.0,
                    primary.opacity(0.65),
                );
            }
            window.paint_quad(PaintQuad {
                bounds: Bounds::new(
                    point(px(kx - kr), px(cy - kr)),
                    size(px(kr * 2.0), px(kr * 2.0)),
                ),
                corner_radii: Corners::all(px(kr)),
                background: core.opacity(0.96).into(),
                border_widths: Edges::all(px(1.2)),
                border_color: primary.opacity(0.90),
                border_style: BorderStyle::Solid,
            });
        },
    )
    .w(px(92.))
    .h(px(24.))
}

const VINYL_COVER_PX: usize = 192;

struct DecodedSquareCover {
    /// Pre-cropped 192x192 BGRA source buffer matching GPUI `RenderImage` channel order.
    bgra: Vec<u8>,
}

/// Caches decoded station cover squares and manages a single live rotated GPU `RenderImage`
/// (dropping the previous sprite atlas entry on each rotation step so VRAM stays O(1)).
pub struct VinylCoverCache {
    decoded: HashMap<&'static str, DecodedSquareCover>,
    active_scene: &'static str,
    active_angle: f32,
    active_dim: f32,
    active_image: Option<Arc<RenderImage>>,
}

impl VinylCoverCache {
    pub fn new() -> Self {
        Self {
            decoded: HashMap::new(),
            active_scene: "",
            active_angle: -10.0,
            active_dim: -1.0,
            active_image: None,
        }
    }

    fn decode_scene(scene_asset: &'static str) -> Option<DecodedSquareCover> {
        let bytes = crate::platform::Assets.load(scene_asset).ok().flatten()?;
        let dyn_img =
            image::load_from_memory_with_format(bytes.as_ref(), image::ImageFormat::Png).ok()?;
        let rgba = dyn_img.to_rgba8();
        let (sw, sh) = rgba.dimensions();
        if sw == 0 || sh == 0 {
            return None;
        }
        // Center square crop ("object-fit: cover") bilinearly sampled to VINYL_COVER_PX^2 in BGRA
        let side = sw.min(sh) as f32;
        let crop_x = (sw as f32 - side) * 0.5;
        let crop_y = (sh as f32 - side) * 0.5;
        let raw = rgba.as_raw();
        let mut bgra = vec![0u8; VINYL_COVER_PX * VINYL_COVER_PX * 4];
        let max_x = (sw - 1) as f32;
        let max_y = (sh - 1) as f32;

        for y in 0..VINYL_COVER_PX {
            let fy = (crop_y + ((y as f32 + 0.5) / VINYL_COVER_PX as f32) * side - 0.5)
                .clamp(0.0, max_y);
            let y0 = fy.floor() as usize;
            let y1 = (y0 + 1).min(sh as usize - 1);
            let ty = fy - y0 as f32;
            for x in 0..VINYL_COVER_PX {
                let fx = (crop_x + ((x as f32 + 0.5) / VINYL_COVER_PX as f32) * side - 0.5)
                    .clamp(0.0, max_x);
                let x0 = fx.floor() as usize;
                let x1 = (x0 + 1).min(sw as usize - 1);
                let tx = fx - x0 as f32;

                let idx00 = (y0 * sw as usize + x0) * 4;
                let idx10 = (y0 * sw as usize + x1) * 4;
                let idx01 = (y1 * sw as usize + x0) * 4;
                let idx11 = (y1 * sw as usize + x1) * 4;

                let sample = |ch: usize| -> u8 {
                    let c00 = raw[idx00 + ch] as f32;
                    let c10 = raw[idx10 + ch] as f32;
                    let c01 = raw[idx01 + ch] as f32;
                    let c11 = raw[idx11 + ch] as f32;
                    let top = c00 + (c10 - c00) * tx;
                    let bot = c01 + (c11 - c01) * tx;
                    (top + (bot - top) * ty).round().clamp(0.0, 255.0) as u8
                };

                let dst = (y * VINYL_COVER_PX + x) * 4;
                bgra[dst] = sample(2); // B
                bgra[dst + 1] = sample(1); // G
                bgra[dst + 2] = sample(0); // R
                bgra[dst + 3] = sample(3); // A
            }
        }
        Some(DecodedSquareCover { bgra })
    }

    fn build_rotated_image(
        src: &DecodedSquareCover,
        angle: f32,
        dim: f32,
    ) -> Option<Arc<RenderImage>> {
        let sz = VINYL_COVER_PX;
        let half = (sz as f32 - 1.0) * 0.5;
        let r_max = half - 0.5;
        let (sin_a, cos_a) = angle.sin_cos();
        let dim_scale = (0.94 * dim).clamp(0.08, 1.0);
        let max_coord = (sz - 1) as f32;
        let mut out = vec![0u8; sz * sz * 4];

        for y in 0..sz {
            let dy = y as f32 - half;
            for x in 0..sz {
                let dx = x as f32 - half;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > r_max + 1.0 {
                    continue;
                }
                let edge_alpha = ((r_max + 0.5 - dist) / 1.4).clamp(0.0, 1.0);
                if edge_alpha <= 0.002 {
                    continue;
                }
                // Inverse clockwise rotation in screen coords (+X right, +Y down)
                let sx = (half + dx * cos_a + dy * sin_a).clamp(0.0, max_coord);
                let sy = (half - dx * sin_a + dy * cos_a).clamp(0.0, max_coord);
                let x0 = sx.floor() as usize;
                let y0 = sy.floor() as usize;
                let x1 = (x0 + 1).min(sz - 1);
                let y1 = (y0 + 1).min(sz - 1);
                let tx = sx - x0 as f32;
                let ty = sy - y0 as f32;

                let i00 = (y0 * sz + x0) * 4;
                let i10 = (y0 * sz + x1) * 4;
                let i01 = (y1 * sz + x0) * 4;
                let i11 = (y1 * sz + x1) * 4;

                let dst = (y * sz + x) * 4;
                for ch in 0..3 {
                    let c00 = src.bgra[i00 + ch] as f32;
                    let c10 = src.bgra[i10 + ch] as f32;
                    let c01 = src.bgra[i01 + ch] as f32;
                    let c11 = src.bgra[i11 + ch] as f32;
                    let top = c00 + (c10 - c00) * tx;
                    let bot = c01 + (c11 - c01) * tx;
                    out[dst + ch] = ((top + (bot - top) * ty) * dim_scale)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
                out[dst + 3] = (255.0 * edge_alpha).round().clamp(0.0, 255.0) as u8;
            }
        }
        let buf = image::RgbaImage::from_raw(sz as u32, sz as u32, out)?;
        let frame = image::Frame::new(buf);
        Some(Arc::new(RenderImage::new(smallvec::smallvec![frame])))
    }

    pub fn image_for_angle(
        &mut self,
        scene_asset: &'static str,
        angle: f32,
        dim: f32,
    ) -> (Option<Arc<RenderImage>>, Option<Arc<RenderImage>>) {
        let normalized_angle = angle.rem_euclid(std::f32::consts::TAU);
        let angle_delta = (normalized_angle - self.active_angle).abs();
        let wrapped_delta = angle_delta.min((std::f32::consts::TAU - angle_delta).abs());
        let needs_rebuild = self.active_image.is_none()
            || self.active_scene != scene_asset
            || wrapped_delta >= 0.012
            || (dim - self.active_dim).abs() >= 0.03;

        if !needs_rebuild {
            return (self.active_image.clone(), None);
        }

        if !self.decoded.contains_key(scene_asset) {
            if let Some(decoded) = Self::decode_scene(scene_asset) {
                self.decoded.insert(scene_asset, decoded);
            } else {
                return (self.active_image.clone(), None);
            }
        }

        let Some(src) = self.decoded.get(scene_asset) else {
            return (self.active_image.clone(), None);
        };
        let Some(next) = Self::build_rotated_image(src, normalized_angle, dim) else {
            return (self.active_image.clone(), None);
        };

        let stale = self.active_image.replace(Arc::clone(&next));
        self.active_scene = scene_asset;
        self.active_angle = normalized_angle;
        self.active_dim = dim;
        (Some(next), stale)
    }
}

/// GPU canvas painting the genuinely spinning circular center album artwork on the vinyl platter.
pub fn radio_vinyl_cover_canvas(
    scene_asset: &'static str,
    vinyl_angle: f32,
    dim_factor: f32,
    cover_d: f32,
    reduced_motion: bool,
    cache: Arc<Mutex<VinylCoverCache>>,
) -> impl IntoElement {
    let spin = if reduced_motion { 0.0 } else { vinyl_angle };
    let dim = dim_factor.clamp(0.08, 1.0);
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let (current, stale) = match cache.lock() {
                Ok(mut guard) => guard.image_for_angle(scene_asset, spin, dim),
                Err(_) => (None, None),
            };
            if let Some(old) = stale {
                let _ = window.drop_image(old);
            }
            if let Some(img) = current {
                let r = f32::from(bounds.size.width).min(f32::from(bounds.size.height)) * 0.5;
                let _ = window.paint_image(bounds, bounds, Corners::all(px(r)), img, 0, false);
            }
        },
    )
    .size(px(cover_d))
}

#[cfg(test)]
mod vinyl_cover_tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn vinyl_cover_cache_decodes_and_rotates_circular_bgra_frames() {
        let mut cache = VinylCoverCache::new();
        let (img0, stale0) = cache.image_for_angle("scene-spirited.png", 0.0, 1.0);
        assert!(stale0.is_none());
        let img0 = img0.expect("decoded initial vinyl cover");
        let bytes0 = img0.as_bytes(0).expect("frame 0 bytes").to_vec();
        assert_eq!(bytes0.len(), VINYL_COVER_PX * VINYL_COVER_PX * 4);
        // Top-left corner outside circular mask must be fully transparent
        assert_eq!(bytes0[3], 0);
        // Center pixel inside circle must be opaque
        let mid = ((VINYL_COVER_PX / 2) * VINYL_COVER_PX + (VINYL_COVER_PX / 2)) * 4;
        assert_eq!(bytes0[mid + 3], 255);

        // Rotating by 1.2 rad produces a genuinely rotated frame and returns the stale one for atlas cleanup
        let (img1, stale1) = cache.image_for_angle("scene-spirited.png", 1.2, 1.0);
        assert!(stale1.is_some());
        let img1 = img1.expect("decoded rotated vinyl cover");
        let bytes1 = img1.as_bytes(0).expect("rotated bytes");
        assert_ne!(bytes0.as_slice(), bytes1);
    }
}

use gpui_kit::*;
use std::{
    collections::VecDeque,
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
                // Studio Ghibli hand-painted atmospheric lighting & gentle floating motes
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
                        paint_glow_orb(window, cloud1, 96.0, 180.0, Hsla::from(rgb(0xffffff).opacity(0.42)));
                        paint_glow_orb(window, cloud2, 108.0, 195.0, Hsla::from(rgb(0xffffff).opacity(0.38)));
                        paint_glow_orb(window, cloud3, 92.0, 170.0, Hsla::from(rgb(0x93c5fd).opacity(0.24)));
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
                        paint_glow_orb(window, sun1, 92.0, 185.0, Hsla::from(rgb(0xfff5cc).opacity(0.36)));
                        paint_glow_orb(window, canopy2, 102.0, 190.0, Hsla::from(rgb(0xa7ddb0).opacity(0.26)));
                        paint_glow_orb(window, moss3, 88.0, 165.0, Hsla::from(rgb(0xdff5d8).opacity(0.32)));
                        if !snap.reduced_motion {
                            for i in 0..10_usize {
                                let seed = i as f32 * 1.618_034;
                                let mx = ox
                                    + w * (0.22 + ((seed * 2.91).sin() * 0.5 + 0.5) * 0.72)
                                    + (p * 0.42 + seed).sin() * 14.0
                                    - px_norm * 14.0;
                                let travel = ((p * 0.022 + seed * 0.31).rem_euclid(1.0)) * h;
                                let my = oy + h - travel;
                                let pulse = 0.40 + 0.60 * ((p * 1.3 + seed * 2.1).sin() * 0.5 + 0.5);
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
                        paint_glow_orb(window, glow1, 98.0, 190.0, Hsla::from(rgb(0xffd7c4).opacity(0.38)));
                        paint_glow_orb(window, glow2, 102.0, 190.0, Hsla::from(rgb(0xf6bcc5).opacity(0.30)));
                        paint_glow_orb(window, glow3, 94.0, 175.0, Hsla::from(rgb(0xb2c9eb).opacity(0.24)));
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
                                    bounds: Bounds::new(point(px(rx), px(ry)), size(px(rw), px(2.0))),
                                    corner_radii: Corners::all(px(1.0)),
                                    background: Hsla::from(rgb(0xfff0d9).opacity(0.32 * shimmer)).into(),
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
                        paint_glow_orb(window, cloud1, 96.0, 185.0, Hsla::from(rgb(0xd4cbf7).opacity(0.32)));
                        paint_glow_orb(window, cloud2, 102.0, 190.0, Hsla::from(rgb(0xffffff).opacity(0.40)));
                        paint_glow_orb(window, bloom3, 90.0, 170.0, Hsla::from(rgb(0xfff1ba).opacity(0.28)));
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
                                    bounds: Bounds::new(point(px(mx), px(my)), size(px(5.0), px(3.0))),
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
                        paint_glow_orb(window, sun1, 96.0, 185.0, Hsla::from(rgb(0xffe7ba).opacity(0.40)));
                        paint_glow_orb(window, coast2, 98.0, 185.0, Hsla::from(rgb(0xf5c4a2).opacity(0.28)));
                        paint_glow_orb(window, sea3, 92.0, 175.0, Hsla::from(rgb(0xbae6fd).opacity(0.24)));
                    }
                    ThemeAtmosphere::Lantern => {
                        // Cozy Spirited Away night lantern glow & gently rising spirit fireflies
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
                        paint_glow_orb(window, lamp1, 84.0, 180.0, Hsla::from(rgb(0x6d4c8c).opacity(0.16)));
                        paint_glow_orb(window, lamp2, 86.0, 185.0, Hsla::from(rgb(0xf5b962).opacity(0.16)));
                        paint_glow_orb(window, river3, 74.0, 165.0, Hsla::from(rgb(0xd96b4e).opacity(0.12)));
                        if !snap.reduced_motion {
                            for i in 0..12_usize {
                                let seed = i as f32 * 1.618_034;
                                let mx = ox
                                    + w * (0.20 + ((seed * 3.17).sin() * 0.5 + 0.5) * 0.76)
                                    + (p * 0.45 + seed).sin() * 14.0
                                    - px_norm * 16.0;
                                let travel = ((p * 0.025 + seed * 0.37).rem_euclid(1.0)) * h;
                                let my = oy + h - travel;
                                let twinkle = 0.35 + 0.65 * ((p * 1.4 + seed * 2.3).sin() * 0.5 + 0.5);
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
                // Tiny crimson ribbon accent honoring Kiki's little black cat companion
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

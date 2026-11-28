use anyhow::{Context, Result};
use std::path::Path;

const WIDTH: usize = 960;
const HEIGHT: usize = 1240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteCardStyle {
    Sky,
    Totoro,
    Spirited,
    Howl,
    Kiki,
    Lantern,
    Prism,
    Paper,
}

impl QuoteCardStyle {
    pub const ALL: [Self; 8] = [
        Self::Sky,
        Self::Totoro,
        Self::Spirited,
        Self::Howl,
        Self::Kiki,
        Self::Lantern,
        Self::Prism,
        Self::Paper,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sky => "晴空旧书房 · 蓝天流云",
            Self::Totoro => "夏风樟树下 · 林荫晴雨",
            Self::Spirited => "浅海晚风号 · 浅海黄昏",
            Self::Howl => "山谷秘密花园 · 晨雾花海",
            Self::Kiki => "海港烘焙屋 · 海风暖阳",
            Self::Lantern => "雨夜灯笼巷 · 暖灯夜读",
            Self::Prism => "星空棱镜",
            Self::Paper => "素纸晨光",
        }
    }

    pub fn from_theme_id(id: &str) -> Self {
        match id {
            "totoro" => Self::Totoro,
            "spirited" => Self::Spirited,
            "howl" => Self::Howl,
            "kiki" => Self::Kiki,
            "lantern" => Self::Lantern,
            "prism" | "night" => Self::Prism,
            "paper" => Self::Paper,
            _ => Self::Sky,
        }
    }
}

#[derive(Clone, Debug)]
pub struct QuoteCardDraft {
    pub text: String,
    pub attribution: String,
    pub note: Option<String>,
    pub style: QuoteCardStyle,
}

#[derive(Clone, Copy)]
struct BoxRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl BoxRect {
    const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Clone, Copy)]
struct TextStyle<'a> {
    font_family: &'a str,
    max_font_px: i32,
    min_font_px: i32,
    weight: i32,
    color: [u8; 3],
    single_line: bool,
}

impl<'a> TextStyle<'a> {
    const fn fixed(font_family: &'a str, font_px: i32, weight: i32, color: [u8; 3]) -> Self {
        Self {
            font_family,
            max_font_px: font_px,
            min_font_px: font_px,
            weight,
            color,
            single_line: false,
        }
    }
}

struct Canvas {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            rgb: vec![0; w * h * 3],
        }
    }

    #[inline]
    fn blend_px(&mut self, x: i32, y: i32, color: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 || alpha <= 0.001 {
            return;
        }
        let a = alpha.clamp(0.0, 1.0);
        let idx = (y as usize * self.w + x as usize) * 3;
        let inv = 1.0 - a;
        self.rgb[idx] = (self.rgb[idx] as f32 * inv + color[0] as f32 * a).round() as u8;
        self.rgb[idx + 1] = (self.rgb[idx + 1] as f32 * inv + color[1] as f32 * a).round() as u8;
        self.rgb[idx + 2] = (self.rgb[idx + 2] as f32 * inv + color[2] as f32 * a).round() as u8;
    }

    fn fill_vertical_gradient(&mut self, stops: &[(f32, [u8; 3])]) {
        for y in 0..self.h {
            let t = y as f32 / (self.h - 1).max(1) as f32;
            let c = sample_stops(stops, t);
            let row = y * self.w * 3;
            for x in 0..self.w {
                let idx = row + x * 3;
                self.rgb[idx] = c[0];
                self.rgb[idx + 1] = c[1];
                self.rgb[idx + 2] = c[2];
            }
        }
    }

    fn fill_radial_glow(&mut self, cx: f32, cy: f32, radius: f32, color: [u8; 3], max_alpha: f32) {
        let x0 = ((cx - radius).floor() as i32).max(0);
        let x1 = ((cx + radius).ceil() as i32).min(self.w as i32 - 1);
        let y0 = ((cy - radius).floor() as i32).max(0);
        let y1 = ((cy + radius).ceil() as i32).min(self.h as i32 - 1);
        let r2 = (radius * radius).max(1.0);
        for y in y0..=y1 {
            let dy = y as f32 - cy;
            for x in x0..=x1 {
                let dx = x as f32 - cx;
                let d2 = dx * dx + dy * dy;
                if d2 < r2 {
                    let norm = 1.0 - (d2 / r2).sqrt();
                    let a = norm * norm * max_alpha;
                    self.blend_px(x, y, color, a);
                }
            }
        }
    }

    fn fill_rounded_rect(&mut self, rect: BoxRect, radius: f32, color: [u8; 3], alpha: f32) {
        let BoxRect {
            x: rx,
            y: ry,
            w: rw,
            h: rh,
        } = rect;
        let x0 = rx.max(0);
        let y0 = ry.max(0);
        let x1 = (rx + rw).min(self.w as i32);
        let y1 = (ry + rh).min(self.h as i32);
        let r = radius.max(0.0);
        for y in y0..y1 {
            for x in x0..x1 {
                let cov = rounded_rect_coverage(
                    x as f32 + 0.5,
                    y as f32 + 0.5,
                    rx as f32,
                    ry as f32,
                    rw as f32,
                    rh as f32,
                    r,
                );
                if cov > 0.001 {
                    self.blend_px(x, y, color, alpha * cov);
                }
            }
        }
    }

    fn stroke_rounded_rect(
        &mut self,
        rect: BoxRect,
        radius: f32,
        thickness: f32,
        color: [u8; 3],
        alpha: f32,
    ) {
        let BoxRect {
            x: rx,
            y: ry,
            w: rw,
            h: rh,
        } = rect;
        let pad = thickness.ceil() as i32 + 2;
        let x0 = (rx - pad).max(0);
        let y0 = (ry - pad).max(0);
        let x1 = (rx + rw + pad).min(self.w as i32);
        let y1 = (ry + rh + pad).min(self.h as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                let d = rounded_rect_sdf(
                    x as f32 + 0.5,
                    y as f32 + 0.5,
                    rx as f32,
                    ry as f32,
                    rw as f32,
                    rh as f32,
                    radius,
                );
                let dist_to_edge = (d + thickness * 0.5).abs();
                if dist_to_edge <= thickness * 0.5 + 1.0 {
                    let cov = (thickness * 0.5 + 1.0 - dist_to_edge).clamp(0.0, 1.0);
                    self.blend_px(x, y, color, alpha * cov);
                }
            }
        }
    }

    fn fill_horizontal_gradient_bar(
        &mut self,
        rx: i32,
        ry: i32,
        rw: i32,
        rh: i32,
        stops: &[(f32, [u8; 3])],
        alpha: f32,
    ) {
        for y in ry.max(0)..(ry + rh).min(self.h as i32) {
            for x in rx.max(0)..(rx + rw).min(self.w as i32) {
                let t = (x - rx) as f32 / rw.max(1) as f32;
                let c = sample_stops(stops, t);
                self.blend_px(x, y, c, alpha);
            }
        }
    }
}

fn sample_stops(stops: &[(f32, [u8; 3])], t: f32) -> [u8; 3] {
    let p = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if p >= t0 && p <= t1 {
            let local = ((p - t0) / (t1 - t0).max(0.0001)).clamp(0.0, 1.0);
            return [
                (c0[0] as f32 + (c1[0] as f32 - c0[0] as f32) * local).round() as u8,
                (c0[1] as f32 + (c1[1] as f32 - c0[1] as f32) * local).round() as u8,
                (c0[2] as f32 + (c1[2] as f32 - c0[2] as f32) * local).round() as u8,
            ];
        }
    }
    stops.last().map(|s| s.1).unwrap_or([255, 255, 255])
}

fn rounded_rect_sdf(px: f32, py: f32, rx: f32, ry: f32, rw: f32, rh: f32, r: f32) -> f32 {
    let cx = rx + rw * 0.5;
    let cy = ry + rh * 0.5;
    let hx = rw * 0.5 - r;
    let hy = rh * 0.5 - r;
    let qx = (px - cx).abs() - hx;
    let qy = (py - cy).abs() - hy;
    let out = (qx.max(0.0).hypot(qy.max(0.0))) - r;
    let inside = qx.max(qy).min(0.0);
    out + inside
}

fn rounded_rect_coverage(px: f32, py: f32, rx: f32, ry: f32, rw: f32, rh: f32, r: f32) -> f32 {
    let d = rounded_rect_sdf(px, py, rx, ry, rw, rh, r);
    (0.5 - d).clamp(0.0, 1.0)
}

pub fn render_png(draft: &QuoteCardDraft) -> Result<Vec<u8>> {
    let mut canvas = Canvas::new(WIDTH, HEIGHT);
    let (ink, muted, accent, card_bg, card_alpha, card_border, note_bg) = match draft.style {
        QuoteCardStyle::Sky => {
            canvas.fill_vertical_gradient(&[
                (0.0, [62, 152, 224]),
                (0.38, [142, 203, 246]),
                (0.76, [218, 238, 253]),
                (1.0, [245, 251, 255]),
            ]);
            // Original sunlit cumulus clouds in a bright reading sky
            for &(cx, cy, rad, col, a) in &[
                (170.0, 190.0, 210.0, [255, 255, 255], 0.52),
                (310.0, 140.0, 175.0, [255, 252, 244], 0.44),
                (790.0, 220.0, 240.0, [255, 255, 255], 0.58),
                (880.0, 310.0, 190.0, [232, 245, 255], 0.48),
                (140.0, 1080.0, 260.0, [255, 255, 255], 0.72),
                (440.0, 1140.0, 240.0, [255, 255, 255], 0.68),
                (790.0, 1090.0, 270.0, [255, 255, 255], 0.72),
                (640.0, 155.0, 130.0, [255, 245, 215], 0.34),
            ] {
                canvas.fill_radial_glow(cx, cy, rad, col, a);
            }
            (
                [26, 50, 76],
                [79, 110, 142],
                [37, 115, 184],
                [249, 253, 255],
                0.93,
                [160, 205, 238],
                [225, 240, 252],
            )
        }
        QuoteCardStyle::Totoro => {
            canvas.fill_vertical_gradient(&[
                (0.0, [134, 189, 146]),
                (0.42, [196, 224, 194]),
                (0.78, [232, 244, 226]),
                (1.0, [246, 251, 241]),
            ]);
            for &(cx, cy, rad, col, a) in &[
                (180.0, 175.0, 230.0, [255, 246, 206], 0.56),
                (790.0, 210.0, 240.0, [164, 216, 168], 0.44),
                (300.0, 1110.0, 260.0, [242, 250, 235], 0.68),
                (760.0, 1080.0, 250.0, [255, 247, 214], 0.54),
                (670.0, 165.0, 110.0, [255, 244, 196], 0.44),
            ] {
                canvas.fill_radial_glow(cx, cy, rad, col, a);
            }
            (
                [31, 54, 38],
                [78, 107, 86],
                [61, 122, 82],
                [249, 252, 246],
                0.94,
                [172, 208, 178],
                [231, 242, 228],
            )
        }
        QuoteCardStyle::Spirited => {
            canvas.fill_vertical_gradient(&[
                (0.0, [152, 168, 214]),
                (0.36, [235, 184, 176]),
                (0.72, [248, 220, 205]),
                (1.0, [214, 226, 245]),
            ]);
            for &(cx, cy, rad, col, a) in &[
                (210.0, 210.0, 240.0, [255, 236, 214], 0.52),
                (780.0, 230.0, 230.0, [248, 198, 188], 0.48),
                (480.0, 1110.0, 320.0, [255, 244, 234], 0.64),
                (760.0, 1070.0, 220.0, [255, 224, 176], 0.42),
            ] {
                canvas.fill_radial_glow(cx, cy, rad, col, a);
            }
            (
                [59, 36, 39],
                [116, 84, 88],
                [184, 75, 66],
                [255, 249, 245],
                0.94,
                [226, 182, 174],
                [247, 232, 226],
            )
        }
        QuoteCardStyle::Howl => {
            canvas.fill_vertical_gradient(&[
                (0.0, [145, 166, 232]),
                (0.40, [202, 198, 242]),
                (0.76, [238, 235, 250]),
                (1.0, [248, 247, 238]),
            ]);
            for &(cx, cy, rad, col, a) in &[
                (190.0, 185.0, 220.0, [255, 255, 255], 0.54),
                (780.0, 210.0, 240.0, [255, 248, 212], 0.46),
                (240.0, 1100.0, 250.0, [250, 218, 228], 0.46),
                (740.0, 1090.0, 260.0, [255, 246, 204], 0.52),
            ] {
                canvas.fill_radial_glow(cx, cy, rad, col, a);
            }
            (
                [43, 39, 64],
                [95, 90, 122],
                [99, 84, 158],
                [252, 251, 247],
                0.94,
                [196, 188, 230],
                [238, 235, 248],
            )
        }
        QuoteCardStyle::Kiki => {
            canvas.fill_vertical_gradient(&[
                (0.0, [148, 198, 226]),
                (0.38, [238, 214, 182]),
                (0.76, [247, 234, 214]),
                (1.0, [253, 248, 238]),
            ]);
            for &(cx, cy, rad, col, a) in &[
                (180.0, 180.0, 230.0, [255, 246, 220], 0.58),
                (790.0, 220.0, 235.0, [242, 196, 158], 0.42),
                (480.0, 1110.0, 300.0, [255, 250, 238], 0.68),
            ] {
                canvas.fill_radial_glow(cx, cy, rad, col, a);
            }
            (
                [59, 42, 30],
                [117, 93, 74],
                [174, 87, 42],
                [255, 253, 247],
                0.95,
                [222, 192, 164],
                [247, 238, 224],
            )
        }
        QuoteCardStyle::Lantern => {
            canvas.fill_vertical_gradient(&[
                (0.0, [17, 15, 27]),
                (0.50, [29, 23, 40]),
                (1.0, [19, 15, 28]),
            ]);
            for &(cx, cy, rad, col, a) in &[
                (210.0, 200.0, 290.0, [112, 76, 146], 0.24),
                (770.0, 260.0, 310.0, [230, 159, 82], 0.28),
                (680.0, 1050.0, 300.0, [196, 86, 68], 0.20),
                (220.0, 1080.0, 240.0, [245, 194, 108], 0.16),
            ] {
                canvas.fill_radial_glow(cx, cy, rad, col, a);
            }
            (
                [245, 233, 214],
                [188, 168, 149],
                [230, 159, 82],
                [32, 27, 38],
                0.95,
                [230, 159, 82],
                [46, 37, 52],
            )
        }
        QuoteCardStyle::Prism => {
            canvas.fill_vertical_gradient(&[
                (0.0, [8, 10, 18]),
                (0.55, [13, 17, 30]),
                (1.0, [9, 12, 22]),
            ]);
            canvas.fill_radial_glow(220.0, 210.0, 340.0, [124, 58, 237], 0.24);
            canvas.fill_radial_glow(770.0, 320.0, 320.0, [14, 165, 233], 0.20);
            canvas.fill_radial_glow(680.0, 1040.0, 300.0, [245, 158, 11], 0.14);
            // Corner optical dot-matrix
            for row in 0..7 {
                for col in 0..9 {
                    let gx = 78 + col * 24;
                    let gy = 1170 - row * 24;
                    let fade = (1.0 - col as f32 / 9.0) * (1.0 - row as f32 / 7.0);
                    canvas.fill_rounded_rect(
                        BoxRect::new(gx, gy, 3, 3),
                        1.5,
                        [155, 135, 255],
                        0.36 * fade,
                    );
                }
            }
            (
                [238, 241, 250],
                [148, 160, 190],
                [167, 139, 250],
                [18, 22, 36],
                0.94,
                [155, 135, 255],
                [28, 34, 54],
            )
        }
        QuoteCardStyle::Paper => {
            canvas.fill_vertical_gradient(&[
                (0.0, [244, 238, 225]),
                (0.5, [249, 245, 236]),
                (1.0, [236, 228, 212]),
            ]);
            // Subtle tactile paper warmth grain
            let mut seed = 0x517C_C1B7_2722_0A95u64;
            for y in (0..HEIGHT).step_by(2) {
                for x in (0..WIDTH).step_by(2) {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    if (seed & 7) == 0 {
                        let dark = (seed & 8) != 0;
                        let col = if dark {
                            [160, 142, 118]
                        } else {
                            [255, 253, 248]
                        };
                        canvas.blend_px(x as i32, y as i32, col, 0.06);
                    }
                }
            }
            (
                [46, 41, 35],
                [112, 102, 90],
                [138, 94, 59],
                [255, 252, 245],
                0.96,
                [206, 192, 172],
                [243, 236, 224],
            )
        }
    };

    // Soft outer shadow beneath inner stationery card
    let (cx, cy, cw, ch) = (76, 86, WIDTH as i32 - 152, HEIGHT as i32 - 172);
    for layer in 0..6 {
        let spread = (layer + 1) * 5;
        let shadow_alpha = match draft.style {
            QuoteCardStyle::Prism | QuoteCardStyle::Lantern => 0.10 - layer as f32 * 0.014,
            _ => 0.045 - layer as f32 * 0.006,
        };
        canvas.fill_rounded_rect(
            BoxRect::new(cx - spread / 2, cy + spread, cw + spread, ch + spread / 2),
            34.0,
            [12, 22, 38],
            shadow_alpha.max(0.005),
        );
    }

    // Inner stationery card body & crisp border
    let card_rect = BoxRect::new(cx, cy, cw, ch);
    canvas.fill_rounded_rect(card_rect, 30.0, card_bg, card_alpha);
    canvas.stroke_rounded_rect(card_rect, 30.0, 2.0, card_border, 0.55);

    if draft.style == QuoteCardStyle::Paper {
        canvas.stroke_rounded_rect(
            BoxRect::new(cx + 18, cy + 18, cw - 36, ch - 36),
            20.0,
            1.2,
            accent,
            0.25,
        );
    }

    // Top & bottom signature light beams
    let beam_stops: &[(f32, [u8; 3])] = match draft.style {
        QuoteCardStyle::Prism => &[
            (0.0, [251, 191, 36]),
            (0.25, [251, 113, 133]),
            (0.50, [168, 85, 247]),
            (0.75, [56, 189, 248]),
            (1.0, [52, 211, 153]),
        ],
        QuoteCardStyle::Sky => &[
            (0.0, [147, 197, 253]),
            (0.5, [37, 115, 184]),
            (1.0, [186, 230, 253]),
        ],
        QuoteCardStyle::Totoro => &[
            (0.0, [186, 228, 184]),
            (0.5, [61, 122, 82]),
            (1.0, [242, 224, 166]),
        ],
        QuoteCardStyle::Spirited => &[
            (0.0, [245, 194, 168]),
            (0.5, [184, 75, 66]),
            (1.0, [166, 188, 230]),
        ],
        QuoteCardStyle::Howl => &[
            (0.0, [199, 190, 242]),
            (0.5, [99, 84, 158]),
            (1.0, [248, 224, 170]),
        ],
        QuoteCardStyle::Kiki => &[
            (0.0, [242, 200, 155]),
            (0.5, [174, 87, 42]),
            (1.0, [158, 206, 232]),
        ],
        QuoteCardStyle::Lantern => &[
            (0.0, [196, 86, 68]),
            (0.5, [230, 159, 82]),
            (1.0, [253, 228, 168]),
        ],
        QuoteCardStyle::Paper => &[
            (0.0, [214, 188, 154]),
            (0.5, [138, 94, 59]),
            (1.0, [214, 188, 154]),
        ],
    };
    canvas.fill_horizontal_gradient_bar(cx + 42, cy + 2, cw - 84, 5, beam_stops, 0.92);
    canvas.fill_horizontal_gradient_bar(cx + 42, cy + ch - 7, cw - 84, 4, beam_stops, 0.68);

    // Top studio pill badge & decorative quotation accent bar
    let pad_x = cx + 68;
    let content_w = cw - 136;
    canvas.fill_rounded_rect(BoxRect::new(pad_x, cy + 56, 256, 36), 18.0, accent, 0.14);
    canvas.fill_rounded_rect(BoxRect::new(pad_x + 16, cy + 70, 8, 8), 4.0, accent, 0.92);

    let header_tag = match draft.style {
        QuoteCardStyle::Sky => "GLOW · 晴空书房书摘",
        QuoteCardStyle::Totoro => "GLOW · 夏风樟树书摘",
        QuoteCardStyle::Spirited => "GLOW · 浅海晚风书摘",
        QuoteCardStyle::Howl => "GLOW · 秘密花园书摘",
        QuoteCardStyle::Kiki => "GLOW · 海港暖阳书摘",
        QuoteCardStyle::Lantern => "GLOW · 雨夜灯巷书摘",
        QuoteCardStyle::Prism => "GLOW · 星空棱镜书摘",
        QuoteCardStyle::Paper => "GLOW · 素纸晨光书摘",
    };
    draw_text(
        &mut canvas,
        header_tag,
        BoxRect::new(pad_x + 34, cy + 61, 220, 28),
        TextStyle::fixed("Microsoft YaHei UI", 19, 600, accent),
    );

    // Big decorative open quote glyph
    draw_text(
        &mut canvas,
        "“",
        BoxRect::new(pad_x - 8, cy + 116, 96, 96),
        TextStyle::fixed("KaiTi", 92, 400, accent),
    );

    // Main quote passage (auto-sized & fitted cleanly for both short and long multi-line text)
    let quote_clean = normalize_card_text(&draft.text, 360);
    let char_count = quote_clean.chars().count();
    let max_font_px = if char_count <= 48 {
        44
    } else if char_count <= 110 {
        36
    } else if char_count <= 200 {
        30
    } else {
        26
    };
    let has_note = draft.note.as_deref().is_some_and(|n| !n.trim().is_empty());
    let quote_y = cy + 204;
    let footer_div_y = cy + ch - 138;
    let attr_y = footer_div_y - 46;
    let note_box_h = 152;
    let max_note_y = attr_y - 26 - note_box_h;
    let max_quote_h = if has_note {
        (max_note_y - quote_y - 28).max(260)
    } else {
        (attr_y - quote_y - 36).max(360)
    };
    let used_h = draw_text_fitted(
        &mut canvas,
        &quote_clean,
        BoxRect::new(pad_x, quote_y, content_w, max_quote_h),
        TextStyle {
            font_family: "KaiTi",
            max_font_px,
            min_font_px: 20,
            weight: 400,
            color: ink,
            single_line: false,
        },
    );

    // Optional personal note callout box with guaranteed breathing room above attribution
    if let Some(note) = draft
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        let note_clean = normalize_card_text(note, 180);
        let box_y = (quote_y + used_h.min(max_quote_h) + 28).min(max_note_y);
        canvas.fill_rounded_rect(
            BoxRect::new(pad_x, box_y, content_w, note_box_h),
            18.0,
            note_bg,
            0.92,
        );
        canvas.fill_rounded_rect(
            BoxRect::new(pad_x, box_y + 14, 5, note_box_h - 28),
            2.5,
            accent,
            0.90,
        );
        draw_text(
            &mut canvas,
            "读时批注 · MARGINALIA",
            BoxRect::new(pad_x + 24, box_y + 16, content_w - 40, 26),
            TextStyle::fixed("Microsoft YaHei UI", 17, 600, accent),
        );
        draw_text_fitted(
            &mut canvas,
            &note_clean,
            BoxRect::new(pad_x + 24, box_y + 48, content_w - 44, note_box_h - 60),
            TextStyle {
                font_family: "Microsoft YaHei UI",
                max_font_px: 21,
                min_font_px: 17,
                weight: 400,
                color: ink,
                single_line: false,
            },
        );
    }

    // Attribution & signature footer (single-line ellipsis so long titles never collide with divider)
    canvas.fill_rounded_rect(BoxRect::new(pad_x, attr_y + 14, 34, 3), 1.5, accent, 0.85);
    let attr_single = draft
        .attribution
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    draw_text_fitted(
        &mut canvas,
        &attr_single,
        BoxRect::new(pad_x + 48, attr_y, content_w - 48, 34),
        TextStyle {
            font_family: "Microsoft YaHei UI",
            max_font_px: 23,
            min_font_px: 23,
            weight: 500,
            color: muted,
            single_line: true,
        },
    );

    // Hairline divider
    canvas.fill_rounded_rect(
        BoxRect::new(pad_x, footer_div_y, content_w, 2),
        1.0,
        card_border,
        0.45,
    );

    // Footer emblem + tagline
    let seal_y = footer_div_y + 30;
    canvas.fill_rounded_rect(BoxRect::new(pad_x, seal_y, 50, 50), 14.0, accent, 0.92);
    draw_text(
        &mut canvas,
        "微",
        BoxRect::new(pad_x + 11, seal_y + 8, 36, 36),
        TextStyle::fixed("KaiTi", 30, 700, [255, 255, 255]),
    );
    draw_text(
        &mut canvas,
        "微光阅读 · GLOW READING",
        BoxRect::new(pad_x + 68, seal_y + 4, 420, 28),
        TextStyle::fixed("Microsoft YaHei UI", 20, 600, ink),
    );
    draw_text(
        &mut canvas,
        "一盏灯，一本书 · 把触动留给时间",
        BoxRect::new(pad_x + 68, seal_y + 30, 460, 24),
        TextStyle::fixed("Microsoft YaHei UI", 17, 400, muted),
    );

    Ok(encode_png_rgb(WIDTH as u32, HEIGHT as u32, &canvas.rgb))
}

fn normalize_card_text(raw: &str, max_chars: usize) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::new();
    let mut newline_run = 0u8;
    for ch in normalized.trim().chars() {
        if ch == '\n' {
            newline_run = newline_run.saturating_add(1);
            if newline_run <= 2 {
                out.push('\n');
            }
        } else {
            newline_run = 0;
            out.push(ch);
        }
    }
    if out.chars().count() > max_chars {
        let mut truncated: String = out.chars().take(max_chars.saturating_sub(1)).collect();
        truncated = truncated.trim_end().to_owned();
        truncated.push('…');
        truncated
    } else {
        out
    }
}

pub fn save_png(draft: &QuoteCardDraft, path: &Path) -> Result<()> {
    let bytes = render_png(draft)?;
    glow_core::store::atomic_write(path, &bytes).context("无法保存微光书摘卡 PNG")
}

fn draw_text(canvas: &mut Canvas, text: &str, rect: BoxRect, style: TextStyle<'_>) -> i32 {
    draw_text_fitted(canvas, text, rect, style)
}

#[cfg(target_os = "windows")]
fn draw_text_fitted(canvas: &mut Canvas, text: &str, rect: BoxRect, style: TextStyle<'_>) -> i32 {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    let BoxRect { x, y, w, h } = rect;
    let TextStyle {
        font_family,
        max_font_px,
        min_font_px,
        weight,
        color,
        single_line,
    } = style;

    if text.is_empty() || w <= 2 || h <= 2 {
        return 0;
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        bi_size: u32,
        bi_width: i32,
        bi_height: i32,
        bi_planes: u16,
        bi_bit_count: u16,
        bi_compression: u32,
        bi_size_image: u32,
        bi_x_pels_per_meter: i32,
        bi_y_pels_per_meter: i32,
        bi_clr_used: u32,
        bi_clr_important: u32,
    }
    #[repr(C)]
    struct BitmapInfo {
        bmi_header: BitmapInfoHeader,
        bmi_colors: [u32; 1],
    }
    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(hdc: *mut c_void) -> *mut c_void;
        fn DeleteDC(hdc: *mut c_void) -> i32;
        fn CreateDIBSection(
            hdc: *mut c_void,
            pbmi: *const BitmapInfo,
            usage: u32,
            ppv_bits: *mut *mut c_void,
            h_section: *mut c_void,
            offset: u32,
        ) -> *mut c_void;
        fn CreateFontW(
            c_height: i32,
            c_width: i32,
            c_escapement: i32,
            c_orientation: i32,
            c_weight: i32,
            b_italic: u32,
            b_underline: u32,
            b_strike_out: u32,
            i_char_set: u32,
            i_out_precision: u32,
            i_clip_precision: u32,
            i_quality: u32,
            i_pitch_and_family: u32,
            psz_face_name: *const u16,
        ) -> *mut c_void;
        fn SelectObject(hdc: *mut c_void, h: *mut c_void) -> *mut c_void;
        fn DeleteObject(ho: *mut c_void) -> i32;
        fn SetBkMode(hdc: *mut c_void, mode: i32) -> i32;
        fn SetTextColor(hdc: *mut c_void, color: u32) -> u32;
        fn GdiFlush() -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn DrawTextW(
            hdc: *mut c_void,
            lpch_text: *const u16,
            cch_text: i32,
            lprc: *mut Rect,
            format: u32,
        ) -> i32;
    }

    const BI_RGB: u32 = 0;
    const DIB_RGB_COLORS: u32 = 0;
    const TRANSPARENT: i32 = 1;
    const DEFAULT_CHARSET: u32 = 1;
    const ANTIALIASED_QUALITY: u32 = 4;
    const DT_LEFT: u32 = 0x0000;
    const DT_WORDBREAK: u32 = 0x0010;
    const DT_SINGLELINE: u32 = 0x0020;
    const DT_CALCRECT: u32 = 0x0400;
    const DT_NOPREFIX: u32 = 0x0800;
    const DT_EDITCONTROL: u32 = 0x2000;
    const DT_END_ELLIPSIS: u32 = 0x8000;

    let mut face: Vec<u16> = font_family.encode_utf16().collect();
    face.push(0);

    unsafe {
        let hdc = CreateCompatibleDC(null_mut());
        if hdc.is_null() {
            return 0;
        }
        let bmi = BitmapInfo {
            bmi_header: BitmapInfoHeader {
                bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                bi_width: w,
                bi_height: -h, // top-down DIB
                bi_planes: 1,
                bi_bit_count: 32,
                bi_compression: BI_RGB,
                bi_size_image: 0,
                bi_x_pels_per_meter: 0,
                bi_y_pels_per_meter: 0,
                bi_clr_used: 0,
                bi_clr_important: 0,
            },
            bmi_colors: [0],
        };
        let mut bits: *mut c_void = null_mut();
        let hbm = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if hbm.is_null() || bits.is_null() {
            if !hbm.is_null() {
                DeleteObject(hbm);
            }
            DeleteDC(hdc);
            return 0;
        }
        let old_bm = SelectObject(hdc, hbm);

        let base_flags = if single_line {
            DT_LEFT | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS
        } else {
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_EDITCONTROL
        };

        let measure_height = |hdc: *mut c_void, sample: &str| -> i32 {
            let wide: Vec<u16> = sample.encode_utf16().collect();
            let mut rc = Rect {
                left: 0,
                top: 0,
                right: w,
                bottom: h,
            };
            DrawTextW(
                hdc,
                wide.as_ptr(),
                wide.len() as i32,
                &mut rc,
                base_flags | DT_CALCRECT,
            );
            rc.bottom
        };

        let min_px = min_font_px.min(max_font_px).max(12);
        let mut px_try = max_font_px.max(min_px);
        let (chosen_font, old_font, mut chosen_h) = loop {
            let hfont = CreateFontW(
                -px_try,
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                0,
                0,
                ANTIALIASED_QUALITY,
                0,
                face.as_ptr(),
            );
            let prev = if !hfont.is_null() {
                SelectObject(hdc, hfont)
            } else {
                null_mut()
            };
            let mh = measure_height(hdc, text);
            if single_line || mh <= h || px_try <= min_px {
                break (hfont, prev, mh);
            }
            if !prev.is_null() {
                SelectObject(hdc, prev);
            }
            if !hfont.is_null() {
                DeleteObject(hfont);
            }
            px_try = (px_try - 2).max(min_px);
        };

        // If multi-line text still exceeds max height at min_font_px, binary-search a clean
        // character-boundary truncation with "…" so glyphs are never sliced mid-line.
        let fitted_text = if !single_line && chosen_h > h {
            let chars: Vec<char> = text.chars().collect();
            let mut lo = 1usize;
            let mut hi = chars.len();
            let mut best = String::from("…");
            let mut best_h = h;
            while lo <= hi {
                let mid = (lo + hi) / 2;
                let mut candidate: String = chars[..mid].iter().collect();
                candidate = candidate.trim_end().to_owned();
                candidate.push('…');
                let mh = measure_height(hdc, &candidate);
                if mh <= h {
                    best = candidate;
                    best_h = mh;
                    lo = mid + 1;
                } else {
                    if mid == 0 {
                        break;
                    }
                    hi = mid - 1;
                }
            }
            chosen_h = best_h;
            best
        } else {
            text.to_owned()
        };

        // Zero-fill DIB buffer cleanly to black
        let byte_len = (w as usize) * (h as usize) * 4;
        std::ptr::write_bytes(bits as *mut u8, 0, byte_len);

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, 0x00FF_FFFF);

        let wide: Vec<u16> = fitted_text.encode_utf16().collect();
        let mut draw_rect = Rect {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        DrawTextW(
            hdc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut draw_rect,
            base_flags,
        );
        GdiFlush();

        let pixels = std::slice::from_raw_parts(bits as *const u8, byte_len);
        for py in 0..h {
            let row_off = (py as usize) * (w as usize) * 4;
            for px in 0..w {
                let b = pixels[row_off + (px as usize) * 4];
                let g = pixels[row_off + (px as usize) * 4 + 1];
                let r = pixels[row_off + (px as usize) * 4 + 2];
                let cov = ((r as u16 + g as u16 + b as u16) as f32) / (3.0 * 255.0);
                if cov > 0.005 {
                    canvas.blend_px(x + px, y + py, color, cov);
                }
            }
        }

        if !old_font.is_null() {
            SelectObject(hdc, old_font);
        }
        if !chosen_font.is_null() {
            DeleteObject(chosen_font);
        }
        SelectObject(hdc, old_bm);
        DeleteObject(hbm);
        DeleteDC(hdc);

        chosen_h.min(h)
    }
}

#[cfg(not(target_os = "windows"))]
fn draw_text_fitted(_canvas: &mut Canvas, text: &str, rect: BoxRect, style: TextStyle<'_>) -> i32 {
    if text.is_empty() || rect.w <= 2 || rect.h <= 2 {
        return 0;
    }
    style.max_font_px.min(rect.h)
}

/// Pure-Rust RFC 2083 + RFC 1950/1951 PNG encoder (RGB8, PNG Sub filter + Fixed-Huffman RLE deflate).
fn encode_png_rgb(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let stride = (width as usize) * 3;
    let mut filtered = Vec::with_capacity((stride + 1) * (height as usize));
    for y in 0..height as usize {
        // Filter type 1 (Sub) turns smooth gradients & solid areas into long runs of small/zero bytes
        filtered.push(1);
        let row = &rgb[y * stride..(y + 1) * stride];
        for i in 0..stride {
            let left = if i >= 3 { row[i - 3] } else { 0 };
            filtered.push(row[i].wrapping_sub(left));
        }
    }
    let zlib = zlib_compress_fixed_rle(&filtered);

    let mut out = Vec::with_capacity(zlib.len() + 128);
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(2); // color type: Truecolor RGB
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut out, b"IHDR", &ihdr);
    write_png_chunk(&mut out, b"IDAT", &zlib);
    write_png_chunk(&mut out, b"IEND", &[]);
    out
}

fn write_png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(kind, data);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(kind: &[u8; 4], data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in kind.iter().chain(data.iter()) {
        c ^= b as u32;
        for _ in 0..8 {
            if (c & 1) != 0 {
                c = (c >> 1) ^ 0xEDB8_8320;
            } else {
                c >>= 1;
            }
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

struct BitWriter {
    buf: Vec<u8>,
    acc: u32,
    bits: u8,
}

impl BitWriter {
    fn new(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
            acc: 0,
            bits: 0,
        }
    }

    fn write_bits_lsb(&mut self, value: u32, count: u8) {
        self.acc |= (value & ((1u32 << count) - 1)) << self.bits;
        self.bits += count;
        while self.bits >= 8 {
            self.buf.push((self.acc & 0xFF) as u8);
            self.acc >>= 8;
            self.bits -= 8;
        }
    }

    /// Huffman symbols in DEFLATE are written MSB-first.
    fn write_bits_msb(&mut self, value: u32, count: u8) {
        let mut rev = 0u32;
        for i in 0..count {
            if (value & (1u32 << (count - 1 - i))) != 0 {
                rev |= 1u32 << i;
            }
        }
        self.write_bits_lsb(rev, count);
    }

    fn emit_fixed_litlen(&mut self, sym: u16) {
        match sym {
            0..=143 => self.write_bits_msb(0x30 + sym as u32, 8),
            144..=255 => self.write_bits_msb(0x190 + (sym as u32 - 144), 9),
            256..=279 => self.write_bits_msb(sym as u32 - 256, 7),
            280..=287 => self.write_bits_msb(0xC0 + (sym as u32 - 280), 8),
            _ => unreachable!(),
        }
    }

    fn emit_length_dist(&mut self, length: usize, dist_code: u32) {
        // RFC 1951 length table (lengths 3..=258)
        const LEN_BASE: &[(u16, usize, u8)] = &[
            (257, 3, 0),
            (258, 4, 0),
            (259, 5, 0),
            (260, 6, 0),
            (261, 7, 0),
            (262, 8, 0),
            (263, 9, 0),
            (264, 10, 0),
            (265, 11, 1),
            (266, 13, 1),
            (267, 15, 1),
            (268, 17, 1),
            (269, 19, 2),
            (270, 23, 2),
            (271, 27, 2),
            (272, 31, 2),
            (273, 35, 3),
            (274, 43, 3),
            (275, 51, 3),
            (276, 59, 3),
            (277, 67, 4),
            (278, 83, 4),
            (279, 99, 4),
            (280, 115, 4),
            (281, 131, 5),
            (282, 163, 5),
            (283, 195, 5),
            (284, 227, 5),
            (285, 258, 0),
        ];
        let mut chosen = LEN_BASE[0];
        for &entry in LEN_BASE {
            if length >= entry.1 {
                chosen = entry;
            } else {
                break;
            }
        }
        self.emit_fixed_litlen(chosen.0);
        if chosen.2 > 0 {
            self.write_bits_lsb((length - chosen.1) as u32, chosen.2);
        }
        // Fixed distance codes are 5 bits MSB-first; dist_code 0 = dist 1, dist_code 2 = dist 3
        self.write_bits_msb(dist_code, 5);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.buf.push((self.acc & 0xFF) as u8);
        }
        self.buf
    }
}

fn zlib_compress_fixed_rle(data: &[u8]) -> Vec<u8> {
    let mut bw = BitWriter::new(data.len() / 4 + 64);
    // Zlib header: CMF=0x78 (deflate, 32K window), FLG=0x01 (check bits valid, no dict)
    bw.buf.push(0x78);
    bw.buf.push(0x01);
    // BFINAL=1, BTYPE=01 (fixed Huffman)
    bw.write_bits_lsb(1, 1);
    bw.write_bits_lsb(1, 2);

    let mut i = 0usize;
    let n = data.len();
    while i < n {
        // Check distance-1 run (repeated identical byte) and distance-3 run (repeated RGB delta)
        let mut best_len = 0usize;
        let mut best_dist_code = 0u32;
        if i >= 1 {
            let prev = data[i - 1];
            let mut l = 0usize;
            while l < 258 && i + l < n && data[i + l] == prev {
                l += 1;
            }
            if l >= 3 {
                best_len = l;
                best_dist_code = 0; // distance 1
            }
        }
        if i >= 3 && best_len < 258 {
            let mut l = 0usize;
            while l < 258 && i + l < n && data[i + l] == data[i + l - 3] {
                l += 1;
            }
            if l > best_len && l >= 3 {
                best_len = l;
                best_dist_code = 2; // distance 3
            }
        }
        if best_len >= 3 {
            bw.emit_length_dist(best_len, best_dist_code);
            i += best_len;
        } else {
            bw.emit_fixed_litlen(data[i] as u16);
            i += 1;
        }
    }
    bw.emit_fixed_litlen(256); // End-of-block
    let mut out = bw.finish();
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_valid_png_for_all_styles_and_maps_glow_space_themes() {
        assert_eq!(QuoteCardStyle::ALL.len(), 8);
        assert_eq!(QuoteCardStyle::from_theme_id("forest"), QuoteCardStyle::Sky);
        assert_eq!(
            QuoteCardStyle::from_theme_id("totoro"),
            QuoteCardStyle::Totoro
        );
        assert_eq!(
            QuoteCardStyle::from_theme_id("spirited"),
            QuoteCardStyle::Spirited
        );
        assert_eq!(QuoteCardStyle::from_theme_id("howl"), QuoteCardStyle::Howl);
        assert_eq!(QuoteCardStyle::from_theme_id("kiki"), QuoteCardStyle::Kiki);
        assert_eq!(
            QuoteCardStyle::from_theme_id("lantern"),
            QuoteCardStyle::Lantern
        );
        for style in QuoteCardStyle::ALL {
            let draft = QuoteCardDraft {
                text: "傍晚的风从树梢滑下来，把白天的喧嚣留在林子外面。".into(),
                attribution: "《在林间，慢慢来》 · 微光阅读".into(),
                note: Some("这一句读着很安静。".into()),
                style,
            };
            let bytes = render_png(&draft).unwrap();
            assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
            assert!(bytes.len() > 10_000, "PNG should contain real image data");
            assert_eq!(&bytes[12..16], b"IHDR");
            let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
            assert_eq!((w, h), (WIDTH as u32, HEIGHT as u32));
            assert_eq!(&bytes[bytes.len() - 8..bytes.len() - 4], b"IEND");
        }
    }

    #[test]
    fn stress_long_multiline_cjk_ascii_and_long_attribution_render_cleanly() {
        let long_quote = "云层在蓝天之上缓慢堆叠，像一座安静漂浮的岛屿。\n\n\n\
            Even in a busy world, a quiet page with soft light & warm breeze makes room for breath. \
            窗外的树影摇曳，手边的书页翻过一章又一章；那些被轻轻划下来的句子，\
            会在往后许多个平常的午后重新与你相逢。".repeat(3);
        let long_note = "读到这一段突然想停下来泡一杯茶。\n\n\
            如果文字能像晴空里的流云一样托住疲惫，那每天二十分钟静读就已经非常珍贵。"
            .repeat(2);
        let draft = QuoteCardDraft {
            text: long_quote,
            attribution: "《一座漂浮在蓝天白云之间的超长书名测试示例合集》 · 微光空间原创典藏版"
                .into(),
            note: Some(long_note),
            style: QuoteCardStyle::Sky,
        };
        let bytes = render_png(&draft).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(bytes.len() > 20_000);
        if let Ok(path) = std::env::var("GLOW_DUMP_CARD") {
            let _ = std::fs::write(path, &bytes);
        }
        // Empty note & empty text edge cases must not panic
        let empty = QuoteCardDraft {
            text: "   ".into(),
            attribution: "".into(),
            note: Some("   \n  ".into()),
            style: QuoteCardStyle::Prism,
        };
        assert!(
            render_png(&empty)
                .unwrap()
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
    }
}

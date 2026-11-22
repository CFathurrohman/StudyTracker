//! Calm background scenes, drawn from simple shapes. The same shapes are rasterized on the CPU at a tiny
//! size and blurred; glass panels show that blurred copy, so they look frosted over the scene behind them.

use eframe::egui::{self, epaint::Vertex, pos2, Color32, ColorImage, Mesh, Painter, Pos2, Rect, Shape, Stroke, TextureHandle, TextureOptions};
use std::f32::consts::PI;

pub const SCENES: &[(&str, &str)] = &[
    ("night", "Night room"),
    ("rain", "Rainy room"),
    ("sunset", "Sunset"),
    ("forest", "Morning forest"),
    ("aurora", "Aurora"),
    ("plain", "Plain"),
];

/// Shapes in a unit space: x and y go 0..1 across the area; sizes (`r`) are in units of the area height.
#[derive(Clone)]
enum Prim {
    VGrad { x0: f32, y0: f32, x1: f32, y1: f32, top: [u8; 4], bot: [u8; 4] },
    Rect { x0: f32, y0: f32, x1: f32, y1: f32, c: [u8; 4] },
    Circle { x: f32, y: f32, r: f32, c: [u8; 4] },
    Glow { x: f32, y: f32, r: f32, c: [u8; 4] },
    Poly { pts: Vec<(f32, f32)>, c: [u8; 4] },
}

fn c(hex: u32) -> [u8; 4] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]
}
fn ca(hex: u32, a: f32) -> [u8; 4] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, (a * 255.0) as u8]
}
fn col(v: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(v[0], v[1], v[2], v[3])
}

fn room(p: &mut Vec<Prim>, wall_top: u32, wall_bot: u32, sky_top: u32, sky_bot: u32, curtain: u32, desk: u32) {
    use Prim::*;
    p.push(VGrad { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0, top: c(wall_top), bot: c(wall_bot) });
    // Window with sky.
    p.push(VGrad { x0: 0.5, y0: 0.07, x1: 0.93, y1: 0.66, top: c(sky_top), bot: c(sky_bot) });
    // Curtains.
    p.push(Rect { x0: 0.44, y0: 0.03, x1: 0.53, y1: 0.76, c: c(curtain) });
    p.push(Rect { x0: 0.9, y0: 0.03, x1: 0.99, y1: 0.76, c: c(curtain) });
    // Desk.
    p.push(Rect { x0: 0.0, y0: 0.8, x1: 1.0, y1: 1.0, c: c(desk) });
    p.push(Rect { x0: 0.0, y0: 0.8, x1: 1.0, y1: 0.815, c: c(lighten(desk, 18)) });
}

fn city(p: &mut Vec<Prim>, base: u32, lit: u32, x0: f32, x1: f32, floor: f32, seed: u32) {
    let mut s = seed;
    let mut x = x0;
    while x < x1 {
        s = s.wrapping_mul(1103515245).wrapping_add(12345);
        let w = 0.025 + (s >> 16 & 0xFF) as f32 / 255.0 * 0.03;
        let h = 0.06 + (s >> 8 & 0xFF) as f32 / 255.0 * 0.16;
        let xe = (x + w).min(x1);
        p.push(Prim::Rect { x0: x, y0: floor - h, x1: xe, y1: floor, c: c(base) });
        // A few lit windows.
        let mut yy = floor - h + 0.015;
        while yy < floor - 0.02 {
            s = s.wrapping_mul(1103515245).wrapping_add(12345);
            if s >> 20 & 3 == 0 {
                p.push(Prim::Rect { x0: x + w * 0.3, y0: yy, x1: x + w * 0.3 + 0.006, y1: yy + 0.01, c: ca(lit, 0.9) });
            }
            yy += 0.025;
        }
        x = xe + 0.004;
    }
}

fn lighten(hex: u32, d: u8) -> u32 {
    let ch = |v: u32| (v as u8).saturating_add(d) as u32;
    ch(hex >> 16) << 16 | ch(hex >> 8 & 0xFF) << 8 | ch(hex & 0xFF)
}

fn lamp(p: &mut Vec<Prim>, x: f32, glow: u32) {
    use Prim::*;
    p.push(Glow { x, y: 0.66, r: 0.55, c: ca(glow, 0.35) });
    p.push(Rect { x0: x - 0.004, y0: 0.62, x1: x + 0.004, y1: 0.8, c: c(0x2A2233) });
    p.push(Poly { pts: vec![(x - 0.05, 0.64), (x + 0.05, 0.64), (x + 0.025, 0.57), (x - 0.025, 0.57)], c: c(0xE8B26A) });
    p.push(Rect { x0: x - 0.035, y0: 0.79, x1: x + 0.035, y1: 0.8, c: c(0x2A2233) });
    p.push(Glow { x, y: 0.66, r: 0.12, c: ca(0xFFE2A8, 0.6) });
}

fn plant(p: &mut Vec<Prim>, x: f32, leaf: u32) {
    use Prim::*;
    p.push(Poly { pts: vec![(x - 0.03, 0.72), (x + 0.03, 0.72), (x + 0.022, 0.8), (x - 0.022, 0.8)], c: c(0x4A3428) });
    for (dx, dy, r) in [(-0.02, 0.66, 0.05), (0.02, 0.64, 0.055), (0.0, 0.6, 0.05), (-0.035, 0.62, 0.035), (0.04, 0.68, 0.035)] {
        p.push(Circle { x: x + dx, y: dy, r, c: c(leaf) });
    }
}

fn books(p: &mut Vec<Prim>, x: f32) {
    let colors = [0x7A3B3B, 0x3B5A7A, 0x6A5A2E, 0x3B6A55];
    for (i, cc) in colors.iter().enumerate() {
        let y1 = 0.8 - i as f32 * 0.018;
        p.push(Prim::Rect { x0: x + i as f32 * 0.004, y0: y1 - 0.016, x1: x + 0.11 - i as f32 * 0.006, y1, c: c(*cc) });
    }
}

fn prims(scene: &str, light: bool) -> Vec<Prim> {
    use Prim::*;
    let mut p = vec![];
    match scene {
        "night" => {
            room(&mut p, 0x1B1E3A, 0x0C0E1E, 0x16214D, 0x3A3A78, 0x272B5E, 0x17122A);
            p.push(Glow { x: 0.78, y: 0.2, r: 0.18, c: ca(0xFFF1C1, 0.25) });
            p.push(Circle { x: 0.78, y: 0.2, r: 0.045, c: c(0xFFF3CF) });
            city(&mut p, 0x121633, 0xFFD27A, 0.53, 0.9, 0.66, 7);
            p.push(Rect { x0: 0.705, y0: 0.07, x1: 0.715, y1: 0.66, c: c(0x0B0D1C) });
            p.push(Rect { x0: 0.5, y0: 0.36, x1: 0.93, y1: 0.37, c: c(0x0B0D1C) });
            lamp(&mut p, 0.16, 0xFFB45A);
            plant(&mut p, 0.36, 0x1E3B2E);
            books(&mut p, 0.22);
        }
        "rain" => {
            room(&mut p, 0x1C2733, 0x0D131A, 0x3B4B5C, 0x27323F, 0x2B3A48, 0x161C22);
            city(&mut p, 0x1A232D, 0xE8C878, 0.53, 0.9, 0.66, 3);
            p.push(Rect { x0: 0.705, y0: 0.07, x1: 0.715, y1: 0.66, c: c(0x0E141A) });
            p.push(Rect { x0: 0.5, y0: 0.36, x1: 0.93, y1: 0.37, c: c(0x0E141A) });
            p.push(Rect { x0: 0.5, y0: 0.07, x1: 0.93, y1: 0.66, c: ca(0x8FA6BD, 0.08) });
            lamp(&mut p, 0.16, 0xFFC27A);
            plant(&mut p, 0.36, 0x23402F);
            books(&mut p, 0.22);
        }
        "sunset" => {
            p.push(VGrad { x0: 0.0, y0: 0.0, x1: 1.0, y1: 0.62, top: c(0x5B3A8C), bot: c(0xFF9E6B) });
            p.push(VGrad { x0: 0.0, y0: 0.62, x1: 1.0, y1: 1.0, top: c(0xF28A6B), bot: c(0x3A1E45) });
            p.push(Glow { x: 0.68, y: 0.56, r: 0.45, c: ca(0xFFD9A0, 0.5) });
            p.push(Circle { x: 0.68, y: 0.56, r: 0.09, c: c(0xFFE3B0) });
            p.push(Circle { x: 0.15, y: 1.35, r: 0.75, c: c(0x7A3F6E) });
            p.push(Circle { x: 0.85, y: 1.45, r: 0.8, c: c(0x5E2F5C) });
            p.push(Circle { x: 0.45, y: 1.6, r: 0.85, c: c(0x3E1F45) });
            p.push(Circle { x: 0.0, y: 1.75, r: 0.9, c: c(0x2A1533) });
        }
        "forest" => {
            p.push(VGrad { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0, top: c(0xBFE3D2), bot: c(0x2F5D4E) });
            p.push(Glow { x: 0.3, y: 0.25, r: 0.5, c: ca(0xFFF4D6, 0.55) });
            for (layer, (color, base, h)) in [(0x7FA898, 0.6f32, 0.28f32), (0x4F7F6C, 0.72, 0.32), (0x2F5A4A, 0.86, 0.36), (0x1C3B30, 1.02, 0.42)]
                .iter()
                .enumerate()
            {
                let n = 9 + layer * 3;
                for i in 0..=n {
                    let x = i as f32 / n as f32 + (layer as f32 * 0.037) % 0.05;
                    let hh = h * (0.7 + 0.3 * (((i * 7 + layer * 3) % 5) as f32 / 4.0));
                    p.push(Poly { pts: vec![(x - 0.045, *base), (x, base - hh), (x + 0.045, *base)], c: c(*color) });
                }
                p.push(Rect { x0: 0.0, y0: *base - 0.01, x1: 1.0, y1: 1.0, c: c(*color) });
                p.push(Rect { x0: 0.0, y0: base - 0.08, x1: 1.0, y1: *base, c: ca(0xDDEFE6, 0.12) });
            }
        }
        "aurora" => {
            p.push(VGrad { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0, top: c(0x050B1E), bot: c(0x0B1430) });
            p.push(Glow { x: 0.25, y: 0.3, r: 0.6, c: ca(0x2DD4BF, 0.35) });
            p.push(Glow { x: 0.7, y: 0.2, r: 0.55, c: ca(0x8B5CF6, 0.35) });
            p.push(Glow { x: 0.55, y: 0.7, r: 0.6, c: ca(0x38BDF8, 0.22) });
            p.push(Glow { x: 0.9, y: 0.85, r: 0.5, c: ca(0xF472B6, 0.18) });
        }
        _ => {
            let (a, b) = if light { (0xF4F1EC, 0xE3DED6) } else { (0x14161D, 0x0B0C10) };
            p.push(VGrad { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0, top: c(a), bot: c(b) });
        }
    }
    p
}

// ---------------------------------------------------------------- painting (sharp)

fn to_screen(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.left() + x * r.width(), r.top() + y * r.height())
}

fn grad_mesh(rect: Rect, top: Color32, bot: Color32) -> Shape {
    let mut m = Mesh::default();
    for (p, c) in [(rect.left_top(), top), (rect.right_top(), top), (rect.right_bottom(), bot), (rect.left_bottom(), bot)] {
        m.vertices.push(Vertex { pos: p, uv: egui::epaint::WHITE_UV, color: c });
    }
    m.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    Shape::mesh(m)
}

pub fn glow(painter: &Painter, center: Pos2, r: f32, color: Color32) {
    let mut m = Mesh::default();
    m.vertices.push(Vertex { pos: center, uv: egui::epaint::WHITE_UV, color });
    let n = 48;
    let rings = [(0.45f32, 0.55f32), (1.0, 0.0)];
    // center, then a mid ring and an outer ring (smoother falloff than a single ring)
    for (k, a) in rings {
        for i in 0..n {
            let t = i as f32 / n as f32 * PI * 2.0;
            let [cr, cg, cb, ca] = color.to_array();
            let _ = (cr, cg, cb);
            let col = Color32::from_rgba_premultiplied(
                (cr as f32 * a) as u8,
                (cg as f32 * a) as u8,
                (cb as f32 * a) as u8,
                (ca as f32 * a) as u8,
            );
            m.vertices.push(Vertex { pos: pos2(center.x + t.cos() * r * k, center.y + t.sin() * r * k), uv: egui::epaint::WHITE_UV, color: col });
        }
    }
    let n = n as u32;
    for i in 0..n {
        let j = (i + 1) % n;
        m.indices.extend_from_slice(&[0, 1 + i, 1 + j]);
        m.indices.extend_from_slice(&[1 + i, 1 + n + i, 1 + n + j, 1 + i, 1 + n + j, 1 + j]);
    }
    painter.add(Shape::mesh(m));
}

pub fn paint(painter: &Painter, r: Rect, scene: &str, light: bool, t: f64) {
    let h = r.height();
    for p in prims(scene, light) {
        match p {
            Prim::VGrad { x0, y0, x1, y1, top, bot } => {
                painter.add(grad_mesh(Rect::from_min_max(to_screen(r, x0, y0), to_screen(r, x1, y1)), col(top), col(bot)));
            }
            Prim::Rect { x0, y0, x1, y1, c } => {
                painter.rect_filled(Rect::from_min_max(to_screen(r, x0, y0), to_screen(r, x1, y1)), 0.0, col(c));
            }
            Prim::Circle { x, y, r: rr, c } => {
                painter.circle_filled(to_screen(r, x, y), rr * h, col(c));
            }
            Prim::Glow { x, y, r: rr, c } => glow(painter, to_screen(r, x, y), rr * h, col(c)),
            Prim::Poly { pts, c } => {
                painter.add(Shape::convex_polygon(pts.iter().map(|(x, y)| to_screen(r, *x, *y)).collect(), col(c), Stroke::NONE));
            }
        }
    }
    extras(painter, r, scene, t);
}

/// Small moving details that don't need to appear in the blurred copy.
fn extras(painter: &Painter, r: Rect, scene: &str, t: f64) {
    let win = Rect::from_min_max(to_screen(r, 0.5, 0.07), to_screen(r, 0.93, 0.5));
    let hash = |i: u32| {
        let mut x = i.wrapping_mul(2654435761);
        x ^= x >> 13;
        x = x.wrapping_mul(1274126177);
        (x & 0xFFFF) as f32 / 65535.0
    };
    match scene {
        "night" => {
            for i in 0..40 {
                let p = pos2(win.left() + hash(i) * win.width(), win.top() + hash(i + 99) * win.height() * 0.8);
                if (p.x - to_screen(r, 0.78, 0.2).x).abs() < r.height() * 0.07 && (p.y - to_screen(r, 0.78, 0.2).y).abs() < r.height() * 0.07 {
                    continue;
                }
                let tw = 0.4 + 0.6 * ((t * (0.6 + hash(i + 7) as f64) + hash(i + 3) as f64 * 6.0).sin() * 0.5 + 0.5) as f32;
                painter.circle_filled(p, 0.6 + hash(i + 5) * 1.0, Color32::from_white_alpha((tw * 200.0) as u8));
            }
        }
        "rain" => {
            let win = Rect::from_min_max(to_screen(r, 0.5, 0.07), to_screen(r, 0.93, 0.66));
            let painter = painter.with_clip_rect(win);
            for i in 0..70 {
                let speed = 0.7 + hash(i) * 0.6;
                let y = ((t as f32 * speed + hash(i + 50)) % 1.0) * (win.height() + 40.0) + win.top() - 20.0;
                let x = win.left() + hash(i + 11) * win.width();
                painter.line_segment([pos2(x, y), pos2(x - 2.0, y + 14.0)], Stroke::new(1.0, Color32::from_white_alpha(50)));
            }
        }
        "forest" => {
            for i in 0..18 {
                let x = r.left() + (hash(i) * r.width() + (t as f32 * 8.0 * (hash(i + 3) - 0.5))).rem_euclid(r.width());
                let y = r.top() + r.height() * (0.55 + hash(i + 9) * 0.4) + (t as f32 * 0.8 + i as f32).sin() * 8.0;
                let a = ((t * 1.5 + i as f64).sin() * 0.5 + 0.5) as f32;
                glow(painter, pos2(x, y), 7.0, Color32::from_rgba_unmultiplied(255, 240, 150, (a * 140.0) as u8));
            }
        }
        "aurora" => {
            for (i, color) in [(0u32, Color32::from_rgba_unmultiplied(45, 212, 191, 40)), (1, Color32::from_rgba_unmultiplied(139, 92, 246, 40))] {
                let x = r.left() + r.width() * (0.3 + 0.4 * ((t * 0.05 + i as f64 * 2.0).sin() * 0.5 + 0.5) as f32);
                glow(painter, pos2(x, r.top() + r.height() * (0.3 + 0.2 * i as f32)), r.height() * 0.5, color);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------- blurred copy (CPU)

fn inside_poly(pts: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut inside = false;
    let n = pts.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = pts[i];
        let (xj, yj) = pts[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn blend(dst: &mut [f32; 3], src: [u8; 4], a_mul: f32) {
    let a = src[3] as f32 / 255.0 * a_mul;
    for k in 0..3 {
        dst[k] = dst[k] * (1.0 - a) + src[k] as f32 * a;
    }
}

/// Low-resolution, blurred raster of the scene for frosted glass.
pub fn blurred(scene: &str, light: bool, w: usize, h: usize, aspect: f32) -> ColorImage {
    let prims = prims(scene, light);
    let mut px = vec![[0f32; 3]; w * h];
    for yy in 0..h {
        for xx in 0..w {
            let (x, y) = ((xx as f32 + 0.5) / w as f32, (yy as f32 + 0.5) / h as f32);
            let d = &mut px[yy * w + xx];
            for p in &prims {
                match p {
                    Prim::VGrad { x0, y0, x1, y1, top, bot } => {
                        if x >= *x0 && x <= *x1 && y >= *y0 && y <= *y1 {
                            let k = (y - y0) / (y1 - y0).max(1e-4);
                            let c = [0, 1, 2, 3].map(|i| (top[i] as f32 * (1.0 - k) + bot[i] as f32 * k) as u8);
                            blend(d, c, 1.0);
                        }
                    }
                    Prim::Rect { x0, y0, x1, y1, c } => {
                        if x >= *x0 && x <= *x1 && y >= *y0 && y <= *y1 {
                            blend(d, *c, 1.0);
                        }
                    }
                    Prim::Circle { x: cx, y: cy, r, c } => {
                        let dx = (x - cx) * aspect;
                        if dx * dx + (y - cy) * (y - cy) <= r * r {
                            blend(d, *c, 1.0);
                        }
                    }
                    Prim::Glow { x: cx, y: cy, r, c } => {
                        let dx = (x - cx) * aspect;
                        let dist = (dx * dx + (y - cy) * (y - cy)).sqrt() / r;
                        if dist < 1.0 {
                            blend(d, *c, (1.0 - dist).powf(1.6));
                        }
                    }
                    Prim::Poly { pts, c } => {
                        if inside_poly(pts, x, y) {
                            blend(d, *c, 1.0);
                        }
                    }
                }
            }
        }
    }
    // Two box-blur passes.
    for _ in 0..2 {
        px = box_blur(&px, w, h, 2);
    }
    let mut img = ColorImage::new([w, h], vec![Color32::BLACK; w * h]);
    // Vibrancy: glass makes what's behind it a little more saturated and luminous.
    for (i, p) in px.iter().enumerate() {
        let l = 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2];
        let v = p.map(|c| (l + (c - l) * 1.45) * 1.06 + 4.0);
        img.pixels[i] = Color32::from_rgb(v[0].clamp(0.0, 255.0) as u8, v[1].clamp(0.0, 255.0) as u8, v[2].clamp(0.0, 255.0) as u8);
    }
    img
}

fn box_blur(src: &[[f32; 3]], w: usize, h: usize, r: i32) -> Vec<[f32; 3]> {
    let mut tmp = vec![[0f32; 3]; w * h];
    let mut out = vec![[0f32; 3]; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut s = [0f32; 3];
            let mut n = 0.0;
            for dx in -r..=r {
                let xx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                for k in 0..3 {
                    s[k] += src[y * w + xx][k];
                }
                n += 1.0;
            }
            tmp[y * w + x] = s.map(|v| v / n);
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut s = [0f32; 3];
            let mut n = 0.0;
            for dy in -r..=r {
                let yy = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                for k in 0..3 {
                    s[k] += tmp[yy * w + x][k];
                }
                n += 1.0;
            }
            out[y * w + x] = s.map(|v| v / n);
        }
    }
    out
}

/// Keeps the blurred texture in sync with the scene and window size.
#[derive(Default)]
pub struct Backdrop {
    key: (String, bool, u32, u32),
    pub tex: Option<TextureHandle>,
}

impl Backdrop {
    pub fn update(&mut self, ctx: &egui::Context, scene: &str, light: bool, size: egui::Vec2) {
        let key = (scene.to_string(), light, (size.x / 40.0) as u32, (size.y / 40.0) as u32);
        if self.tex.is_some() && key == self.key {
            return;
        }
        let w = 96usize;
        let h = ((w as f32) * size.y / size.x.max(1.0)).round().clamp(24.0, 128.0) as usize;
        let img = blurred(scene, light, w, h, size.x / size.y.max(1.0));
        match &mut self.tex {
            Some(t) => t.set(img, TextureOptions::LINEAR),
            None => self.tex = Some(ctx.load_texture("studytracker.backdrop", img, TextureOptions::LINEAR)),
        }
        self.key = key;
    }
}

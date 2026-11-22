//! Meng as a little cat that lives along the bottom of the window: walks, sits, grooms, stretches,
//! naps, plays with a ball of yarn, looks at the cursor, and hops when petted.
//! What it does depends on what you do: quiet while you focus, playful on breaks.

use crate::mascot::{self, Mood};
use crate::theme::rgb;
use eframe::egui::{self, pos2, vec2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};
use std::f32::consts::PI;

const FUR: u32 = 0xFFA552;
const FUR_DARK: u32 = 0xE58A3A;
const BELLY: u32 = 0xFFE3C2;
const YARN: u32 = 0xF472B6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    Sit,
    Walk,
    Groom,
    Stretch,
    Sleep,
    Play,
    Look,
    Hop,
}

impl Act {
    pub fn label(self) -> &'static str {
        match self {
            Act::Sit => "lounging",
            Act::Walk => "going for a stroll",
            Act::Groom => "grooming",
            Act::Stretch => "stretching",
            Act::Sleep => "napping",
            Act::Play => "playing with yarn",
            Act::Look => "watching you",
            Act::Hop => "enjoying the pets",
        }
    }
}

/// What the user is doing, which shapes Meng's mood.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Focus,
    Break,
    Idle,
}

pub struct Cat {
    pub x: f32,
    facing: f32,
    pub act: Act,
    started: f64,
    until: f64,
    target: f32,
    yarn: f32,
    yarn_spin: f32,
    seed: u64,
    hearts: Vec<(Pos2, f64)>,
    last_t: f64,
    pub bubble: Option<(String, f64)>,
}

impl Default for Cat {
    fn default() -> Self {
        Self {
            x: f32::NAN,
            facing: 1.0,
            act: Act::Sit,
            started: 0.0,
            until: 3.0,
            target: 0.0,
            yarn: f32::NAN,
            yarn_spin: 0.0,
            seed: 0x9E3779B97F4A7C15,
            hearts: vec![],
            last_t: 0.0,
            bubble: None,
        }
    }
}

impl Cat {
    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 40) as f32 / (1u64 << 24) as f32
    }

    fn pick(&mut self, ctx: Context) -> Act {
        let table: &[(Act, f32)] = match ctx {
            Context::Focus => &[(Act::Sleep, 6.0), (Act::Sit, 2.0), (Act::Groom, 1.0), (Act::Walk, 0.6)],
            Context::Break => &[(Act::Play, 4.0), (Act::Walk, 3.0), (Act::Groom, 2.0), (Act::Stretch, 2.0), (Act::Sit, 1.0)],
            Context::Idle => &[(Act::Walk, 3.0), (Act::Sit, 3.0), (Act::Groom, 2.0), (Act::Sleep, 2.0), (Act::Stretch, 1.2), (Act::Play, 1.2)],
        };
        let total: f32 = table.iter().map(|a| a.1).sum();
        let mut r = self.rand() * total;
        for (a, w) in table {
            if r < *w {
                return *a;
            }
            r -= w;
        }
        Act::Sit
    }

    fn start(&mut self, act: Act, t: f64, min_x: f32, max_x: f32) {
        self.act = act;
        self.started = t;
        let r = self.rand() as f64;
        self.until = t + match act {
            Act::Sleep => 25.0 + r * 35.0,
            Act::Sit => 6.0 + r * 8.0,
            Act::Groom => 5.0 + r * 4.0,
            Act::Stretch => 2.6,
            Act::Play => 7.0 + r * 5.0,
            Act::Look => 3.0,
            Act::Hop => 1.2,
            Act::Walk => 30.0, // ends on arrival
        };
        if act == Act::Walk {
            let span = (max_x - min_x).max(1.0);
            let mut tx = min_x + self.rand() * span;
            if (tx - self.x).abs() < span * 0.15 {
                tx = if self.x > min_x + span / 2.0 { min_x + span * 0.2 } else { max_x - span * 0.2 };
            }
            self.target = tx;
            self.facing = if tx > self.x { 1.0 } else { -1.0 };
        }
        if act == Act::Play {
            let side = if self.x > (min_x + max_x) / 2.0 { -1.0 } else { 1.0 };
            self.facing = side;
            self.yarn = (self.x + side * 90.0).clamp(min_x, max_x);
        }
    }

    /// Advances Meng. `floor` is the strip it lives on (its bottom edge is the ground).
    pub fn update(&mut self, t: f64, floor: Rect, ctx: Context, pointer: Option<Pos2>, size: f32) {
        let dt = ((t - self.last_t) as f32).clamp(0.0, 0.1);
        self.last_t = t;
        let (min_x, max_x) = (floor.left() + size * 0.8, floor.right() - size * 0.8);
        if self.x.is_nan() {
            self.x = min_x + (max_x - min_x) * 0.75;
        }
        self.x = self.x.clamp(min_x, max_x.max(min_x));
        self.hearts.retain(|h| t - h.1 < 1.6);
        if self.bubble.as_ref().is_some_and(|b| t > b.1) {
            self.bubble = None;
        }

        // Curious about the cursor when it comes close (unless asleep).
        if let Some(p) = pointer {
            let near = (p.x - self.x).abs() < size * 2.2 && (p.y - floor.bottom()).abs() < size * 2.0;
            if near && !matches!(self.act, Act::Sleep | Act::Hop | Act::Look | Act::Play) {
                self.start(Act::Look, t, min_x, max_x);
            }
            if self.act == Act::Look {
                self.facing = if p.x >= self.x { 1.0 } else { -1.0 };
                if near {
                    self.until = self.until.max(t + 1.0);
                }
            }
        }

        match self.act {
            Act::Walk => {
                let speed = size * 0.9;
                let d = self.target - self.x;
                if d.abs() < 2.0 {
                    let next = self.pick(ctx);
                    let next = if next == Act::Walk { Act::Sit } else { next };
                    self.start(next, t, min_x, max_x);
                } else {
                    self.x += d.signum() * (speed * dt).min(d.abs());
                    self.facing = d.signum();
                }
            }
            Act::Play => {
                if self.yarn.is_nan() {
                    self.yarn = (self.x + self.facing * size * 1.4).clamp(min_x, max_x);
                }
                // Bat the yarn: when close, swipe it forward; walk after it.
                let d = self.yarn - self.x;
                let reach = size * 0.75;
                if d.abs() > reach {
                    self.x += d.signum() * (size * 1.4 * dt).min(d.abs() - reach);
                    self.facing = d.signum();
                } else if self.rand() < dt * 1.6 {
                    let push = self.facing * size * (0.6 + self.rand() * 1.2);
                    self.yarn = (self.yarn + push).clamp(min_x, max_x);
                    if self.yarn <= min_x + 1.0 || self.yarn >= max_x - 1.0 {
                        self.facing = -self.facing;
                    }
                }
                self.yarn_spin += (self.yarn - self.x).signum() * dt * 3.0;
                if t > self.until {
                    let next = self.pick(ctx);
                    self.start(next, t, min_x, max_x);
                }
            }
            _ => {
                if t > self.until {
                    let mut next = self.pick(ctx);
                    if next == self.act && next != Act::Sleep {
                        next = Act::Walk;
                    }
                    // Waking up from a nap: stretch first, like real cats.
                    if self.act == Act::Sleep && next != Act::Sleep {
                        next = Act::Stretch;
                    }
                    self.start(next, t, min_x, max_x);
                }
            }
        }
        // Focus started while playing: settle down soon.
        if ctx == Context::Focus && matches!(self.act, Act::Play) && t - self.started > 2.0 {
            self.start(Act::Sit, t, min_x, max_x);
        }
    }

    /// Development: hold one activity (STUDYTRACKER_DEMO_CAT) so screenshots can show it.
    pub fn hold(&mut self, act: Act) {
        self.act = act;
        self.until = f64::MAX;
    }

    pub fn pet(&mut self, t: f64, floor: Rect, size: f32) {
        let head = self.head_pos(floor.bottom(), size, t);
        for i in 0..3 {
            self.hearts.push((head + vec2((i as f32 - 1.0) * size * 0.25, -size * 0.2), t + i as f64 * 0.15));
        }
        let (min_x, max_x) = (floor.left() + size * 0.8, floor.right() - size * 0.8);
        self.start(Act::Hop, t, min_x, max_x);
    }

    pub fn say(&mut self, text: impl Into<String>, t: f64, secs: f64) {
        self.bubble = Some((text.into(), t + secs));
    }

    /// Screen rect Meng occupies (for clicks).
    pub fn rect(&self, ground: f32, size: f32) -> Rect {
        Rect::from_center_size(pos2(self.x, ground - size * 0.55), vec2(size * 1.6, size * 1.15))
    }

    fn head_pos(&self, ground: f32, size: f32, t: f64) -> Pos2 {
        let (dx, dy) = match self.act {
            Act::Walk | Act::Hop => (0.42, 0.8),
            Act::Sit | Act::Look => (0.1, 0.84),
            Act::Groom => (0.13, 0.78),
            Act::Sleep => (0.38, 0.34),
            Act::Stretch => (0.5, 0.36),
            Act::Play => (0.42, 0.5),
        };
        let hop = if self.act == Act::Hop { hop_height(t - self.started) } else { 0.0 };
        pos2(self.x + self.facing * dx * size, ground - (dy + hop) * size)
    }

    pub fn draw(&self, p: &Painter, ground: f32, size: f32, t: f64, text_color: Color32, bubble_bg: Color32) {
        let f = self.facing;
        let s = size;
        let age = t - self.started;
        let hop = if self.act == Act::Hop { hop_height(age) } else { 0.0 };
        let x0 = self.x;
        let pt = |dx: f32, dy: f32| pos2(x0 + f * dx * s, ground - (dy + hop) * s);
        let fur = rgb(FUR);
        let dark = rgb(FUR_DARK);
        let lw = s * 0.11;

        // Ground shadow.
        let sh_w = if self.act == Act::Sleep { 0.55 } else { 0.45 } * s * (1.0 - hop * 0.6);
        ellipse(p, pos2(x0, ground - 1.0), sh_w, s * 0.05, 0.0, Color32::from_black_alpha(50));

        if self.act == Act::Play {
            yarn(p, pos2(self.yarn, ground - s * 0.13), s * 0.13, self.yarn_spin);
        }

        let sway = (t * 2.2).sin() as f32;
        let mood;
        match self.act {
            Act::Walk | Act::Hop => {
                let ph = (t * 9.0) as f32;
                let walking = self.act == Act::Walk;
                let bob = if walking { (ph * 2.0).sin().abs() * 0.02 } else { 0.0 };
                tail(p, &[pt(-0.38, 0.5), pt(-0.56, 0.62 + sway * 0.03), pt(-0.64, 0.82), pt(-0.58 + sway * 0.05, 0.98)], lw, fur, dark);
                for (i, lx) in [-0.27f32, -0.14, 0.2, 0.32].iter().enumerate() {
                    let k = if walking { (ph + i as f32 * PI * 0.5 + if i % 2 == 0 { 0.0 } else { PI }).sin() } else { 0.0 };
                    let near_side = i % 2 == 1;
                    let c = if near_side { fur } else { dark };
                    limb(p, pt(*lx, 0.34), pt(lx + k * 0.07, (k.max(0.0) * 0.05).max(0.0) + 0.02), lw, c);
                }
                ellipse(p, pt(0.0, 0.44 + bob), 0.42 * s, 0.22 * s, 0.0, fur);
                ellipse(p, pt(0.08, 0.36 + bob), 0.26 * s, 0.1 * s, 0.0, rgb(BELLY));
                stripes(p, pt(-0.12, 0.62 + bob), s, f);
                mood = if self.act == Act::Hop { Mood::Celebrate } else { Mood::Normal };
            }
            Act::Sit | Act::Look | Act::Groom => {
                let tip = (t * 1.6).sin() as f32 * 0.05;
                tail(p, &[pt(-0.24, 0.08), pt(-0.02, 0.02), pt(0.22, 0.04), pt(0.34 + tip, 0.1 + tip.abs())], lw, fur, dark);
                ellipse(p, pt(-0.12, 0.2), 0.22 * s, 0.2 * s, 0.0, fur);
                ellipse(p, pt(0.0, 0.38), 0.24 * s, 0.33 * s, 0.15 * f, fur);
                ellipse(p, pt(0.08, 0.34), 0.12 * s, 0.22 * s, 0.1 * f, rgb(BELLY));
                limb(p, pt(0.12, 0.38), pt(0.14, 0.03), lw, dark);
                if self.act == Act::Groom {
                    let lick = (t * 7.0).sin() as f32 * 0.03;
                    limb(p, pt(0.2, 0.42), pt(0.24, 0.66 + lick), lw, fur);
                } else {
                    limb(p, pt(0.21, 0.38), pt(0.23, 0.03), lw, fur);
                }
                stripes(p, pt(-0.1, 0.58), s, f);
                mood = if self.act == Act::Groom { Mood::Happy } else { Mood::Normal };
            }
            Act::Sleep => {
                let breath = 1.0 + (t * 1.8).sin() as f32 * 0.035;
                tail(p, &[pt(-0.42, 0.14), pt(-0.34, 0.03), pt(-0.02, 0.0), pt(0.28, 0.04)], lw, fur, dark);
                ellipse(p, pt(0.0, 0.2), 0.46 * s, 0.21 * s * breath, 0.0, fur);
                stripes(p, pt(-0.12, 0.36), s, f);
                mood = Mood::Sleepy;
            }
            Act::Stretch => {
                let k = ((age / 2.6) as f32).clamp(0.0, 1.0);
                let stretch = (k * PI).sin();
                tail(p, &[pt(-0.42, 0.5), pt(-0.56, 0.72), pt(-0.5, 0.94), pt(-0.4 + sway * 0.04, 1.04)], lw, fur, dark);
                limb(p, pt(-0.3, 0.4), pt(-0.32, 0.02), lw, dark);
                limb(p, pt(-0.18, 0.38), pt(-0.2, 0.02), lw, fur);
                limb(p, pt(0.22, 0.24), pt(0.42 + 0.2 * stretch, 0.02), lw, dark);
                limb(p, pt(0.3, 0.24), pt(0.52 + 0.2 * stretch, 0.02), lw, fur);
                ellipse(p, pt(0.0, 0.34 - 0.08 * stretch), 0.46 * s, 0.19 * s, -0.3 * stretch * f, fur);
                stripes(p, pt(-0.14, 0.46), s, f);
                mood = Mood::Happy;
            }
            Act::Play => {
                let wiggle = (t * 16.0).sin() as f32 * 0.02;
                tail(p, &[pt(-0.38 + wiggle, 0.36), pt(-0.56, 0.42), pt(-0.66, 0.56 + sway * 0.06), pt(-0.6, 0.7)], lw, fur, dark);
                limb(p, pt(-0.24 + wiggle, 0.26), pt(-0.26, 0.02), lw, dark);
                limb(p, pt(-0.12 + wiggle, 0.26), pt(-0.12, 0.02), lw, fur);
                let swipe = ((t * 5.0).sin() as f32).max(0.0) * 0.12;
                limb(p, pt(0.22, 0.24), pt(0.34 + swipe, 0.02 + swipe * 0.6), lw, dark);
                limb(p, pt(0.3, 0.24), pt(0.4, 0.02), lw, fur);
                ellipse(p, pt(0.0 + wiggle, 0.3), 0.42 * s, 0.18 * s, 0.08 * f, fur);
                stripes(p, pt(-0.12 + wiggle, 0.44), s, f);
                mood = Mood::Focus;
            }
        }
        let head = self.head_pos(ground, s, t);
        mascot::draw(p, head - vec2(0.0, s * 0.04), s * 0.66, mood, t);

        // Hearts when petted.
        for (h, born) in &self.hearts {
            let age = (t - born) as f32;
            if age < 0.0 {
                continue;
            }
            let pos = *h - vec2((age * 3.0).sin() * 4.0, age * s * 0.6);
            heart(p, pos, s * 0.09, Color32::from_rgba_unmultiplied(255, 90, 130, ((1.0 - age / 1.6) * 255.0) as u8));
        }

        // Speech bubble.
        if let Some((text, _)) = &self.bubble {
            let font = FontId::new(12.5, egui::FontFamily::Name("semibold".into()));
            let galley = p.layout(text.clone(), font, text_color, 230.0);
            let size = galley.size() + vec2(24.0, 16.0);
            let mut r = Rect::from_center_size(head - vec2(0.0, s * 0.42 + size.y / 2.0), size);
            let clip = p.clip_rect();
            let shift = (clip.left() + 4.0 - r.left()).max(0.0) - (r.right() - clip.right() + 4.0).max(0.0);
            r = r.translate(vec2(shift, 0.0));
            p.rect_filled(r, 12.0, bubble_bg);
            p.rect_stroke(r, 12.0, Stroke::new(1.0, Color32::from_white_alpha(40)), egui::StrokeKind::Inside);
            let tx = head.x.clamp(r.left() + 14.0, r.right() - 14.0);
            p.add(Shape::convex_polygon(vec![pos2(tx - 7.0, r.bottom() - 1.0), pos2(tx + 7.0, r.bottom() - 1.0), pos2(tx, r.bottom() + 7.0)], bubble_bg, Stroke::NONE));
            p.galley(r.min + vec2(12.0, 8.0), galley, text_color);
        }
    }
}

fn hop_height(age: f64) -> f32 {
    let k = (age / 0.5).fract() as f32;
    if age > 1.0 { 0.0 } else { (k * PI).sin() * 0.35 }
}

pub fn ellipse(p: &Painter, c: Pos2, rx: f32, ry: f32, rot: f32, color: Color32) {
    let (sn, cs) = rot.sin_cos();
    let pts: Vec<Pos2> = (0..36)
        .map(|i| {
            let a = i as f32 / 36.0 * PI * 2.0;
            let (x, y) = (a.cos() * rx, a.sin() * ry);
            pos2(c.x + x * cs - y * sn, c.y + x * sn + y * cs)
        })
        .collect();
    p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

fn limb(p: &Painter, a: Pos2, b: Pos2, w: f32, c: Color32) {
    p.line_segment([a, b], Stroke::new(w, c));
    p.circle_filled(a, w / 2.0, c);
    p.circle_filled(b, w / 2.0, c);
}

/// Smooth tail through control points (Catmull-Rom), drawn as overlapping discs so it has round ends.
fn tail(p: &Painter, pts: &[Pos2], w: f32, c: Color32, tip: Color32) {
    let n = pts.len();
    let get = |i: isize| pts[i.clamp(0, n as isize - 1) as usize];
    let steps = 10;
    let total = (n - 1) * steps;
    for seg in 0..n - 1 {
        let (p0, p1, p2, p3) = (get(seg as isize - 1), get(seg as isize), get(seg as isize + 1), get(seg as isize + 2));
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let q = |a: f32, b: f32, c2: f32, d: f32| 0.5 * (2.0 * b + (-a + c2) * t + (2.0 * a - 5.0 * b + 4.0 * c2 - d) * t2 + (-a + 3.0 * b - 3.0 * c2 + d) * t3);
            let pos = pos2(q(p0.x, p1.x, p2.x, p3.x), q(p0.y, p1.y, p2.y, p3.y));
            let idx = seg * steps + k;
            let col = if idx > total * 4 / 5 { tip } else { c };
            p.circle_filled(pos, w * (0.5 - 0.1 * idx as f32 / total as f32), col);
        }
    }
}

fn stripes(p: &Painter, c: Pos2, s: f32, f: f32) {
    let st = Stroke::new(s * 0.045, rgb(FUR_DARK));
    for i in 0..3 {
        let x = c.x + f * (i as f32 * 0.1 - 0.1) * s;
        p.line_segment([pos2(x, c.y - s * 0.03), pos2(x - f * s * 0.03, c.y + s * 0.07)], st);
    }
}

fn yarn(p: &Painter, c: Pos2, r: f32, spin: f32) {
    p.circle_filled(c, r, rgb(YARN));
    let st = Stroke::new(1.2, Color32::from_rgba_unmultiplied(255, 255, 255, 90));
    for i in 0..3 {
        let a = spin + i as f32 * PI / 3.0;
        let d = vec2(a.cos(), a.sin()) * r * 0.85;
        p.line_segment([c - d, c + d], st);
    }
    p.add(Shape::line(
        vec![c + vec2(r * 0.7, r * 0.6), c + vec2(r * 1.6, r * 0.9), c + vec2(r * 2.4, r * 0.95)],
        Stroke::new(1.2, rgb(YARN)),
    ));
}

fn heart(p: &Painter, c: Pos2, r: f32, col: Color32) {
    p.circle_filled(c + vec2(-r * 0.5, 0.0), r * 0.55, col);
    p.circle_filled(c + vec2(r * 0.5, 0.0), r * 0.55, col);
    p.add(Shape::convex_polygon(vec![c + vec2(-r * 1.02, r * 0.15), c + vec2(r * 1.02, r * 0.15), c + vec2(0.0, r * 1.2)], col, Stroke::NONE));
}

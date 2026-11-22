//! Meng, the orange cat: mascot, app icon, and study assistant.
//! Drawn with vectors so it looks sharp at any size and needs no image files.

use crate::store::{self, Data};
use crate::theme::{alpha, rgb};
use crate::timer::{fmt_dur, Timer};
use eframe::egui::{self, pos2, vec2, Color32, Painter, Pos2, Shape, Stroke};
use std::f32::consts::PI;

const FUR: u32 = 0xFFA552;
const FUR_DARK: u32 = 0xE07F2E;
const EAR_IN: u32 = 0xFF9EAA;
const MUZZLE: u32 = 0xFFEBD2;
const EYE: u32 = 0x2B2230;
const NOSE: u32 = 0xFF6F86;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    Normal,
    Focus,
    Happy,
    Celebrate,
    Sleepy,
}

/// Draws Meng's head centered at `c`, fitting in a square of side `size`. `t` is time in seconds (animation).
pub fn draw(p: &Painter, c: Pos2, size: f32, mood: Mood, t: f64) {
    let bounce = match mood {
        Mood::Celebrate => ((t * 6.0).sin().abs() as f32) * -size * 0.06,
        Mood::Sleepy => ((t * 1.2).sin() as f32) * size * 0.012,
        _ => ((t * 2.0).sin() as f32) * size * 0.012,
    };
    let c = c + vec2(0.0, bounce + size * 0.06);
    let r = size * 0.36;
    let fur = rgb(FUR);
    let fur_dark = rgb(FUR_DARK);

    // Ears (outer, then inner).
    for side in [-1.0f32, 1.0] {
        let ear = [
            pos2(c.x + side * r * 0.98, c.y - r * 0.18),
            pos2(c.x + side * r * 0.80, c.y - r * 1.22),
            pos2(c.x + side * r * 0.18, c.y - r * 0.82),
        ];
        p.add(Shape::convex_polygon(ear.to_vec(), fur, Stroke::NONE));
        let cen = pos2((ear[0].x + ear[1].x + ear[2].x) / 3.0, (ear[0].y + ear[1].y + ear[2].y) / 3.0);
        let inner: Vec<Pos2> = ear.iter().map(|q| cen + (*q - cen) * 0.55 + vec2(0.0, r * 0.04)).collect();
        p.add(Shape::convex_polygon(inner, rgb(EAR_IN), Stroke::NONE));
    }

    // Head: a slightly wide ellipse built from a polygon.
    let head: Vec<Pos2> = (0..48)
        .map(|i| {
            let a = i as f32 / 48.0 * PI * 2.0;
            pos2(c.x + a.cos() * r * 1.08, c.y + a.sin() * r * 0.95)
        })
        .collect();
    p.add(Shape::convex_polygon(head, fur, Stroke::NONE));

    // Tabby stripes on the forehead.
    let st = Stroke::new(r * 0.07, fur_dark);
    for dx in [-0.22f32, 0.0, 0.22] {
        let len = if dx == 0.0 { 0.30 } else { 0.22 };
        p.line_segment([pos2(c.x + dx * r, c.y - r * 0.9), pos2(c.x + dx * r * 0.8, c.y - r * (0.9 - len))], st);
    }

    // Muzzle and cheeks.
    for side in [-1.0f32, 1.0] {
        p.circle_filled(pos2(c.x + side * r * 0.2, c.y + r * 0.38), r * 0.26, rgb(MUZZLE));
        p.circle_filled(pos2(c.x + side * r * 0.66, c.y + r * 0.3), r * 0.14, alpha(rgb(EAR_IN), 0.55));
    }

    // Eyes.
    let eye = rgb(EYE);
    let blink = mood == Mood::Normal && (t % 4.0) > 3.86;
    for side in [-1.0f32, 1.0] {
        let e = pos2(c.x + side * r * 0.42, c.y - r * 0.04);
        let er = r * 0.14;
        match mood {
            Mood::Happy | Mood::Celebrate => {
                // ^ ^
                let s = Stroke::new(r * 0.09, eye);
                p.line_segment([pos2(e.x - er, e.y + er * 0.5), pos2(e.x, e.y - er * 0.6)], s);
                p.line_segment([pos2(e.x, e.y - er * 0.6), pos2(e.x + er, e.y + er * 0.5)], s);
            }
            Mood::Sleepy => {
                let pts: Vec<Pos2> =
                    (0..=8).map(|i| { let a = i as f32 / 8.0 * PI; pos2(e.x - a.cos() * er, e.y + a.sin() * er * 0.45) }).collect();
                p.add(Shape::line(pts, Stroke::new(r * 0.08, eye)));
            }
            _ if blink => {
                p.line_segment([pos2(e.x - er, e.y), pos2(e.x + er, e.y)], Stroke::new(r * 0.08, eye));
            }
            Mood::Focus => {
                p.circle_filled(e, er, eye);
                // Lid: a fur-colored cap over the top of the eye gives a determined look.
                let lid = egui::Rect::from_min_max(pos2(e.x - er * 1.3, e.y - er * 1.4), pos2(e.x + er * 1.3, e.y - er * 0.25));
                p.rect_filled(lid, 0.0, fur);
                p.line_segment(
                    [pos2(e.x - er * 1.1, e.y - er * 0.25 - side * er * 0.25), pos2(e.x + er * 1.1, e.y - er * 0.25 + side * er * 0.25)],
                    Stroke::new(r * 0.06, eye),
                );
            }
            Mood::Normal => {
                p.circle_filled(e, er, eye);
                p.circle_filled(e + vec2(-er * 0.35, -er * 0.4), er * 0.35, Color32::WHITE);
            }
        }
    }

    // Nose and mouth.
    let n = pos2(c.x, c.y + r * 0.2);
    p.add(Shape::convex_polygon(
        vec![pos2(n.x - r * 0.09, n.y - r * 0.05), pos2(n.x + r * 0.09, n.y - r * 0.05), pos2(n.x, n.y + r * 0.06)],
        rgb(NOSE),
        Stroke::NONE,
    ));
    let ms = Stroke::new(r * 0.05, eye);
    let open = matches!(mood, Mood::Celebrate);
    for side in [-1.0f32, 1.0] {
        let pts: Vec<Pos2> = (0..=8)
            .map(|i| {
                let a = i as f32 / 8.0 * PI;
                pos2(n.x + side * r * 0.1 - side * a.cos() * r * 0.1, n.y + r * 0.1 + a.sin() * r * 0.09)
            })
            .collect();
        p.add(Shape::line(pts, ms));
    }
    if open {
        p.circle_filled(pos2(n.x, n.y + r * 0.27), r * 0.08, rgb(NOSE));
    }

    // Whiskers.
    let ws = Stroke::new((r * 0.03).max(0.8), alpha(rgb(EYE), 0.55));
    for side in [-1.0f32, 1.0] {
        for (dy, tilt) in [(0.28f32, -0.06f32), (0.4, 0.06)] {
            p.line_segment(
                [pos2(c.x + side * r * 0.5, c.y + r * dy), pos2(c.x + side * r * 1.18, c.y + r * (dy + tilt))],
                ws,
            );
        }
    }

    match mood {
        Mood::Celebrate => {
            for i in 0..6 {
                let a = i as f32 / 6.0 * PI * 2.0 + t as f32 * 1.5;
                let q = c + vec2(a.cos(), a.sin()) * r * 1.45;
                sparkle(p, q, r * 0.12, [0xFFC233, 0xFF4F79, 0x38BDF8][i % 3]);
            }
        }
        Mood::Sleepy => {
            let k = (t % 3.0) as f32 / 3.0;
            p.text(
                pos2(c.x + r * 1.0, c.y - r * (1.0 + k * 0.5)),
                egui::Align2::CENTER_CENTER,
                "z",
                egui::FontId::proportional(r * 0.45),
                alpha(rgb(0x8B91A1), 1.0 - k),
            );
        }
        _ => {}
    }
}

fn sparkle(p: &Painter, c: Pos2, s: f32, color: u32) {
    let col = rgb(color);
    let pts = vec![pos2(c.x, c.y - s), pos2(c.x + s * 0.3, c.y), pos2(c.x, c.y + s), pos2(c.x - s * 0.3, c.y)];
    p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
    let pts = vec![pos2(c.x - s, c.y), pos2(c.x, c.y - s * 0.3), pos2(c.x + s, c.y), pos2(c.x, c.y + s * 0.3)];
    p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
}

// ---------- App icon (rasterized in software, no image files) ----------

fn in_tri(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let s = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1);
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

/// Color of the icon at a point in 0..1 space (None = transparent).
fn icon_sample(x: f32, y: f32) -> Option<u32> {
    // Rounded square background.
    let (m, rad) = (0.02, 0.22);
    let qx = (x - 0.5).abs() - (0.5 - m - rad);
    let qy = (y - 0.5).abs() - (0.5 - m - rad);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
    if outside > 0.0 {
        return None;
    }
    let mut col = if y < 0.5 { 0x232838 } else { 0x1A1D29 };
    let (cx, cy, r) = (0.5, 0.56, 0.3);
    let ell = |ex: f32, ey: f32, rx: f32, ry: f32| ((x - ex) / rx).powi(2) + ((y - ey) / ry).powi(2) <= 1.0;
    for side in [-1.0f32, 1.0] {
        let a = (cx + side * r * 0.98, cy - r * 0.18);
        let b = (cx + side * r * 0.80, cy - r * 1.22);
        let c = (cx + side * r * 0.18, cy - r * 0.82);
        if in_tri((x, y), a, b, c) {
            col = FUR;
            let cen = ((a.0 + b.0 + c.0) / 3.0, (a.1 + b.1 + c.1) / 3.0 + r * 0.04);
            let sh = |q: (f32, f32)| (cen.0 + (q.0 - cen.0) * 0.55, cen.1 + (q.1 - cen.1) * 0.55);
            if in_tri((x, y), sh(a), sh(b), sh(c)) {
                col = EAR_IN;
            }
        }
    }
    if ell(cx, cy, r * 1.08, r * 0.95) {
        col = FUR;
        for dx in [-0.22f32, 0.0, 0.22] {
            let len = if dx == 0.0 { 0.3 } else { 0.22 };
            if (x - (cx + dx * r)).abs() < r * 0.04 && y > cy - r * 0.92 && y < cy - r * (0.9 - len) {
                col = FUR_DARK;
            }
        }
        for side in [-1.0f32, 1.0] {
            if ell(cx + side * r * 0.2, cy + r * 0.38, r * 0.26, r * 0.26) {
                col = MUZZLE;
            }
            if ell(cx + side * r * 0.42, cy - r * 0.04, r * 0.15, r * 0.17) {
                col = EYE;
                if ell(cx + side * r * 0.42 - r * 0.05, cy - r * 0.1, r * 0.05, r * 0.05) {
                    col = 0xFFFFFF;
                }
            }
        }
        if in_tri((x, y), (cx - r * 0.1, cy + r * 0.14), (cx + r * 0.1, cy + r * 0.14), (cx, cy + r * 0.27)) {
            col = NOSE;
        }
    }
    Some(col)
}

pub fn icon(size: u32) -> egui::IconData {
    let ss = 4; // 4x4 supersampling
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = (px as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32;
                    if let Some(c) = icon_sample(x, y) {
                        r += c >> 16 & 0xFF;
                        g += c >> 8 & 0xFF;
                        b += c & 0xFF;
                        a += 1;
                    }
                }
            }
            let n = (ss * ss) as u32;
            if a == 0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                rgba.extend_from_slice(&[(r / a) as u8, (g / a) as u8, (b / a) as u8, (a * 255 / n) as u8]);
            }
        }
    }
    egui::IconData { rgba, width: size, height: size }
}

// ---------- Assistant ----------

/// What Meng says right now, plus the face that goes with it.
pub fn advice(data: &Data, timer: &Timer, idle_paused: bool) -> Vec<(Mood, String)> {
    let mut out: Vec<(Mood, String)> = vec![];
    let name = &data.settings.mascot_name;
    let hour = store::local(store::now()).format("%H").to_string().parse::<u32>().unwrap_or(12);
    let today = store::today();
    let done = data.secs_on(today);
    let goal = data.settings.daily_goal_min as i64 * 60;
    let streak = data.streak(60);

    if idle_paused {
        out.push((Mood::Sleepy, "Stepped away? I paused the timer. Idle time doesn't count.".into()));
        return out;
    }
    if timer.running && !timer.is_focus() {
        out.push((Mood::Happy, "Break time! Drink some water and stretch a little.".into()));
        out.push((Mood::Happy, "Look at something far away for 20 seconds to rest your eyes.".into()));
        out.push((Mood::Happy, "Put the phone down. Let the break be a real break.".into()));
        return out;
    }
    if timer.running {
        out.push((Mood::Focus, "Stay focused! I'll keep an eye on the clock.".into()));
        out.push((Mood::Focus, "Something else on your mind? Jot it down and deal with it after.".into()));
        out.push((Mood::Focus, "One small step every day is already a lot.".into()));
        if goal > 0 && done < goal {
            out.push((Mood::Focus, format!("{} to go for today's goal. You've got this!", fmt_dur(goal - done))));
        }
        return out;
    }

    if hour >= 23 || hour < 4 {
        out.push((Mood::Sleepy, "It's late. Good sleep is part of studying too.".into()));
    } else {
        let greet = match hour {
            4..=10 => "Good morning",
            11..=14 => "Good afternoon",
            15..=17 => "Good evening",
            _ => "Good evening",
        };
        out.push((Mood::Normal, format!("{greet}! I'm {name}. What are we studying today?")));
    }
    if goal > 0 {
        if done >= goal {
            out.push((Mood::Happy, format!("You hit today's {} goal! {name} is proud of you.", fmt_dur(goal))));
        } else {
            let left = goal - done;
            let pomos = (left as f64 / (data.settings.focus_min.max(1) as f64 * 60.0)).ceil() as i64;
            out.push((
                Mood::Normal,
                format!("{} of {} so far today. About {} more pomodoros.", fmt_dur(done), fmt_dur(goal), pomos),
            ));
        }
    }
    if streak >= 2 {
        out.push((Mood::Happy, format!("{streak}-day streak! Let's keep it going.")));
    } else if done == 0 && data.secs_on(today.pred_opt().unwrap()) > 0 {
        out.push((Mood::Normal, "You studied yesterday. One session today starts a streak!".into()));
    }
    let open_tasks = data.tasks.iter().filter(|t| !t.done).count();
    if open_tasks > 0 {
        out.push((Mood::Normal, format!("{open_tasks} tasks still open. Pick one and press Start.")));
    } else {
        out.push((Mood::Normal, "No tasks yet. Add one on the Tasks page to give your focus a target.".into()));
    }
    // A subject left alone this week.
    let week_ago = today - chrono::Duration::days(6);
    let mut per: Vec<(String, i64)> = data
        .subjects
        .iter()
        .filter(|s| !s.archived)
        .map(|s| {
            let secs = data
                .sessions
                .iter()
                .filter(|x| x.subject_id == Some(s.id) && store::local_day(x.start) >= week_ago)
                .map(|x| x.secs)
                .sum();
            (s.name.clone(), secs)
        })
        .collect();
    per.sort_by_key(|p| p.1);
    if per.len() >= 2 && per[0].1 == 0 && per.last().unwrap().1 > 0 {
        out.push((Mood::Normal, format!("You haven't touched {} this week. One session?", per[0].0)));
    }
    out.push((Mood::Normal, "Tip: Ctrl+Alt+Space starts or pauses from anywhere.".into()));
    out
}

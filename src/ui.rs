//! Shared widgets and fonts: cards, buttons, chips, toggles, rings.

use crate::theme::{alpha, Pal};
use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Pos2, Response,
    RichText, Sense, Shape, Stroke, Ui,
};
use std::f32::consts::PI;
use std::sync::Arc;

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fallbacks = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for (key, bytes) in [
        ("inter", &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..]),
        ("inter-semibold", &include_bytes!("../assets/fonts/Inter-SemiBold.ttf")[..]),
        ("inter-bold", &include_bytes!("../assets/fonts/Inter-Bold.ttf")[..]),
    ] {
        fonts.font_data.insert(key.into(), Arc::new(FontData::from_static(bytes)));
    }
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "inter".into());
    for (fam, key) in [("semibold", "inter-semibold"), ("bold", "inter-bold")] {
        let mut list = vec![key.to_string()];
        list.extend(fallbacks.iter().cloned());
        fonts.families.insert(FontFamily::Name(fam.into()), list);
    }
    ctx.set_fonts(fonts);
}

#[derive(Clone, Copy)]
pub enum W {
    R,
    S,
    B,
}

pub fn font(size: f32, w: W) -> FontId {
    match w {
        W::R => FontId::proportional(size),
        W::S => FontId::new(size, FontFamily::Name("semibold".into())),
        W::B => FontId::new(size, FontFamily::Name("bold".into())),
    }
}

pub fn txt(s: impl Into<String>, size: f32, w: W, c: Color32) -> RichText {
    RichText::new(s).font(font(size, w)).color(c)
}

pub fn card<R>(ui: &mut Ui, p: &Pal, add: impl FnOnce(&mut Ui) -> R) -> R {
    crate::glass::frame(ui, p, 20, Margin::same(18), add).inner
}

/// Smaller glass panel for list rows.
pub fn row<R>(ui: &mut Ui, p: &Pal, add: impl FnOnce(&mut Ui) -> R) -> R {
    crate::glass::frame(ui, p, 14, Margin::symmetric(14, 10), add).inner
}

pub fn heading(ui: &mut Ui, p: &Pal, title: &str, sub: &str) {
    ui.label(txt(title, 24.0, W::B, p.text));
    if !sub.is_empty() {
        ui.label(txt(sub, 13.0, W::R, p.subtext));
    }
    ui.add_space(8.0);
}

pub fn section(ui: &mut Ui, p: &Pal, s: &str) {
    ui.label(txt(s.to_uppercase(), 11.0, W::S, p.subtext));
}

/// Height of every regular button, so rows of actions line up across pages.
pub const BTN_H: f32 = 30.0;

pub fn accent_button(ui: &mut Ui, p: &Pal, label: &str, size: egui::Vec2) -> Response {
    ui.add(
        egui::Button::new(txt(label, 14.0, W::S, p.on_accent))
            .fill(p.accent)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(12))
            .min_size(size),
    )
}

/// Vector icons (fonts lack some media glyphs): play, pause, stop, skip, expand.
pub fn paint_icon(painter: &egui::Painter, name: &str, c: Pos2, s: f32, col: Color32) {
    let h = s / 2.0;
    match name {
        "play" => {
            let pts = vec![pos2(c.x - h * 0.7, c.y - h), pos2(c.x + h, c.y), pos2(c.x - h * 0.7, c.y + h)];
            painter.add(Shape::convex_polygon(pts, col, Stroke::NONE));
        }
        "pause" => {
            for dx in [-h * 0.55, h * 0.55] {
                let r = egui::Rect::from_center_size(pos2(c.x + dx, c.y), vec2(h * 0.6, s));
                painter.rect_filled(r, CornerRadius::same(1), col);
            }
        }
        "stop" => {
            painter.rect_filled(egui::Rect::from_center_size(c, vec2(s * 0.85, s * 0.85)), CornerRadius::same(2), col);
        }
        "skip" => {
            let pts = vec![pos2(c.x - h, c.y - h * 0.9), pos2(c.x + h * 0.45, c.y), pos2(c.x - h, c.y + h * 0.9)];
            painter.add(Shape::convex_polygon(pts, col, Stroke::NONE));
            let r = egui::Rect::from_min_max(pos2(c.x + h * 0.55, c.y - h * 0.9), pos2(c.x + h, c.y + h * 0.9));
            painter.rect_filled(r, CornerRadius::same(1), col);
        }
        "expand" => {
            let st = Stroke::new((s * 0.14).max(1.4), col);
            let (a, b) = (pos2(c.x - h, c.y + h), pos2(c.x + h, c.y - h));
            painter.line_segment([a, b], st);
            painter.line_segment([b, pos2(b.x - h * 0.9, b.y)], st);
            painter.line_segment([b, pos2(b.x, b.y + h * 0.9)], st);
            painter.line_segment([a, pos2(a.x + h * 0.9, a.y)], st);
            painter.line_segment([a, pos2(a.x, a.y - h * 0.9)], st);
        }
        "home" => {
            let st = Stroke::new(s * 0.12, col);
            painter.add(Shape::line(vec![pos2(c.x - h, c.y - h * 0.05), pos2(c.x, c.y - h * 0.95), pos2(c.x + h, c.y - h * 0.05)], st));
            painter.add(Shape::line(vec![pos2(c.x - h * 0.7, c.y - h * 0.3), pos2(c.x - h * 0.7, c.y + h * 0.9), pos2(c.x + h * 0.7, c.y + h * 0.9), pos2(c.x + h * 0.7, c.y - h * 0.3)], st));
            painter.rect_filled(egui::Rect::from_min_max(pos2(c.x - h * 0.22, c.y + h * 0.3), pos2(c.x + h * 0.22, c.y + h * 0.9)), CornerRadius::same(1), col);
        }
        "timer" => {
            let st = Stroke::new(s * 0.12, col);
            painter.circle_stroke(pos2(c.x, c.y + h * 0.1), h * 0.85, st);
            painter.line_segment([pos2(c.x, c.y + h * 0.1), pos2(c.x, c.y - h * 0.4)], st);
            painter.line_segment([pos2(c.x, c.y + h * 0.1), pos2(c.x + h * 0.35, c.y + h * 0.3)], st);
            painter.line_segment([pos2(c.x - h * 0.3, c.y - h), pos2(c.x + h * 0.3, c.y - h)], st);
        }
        "tasks" => {
            let st = Stroke::new(s * 0.12, col);
            painter.rect_stroke(egui::Rect::from_center_size(c, vec2(s * 0.9, s * 0.9)), CornerRadius::same(3), st, egui::StrokeKind::Middle);
            painter.add(Shape::line(vec![pos2(c.x - h * 0.45, c.y), pos2(c.x - h * 0.1, c.y + h * 0.35), pos2(c.x + h * 0.5, c.y - h * 0.35)], st));
        }
        "book" => {
            let st = Stroke::new(s * 0.12, col);
            for side in [-1.0f32, 1.0] {
                painter.add(Shape::line(
                    vec![pos2(c.x, c.y - h * 0.65), pos2(c.x + side * h * 0.95, c.y - h * 0.85), pos2(c.x + side * h * 0.95, c.y + h * 0.7), pos2(c.x, c.y + h * 0.9)],
                    st,
                ));
            }
            painter.line_segment([pos2(c.x, c.y - h * 0.65), pos2(c.x, c.y + h * 0.9)], st);
        }
        "history" => {
            let st = Stroke::new(s * 0.12, col);
            painter.add(Shape::line(arc_points(c, h * 0.9, -PI * 0.85, PI * 0.95), st));
            painter.add(Shape::convex_polygon(
                vec![pos2(c.x - h * 1.05, c.y - h * 0.7), pos2(c.x - h * 0.35, c.y - h * 0.4), pos2(c.x - h * 1.0, c.y + h * 0.05)],
                col,
                Stroke::NONE,
            ));
            painter.line_segment([c, pos2(c.x, c.y - h * 0.5)], st);
            painter.line_segment([c, pos2(c.x + h * 0.4, c.y + h * 0.2)], st);
        }
        "stats" => {
            for (i, k) in [0.45f32, 0.9, 0.65].iter().enumerate() {
                let x = c.x - h * 0.7 + i as f32 * h * 0.7;
                let r = egui::Rect::from_min_max(pos2(x - h * 0.22, c.y + h - s * k), pos2(x + h * 0.22, c.y + h));
                painter.rect_filled(r, CornerRadius::same(1), col);
            }
        }
        "gear" => {
            for i in 0..8 {
                let a = i as f32 / 8.0 * PI * 2.0;
                let d = vec2(a.cos(), a.sin());
                painter.line_segment([c + d * h * 0.55, c + d * h * 0.98], Stroke::new(s * 0.2, col));
            }
            painter.circle_stroke(c, h * 0.5, Stroke::new(s * 0.2, col));
        }
        "flame" => {
            // Teardrop: lower half-circle plus a tip on top.
            let mut pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = i as f32 / 16.0 * PI;
                    pos2(c.x + a.cos() * h * 0.62, c.y + h * 0.3 + a.sin() * h * 0.62)
                })
                .collect();
            pts.push(pos2(c.x - h * 0.62, c.y + h * 0.3));
            pts.push(pos2(c.x, c.y - h));
            pts.reverse();
            painter.add(Shape::convex_polygon(pts, col, Stroke::NONE));
        }
        _ => {}
    }
}

/// Big button with a vector icon and a label.
pub fn icon_button(ui: &mut Ui, p: &Pal, icon: &str, label: &str, size: egui::Vec2, primary: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let (bg, fg) = if primary { (p.accent, p.on_accent) } else { (p.surface2, p.text) };
    let bg = if resp.hovered() { if primary { alpha(p.accent, 0.88) } else { p.hover } } else { bg };
    ui.painter().rect_filled(rect, CornerRadius::same(12), bg);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font(14.0, W::S), fg);
    let total = 14.0 + 10.0 + galley.size().x;
    let x0 = rect.center().x - total / 2.0;
    paint_icon(ui.painter(), icon, pos2(x0 + 7.0, rect.center().y), 13.0, fg);
    ui.painter().galley(pos2(x0 + 24.0, rect.center().y - galley.size().y / 2.0), galley, fg);
    resp
}

pub fn soft_button(ui: &mut Ui, p: &Pal, label: &str, size: egui::Vec2) -> Response {
    ui.add(
        egui::Button::new(txt(label, 13.0, W::S, p.text))
            .fill(p.surface2)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(12))
            .min_size(size),
    )
}

pub fn danger_button(ui: &mut Ui, p: &Pal, label: &str) -> Response {
    ui.add(
        egui::Button::new(txt(label, 13.0, W::S, p.danger))
            .fill(alpha(p.danger, 0.12))
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(10))
            .min_size(vec2(0.0, BTN_H)),
    )
}

pub fn chip(ui: &mut Ui, p: &Pal, label: &str, selected: bool) -> Response {
    let (fill, col) = if selected { (p.accent_soft, p.accent) } else { (p.surface2, p.subtext) };
    ui.add(
        egui::Button::new(txt(label, 12.0, W::S, col))
            .fill(fill)
            .stroke(if selected { Stroke::new(1.0, alpha(p.accent, 0.6)) } else { Stroke::NONE })
            .corner_radius(CornerRadius::same(255))
            .min_size(vec2(0.0, 28.0)),
    )
}

/// iOS-like switch.
pub fn toggle(ui: &mut Ui, p: &Pal, on: &mut bool) -> Response {
    let size = vec2(40.0, 22.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let k = ui.ctx().animate_bool_responsive(resp.id, *on);
    let bg = if *on { p.accent } else { p.track };
    ui.painter().rect_filled(rect, CornerRadius::same(11), bg);
    let x = egui::lerp(rect.left() + 11.0..=rect.right() - 11.0, k);
    ui.painter().circle_filled(pos2(x, rect.center().y), 8.5, Color32::WHITE);
    resp
}

pub fn setting_row(ui: &mut Ui, p: &Pal, label: &str, hint: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(txt(label, 13.0, W::S, p.text));
            if !hint.is_empty() {
                ui.label(txt(hint, 11.0, W::R, p.subtext));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
    ui.add_space(4.0);
}

pub fn dot(ui: &mut Ui, color: Color32, r: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(r * 2.0 + 2.0, r * 2.0 + 2.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), r, color);
}

pub fn arc_points(c: Pos2, r: f32, from: f32, to: f32) -> Vec<Pos2> {
    let n = (((to - from).abs() / (2.0 * PI)) * 96.0).ceil().max(2.0) as usize;
    (0..=n).map(|i| {
        let a = from + (to - from) * i as f32 / n as f32;
        pos2(c.x + a.cos() * r, c.y + a.sin() * r)
    }).collect()
}

/// Progress ring starting at 12 o'clock.
pub fn ring(painter: &egui::Painter, c: Pos2, r: f32, width: f32, frac: f32, track: Color32, color: Color32) {
    painter.circle_stroke(c, r, Stroke::new(width, track));
    let frac = frac.clamp(0.0, 1.0);
    if frac > 0.001 {
        let start = -PI / 2.0;
        let pts = arc_points(c, r, start, start + frac * 2.0 * PI);
        let end = *pts.last().unwrap();
        painter.add(Shape::line(pts, Stroke::new(width, color)));
        painter.circle_filled(pos2(c.x, c.y - r), width / 2.0, color);
        painter.circle_filled(end, width / 2.0, color);
    }
}

pub fn progress_bar(ui: &mut Ui, p: &Pal, frac: f32, color: Color32, height: f32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    let rr = CornerRadius::same((height / 2.0) as u8);
    ui.painter().rect_filled(rect, rr, p.track);
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        let mut r2 = rect;
        r2.set_width((rect.width() * f).max(height));
        ui.painter().rect_filled(r2, rr, color);
    }
}

pub fn subject_combo(ui: &mut Ui, id: &str, data: &crate::store::Data, sel: &mut Option<u64>, width: f32) -> bool {
    let mut changed = false;
    let label = data.subject_name(*sel);
    egui::ComboBox::from_id_salt(id).selected_text(label).width(width).show_ui(ui, |ui| {
        changed |= ui.selectable_value(sel, None, "No subject").changed();
        for s in data.subjects.iter().filter(|s| !s.archived) {
            changed |= ui.selectable_value(sel, Some(s.id), s.name.clone()).changed();
        }
    });
    changed
}

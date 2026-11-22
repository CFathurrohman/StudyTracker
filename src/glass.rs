//! Frosted / liquid glass panels. A panel shows the blurred scene texture at its own screen position,
//! lightly tinted, with a soft shadow, a top sheen, an inner glow, and a rim that catches the light
//! (bright top-left, softer bottom-right), which is what makes glass read as liquid rather than flat.

use crate::theme::Pal;
use eframe::egui::{self, epaint::{PathShape, PathStroke, RectShape, Vertex}, pos2, vec2, Color32, CornerRadius, Mesh, Pos2, Rect, Shape, TextureId, Vec2};
use std::f32::consts::PI;
use std::cell::RefCell;

#[derive(Clone, Copy)]
pub struct Glass {
    pub tex: Option<TextureId>,
    pub screen: Rect,
    pub light: bool,
    /// 0 = very clear, 1 = milky.
    pub tint: f32,
}

thread_local! {
    static CURRENT: RefCell<Option<Glass>> = const { RefCell::new(None) };
}

pub fn set(g: Option<Glass>) {
    CURRENT.with(|c| *c.borrow_mut() = g);
}

pub fn get() -> Option<Glass> {
    CURRENT.with(|c| *c.borrow())
}

fn a(c: Color32, k: f32) -> Color32 {
    crate::theme::alpha(c, k)
}

/// Outline of a rounded rectangle, clockwise from the top-left corner, with outward normals.
fn outline(rect: Rect, radius: f32) -> Vec<(Pos2, Vec2)> {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.5);
    let corners = [
        (pos2(rect.left() + r, rect.top() + r), PI),
        (pos2(rect.right() - r, rect.top() + r), PI * 1.5),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), PI * 0.5),
    ];
    let segs = 10;
    let mut out = Vec::with_capacity(4 * (segs + 1));
    for (c, a0) in corners {
        for i in 0..=segs {
            let a = a0 + (i as f32 / segs as f32) * PI * 0.5;
            let n = vec2(a.cos(), a.sin());
            out.push((c + n * r, n));
        }
    }
    out
}

/// Vertical sheen over the top of the panel, gone by `fade` of the height.
fn sheen(path: &[(Pos2, Vec2)], rect: Rect, top_alpha: f32, fade: f32) -> Shape {
    let at = |y: f32| {
        let k = (1.0 - (y - rect.top()) / (rect.height() * fade).max(1.0)).clamp(0.0, 1.0);
        Color32::from_white_alpha((top_alpha * k * k * 255.0) as u8)
    };
    let mut m = Mesh::default();
    let c = rect.center();
    m.vertices.push(Vertex { pos: c, uv: egui::epaint::WHITE_UV, color: at(c.y) });
    for (p, _) in path {
        m.vertices.push(Vertex { pos: *p, uv: egui::epaint::WHITE_UV, color: at(p.y) });
    }
    let n = path.len() as u32;
    for i in 0..n {
        m.indices.extend_from_slice(&[0, 1 + i, 1 + (i + 1) % n]);
    }
    Shape::mesh(m)
}

/// The shapes of a glass panel (paint them *under* the panel's content).
pub fn shapes(rect: Rect, radius: u8, p: &Pal, strong: bool) -> Vec<Shape> {
    let mut out = vec![];
    let cr = CornerRadius::same(radius);
    let g = get();
    let light = g.map_or(p.light, |g| g.light);
    // Soft, wide shadow lifts the panel off the scene.
    let shadow = egui::epaint::Shadow { offset: [0, 12], blur: 36, spread: 0, color: Color32::from_black_alpha(if light { 22 } else { 60 }) };
    out.push(shadow.as_shape(rect, cr).into());
    match g {
        Some(Glass { tex: Some(tex), screen, tint, .. }) => {
            // The scene behind, blurred and a little more vivid (vibrancy).
            let uv = Rect::from_min_max(
                pos2((rect.left() - screen.left()) / screen.width(), (rect.top() - screen.top()) / screen.height()),
                pos2((rect.right() - screen.left()) / screen.width(), (rect.bottom() - screen.top()) / screen.height()),
            );
            out.push(RectShape::filled(rect, cr, Color32::WHITE).with_texture(tex, uv).into());
            // Light tint keeps text readable without hiding the colors behind.
            let hover = if strong { 0.08 } else { 0.0 };
            if light {
                out.push(RectShape::filled(rect, cr, a(Color32::WHITE, 0.16 + 0.42 * tint + hover)).into());
            } else {
                out.push(RectShape::filled(rect, cr, a(Color32::from_rgb(8, 10, 18), 0.2 + 0.4 * tint)).into());
                out.push(RectShape::filled(rect, cr, Color32::from_white_alpha((8.0 + hover * 160.0) as u8)).into());
            }
        }
        _ => out.push(RectShape::filled(rect, cr, p.surface).into()),
    }
    // Sheen over the top, kept just inside the rim so its edge never shows.
    let inner = rect.shrink(1.5);
    out.push(sheen(&outline(inner, (radius as f32 - 1.5).max(1.0)), rect, if light { 0.26 } else { 0.07 }, 0.5));
    // The rim: one thin anti-aliased stroke whose brightness follows the light, strongest top-left,
    // with a softer catch-light bottom-right.
    let edge = rect.shrink(0.6);
    let points: Vec<Pos2> = outline(edge, (radius as f32 - 0.6).max(0.5)).into_iter().map(|(p, _)| p).collect();
    let (c, half) = (rect.center(), rect.size() / 2.0);
    // Light comes from above: the top edge glows, the sides stay faint (so they never read as lines),
    // and the bottom gets only a soft catch-light.
    let (r0, r1, r2) = if light { (0.28, 0.6, 0.12) } else { (0.05, 0.42, 0.06) };
    let stroke = PathStroke::new_uv(1.2, move |_, pos| {
        let n = vec2((pos.x - c.x) / half.x, (pos.y - c.y) / half.y).normalized();
        let a = (r0 + r1 * (-n.y).max(0.0).powf(1.6) + r2 * n.y.max(0.0).powi(2)).clamp(0.0, 1.0);
        Color32::from_white_alpha((a * 255.0) as u8)
    });
    out.push(Shape::Path(PathShape { points, closed: true, fill: Color32::TRANSPARENT, stroke }));
    out
}

/// A frame whose background is a glass panel.
pub fn frame<R>(ui: &mut egui::Ui, p: &Pal, radius: u8, margin: egui::Margin, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let slot = ui.painter().add(Shape::Noop);
    let resp = egui::Frame::new().inner_margin(margin).corner_radius(radius).show(ui, add);
    let rect = resp.response.rect;
    ui.painter().set(slot, Shape::Vec(shapes(rect, radius, p, false)));
    resp
}

/// Glass pill button. Returns the response.
pub fn pill(ui: &mut egui::Ui, p: &Pal, size: egui::Vec2) -> (Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let r = (size.y / 2.0).min(255.0) as u8;
    for s in shapes(rect, r, p, resp.hovered()) {
        ui.painter().add(s);
    }
    (rect, resp)
}


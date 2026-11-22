//! Color schemes and accents, same palette family as FathNotch. Applied to egui's visuals.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

pub struct Scheme {
    pub id: &'static str,
    pub name: &'static str,
    pub bg: u32,
    pub surface: u32,
    pub surface2: u32,
    pub hover: u32,
    pub text: u32,
    pub subtext: u32,
    pub light: bool,
    pub accent: &'static str,
}

const fn s(
    id: &'static str, name: &'static str, bg: u32, surface: u32, surface2: u32, hover: u32, text: u32,
    subtext: u32, light: bool, accent: &'static str,
) -> Scheme {
    Scheme { id, name, bg, surface, surface2, hover, text, subtext, light, accent }
}

pub const SCHEMES: &[Scheme] = &[
    s("midnight", "Midnight", 0x0B0C10, 0x111318, 0x1A1D24, 0x262A33, 0xF4F6FA, 0x8B91A1, false, "orange"),
    s("graphite", "Graphite", 0x1B1B1D, 0x242427, 0x2D2D31, 0x38383D, 0xF2F2F4, 0x94949A, false, "blue"),
    s("slate", "Slate", 0x161A20, 0x1F242C, 0x283039, 0x333C47, 0xEEF1F5, 0x8F99A6, false, "sky"),
    s("nord", "Nord", 0x1A1F29, 0x232A36, 0x2C3442, 0x38414F, 0xECEFF4, 0x8E99AB, false, "teal"),
    s("ocean", "Ocean", 0x07141F, 0x0E1E2B, 0x152837, 0x1F3445, 0xE6F1F8, 0x86A0B3, false, "sky"),
    s("forest", "Forest", 0x0F1712, 0x17211B, 0x1F2B23, 0x29372E, 0xE9F2EC, 0x8BA295, false, "mint"),
    s("plum", "Plum", 0x170F1A, 0x211725, 0x2B1F30, 0x372A3D, 0xF4ECF6, 0xA592AB, false, "lavender"),
    s("mocha", "Mocha", 0x1C1714, 0x26201C, 0x302823, 0x3C332C, 0xF4EDE6, 0xA39589, false, "amber"),
    s("rosewood", "Rosewood", 0x1C1012, 0x27181B, 0x322024, 0x3E2A2F, 0xF7ECEE, 0xAE9196, false, "rose"),
    s("nebula", "Nebula", 0x0B0718, 0x161026, 0x201833, 0x2B2241, 0xF1ECFA, 0x9C92B3, false, "lavender"),
    s("daylight", "Daylight", 0xFFFFFF, 0xF3F3F5, 0xE9E9EC, 0xDCDCE1, 0x1C1C1E, 0x6E6E75, true, "blue"),
    s("paper", "Paper", 0xFBF8F3, 0xF2EDE4, 0xE8E1D5, 0xDDD4C5, 0x24201A, 0x776E61, true, "orange"),
    s("frost", "Frost", 0xF6F9FC, 0xEAF0F6, 0xDFE7F0, 0xD2DCE8, 0x18212B, 0x66727F, true, "indigo"),
    s("dawn", "Dawn", 0xFFF7F0, 0xF5EDE8, 0xECE2DD, 0xE0D4CF, 0x241C1A, 0x7A6B66, true, "pink"),
];

pub const ACCENTS: &[(&str, &str, u32)] = &[
    ("orange", "Orange", 0xFF8A3D),
    ("amber", "Amber", 0xFFC233),
    ("lime", "Lime", 0xA3E635),
    ("green", "Green", 0x32D74B),
    ("mint", "Mint", 0x34D3A0),
    ("teal", "Teal", 0x3CD3E0),
    ("sky", "Sky", 0x38BDF8),
    ("blue", "Blue", 0x0A84FF),
    ("indigo", "Indigo", 0x6E6CF0),
    ("lavender", "Lavender", 0xA78BFA),
    ("purple", "Purple", 0xBF5AF2),
    ("pink", "Pink", 0xFF4F79),
    ("rose", "Rose", 0xFB7185),
    ("red", "Red", 0xFF453A),
];

pub fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub fn scheme(id: &str) -> &'static Scheme {
    SCHEMES.iter().find(|s| s.id == id).unwrap_or(&SCHEMES[0])
}

pub fn accent(id: &str) -> u32 {
    ACCENTS.iter().find(|a| a.0 == id).map_or(0xFF8A3D, |a| a.2)
}

/// Colors the custom-drawn parts use.
#[derive(Clone, Copy)]
pub struct Pal {
    pub bg: Color32,
    pub surface: Color32,
    pub surface2: Color32,
    pub hover: Color32,
    pub stroke: Color32,
    pub track: Color32,
    pub text: Color32,
    pub subtext: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub danger: Color32,
    pub success: Color32,
    pub light: bool,
}

pub fn alpha(c: Color32, a: f32) -> Color32 {
    let [r, g, b, _] = c.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Theme colors for a glass UI: controls are translucent so the frosted panels show through.
pub fn build(scheme_id: &str, accent_id: &str) -> Pal {
    let mut p = build_solid(scheme_id, accent_id);
    let (k2, kh) = if p.light { (0.55, 0.8) } else { (0.08, 0.14) };
    p.surface2 = alpha(Color32::WHITE, k2);
    p.hover = alpha(Color32::WHITE, kh);
    p
}

fn build_solid(scheme_id: &str, accent_id: &str) -> Pal {
    let sc = scheme(scheme_id);
    let acc = if accent_id.is_empty() || accent_id == "auto" { accent(sc.accent) } else { accent(accent_id) };
    let text = rgb(sc.text);
    if accent_id == "glass" {
        // Neutral glass: no hue, just frosted white (or soft graphite on light themes).
        let (acc, on) = if sc.light { (rgb(0x2B2F38), Color32::WHITE) } else { (rgb(0xEEF0F6), rgb(0x15171E)) };
        return Pal {
            bg: rgb(sc.bg),
            surface: rgb(sc.surface),
            surface2: rgb(sc.surface2),
            hover: rgb(sc.hover),
            stroke: alpha(text, if sc.light { 0.12 } else { 0.08 }),
            track: alpha(text, 0.12),
            text,
            subtext: rgb(sc.subtext),
            accent: acc,
            accent_soft: if sc.light { Color32::from_black_alpha(18) } else { Color32::from_white_alpha(30) },
            on_accent: on,
            danger: rgb(if sc.light { 0xD7362B } else { 0xFF453A }),
            success: rgb(if sc.light { 0x1F9D55 } else { 0x32D74B }),
            light: sc.light,
        };
    }
    Pal {
        bg: rgb(sc.bg),
        surface: rgb(sc.surface),
        surface2: rgb(sc.surface2),
        hover: rgb(sc.hover),
        stroke: alpha(text, if sc.light { 0.12 } else { 0.08 }),
        track: alpha(text, if sc.light { 0.12 } else { 0.12 }),
        text,
        subtext: rgb(sc.subtext),
        accent: rgb(acc),
        accent_soft: alpha(rgb(acc), 0.18),
        on_accent: if matches!(accent_id_resolved(sc, accent_id), "amber" | "lime") { rgb(0x1C1C1E) } else { Color32::WHITE },
        danger: rgb(if sc.light { 0xD7362B } else { 0xFF453A }),
        success: rgb(if sc.light { 0x1F9D55 } else { 0x32D74B }),
        light: sc.light,
    }
}

fn accent_id_resolved<'a>(sc: &'a Scheme, accent_id: &'a str) -> &'a str {
    if accent_id.is_empty() || accent_id == "auto" { sc.accent } else { accent_id }
}

pub fn apply(ctx: &egui::Context, p: &Pal) {
    let mut v = if p.light { Visuals::light() } else { Visuals::dark() };
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.surface2;
    v.faint_bg_color = p.surface;
    v.code_bg_color = p.surface2;
    v.override_text_color = Some(p.text);
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.window_stroke = Stroke::new(1.0, p.stroke);
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(10);
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: alpha(Color32::BLACK, 0.25) };
    v.window_shadow = v.popup_shadow;
    let r = CornerRadius::same(8);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.stroke);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = p.surface2;
    w.inactive.weak_bg_fill = p.surface2;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = p.hover;
    w.hovered.weak_bg_fill = p.hover;
    w.hovered.bg_stroke = Stroke::new(1.0, alpha(p.accent, 0.5));
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = p.accent_soft;
    w.active.weak_bg_fill = p.accent_soft;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.5, p.text);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;
    w.open = w.active;
    v.slider_trailing_fill = true;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 30.0;
        // Thin scrollbars that float over content and only appear on hover (no white lines).
        s.spacing.scroll = egui::style::ScrollStyle::floating();
    });
}

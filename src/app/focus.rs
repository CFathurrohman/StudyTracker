//! Immersive focus mode: the scene fills the window; a big clock (an analog "time left" wedge for
//! Pomodoro and countdowns), the subject, and minimal glass controls.

use super::home::{hms, pie_clock};
use super::{App, Page};
use crate::glass;
use crate::theme::alpha;
use crate::timer::{fmt_clock, Mode};
use crate::ui::{self, W};
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, Rect, Ui};

impl App {
    pub(super) fn page_focus(&mut self, ui: &mut Ui) {
        let p = self.pal;
        let d = self.durations();
        let full = ui.max_rect();
        let white = Color32::from_rgb(248, 248, 252);
        let soft = Color32::from_white_alpha(190);
        // Back button.
        let back = Rect::from_min_size(full.min + vec2(8.0, 8.0), vec2(118.0, 38.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(back));
        let (r, resp) = glass::pill(&mut child, &p, back.size());
        child.painter().text(r.center(), Align2::CENTER_CENTER, "‹  Back", ui::font(13.0, W::S), p.text);
        if resp.clicked() {
            self.page = Page::Home;
        }

        let subject = self.data.subject_name(self.subject);
        let task = self.data.task(self.task).map(|t| t.title.clone());
        let title = task.unwrap_or(subject.clone());
        let shadow = Color32::from_black_alpha(90);
        let text_shadowed = |ui: &Ui, pos: egui::Pos2, s: &str, font: egui::FontId, col: Color32| {
            ui.painter().text(pos + vec2(0.0, 2.0), Align2::CENTER_CENTER, s, font.clone(), shadow);
            ui.painter().text(pos, Align2::CENTER_CENTER, s, font, col);
        };
        let phase = if self.idle_notice.is_some() {
            "paused while you were away".to_string()
        } else if self.timer.mode == Mode::Pomodoro {
            format!("{} · {}", self.timer.phase.label().to_lowercase(), subject)
        } else {
            format!("{} · {}", self.timer.mode.label().to_lowercase(), subject)
        };
        let today = format!("today {}", hms(self.live_today()));
        let timed = self.timer.target_secs(&d).is_some();
        let cx = full.center().x;

        // Layout: title, then either a wedge clock with the time under it, or big stopwatch digits.
        let controls_y;
        if timed {
            let r = (full.height() * 0.2).clamp(90.0, 150.0);
            let c = pos2(cx, full.center().y - full.height() * 0.1);
            text_shadowed(ui, pos2(cx, c.y - r - 52.0), &title, ui::font(20.0, W::S), white);
            text_shadowed(ui, pos2(cx, c.y - r - 24.0), &phase, ui::font(13.0, W::S), soft);
            pie_clock(ui.painter(), c, r, 1.0 - self.timer.progress(&d), &p, true);
            text_shadowed(ui, pos2(cx, c.y + r + 38.0), &fmt_clock(self.timer.display_secs(&d)), ui::font(52.0, W::B), white);
            text_shadowed(ui, pos2(cx, c.y + r + 74.0), &today, ui::font(13.0, W::S), soft);
            controls_y = c.y + r + 96.0;
        } else {
            let c = pos2(cx, full.center().y - full.height() * 0.1);
            text_shadowed(ui, c - vec2(0.0, 108.0), &title, ui::font(20.0, W::S), white);
            text_shadowed(ui, c - vec2(0.0, 76.0), &phase, ui::font(13.0, W::S), soft);
            text_shadowed(ui, c, &hms(self.timer.elapsed().as_secs() as i64), ui::font(104.0, W::B), white);
            text_shadowed(ui, c + vec2(0.0, 70.0), &today, ui::font(14.0, W::S), soft);
            controls_y = c.y + 104.0;
        }

        // Controls.
        let bw = [140.0, 52.0, 52.0];
        let show_skip = self.timer.mode == Mode::Pomodoro;
        let total: f32 = if show_skip { bw.iter().sum::<f32>() + 20.0 } else { bw[0] + bw[1] + 10.0 };
        let mut x = cx - total / 2.0;
        let y = controls_y;
        let mut button = |ui: &mut Ui, w: f32, icon: &str, label: &str, primary: bool| -> bool {
            let rect = Rect::from_min_size(pos2(x, y), vec2(w, 52.0));
            x += w + 10.0;
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            let (r, resp) = glass::pill(&mut child, &p, rect.size());
            if primary {
                child.painter().rect_filled(r.shrink(3.0), CornerRadius::same(23), alpha(p.accent, if resp.hovered() { 0.95 } else { 0.85 }));
            }
            let fg = if primary { p.on_accent } else { p.text };
            if label.is_empty() {
                ui::paint_icon(child.painter(), icon, r.center(), 14.0, fg);
            } else {
                ui::paint_icon(child.painter(), icon, pos2(r.left() + 34.0, r.center().y), 14.0, fg);
                child.painter().text(pos2(r.left() + 52.0, r.center().y), Align2::LEFT_CENTER, label, ui::font(15.0, W::S), fg);
            }
            resp.clicked()
        };
        let (icon, label) = if self.timer.running { ("pause", "Pause") } else { ("play", if self.timer.is_idle() { "Start" } else { "Resume" }) };
        if button(ui, bw[0], icon, label, true) {
            self.toggle_timer();
        }
        if button(ui, bw[1], "stop", "", false) {
            self.stop_timer();
        }
        if show_skip && button(ui, bw[2], "skip", "", false) {
            self.skip();
        }
        let quote = if self.timer.running { "take a breath. you're here now." } else { "one small step is enough to begin." };
        text_shadowed(ui, pos2(cx, y + 84.0), quote, ui::font(13.0, W::R), soft);
    }
}

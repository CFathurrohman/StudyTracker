//! Insights: day / week / month views of study time, compared with the previous period, with study
//! span (first start to last finish), a month calendar, start & finish times, and a year of activity.

use super::home::{day_title, hms, MONTHS_SHORT};
use super::{bar_chart, heatmap, App};
use crate::store::{self, Session};
use crate::sys;
use crate::theme::{alpha, rgb};
use crate::timer::fmt_dur;
use crate::ui::{self, txt, W};
use chrono::{Datelike, NaiveDate};
use eframe::egui::{pos2, vec2, Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke, Ui};

const MONTHS_LONG: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Period {
    Day,
    Week,
    Month,
}

fn range(period: Period, anchor: NaiveDate) -> (NaiveDate, NaiveDate) {
    match period {
        Period::Day => (anchor, anchor),
        Period::Week => {
            let mon = anchor - chrono::Duration::days(anchor.weekday().num_days_from_monday() as i64);
            (mon, mon + chrono::Duration::days(6))
        }
        Period::Month => {
            let first = anchor.with_day(1).unwrap();
            let next = if first.month() == 12 {
                NaiveDate::from_ymd_opt(first.year() + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(first.year(), first.month() + 1, 1).unwrap()
            };
            (first, next.pred_opt().unwrap())
        }
    }
}

fn shift(period: Period, anchor: NaiveDate, dir: i64) -> NaiveDate {
    match period {
        Period::Day => anchor + chrono::Duration::days(dir),
        Period::Week => anchor + chrono::Duration::days(7 * dir),
        Period::Month => {
            let m = anchor.year() * 12 + anchor.month0() as i32 + dir as i32;
            NaiveDate::from_ymd_opt(m.div_euclid(12), m.rem_euclid(12) as u32 + 1, 1).unwrap()
        }
    }
}

fn label(period: Period, from: NaiveDate, to: NaiveDate) -> String {
    match period {
        Period::Day => day_title(from),
        Period::Week => format!("{} {} – {} {}", MONTHS_SHORT[from.month0() as usize], from.day(), MONTHS_SHORT[to.month0() as usize], to.day()),
        Period::Month => format!("{} {}", MONTHS_LONG[from.month0() as usize], from.year()),
    }
}

/// Minutes after the study day began.
fn day_minutes(ts: i64, day: NaiveDate) -> f32 {
    (ts - store::day_begin(day)) as f32 / 60.0
}

fn hhmm(minutes: f32) -> String {
    let m = (minutes.round() as i64 + store::day_start_hour() as i64 * 60).rem_euclid(24 * 60);
    format!("{:02}:{:02}", m / 60, m % 60)
}

impl App {
    fn sessions_in(&self, from: NaiveDate, to: NaiveDate) -> Vec<Session> {
        let mut v: Vec<Session> = self
            .data
            .sessions
            .iter()
            .filter(|s| {
                let d = store::local_day(s.start);
                d >= from && d <= to
            })
            .cloned()
            .collect();
        // The block running right now counts too.
        let today = store::today();
        if today >= from && today <= to && self.timer.is_focus() {
            if let Some(start) = self.timer.block_start {
                v.push(Session { id: 0, subject_id: self.subject, task_id: None, start, secs: self.running_secs(), note: String::new(), kind: String::new() });
            }
        }
        v
    }

    pub(super) fn page_insights(&mut self, ui: &mut Ui) {
        let p = self.pal;
        let today = store::today();
        ui.horizontal(|ui| {
            ui.vertical(|ui| ui::heading(ui, &p, "Insights", "Your study habits, day by day."));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui::soft_button(ui, &p, "Export CSV + JSON", vec2(0.0, ui::BTN_H)).clicked() {
                    match store::export(&self.data) {
                        Ok(dir) => {
                            sys::open_folder(&dir);
                            self.toast("Exported to the exports folder");
                        }
                        Err(e) => self.toast(format!("Export failed: {e}")),
                    }
                }
            });
        });

        // Period picker.
        let (from, to) = range(self.ins_period, self.ins_anchor);
        ui.horizontal(|ui| {
            for (per, name) in [(Period::Day, "Day"), (Period::Week, "Week"), (Period::Month, "Month")] {
                if ui::chip(ui, &p, name, self.ins_period == per).clicked() {
                    self.ins_period = per;
                }
            }
            ui.add_space(12.0);
            if ui::soft_button(ui, &p, "‹", vec2(ui::BTN_H, ui::BTN_H)).clicked() {
                self.ins_anchor = shift(self.ins_period, self.ins_anchor, -1);
            }
            ui.label(txt(label(self.ins_period, from, to), 15.0, W::B, p.text));
            if ui::soft_button(ui, &p, "›", vec2(ui::BTN_H, ui::BTN_H)).clicked() {
                self.ins_anchor = shift(self.ins_period, self.ins_anchor, 1);
            }
            if !(today >= from && today <= to) && ui::soft_button(ui, &p, "Today", vec2(0.0, ui::BTN_H)).clicked() {
                self.ins_anchor = today;
            }
        });
        ui.add_space(8.0);

        let (from, to) = range(self.ins_period, self.ins_anchor);
        let sessions = self.sessions_in(from, to);
        let (pf, pt) = range(self.ins_period, shift(self.ins_period, self.ins_anchor, -1));
        let prev_total: i64 = self.sessions_in(pf, pt).iter().map(|s| s.secs).sum();
        let total: i64 = sessions.iter().map(|s| s.secs).sum();
        let last = to.min(today);
        let elapsed_days = if last < from { 1 } else { (last - from).num_days() + 1 };
        // Study span per day: first start and last finish.
        let mut spans: Vec<(f32, f32)> = vec![];
        let mut day = from;
        while day <= last {
            let ss: Vec<&Session> = sessions.iter().filter(|s| store::local_day(s.start) == day).collect();
            if let (Some(a), Some(b)) = (ss.iter().map(|s| s.start).min(), ss.iter().map(|s| s.start + s.secs).max()) {
                spans.push((day_minutes(a, day), day_minutes(b, day)));
            }
            day = day.succ_opt().unwrap();
        }
        let span_text = if spans.is_empty() {
            "–".to_string()
        } else {
            let n = spans.len() as f32;
            let (a, b) = (spans.iter().map(|s| s.0).sum::<f32>() / n, spans.iter().map(|s| s.1).sum::<f32>() / n);
            format!("{} – {}", hhmm(a), hhmm(b))
        };
        let longest = sessions.iter().map(|s| s.secs).max().unwrap_or(0);
        let diff = total - prev_total;
        let prev_name = match self.ins_period {
            Period::Day => "yesterday",
            Period::Week => "last week",
            Period::Month => "last month",
        };
        let cards: [(&str, String, Option<(String, Color32)>); 5] = [
            ("Total", hms(total), Some((format!("{}{} vs {prev_name}", if diff >= 0 { "+" } else { "−" }, hms(diff.abs())), if diff >= 0 { p.success } else { p.danger }))),
            ("Daily average", fmt_dur(total / elapsed_days.max(1)), None),
            (if self.ins_period == Period::Day { "Study span" } else { "Avg. study span" }, span_text, None),
            ("Longest session", fmt_dur(longest), None),
            ("Sessions", sessions.len().to_string(), None),
        ];
        ui.columns(cards.len(), |cols| {
            for (i, (name, value, sub)) in cards.iter().enumerate() {
                ui::card(&mut cols[i], &p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(64.0);
                    ui::section(ui, &p, name);
                    ui.label(txt(value, 20.0, W::B, p.text));
                    if let Some((s, c)) = sub {
                        ui.label(txt(s, 11.0, W::S, *c));
                    }
                });
            }
        });
        ui.add_space(12.0);

        let goal = self.data.settings.daily_goal_min as i64 * 60;
        match self.ins_period {
            Period::Day => {
                ui::card(ui, &p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(txt("Minutes per hour", 14.0, W::B, p.text));
                    hour_bars(ui, &p, &sessions, from);
                });
            }
            Period::Week => {
                ui::card(ui, &p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(txt("Study time per day", 14.0, W::B, p.text));
                    let days: Vec<NaiveDate> = (0..7).map(|i| from + chrono::Duration::days(i)).collect();
                    let vals: Vec<i64> = days.iter().map(|d| sessions.iter().filter(|s| store::local_day(s.start) == *d).map(|s| s.secs).sum()).collect();
                    bar_chart(ui, &p, &days, &vals, goal);
                });
            }
            Period::Month => {
                ui::card(ui, &p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(txt("Calendar", 14.0, W::B, p.text));
                    month_calendar(ui, &p, &sessions, from, to, goal);
                });
            }
        }
        ui.add_space(12.0);

        if self.ins_period != Period::Day {
            ui::card(ui, &p, |ui| {
                ui.set_width(ui.available_width());
                ui.label(txt("Start & finish times", 14.0, W::B, p.text));
                start_finish(ui, &p, &sessions, from, to);
            });
            ui.add_space(12.0);
        }

        ui::card(ui, &p, |ui| {
            ui.set_width(ui.available_width());
            ui.label(txt("By subject", 14.0, W::B, p.text));
            ui.add_space(6.0);
            let mut ids: Vec<Option<u64>> = self.data.subjects.iter().map(|s| Some(s.id)).collect();
            ids.push(None);
            let mut rows: Vec<(String, u32, i64)> = ids
                .into_iter()
                .map(|id| (self.data.subject_name(id), self.data.subject_color(id), sessions.iter().filter(|s| s.subject_id == id).map(|s| s.secs).sum()))
                .filter(|r| r.2 > 0)
                .collect();
            rows.sort_by_key(|r| -r.2);
            if rows.is_empty() {
                ui.label(txt("No study time in this period.", 12.0, W::R, p.subtext));
            }
            for (name, color, secs) in rows {
                ui.horizontal(|ui| {
                    ui::dot(ui, rgb(color), 4.0);
                    ui.label(txt(name, 12.5, W::S, p.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let pct = if total > 0 { secs as f32 / total as f32 * 100.0 } else { 0.0 };
                        ui.label(txt(format!("{}  ·  {:.0}%", hms(secs), pct), 12.0, W::S, p.subtext));
                    });
                });
                ui::progress_bar(ui, &p, if total > 0 { secs as f32 / total as f32 } else { 0.0 }, rgb(color), 6.0);
                ui.add_space(2.0);
            }
        });
        ui.add_space(12.0);

        ui::card(ui, &p, |ui| {
            ui.set_width(ui.available_width());
            let year = self.data.secs_between(today - chrono::Duration::days(364), today);
            let days = (0..365).filter(|i| self.data.secs_on(today - chrono::Duration::days(*i)) > 0).count();
            ui.horizontal(|ui| {
                ui.label(txt("Activity · last 12 months", 14.0, W::B, p.text));
                ui.label(txt(format!("· {} in {days} active days · streak {} (best {})", fmt_dur(year), self.data.streak(60), self.data.best_streak(60)), 12.0, W::R, p.subtext));
            });
            ui.add_space(6.0);
            heatmap(ui, &p, &self.data, goal);
        });
        ui.add_space(80.0);
    }
}

fn hour_bars(ui: &mut Ui, p: &crate::theme::Pal, sessions: &[Session], day: NaiveDate) {
    let mut mins = [0f32; 24];
    let begin = store::day_begin(day);
    for s in sessions {
        for h in 0..24 {
            let (hs, he) = (begin + h * 3600, begin + (h + 1) * 3600);
            let overlap = (s.start + s.secs).min(he) - s.start.max(hs);
            if overlap > 0 {
                mins[h as usize] += overlap as f32 / 60.0;
            }
        }
    }
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 170.0), Sense::hover());
    let plot = Rect::from_min_max(rect.min + vec2(28.0, 8.0), rect.max - vec2(0.0, 20.0));
    for m in [0.0, 30.0, 60.0] {
        let y = plot.bottom() - m / 60.0 * plot.height();
        ui.painter().line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], Stroke::new(1.0, p.stroke));
        ui.painter().text(pos2(plot.left() - 4.0, y), Align2::RIGHT_CENTER, format!("{m:.0}"), ui::font(9.5, W::R), p.subtext);
    }
    let slot = plot.width() / 24.0;
    for (h, m) in mins.iter().enumerate() {
        let cx = plot.left() + slot * (h as f32 + 0.5);
        let bh = (m / 60.0 * plot.height()).max(if *m > 0.0 { 2.0 } else { 0.0 });
        let r = Rect::from_min_max(pos2(cx - slot * 0.32, plot.bottom() - bh), pos2(cx + slot * 0.32, plot.bottom()));
        let hovered = resp.hover_pos().is_some_and(|q| (q.x - cx).abs() < slot / 2.0);
        ui.painter().rect_filled(r, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, alpha(p.accent, if hovered { 1.0 } else { 0.75 }));
        if h % 3 == 0 {
            ui.painter().text(pos2(cx, plot.bottom() + 10.0), Align2::CENTER_CENTER, hhmm(h as f32 * 60.0), ui::font(9.5, W::R), p.subtext);
        }
        if hovered && *m > 0.0 {
            ui.painter().text(pos2(cx, r.top() - 8.0), Align2::CENTER_CENTER, format!("{m:.0}m"), ui::font(11.0, W::B), p.text);
        }
    }
}

fn month_calendar(ui: &mut Ui, p: &crate::theme::Pal, sessions: &[Session], from: NaiveDate, to: NaiveDate, goal: i64) {
    let w = ui.available_width();
    let cw = w / 7.0;
    let ch = 58.0;
    let lead = from.weekday().num_days_from_monday() as i64;
    let weeks = ((lead + (to - from).num_days() + 1) as f32 / 7.0).ceil() as usize;
    let (rect, _) = ui.allocate_exact_size(vec2(w, 18.0 + weeks as f32 * ch), Sense::hover());
    for (i, d) in super::home::DAYS_SHORT.iter().enumerate() {
        ui.painter().text(pos2(rect.left() + cw * (i as f32 + 0.5), rect.top() + 8.0), Align2::CENTER_CENTER, *d, ui::font(10.5, W::S), p.subtext);
    }
    let today = store::today();
    let full = goal.max(3600) as f32;
    let mut day = from;
    while day <= to {
        let idx = lead + (day - from).num_days();
        let (col, row) = (idx % 7, idx / 7);
        let r = Rect::from_min_size(pos2(rect.left() + col as f32 * cw, rect.top() + 18.0 + row as f32 * ch), vec2(cw, ch)).shrink(3.0);
        let secs: i64 = sessions.iter().filter(|s| store::local_day(s.start) == day).map(|s| s.secs).sum();
        let fill = if secs > 0 { alpha(p.accent, 0.18 + 0.72 * (secs as f32 / full).min(1.0)) } else { Color32::from_white_alpha(if p.light { 60 } else { 6 }) };
        ui.painter().rect_filled(r, CornerRadius::same(8), fill);
        if day == today {
            ui.painter().rect_stroke(r, CornerRadius::same(8), Stroke::new(1.5, p.accent), eframe::egui::StrokeKind::Inside);
        }
        let strong = secs as f32 / full > 0.6;
        let fg = if strong { p.on_accent } else { p.text };
        ui.painter().text(r.left_top() + vec2(8.0, 10.0), Align2::LEFT_CENTER, day.day().to_string(), ui::font(11.0, W::S), if day > today { p.subtext } else { fg });
        if secs > 0 {
            ui.painter().text(r.center() + vec2(0.0, 8.0), Align2::CENTER_CENTER, hms(secs), ui::font(11.5, W::B), fg);
        }
        day = day.succ_opt().unwrap();
    }
}

fn start_finish(ui: &mut Ui, p: &crate::theme::Pal, sessions: &[Session], from: NaiveDate, to: NaiveDate) {
    let days: Vec<NaiveDate> = std::iter::successors(Some(from), |d| d.succ_opt().filter(|n| *n <= to)).collect();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 200.0), Sense::hover());
    let plot = Rect::from_min_max(rect.min + vec2(40.0, 8.0), rect.max - vec2(0.0, 18.0));
    for h in (0..=24).step_by(4) {
        let y = plot.top() + h as f32 / 24.0 * plot.height();
        ui.painter().line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], Stroke::new(1.0, p.stroke));
        ui.painter().text(pos2(plot.left() - 4.0, y), Align2::RIGHT_CENTER, hhmm(h as f32 * 60.0), ui::font(9.5, W::R), p.subtext);
    }
    let slot = plot.width() / days.len().max(1) as f32;
    for (i, d) in days.iter().enumerate() {
        let cx = plot.left() + slot * (i as f32 + 0.5);
        let ss: Vec<&Session> = sessions.iter().filter(|s| store::local_day(s.start) == *d).collect();
        if days.len() <= 7 || d.day() % 5 == 1 {
            let lab = if days.len() <= 7 { super::home::DAYS_SHORT[d.weekday().num_days_from_monday() as usize].to_string() } else { d.day().to_string() };
            ui.painter().text(pos2(cx, plot.bottom() + 9.0), Align2::CENTER_CENTER, lab, ui::font(9.5, W::R), p.subtext);
        }
        let (Some(a), Some(b)) = (ss.iter().map(|s| s.start).min(), ss.iter().map(|s| s.start + s.secs).max()) else { continue };
        let (ya, yb) = (plot.top() + day_minutes(a, *d) / 1440.0 * plot.height(), plot.top() + day_minutes(b, *d) / 1440.0 * plot.height());
        let w = (slot * 0.45).clamp(3.0, 18.0);
        let r = Rect::from_min_max(pos2(cx - w / 2.0, ya), pos2(cx + w / 2.0, yb.max(ya + 3.0)));
        ui.painter().rect_filled(r, CornerRadius::same((w / 2.0) as u8), alpha(p.accent, 0.35));
        // Actual study blocks inside the span.
        for s in &ss {
            let (y0, y1) = (plot.top() + day_minutes(s.start, *d) / 1440.0 * plot.height(), plot.top() + day_minutes(s.start + s.secs, *d) / 1440.0 * plot.height());
            ui.painter().rect_filled(Rect::from_min_max(pos2(cx - w / 2.0, y0), pos2(cx + w / 2.0, y1.max(y0 + 1.5))), CornerRadius::same(2), p.accent);
        }
        if resp.hover_pos().is_some_and(|q| (q.x - cx).abs() < slot / 2.0) {
            let text = format!("{}  {} – {}", day_title(*d), store::local(a).format("%H:%M"), store::local(b).format("%H:%M"));
            ui.painter().text(pos2(cx.clamp(plot.left() + 80.0, plot.right() - 80.0), plot.top() - 1.0), Align2::CENTER_CENTER, text, ui::font(11.0, W::B), p.text);
        }
    }
}

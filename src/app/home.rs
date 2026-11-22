//! Home: a desktop take on the classic study-timer layout. On the left, today's total, the current
//! session, and each subject with its own time and to-dos. On the right, a week strip and a timeline
//! of the selected day with every study block where it actually happened.

use super::{App, Page};
use crate::store::{self, DDay, Task};
use crate::theme::{alpha, rgb};
use crate::timer::{fmt_clock, fmt_dur, Mode, Phase};
use crate::ui::{self, txt, W};
use chrono::{Datelike, NaiveDate};
use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke, Ui};

pub(super) const DAYS_SHORT: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
pub(super) const MONTHS_SHORT: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const HOUR_H: f32 = 46.0;

pub(super) fn hms(secs: i64) -> String {
    let s = secs.max(0);
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

pub(super) fn day_title(d: NaiveDate) -> String {
    format!("{}, {} {}", DAYS_SHORT[d.weekday().num_days_from_monday() as usize], MONTHS_SHORT[d.month0() as usize], d.day())
}

/// "08:05" for a unix time.
pub(super) fn clock(ts: i64) -> String {
    store::local(ts).format("%H:%M").to_string()
}

impl App {
    /// Focus seconds in the current (unsaved) block, if it counts as study time.
    pub(super) fn running_secs(&self) -> i64 {
        if self.timer.is_focus() && !self.timer.is_idle() { self.timer.elapsed().as_secs() as i64 } else { 0 }
    }

    pub(super) fn live_today(&self) -> i64 {
        self.data.secs_on(store::today()) + self.running_secs()
    }

    fn subject_on(&self, id: Option<u64>, day: NaiveDate) -> i64 {
        let saved: i64 = self.data.sessions.iter().filter(|s| s.subject_id == id && store::local_day(s.start) == day).map(|s| s.secs).sum();
        saved + if self.subject == id && day == store::today() { self.running_secs() } else { 0 }
    }

    /// One tap on a subject starts studying it (or pauses it if it's the one running).
    pub(super) fn tap_subject(&mut self, id: Option<u64>) {
        if self.subject == id && self.timer.is_focus() && !self.timer.is_idle() {
            self.toggle_timer();
            return;
        }
        let ev = self.timer.stop();
        self.handle(ev);
        if self.timer.mode == Mode::Pomodoro && self.timer.phase != Phase::Focus {
            let ev = self.timer.set_phase(Phase::Focus);
            self.handle(ev);
        }
        self.subject = id;
        if self.data.task(self.task).is_some_and(|t| t.subject_id != id) {
            self.task = None;
        }
        self.idle_notice = None;
        self.timer.start();
    }

    fn todos_on(&self, day: NaiveDate) -> Vec<Task> {
        let key = store::day_key(day);
        self.data.tasks.iter().filter(|t| t.day.as_deref() == Some(key.as_str())).cloned().collect()
    }

    fn add_todo(&mut self, subject: Option<u64>, day: NaiveDate, title: &str) {
        if title.trim().is_empty() {
            return;
        }
        let id = self.data.new_id();
        self.data.tasks.push(Task {
            id,
            subject_id: subject,
            title: title.trim().into(),
            est_pomos: 1,
            done_pomos: 0,
            done: false,
            created: store::now(),
            day: Some(store::day_key(day)),
        });
        self.dirty = true;
    }

    // ------------------------------------------------------------ page

    pub(super) fn page_home(&mut self, ui: &mut Ui) {
        let gap = 16.0;
        let total = ui.available_width();
        let left_w = ((total - gap) * 0.47).floor();
        let right_w = total - gap - left_w;
        let height = ui.available_height().max(560.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.allocate_ui_with_layout(vec2(left_w, height), Layout::top_down(Align::Min), |ui| {
                ui.set_width(left_w);
                ui.spacing_mut().item_spacing.x = 8.0;
                egui::ScrollArea::vertical().id_salt("home.left").max_height(height).auto_shrink([false, true]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                    self.timer_panel(ui, height);
                });
            });
            ui.allocate_ui_with_layout(vec2(right_w, height), Layout::top_down(Align::Min), |ui| {
                ui.set_width(right_w);
                ui.spacing_mut().item_spacing.x = 8.0;
                self.planner_panel(ui, height);
            });
        });
        self.countdowns_window(ui.ctx());
    }

    // ------------------------------------------------------------ left: timer, subjects, to-dos

    fn timer_panel(&mut self, ui: &mut Ui, height: f32) {
        let p = self.pal;
        let d = self.durations();
        let today = store::today();
        let live = self.live_today();
        let goal = self.data.settings.daily_goal_min as i64 * 60;
        ui::card(ui, &p, |ui| {
            ui.set_width(ui.available_width());
            // Same height as the planner next to it (both cards end at the same line).
            ui.set_min_height(height - 48.0);
            // Date and countdowns.
            ui.horizontal(|ui| {
                ui.label(txt(day_title(today), 14.0, W::S, p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui::soft_button(ui, &p, "D-Day", vec2(0.0, ui::BTN_H)).on_hover_text("Manage countdowns").clicked() {
                        self.show_ddays = true;
                    }
                    let mut ddays: Vec<(String, i64)> = self
                        .data
                        .ddays
                        .iter()
                        .filter_map(|d| NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok().map(|dt| (d.name.clone(), (dt - today).num_days())))
                        .filter(|d| d.1 >= 0)
                        .collect();
                    ddays.sort_by_key(|d| d.1);
                    for (name, n) in ddays.iter().take(2).rev() {
                        let label = if *n == 0 { format!("{name} D-DAY") } else { format!("{name} D-{n}") };
                        let col = if *n <= 7 { p.danger } else { p.accent };
                        egui::Frame::new().fill(alpha(col, 0.14)).corner_radius(255).inner_margin(egui::Margin::symmetric(10, 3)).show(ui, |ui| {
                            ui.label(txt(label, 11.0, W::B, col));
                        });
                    }
                });
            });
            ui.add_space(6.0);
            // Today's total.
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui::section(ui, &p, "Today");
                    ui.label(txt(hms(live), 52.0, W::B, p.text));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(76.0, 76.0), Sense::hover());
                    let frac = if goal > 0 { live as f32 / goal as f32 } else { 0.0 };
                    ui::ring(ui.painter(), r.center(), 31.0, 7.0, frac, p.track, p.accent);
                    ui.painter().text(r.center() - vec2(0.0, 5.0), Align2::CENTER_CENTER, format!("{}%", (frac * 100.0).round().min(999.0)), ui::font(14.0, W::B), p.text);
                    ui.painter().text(r.center() + vec2(0.0, 11.0), Align2::CENTER_CENTER, format!("of {}", fmt_dur(goal)), ui::font(9.0, W::S), p.subtext);
                });
            });
            // Current session.
            let running = !self.timer.is_idle();
            egui::Frame::new().fill(Color32::from_white_alpha(if p.light { 70 } else { 8 })).corner_radius(14).inner_margin(12).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let timed = self.timer.target_secs(&d).is_some();
                    if timed {
                        let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                        pie_clock(ui.painter(), r.center(), 19.0, 1.0 - self.timer.progress(&d), &p, false);
                    }
                    ui.vertical(|ui| {
                        let label = if !running {
                            "Tap ▶ on a subject to start".to_string()
                        } else if self.idle_notice.is_some() {
                            "Paused while you were away".into()
                        } else {
                            let what = if self.timer.mode == Mode::Pomodoro { self.timer.phase.label() } else { self.timer.mode.label() };
                            format!("{what} · {}", self.data.subject_name(self.subject))
                        };
                        ui.label(txt(label, 12.0, W::S, p.subtext));
                        let big = if timed { format!("{} left", fmt_clock(self.timer.display_secs(&d))) } else { hms(self.timer.elapsed().as_secs() as i64) };
                        ui.label(txt(big, 22.0, W::B, if self.timer.running { p.accent } else { p.text }));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if running {
                            if ui::icon_button(ui, &p, "stop", "", vec2(36.0, 36.0), false).on_hover_text("Finish & save").clicked() {
                                self.stop_timer();
                            }
                            let icon = if self.timer.running { "pause" } else { "play" };
                            if ui::icon_button(ui, &p, icon, "", vec2(36.0, 36.0), true).clicked() {
                                self.toggle_timer();
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(txt("Mode", 11.0, W::S, p.subtext));
                    for m in [Mode::Stopwatch, Mode::Pomodoro, Mode::Countdown] {
                        let short = match m {
                            Mode::Stopwatch => "Stopwatch",
                            Mode::Pomodoro => "Pomodoro",
                            Mode::Countdown => "Timer",
                        };
                        if ui::chip(ui, &p, short, self.timer.mode == m).clicked() && self.timer.mode != m {
                            let ev = self.timer.set_mode(m);
                            self.handle(ev);
                            self.idle_notice = None;
                        }
                    }
                });
                if let Some(idle) = self.idle_notice {
                    ui.horizontal(|ui| {
                        ui.label(txt(format!("Away for about {} (not counted).", fmt_dur(idle as i64)), 11.0, W::S, p.text));
                        if ui::soft_button(ui, &p, "Count it", vec2(0.0, ui::BTN_H)).clicked() {
                            self.timer.add_time(std::time::Duration::from_secs(idle));
                            self.idle_notice = None;
                            self.timer.start();
                        }
                    });
                }
            });
            ui.add_space(8.0);

            // Subjects with their to-dos for today.
            let todos = self.todos_on(today);
            let mut groups: Vec<(Option<u64>, String, u32)> =
                self.data.subjects.iter().filter(|s| !s.archived).map(|s| (Some(s.id), s.name.clone(), s.color)).collect();
            if todos.iter().any(|t| t.subject_id.is_none()) || self.todo_for == Some(None) {
                groups.push((None, "Other".into(), 0x8B91A1));
            }
            let mut tap = None;
            for (id, name, color) in groups {
                let secs = self.subject_on(id, today);
                let active = self.subject == id && self.timer.running && self.timer.is_focus();
                let mine: Vec<Task> = todos.iter().filter(|t| t.subject_id == id).cloned().collect();
                let done = mine.iter().filter(|t| t.done).count();
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
                let hover = resp.hovered();
                let bg = if active { alpha(rgb(color), 0.16) } else if hover { Color32::from_white_alpha(if p.light { 60 } else { 10 }) } else { Color32::TRANSPARENT };
                ui.painter().rect_filled(rect, CornerRadius::same(12), bg);
                let bc = pos2(rect.left() + 22.0, rect.center().y);
                ui.painter().circle_filled(bc, 15.0, if active { rgb(color) } else { alpha(rgb(color), 0.25) });
                ui::paint_icon(ui.painter(), if active { "pause" } else { "play" }, bc + vec2(if active { 0.0 } else { 1.0 }, 0.0), 10.0, if active { Color32::WHITE } else { rgb(color) });
                ui.painter().text(pos2(rect.left() + 48.0, rect.center().y), Align2::LEFT_CENTER, &name, ui::font(14.5, W::S), p.text);
                if !mine.is_empty() {
                    let name_w = ui.painter().layout_no_wrap(name.clone(), ui::font(14.5, W::S), p.text).size().x;
                    ui.painter().text(pos2(rect.left() + 56.0 + name_w, rect.center().y), Align2::LEFT_CENTER, format!("{done}/{}", mine.len()), ui::font(11.0, W::S), p.subtext);
                }
                ui.painter().text(pos2(rect.right() - 44.0, rect.center().y), Align2::RIGHT_CENTER, hms(secs), ui::font(15.0, W::B), if active { rgb(color) } else { p.text });
                // "+" adds a to-do under this subject.
                let plus = Rect::from_center_size(pos2(rect.right() - 20.0, rect.center().y), vec2(26.0, 26.0));
                let plus_resp = ui.interact(plus, ui.scope_id().with(("todo.add", id)), Sense::click());
                if hover || plus_resp.hovered() {
                    ui.painter().circle_filled(plus.center(), 12.0, Color32::from_white_alpha(if plus_resp.hovered() { 40 } else { 16 }));
                }
                ui.painter().text(plus.center(), Align2::CENTER_CENTER, "+", ui::font(17.0, W::S), p.subtext);
                if plus_resp.on_hover_text("Add a to-do").clicked() {
                    self.todo_for = Some(id);
                    self.todo_text.clear();
                } else if resp.on_hover_text(if active { "Pause" } else { "Study this subject" }).clicked() {
                    tap = Some(id);
                }
                for t in &mine {
                    self.todo_row(ui, t);
                }
                if self.todo_for == Some(id) {
                    ui.horizontal(|ui| {
                        ui.add_space(44.0);
                        let resp = ui.add(egui::TextEdit::singleline(&mut self.todo_text).hint_text("New to-do, press Enter").desired_width(ui.available_width() - 8.0));
                        resp.request_focus();
                        if resp.lost_focus() {
                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let text = std::mem::take(&mut self.todo_text);
                                self.add_todo(id, today, &text);
                            }
                            self.todo_for = None;
                        }
                    });
                }
            }
            if let Some(id) = tap {
                self.tap_subject(id);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui::soft_button(ui, &p, "+  Subject", vec2(0.0, ui::BTN_H)).clicked() {
                    self.page = Page::Subjects;
                }
                if ui::soft_button(ui, &p, "+  To-do", vec2(0.0, ui::BTN_H)).on_hover_text("A to-do without a subject").clicked() {
                    self.todo_for = Some(None);
                    self.todo_text.clear();
                }
                let open = todos.iter().filter(|t| !t.done).count();
                if open > 0 {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::soft_button(ui, &p, &format!("Move {open} to tomorrow"), vec2(0.0, ui::BTN_H)).on_hover_text("Move today's unfinished to-dos to tomorrow").clicked() {
                            let (tk, tm) = (store::day_key(today), store::day_key(today.succ_opt().unwrap()));
                            for t in self.data.tasks.iter_mut().filter(|t| !t.done && t.day.as_deref() == Some(tk.as_str())) {
                                t.day = Some(tm.clone());
                            }
                            self.dirty = true;
                            self.toast(format!("Moved {open} to-dos to tomorrow"));
                        }
                    });
                }
            });
        });
    }

    fn todo_row(&mut self, ui: &mut Ui, t: &Task) {
        let p = self.pal;
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
        let c = pos2(rect.left() + 56.0, rect.center().y);
        if t.done {
            ui.painter().circle_filled(c, 8.0, p.accent);
            ui.painter().add(egui::Shape::line(vec![c + vec2(-4.0, 0.0), c + vec2(-1.0, 3.0), c + vec2(4.0, -3.0)], Stroke::new(1.8, p.on_accent)));
        } else {
            ui.painter().circle_stroke(c, 7.5, Stroke::new(1.5, if resp.hovered() { p.accent } else { p.subtext }));
        }
        let col = if t.done { p.subtext } else { p.text };
        let galley = ui.painter().layout_no_wrap(t.title.clone(), ui::font(13.0, W::R), col);
        let gw = galley.size().x;
        ui.painter().galley(pos2(rect.left() + 72.0, rect.center().y - galley.size().y / 2.0), galley, col);
        if t.done {
            let y = rect.center().y;
            ui.painter().line_segment([pos2(rect.left() + 72.0, y), pos2(rect.left() + 72.0 + gw, y)], Stroke::new(1.0, p.subtext));
        }
        let del = Rect::from_center_size(pos2(rect.right() - 20.0, rect.center().y), vec2(22.0, 22.0));
        let del_resp = ui.interact(del, ui.scope_id().with(("todo.del", t.id)), Sense::click());
        if resp.hovered() || del_resp.hovered() {
            ui.painter().text(del.center(), Align2::CENTER_CENTER, "×", ui::font(15.0, W::S), if del_resp.hovered() { p.danger } else { p.subtext });
        }
        if del_resp.clicked() {
            self.data.tasks.retain(|x| x.id != t.id);
            self.dirty = true;
        } else if resp.clicked() {
            if let Some(x) = self.data.tasks.iter_mut().find(|x| x.id == t.id) {
                x.done = !x.done;
            }
            self.dirty = true;
        }
    }

    // ------------------------------------------------------------ right: week strip + timeline

    fn planner_panel(&mut self, ui: &mut Ui, height: f32) {
        let p = self.pal;
        let today = store::today();
        let sel = self.planner_day;
        let top = ui.cursor().top();
        ui::card(ui, &p, |ui| {
            ui.set_width(ui.available_width());
            // Week strip.
            let monday = sel - chrono::Duration::days(sel.weekday().num_days_from_monday() as i64);
            ui.horizontal(|ui| {
                ui.label(txt(format!("{} {}", MONTHS_SHORT[sel.month0() as usize], sel.year()), 14.0, W::B, p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui::soft_button(ui, &p, "›", vec2(ui::BTN_H, ui::BTN_H)).on_hover_text("Next week").clicked() {
                        self.planner_day = sel + chrono::Duration::days(7);
                        self.planner_scroll = true;
                    }
                    if sel != today && ui::soft_button(ui, &p, "Today", vec2(0.0, ui::BTN_H)).clicked() {
                        self.planner_day = today;
                        self.planner_scroll = true;
                    }
                    if ui::soft_button(ui, &p, "‹", vec2(ui::BTN_H, ui::BTN_H)).on_hover_text("Previous week").clicked() {
                        self.planner_day = sel - chrono::Duration::days(7);
                        self.planner_scroll = true;
                    }
                });
            });
            let w = ui.available_width();
            let (strip, _) = ui.allocate_exact_size(vec2(w, 58.0), Sense::hover());
            let cw = w / 7.0;
            for i in 0..7 {
                let day = monday + chrono::Duration::days(i);
                let r = Rect::from_min_size(pos2(strip.left() + i as f32 * cw, strip.top()), vec2(cw, 58.0)).shrink2(vec2(3.0, 0.0));
                let resp = ui.interact(r, ui.scope_id().with(("planner.day", i)), Sense::click());
                let is_sel = day == sel;
                if is_sel {
                    ui.painter().rect_filled(r, CornerRadius::same(12), p.accent_soft);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(12), Color32::from_white_alpha(if p.light { 60 } else { 10 }));
                }
                let col = if day == today { p.accent } else if is_sel { p.text } else { p.subtext };
                ui.painter().text(pos2(r.center().x, r.top() + 12.0), Align2::CENTER_CENTER, DAYS_SHORT[i as usize], ui::font(10.5, W::S), p.subtext);
                ui.painter().text(pos2(r.center().x, r.top() + 29.0), Align2::CENTER_CENTER, day.day().to_string(), ui::font(16.0, W::B), col);
                let secs = self.data.secs_on(day) + if day == today { self.running_secs() } else { 0 };
                if secs > 0 {
                    ui.painter().text(pos2(r.center().x, r.top() + 47.0), Align2::CENTER_CENTER, fmt_dur(secs), ui::font(9.5, W::S), p.accent);
                }
                if resp.clicked() {
                    self.planner_day = day;
                    self.planner_scroll = true;
                }
            }
            ui.add_space(4.0);
            // Selected day summary.
            let begin = store::day_begin(sel);
            let mut blocks: Vec<(i64, i64, Option<u64>)> =
                self.data.sessions.iter().filter(|s| store::local_day(s.start) == sel).map(|s| (s.start, s.start + s.secs, s.subject_id)).collect();
            if sel == today && self.timer.is_focus() {
                if let Some(bs) = self.timer.block_start {
                    blocks.push((bs, store::now(), self.subject));
                }
            }
            blocks.sort_by_key(|b| b.0);
            let total: i64 = blocks.iter().map(|b| b.1 - b.0).sum();
            ui.horizontal(|ui| {
                ui.label(txt(day_title(sel), 13.0, W::S, p.text));
                ui.label(txt(fmt_dur(total), 13.0, W::B, p.accent));
                if let (Some(first), Some(last)) = (blocks.first(), blocks.iter().map(|b| b.1).max()) {
                    ui.label(txt(format!("· {} – {}", clock(first.0), clock(last)), 12.0, W::R, p.subtext));
                }
            });
            // To-dos planned for another day (today's are on the left).
            if sel != today {
                let todos = self.todos_on(sel);
                for t in &todos {
                    self.todo_row(ui, t);
                }
                ui.horizontal(|ui| {
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.plan_text)
                            .hint_text(format!("Plan a to-do for {}", day_title(sel)))
                            .desired_width(ui.available_width() - 150.0),
                    );
                    ui::subject_combo(ui, "plan.subject", &self.data, &mut self.plan_subject, 130.0);
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let text = std::mem::take(&mut self.plan_text);
                        let subject = self.plan_subject;
                        self.add_todo(subject, sel, &text);
                    }
                });
            }
            ui.add_space(4.0);

            // Timeline.
            let used = ui.cursor().top() - top;
            let tl_h = (height - used - 30.0).max(260.0);
            let mut area = egui::ScrollArea::vertical().id_salt("planner.timeline").max_height(tl_h).auto_shrink(false);
            if std::mem::take(&mut self.planner_scroll) {
                let anchor = if sel == today { store::now() - 2 * 3600 } else { blocks.first().map_or(begin + 4 * 3600, |b| b.0 - 1800) };
                area = area.vertical_scroll_offset((((anchor - begin) as f32 / 3600.0) * HOUR_H).max(0.0));
            }
            area.show(ui, |ui| {
                let w = ui.available_width();
                let (rect, _) = ui.allocate_exact_size(vec2(w, 24.0 * HOUR_H + 8.0), Sense::hover());
                let painter = ui.painter();
                let x0 = rect.left() + 50.0;
                let start_hour = store::day_start_hour();
                let grid = if p.light { Color32::from_black_alpha(16) } else { Color32::from_white_alpha(10) };
                for h in 0..=24u32 {
                    let y = rect.top() + 4.0 + h as f32 * HOUR_H;
                    painter.line_segment([pos2(x0, y), pos2(rect.right(), y)], Stroke::new(1.0, grid));
                    if h < 24 {
                        painter.text(pos2(rect.left() + 4.0, y), Align2::LEFT_CENTER, format!("{:02}:00", (start_hour + h) % 24), ui::font(10.0, W::S), p.subtext);
                    }
                }
                let to_y = |ts: i64| rect.top() + 4.0 + (ts - begin) as f32 / 3600.0 * HOUR_H;
                for (s, e, subj) in &blocks {
                    let r = Rect::from_min_max(pos2(x0 + 4.0, to_y(*s)), pos2(rect.right() - 6.0, to_y(*e).max(to_y(*s) + 3.0)));
                    let c = rgb(self.data.subject_color(*subj));
                    painter.rect_filled(r, CornerRadius::same(6), alpha(c, 0.78));
                    painter.rect_filled(Rect::from_min_size(r.min, vec2(4.0, r.height())), CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 }, c);
                    if r.height() >= 17.0 {
                        let label = format!("{}  {}  ·  {}–{}", self.data.subject_name(*subj), fmt_dur(e - s), clock(*s), clock(*e));
                        painter.with_clip_rect(r).text(pos2(r.left() + 12.0, r.top() + 9.0), Align2::LEFT_CENTER, label, ui::font(11.5, W::S), Color32::WHITE);
                    }
                }
                if sel == today {
                    let y = to_y(store::now());
                    painter.line_segment([pos2(x0, y), pos2(rect.right(), y)], Stroke::new(1.5, p.danger));
                    painter.circle_filled(pos2(x0, y), 4.0, p.danger);
                    painter.text(pos2(rect.left() + 4.0, y), Align2::LEFT_CENTER, clock(store::now()), ui::font(10.0, W::B), p.danger);
                }
            });
        });
    }

    fn countdowns_window(&mut self, ctx: &egui::Context) {
        if !self.show_ddays {
            return;
        }
        let p = self.pal;
        let today = store::today();
        let mut open = true;
        egui::Window::new("Countdowns").open(&mut open).collapsible(false).resizable(false).default_width(340.0).show(ctx, |ui| {
            let mut items: Vec<(DDay, i64)> = self
                .data
                .ddays
                .iter()
                .filter_map(|d| NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok().map(|dt| (d.clone(), (dt - today).num_days())))
                .collect();
            items.sort_by_key(|i| (i.1 < 0, i.1.abs()));
            if items.is_empty() {
                ui.label(txt("Add an exam or deadline and it shows up on Home.", 12.0, W::R, p.subtext));
            }
            let mut remove = None;
            for (d, n) in &items {
                ui.horizontal(|ui| {
                    let badge = match n {
                        0 => "D-DAY".to_string(),
                        n if *n > 0 => format!("D-{n}"),
                        n => format!("D+{}", -n),
                    };
                    ui.label(txt(badge, 14.0, W::B, if *n >= 0 && *n <= 7 { p.danger } else { p.accent }));
                    ui.label(txt(format!("{}  ·  {}", d.name, d.date), 13.0, W::S, p.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(txt("×", 15.0, W::S, p.subtext)).frame(false)).on_hover_text("Remove").clicked() {
                            remove = Some(d.id);
                        }
                    });
                });
            }
            if let Some(id) = remove {
                self.data.ddays.retain(|d| d.id != id);
                self.dirty = true;
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.dday_name).hint_text("e.g. Finals").desired_width(130.0));
                ui.add(egui::TextEdit::singleline(&mut self.dday_date).hint_text("YYYY-MM-DD").desired_width(96.0));
                if ui::accent_button(ui, &p, "Add", vec2(0.0, ui::BTN_H)).clicked() {
                    if NaiveDate::parse_from_str(self.dday_date.trim(), "%Y-%m-%d").is_ok() && !self.dday_name.trim().is_empty() {
                        let id = self.data.new_id();
                        self.data.ddays.push(DDay { id, name: self.dday_name.trim().into(), date: self.dday_date.trim().into() });
                        self.dday_name.clear();
                        self.dday_date.clear();
                        self.dirty = true;
                    } else {
                        self.toast("Enter a name and a date (YYYY-MM-DD)");
                    }
                }
            });
        });
        if !open {
            self.show_ddays = false;
        }
    }
}

/// Analog "time left" clock: a wedge for the remaining fraction, starting at 12 o'clock.
pub(super) fn pie_clock(painter: &egui::Painter, c: egui::Pos2, r: f32, remaining: f32, p: &crate::theme::Pal, ticks: bool) {
    painter.circle_filled(c, r, Color32::from_white_alpha(if p.light { 150 } else { 22 }));
    let rem = remaining.clamp(0.0, 1.0);
    if rem > 0.001 {
        let start = -std::f32::consts::FRAC_PI_2;
        let arc = ui::arc_points(c, r * 0.92, start, start + rem * std::f32::consts::TAU);
        // One mesh fan (no seams between slices), then an anti-aliased outline for a clean edge.
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(c, p.accent);
        for q in &arc {
            mesh.colored_vertex(*q, p.accent);
        }
        for i in 1..arc.len() as u32 {
            mesh.add_triangle(0, i, i + 1);
        }
        painter.add(egui::Shape::mesh(mesh));
        let mut outline = vec![c];
        outline.extend(arc.iter().copied());
        painter.add(egui::Shape::Path(egui::epaint::PathShape { points: outline, closed: true, fill: Color32::TRANSPARENT, stroke: Stroke::new(1.0, p.accent).into() }));
    }
    if ticks {
        for i in 0..60 {
            let a = i as f32 / 60.0 * std::f32::consts::TAU;
            let d = vec2(a.cos(), a.sin());
            let (inner, w) = if i % 5 == 0 { (0.84, 2.0) } else { (0.92, 1.0) };
            painter.line_segment([c + d * r * inner, c + d * r * 0.99], Stroke::new(w, Color32::from_white_alpha(if i % 5 == 0 { 160 } else { 70 })));
        }
    }
    painter.circle_filled(c, r * 0.08, Color32::from_white_alpha(220));
    painter.circle_stroke(c, r, Stroke::new(1.0, Color32::from_white_alpha(40)));
}

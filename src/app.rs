//! The application: main window pages, the floating mini timer, and wiring between them.

use crate::mascot::{self, Mood};
use crate::store::{self, Data, Session, Subject, Task};
use crate::sys::{self, Hot, Hotkeys};
use crate::theme::{self, alpha, rgb, Pal};
use crate::timer::{fmt_clock, fmt_dur, Durations, Event, Mode, Timer};
use crate::ui::{self, txt, W};
use chrono::{Datelike, Local, NaiveDate, NaiveTime, TimeZone};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke, StrokeKind, Ui, ViewportBuilder,
    ViewportCommand, ViewportId,
};
use std::time::{Duration, Instant};

mod focus;
mod home;
mod insights;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Focus,
    Tasks,
    Subjects,
    History,
    Stats,
    Settings,
}

const PAGES: &[(Page, &str, &str)] = &[
    (Page::Home, "home", "Home"),
    (Page::Focus, "timer", "Focus mode"),
    (Page::Tasks, "tasks", "Tasks"),
    (Page::Subjects, "book", "Subjects"),
    (Page::History, "history", "History"),
    (Page::Stats, "stats", "Insights"),
    (Page::Settings, "gear", "Settings"),
];

struct ManualForm {
    date: String,
    time: String,
    minutes: u32,
    subject: Option<u64>,
    note: String,
    error: String,
}

struct SessionEdit {
    id: u64,
    subject: Option<u64>,
    minutes: u32,
    note: String,
}

pub struct App {
    data: Data,
    pal: Pal,
    theme_key: (String, String),
    timer: Timer,
    subject: Option<u64>,
    task: Option<u64>,
    page: Page,
    dirty: bool,
    last_save: Instant,
    hotkeys: Hotkeys,
    /// Seconds of idle time removed from the block, waiting for the user's choice.
    idle_notice: Option<u64>,
    last_idle_check: Instant,
    say: Option<(Mood, String, Instant)>,
    tip: usize,
    tip_at: Instant,
    toast: Option<(String, Instant)>,
    // Forms
    new_task: String,
    new_task_subject: Option<u64>,
    new_task_est: u32,
    show_done_tasks: bool,
    new_subject: String,
    new_subject_color: u32,
    manual: ManualForm,
    edit: Option<SessionEdit>,
    confirm_delete: Option<u64>,
    shot: Option<crate::shot::Shot>,
    cat: crate::cat::Cat,
    backdrop: crate::scene::Backdrop,
    dday_name: String,
    dday_date: String,
    last_bubble: f64,
    planner_day: NaiveDate,
    planner_scroll: bool,
    /// Subject whose to-do field is open (Some(None) = "Other").
    todo_for: Option<Option<u64>>,
    todo_text: String,
    plan_text: String,
    plan_subject: Option<u64>,
    show_ddays: bool,
    ins_period: insights::Period,
    ins_anchor: NaiveDate,
}

fn float_id() -> ViewportId {
    ViewportId::from_hash_of("studytracker.float")
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, data: Data) -> Self {
        ui::install_fonts(&cc.egui_ctx);
        store::set_day_start(data.settings.day_start_hour);
        let pal = theme::build(&data.settings.scheme, &data.settings.accent);
        theme::apply(&cc.egui_ctx, &pal);
        let first_subject = data.subjects.iter().find(|s| !s.archived).map(|s| s.id);
        let dev_shot = std::env::var_os("STUDYTRACKER_SHOT").is_some();
        let hotkeys = if data.settings.hotkeys && !dev_shot { Hotkeys::register() } else { Hotkeys::none() };
        // STUDYTRACKER_DEMO_RUN=<secs>: start with a running focus block (screenshots).
        let mut timer = Timer::default();
        if let Some(secs) = std::env::var("STUDYTRACKER_DEMO_RUN").ok().and_then(|s| s.parse().ok()) {
            timer.start();
            timer.add_time(Duration::from_secs(secs));
        }
        Self {
            theme_key: (data.settings.scheme.clone(), data.settings.accent.clone()),
            pal,
            timer,
            subject: first_subject,
            task: None,
            page: start_page(),
            dirty: false,
            last_save: Instant::now(),
            hotkeys,
            idle_notice: None,
            last_idle_check: Instant::now(),
            say: None,
            tip: 0,
            tip_at: Instant::now(),
            toast: None,
            new_task: String::new(),
            new_task_subject: first_subject,
            new_task_est: 2,
            show_done_tasks: false,
            new_subject: String::new(),
            new_subject_color: store::SUBJECT_COLORS[0],
            manual: ManualForm {
                date: store::today().format("%Y-%m-%d").to_string(),
                time: Local::now().format("%H:%M").to_string(),
                minutes: 30,
                subject: first_subject,
                note: String::new(),
                error: String::new(),
            },
            edit: None,
            confirm_delete: None,
            shot: crate::shot::Shot::from_env(),
            cat: demo_cat(),
            backdrop: crate::scene::Backdrop::default(),
            dday_name: String::new(),
            dday_date: String::new(),
            last_bubble: -30.0,
            planner_day: store::today(),
            planner_scroll: true,
            todo_for: None,
            todo_text: String::new(),
            plan_text: String::new(),
            plan_subject: first_subject,
            show_ddays: false,
            ins_period: insights::Period::Week,
            ins_anchor: store::today(),
            data,
        }
    }

    fn durations(&self) -> Durations {
        let s = &self.data.settings;
        Durations { focus: s.focus_min, short: s.short_min, long: s.long_min, long_every: s.long_every, countdown: s.countdown_min }
    }

    fn save_now(&mut self) {
        if self.shot.is_some() {
            return; // screenshot runs never write data
        }
        if let Err(e) = store::save(&self.data) {
            self.toast(format!("Couldn't save: {e}"));
        }
        self.dirty = false;
        self.last_save = Instant::now();
    }

    fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }

    fn say(&mut self, mood: Mood, s: impl Into<String>, secs: u64) {
        self.say = Some((mood, s.into(), Instant::now() + Duration::from_secs(secs)));
    }

    fn handle(&mut self, ev: Option<Event>) {
        let Some(ev) = ev else { return };
        match ev {
            Event::FocusEnded { start, secs, mode, completed } => {
                let id = self.data.new_id();
                self.data.sessions.push(Session {
                    id,
                    subject_id: self.subject,
                    task_id: self.task,
                    start,
                    secs,
                    note: String::new(),
                    kind: mode.kind().into(),
                });
                if completed && mode == Mode::Pomodoro {
                    if let Some(t) = self.data.tasks.iter_mut().find(|t| Some(t.id) == self.task) {
                        t.done_pomos += 1;
                    }
                }
                self.dirty = true;
                let total = self.data.secs_on(store::today());
                if completed {
                    if self.data.settings.sound {
                        sys::chime();
                    }
                    self.say(
                        Mood::Celebrate,
                        format!("Session done! +{}. {} today. Nice work!", fmt_dur(secs), fmt_dur(total)),
                        10,
                    );
                } else {
                    self.say(Mood::Happy, format!("Saved {} of study. {} today.", fmt_dur(secs), fmt_dur(total)), 8);
                }
                let goal = self.data.settings.daily_goal_min as i64 * 60;
                if goal > 0 && total >= goal && total - secs < goal {
                    self.say(Mood::Celebrate, "Daily goal reached! You were great today!", 14);
                }
            }
            Event::BreakEnded => {
                if self.data.settings.sound {
                    sys::chime();
                }
                self.say(Mood::Normal, "Break's over. Ready to focus again?", 10);
            }
        }
    }

    fn toggle_timer(&mut self) {
        self.idle_notice = None;
        self.timer.toggle();
    }

    fn stop_timer(&mut self) {
        self.idle_notice = None;
        let ev = self.timer.stop();
        self.handle(ev);
    }

    fn skip(&mut self) {
        self.idle_notice = None;
        let d = self.durations();
        let ev = self.timer.skip(&d);
        self.handle(ev);
    }

    fn mood_and_line(&mut self) -> (Mood, String) {
        if let Some((m, s, until)) = &self.say {
            if Instant::now() < *until {
                return (*m, s.clone());
            }
            self.say = None;
        }
        let tips = mascot::advice(&self.data, &self.timer, self.idle_notice.is_some());
        if self.tip_at.elapsed() > Duration::from_secs(40) {
            self.tip += 1;
            self.tip_at = Instant::now();
        }
        tips.get(self.tip % tips.len().max(1)).cloned().unwrap_or((Mood::Normal, String::new()))
    }

    // ---------------------------------------------------------------- frame logic

    fn tick(&mut self, ctx: &egui::Context) {
        store::set_day_start(self.data.settings.day_start_hour);
        for h in self.hotkeys.poll() {
            match h {
                Hot::ToggleTimer => self.toggle_timer(),
                Hot::ToggleFloat => {
                    self.data.settings.float_visible = !self.data.settings.float_visible;
                    self.dirty = true;
                }
                Hot::Skip => self.skip(),
            }
        }
        let d = self.durations();
        let s = &self.data.settings;
        let ev = self.timer.tick(&d, s.auto_breaks, s.auto_focus);
        self.handle(ev);

        // Idle detection: once a second while focusing.
        if self.last_idle_check.elapsed() >= Duration::from_secs(1) {
            self.last_idle_check = Instant::now();
            let s = &self.data.settings;
            if s.idle_enabled && self.timer.running && self.timer.is_focus() {
                let idle = sys::idle_secs();
                if idle >= s.idle_min.max(1) as u64 * 60 {
                    let removed = self.timer.remove_time(Duration::from_secs(idle));
                    self.timer.pause();
                    self.idle_notice = Some(removed.as_secs());
                    if s.sound {
                        sys::chime();
                    }
                }
            }
        }

        if self.dirty && self.last_save.elapsed() > Duration::from_millis(800) {
            self.save_now();
        }
        ctx.request_repaint_after(Duration::from_millis(if self.timer.running { 250 } else { 1000 }));
        if self.data.settings.float_visible {
            // Keep drawing the mini timer while the main window is minimized.
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::RequestPaintWhileHidden);
        }
    }

    // ---------------------------------------------------------------- sidebar

    fn sidebar(&mut self, ui: &mut Ui) {
        let p = self.pal;
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            mascot::draw(ui.painter(), r.center(), 40.0, Mood::Normal, ui.input(|i| i.time));
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(txt("StudyTracker", 17.0, W::B, p.text));
                ui.label(txt("Study with Meng", 11.0, W::R, p.subtext));
            });
        });
        ui.add_space(18.0);
        egui::Panel::bottom("nav.bottom").frame(egui::Frame::NONE).resizable(false).show_separator_line(false).show(ui, |ui| self.sidebar_bottom(ui));
        for (page, icon, label) in PAGES {
            let sel = self.page == *page;
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
            let bg = if sel { p.accent_soft } else if resp.hovered() { Color32::from_white_alpha(if p.light { 80 } else { 12 }) } else { Color32::TRANSPARENT };
            ui.painter().rect_filled(rect, CornerRadius::same(10), bg);
            let col = if sel { p.accent } else { p.text };
            ui::paint_icon(ui.painter(), icon, pos2(rect.left() + 21.0, rect.center().y), 15.0, col);
            ui.painter().text(pos2(rect.left() + 42.0, rect.center().y), Align2::LEFT_CENTER, *label, ui::font(13.5, W::S), col);
            if resp.clicked() {
                self.page = *page;
            }
            ui.add_space(2.0);
        }
    }

    fn sidebar_bottom(&mut self, ui: &mut Ui) {
        let p = self.pal;
        if self.timer.running || !self.timer.is_idle() {
            let d = self.durations();
            let label = format!(
                "{} {}  {}",
                if self.timer.running { "●" } else { "Paused ·" },
                fmt_clock(self.timer.display_secs(&d)),
                if self.timer.mode == Mode::Pomodoro { self.timer.phase.label() } else { self.timer.mode.label() }
            );
            if ui.add(egui::Button::new(txt(label, 12.0, W::S, p.accent)).fill(p.accent_soft).corner_radius(10)).clicked() {
                self.page = Page::Home;
            }
            ui.add_space(4.0);
        }
        let today = self.live_today();
        let goal = self.data.settings.daily_goal_min as i64 * 60;
        egui::Frame::new().fill(Color32::from_white_alpha(if p.light { 70 } else { 8 })).corner_radius(14).inner_margin(12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(txt("TODAY", 10.0, W::S, p.subtext));
            ui.label(txt(fmt_dur(today), 20.0, W::B, p.text));
            ui::progress_bar(ui, &p, if goal > 0 { today as f32 / goal as f32 } else { 0.0 }, p.accent, 6.0);
            let streak = self.data.streak(60);
            streak_label(ui, &p, &format!("{streak} days · goal {}", fmt_dur(goal)));
        });
        ui.add_space(14.0);
    }

    // ---------------------------------------------------------------- tasks

    fn page_tasks(&mut self, ui: &mut Ui) {
        let p = self.pal;
        ui::heading(ui, &p, "Tasks", "Break your studying into small tasks and estimate how many pomodoros each needs.");
        ui::card(ui, &p, |ui| {
            ui.set_width(ui.available_width());
            // Laid out right to left so the Add button sits on the card's right edge and the field fills the rest.
            ui.allocate_ui_with_layout(vec2(ui.available_width(), ui::BTN_H), Layout::right_to_left(Align::Center), |ui| {
                let add = ui::accent_button(ui, &p, "+  Add", vec2(100.0, ui::BTN_H)).clicked();
                ui.add(egui::DragValue::new(&mut self.new_task_est).range(1..=20).suffix(" pomo"));
                ui::subject_combo(ui, "task.subject", &self.data, &mut self.new_task_subject, 150.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.new_task)
                        .hint_text("New task, e.g. Chapter 3 integral exercises")
                        .desired_width(ui.available_width())
                        .margin(vec2(10.0, 7.0)),
                );
                let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (add || enter) && !self.new_task.trim().is_empty() {
                    let id = self.data.new_id();
                    self.data.tasks.push(Task {
                        id,
                        subject_id: self.new_task_subject,
                        title: self.new_task.trim().to_string(),
                        est_pomos: self.new_task_est,
                        done_pomos: 0,
                        done: false,
                        created: store::now(),
                        day: None,
                    });
                    self.new_task.clear();
                    self.dirty = true;
                }
            });
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let open = self.data.tasks.iter().filter(|t| !t.done).count();
            let done = self.data.tasks.len() - open;
            if ui::chip(ui, &p, &format!("Open ({open})"), !self.show_done_tasks).clicked() {
                self.show_done_tasks = false;
            }
            if ui::chip(ui, &p, &format!("Done ({done})"), self.show_done_tasks).clicked() {
                self.show_done_tasks = true;
            }
        });
        ui.add_space(6.0);
        let mut focus: Option<(u64, Option<u64>)> = None;
        let mut delete: Option<u64> = None;
        let ids: Vec<u64> = self.data.tasks.iter().filter(|t| t.done == self.show_done_tasks).map(|t| t.id).rev().collect();
        if ids.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(txt(if self.show_done_tasks { "No finished tasks yet." } else { "No open tasks. Add one above!" }, 13.0, W::R, p.subtext));
            });
        }
        for id in ids {
            let (color, sname) = {
                let t = self.data.tasks.iter().find(|t| t.id == id).unwrap();
                (rgb(self.data.subject_color(t.subject_id)), self.data.subject_name(t.subject_id))
            };
            let t = self.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
            ui::row(ui, &p, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if ui::toggle(ui, &p, &mut t.done).changed() {
                        self.dirty = true;
                    }
                    ui.add_space(4.0);
                    ui::dot(ui, color, 5.0);
                    ui.vertical(|ui| {
                        let title = if t.done { txt(&t.title, 14.0, W::S, p.subtext).strikethrough() } else { txt(&t.title, 14.0, W::S, p.text) };
                        ui.label(title);
                        ui.label(txt(format!("{sname} · {}/{} pomodoros", t.done_pomos, t.est_pomos), 11.0, W::R, p.subtext));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::danger_button(ui, &p, "Delete").clicked() {
                            delete = Some(t.id);
                        }
                        if !t.done && ui::accent_button(ui, &p, "Focus", vec2(0.0, ui::BTN_H)).clicked() {
                            focus = Some((t.id, t.subject_id));
                        }
                        let frac = t.done_pomos as f32 / t.est_pomos.max(1) as f32;
                        ui.allocate_ui(vec2(120.0, 20.0), |ui| ui::progress_bar(ui, &p, frac, color, 6.0));
                    });
                });
            });
            ui.add_space(4.0);
        }
        if let Some(id) = delete {
            self.data.tasks.retain(|t| t.id != id);
            if self.task == Some(id) {
                self.task = None;
            }
            self.dirty = true;
        }
        if let Some((id, subj)) = focus {
            self.task = Some(id);
            self.subject = subj;
            self.page = Page::Home;
        }
    }

    // ---------------------------------------------------------------- subjects

    fn page_subjects(&mut self, ui: &mut Ui) {
        let p = self.pal;
        ui::heading(ui, &p, "Subjects", "Group your study time. Subjects that already have sessions can be archived.");
        ui::card(ui, &p, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.new_subject).hint_text("New subject name").desired_width(260.0).margin(vec2(10.0, 7.0)),
                );
                let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                for &c in store::SUBJECT_COLORS {
                    if color_swatch(ui, rgb(c), self.new_subject_color == c, &p).clicked() {
                        self.new_subject_color = c;
                    }
                }
                if (ui::accent_button(ui, &p, "+  Add", vec2(100.0, ui::BTN_H)).clicked() || enter) && !self.new_subject.trim().is_empty() {
                    let id = self.data.new_id();
                    self.data.subjects.push(Subject { id, name: self.new_subject.trim().into(), color: self.new_subject_color, archived: false });
                    self.new_subject.clear();
                    let i = store::SUBJECT_COLORS.iter().position(|c| *c == self.new_subject_color).unwrap_or(0);
                    self.new_subject_color = store::SUBJECT_COLORS[(i + 1) % store::SUBJECT_COLORS.len()];
                    self.dirty = true;
                }
            });
        });
        ui.add_space(10.0);
        let mut delete = None;
        let totals: Vec<(u64, i64, usize)> = self
            .data
            .subjects
            .iter()
            .map(|s| {
                let ss = self.data.sessions.iter().filter(|x| x.subject_id == Some(s.id));
                (s.id, ss.clone().map(|x| x.secs).sum(), ss.count())
            })
            .collect();
        for s in self.data.subjects.iter_mut() {
            let (_, total, count) = totals.iter().find(|t| t.0 == s.id).copied().unwrap_or((0, 0, 0));
            ui::row(ui, &p, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let mut c = s.color;
                    let r = color_swatch(ui, rgb(c), true, &p);
                    egui::Popup::menu(&r).show(|ui| {
                        ui.horizontal(|ui| {
                            for &col in store::SUBJECT_COLORS {
                                if color_swatch(ui, rgb(col), col == c, &p).clicked() {
                                    c = col;
                                }
                            }
                        });
                    });
                    if c != s.color {
                        s.color = c;
                        self.dirty = true;
                    }
                    if ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(240.0).margin(vec2(8.0, 6.0))).changed() {
                        self.dirty = true;
                    }
                    ui.label(txt(format!("{} · {count} sessions", fmt_dur(total)), 12.0, W::R, p.subtext));
                    if s.archived {
                        ui.label(txt("ARCHIVED", 10.0, W::S, p.subtext));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if count == 0 {
                            if ui::danger_button(ui, &p, "Delete").clicked() {
                                delete = Some(s.id);
                            }
                        }
                        if ui::soft_button(ui, &p, if s.archived { "Restore" } else { "Archive" }, vec2(0.0, ui::BTN_H)).clicked() {
                            s.archived = !s.archived;
                            self.dirty = true;
                        }
                    });
                });
            });
            ui.add_space(4.0);
        }
        if let Some(id) = delete {
            self.data.subjects.retain(|s| s.id != id);
            for t in self.data.tasks.iter_mut().filter(|t| t.subject_id == Some(id)) {
                t.subject_id = None;
            }
            if self.subject == Some(id) {
                self.subject = None;
            }
            self.dirty = true;
        }
    }

    // ---------------------------------------------------------------- history

    fn page_history(&mut self, ui: &mut Ui) {
        let p = self.pal;
        ui::heading(ui, &p, "History", "Every focus session. Add one by hand if you forgot to start the timer.");
        ui::card(ui, &p, |ui| {
            ui.set_width(ui.available_width());
            ui::section(ui, &p, "Add a session by hand");
            ui.horizontal(|ui| {
                ui.label(txt("Date", 12.0, W::S, p.subtext));
                ui.add(egui::TextEdit::singleline(&mut self.manual.date).desired_width(96.0).hint_text("YYYY-MM-DD"));
                ui.label(txt("Start", 12.0, W::S, p.subtext));
                ui.add(egui::TextEdit::singleline(&mut self.manual.time).desired_width(62.0).hint_text("HH:MM"));
                ui.label(txt("Duration", 12.0, W::S, p.subtext));
                ui.add(egui::DragValue::new(&mut self.manual.minutes).range(1..=720).suffix(" min"));
                ui::subject_combo(ui, "manual.subject", &self.data, &mut self.manual.subject, 140.0);
                ui.add(egui::TextEdit::singleline(&mut self.manual.note).hint_text("Note (optional)").desired_width(ui.available_width() - 90.0));
                if ui::accent_button(ui, &p, "Save", vec2(80.0, ui::BTN_H)).clicked() {
                    match parse_local(&self.manual.date, &self.manual.time) {
                        Some(start) => {
                            let id = self.data.new_id();
                            self.data.sessions.push(Session {
                                id,
                                subject_id: self.manual.subject,
                                task_id: None,
                                start,
                                secs: self.manual.minutes as i64 * 60,
                                note: self.manual.note.trim().into(),
                                kind: "manual".into(),
                            });
                            self.manual.note.clear();
                            self.manual.error.clear();
                            self.dirty = true;
                            self.toast("Session added");
                        }
                        None => self.manual.error = "Couldn't read the date or time. Example: 2026-11-22 and 19:30".into(),
                    }
                }
            });
            if !self.manual.error.is_empty() {
                ui.label(txt(&self.manual.error, 12.0, W::S, p.danger));
            }
        });
        ui.add_space(12.0);

        let mut sessions: Vec<Session> = self.data.sessions.clone();
        sessions.sort_by_key(|s| -s.start);
        if sessions.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label(txt("No sessions yet. Start your first timer!", 13.0, W::R, p.subtext));
            });
            return;
        }
        let mut current_day: Option<NaiveDate> = None;
        let mut delete = None;
        let mut save_edit = false;
        for s in sessions.iter().take(400) {
            let day = store::local_day(s.start);
            if current_day != Some(day) {
                current_day = Some(day);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(txt(day_label(day), 13.0, W::B, p.text));
                    ui.label(txt(fmt_dur(self.data.secs_on(day)), 12.0, W::S, p.accent));
                });
            }
            ui::row(ui, &p, |ui| {
                ui.set_width(ui.available_width());
                let editing = self.edit.as_ref().is_some_and(|e| e.id == s.id);
                ui.horizontal(|ui| {
                    ui::dot(ui, rgb(self.data.subject_color(s.subject_id)), 5.0);
                    let st = store::local(s.start);
                    let en = store::local(s.start + s.secs);
                    ui.label(txt(format!("{}–{}", st.format("%H:%M"), en.format("%H:%M")), 12.0, W::S, p.subtext));
                    if editing {
                        let e = self.edit.as_mut().unwrap();
                        ui::subject_combo(ui, "edit.subject", &self.data, &mut e.subject, 140.0);
                        ui.add(egui::DragValue::new(&mut e.minutes).range(1..=720).suffix(" min"));
                        ui.add(egui::TextEdit::singleline(&mut e.note).hint_text("Note").desired_width(ui.available_width() - 150.0));
                        if ui::accent_button(ui, &p, "Save", vec2(0.0, ui::BTN_H)).clicked() {
                            save_edit = true;
                        }
                        if ui::soft_button(ui, &p, "Cancel", vec2(0.0, ui::BTN_H)).clicked() {
                            self.edit = None;
                        }
                    } else {
                        ui.label(txt(self.data.subject_name(s.subject_id), 13.0, W::S, p.text));
                        ui.label(txt(fmt_dur(s.secs), 13.0, W::B, p.accent));
                        if let Some(t) = self.data.task(s.task_id) {
                            ui.label(txt(format!("· {}", t.title), 12.0, W::R, p.subtext));
                        }
                        if s.kind == "manual" {
                            ui.label(txt("by hand", 10.0, W::S, p.subtext));
                        }
                        if !s.note.is_empty() {
                            ui.label(txt(format!("“{}”", s.note), 12.0, W::R, p.subtext));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let confirm = self.confirm_delete == Some(s.id);
                            if ui::danger_button(ui, &p, if confirm { "Really delete?" } else { "Delete" }).clicked() {
                                if confirm { delete = Some(s.id) } else { self.confirm_delete = Some(s.id) }
                            }
                            if ui::soft_button(ui, &p, "Edit", vec2(0.0, ui::BTN_H)).clicked() {
                                self.edit = Some(SessionEdit { id: s.id, subject: s.subject_id, minutes: (s.secs / 60).max(1) as u32, note: s.note.clone() });
                            }
                        });
                    }
                });
            });
            ui.add_space(3.0);
        }
        if save_edit {
            if let Some(e) = self.edit.take() {
                if let Some(s) = self.data.sessions.iter_mut().find(|s| s.id == e.id) {
                    s.subject_id = e.subject;
                    s.secs = e.minutes as i64 * 60;
                    s.note = e.note.trim().into();
                    self.dirty = true;
                }
            }
        }
        if let Some(id) = delete {
            self.data.sessions.retain(|s| s.id != id);
            self.confirm_delete = None;
            self.dirty = true;
        }
    }

    // ---------------------------------------------------------------- settings

    fn page_settings(&mut self, ui: &mut Ui) {
        let p = self.pal;
        ui::heading(ui, &p, "Settings", "");
        let s = &mut self.data.settings;
        let mut changed = false;

        ui::card(ui, &p, |ui| {
            ui.set_width(ui.available_width());
            ui::section(ui, &p, "Study space");
            ui::setting_row(ui, &p, "Your name", "Used for the greeting on Home", |ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut s.user_name).desired_width(160.0)).changed();
            });
            ui.add_space(4.0);
            ui.label(txt("Scene", 13.0, W::S, p.text));
            ui.horizontal_wrapped(|ui| {
                let t = ui.input(|i| i.time);
                for (id, name) in crate::scene::SCENES {
                    let (rect, resp) = ui.allocate_exact_size(vec2(150.0, 92.0), Sense::click());
                    let thumb = Rect::from_min_size(rect.min, vec2(150.0, 72.0));
                    crate::scene::paint(&ui.painter().with_clip_rect(thumb), thumb, id, p.light, t);
                    let sel = s.scene == *id;
                    let stroke = if sel { Stroke::new(2.0, p.accent) } else if resp.hovered() { Stroke::new(1.0, p.text) } else { Stroke::new(1.0, Color32::from_white_alpha(40)) };
                    ui.painter().rect_stroke(thumb, 10, stroke, StrokeKind::Inside);
                    ui.painter().text(pos2(rect.left() + 2.0, rect.bottom() - 9.0), Align2::LEFT_CENTER, *name, ui::font(12.0, W::S), if sel { p.accent } else { p.text });
                    if resp.clicked() {
                        s.scene = (*id).into();
                        changed = true;
                    }
                }
            });
            ui.add_space(6.0);
            ui::setting_row(ui, &p, "Glass frost", "Left is clearer, right is milkier", |ui| {
                changed |= ui.add(egui::Slider::new(&mut s.glass_tint, 0.0..=1.0).show_value(false)).changed();
            });
            ui::setting_row(ui, &p, "Meng roams around", "A little cat lives at the bottom of the window and reacts to you", |ui| {
                changed |= ui::toggle(ui, &p, &mut s.cat_roam).changed();
            });
        });
        ui.add_space(12.0);

        ui::card(ui, &p, |ui| {
            ui.set_width(ui.available_width());
            ui::section(ui, &p, "Theme");
            ui.horizontal_wrapped(|ui| {
                for sc in theme::SCHEMES {
                    let sel = s.scheme == sc.id;
                    let (rect, resp) = ui.allocate_exact_size(vec2(112.0, 70.0), Sense::click());
                    ui.painter().rect_filled(rect, 12, rgb(sc.bg));
                    let inner = Rect::from_min_size(rect.min + vec2(10.0, 10.0), vec2(56.0, 22.0));
                    ui.painter().rect_filled(inner, 6, rgb(sc.surface2));
                    ui.painter().circle_filled(pos2(rect.right() - 20.0, rect.top() + 21.0), 7.0, rgb(theme::accent(sc.accent)));
                    ui.painter().text(pos2(rect.left() + 10.0, rect.bottom() - 14.0), Align2::LEFT_CENTER, sc.name, ui::font(12.0, W::S), rgb(sc.text));
                    let stroke = if sel { Stroke::new(2.0, p.accent) } else if resp.hovered() { Stroke::new(1.0, p.subtext) } else { Stroke::new(1.0, p.stroke) };
                    ui.painter().rect_stroke(rect, 12, stroke, StrokeKind::Inside);
                    if resp.clicked() {
                        s.scheme = sc.id.into();
                        changed = true;
                    }
                }
            });
            ui.add_space(8.0);
            ui::section(ui, &p, "Accent color");
            ui.horizontal_wrapped(|ui| {
                if ui::chip(ui, &p, "Auto", s.accent == "auto").clicked() {
                    s.accent = "auto".into();
                    changed = true;
                }
                if ui::chip(ui, &p, "Neutral glass", s.accent == "glass").on_hover_text("No color: frosted white buttons and highlights").clicked() {
                    s.accent = "glass".into();
                    changed = true;
                }
                for (id, name, c) in theme::ACCENTS {
                    if color_swatch(ui, rgb(*c), s.accent == *id, &p).on_hover_text(*name).clicked() {
                        s.accent = (*id).into();
                        changed = true;
                    }
                }
            });
        });
        ui.add_space(12.0);
        ui.columns(2, |cols| {
            ui::card(&mut cols[0], &p, |ui| {
                ui.set_width(ui.available_width());
                ui::section(ui, &p, "Timer");
                let mut num = |ui: &mut Ui, label: &str, v: &mut u32, max: u32, suffix: &str| {
                    ui::setting_row(ui, &p, label, "", |ui| {
                        changed |= ui.add(egui::DragValue::new(v).range(1..=max).suffix(suffix)).changed();
                    });
                };
                num(ui, "Focus length", &mut s.focus_min, 180, " min");
                num(ui, "Short break", &mut s.short_min, 60, " min");
                num(ui, "Long break", &mut s.long_min, 90, " min");
                num(ui, "Long break every", &mut s.long_every, 12, " sessions");
                num(ui, "Daily goal", &mut s.daily_goal_min, 1440, " min");
                ui::setting_row(ui, &p, "New day starts at", "Late-night study counts for the day before", |ui| {
                    changed |= ui.add(egui::DragValue::new(&mut s.day_start_hour).range(0..=6).prefix("0").suffix(":00")).changed();
                });
                ui::setting_row(ui, &p, "Start breaks automatically", "", |ui| changed |= ui::toggle(ui, &p, &mut s.auto_breaks).changed());
                ui::setting_row(ui, &p, "Start focus automatically", "After a break ends", |ui| changed |= ui::toggle(ui, &p, &mut s.auto_focus).changed());
                ui::setting_row(ui, &p, "Notification sound", "", |ui| changed |= ui::toggle(ui, &p, &mut s.sound).changed());
            });
            ui::card(&mut cols[1], &p, |ui| {
                ui.set_width(ui.available_width());
                ui::section(ui, &p, "Idle detection");
                ui::setting_row(ui, &p, "Pause when idle", "No keyboard or mouse input pauses the timer", |ui| changed |= ui::toggle(ui, &p, &mut s.idle_enabled).changed());
                ui::setting_row(ui, &p, "Idle after", "", |ui| {
                    changed |= ui.add(egui::DragValue::new(&mut s.idle_min).range(1..=60).suffix(" min")).changed();
                });
                ui.add_space(6.0);
                ui::section(ui, &p, "Floating widget");
                ui::setting_row(ui, &p, "Show widget when minimized", "A small always-on-top timer while the app is minimized", |ui| changed |= ui::toggle(ui, &p, &mut s.float_visible).changed());
                ui::setting_row(ui, &p, "Opacity", "", |ui| {
                    changed |= ui.add(egui::Slider::new(&mut s.float_opacity, 0.4..=1.0).show_value(false)).changed();
                });
                ui.add_space(6.0);
                ui::section(ui, &p, "Mascot");
                ui::setting_row(ui, &p, "Show mascot & assistant", "", |ui| changed |= ui::toggle(ui, &p, &mut s.mascot_enabled).changed());
                ui::setting_row(ui, &p, "Mascot name", "", |ui| {
                    changed |= ui.add(egui::TextEdit::singleline(&mut s.mascot_name).desired_width(120.0)).changed();
                });
            });
        });
        ui.add_space(12.0);
        ui.columns(2, |cols| {
            ui::card(&mut cols[0], &p, |ui| {
                ui.set_width(ui.available_width());
                ui::section(ui, &p, "Global shortcuts");
                for (k, d) in sys::HOTKEY_HELP {
                    ui.horizontal(|ui| {
                        ui.label(txt(*k, 12.0, W::B, p.accent));
                        ui.label(txt(*d, 12.0, W::R, p.text));
                    });
                }
                ui::setting_row(ui, &p, "Enable shortcuts", "Takes effect after restarting the app", |ui| changed |= ui::toggle(ui, &p, &mut s.hotkeys).changed());
            });
            ui::card(&mut cols[1], &p, |ui| {
                ui.set_width(ui.available_width());
                ui::section(ui, &p, "Data (stored offline)");
                ui.label(txt(store::data_dir().display().to_string(), 12.0, W::R, p.subtext));
                ui.horizontal(|ui| {
                    if ui::soft_button(ui, &p, "Open data folder", vec2(0.0, ui::BTN_H)).clicked() {
                        sys::open_folder(&store::data_dir());
                    }
                });
            });
        });
        if changed {
            self.dirty = true;
        }
    }

    // ---------------------------------------------------------------- floating widget

    /// The mini timer appears only while the main window is minimized (and the widget is enabled).
    fn floating(&mut self, ctx: &egui::Context) {
        let minimized = ctx.input(|i| i.viewport().minimized).unwrap_or(false);
        if !self.data.settings.float_visible || !minimized {
            return;
        }
        let mut builder = ViewportBuilder::default()
            .with_title("StudyTracker mini")
            .with_inner_size([300.0, 84.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_taskbar(false)
            .with_icon(mascot::icon(64));
        if let Some([x, y]) = self.data.settings.float_pos {
            builder = builder.with_position([x, y]);
        }
        ctx.show_viewport_immediate(float_id(), builder, |ui, _| self.float_ui(ui));
    }

    fn float_ui(&mut self, ui: &mut Ui) {
        let p = self.pal;
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) {
            self.data.settings.float_visible = false;
            self.dirty = true;
        }
        if let Some(r) = ctx.input(|i| i.viewport().outer_rect) {
            let pos = [r.min.x, r.min.y];
            if self.data.settings.float_pos.is_none_or(|q| (q[0] - pos[0]).abs() > 1.0 || (q[1] - pos[1]).abs() > 1.0) {
                self.data.settings.float_pos = Some(pos);
                self.dirty = true;
            }
        }
        let rect = ui.max_rect().shrink(2.0);
        let op = self.data.settings.float_opacity;
        ui.painter().rect_filled(rect, CornerRadius::same(20), alpha(p.surface, op));
        ui.painter().rect_stroke(rect, CornerRadius::same(20), Stroke::new(1.0, alpha(p.text, 0.12)), StrokeKind::Inside);

        // Whole widget is a drag handle; double-click opens the main window.
        let bg = ui.interact(rect, ui.scope_id().with("float.drag"), Sense::click_and_drag());
        if bg.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if bg.double_clicked() {
            show_main(&ctx);
        }

        let d = self.durations();
        let t = ctx.input(|i| i.time);
        let mood = if self.idle_notice.is_some() {
            Mood::Sleepy
        } else if self.say.as_ref().is_some_and(|s| s.0 == Mood::Celebrate && Instant::now() < s.2) {
            Mood::Celebrate
        } else if self.timer.running && self.timer.is_focus() {
            Mood::Focus
        } else if self.timer.running {
            Mood::Happy
        } else {
            Mood::Normal
        };
        if self.data.settings.mascot_enabled {
            mascot::draw(ui.painter(), pos2(rect.left() + 40.0, rect.center().y), 58.0, mood, t);
        } else {
            ui::ring(ui.painter(), pos2(rect.left() + 40.0, rect.center().y), 22.0, 5.0, self.timer.progress(&d), p.track, p.accent);
        }
        let x = rect.left() + 78.0;
        ui.painter().text(pos2(x, rect.center().y - 9.0), Align2::LEFT_CENTER, fmt_clock(self.timer.display_secs(&d)), ui::font(28.0, W::B), p.text);
        let label = {
            let phase = if self.idle_notice.is_some() {
                "Away".to_string()
            } else if self.timer.mode == Mode::Pomodoro {
                self.timer.phase.label().to_string()
            } else {
                self.timer.mode.label().to_string()
            };
            format!("{phase} · {}", self.data.subject_name(self.subject))
        };
        let col = if self.timer.is_focus() { p.accent } else { p.success };
        ui.painter().text(pos2(x, rect.center().y + 18.0), Align2::LEFT_CENTER, label, ui::font(11.0, W::S), col);
        // Thin progress along the bottom edge.
        let bar = Rect::from_min_size(pos2(rect.left() + 20.0, rect.bottom() - 6.0), vec2(rect.width() - 40.0, 3.0));
        ui.painter().rect_filled(bar, 2, p.track);
        let mut fill = bar;
        fill.set_width(bar.width() * self.timer.progress(&d));
        ui.painter().rect_filled(fill, 2, col);

        // Buttons on the right.
        let mut bx = rect.right() - 26.0;
        let cy = rect.center().y - 2.0;
        if float_btn(ui, &p, pos2(bx, cy), "expand", false).on_hover_text("Open main window").clicked() {
            show_main(&ctx);
        }
        bx -= 34.0;
        if self.timer.mode == Mode::Pomodoro {
            if float_btn(ui, &p, pos2(bx, cy), "skip", false).on_hover_text("Skip").clicked() {
                self.skip();
            }
        } else if float_btn(ui, &p, pos2(bx, cy), "stop", false).on_hover_text("Finish & save").clicked() {
            self.stop_timer();
        }
        bx -= 38.0;
        let icon = if self.timer.running { "pause" } else { "play" };
        if float_btn(ui, &p, pos2(bx, cy), icon, true).on_hover_text("Start / pause").clicked() {
            self.toggle_timer();
        }
    }
}

impl App {
    /// A small glass pill at the bottom of every page (except Home, which shows the timer itself):
    /// play/pause, the running time, the subject, and a jump into focus mode.
    fn timer_pill(&mut self, ctx: &egui::Context, area: Rect) {
        let p = self.pal;
        let d = self.durations();
        let size = vec2(330.0, 52.0);
        let rect = Rect::from_center_size(pos2(area.center().x, area.bottom() - 40.0), size);
        egui::Area::new(egui::Id::unique("timer.pill")).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
            let (r, body) = crate::glass::pill(ui, &p, size);
            let play = Rect::from_center_size(pos2(r.left() + 28.0, r.center().y), vec2(36.0, 36.0));
            let play_resp = ui.interact(play, ui.scope_id().with("pill.play"), Sense::click());
            ui.painter().circle_filled(play.center(), 17.0, alpha(p.accent, if play_resp.hovered() { 1.0 } else { 0.9 }));
            ui::paint_icon(ui.painter(), if self.timer.running { "pause" } else { "play" }, play.center() + vec2(if self.timer.running { 0.0 } else { 1.0 }, 0.0), 11.0, p.on_accent);
            let time = if self.timer.target_secs(&d).is_some() {
                fmt_clock(self.timer.display_secs(&d))
            } else {
                let s = self.timer.elapsed().as_secs();
                format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
            };
            ui.painter().text(pos2(r.left() + 56.0, r.center().y - 7.0), Align2::LEFT_CENTER, time, ui::font(17.0, W::B), p.text);
            let what = if self.timer.mode == Mode::Pomodoro { self.timer.phase.label() } else { self.timer.mode.label() };
            let line = format!("{what} · {}", self.data.subject_name(self.subject));
            ui.painter().with_clip_rect(r.shrink(4.0)).text(pos2(r.left() + 56.0, r.center().y + 11.0), Align2::LEFT_CENTER, line, ui::font(11.0, W::S), p.subtext);
            let focus = Rect::from_center_size(pos2(r.right() - 26.0, r.center().y), vec2(34.0, 34.0));
            let focus_resp = ui.interact(focus, ui.scope_id().with("pill.focus"), Sense::click());
            if focus_resp.hovered() {
                ui.painter().circle_filled(focus.center(), 16.0, Color32::from_white_alpha(if p.light { 90 } else { 20 }));
            }
            ui::paint_icon(ui.painter(), "expand", focus.center(), 12.0, p.text);
            if play_resp.clicked() {
                self.toggle_timer();
            } else if focus_resp.on_hover_text("Focus mode").clicked() {
                self.page = Page::Focus;
            } else if body.on_hover_text("Open Home").clicked() {
                self.page = Page::Home;
            }
        });
    }

    /// Meng the cat: lives along the bottom of the content (on the desk in focus mode), talks in a bubble.
    fn meng(&mut self, ctx: &egui::Context, t: f64, area: Rect, focus_mode: bool) {
        let s = &self.data.settings;
        if !s.mascot_enabled || !s.cat_roam {
            return;
        }
        let room = matches!(s.scene.as_str(), "night" | "rain");
        let size = if focus_mode { 84.0 } else { 58.0 };
        let ground = if focus_mode && room { area.top() + area.height() * 0.805 } else { area.bottom() - 6.0 };
        let floor = Rect::from_min_max(pos2(area.left() + 10.0, ground - size), pos2(area.right() - 10.0, ground));
        let context = if self.timer.running && self.timer.is_focus() {
            crate::cat::Context::Focus
        } else if self.timer.running {
            crate::cat::Context::Break
        } else {
            crate::cat::Context::Idle
        };
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        self.cat.update(t, floor, context, pointer, size);

        // What Meng says: event lines right away, otherwise a tip now and then.
        if let Some((_, line, until)) = &self.say {
            let left = until.saturating_duration_since(Instant::now()).as_secs_f64();
            if left > 0.0 && self.cat.bubble.as_ref().is_none_or(|b| &b.0 != line) {
                self.cat.say(line.clone(), t, left);
                self.last_bubble = t;
            }
        } else if self.cat.bubble.is_none() && t - self.last_bubble > 55.0 && self.cat.act != crate::cat::Act::Sleep {
            let (_, line) = self.mood_and_line();
            self.cat.say(line, t, 9.0);
            self.last_bubble = t;
        }

        let rect = self.cat.rect(ground, size);
        let resp = egui::Area::new(egui::Id::unique("meng.hit"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| ui.allocate_exact_size(rect.size(), Sense::click()).1)
            .inner;
        if resp.clicked() {
            self.cat.pet(t, floor, size);
            let line = match self.cat.act {
                _ if self.timer.running && self.timer.is_focus() => "Purr... I'll keep you company while you focus.",
                _ => "Meow! Thanks for the pets.",
            };
            self.cat.say(line, t, 4.0);
            self.last_bubble = t;
        }
        let resp = resp.on_hover_text(format!("{} is {}", self.data.settings.mascot_name, self.cat.act.label()));
        let _ = resp;
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("meng.paint")));
        let p = self.pal;
        let bubble = if p.light { Color32::from_rgba_unmultiplied(255, 255, 255, 235) } else { Color32::from_rgba_unmultiplied(22, 24, 36, 235) };
        self.cat.draw(&painter, ground, size, t, p.text, bubble);
    }
}

/// STUDYTRACKER_DEMO_CAT=<activity> holds Meng in one pose (screenshots during development).
fn demo_cat() -> crate::cat::Cat {
    use crate::cat::Act;
    let mut cat = crate::cat::Cat::default();
    let act = match std::env::var("STUDYTRACKER_DEMO_CAT").unwrap_or_default().as_str() {
        "walk" => Some(Act::Walk),
        "sleep" => Some(Act::Sleep),
        "groom" => Some(Act::Groom),
        "stretch" => Some(Act::Stretch),
        "play" => Some(Act::Play),
        "hop" => Some(Act::Hop),
        _ => None,
    };
    if let Some(act) = act {
        cat.hold(act);
    }
    cat
}

/// STUDYTRACKER_PAGE opens a given page at start (used for screenshots during development).
fn start_page() -> Page {
    match std::env::var("STUDYTRACKER_PAGE").unwrap_or_default().as_str() {
        "tasks" => Page::Tasks,
        "subjects" => Page::Subjects,
        "history" => Page::History,
        "stats" => Page::Stats,
        "settings" => Page::Settings,
        "focus" => Page::Focus,
        _ => Page::Home,
    }
}

fn streak_label(ui: &mut Ui, p: &Pal, text: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let (r, _) = ui.allocate_exact_size(vec2(12.0, 14.0), Sense::hover());
        ui::paint_icon(ui.painter(), "flame", r.center(), 11.0, rgb(0xFF8A3D));
        ui.label(txt(text, 11.5, W::S, p.subtext));
    });
}

fn show_main(ctx: &egui::Context) {
    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(true));
    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus);
}

fn float_btn(ui: &mut Ui, p: &Pal, c: egui::Pos2, icon: &str, primary: bool) -> egui::Response {
    let r = if primary { 17.0 } else { 14.0 };
    let rect = Rect::from_center_size(c, vec2(r * 2.0, r * 2.0));
    let resp = ui.interact(rect, ui.scope_id().with(("float.btn", icon)), Sense::click());
    let fill = match (primary, resp.hovered()) {
        (true, false) => p.accent,
        (true, true) => alpha(p.accent, 0.85),
        (false, true) => p.hover,
        (false, false) => p.surface2,
    };
    ui.painter().circle_filled(c, r, fill);
    let col = if primary { p.on_accent } else { p.text };
    ui::paint_icon(ui.painter(), icon, c, if primary { 12.0 } else { 10.0 }, col);
    resp
}

fn color_swatch(ui: &mut Ui, c: Color32, selected: bool, p: &Pal) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    ui.painter().circle_filled(rect.center(), 10.0, c);
    if selected {
        ui.painter().circle_stroke(rect.center(), 11.5, Stroke::new(2.0, p.text));
    } else if resp.hovered() {
        ui.painter().circle_stroke(rect.center(), 11.5, Stroke::new(1.0, p.subtext));
    }
    resp
}

fn parse_local(date: &str, time: &str) -> Option<i64> {
    let d = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
    let t = NaiveTime::parse_from_str(time.trim(), "%H:%M").ok()?;
    Local.from_local_datetime(&d.and_time(t)).earliest().map(|x| x.timestamp())
}

const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

fn day_label(d: NaiveDate) -> String {
    let today = store::today();
    if d == today {
        return "Today".into();
    }
    if Some(d) == today.pred_opt() {
        return "Yesterday".into();
    }
    format!("{}, {} {} {}", DAYS[d.weekday().num_days_from_monday() as usize], MONTHS[d.month0() as usize], d.day(), d.year())
}

fn bar_chart(ui: &mut Ui, p: &Pal, days: &[NaiveDate], vals: &[i64], goal: i64) {
    let h = 200.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let painter = ui.painter_at(rect);
    let plot = Rect::from_min_max(rect.min + vec2(36.0, 10.0), rect.max - vec2(0.0, 22.0));
    let max = vals.iter().copied().max().unwrap_or(0).max(goal).max(3600) as f32 * 1.1;
    // Grid lines every hour (or every 2h when tall).
    let step = if max > 6.0 * 3600.0 { 7200.0 } else { 3600.0 };
    let mut v = 0.0;
    while v <= max {
        let y = plot.bottom() - v / max * plot.height();
        painter.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], Stroke::new(1.0, p.stroke));
        painter.text(pos2(plot.left() - 6.0, y), Align2::RIGHT_CENTER, format!("{}h", (v / 3600.0) as i64), ui::font(10.0, W::R), p.subtext);
        v += step;
    }
    let n = vals.len().max(1);
    let slot = plot.width() / n as f32;
    let bw = (slot * 0.6).min(36.0);
    let hover = resp.hover_pos();
    for (i, (&val, d)) in vals.iter().zip(days).enumerate() {
        let cx = plot.left() + slot * (i as f32 + 0.5);
        let bh = val as f32 / max * plot.height();
        let r = Rect::from_min_max(pos2(cx - bw / 2.0, plot.bottom() - bh.max(2.0)), pos2(cx + bw / 2.0, plot.bottom()));
        let hovered = hover.is_some_and(|h| (h.x - cx).abs() < slot / 2.0);
        let col = if goal > 0 && val >= goal { p.accent } else { alpha(p.accent, if hovered { 0.75 } else { 0.5 }) };
        painter.rect_filled(r, CornerRadius { nw: 5, ne: 5, sw: 1, se: 1 }, col);
        if n <= 14 || i % 3 == (n - 1) % 3 {
            let label = if n <= 7 { DAYS[d.weekday().num_days_from_monday() as usize].to_string() } else { format!("{}", d.day()) };
            painter.text(pos2(cx, plot.bottom() + 11.0), Align2::CENTER_CENTER, label, ui::font(10.0, W::R), p.subtext);
        }
        if hovered {
            painter.text(pos2(cx, r.top() - 9.0), Align2::CENTER_CENTER, fmt_dur(val), ui::font(11.0, W::B), p.text);
        }
    }
    if goal > 0 {
        let y = plot.bottom() - goal as f32 / max * plot.height();
        let mut x = plot.left();
        while x < plot.right() {
            painter.line_segment([pos2(x, y), pos2((x + 6.0).min(plot.right()), y)], Stroke::new(1.5, p.success));
            x += 11.0;
        }
        painter.text(pos2(plot.right(), y - 8.0), Align2::RIGHT_CENTER, "goal", ui::font(10.0, W::S), p.success);
    }
}

fn heatmap(ui: &mut Ui, p: &Pal, data: &Data, goal: i64) {
    let weeks: i64 = 53;
    let gap = 3.0;
    let label_w = 30.0;
    let cell = ((ui.available_width() - label_w) / weeks as f32 - gap).clamp(6.0, 16.0);
    let today = store::today();
    let this_monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
    let first = this_monday - chrono::Duration::weeks(weeks - 1);
    let top = 16.0;
    let w = label_w + weeks as f32 * (cell + gap);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, top + 7.0 * (cell + gap) + 20.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let full = goal.max(3600) as f32;
    let mut tip = None;
    for (r, name) in DAYS.iter().enumerate() {
        if r % 2 == 0 {
            let y = rect.top() + top + r as f32 * (cell + gap) + cell / 2.0;
            painter.text(pos2(rect.left(), y), Align2::LEFT_CENTER, *name, ui::font(9.0, W::R), p.subtext);
        }
    }
    let mut last_month = None;
    for wk in 0..weeks {
        let x = rect.left() + label_w + wk as f32 * (cell + gap);
        let week_start = first + chrono::Duration::weeks(wk);
        // Month label above the first week that starts in a new month.
        if last_month != Some(week_start.month()) {
            if last_month.is_some() || week_start.day() <= 7 {
                painter.text(pos2(x, rect.top() + 6.0), Align2::LEFT_CENTER, MONTHS[week_start.month0() as usize], ui::font(10.0, W::S), p.subtext);
            }
            last_month = Some(week_start.month());
        }
        for dow in 0..7 {
            let d = week_start + chrono::Duration::days(dow);
            if d > today {
                continue;
            }
            let y = rect.top() + top + dow as f32 * (cell + gap);
            let cr = Rect::from_min_size(pos2(x, y), vec2(cell, cell));
            let secs = data.secs_on(d);
            let col = if secs == 0 { p.track } else { alpha(p.accent, 0.25 + 0.75 * (secs as f32 / full).min(1.0)) };
            let radius = (cell / 4.0) as u8;
            painter.rect_filled(cr, radius, col);
            if resp.hover_pos().is_some_and(|h| cr.expand(gap / 2.0).contains(h)) {
                painter.rect_stroke(cr, radius, Stroke::new(1.0, p.text), StrokeKind::Outside);
                tip = Some(format!("{}: {}", day_label(d), fmt_dur(secs)));
            }
        }
    }
    let label = tip.unwrap_or_else(|| "Brighter = more study time. Hover a day for details.".into());
    painter.text(pos2(rect.left(), rect.bottom() - 7.0), Align2::LEFT_CENTER, label, ui::font(11.0, W::R), p.subtext);
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let key = (self.data.settings.scheme.clone(), self.data.settings.accent.clone());
        if key != self.theme_key {
            self.pal = theme::build(&key.0, &key.1);
            theme::apply(&ctx, &self.pal);
            self.theme_key = key;
        }
        let p = self.pal;
        let t = ctx.input(|i| i.time);
        let screen = ui.max_rect();
        // Scene behind everything, and its blurred copy for the glass panels.
        let scene = self.data.settings.scene.clone();
        crate::scene::paint(ui.painter(), screen, &scene, p.light, t);
        self.backdrop.update(&ctx, &scene, p.light, screen.size());
        crate::glass::set(Some(crate::glass::Glass {
            tex: self.backdrop.tex.as_ref().map(|t| t.id()),
            screen,
            light: p.light,
            tint: self.data.settings.glass_tint,
        }));
        if ctx.input(|i| i.viewport().close_requested()) {
            let ev = self.timer.stop();
            self.handle(ev);
            self.save_now();
        }

        let focus_mode = self.page == Page::Focus && !self.shot.as_ref().is_some_and(|s| s.float);
        if !focus_mode {
            egui::Panel::left("nav")
                .exact_size(236.0)
                .show_separator_line(false)
                .resizable(false)
                .frame(egui::Frame::NONE.inner_margin(egui::Margin { left: 14, right: 0, top: 14, bottom: 14 }))
                .show(ui, |ui| {
                    crate::glass::frame(ui, &p, 22, egui::Margin::symmetric(12, 4), |ui| {
                        ui.set_min_size(ui.available_size());
                        self.sidebar(ui)
                    });
                });
        }
        let central = egui::CentralPanel::default_margins()
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(if focus_mode { 16 } else { 22 })))
            .show(ui, |ui| {
                if self.shot.as_ref().is_some_and(|s| s.float) {
                    // Screenshot of the floating widget: draw it here, at its real size.
                    let r = Rect::from_min_size(ui.min_rect().min, vec2(300.0, 84.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| self.float_ui(ui));
                    return;
                }
                if focus_mode {
                    self.page_focus(ui);
                    return;
                }
                if self.page == Page::Home {
                    self.page_home(ui);
                    return;
                }
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match self.page {
                    Page::Home | Page::Focus => self.page_home(ui),
                    Page::Tasks => self.page_tasks(ui),
                    Page::Subjects => self.page_subjects(ui),
                    Page::History => self.page_history(ui),
                    Page::Stats => self.page_insights(ui),
                    Page::Settings => self.page_settings(ui),
                });
            })
            .response
            .rect;
        if !focus_mode && !matches!(self.page, Page::Home) && self.shot.as_ref().is_none_or(|s| !s.float) {
            self.timer_pill(&ctx, central);
        }
        self.meng(&ctx, t, if focus_mode { screen } else { central }, focus_mode);

        if let Some((msg, at)) = &self.toast {
            if at.elapsed() < Duration::from_secs(3) {
                egui::Area::new(egui::Id::unique("toast")).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -24.0)).show(&ctx, |ui| {
                    egui::Frame::new().fill(p.text).corner_radius(10).inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
                        ui.label(txt(msg, 13.0, W::S, p.bg));
                    });
                });
            } else {
                self.toast = None;
            }
        }

        // Screenshot runs leave out the separate widget window (its GL context can race the capture).
        if self.shot.is_none() {
            self.floating(&ctx);
        }
        if let Some(shot) = &mut self.shot {
            shot.step(&ctx);
        }
        // Scenes and Meng are animated: keep frames coming (gently).
        ctx.request_repaint_after(Duration::from_millis(33));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let ev = self.timer.stop();
        self.handle(ev);
        self.save_now();
    }
}

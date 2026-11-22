//! Timer engine: Pomodoro (focus / short break / long break), stopwatch, and countdown.
//! Time is measured with `Instant`, so it stays right even when frames are skipped.

use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Pomodoro,
    Stopwatch,
    Countdown,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Pomodoro => "Pomodoro",
            Mode::Stopwatch => "Stopwatch",
            Mode::Countdown => "Countdown",
        }
    }
    pub fn kind(self) -> &'static str {
        match self {
            Mode::Pomodoro => "pomodoro",
            Mode::Stopwatch => "stopwatch",
            Mode::Countdown => "countdown",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Focus => "Focus",
            Phase::ShortBreak => "Short break",
            Phase::LongBreak => "Long break",
        }
    }
    pub fn is_break(self) -> bool {
        self != Phase::Focus
    }
}

/// Something the app has to react to (save a session, play a sound, update the mascot).
pub enum Event {
    /// A focus block ended. `completed` is false when the user stopped it early.
    FocusEnded { start: i64, secs: i64, mode: Mode, completed: bool },
    BreakEnded,
}

pub struct Timer {
    pub mode: Mode,
    pub phase: Phase,
    pub running: bool,
    accumulated: Duration,
    resumed_at: Option<Instant>,
    /// Wall-clock start of the current block (unix secs), set on first start.
    pub block_start: Option<i64>,
    /// Focus blocks finished in this Pomodoro cycle.
    pub cycle_done: u32,
}

pub struct Durations {
    pub focus: u32,
    pub short: u32,
    pub long: u32,
    pub long_every: u32,
    pub countdown: u32,
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            mode: Mode::Pomodoro,
            phase: Phase::Focus,
            running: false,
            accumulated: Duration::ZERO,
            resumed_at: None,
            block_start: None,
            cycle_done: 0,
        }
    }
}

impl Timer {
    pub fn elapsed(&self) -> Duration {
        self.accumulated + self.resumed_at.map_or(Duration::ZERO, |t| t.elapsed())
    }

    /// Length of the current block in seconds; None for the stopwatch.
    pub fn target_secs(&self, d: &Durations) -> Option<u64> {
        let min = match (self.mode, self.phase) {
            (Mode::Stopwatch, _) => return None,
            (Mode::Countdown, _) => d.countdown,
            (Mode::Pomodoro, Phase::Focus) => d.focus,
            (Mode::Pomodoro, Phase::ShortBreak) => d.short,
            (Mode::Pomodoro, Phase::LongBreak) => d.long,
        };
        Some(min.max(1) as u64 * 60)
    }

    /// Seconds to show: remaining for timed blocks, elapsed for the stopwatch.
    pub fn display_secs(&self, d: &Durations) -> u64 {
        let e = self.elapsed().as_secs();
        match self.target_secs(d) {
            Some(t) => t.saturating_sub(e),
            None => e,
        }
    }

    /// 0..1 progress through the block (stopwatch: through the current hour).
    pub fn progress(&self, d: &Durations) -> f32 {
        let e = self.elapsed().as_secs_f32();
        match self.target_secs(d) {
            Some(t) => (e / t as f32).clamp(0.0, 1.0),
            None => (e % 3600.0) / 3600.0,
        }
    }

    pub fn is_focus(&self) -> bool {
        self.mode != Mode::Pomodoro || self.phase == Phase::Focus
    }

    pub fn is_idle(&self) -> bool {
        !self.running && self.accumulated.is_zero()
    }

    pub fn start(&mut self) {
        if !self.running {
            self.running = true;
            self.resumed_at = Some(Instant::now());
            if self.block_start.is_none() {
                self.block_start = Some(crate::store::now());
            }
        }
    }

    pub fn pause(&mut self) {
        if self.running {
            self.accumulated = self.elapsed();
            self.resumed_at = None;
            self.running = false;
        }
    }

    pub fn toggle(&mut self) {
        if self.running { self.pause() } else { self.start() }
    }

    /// Removes time (idle) from the block. Returns how much was actually removed.
    pub fn remove_time(&mut self, d: Duration) -> Duration {
        let was_running = self.running;
        self.pause();
        let removed = d.min(self.accumulated);
        self.accumulated -= removed;
        if was_running {
            self.start();
        }
        removed
    }

    pub fn add_time(&mut self, d: Duration) {
        let was_running = self.running;
        self.pause();
        self.accumulated += d;
        if was_running {
            self.start();
        }
    }

    fn reset_block(&mut self) {
        self.running = false;
        self.accumulated = Duration::ZERO;
        self.resumed_at = None;
        self.block_start = None;
    }

    pub fn set_mode(&mut self, mode: Mode) -> Option<Event> {
        let ev = self.stop();
        self.mode = mode;
        self.phase = Phase::Focus;
        self.cycle_done = 0;
        ev
    }

    /// Switches to a Pomodoro phase by hand (discards break time, records focus time).
    pub fn set_phase(&mut self, phase: Phase) -> Option<Event> {
        let ev = self.stop();
        self.phase = phase;
        ev
    }

    /// Ends the current block early. Focus time of at least a minute is reported to be saved.
    pub fn stop(&mut self) -> Option<Event> {
        let secs = self.elapsed().as_secs() as i64;
        let ev = match (self.is_focus(), self.block_start) {
            (true, Some(start)) if secs >= 60 => {
                Some(Event::FocusEnded { start, secs, mode: self.mode, completed: false })
            }
            _ => None,
        };
        self.reset_block();
        ev
    }

    /// Skips to the next Pomodoro phase (records focus time like `stop`).
    pub fn skip(&mut self, d: &Durations) -> Option<Event> {
        let was_focus = self.phase == Phase::Focus;
        let ev = self.stop();
        if self.mode == Mode::Pomodoro {
            if was_focus {
                self.cycle_done += 1;
            }
            self.phase = self.next_phase(was_focus, d);
        }
        ev
    }

    fn next_phase(&mut self, was_focus: bool, d: &Durations) -> Phase {
        if !was_focus {
            return Phase::Focus;
        }
        if self.cycle_done >= d.long_every.max(1) {
            self.cycle_done = 0;
            Phase::LongBreak
        } else {
            Phase::ShortBreak
        }
    }

    /// Call often. Ends the block when its time is up and moves on to the next phase.
    pub fn tick(&mut self, d: &Durations, auto_breaks: bool, auto_focus: bool) -> Option<Event> {
        let target = self.target_secs(d)?;
        if !self.running || self.elapsed().as_secs() < target {
            return None;
        }
        let start = self.block_start.unwrap_or_else(crate::store::now);
        let was_focus = self.is_focus();
        self.reset_block();
        match self.mode {
            Mode::Countdown => Some(Event::FocusEnded { start, secs: target as i64, mode: Mode::Countdown, completed: true }),
            Mode::Stopwatch => None,
            Mode::Pomodoro => {
                if was_focus {
                    self.cycle_done += 1;
                }
                self.phase = self.next_phase(was_focus, d);
                let auto = if self.phase.is_break() { auto_breaks } else { auto_focus };
                if auto {
                    self.start();
                }
                Some(if was_focus {
                    Event::FocusEnded { start, secs: target as i64, mode: Mode::Pomodoro, completed: true }
                } else {
                    Event::BreakEnded
                })
            }
        }
    }
}

pub fn fmt_clock(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

pub fn fmt_dur(secs: i64) -> String {
    let m = secs.max(0) / 60;
    let (h, m) = (m / 60, m % 60);
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d() -> Durations {
        Durations { focus: 25, short: 5, long: 15, long_every: 2, countdown: 10 }
    }

    #[test]
    fn short_stop_is_not_recorded() {
        let mut t = Timer::default();
        t.start();
        t.add_time(Duration::from_secs(30));
        assert!(t.stop().is_none());
        assert!(t.is_idle());
    }

    #[test]
    fn stop_records_focus_time() {
        let mut t = Timer::default();
        t.start();
        t.add_time(Duration::from_secs(600));
        match t.stop() {
            Some(Event::FocusEnded { secs, completed: false, .. }) => assert!((600..602).contains(&secs)),
            _ => panic!("expected a recorded focus block"),
        }
    }

    #[test]
    fn pomodoro_cycles_to_long_break() {
        let (mut t, d) = (Timer::default(), d());
        for expected in [Phase::ShortBreak, Phase::Focus, Phase::LongBreak, Phase::Focus] {
            t.start();
            t.add_time(Duration::from_secs(60 * 60));
            assert!(t.tick(&d, false, false).is_some());
            assert_eq!(t.phase, expected);
        }
    }

    #[test]
    fn completed_focus_reports_full_length() {
        let (mut t, d) = (Timer::default(), d());
        t.start();
        t.add_time(Duration::from_secs(26 * 60));
        match t.tick(&d, true, false) {
            Some(Event::FocusEnded { secs, completed: true, .. }) => assert_eq!(secs, 25 * 60),
            _ => panic!("expected completed focus"),
        }
        assert!(t.running, "break should auto-start");
    }

    #[test]
    fn idle_time_is_removed() {
        let mut t = Timer::default();
        t.start();
        t.add_time(Duration::from_secs(300));
        t.remove_time(Duration::from_secs(120));
        let e = t.elapsed().as_secs();
        assert!((180..182).contains(&e));
    }

    #[test]
    fn stopwatch_never_ends_by_itself() {
        let (mut t, d) = (Timer::default(), d());
        t.set_mode(Mode::Stopwatch);
        t.start();
        t.add_time(Duration::from_secs(5 * 3600));
        assert!(t.tick(&d, true, true).is_none());
        assert_eq!(t.display_secs(&d), t.elapsed().as_secs());
    }

    #[test]
    fn clock_format() {
        assert_eq!(fmt_clock(65), "01:05");
        assert_eq!(fmt_clock(3725), "1:02:05");
        assert_eq!(fmt_dur(90 * 60), "1h 30m");
        assert_eq!(fmt_dur(120 * 60), "2h");
    }
}

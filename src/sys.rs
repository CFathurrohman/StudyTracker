//! Small Windows helpers: idle time, notification sound, opening folders, global hotkeys.

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

/// Seconds since the last keyboard or mouse input anywhere in Windows.
#[cfg(windows)]
pub fn idle_secs() -> u64 {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    unsafe {
        if GetLastInputInfo(&mut info) == 0 {
            return 0;
        }
        (GetTickCount().wrapping_sub(info.dwTime) / 1000) as u64
    }
}

#[cfg(not(windows))]
pub fn idle_secs() -> u64 {
    0
}

pub fn chime() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
        use windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONASTERISK;
        MessageBeep(MB_ICONASTERISK);
    }
}

pub fn open_folder(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hot {
    ToggleTimer,
    ToggleFloat,
    Skip,
}

pub const HOTKEY_HELP: &[(&str, &str)] = &[
    ("Ctrl + Alt + Space", "Start / pause the timer"),
    ("Ctrl + Alt + W", "Turn the floating widget on / off"),
    ("Ctrl + Alt + N", "Skip to the next phase"),
];

pub struct Hotkeys {
    _manager: Option<GlobalHotKeyManager>,
    ids: Vec<(u32, Hot)>,
}

impl Hotkeys {
    pub fn none() -> Self {
        Self { _manager: None, ids: vec![] }
    }

    pub fn register() -> Self {
        let Ok(manager) = GlobalHotKeyManager::new() else {
            return Self { _manager: None, ids: vec![] };
        };
        let mods = Some(Modifiers::CONTROL | Modifiers::ALT);
        let mut ids = vec![];
        for (code, hot) in [(Code::Space, Hot::ToggleTimer), (Code::KeyW, Hot::ToggleFloat), (Code::KeyN, Hot::Skip)] {
            let hk = HotKey::new(mods, code);
            if manager.register(hk).is_ok() {
                ids.push((hk.id(), hot));
            }
        }
        Self { _manager: Some(manager), ids }
    }

    /// Hotkeys pressed since the last call.
    pub fn poll(&self) -> Vec<Hot> {
        let mut out = vec![];
        while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
            if ev.state == HotKeyState::Pressed {
                if let Some((_, h)) = self.ids.iter().find(|(id, _)| *id == ev.id) {
                    out.push(*h);
                }
            }
        }
        out
    }
}

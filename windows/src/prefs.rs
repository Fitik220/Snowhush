use std::ffi::c_void;

use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW};
use windows::core::{BOOL, PCWSTR, w};

use snowhush::cursor_snow::{CursorSettings, Level};
use snowhush::simulation::Snowfall;

const KEY: PCWSTR = w!("Software\\Snowhush");
const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = w!("Snowhush");

#[derive(Clone, Copy)]
pub struct Settings {
    pub snowfall: Snowfall,
    pub accumulate: bool,
    pub melt: bool,
    pub snow_on_windows: bool,
    pub behind_windows: bool,
    pub show_in_screenshots: bool,
    pub welcomed: bool,
    pub cursor_snow: CursorSettings,
}

impl Settings {
    pub fn load() -> Settings {
        let calm = !animations_enabled();
        let snowfall = read(w!("snowfall")).and_then(|i| Snowfall::ALL.get(i as usize).copied());
        Settings {
            snowfall: snowfall.unwrap_or(if calm { Snowfall::Light } else { Snowfall::Steady }),
            accumulate: read(w!("accumulate")).is_none_or(|v| v != 0),
            melt: read(w!("meltSnow")).is_none_or(|v| v != 0),
            snow_on_windows: read(w!("snowOnWindows")).is_none_or(|v| v != 0),
            behind_windows: read(w!("behindWindows")).is_some_and(|v| v != 0),
            show_in_screenshots: read(w!("showInScreenshots")).is_some_and(|v| v != 0),
            welcomed: read(w!("welcomed")).is_some_and(|v| v != 0),
            cursor_snow: load_cursor_settings(KEY),
        }
    }

    pub fn save(&self) {
        let snowfall = Snowfall::ALL.iter().position(|&s| s == self.snowfall).unwrap_or(2);
        write(w!("snowfall"), snowfall as u32);
        write(w!("accumulate"), self.accumulate as u32);
        write(w!("meltSnow"), self.melt as u32);
        write(w!("snowOnWindows"), self.snow_on_windows as u32);
        write(w!("behindWindows"), self.behind_windows as u32);
        write(w!("showInScreenshots"), self.show_in_screenshots as u32);
        write(w!("welcomed"), self.welcomed as u32);
        save_cursor_settings(KEY, self.cursor_snow);
    }
}

fn load_cursor_settings(key: PCWSTR) -> CursorSettings {
    CursorSettings {
        enabled: read_key(key, w!("cursorSnow")).is_some_and(|v| v != 0),
        amount: Level::from_index(read_key(key, w!("cursorSnowAmount")).unwrap_or(1)),
        adhesion: Level::from_index(read_key(key, w!("cursorSnowAdhesion")).unwrap_or(1)),
        shake: read_key(key, w!("cursorSnowShake")).is_none_or(|v| v != 0),
        melting: read_key(key, w!("cursorSnowMelting")).is_none_or(|v| v != 0),
    }
}

fn save_cursor_settings(key: PCWSTR, settings: CursorSettings) {
    write_key(key, w!("cursorSnow"), settings.enabled as u32);
    write_key(key, w!("cursorSnowAmount"), settings.amount.index());
    write_key(key, w!("cursorSnowAdhesion"), settings.adhesion.index());
    write_key(key, w!("cursorSnowShake"), settings.shake as u32);
    write_key(key, w!("cursorSnowMelting"), settings.melting as u32);
}

pub fn opens_at_login() -> bool {
    let mut buffer = [0u16; 1024];
    let mut size = (buffer.len() * 2) as u32;
    let status =
        unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN, RUN_VALUE, RRF_RT_REG_SZ, None, Some(buffer.as_mut_ptr().cast::<c_void>()), Some(&mut size)) };
    if status != ERROR_SUCCESS {
        return false;
    }
    let length = buffer.iter().position(|&c| c == 0).unwrap_or(0);
    command().is_some_and(|command| command[..command.len() - 1] == buffer[..length])
}

pub fn set_opens_at_login(on: bool) -> WIN32_ERROR {
    unsafe {
        if !on {
            return RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, RUN_VALUE);
        }
        let Some(command) = command() else { return WIN32_ERROR(1) };
        RegSetKeyValueW(HKEY_CURRENT_USER, RUN, RUN_VALUE, REG_SZ.0, Some(command.as_ptr().cast()), (command.len() * 2) as u32)
    }
}

/// The quoted path of this executable, NUL-terminated.
fn command() -> Option<Vec<u16>> {
    let path = std::env::current_exe().ok()?;
    Some(format!("\"{}\"", path.display()).encode_utf16().chain([0]).collect())
}

fn read(name: PCWSTR) -> Option<u32> {
    read_key(KEY, name)
}

fn read_key(key: PCWSTR, name: PCWSTR) -> Option<u32> {
    let mut value = 0u32;
    let mut size = 4u32;
    let status =
        unsafe { RegGetValueW(HKEY_CURRENT_USER, key, name, RRF_RT_REG_DWORD, None, Some((&raw mut value).cast::<c_void>()), Some(&mut size)) };
    (status == ERROR_SUCCESS).then_some(value)
}

fn write(name: PCWSTR, value: u32) {
    write_key(KEY, name, value)
}

fn write_key(key: PCWSTR, name: PCWSTR, value: u32) {
    unsafe {
        let _ = RegSetKeyValueW(HKEY_CURRENT_USER, key, name, REG_DWORD.0, Some((&raw const value).cast::<c_void>()), 4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Registry::RegDeleteTreeW;

    #[test]
    fn cursor_settings_survive_registry_round_trip() {
        const TEST_KEY: PCWSTR = w!("Software\\Snowhush-CursorSnow-Test");
        let settings = CursorSettings { enabled: true, amount: Level::High, adhesion: Level::Low, shake: false, melting: false };
        save_cursor_settings(TEST_KEY, settings);
        let loaded = load_cursor_settings(TEST_KEY);
        unsafe {
            let _ = RegDeleteTreeW(HKEY_CURRENT_USER, TEST_KEY);
        }
        assert!(loaded.enabled);
        assert_eq!(loaded.amount, Level::High);
        assert_eq!(loaded.adhesion, Level::Low);
        assert!(!loaded.shake && !loaded.melting);
    }
}

/// The "Animation effects" setting: off means the person prefers less motion.
pub fn animations_enabled() -> bool {
    let mut enabled = BOOL(1);
    let result = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some((&raw mut enabled).cast::<c_void>()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0))
    };
    result.is_err() || enabled.as_bool()
}

//! The primary monitor's current mode, used to pick "native" defaults.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
}

#[cfg(windows)]
pub fn primary() -> Option<DisplayMode> {
    use windows_sys::Win32::Graphics::Gdi::{DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW};
    // SAFETY: DEVMODEW is plain data; dmSize tells the API which layout we pass.
    unsafe {
        let mut mode: DEVMODEW = std::mem::zeroed();
        mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut mode) == 0 {
            return None;
        }
        Some(DisplayMode {
            width: mode.dmPelsWidth,
            height: mode.dmPelsHeight,
            refresh_hz: mode.dmDisplayFrequency.max(1),
        })
    }
}

#[cfg(not(windows))]
pub fn primary() -> Option<DisplayMode> {
    None
}

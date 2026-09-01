//! Windows application-theme discovery.

use quadrant_application::SystemTheme;
use windows::{
    Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
    },
    core::PCWSTR,
};

const PERSONALIZE_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
const APPS_USE_LIGHT_THEME: &str = "AppsUseLightTheme";

pub(super) fn current_system_theme() -> SystemTheme {
    let key = wide_null(PERSONALIZE_KEY);
    let name = wide_null(APPS_USE_LIGHT_THEME);
    let mut value = 1_u32;
    let mut size = u32::try_from(std::mem::size_of::<u32>()).unwrap_or(4);
    // SAFETY: both PCWSTR values are live nul-terminated buffers; value and
    // size point to valid writable storage of the declared REG_DWORD size.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&raw mut size),
        )
    };
    if result == ERROR_SUCCESS {
        theme_from_apps_use_light_theme(value)
    } else {
        SystemTheme::Light
    }
}

const fn theme_from_apps_use_light_theme(value: u32) -> SystemTheme {
    if value == 0 {
        SystemTheme::Dark
    } else {
        SystemTheme::Light
    }
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use quadrant_application::SystemTheme;

    use super::theme_from_apps_use_light_theme;

    #[test]
    fn windows_theme_registry_value_maps_to_normalized_theme() {
        assert_eq!(theme_from_apps_use_light_theme(0), SystemTheme::Dark);
        assert_eq!(theme_from_apps_use_light_theme(1), SystemTheme::Light);
    }
}

//! Platform capability boundary and target-specific integrations.

use quadrant_application::{SystemTheme, SystemThemeSource};

/// Capabilities exposed to application/UI code without leaking OS checks.
#[allow(clippy::struct_excessive_bools)] // Independent feature flags, not one state machine.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformCapabilities {
    /// Whether a global Quick Add shortcut is available.
    pub global_hotkey: bool,
    /// Whether a tray or status item is available.
    pub tray: bool,
    /// Whether autostart can be configured.
    pub autostart: bool,
    /// Whether native notifications are available.
    pub native_notifications: bool,
    /// Whether a native window backdrop is available.
    pub native_backdrop: bool,
}

/// Cross-platform theme source used until native observation is implemented.
///
/// The fallback is deliberately light and never makes startup fail. Target-specific
/// observation can replace this implementation inside this crate in M3.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformThemeSource;

impl SystemThemeSource for PlatformThemeSource {
    fn current_theme(&self) -> SystemTheme {
        SystemTheme::Light
    }
}

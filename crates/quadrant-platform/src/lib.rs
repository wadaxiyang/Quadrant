//! Platform capability boundary and target-specific integrations.

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

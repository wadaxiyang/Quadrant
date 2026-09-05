// SPDX-License-Identifier: GPL-3.0-only

use quadrant_application::{
    CompletedViewState, DesktopSettings, FocusViewState, MaintenanceState, QuadrantsViewState,
    ReviewViewState, SystemTheme, ThemeMode, TodayViewState, UpdateViewState, UtcTimestamp,
};
use serde::{Deserialize, Serialize};

/// Normalized capabilities reported by the Agent, with no platform/UI handles.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Independent registered capabilities, not lifecycle states.
pub struct PlatformCapabilities {
    /// Login startup registration is available.
    pub autostart: bool,
    /// An operational tray/status item can reopen a GUI.
    pub tray: bool,
    /// The global Quick Add shortcut is registered.
    pub global_hotkey: bool,
    /// Native notification delivery is available.
    pub native_notifications: bool,
    /// Agent single-instance coordination is available.
    pub single_instance: bool,
}

/// Complete authoritative first-screen state supplied by the Agent.
///
/// The Agent must gather this at one serialized application boundary, before
/// publishing later events to the accepted connection. GUI drafts, navigation,
/// scroll positions, and window visibility are deliberately absent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppSnapshot {
    /// Agent clock at projection capture; Focus retains its actual time anchors.
    pub captured_at: UtcTimestamp,
    /// Inbox and all four quadrants.
    pub quadrants: QuadrantsViewState,
    /// Derived Today sections.
    pub today: TodayViewState,
    /// Current running/paused Focus session, choices, and Pomodoro settings.
    pub focus: FocusViewState,
    /// Current Review range and aggregates.
    pub review: ReviewViewState,
    /// Bounded Completed history.
    pub completed: CompletedViewState,
    /// Backup directory, newest backup, and staged restore state.
    pub maintenance: MaintenanceState,
    /// Persisted startup and window policy.
    pub desktop_settings: DesktopSettings,
    /// Persisted theme preference.
    pub theme_mode: ThemeMode,
    /// Normalized host appearance.
    pub system_theme: SystemTheme,
    /// Actual registered platform capabilities.
    pub platform_capabilities: PlatformCapabilities,
    /// Agent-owned distribution and application-version metadata.
    pub update_state: UpdateViewState,
}

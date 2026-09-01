//! Cross-platform startup-registration adapter.

use quadrant_application::{AutostartError, AutostartService};

/// Platform implementation of the application autostart capability.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformAutostartService;

impl AutostartService for PlatformAutostartService {
    fn is_supported(&self) -> bool {
        cfg!(target_os = "windows")
    }

    fn set_enabled(&self, enabled: bool, start_hidden: bool) -> Result<(), AutostartError> {
        set_platform_autostart(enabled, start_hidden)
    }
}

#[cfg(target_os = "windows")]
fn set_platform_autostart(enabled: bool, start_hidden: bool) -> Result<(), AutostartError> {
    crate::windows::set_autostart(enabled, start_hidden)
}

#[cfg(not(target_os = "windows"))]
fn set_platform_autostart(enabled: bool, _start_hidden: bool) -> Result<(), AutostartError> {
    if enabled {
        Err(AutostartError::new(
            "autostart is not implemented for this target",
        ))
    } else {
        Ok(())
    }
}

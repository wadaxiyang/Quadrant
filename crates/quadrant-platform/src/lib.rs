//! Platform capability boundary and target-specific integrations.

use std::{env, io, path::PathBuf};

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

/// Cross-platform application data paths resolved at the platform boundary.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformPaths;

impl PlatformPaths {
    /// Resolves and creates Quadrant's private data directory, returning the database path.
    ///
    /// `QUADRANT_DATA_DIR` can override the directory for development and packaging tests.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when no platform data directory can be resolved or created.
    pub fn database_path(self) -> io::Result<PathBuf> {
        let directory = env::var_os("QUADRANT_DATA_DIR")
            .map(PathBuf::from)
            .or_else(default_data_directory)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "no supported application data directory is available",
                )
            })?;
        std::fs::create_dir_all(&directory)?;
        Ok(directory.join("quadrant.db"))
    }
}

#[cfg(target_os = "windows")]
fn default_data_directory() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("Quadrant"))
}

#[cfg(target_os = "macos")]
fn default_data_directory() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from).map(|path| {
        path.join("Library")
            .join("Application Support")
            .join("Quadrant")
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn default_data_directory() -> Option<PathBuf> {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|path| path.join(".local").join("share"))
        })
        .map(|path| path.join("quadrant"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
fn default_data_directory() -> Option<PathBuf> {
    None
}

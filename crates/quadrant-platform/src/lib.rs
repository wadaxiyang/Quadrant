//! Platform capability boundary and target-specific integrations.

mod autostart;
mod desktop;
mod external;
mod instance;
mod notifications;
#[cfg(target_os = "windows")]
mod windows;

use std::{env, fmt, io, path::PathBuf, sync::Arc};

use jiff::{Timestamp, tz::TimeZone};
use quadrant_application::{
    CalendarError, DesktopEvent, LocalDate, SystemTheme, SystemThemeSource, TodayContext,
    TodayContextSource, UtcTimestamp,
};

const DATABASE_FILE_NAME: &str = "quadrant-rust.db";

pub use autostart::PlatformAutostartService;
pub use desktop::DesktopIntegration;
pub use external::PlatformExternalOpener;
pub use instance::{ActivationListener, SingleInstanceCoordinator};
pub use notifications::PlatformNotificationDelivery;

/// Reports a fatal startup failure before the Slint shell exists.
pub fn report_startup_error(error: &dyn std::fmt::Display) {
    let detail = error.to_string();
    eprintln!("Quadrant could not start: {detail}");
    #[cfg(target_os = "windows")]
    windows::show_startup_error(&detail);
}

/// Thread-safe desktop event destination implemented by the UI adapter.
pub type DesktopEventSink = Arc<dyn Fn(DesktopEvent) + Send + Sync>;

/// Normalized platform integration failure without leaking backend error types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformIntegrationError {
    detail: String,
}

impl PlatformIntegrationError {
    /// Wraps diagnostic context from a platform implementation.
    #[must_use]
    pub fn new(detail: impl fmt::Display) -> Self {
        Self {
            detail: detail.to_string(),
        }
    }
}

impl fmt::Display for PlatformIntegrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for PlatformIntegrationError {}

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
    /// Whether primary-instance activation forwarding is available.
    pub single_instance: bool,
}

/// Cross-platform theme source used until native observation is implemented.
///
/// The fallback is deliberately light and never makes startup fail. Target-specific
/// observation can replace this implementation inside this crate in M3.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformThemeSource;

impl SystemThemeSource for PlatformThemeSource {
    fn current_theme(&self) -> SystemTheme {
        current_system_theme()
    }
}

#[cfg(target_os = "windows")]
fn current_system_theme() -> SystemTheme {
    windows::current_system_theme()
}

#[cfg(not(target_os = "windows"))]
const fn current_system_theme() -> SystemTheme {
    SystemTheme::Light
}

/// DST-aware local calendar boundaries backed by the host system timezone.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformTodayContextSource;

impl TodayContextSource for PlatformTodayContextSource {
    fn today_context(&self, now: UtcTimestamp) -> Result<TodayContext, CalendarError> {
        let timestamp = Timestamp::from_second(now.unix_seconds()).map_err(CalendarError::new)?;
        let zoned = timestamp.to_zoned(TimeZone::system());
        let day_start = zoned.start_of_day().map_err(CalendarError::new)?;
        let next_day_start = zoned
            .tomorrow()
            .and_then(|tomorrow| tomorrow.start_of_day())
            .map_err(CalendarError::new)?;
        let date = zoned.date();
        let month = u8::try_from(date.month()).map_err(CalendarError::new)?;
        let day = u8::try_from(date.day()).map_err(CalendarError::new)?;
        Ok(TodayContext {
            local_date: LocalDate::from_calendar_date(i32::from(date.year()), month, day)
                .map_err(CalendarError::new)?,
            day_start_utc: UtcTimestamp::from_unix_seconds(day_start.timestamp().as_second()),
            next_day_start_utc: UtcTimestamp::from_unix_seconds(
                next_day_start.timestamp().as_second(),
            ),
        })
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
        // The read-only .NET reference used `quadrant.db` in the same directory.
        // Keep the rewrite's clean schema physically separate so opening Quadrant
        // can never attempt Rust migrations against a legacy database.
        Ok(directory.join(DATABASE_FILE_NAME))
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use quadrant_application::{TodayContextSource, UtcTimestamp};

    use super::{DATABASE_FILE_NAME, PlatformTodayContextSource};

    #[test]
    fn rewrite_database_does_not_reuse_the_legacy_file_name() {
        assert_eq!(DATABASE_FILE_NAME, "quadrant-rust.db");
        assert_ne!(DATABASE_FILE_NAME, "quadrant.db");
        assert_eq!(
            Path::new("profile").join(DATABASE_FILE_NAME),
            Path::new("profile").join("quadrant-rust.db")
        );
    }

    #[test]
    fn system_today_context_contains_the_requested_instant() {
        let now = UtcTimestamp::from_unix_seconds(1_788_192_000);
        let context = PlatformTodayContextSource
            .today_context(now)
            .expect("system calendar context");

        assert!(context.day_start_utc <= now);
        assert!(now < context.next_day_start_utc);
        assert!(context.day_start_utc < context.next_day_start_utc);
    }
}

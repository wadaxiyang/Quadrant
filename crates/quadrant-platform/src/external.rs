//! Native file-manager and browser opening behind the platform boundary.

use std::{path::Path, process::Command};

use quadrant_application::{ExternalOpener, PlatformActionError};

/// Cross-platform external target opener.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformExternalOpener;

impl ExternalOpener for PlatformExternalOpener {
    fn open_path(&self, path: &Path) -> Result<(), PlatformActionError> {
        if !path.is_dir() {
            return Err(PlatformActionError::new("target directory does not exist"));
        }
        spawn_path(path)
    }

    fn open_url(&self, url: &str) -> Result<(), PlatformActionError> {
        if !url.starts_with("https://") || url.chars().any(char::is_whitespace) {
            return Err(PlatformActionError::new(
                "only absolute HTTPS URLs are supported",
            ));
        }
        spawn_url(url)
    }
}

#[cfg(target_os = "windows")]
fn spawn_path(path: &Path) -> Result<(), PlatformActionError> {
    Command::new("explorer.exe")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(PlatformActionError::new)
}

#[cfg(target_os = "windows")]
fn spawn_url(url: &str) -> Result<(), PlatformActionError> {
    Command::new("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(PlatformActionError::new)
}

#[cfg(target_os = "macos")]
fn spawn_path(path: &Path) -> Result<(), PlatformActionError> {
    spawn_open_command("open", path.as_os_str())
}

#[cfg(target_os = "macos")]
fn spawn_url(url: &str) -> Result<(), PlatformActionError> {
    spawn_open_command("open", url)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_path(path: &Path) -> Result<(), PlatformActionError> {
    spawn_open_command("xdg-open", path.as_os_str())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_url(url: &str) -> Result<(), PlatformActionError> {
    spawn_open_command("xdg-open", url)
}

#[cfg(any(target_os = "macos", all(unix, not(target_os = "macos"))))]
fn spawn_open_command(
    executable: &str,
    target: impl AsRef<std::ffi::OsStr>,
) -> Result<(), PlatformActionError> {
    Command::new(executable)
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(PlatformActionError::new)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
fn spawn_path(_path: &Path) -> Result<(), PlatformActionError> {
    Err(PlatformActionError::new(
        "opening folders is unsupported on this platform",
    ))
}

#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
fn spawn_url(_url: &str) -> Result<(), PlatformActionError> {
    Err(PlatformActionError::new(
        "opening URLs is unsupported on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use quadrant_application::ExternalOpener;

    use super::PlatformExternalOpener;

    #[test]
    fn opener_rejects_non_https_and_whitespace_urls_before_spawning() {
        assert!(
            PlatformExternalOpener
                .open_url("http://example.com")
                .is_err()
        );
        assert!(
            PlatformExternalOpener
                .open_url("https://example.com/bad path")
                .is_err()
        );
    }
}

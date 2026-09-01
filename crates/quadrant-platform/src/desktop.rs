//! Desktop hotkey/tray integration lifecycle.

use crate::{DesktopEventSink, PlatformCapabilities, PlatformIntegrationError};

/// Running platform desktop integration.
pub struct DesktopIntegration {
    #[cfg(target_os = "windows")]
    inner: Option<crate::windows::WindowsDesktopIntegration>,
    capabilities: PlatformCapabilities,
}

impl DesktopIntegration {
    /// Starts target-specific hotkey/tray integration.
    ///
    /// # Errors
    ///
    /// Returns a platform error when the integration thread cannot be created or initialized.
    pub fn start(sink: DesktopEventSink) -> Result<Self, PlatformIntegrationError> {
        start_integration(sink)
    }

    /// Returns the capabilities that initialized successfully.
    #[must_use]
    pub const fn capabilities(&self) -> PlatformCapabilities {
        self.capabilities
    }

    /// Stops platform event registration and joins its event thread.
    pub fn shutdown(mut self) {
        #[cfg(target_os = "windows")]
        if let Some(inner) = self.inner.take() {
            inner.shutdown();
        }
    }
}

#[cfg(target_os = "windows")]
fn start_integration(
    sink: DesktopEventSink,
) -> Result<DesktopIntegration, PlatformIntegrationError> {
    let (inner, capabilities) = crate::windows::WindowsDesktopIntegration::start(sink)?;
    Ok(DesktopIntegration {
        inner: Some(inner),
        capabilities,
    })
}

#[cfg(not(target_os = "windows"))]
fn start_integration(
    _sink: DesktopEventSink,
) -> Result<DesktopIntegration, PlatformIntegrationError> {
    Ok(DesktopIntegration {
        capabilities: PlatformCapabilities::default(),
    })
}

impl std::fmt::Debug for DesktopIntegration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DesktopIntegration")
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

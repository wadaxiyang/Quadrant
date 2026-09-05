//! Cross-platform single-instance ownership and activation forwarding.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::Write,
    path::Path,
    thread,
    time::Duration,
};

use interprocess::local_socket::{
    ConnectOptions, GenericNamespaced, ListenerOptions, ToNsName as _,
    tokio::{Listener, prelude::*},
};
use quadrant_application::DesktopEvent;
use tokio::{io::AsyncReadExt, sync::oneshot};

use crate::{DesktopEventSink, PlatformIntegrationError};

/// Process-lifetime ownership guard plus activation socket identity.
pub struct SingleInstanceCoordinator {
    _guard: single_instance::SingleInstance,
    primary: bool,
    socket_name: String,
    profile_identity: u64,
}

impl SingleInstanceCoordinator {
    /// Claims the instance identity associated with one Quadrant data store.
    ///
    /// # Errors
    ///
    /// Returns a platform error if the OS ownership primitive cannot be created.
    pub fn claim(database_path: &Path) -> Result<Self, PlatformIntegrationError> {
        let database_path = crate::ipc::canonical_database_path(database_path)
            .map_err(PlatformIntegrationError::new)?;
        let identity = instance_identity(&database_path);
        let guard = single_instance::SingleInstance::new(&format!(
            "Quadrant.Tasks.Instance.{identity:016x}"
        ))
        .map_err(PlatformIntegrationError::new)?;
        let primary = guard.is_single();
        Ok(Self {
            _guard: guard,
            primary,
            socket_name: format!("quadrant-tasks-activation-{identity:016x}"),
            profile_identity: identity,
        })
    }

    /// Returns whether this process owns the primary-instance lock.
    #[must_use]
    pub const fn is_primary(&self) -> bool {
        self.primary
    }

    /// Binds the secure Agent endpoint only while this process owns the profile.
    ///
    /// # Errors
    /// Rejects secondary ownership and native endpoint/permission failures.
    pub fn bind_agent_listener(
        &self,
        endpoint: &crate::AgentEndpoint,
    ) -> Result<crate::AgentListener, PlatformIntegrationError> {
        if !self.primary || self.profile_identity != endpoint.profile_identity() {
            return Err(PlatformIntegrationError::new(
                "a secondary instance cannot bind Agent IPC",
            ));
        }
        endpoint.bind().map_err(PlatformIntegrationError::new)
    }

    /// Binds the primary activation listener on the current Tokio runtime.
    ///
    /// # Errors
    ///
    /// Returns an IPC error when the activation endpoint cannot be created.
    pub fn bind_activation_listener(&self) -> Result<ActivationListener, PlatformIntegrationError> {
        if !self.primary {
            return Err(PlatformIntegrationError::new(
                "a secondary instance cannot bind the activation listener",
            ));
        }
        let name = self
            .socket_name
            .as_str()
            .to_ns_name::<GenericNamespaced>()
            .map_err(PlatformIntegrationError::new)?;
        let listener = ListenerOptions::new()
            .name(name)
            .try_overwrite(true)
            .create_tokio()
            .map_err(PlatformIntegrationError::new)?;
        Ok(ActivationListener { listener })
    }

    /// Requests that the already-running primary instance show its main window.
    ///
    /// A short bounded retry handles the primary's startup interval between lock
    /// acquisition and activation-listener creation.
    ///
    /// # Errors
    ///
    /// Returns an IPC error if the primary cannot be reached within the retry window.
    pub fn notify_primary(&self) -> Result<(), PlatformIntegrationError> {
        if self.primary {
            return Err(PlatformIntegrationError::new(
                "the primary instance cannot redirect activation to itself",
            ));
        }
        let mut last_error = None;
        for _ in 0..20 {
            let name = self
                .socket_name
                .as_str()
                .to_ns_name::<GenericNamespaced>()
                .map_err(PlatformIntegrationError::new)?;
            match ConnectOptions::new().name(name).connect_sync() {
                Ok(mut stream) => {
                    stream
                        .write_all(b"show\n")
                        .map_err(PlatformIntegrationError::new)?;
                    return Ok(());
                }
                Err(error) => {
                    last_error = Some(error);
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
        Err(PlatformIntegrationError::new(last_error.map_or_else(
            || "the primary activation endpoint was unavailable".to_owned(),
            |error| error.to_string(),
        )))
    }
}

/// Primary-instance async activation receiver.
#[derive(Debug)]
pub struct ActivationListener {
    listener: Listener,
}

impl ActivationListener {
    /// Accepts redirected launches until shutdown is requested.
    pub async fn run(self, sink: DesktopEventSink, mut shutdown: oneshot::Receiver<()>) {
        loop {
            tokio::select! {
                _ = &mut shutdown => break,
                connection = self.listener.accept() => {
                    match connection {
                        Ok(mut connection) => {
                            let mut message = [0_u8; 32];
                            if let Ok(Ok(size)) = tokio::time::timeout(
                                Duration::from_secs(1),
                                connection.read(&mut message),
                            ).await
                                && message[..size].starts_with(b"show")
                            {
                                sink(DesktopEvent::ShowMainWindow);
                            }
                        }
                        Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                    }
                }
            }
        }
    }
}

pub(crate) fn instance_identity(database_path: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    #[cfg(target_os = "windows")]
    database_path
        .to_string_lossy()
        .to_lowercase()
        .hash(&mut hasher);
    #[cfg(not(target_os = "windows"))]
    database_path.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::instance_identity;

    #[test]
    fn instance_identity_is_stable_and_data_store_specific() {
        let first = instance_identity(Path::new("C:/Quadrant/profile-a/quadrant.db"));
        assert_eq!(
            first,
            instance_identity(Path::new("C:/Quadrant/profile-a/quadrant.db"))
        );
        assert_ne!(
            first,
            instance_identity(Path::new("C:/Quadrant/profile-b/quadrant.db"))
        );
    }
}

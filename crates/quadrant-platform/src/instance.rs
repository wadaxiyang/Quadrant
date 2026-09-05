//! Cross-platform single-instance ownership and activation forwarding.

use std::{io::Write, path::Path, thread, time::Duration};

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

/// Stable profile naming contract: callers first use `canonical_database_path`.
/// Windows hashes normalized UTF-16LE code units (no BOM/terminator); Unix hashes
/// exact native path bytes. Never use `std::hash` or lossy string conversion here.
pub(crate) fn instance_identity(database_path: &Path) -> u64 {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        windows_path_identity(database_path.as_os_str().encode_wide())
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        stable_path_hash(database_path.as_os_str().as_bytes().iter().copied())
    }
}

// FNV-1a-64: XOR each byte, then multiply modulo 2^64. Constants, byte encoding
// and normalization are protocol naming rules, pinned by fixed test vectors.
// Algorithm definition: https://www.rfc-editor.org/rfc/rfc9923.html#section-2
fn stable_path_hash(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes.into_iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(any(target_os = "windows", test))]
fn windows_path_identity(units: impl IntoIterator<Item = u16>) -> u64 {
    // Fold ASCII only: Unicode case tables must not silently change the identity
    // with a toolchain upgrade. Non-ASCII units (including surrogates) stay exact.
    let mut units: Vec<u16> = units
        .into_iter()
        .map(|unit| match unit {
            0x41..=0x5a => unit + 0x20,
            0x2f => 0x5c,
            _ => unit,
        })
        .collect();
    if units.starts_with(&[0x5c, 0x5c, 0x3f, 0x5c, 0x75, 0x6e, 0x63, 0x5c]) {
        // \\?\UNC\server\share -> \\server\share
        units.drain(2..8);
    } else if units.starts_with(&[0x5c, 0x5c, 0x3f, 0x5c])
        && units
            .get(4)
            .is_some_and(|unit| (0x61..=0x7a).contains(unit))
        && units.get(5..7) == Some(&[0x3a, 0x5c])
    {
        // \\?\C:\path -> C:\path; leave other device namespaces distinct.
        units.drain(..4);
    }
    stable_path_hash(units.into_iter().flat_map(u16::to_le_bytes))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{instance_identity, stable_path_hash, windows_path_identity};

    #[test]
    fn stable_hash_has_fixed_byte_vectors() {
        for (bytes, expected) in [
            (b"".as_slice(), 0xcbf2_9ce4_8422_2325),
            (b"a".as_slice(), 0xaf63_dc4c_8601_ec8c),
            (b"foobar".as_slice(), 0x8594_4171_f739_67e8),
            (
                b"/home/example/.local/share/Quadrant/quadrant-rust.db".as_slice(),
                0x886d_0eec_5293_5341,
            ),
            (
                b"/home/example/.local/share/quadrant/quadrant-rust.db".as_slice(),
                0x58f6_44d7_1ca9_5d61,
            ),
            (
                b"/tmp/profile-\xff/quadrant.db".as_slice(),
                0x1267_e0fc_e266_b926,
            ),
        ] {
            assert_eq!(stable_path_hash(bytes.iter().copied()), expected);
        }
    }

    #[test]
    fn windows_normalization_has_fixed_profile_vectors() {
        for (path, expected) in [
            (
                "C:/Users/example/AppData/Local/Quadrant/quadrant-rust.db",
                0x7023_0d67_6261_96c7,
            ),
            (
                r"c:\users\example\appdata\local\quadrant\quadrant-rust.db",
                0x7023_0d67_6261_96c7,
            ),
            (
                r"\\?\C:\Users\Example\AppData\Local\Quadrant\QUADRANT-RUST.DB",
                0x7023_0d67_6261_96c7,
            ),
            (
                r"\\SERVER\Share\Quadrant\quadrant-rust.db",
                0xe9f0_0875_678a_cf6c,
            ),
            (
                r"\\?\UNC\Server\Share\Quadrant\quadrant-rust.db",
                0xe9f0_0875_678a_cf6c,
            ),
            ("C:/Quadrant/profile-a/quadrant.db", 0x6cda_4060_eee5_fe9f),
            ("C:/Quadrant/profile-b/quadrant.db", 0xa29f_afd8_a205_46d4),
            (
                "C:/Users/示例/Quadrant/quadrant-rust.db",
                0xa0f7_7aed_fb26_97fc,
            ),
        ] {
            assert_eq!(
                windows_path_identity(path.encode_utf16()),
                expected,
                "{path}"
            );
        }
    }

    #[test]
    fn windows_encoding_preserves_unpaired_utf16() {
        assert_eq!(
            windows_path_identity([0x43, 0x3a, 0x5c, 0xd800]),
            0xf7ae_960d_52af_9658
        );
        assert_ne!(
            windows_path_identity([0x43, 0x3a, 0x5c, 0xd800]),
            windows_path_identity([0x43, 0x3a, 0x5c, 0xfffd])
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn native_windows_identity_matches_fixed_vector() {
        assert_eq!(
            instance_identity(Path::new(
                "C:/Users/example/AppData/Local/Quadrant/quadrant-rust.db"
            )),
            0x7023_0d67_6261_96c7
        );
    }

    #[cfg(unix)]
    #[test]
    fn native_unix_identity_preserves_bytes_and_case() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
        for (bytes, expected) in [
            (
                b"/home/example/.local/share/Quadrant/quadrant-rust.db".as_slice(),
                0x886d_0eec_5293_5341,
            ),
            (
                b"/home/example/.local/share/quadrant/quadrant-rust.db".as_slice(),
                0x58f6_44d7_1ca9_5d61,
            ),
            (
                b"/tmp/profile-\xff/quadrant.db".as_slice(),
                0x1267_e0fc_e266_b926,
            ),
        ] {
            assert_eq!(
                instance_identity(Path::new(OsStr::from_bytes(bytes))),
                expected
            );
        }
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn canonical_aliases_share_lock_and_endpoint_identity() {
        use super::SingleInstanceCoordinator;
        let directory = std::env::temp_dir().join(format!(
            "quadrant-identity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        {
            let original = directory.join("quadrant-rust.db");
            let alias = directory.join(".").join("QUADRANT-RUST.DB");
            let primary = SingleInstanceCoordinator::claim(&original).unwrap();
            assert!(primary.is_primary());
            let secondary = SingleInstanceCoordinator::claim(&alias).unwrap();
            assert!(!secondary.is_primary());
            let endpoint = crate::AgentEndpoint::for_database(&alias).unwrap();
            assert_eq!(primary.profile_identity, endpoint.profile_identity());
            let listener = primary.bind_agent_listener(&endpoint).unwrap();
            // Both normalized paths also select the same actual named pipe.
            let client = crate::AgentEndpoint::for_database(&original).unwrap();
            let (connected, accepted) =
                tokio::time::timeout(std::time::Duration::from_secs(3), async {
                    tokio::join!(client.connect(), listener.accept())
                })
                .await
                .unwrap();
            let client = connected.unwrap();
            let (server, _) = accepted.unwrap();
            client.close().await;
            server.close().await;
        }
        std::fs::remove_dir(&directory).unwrap();
    }
}

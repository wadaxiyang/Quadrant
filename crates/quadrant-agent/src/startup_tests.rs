// SPDX-License-Identifier: GPL-3.0-only
//! Startup and real IPC regression with isolated storage and no native desktop effects.

use super::*;
use quadrant_application::{AutostartError, PlatformActionError};
use quadrant_protocol::{
    ClientHello, ClientMessage, GuiDisposition, GuiLaunchMode, PROTOCOL_VERSION, RequestId,
    ServerMessage, SessionId,
    codec::{read_message_async, write_message_async},
};
use std::{path::PathBuf, sync::atomic::Ordering, time::Duration};

struct Profile(PathBuf);
impl Profile {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "quadrant-startup-test-{}",
            SessionId::generate().as_uuid()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("quadrant-rust.db")
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct TestHost;
impl AutostartService for TestHost {
    fn is_supported(&self) -> bool {
        false
    }
    fn set_enabled(&self, _enabled: bool, _hidden: bool) -> Result<(), AutostartError> {
        Ok(())
    }
}
impl ExternalOpener for TestHost {
    fn open_path(&self, _path: &Path) -> Result<(), PlatformActionError> {
        panic!("startup must not open external paths")
    }
    fn open_url(&self, _url: &str) -> Result<(), PlatformActionError> {
        panic!("startup must not open external URLs")
    }
}
impl quadrant_platform::GuiLauncher for TestHost {
    fn launch_main(&self) -> std::io::Result<quadrant_platform::GuiProcess> {
        panic!("headless startup must not launch GUI")
    }
    fn launch_quick_add(&self) -> std::io::Result<quadrant_platform::GuiProcess> {
        panic!("headless startup must not launch capture")
    }
}

fn host() -> HostServices {
    HostServices {
        gui_launcher: Arc::new(TestHost),
        clock: Arc::new(SystemClock),
        autostart: Arc::new(TestHost),
        reminders: Arc::new(|_| Ok(())),
        opener: Arc::new(TestHost),
        focus_completed: Arc::new(|| Ok(())),
        desktop_integration: false,
    }
}

#[tokio::test]
async fn headless_startup_defers_snapshot_until_first_gui_request() {
    let profile = Profile::new();
    let database = profile.database();
    let endpoint = AgentEndpoint::for_database(&database).unwrap();
    let agent = tokio::task::spawn_blocking(move || Agent::open(&database, host()))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    // The fresh database was migrated before IPC binding, without UI projections.
    assert!(agent.services.store.schema_version().unwrap() > 0);
    let calls = agent.services.snapshot_calls.clone();
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let (stop, stopped) = oneshot::channel();
    let worker = tokio::spawn(agent.run(stopped));
    tokio::time::timeout(Duration::from_secs(8), async {
        let mut stream = endpoint.connect().await.unwrap();
        write_message_async(
            &mut stream,
            &ClientMessage::Hello(ClientHello {
                protocol_version: PROTOCOL_VERSION,
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
                process_id: std::process::id(),
                mode: GuiLaunchMode::Main,
            }),
        )
        .await
        .unwrap();
        let Some(ServerMessage::HelloAck(ack)) = read_message_async(&mut stream).await.unwrap()
        else {
            panic!("expected HelloAck");
        };
        assert_eq!(ack.disposition, GuiDisposition::Accepted);
        // Runtime startup and Hello acceptance do not construct an AppSnapshot.
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let request_id = RequestId::new(1).unwrap();
        write_message_async(
            &mut stream,
            &ClientMessage::GetInitialSnapshot { request_id },
        )
        .await
        .unwrap();
        let Some(ServerMessage::InitialSnapshot {
            request_id: actual,
            snapshot,
        }) = read_message_async(&mut stream).await.unwrap()
        else {
            panic!("expected complete initial snapshot");
        };
        assert_eq!(actual, request_id);
        assert!(snapshot.platform_capabilities.single_instance);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        stream.close().await;
        stop.send(()).unwrap();
        worker.await.unwrap().unwrap();
    })
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn corrupt_database_still_fails_before_ipc_is_bound() {
    let profile = Profile::new();
    let database = profile.database();
    let endpoint = AgentEndpoint::for_database(&database).unwrap();
    std::fs::write(&database, b"not a SQLite database").unwrap();
    let result = tokio::task::spawn_blocking(move || Agent::open(&database, host()))
        .await
        .unwrap();
    assert!(matches!(result, Err(AgentError::Repository(_))));
    assert!(endpoint.connect().await.is_err());
}

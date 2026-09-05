// SPDX-License-Identifier: GPL-3.0-only
//! Real current-user local streams with a scripted peer. No UI, database or native effects.

use super::*;
use quadrant_application::{
    ApplicationEvent, QuickAddSubmission, TaskPlacement, UiIntent, UtcTimestamp,
};
use quadrant_platform::{AgentListener, AgentStream, SingleInstanceCoordinator};
use quadrant_protocol::{
    ClientMessage, CommandOutcome, GuiDisposition, ServerEvent, ServerHello, ServerMessage,
    SessionId,
    codec::{read_message_async, write_message_async},
};
use std::{path::PathBuf, time::Duration};
use tokio::io::AsyncWriteExt;

struct Peer {
    listener: AgentListener,
    endpoint: AgentEndpoint,
    _guard: SingleInstanceCoordinator,
    directory: PathBuf,
}
impl Peer {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "quadrant-client-{}",
            SessionId::generate().as_uuid()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("quadrant-rust.db");
        let guard = SingleInstanceCoordinator::claim(&path).unwrap();
        let endpoint = AgentEndpoint::for_database(&path).unwrap();
        let listener = guard.bind_agent_listener(&endpoint).unwrap();
        Self {
            listener,
            endpoint,
            _guard: guard,
            directory,
        }
    }
    async fn accept(&self, snapshot: AppSnapshot) -> AgentStream {
        let (mut stream, identity) = self.listener.accept().await.unwrap();
        let ClientMessage::Hello(hello) = read(&mut stream).await else {
            panic!("Hello first");
        };
        if let Some(pid) = identity.process_id {
            assert_eq!(pid, hello.process_id);
        }
        let ack = ServerHello::negotiate(
            &hello,
            "different-app-version",
            false,
            SessionId::generate(),
        );
        write_message_async(&mut stream, &ServerMessage::HelloAck(ack))
            .await
            .unwrap();
        let ClientMessage::GetInitialSnapshot { request_id } = read(&mut stream).await else {
            panic!("snapshot first");
        };
        assert_eq!(request_id.get(), 1);
        write_message_async(
            &mut stream,
            &ServerMessage::InitialSnapshot {
                request_id,
                snapshot: Box::new(snapshot),
            },
        )
        .await
        .unwrap();
        stream
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn snapshot() -> AppSnapshot {
    serde_json::from_str(include_str!(
        "../../../quadrant-protocol/tests/fixtures/snapshot_v1.json"
    ))
    .unwrap()
}

#[tokio::test]
async fn user_launch_starts_missing_agent_once_before_loading_snapshot() {
    let directory = std::env::temp_dir().join(format!(
        "quadrant-bootstrap-{}",
        SessionId::generate().as_uuid()
    ));
    std::fs::create_dir(&directory).unwrap();
    let database = directory.join("quadrant-rust.db");
    let guard = SingleInstanceCoordinator::claim(&database).unwrap();
    let endpoint = AgentEndpoint::for_database(&database).unwrap();
    let server_endpoint = endpoint.clone();
    let (started, starting) = oneshot::channel::<Peer>();
    let server = tokio::spawn(async move {
        let peer = starting.await.unwrap();
        let mut stream = peer.accept(snapshot()).await;
        assert!(
            read_message_async::<_, ClientMessage>(&mut stream)
                .await
                .unwrap()
                .is_none()
        );
    });
    let mut calls = 0;
    let result = GuiClient::connect_or_start(endpoint, GuiLaunchMode::Main, true, || {
        calls += 1;
        let listener = guard.bind_agent_listener(&server_endpoint).unwrap();
        assert!(
            started
                .send(Peer {
                    listener,
                    endpoint: server_endpoint,
                    _guard: guard,
                    directory
                })
                .is_ok()
        );
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(result.as_ref().unwrap().0.snapshot(), &snapshot());
    drop(result);
    server.await.unwrap();
}

#[tokio::test]
async fn agent_launched_child_does_not_resurrect_missing_parent() {
    let peer = Peer::new();
    let endpoint = peer.endpoint.clone();
    drop(peer);
    let result = GuiClient::connect_or_start(endpoint, GuiLaunchMode::Main, false, || {
        panic!("must not restart parent")
    })
    .await;
    assert!(result.err().unwrap().agent_absent());
    assert!(!ClientError::Incompatible.agent_absent());
    assert!(!ClientError::Timeout.agent_absent());
    assert!(
        !ClientError::Io(std::io::Error::from(std::io::ErrorKind::PermissionDenied)).agent_absent()
    );
}
fn command() -> GuiCommand {
    UiIntent::SubmitQuickAdd(QuickAddSubmission {
        title: "Only once".to_owned(),
        placement: TaskPlacement::Inbox,
    })
    .into()
}
async fn read(stream: &mut AgentStream) -> ClientMessage {
    tokio::time::timeout(Duration::from_secs(5), read_message_async(stream))
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}
async fn update(updates: &mut mpsc::UnboundedReceiver<ClientUpdate>) -> ClientUpdate {
    tokio::time::timeout(Duration::from_secs(5), updates.recv())
        .await
        .unwrap()
        .unwrap()
}
async fn ready(updates: &mut mpsc::UnboundedReceiver<ClientUpdate>) {
    loop {
        if matches!(
            update(updates).await,
            ClientUpdate::Connection {
                state: ConnectionState::Ready,
                ..
            }
        ) {
            return;
        }
    }
}
async fn start(
    endpoint: AgentEndpoint,
) -> (
    ClientHandle,
    mpsc::UnboundedReceiver<ClientUpdate>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    start_mode(endpoint, GuiLaunchMode::Main).await
}

async fn start_mode(
    endpoint: AgentEndpoint,
    mode: GuiLaunchMode,
) -> (
    ClientHandle,
    mpsc::UnboundedReceiver<ClientUpdate>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let (client, handle) = GuiClient::connect(endpoint, mode).await.unwrap().unwrap();
    assert_eq!(client.snapshot(), &snapshot());
    let (sender, mut updates) = mpsc::unbounded_channel();
    let (stop, shutdown) = oneshot::channel();
    let worker = tokio::spawn(client.run(
        Arc::new(move |event| {
            let _ = sender.send(event);
        }),
        shutdown,
    ));
    ready(&mut updates).await;
    (handle, updates, stop, worker)
}

#[tokio::test]
async fn command_waits_for_correlated_result_and_fragmented_pushes_remain_ordered() {
    let peer = Peer::new();
    let endpoint = peer.endpoint.clone();
    let (release, released) = oneshot::channel();
    let (sent, received) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut stream = peer.accept(snapshot()).await;
        let ClientMessage::Command {
            request_id,
            command: actual,
        } = read(&mut stream).await
        else {
            panic!("command");
        };
        assert_eq!(request_id.get(), 2);
        assert_eq!(actual, command());
        sent.send(()).unwrap();
        released.await.unwrap();
        let event =
            ServerMessage::Event(ApplicationEvent::OperationSucceeded("Saved".into()).into());
        let mut bytes = Vec::new();
        quadrant_protocol::codec::write_message(&mut bytes, &event).unwrap();
        for chunk in bytes.chunks(2) {
            stream.write_all(chunk).await.unwrap();
            tokio::task::yield_now().await;
        }
        write_message_async(
            &mut stream,
            &ServerMessage::CommandResult {
                request_id,
                outcome: CommandOutcome::Succeeded,
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            read(&mut stream).await,
            ClientMessage::GuiClosing { .. }
        ));
        stream.close().await;
    });
    let (handle, mut updates, stop, worker) = start(endpoint).await;
    handle.submit(command()).unwrap();
    received.await.unwrap();
    assert!(handle.submit(command()).is_err());
    assert!(matches!(
        update(&mut updates).await,
        ClientUpdate::Connection {
            state: ConnectionState::Busy,
            ..
        }
    ));
    assert!(updates.try_recv().is_err()); // no optimistic completion
    release.send(()).unwrap();
    assert!(matches!(
        update(&mut updates).await,
        ClientUpdate::Event(ServerEvent::Application(_))
    ));
    assert!(matches!(
        update(&mut updates).await,
        ClientUpdate::CommandFinished {
            outcome: CommandOutcome::Succeeded,
            ..
        }
    ));
    ready(&mut updates).await;
    stop.send(()).unwrap();
    worker.await.unwrap();
    server.await.unwrap();
    assert!(handle.submit(command()).is_err());
}

#[tokio::test]
async fn lost_response_reconnects_with_new_snapshot_and_never_replays_old_commands() {
    let peer = Peer::new();
    let endpoint = peer.endpoint.clone();
    let server = tokio::spawn(async move {
        let mut first = peer.accept(snapshot()).await;
        assert!(matches!(
            read(&mut first).await,
            ClientMessage::Command { .. }
        ));
        drop(first); // mutation happened but its response was lost
        let mut fresh = snapshot();
        fresh.captured_at = UtcTimestamp::from_unix_seconds(123);
        let mut second = peer.accept(fresh).await;
        // No command from the old connection may appear here.
        assert!(matches!(
            read(&mut second).await,
            ClientMessage::GuiClosing { .. }
        ));
        second.close().await;
    });
    let (handle, mut updates, stop, worker) = start(endpoint).await;
    handle.submit(command()).unwrap();
    loop {
        match update(&mut updates).await {
            ClientUpdate::Connection {
                state: ConnectionState::Reconnecting,
                ..
            } => assert!(handle.submit(command()).is_err()),
            ClientUpdate::Snapshot(state) if state.captured_at.unix_seconds() == 123 => break,
            ClientUpdate::CommandFinished { .. } => panic!("lost response has unknown outcome"),
            _ => {}
        }
    }
    ready(&mut updates).await;
    // Simulate an enqueue delayed across reconnect after the old callback's CAS.
    handle
        .sender
        .try_send(Submission {
            epoch: 4,
            command: command(),
        })
        .unwrap();
    tokio::task::yield_now().await;
    stop.send(()).unwrap();
    worker.await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn mismatch_and_existing_gui_redirect_finish_before_presentation() {
    for disposition in [
        GuiDisposition::RejectIncompatibleVersion,
        GuiDisposition::ActivateExistingAndExit,
    ] {
        let peer = Peer::new();
        let endpoint = peer.endpoint.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = peer.listener.accept().await.unwrap();
            assert!(matches!(read(&mut stream).await, ClientMessage::Hello(_)));
            write_message_async(
                &mut stream,
                &ServerMessage::HelloAck(ServerHello {
                    protocol_version: quadrant_protocol::PROTOCOL_VERSION,
                    agent_version: "other".into(),
                    session_id: None,
                    disposition,
                }),
            )
            .await
            .unwrap();
            stream.close().await;
        });
        let result = GuiClient::connect(endpoint, GuiLaunchMode::Main).await;
        match disposition {
            GuiDisposition::RejectIncompatibleVersion => {
                assert!(matches!(result, Err(ClientError::Incompatible)));
            }
            _ => assert!(matches!(result, Ok(None))),
        }
        server.await.unwrap();
    }
}

#[tokio::test]
async fn shutdown_cancels_partial_reader_even_with_retained_callback_handle() {
    let peer = Peer::new();
    let endpoint = peer.endpoint.clone();
    let (sent, received) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut stream = peer.accept(snapshot()).await;
        stream.write_all(&[0, 0]).await.unwrap(); // unfinished server frame
        sent.send(()).unwrap();
        assert!(matches!(
            read(&mut stream).await,
            ClientMessage::GuiClosing { .. }
        ));
    });
    let (handle, _updates, stop, worker) = start(endpoint).await;
    received.await.unwrap();
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();
    assert!(handle.submit(command()).is_err());
    server.await.unwrap();
}

#[tokio::test]
async fn agent_full_exit_does_not_trigger_reconnect() {
    let peer = Peer::new();
    let endpoint = peer.endpoint.clone();
    let server = tokio::spawn(async move {
        let mut stream = peer.accept(snapshot()).await;
        write_message_async(
            &mut stream,
            &ServerMessage::Event(ServerEvent::AgentShuttingDown),
        )
        .await
        .unwrap();
        stream.close().await;
    });
    let (handle, mut updates, _stop, worker) = start(endpoint).await;
    assert!(matches!(
        update(&mut updates).await,
        ClientUpdate::Event(ServerEvent::AgentShuttingDown)
    ));
    worker.await.unwrap();
    assert!(handle.submit(command()).is_err());
    server.await.unwrap();
}

#[tokio::test]
async fn exhausted_recovery_stays_offline_and_rejects_submissions() {
    let peer = Peer::new();
    let endpoint = peer.endpoint.clone();
    let (release, released) = oneshot::channel();
    let server = tokio::spawn(async move {
        let stream = peer.accept(snapshot()).await;
        released.await.unwrap();
        drop(stream);
    });
    let (handle, mut updates, _stop, worker) = start(endpoint).await;
    release.send(()).unwrap();
    server.await.unwrap();
    loop {
        if matches!(
            update(&mut updates).await,
            ClientUpdate::Connection {
                state: ConnectionState::Unavailable,
                ..
            }
        ) {
            break;
        }
    }
    worker.await.unwrap();
    assert!(handle.submit(command()).is_err());
}

#[tokio::test]
async fn capture_reconnect_redirect_retains_offline_presentation_without_replay() {
    let peer = Peer::new();
    let endpoint = peer.endpoint.clone();
    let server = tokio::spawn(async move {
        let mut first = peer.accept(snapshot()).await;
        assert!(matches!(
            read(&mut first).await,
            ClientMessage::Command { .. }
        ));
        first.close().await; // Drop a response after receiving the mutation.
        let (mut second, _) = peer.listener.accept().await.unwrap();
        let ClientMessage::Hello(hello) = read(&mut second).await else {
            panic!("hello required");
        };
        assert_eq!(hello.mode, GuiLaunchMode::QuickAdd);
        let ack = ServerHello::negotiate(&hello, "test", true, SessionId::generate());
        write_message_async(&mut second, &ServerMessage::HelloAck(ack))
            .await
            .unwrap();
        assert!(
            read_message_async::<_, ClientMessage>(&mut second)
                .await
                .unwrap()
                .is_none()
        );
    });
    let (handle, mut updates, _stop, worker) = start_mode(endpoint, GuiLaunchMode::QuickAdd).await;
    handle.submit(command()).unwrap();
    loop {
        match update(&mut updates).await {
            ClientUpdate::Connection {
                state: ConnectionState::Unavailable,
                message,
            } => {
                assert!(message.contains("copy any unsaved text"));
                assert!(handle.submit(command()).is_err());
                break;
            }
            ClientUpdate::Event(ServerEvent::ExitGui) | ClientUpdate::CommandFinished { .. } => {
                panic!("unknown draft must remain visible")
            }
            _ => {}
        }
    }
    worker.await.unwrap();
    server.await.unwrap();
}

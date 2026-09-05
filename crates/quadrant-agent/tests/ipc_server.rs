// SPDX-License-Identifier: GPL-3.0-only
//! Real local IPC with isolated storage and deterministic host ports; no desktop effects.

use quadrant_agent::{Agent, HostServices};
use quadrant_application::*;
use quadrant_platform::{AgentEndpoint, AgentStream};
use quadrant_protocol::{
    AppSnapshot, ClientHello, ClientMessage, CommandOutcome, GuiCommand, GuiDisposition,
    GuiLaunchMode, PROTOCOL_VERSION, ProtocolError, RequestId, ServerEvent, ServerHello,
    ServerMessage, SessionId,
    codec::{read_message_async, write_message_async},
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

const NOW: i64 = 1_788_560_000;
const TIMEOUT: Duration = Duration::from_secs(8);

struct FakeGui(mpsc::UnboundedSender<oneshot::Sender<()>>);
impl quadrant_platform::GuiLauncher for FakeGui {
    fn launch_main(&self) -> std::io::Result<quadrant_platform::GuiProcess> {
        let (exit, exited) = oneshot::channel();
        self.0.send(exit).unwrap();
        Ok(quadrant_platform::GuiProcess {
            id: std::process::id(),
            completion: Box::pin(async move {
                let _ = exited.await;
                Ok(())
            }),
        })
    }
}

#[tokio::test]
async fn tray_reopens_after_gui_close_and_crash_and_full_exit_stops_agent() {
    let (sender, mut launches) = mpsc::unbounded_channel();
    let harness = Harness::start_with(Profile::new(), false, Arc::new(FakeGui(sender))).await;
    (harness.desktop)(DesktopEvent::OpenQuickAdd);
    let first = tokio::time::timeout(TIMEOUT, launches.recv())
        .await
        .unwrap()
        .unwrap();
    (harness.desktop)(DesktopEvent::ShowMainWindow);
    let mut client = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    client.snapshot().await;
    assert!(matches!(
        receive(&mut client.stream).await,
        Some(ServerMessage::Event(ServerEvent::OpenQuickAdd))
    ));
    assert!(launches.try_recv().is_err());
    (harness.desktop)(DesktopEvent::ShowMainWindow);
    assert!(matches!(
        receive(&mut client.stream).await,
        Some(ServerMessage::Event(ServerEvent::ActivateMainWindow))
    ));

    client
        .success(UiIntent::SubmitQuickAdd(QuickAddSubmission {
            title: "Survives GUI exit".into(),
            placement: TaskPlacement::Inbox,
        }))
        .await;
    write_message_async(
        &mut client.stream,
        &ClientMessage::GuiClosing {
            session_id: client.session,
        },
    )
    .await
    .unwrap();
    // EOF acknowledges removal of this session before the next tray request.
    assert!(receive(&mut client.stream).await.is_none());
    drop(client);
    first.send(()).unwrap();
    (harness.desktop)(DesktopEvent::ShowMainWindow);
    let second = tokio::time::timeout(TIMEOUT, launches.recv())
        .await
        .unwrap()
        .unwrap();
    let mut client = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    assert_eq!(
        client.snapshot().await.quadrants.inbox[0].title,
        "Survives GUI exit"
    );
    let old_session = client.session;
    drop(client); // Abrupt EOF, without GuiClosing.
    second.send(()).unwrap();
    let mut recovered = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    assert_ne!(recovered.session, old_session);
    recovered.snapshot().await;
    assert!(launches.try_recv().is_err()); // Crash does not auto-respawn.
    (harness.desktop)(DesktopEvent::ExitRequested);
    assert!(matches!(
        receive(&mut recovered.stream).await,
        Some(ServerMessage::Event(ServerEvent::AgentShuttingDown))
    ));
    harness.finish().await;
}

#[tokio::test]
async fn login_uses_start_hidden_but_gui_bootstrap_never_spawns_a_second_gui() {
    for (hidden, login, should_launch) in [
        (true, true, false),
        (false, true, true),
        (false, false, false),
    ] {
        let profile = Profile::new();
        let store = quadrant_storage::SqliteStore::open(profile.database()).unwrap();
        store
            .save_desktop_settings(
                DesktopSettings {
                    start_hidden: hidden,
                    ..DesktopSettings::default()
                },
                UtcTimestamp::from_unix_seconds(NOW),
            )
            .unwrap();
        drop(store);
        let (sender, mut launches) = mpsc::unbounded_channel();
        let harness = Harness::start_with(profile, login, Arc::new(FakeGui(sender))).await;
        let mut client = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
        assert_eq!(
            client.snapshot().await.desktop_settings.start_hidden,
            hidden
        );
        if should_launch {
            launches.try_recv().unwrap().send(()).unwrap();
        } else {
            assert!(launches.try_recv().is_err());
        }
        drop(client);
        harness.finish().await;
    }
}

struct Profile(PathBuf);
impl Profile {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("quadrant-agent-test-{}", TaskId::generate()));
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

struct TestClock(AtomicI64);
impl Clock for TestClock {
    fn now(&self) -> UtcTimestamp {
        UtcTimestamp::from_unix_seconds(self.0.load(Ordering::SeqCst))
    }
}
struct NoGui;
impl quadrant_platform::GuiLauncher for NoGui {
    fn launch_main(&self) -> std::io::Result<quadrant_platform::GuiProcess> {
        panic!("unexpected native GUI launch in isolated service test")
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
        Ok(())
    }
    fn open_url(&self, _url: &str) -> Result<(), PlatformActionError> {
        Ok(())
    }
}

struct Harness {
    desktop: quadrant_platform::DesktopEventSink,
    profile: Profile,
    endpoint: AgentEndpoint,
    worker: JoinHandle<Result<(), quadrant_agent::AgentError>>,
    stop: oneshot::Sender<()>,
    clock: Arc<TestClock>,
    delivered: mpsc::UnboundedReceiver<ReminderAlert>,
    focus_notifications: Arc<AtomicUsize>,
}

impl Harness {
    async fn start(profile: Profile) -> Self {
        Self::start_with(profile, false, Arc::new(NoGui)).await
    }

    async fn start_with(
        profile: Profile,
        startup: bool,
        launcher: Arc<dyn quadrant_platform::GuiLauncher>,
    ) -> Self {
        let clock = Arc::new(TestClock(AtomicI64::new(NOW)));
        let (sender, delivered) = mpsc::unbounded_channel();
        let focus_notifications = Arc::new(AtomicUsize::new(0));
        let notifications = focus_notifications.clone();
        let host = HostServices {
            gui_launcher: launcher,
            clock: clock.clone(),
            autostart: Arc::new(TestHost),
            opener: Arc::new(TestHost),
            reminders: Arc::new(move |alert| {
                let _ = sender.send(alert);
                Ok(())
            }),
            focus_completed: Arc::new(move || {
                notifications.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
            desktop_integration: false,
        };
        let path = profile.database();
        let endpoint = AgentEndpoint::for_database(&path).unwrap();
        let agent = tokio::task::spawn_blocking(move || Agent::open(&path, host))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let (stop, stopped) = oneshot::channel();
        let desktop = agent.desktop_event_sink();
        let worker = tokio::spawn(agent.run_with_startup(stopped, startup));
        Self {
            desktop,
            profile,
            endpoint,
            worker,
            stop,
            clock,
            delivered,
            focus_notifications,
        }
    }

    async fn finish(self) {
        let _ = self.stop.send(());
        tokio::time::timeout(TIMEOUT, self.worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(self.profile.0.join("logs/quadrant-agent.log").exists());
    }
}

async fn receive(stream: &mut AgentStream) -> Option<ServerMessage> {
    tokio::time::timeout(TIMEOUT, read_message_async(stream))
        .await
        .expect("IPC response deadline")
        .expect("valid frame")
}

async fn hello(
    endpoint: &AgentEndpoint,
    mode: GuiLaunchMode,
    version: u32,
) -> (AgentStream, ServerHello) {
    let mut stream = endpoint.connect().await.unwrap();
    write_message_async(
        &mut stream,
        &ClientMessage::Hello(ClientHello {
            protocol_version: version,
            app_version: "different-package-version".to_owned(),
            process_id: std::process::id(),
            mode,
        }),
    )
    .await
    .unwrap();
    let Some(ServerMessage::HelloAck(ack)) = receive(&mut stream).await else {
        panic!("HelloAck required");
    };
    (stream, ack)
}

struct Client {
    stream: AgentStream,
    session: SessionId,
    sequence: u64,
}
impl Client {
    async fn connect(endpoint: &AgentEndpoint, mode: GuiLaunchMode) -> Self {
        // A just-closed connection may still have its ordered EOF notification queued.
        for _ in 0..50 {
            let (stream, ack) = hello(endpoint, mode, PROTOCOL_VERSION).await;
            if ack.disposition == GuiDisposition::Accepted {
                return Self {
                    stream,
                    session: ack.session_id.unwrap(),
                    sequence: 0,
                };
            }
            assert_eq!(ack.disposition, GuiDisposition::ActivateExistingAndExit);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("previous GUI session was not cleaned up");
    }
    fn next(&mut self) -> RequestId {
        self.sequence += 1;
        RequestId::new(self.sequence).unwrap()
    }
    async fn snapshot(&mut self) -> AppSnapshot {
        let request_id = self.next();
        write_message_async(
            &mut self.stream,
            &ClientMessage::GetInitialSnapshot { request_id },
        )
        .await
        .unwrap();
        loop {
            match receive(&mut self.stream).await {
                Some(ServerMessage::InitialSnapshot {
                    request_id: actual,
                    snapshot,
                }) => {
                    assert_eq!(actual, request_id);
                    return *snapshot;
                }
                Some(ServerMessage::Event(_)) => {}
                other => panic!("expected snapshot, got {other:?}"),
            }
        }
    }
    async fn command(
        &mut self,
        command: impl Into<GuiCommand>,
    ) -> (CommandOutcome, Vec<ServerEvent>) {
        let request_id = self.next();
        write_message_async(
            &mut self.stream,
            &ClientMessage::Command {
                request_id,
                command: command.into(),
            },
        )
        .await
        .unwrap();
        let mut events = Vec::new();
        loop {
            match receive(&mut self.stream).await {
                Some(ServerMessage::CommandResult {
                    request_id: actual,
                    outcome,
                }) => {
                    assert_eq!(actual, request_id);
                    return (outcome, events);
                }
                Some(ServerMessage::Event(event)) => events.push(event),
                other => panic!("expected result, got {other:?}"),
            }
        }
    }
    async fn success(&mut self, intent: UiIntent) -> Vec<ServerEvent> {
        let (outcome, events) = self.command(intent).await;
        assert_eq!(outcome, CommandOutcome::Succeeded);
        events
    }
}

#[tokio::test]
async fn task_commands_snapshot_backup_and_shutdown_work_without_slint() {
    let harness = Harness::start(Profile::new()).await;
    let mut client = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    assert!(client.snapshot().await.quadrants.inbox.is_empty());
    client
        .success(UiIntent::SubmitQuickAdd(QuickAddSubmission {
            title: "Agent capture 📝".to_owned(),
            placement: TaskPlacement::Inbox,
        }))
        .await;
    let state = client.snapshot().await;
    let id = state.quadrants.inbox[0].id;
    let (outcome, _) = client
        .command(UiIntent::SubmitQuickAdd(QuickAddSubmission {
            title: " ".to_owned(),
            placement: TaskPlacement::Inbox,
        }))
        .await;
    assert!(matches!(outcome, CommandOutcome::Failed(_)));
    client
        .success(UiIntent::MoveTask {
            task_id: id,
            placement: TaskPlacement::Quadrant(Quadrant::Q2),
        })
        .await;
    client.success(UiIntent::CompleteTask(id)).await;
    let state = client.snapshot().await;
    assert!(state.quadrants.q2.is_empty());
    assert_eq!(state.completed.tasks.len(), 1);
    assert_eq!(state.review.current.completed_tasks, 1);
    client.success(UiIntent::SetTheme(ThemeMode::Dark)).await;
    assert_eq!(client.snapshot().await.theme_mode, ThemeMode::Dark);
    client.success(UiIntent::CreateBackup).await;
    assert!(client.snapshot().await.maintenance.latest_backup.is_some());
    client.success(UiIntent::ReopenTask(id)).await;
    client.success(UiIntent::DeleteTask(id)).await;
    assert!(client.snapshot().await.completed.tasks.is_empty());
    let (outcome, _) = client.command(GuiCommand::ExitApplication).await;
    assert_eq!(outcome, CommandOutcome::Succeeded);
    let mut shutting_down = false;
    let mut exit_gui = false;
    while let Some(message) = receive(&mut client.stream).await {
        shutting_down |= message == ServerMessage::Event(ServerEvent::AgentShuttingDown);
        exit_gui |= message == ServerMessage::Event(ServerEvent::ExitGui);
    }
    assert!(shutting_down && exit_gui);
    harness.finish().await;
}

#[tokio::test]
async fn handshake_rejects_incompatible_and_activates_existing_gui() {
    let harness = Harness::start(Profile::new()).await;
    let mut main = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    main.snapshot().await;
    let (mut rejected, ack) =
        hello(&harness.endpoint, GuiLaunchMode::Main, PROTOCOL_VERSION + 1).await;
    assert_eq!(ack.disposition, GuiDisposition::RejectIncompatibleVersion);
    assert_eq!(ack.session_id, None);
    assert_eq!(receive(&mut rejected).await, None);
    for (mode, expected) in [
        (GuiLaunchMode::Main, ServerEvent::ActivateMainWindow),
        (GuiLaunchMode::QuickAdd, ServerEvent::OpenQuickAdd),
    ] {
        let (mut redirected, ack) = hello(&harness.endpoint, mode, PROTOCOL_VERSION).await;
        assert_eq!(ack.disposition, GuiDisposition::ActivateExistingAndExit);
        assert_eq!(ack.session_id, None);
        assert_eq!(
            receive(&mut main.stream).await,
            Some(ServerMessage::Event(expected))
        );
        assert_eq!(receive(&mut redirected).await, None);
    }
    harness.finish().await;
}

#[tokio::test]
async fn disconnect_and_gui_closing_preserve_focus_and_allow_fresh_sessions() {
    let harness = Harness::start(Profile::new()).await;
    let mut first = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    first.snapshot().await;
    first
        .success(UiIntent::StartFocus(FocusStartRequest {
            mode: FocusMode::Stopwatch,
            pomodoro_kind: None,
            task_id: None,
        }))
        .await;
    let before = first.snapshot().await.focus.session.unwrap().record().id;
    drop(first); // Crash-style EOF; no GuiClosing is sent.
    harness.clock.0.store(NOW + 125, Ordering::SeqCst);
    let mut second = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    let snapshot = second.snapshot().await;
    let focus = snapshot.focus.session.unwrap();
    assert_eq!(focus.record().id, before);
    assert_eq!(focus.elapsed_seconds_at(snapshot.captured_at), 125);
    second.success(UiIntent::PauseFocus).await;
    write_message_async(
        &mut second.stream,
        &ClientMessage::GuiClosing {
            session_id: second.session,
        },
    )
    .await
    .unwrap();
    assert_eq!(receive(&mut second.stream).await, None);
    let mut third = Client::connect(&harness.endpoint, GuiLaunchMode::QuickAdd).await;
    assert_eq!(
        third
            .snapshot()
            .await
            .focus
            .session
            .unwrap()
            .record()
            .status,
        FocusStatus::Paused
    );
    harness.finish().await;
}

#[tokio::test]
async fn duplicate_request_cannot_execute_a_mutation_twice() {
    let harness = Harness::start(Profile::new()).await;
    let mut client = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    client.snapshot().await;
    let intent = UiIntent::SubmitQuickAdd(QuickAddSubmission {
        title: "Exactly one".to_owned(),
        placement: TaskPlacement::Inbox,
    });
    client.success(intent.clone()).await;
    write_message_async(
        &mut client.stream,
        &ClientMessage::Command {
            request_id: RequestId::new(client.sequence).unwrap(),
            command: intent.into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        receive(&mut client.stream).await,
        Some(ServerMessage::ProtocolError(
            ProtocolError::DuplicateRequest
        ))
    );
    assert_eq!(receive(&mut client.stream).await, None);
    let mut fresh = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    assert_eq!(fresh.snapshot().await.quadrants.inbox.len(), 1);
    harness.finish().await;
}

#[tokio::test]
async fn handshake_required_pid_validation_and_bad_frames_are_connection_local() {
    use tokio::io::AsyncWriteExt;
    let harness = Harness::start(Profile::new()).await;
    let mut early = harness.endpoint.connect().await.unwrap();
    write_message_async(
        &mut early,
        &ClientMessage::GetInitialSnapshot {
            request_id: RequestId::new(1).unwrap(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        receive(&mut early).await,
        Some(ServerMessage::ProtocolError(
            ProtocolError::HandshakeRequired
        ))
    );
    let mut forged = harness.endpoint.connect().await.unwrap();
    write_message_async(
        &mut forged,
        &ClientMessage::Hello(ClientHello {
            protocol_version: PROTOCOL_VERSION,
            app_version: "test".to_owned(),
            process_id: 0,
            mode: GuiLaunchMode::Main,
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        receive(&mut forged).await,
        Some(ServerMessage::ProtocolError(ProtocolError::InvalidSession))
    );
    let mut malformed = harness.endpoint.connect().await.unwrap();
    malformed.write_all(&0_u32.to_be_bytes()).await.unwrap();
    assert_eq!(receive(&mut malformed).await, None);
    let mut stalled = harness.endpoint.connect().await.unwrap();
    stalled.write_all(&[0, 0]).await.unwrap();
    let mut normal = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    assert!(normal.snapshot().await.quadrants.inbox.is_empty());
    // Exit is bounded even when another peer never finishes its first frame.
    harness.finish().await;
}

#[tokio::test]
async fn reminders_and_pomodoro_complete_before_any_gui_connects() {
    let profile = Profile::new();
    let store = quadrant_storage::SqliteStore::open(profile.database()).unwrap();
    let now = UtcTimestamp::from_unix_seconds(NOW);
    let mut draft = NewTask::quick_capture("Offline reminder", TaskPlacement::Inbox).unwrap();
    draft.reminder = Some(ScheduledInstant {
        at_utc: now,
        time_zone: TimeZoneId::new("UTC").unwrap(),
    });
    let task_id = TaskId::generate();
    store.create_task(task_id, draft, now).unwrap();
    let session = FocusSession::start(
        FocusSessionId::from_uuid(TaskId::generate().as_uuid()),
        None,
        FocusMode::Pomodoro,
        Some(PomodoroKind::Focus),
        PomodoroSettings {
            focus_minutes: 1,
            ..PomodoroSettings::default()
        },
        UtcTimestamp::from_unix_seconds(NOW - 60),
        LocalDate::parse_iso("2026-09-05").unwrap(),
    )
    .unwrap();
    store.create_focus_session(session).unwrap();
    drop(store); // No database access outside Agent after this point.
    let mut harness = Harness::start(profile).await;
    let alert = tokio::time::timeout(TIMEOUT, harness.delivered.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(alert.task_id, task_id);
    tokio::time::timeout(TIMEOUT, async {
        while harness.focus_notifications.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut client = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    let snapshot = client.snapshot().await;
    assert!(snapshot.focus.session.is_none());
    let events = client.success(UiIntent::OpenTaskEditor(task_id)).await;
    assert!(events.iter().any(|event| matches!(event, ServerEvent::Application(event) if matches!(&**event, ApplicationEvent::TaskEditorLoaded(editor) if editor.reminder_at.is_empty()))));
    assert_eq!(harness.focus_notifications.load(Ordering::SeqCst), 1);
    harness.finish().await;
}

#[tokio::test]
async fn same_profile_lock_rejects_second_owner_and_is_released_after_shutdown() {
    let harness = Harness::start(Profile::new()).await;
    let path = harness.profile.database();
    let second = tokio::task::spawn_blocking(move || Agent::open(&path, HostServices::native()))
        .await
        .unwrap()
        .unwrap();
    assert!(second.is_none()); // Native ports are never called by a secondary.
    let guard =
        quadrant_platform::SingleInstanceCoordinator::claim(&harness.profile.database()).unwrap();
    assert!(!guard.is_primary()); // Same guard is used by the transitional old GUI.
    drop(guard);
    let endpoint = harness.endpoint.clone();
    harness.finish().await;
    assert!(endpoint.connect().await.is_err());
}

#[tokio::test]
async fn unread_client_cannot_hold_other_connections_or_agent_shutdown_open() {
    let harness = Harness::start(Profile::new()).await;
    let mut unread = Client::connect(&harness.endpoint, GuiLaunchMode::QuickAdd).await;
    let request_id = unread.next();
    write_message_async(
        &mut unread.stream,
        &ClientMessage::GetInitialSnapshot { request_id },
    )
    .await
    .unwrap();
    // Leave the snapshot in the pipe. Its close must not enter a shared,
    // indefinitely blocking flush queue and prevent the Main client's EOF.
    let mut main = Client::connect(&harness.endpoint, GuiLaunchMode::Main).await;
    main.snapshot().await;
    write_message_async(
        &mut main.stream,
        &ClientMessage::GuiClosing {
            session_id: main.session,
        },
    )
    .await
    .unwrap();
    assert!(receive(&mut main.stream).await.is_none());
    harness.finish().await;
    drop(unread);
}

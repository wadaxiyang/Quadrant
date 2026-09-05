// SPDX-License-Identifier: GPL-3.0-only
//! Wire compatibility, framing boundaries, and shared-value validation.

use std::{
    fmt::Debug,
    io::{self, Cursor, Read},
};

use quadrant_protocol::{
    AppSnapshot, ClientHello, ClientMessage, CommandOutcome, GuiCommand, GuiDisposition,
    GuiLaunchMode, PROTOCOL_VERSION, ProtocolError, RequestId, ServerEvent, ServerHello,
    ServerMessage, SessionId, application::*, codec::*,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
use uuid::Uuid;

fn request_id() -> RequestId {
    RequestId::new(1).expect("nonzero fixture sequence")
}

fn session_id() -> SessionId {
    SessionId::from_uuid(Uuid::from_u128(42))
}

fn hello(mode: GuiLaunchMode) -> ClientHello {
    ClientHello {
        protocol_version: PROTOCOL_VERSION,
        app_version: "0.1.0".to_owned(),
        process_id: 1234,
        mode,
    }
}

fn snapshot() -> AppSnapshot {
    serde_json::from_str(include_str!("fixtures/snapshot_v1.json")).expect("version 1 fixture")
}

fn round_trip<T: Serialize + DeserializeOwned + Debug + PartialEq>(value: &T) {
    let mut bytes = Vec::new();
    write_message(&mut bytes, value).expect("encode frame");
    let mut stream = Cursor::new(bytes);
    assert_eq!(
        read_message::<_, T>(&mut stream)
            .expect("decode frame")
            .as_ref(),
        Some(value)
    );
    assert_eq!(read_message::<_, T>(&mut stream).expect("clean EOF"), None);
}

#[test]
fn hello_wire_shape_is_pinned_independently_of_package_version() {
    let message = ClientMessage::Hello(hello(GuiLaunchMode::Main));
    assert_eq!(
        String::from_utf8(encode_payload(&message).expect("JSON")).expect("UTF-8"),
        r#"{"type":"hello","payload":{"protocol_version":1,"app_version":"0.1.0","process_id":1234,"mode":"main"}}"#,
    );
    round_trip(&message);
}

#[test]
fn handshake_prioritizes_wire_version_over_app_version_and_existing_session() {
    for mode in [GuiLaunchMode::Main, GuiLaunchMode::QuickAdd] {
        let mut client = hello(mode);
        let accepted = ServerHello::negotiate(&client, "99.0.0", false, session_id());
        assert_eq!(accepted.disposition, GuiDisposition::Accepted);
        assert_eq!(accepted.session_id, Some(session_id()));
        round_trip(&ServerMessage::HelloAck(accepted));
        let redirected = ServerHello::negotiate(&client, "0.1.0", true, session_id());
        assert_eq!(
            redirected.disposition,
            GuiDisposition::ActivateExistingAndExit
        );
        assert_eq!(redirected.session_id, None);
        round_trip(&ServerMessage::HelloAck(redirected));
        for version in [0, PROTOCOL_VERSION + 1, u32::MAX] {
            client.protocol_version = version;
            for existing in [false, true] {
                let rejected = ServerHello::negotiate(&client, "0.1.0", existing, session_id());
                assert_eq!(
                    rejected.disposition,
                    GuiDisposition::RejectIncompatibleVersion
                );
                assert_eq!(rejected.session_id, None);
                round_trip(&ServerMessage::HelloAck(rejected));
            }
        }
    }
}

#[test]
fn snapshot_fixture_preserves_all_state_and_live_focus_time_anchors() {
    let snapshot = snapshot();
    assert_eq!(
        snapshot.desktop_settings.close_behavior,
        WindowCloseBehavior::CloseGuiKeepAgent
    );
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/snapshot_v1.json")).unwrap();
    assert_eq!(serde_json::to_value(&snapshot).unwrap(), expected);
    let session = snapshot.focus.session.as_ref().expect("running Focus");
    assert_eq!(session.elapsed_seconds_at(snapshot.captured_at), 60);
    assert_eq!(
        session.remaining_seconds_at(snapshot.captured_at),
        Some(1440)
    );
    round_trip(&ServerMessage::InitialSnapshot {
        request_id: request_id(),
        snapshot: Box::new(snapshot),
    });
}

fn editor() -> TaskEditorSubmission {
    TaskEditorSubmission {
        task_id: TaskId::from_uuid(Uuid::from_u128(1)),
        title: "测试 \"task\"\n📝".to_owned(),
        notes: "First line\nSecond line".to_owned(),
        placement: TaskPlacement::Quadrant(Quadrant::Q2),
        planned_on: "2026-09-05".to_owned(),
        due_at: "2026-09-05T10:00:00+08:00".to_owned(),
        due_time_zone: "Asia/Shanghai".to_owned(),
        reminder_at: "2026-09-05T09:00:00+08:00".to_owned(),
        reminder_time_zone: "Asia/Shanghai".to_owned(),
        recurrence: RecurrenceChoice::CustomDays,
        custom_interval_days: "14".to_owned(),
    }
}

#[test]
fn every_current_application_intent_can_cross_the_command_boundary() {
    let submission = editor();
    let id = submission.task_id;
    let commands = [
        UiIntent::Navigate(NavigationRoute::Today),
        UiIntent::OpenQuickAdd,
        UiIntent::SubmitQuickAdd(QuickAddSubmission {
            title: "捕获".to_owned(),
            placement: TaskPlacement::Inbox,
        }),
        UiIntent::SetTheme(ThemeMode::Dark),
        UiIntent::SetDesktopSettings(DesktopSettings::default()),
        UiIntent::StartFocus(FocusStartRequest {
            mode: FocusMode::Pomodoro,
            pomodoro_kind: Some(PomodoroKind::Focus),
            task_id: Some(id),
        }),
        UiIntent::PauseFocus,
        UiIntent::ResumeFocus,
        UiIntent::FinishFocus,
        UiIntent::CancelFocus,
        UiIntent::SetPomodoroSettings(PomodoroSettings::default()),
        UiIntent::SetReviewRange(ReviewRange::NinetyDays),
        UiIntent::LoadMoreCompleted,
        UiIntent::CreateBackup,
        UiIntent::StageLatestRestore,
        UiIntent::OpenBackupDirectory,
        UiIntent::OpenReleasePage,
        UiIntent::MoveTask {
            task_id: id,
            placement: TaskPlacement::Quadrant(Quadrant::Q4),
        },
        UiIntent::ReorderTask {
            task_id: id,
            direction: ReorderDirection::Down,
        },
        UiIntent::OpenTaskEditor(id),
        UiIntent::SubmitTaskEditor(submission.clone()),
        UiIntent::CompleteTask(id),
        UiIntent::ReopenTask(id),
        UiIntent::DeleteTask(id),
        UiIntent::UpdateTask {
            task_id: id,
            update: submission.into_update().expect("valid editor"),
        },
    ];
    for intent in commands {
        round_trip(&ClientMessage::Command {
            request_id: request_id(),
            command: intent.into(),
        });
    }
}

#[test]
fn every_current_application_event_can_cross_the_push_boundary() {
    let state = snapshot();
    let events = [
        ApplicationEvent::QuadrantsChanged(state.quadrants),
        ApplicationEvent::TodayChanged(state.today),
        ApplicationEvent::FocusChanged(state.focus),
        ApplicationEvent::ReviewChanged(state.review),
        ApplicationEvent::CompletedChanged(state.completed),
        ApplicationEvent::MaintenanceChanged(state.maintenance),
        ApplicationEvent::DesktopSettingsChanged(state.desktop_settings),
        ApplicationEvent::ReminderDue(ReminderAlert {
            task_id: editor().task_id,
            title: "提醒".to_owned(),
            scheduled_for: state.captured_at,
        }),
        ApplicationEvent::TaskEditorLoaded(TaskEditorState {
            task_id: editor().task_id,
            title: "Test".to_owned(),
            notes: String::new(),
            placement: TaskPlacement::Inbox,
            planned_on: String::new(),
            due_at: String::new(),
            due_time_zone: String::new(),
            reminder_at: String::new(),
            reminder_time_zone: String::new(),
            recurrence: RecurrenceChoice::None,
            custom_interval_days: String::new(),
        }),
        ApplicationEvent::TaskEditorSaved,
        ApplicationEvent::TaskEditorValidationFailed {
            field: TaskEditorField::DueTimeZone,
            message: "Invalid timezone".to_owned(),
        },
        ApplicationEvent::OperationSucceeded("Saved".to_owned()),
        ApplicationEvent::OperationFailed(UserFacingError {
            message: "Unable to save".to_owned(),
        }),
    ];
    for event in events {
        round_trip(&ServerMessage::Event(event.into()));
    }
}

#[test]
fn lifecycle_and_response_envelopes_preserve_correlation() {
    for message in [
        ClientMessage::Hello(hello(GuiLaunchMode::QuickAdd)),
        ClientMessage::GetInitialSnapshot {
            request_id: request_id(),
        },
        ClientMessage::Command {
            request_id: request_id(),
            command: GuiCommand::ExitApplication,
        },
        ClientMessage::GuiClosing {
            session_id: session_id(),
        },
    ] {
        round_trip(&message);
    }
    for event in [
        ServerEvent::ActivateMainWindow,
        ServerEvent::OpenQuickAdd,
        ServerEvent::ExitGui,
        ServerEvent::AgentShuttingDown,
        ServerEvent::ThemeChanged {
            theme_mode: ThemeMode::Light,
            system_theme: SystemTheme::Dark,
        },
        ServerEvent::PlatformCapabilitiesChanged(quadrant_protocol::PlatformCapabilities::default()),
    ] {
        round_trip(&ServerMessage::Event(event));
    }
    for outcome in [
        CommandOutcome::Succeeded,
        CommandOutcome::Failed(UserFacingError {
            message: "Rejected".to_owned(),
        }),
    ] {
        round_trip(&ServerMessage::CommandResult {
            request_id: request_id(),
            outcome,
        });
    }
    for error in [
        ProtocolError::HandshakeRequired,
        ProtocolError::DuplicateHello,
        ProtocolError::InvalidSession,
        ProtocolError::DuplicateRequest,
        ProtocolError::InvalidMessage,
    ] {
        round_trip(&ServerMessage::ProtocolError(error));
    }
    assert_eq!(request_id().get(), 1);
    assert_eq!(session_id().as_uuid(), Uuid::from_u128(42));
}

#[test]
fn decoding_does_not_bypass_validated_domain_constructors() {
    let update = editor().into_update().unwrap();
    let valid = serde_json::to_value(update).unwrap();
    for (field, bad_value) in [
        ("title", json!(" ")),
        ("title", json!("a".repeat(501))),
        ("planned_on", json!("2026-02-30")),
        ("due", json!({"at_utc":1,"time_zone":"bad zone"})),
        (
            "recurrence",
            json!({"version":99,"pattern":{"frequency":"daily"}}),
        ),
        (
            "recurrence",
            json!({"version":1,"pattern":{"frequency":"custom_days","interval_days":0}}),
        ),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = bad_value;
        assert!(
            serde_json::from_value::<TaskDetailsUpdate>(invalid).is_err(),
            "{field}"
        );
    }
    let mut invalid = serde_json::to_value(snapshot()).unwrap();
    invalid["focus"]["session"]["active_segment_started_at"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<AppSnapshot>(invalid).is_err());
    assert!(serde_json::from_str::<TaskId>(r#""invalid-uuid""#).is_err());
    assert!(serde_json::from_str::<RequestId>("0").is_err());
    assert_eq!(RequestId::new(0), None);
}

#[test]
fn raw_submissions_remain_subject_to_application_validation_after_decode() {
    let mut submission = editor();
    submission.title = " ".to_owned();
    let decoded: TaskEditorSubmission =
        decode_payload(&encode_payload(&submission).unwrap()).unwrap();
    assert!(decoded.into_update().is_err());
    let settings = PomodoroSettings {
        focus_minutes: 0,
        ..Default::default()
    };
    let decoded: PomodoroSettings = decode_payload(&encode_payload(&settings).unwrap()).unwrap();
    assert!(decoded.validate().is_err());
}

struct FragmentedReader {
    bytes: Cursor<Vec<u8>>,
    interrupt_once: bool,
}

impl Read for FragmentedReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if std::mem::take(&mut self.interrupt_once) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let length = output.len().min(1);
        self.bytes.read(&mut output[..length])
    }
}

#[test]
fn framing_handles_fragmentation_interruption_and_concatenated_messages() {
    let messages = [
        ClientMessage::Hello(hello(GuiLaunchMode::Main)),
        ClientMessage::GetInitialSnapshot {
            request_id: request_id(),
        },
    ];
    let mut bytes = Vec::new();
    for message in &messages {
        write_message(&mut bytes, message).unwrap();
    }
    let mut reader = FragmentedReader {
        bytes: Cursor::new(bytes),
        interrupt_once: true,
    };
    for expected in &messages {
        assert_eq!(
            read_message::<_, ClientMessage>(&mut reader)
                .unwrap()
                .as_ref(),
            Some(expected)
        );
    }
    assert_eq!(read_message::<_, ClientMessage>(&mut reader).unwrap(), None);
}

#[test]
fn every_partial_frame_is_an_error_and_only_empty_stream_is_clean_eof() {
    let message = ClientMessage::Hello(hello(GuiLaunchMode::Main));
    let mut bytes = Vec::new();
    write_message(&mut bytes, &message).unwrap();
    for length in 1..bytes.len() {
        assert!(
            matches!(read_message::<_, ClientMessage>(&mut &bytes[..length]),
            Err(CodecError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof)
        );
    }
    assert_eq!(
        read_message::<_, ClientMessage>(&mut &[][..]).unwrap(),
        None
    );
}

#[test]
fn length_limits_are_checked_before_reading_or_writing_payloads() {
    for length in [
        0_u32,
        u32::try_from(MAX_MESSAGE_BYTES + 1).unwrap(),
        u32::MAX,
    ] {
        assert!(matches!(
            read_message::<_, ClientMessage>(&mut &length.to_be_bytes()[..]),
            Err(CodecError::InvalidLength(_))
        ));
    }
    let mut output = Vec::new();
    assert!(matches!(
        write_message(&mut output, &"x".repeat(MAX_MESSAGE_BYTES)),
        Err(CodecError::InvalidLength(_))
    ));
    assert!(output.is_empty());
    // JSON quotes count toward the payload cap; the exact boundary is accepted.
    let maximum = "x".repeat(MAX_MESSAGE_BYTES - 2);
    let payload = encode_payload(&maximum).unwrap();
    assert_eq!(payload.len(), MAX_MESSAGE_BYTES);
    assert_eq!(decode_payload::<String>(&payload).unwrap(), maximum);
}

#[test]
fn malformed_unknown_and_trailing_payloads_are_rejected() {
    for payload in [
        &b"not json"[..], &b"\xff"[..], &b"{} {}"[..],
        &br#"{"type":"unknown"}"#[..],
        &br#"{"type":"get_initial_snapshot","payload":{"request_id":0}}"#[..],
        &br#"{"type":"command","payload":{"request_id":1,"command":{"type":"application","payload":"UnknownIntent"}}}"#[..],
    ] {
        assert!(matches!(decode_payload::<ClientMessage>(payload), Err(CodecError::Json(_))));
    }
    assert!(matches!(
        decode_payload::<ClientMessage>(&[]),
        Err(CodecError::InvalidLength(0))
    ));
}

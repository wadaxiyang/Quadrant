// SPDX-License-Identifier: GPL-3.0-only
//! Single owner of GUI sessions, request ordering, and projection publication.

use crate::{
    AgentError,
    log::AgentLog,
    services::{Services, is_failure},
    transport::Input,
};
use quadrant_application::{
    ApplicationEvent, DesktopEvent, FocusSchedulerHandle, ReminderSchedulerHandle, UiIntent,
    UserFacingError,
};
use quadrant_protocol::{
    ClientMessage, CommandOutcome, GuiCommand, GuiDisposition, GuiLaunchMode, PlatformCapabilities,
    ProtocolError, RequestId, ServerEvent, ServerHello, ServerMessage, SessionId,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, oneshot};

struct Connection {
    output: mpsc::Sender<ServerMessage>,
    peer: quadrant_platform::PeerIdentity,
    mode: Option<GuiLaunchMode>,
    snapshot_ready: bool,
    last_request: u64,
}

pub(crate) struct Broker {
    lifecycle: crate::lifecycle::Lifecycle,
    show_at_startup: bool,
    pending_quick_add: bool,
    launch_error_reported: bool,
    sessions: HashMap<SessionId, Connection>,
    services: Services,
    capabilities: PlatformCapabilities,
    reminders: ReminderSchedulerHandle,
    focus: FocusSchedulerHandle,
    log: Arc<AgentLog>,
    startup_notices: Vec<ApplicationEvent>,
    focus_notification: Option<
        Arc<dyn Fn() -> Result<(), quadrant_platform::PlatformIntegrationError> + Send + Sync>,
    >,
}

impl Broker {
    pub fn new(
        services: Services,
        capabilities: PlatformCapabilities,
        reminders: ReminderSchedulerHandle,
        focus: FocusSchedulerHandle,
        log: Arc<AgentLog>,
    ) -> Self {
        Self {
            lifecycle: crate::lifecycle::Lifecycle::new(Arc::new(
                quadrant_platform::PlatformGuiLauncher,
            )),
            show_at_startup: false,
            pending_quick_add: false,
            launch_error_reported: false,
            sessions: HashMap::new(),
            services,
            capabilities,
            reminders,
            focus,
            log,
            startup_notices: Vec::new(),
            focus_notification: None,
        }
    }

    pub fn with_startup_notices(mut self, notices: Vec<ApplicationEvent>) -> Self {
        self.startup_notices = notices;
        self
    }
    pub fn with_lifecycle(
        mut self,
        launcher: Arc<dyn quadrant_platform::GuiLauncher>,
        show_at_startup: bool,
    ) -> Self {
        self.lifecycle = crate::lifecycle::Lifecycle::new(launcher);
        self.show_at_startup = show_at_startup;
        self
    }
    pub fn with_focus_notification(
        mut self,
        notification: Arc<
            dyn Fn() -> Result<(), quadrant_platform::PlatformIntegrationError> + Send + Sync,
        >,
    ) -> Self {
        self.focus_notification = Some(notification);
        self
    }

    pub async fn run(
        mut self,
        mut input: mpsc::Receiver<Input>,
        mut background: mpsc::UnboundedReceiver<ApplicationEvent>,
        mut desktop: mpsc::UnboundedReceiver<DesktopEvent>,
        mut shutdown: oneshot::Receiver<()>,
    ) -> Result<(), AgentError> {
        if self.show_at_startup {
            self.activate(GuiLaunchMode::Main).await;
        }
        let result = loop {
            tokio::select! {
                biased;
                _ = &mut shutdown => break Ok(()),
                event = desktop.recv() => match event {
                    Some(DesktopEvent::ExitRequested) | None => break Ok(()),
                    Some(DesktopEvent::ShowMainWindow) => self.activate(GuiLaunchMode::Main).await,
                    Some(DesktopEvent::OpenQuickAdd) => self.activate(GuiLaunchMode::QuickAdd).await,
                },
                change = self.lifecycle.changed() => {
                    self.log.event(change.event);
                    if change.startup_failed && self.existing(GuiLaunchMode::Main).is_none() {
                        self.launch_failed().await;
                    }
                },
                event = background.recv() => if let Some(event) = event { self.background(event).await; },
                request = input.recv() => match request {
                    Some(Input::ListenerFailed) | None => break Err(AgentError::Listener),
                    Some(request) => if self.input(request).await { break Ok(()); },
                }
            }
        };
        self.log.event("agent_shutdown");
        // Queue before dropping senders. Transport drains them with a bounded
        // deadline while cancellation of readers closes any retained UI sender.
        let ids: Vec<_> = self.sessions.keys().copied().collect();
        for id in ids {
            self.send(id, ServerMessage::Event(ServerEvent::AgentShuttingDown));
            self.send(id, ServerMessage::Event(ServerEvent::ExitGui));
        }
        self.sessions.clear();
        self.lifecycle.shutdown().await;
        result
    }

    async fn input(&mut self, input: Input) -> bool {
        match input {
            Input::Connected { id, peer, outgoing } => {
                self.sessions.insert(
                    id,
                    Connection {
                        output: outgoing,
                        peer,
                        mode: None,
                        snapshot_ready: false,
                        last_request: 0,
                    },
                );
                self.log.event("gui_connected");
            }
            Input::Disconnected(id) => {
                self.sessions.remove(&id);
                self.log.event("gui_disconnected");
            }
            Input::Message { id, message } => return self.message(id, message).await,
            Input::ListenerFailed => {}
        }
        false
    }

    async fn message(&mut self, id: SessionId, message: ClientMessage) -> bool {
        let Some(connection) = self.sessions.get(&id) else {
            return false;
        };
        if let ClientMessage::Hello(hello) = message {
            self.hello(id, &hello);
            return false;
        }
        if connection.mode.is_none() {
            self.reject(id, ProtocolError::HandshakeRequired);
            return false;
        }
        match message {
            ClientMessage::GetInitialSnapshot { request_id } => {
                if !self.register_request(id, request_id) {
                    return false;
                }
                let services = self.services.clone();
                let capabilities = self.capabilities;
                match tokio::task::spawn_blocking(move || services.snapshot(capabilities)).await {
                    Ok(Ok(snapshot)) => {
                        self.send(
                            id,
                            ServerMessage::InitialSnapshot {
                                request_id,
                                snapshot: Box::new(snapshot),
                            },
                        );
                        if let Some(session) = self.sessions.get_mut(&id) {
                            session.snapshot_ready = true;
                        }
                        self.lifecycle.connected();
                        if std::mem::take(&mut self.pending_quick_add) {
                            self.send_activation(id, GuiLaunchMode::QuickAdd);
                        }
                        for notice in self.startup_notices.clone() {
                            self.send(id, ServerMessage::Event(notice.into()));
                        }
                    }
                    _ => self.failed(
                        id,
                        request_id,
                        "Initial application state could not be loaded.",
                    ),
                }
            }
            ClientMessage::Command {
                request_id,
                command,
            } => {
                if !self.register_request(id, request_id) {
                    return false;
                }
                match command {
                    GuiCommand::ExitApplication => {
                        self.send(
                            id,
                            ServerMessage::CommandResult {
                                request_id,
                                outcome: CommandOutcome::Succeeded,
                            },
                        );
                        return true;
                    }
                    GuiCommand::Application(intent) => self.command(id, request_id, *intent).await,
                }
            }
            ClientMessage::GuiClosing { session_id } => {
                if session_id == id {
                    self.sessions.remove(&id);
                    self.log.event("gui_closing");
                } else {
                    self.reject(id, ProtocolError::InvalidSession);
                }
            }
            ClientMessage::Hello(_) => {} // Handled before all post-handshake requests.
        }
        false
    }

    fn hello(&mut self, id: SessionId, hello: &quadrant_protocol::ClientHello) {
        let Some(connection) = self.sessions.get(&id) else {
            return;
        };
        if connection.mode.is_some() {
            self.reject(id, ProtocolError::DuplicateHello);
            return;
        }
        if hello.process_id == 0
            || connection
                .peer
                .process_id
                .is_some_and(|pid| pid != hello.process_id)
        {
            self.reject(id, ProtocolError::InvalidSession);
            return;
        }
        let existing = self.existing(hello.mode);
        let ack = ServerHello::negotiate(hello, env!("CARGO_PKG_VERSION"), existing.is_some(), id);
        let disposition = ack.disposition;
        self.send(id, ServerMessage::HelloAck(ack));
        match disposition {
            GuiDisposition::Accepted => {
                if let Some(session) = self.sessions.get_mut(&id) {
                    session.mode = Some(hello.mode);
                }
                self.log.event("gui_accepted");
            }
            GuiDisposition::ActivateExistingAndExit => {
                if let Some(existing) = existing {
                    self.send_activation(existing, hello.mode);
                }
                self.sessions.remove(&id);
                self.log.event("gui_redirected");
            }
            GuiDisposition::RejectIncompatibleVersion => {
                self.sessions.remove(&id);
                self.log.event("protocol_mismatch");
            }
        }
    }

    async fn command(&mut self, id: SessionId, request_id: RequestId, intent: UiIntent) {
        if matches!(intent, UiIntent::OpenQuickAdd) {
            self.send(id, ServerMessage::Event(ServerEvent::OpenQuickAdd));
            self.send(
                id,
                ServerMessage::CommandResult {
                    request_id,
                    outcome: CommandOutcome::Succeeded,
                },
            );
            return;
        }
        let reminders = intent.affects_reminder_schedule();
        let focus = intent.affects_focus_schedule();
        let services = self.services.clone();
        match tokio::task::spawn_blocking(move || services.command(&intent)).await {
            Ok(Ok(events)) => {
                let outcome = events
                    .iter()
                    .find_map(|event| match event {
                        ServerEvent::Application(event) => match &**event {
                            ApplicationEvent::OperationFailed(error) => {
                                Some(CommandOutcome::Failed(error.clone()))
                            }
                            ApplicationEvent::TaskEditorValidationFailed { message, .. } => {
                                Some(CommandOutcome::Failed(UserFacingError {
                                    message: message.clone(),
                                }))
                            }
                            _ => None,
                        },
                        _ => None,
                    })
                    .unwrap_or(CommandOutcome::Succeeded);
                for event in events {
                    self.publish(Some(id), event);
                }
                self.send(
                    id,
                    ServerMessage::CommandResult {
                        request_id,
                        outcome,
                    },
                );
                // A mutation may have committed even if its projection reload
                // failed. Recompute schedules for every relevant completed call.
                if reminders {
                    self.reminders.schedule_changed();
                }
                if focus {
                    self.focus.schedule_changed();
                }
            }
            _ => self.failed(
                id,
                request_id,
                "The background operation stopped unexpectedly.",
            ),
        }
    }

    async fn background(&mut self, event: ApplicationEvent) {
        if matches!(event, ApplicationEvent::FocusChanged(_)) {
            if let Some(notify) = self.focus_notification.clone()
                && !matches!(
                    tokio::task::spawn_blocking(move || notify()).await,
                    Ok(Ok(()))
                )
            {
                self.log.event("focus_notification_failed");
            }
            let services = self.services.clone();
            if let Ok(Ok(events)) =
                tokio::task::spawn_blocking(move || services.refresh_background_focus()).await
            {
                for event in events {
                    self.publish(None, event);
                }
            } else {
                self.log.event("focus_refresh_failed");
            }
        } else {
            if is_failure(&event) {
                self.log.event("background_operation_failed");
            }
            self.publish(None, event.into());
        }
    }

    fn publish(&mut self, origin: Option<SessionId>, event: ServerEvent) {
        let private = matches!(&event, ServerEvent::Application(event) if matches!(&**event,
            ApplicationEvent::TaskEditorLoaded(_) | ApplicationEvent::TaskEditorSaved |
            ApplicationEvent::TaskEditorValidationFailed { .. } | ApplicationEvent::OperationSucceeded(_) |
            ApplicationEvent::OperationFailed(_)));
        if private && let Some(id) = origin {
            self.send(id, ServerMessage::Event(event));
            return;
        }
        let ids: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, session)| session.snapshot_ready)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.send(id, ServerMessage::Event(event.clone()));
        }
    }

    fn existing(&self, mode: GuiLaunchMode) -> Option<SessionId> {
        self.sessions
            .iter()
            .find(|(_, s)| s.mode == Some(GuiLaunchMode::Main) && !s.output.is_closed())
            .or_else(|| {
                self.sessions
                    .iter()
                    .find(|(_, s)| s.mode == Some(mode) && !s.output.is_closed())
            })
            .map(|(id, _)| *id)
    }

    async fn activate(&mut self, mode: GuiLaunchMode) {
        if let Some(id) = self.existing(mode) {
            self.send_activation(id, mode);
        } else {
            self.pending_quick_add |= mode == GuiLaunchMode::QuickAdd;
            match self.lifecycle.launch().await {
                Ok(true) => self.log.event("gui_spawned"),
                Ok(false) => self.log.event("gui_launch_coalesced"),
                Err(_) => {
                    self.launch_failed().await;
                }
            }
        }
    }

    async fn launch_failed(&mut self) {
        self.log.event("gui_launch_failed");
        self.pending_quick_add = false;
        if !self.launch_error_reported {
            self.startup_notices.push(crate::services::failure("The previous interface launch failed. Check that both programs come from the same complete installation."));
            self.launch_error_reported = true;
        }
        if self.capabilities.native_notifications {
            let _ = tokio::task::spawn_blocking(
                quadrant_platform::PlatformNotificationDelivery::gui_launch_failed,
            )
            .await;
        }
    }

    fn send_activation(&mut self, id: SessionId, mode: GuiLaunchMode) {
        self.send(
            id,
            ServerMessage::Event(match mode {
                GuiLaunchMode::Main => ServerEvent::ActivateMainWindow,
                GuiLaunchMode::QuickAdd => ServerEvent::OpenQuickAdd,
            }),
        );
        self.log.event("gui_activated");
    }

    fn register_request(&mut self, id: SessionId, request: RequestId) -> bool {
        let Some(session) = self.sessions.get_mut(&id) else {
            return false;
        };
        if request.get() <= session.last_request {
            self.reject(id, ProtocolError::DuplicateRequest);
            return false;
        }
        session.last_request = request.get();
        true
    }

    fn send(&mut self, id: SessionId, message: ServerMessage) {
        if self
            .sessions
            .get(&id)
            .is_some_and(|session| session.output.try_send(message).is_err())
        {
            // Bound memory and drop a lagging/dead session; reconnect gets a new snapshot.
            self.sessions.remove(&id);
            self.log.event("gui_output_unavailable");
        }
    }

    fn reject(&mut self, id: SessionId, error: ProtocolError) {
        self.send(id, ServerMessage::ProtocolError(error));
        self.sessions.remove(&id);
        self.log.event("gui_protocol_rejected");
    }

    fn failed(&mut self, id: SessionId, request_id: RequestId, message: &str) {
        self.send(
            id,
            ServerMessage::CommandResult {
                request_id,
                outcome: CommandOutcome::Failed(UserFacingError {
                    message: message.to_owned(),
                }),
            },
        );
    }
}

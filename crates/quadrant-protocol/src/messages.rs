// SPDX-License-Identifier: GPL-3.0-only

use std::num::NonZeroU64;

use quadrant_application::{ApplicationEvent, SystemTheme, ThemeMode, UiIntent, UserFacingError};
use serde::{Deserialize, Serialize};

use crate::{AppSnapshot, ClientHello, PlatformCapabilities, ServerHello, SessionId};

/// Nonzero, strictly increasing request sequence within one accepted connection.
///
/// Reconnection starts a new sequence. This is correlation, not deduplication:
/// never replay a mutation automatically after losing its response.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(NonZeroU64);

impl RequestId {
    /// Returns an ID when `value` is nonzero.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the sequence number for request bookkeeping.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Typed requests processed by the Agent's application/lifecycle authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum GuiCommand {
    /// Execute an existing typed use case and publish its resulting events.
    ///
    /// Navigation is a projection-refresh hint; it does not transfer ownership
    /// of the GUI's selected route. `OpenQuickAdd` is a presentation request
    /// routed by the Agent to an appropriate GUI session.
    Application(Box<UiIntent>),
    /// Explicit full-application exit, distinct from closing one GUI connection.
    ExitApplication,
}

impl From<UiIntent> for GuiCommand {
    fn from(value: UiIntent) -> Self {
        Self::Application(Box::new(value))
    }
}

/// GUI-to-Agent stream messages.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Must precede all other messages, exactly once per connection.
    Hello(ClientHello),
    /// Load all authoritative first-screen state after handshake acceptance.
    GetInitialSnapshot {
        /// Correlation for the snapshot response.
        request_id: RequestId,
    },
    /// Submit work; receiving bytes is not evidence that a mutation succeeded.
    Command {
        /// Correlation for the final command result.
        request_id: RequestId,
        /// Typed use case or lifecycle action.
        command: GuiCommand,
    },
    /// Best-effort orderly disconnect notification; EOF must also clear session.
    GuiClosing {
        /// Must match the identity assigned to this connection.
        session_id: SessionId,
    },
}

/// Final result of a command after application validation and execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum CommandOutcome {
    /// The operation completed; state changes arrive through typed events.
    Succeeded,
    /// No success may be assumed; display this safe application failure.
    Failed(UserFacingError),
}

/// Unsolicited Agent-to-GUI updates, ordered on the same stream as responses.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ServerEvent {
    /// Projection, editor, settings, reminder, or operation-feedback event.
    Application(Box<ApplicationEvent>),
    /// Apply the authoritative user preference and normalized host theme.
    ThemeChanged {
        /// Persisted preference.
        theme_mode: ThemeMode,
        /// Host appearance.
        system_theme: SystemTheme,
    },
    /// Update the actual desktop capability state.
    PlatformCapabilitiesChanged(PlatformCapabilities),
    /// Bring the existing main window to the foreground.
    ActivateMainWindow,
    /// Open or activate the Quick Add surface.
    OpenQuickAdd,
    /// End this GUI's event loop and drop its shell through normal shutdown.
    ExitGui,
    /// The Agent is stopping; do not reconnect or restart it automatically.
    AgentShuttingDown,
}

impl From<ApplicationEvent> for ServerEvent {
    fn from(value: ApplicationEvent) -> Self {
        Self::Application(Box::new(value))
    }
}

/// Stable protocol violations; transport diagnostics stay out of UI messages.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolError {
    /// A request arrived before an accepted Hello.
    HandshakeRequired,
    /// Hello was sent more than once on the same connection.
    DuplicateHello,
    /// A supplied identity does not belong to this connection.
    InvalidSession,
    /// A request sequence was reused on this connection.
    DuplicateRequest,
    /// A malformed/unknown message cannot be interpreted at this wire version.
    InvalidMessage,
}

/// Agent-to-GUI stream messages.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Accept, redirect, or reject the initial Hello.
    HelloAck(ServerHello),
    /// Full initial state, before subsequent live projection events.
    InitialSnapshot {
        /// Corresponding snapshot request.
        request_id: RequestId,
        /// Boxed to keep routine message values small.
        snapshot: Box<AppSnapshot>,
    },
    /// Final result, emitted exactly once for each accepted command request.
    CommandResult {
        /// Corresponding command request.
        request_id: RequestId,
        /// Authoritative execution outcome.
        outcome: CommandOutcome,
    },
    /// Ordered push update; no database polling is needed in the GUI.
    Event(ServerEvent),
    /// Fatal connection-protocol violation; close the stream after delivery.
    ProtocolError(ProtocolError),
}

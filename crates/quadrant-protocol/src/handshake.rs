// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::PROTOCOL_VERSION;

/// Agent-assigned connection identity; never an authorization credential.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Generates an identity for a newly accepted GUI connection.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wraps an existing UUID, including deterministic test identities.
    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the UUID for platform diagnostics and session tracking.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// Presentation surface requested before any Slint component is constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiLaunchMode {
    /// Normal main-window session.
    Main,
    /// Dedicated Quick Add session with no main window.
    QuickAdd,
}

/// First message sent on a new local connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientHello {
    /// Client's wire contract version.
    pub protocol_version: u32,
    /// Client's application version, used only for diagnostics.
    pub app_version: String,
    /// Claimed GUI PID, to be checked by platform/session supervision.
    pub process_id: u32,
    /// Requested surface.
    pub mode: GuiLaunchMode,
}

/// Agent's decision about this connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiDisposition {
    /// Register the new connection as an active GUI session.
    Accepted,
    /// Route activation to an existing session and terminate the new GUI.
    ActivateExistingAndExit,
    /// Stop; these binaries do not share a supported wire contract.
    RejectIncompatibleVersion,
}

/// Handshake response. Rejected/redirected connections receive no session ID.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerHello {
    /// Agent's supported protocol version.
    pub protocol_version: u32,
    /// Agent application version, used only for diagnostics.
    pub agent_version: String,
    /// Present only for an accepted connection.
    pub session_id: Option<SessionId>,
    /// Whether this GUI may proceed to request state.
    pub disposition: GuiDisposition,
}

impl ServerHello {
    /// Builds a handshake decision, checking protocol compatibility first.
    ///
    /// The Agent supplies `existing_gui` from its live IPC-session registry and
    /// routes Main/Quick Add activation itself. This function performs no spawn,
    /// registration, activation, or OS authentication.
    #[must_use]
    pub fn negotiate(
        hello: &ClientHello,
        agent_version: impl Into<String>,
        existing_gui: bool,
        candidate_session_id: SessionId,
    ) -> Self {
        let disposition = if hello.protocol_version != PROTOCOL_VERSION {
            GuiDisposition::RejectIncompatibleVersion
        } else if existing_gui {
            GuiDisposition::ActivateExistingAndExit
        } else {
            GuiDisposition::Accepted
        };
        Self {
            protocol_version: PROTOCOL_VERSION,
            agent_version: agent_version.into(),
            session_id: (disposition == GuiDisposition::Accepted).then_some(candidate_session_id),
            disposition,
        }
    }
}

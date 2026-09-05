// SPDX-License-Identifier: GPL-3.0-only
//! Local client-to-presentation messages, not additional wire protocol variants.

use crate::{AppSnapshot, CommandOutcome, GuiCommand, ServerEvent};

/// Availability of authoritative application operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    /// Snapshot loaded and ready for a command.
    Ready,
    /// Waiting for the Agent's correlated command result.
    Busy,
    /// Connection lost; bounded recovery is underway.
    Reconnecting,
    /// Recovery exhausted or the peer's protocol is incompatible.
    Unavailable,
}

/// Ordered messages delivered on the GUI's event loop by its transport adapter.
#[derive(Clone, Debug)]
pub enum ClientUpdate {
    /// Replace authoritative projections after a fresh accepted session.
    Snapshot(Box<AppSnapshot>),
    /// Apply one Agent event.
    Event(ServerEvent),
    /// Resolve a submitted command using its actual Agent response.
    CommandFinished {
        /// Original command, used to resolve the corresponding presentation.
        command: GuiCommand,
        /// Confirmed result; connection loss is never converted to success/failure.
        outcome: CommandOutcome,
    },
    /// Set connection availability and a user-facing explanation.
    Connection {
        /// Current transport availability.
        state: ConnectionState,
        /// Empty when ready, otherwise a safe status/error message.
        message: String,
    },
}

// SPDX-License-Identifier: GPL-3.0-only
//! Shared, versioned local Agent/GUI contract.
//!
//! The first client message must be [`ClientMessage::Hello`]. Only an accepted
//! handshake permits requests. Application versions are diagnostic metadata;
//! compatibility is determined exclusively by [`PROTOCOL_VERSION`].
//!
//! Messages use length-prefixed UTF-8 JSON (see [`codec`]). No endpoint, runtime,
//! database, or UI is created here. Phase 2 supplies `interprocess` local streams
//! and current-user access control through the platform boundary. A claimed PID
//! or session ID is not authentication.
//!
//! Shared application DTOs are part of this wire version. Incompatible changes
//! to their serialized shape require a protocol-version increment. Business
//! validation still belongs to application services after decoding.

#![forbid(unsafe_code)]

mod client_update;
pub mod codec;
mod handshake;
mod messages;
mod snapshot;
pub use client_update::{ClientUpdate, ConnectionState};

pub use handshake::{ClientHello, GuiDisposition, GuiLaunchMode, ServerHello, SessionId};
pub use messages::{
    ClientMessage, CommandOutcome, GuiCommand, ProtocolError, RequestId, ServerEvent, ServerMessage,
};
pub use snapshot::{AppSnapshot, PlatformCapabilities};

/// Current wire contract version, independent of Cargo's application version.
pub const PROTOCOL_VERSION: u32 = 1;

/// Application projections/value types shared by both protocol consumers.
pub use quadrant_application as application;

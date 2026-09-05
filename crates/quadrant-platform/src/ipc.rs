// SPDX-License-Identifier: GPL-3.0-only
//! Current-user-only local Agent endpoint. No protocol or business types.

#[cfg(unix)]
mod unix;
#[cfg(target_os = "windows")]
mod windows;

use interprocess::local_socket::{
    ConnectOptions, Name,
    tokio::{Listener, Stream, prelude::*},
};
use std::{
    io,
    path::{Path, PathBuf},
};

/// Portable async duplex local stream; OS details stay in this module.
pub struct AgentStream(Stream);

impl AgentStream {
    /// Drains final server messages within a bounded deadline, then closes.
    /// Client endpoints close immediately; EOF also releases the GUI session.
    pub async fn close(self) {
        #[cfg(target_os = "windows")]
        windows::drain(&self.0).await;
        drop(self);
    }
}

impl Drop for AgentStream {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        windows::disconnect(&self.0);
    }
}

impl tokio::io::AsyncRead for AgentStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for AgentStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        std::pin::Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
    }
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}

/// Verified transport identity, independent of the Hello payload.
#[derive(Clone, Copy, Debug)]
pub struct PeerIdentity {
    /// Kernel-reported PID when available (some Unix systems supply only UID).
    pub process_id: Option<u32>,
}

/// Profile-specific endpoint resolved from the canonical data directory.
#[derive(Clone, Debug)]
pub struct AgentEndpoint {
    database_path: PathBuf,
}

impl AgentEndpoint {
    /// Resolves a stable profile without relying on the current working directory.
    ///
    /// # Errors
    /// Returns an error for a missing data directory or invalid database filename.
    pub fn for_database(database_path: &Path) -> io::Result<Self> {
        Ok(Self {
            database_path: canonical_database_path(database_path)?,
        })
    }

    /// Connects on the caller's existing Tokio runtime.
    ///
    /// # Errors
    /// Returns endpoint/permission/connection failures without any TCP fallback.
    pub async fn connect(&self) -> io::Result<AgentStream> {
        let stream = ConnectOptions::new()
            .name(self.name()?)
            .connect_tokio()
            .await?;
        verify_peer(&stream)?;
        Ok(AgentStream(stream))
    }

    pub(crate) fn bind(&self) -> io::Result<AgentListener> {
        Ok(AgentListener { inner: bind(self)? })
    }

    pub(crate) fn profile_identity(&self) -> u64 {
        crate::instance::instance_identity(&self.database_path)
    }

    fn name(&self) -> io::Result<Name<'static>> {
        endpoint_name(&self.database_path)
    }
}

/// Bound listener created only after the profile ownership guard is acquired.
pub struct AgentListener {
    inner: Listener,
}

impl AgentListener {
    /// Accepts and authenticates one local peer.
    ///
    /// # Errors
    /// Returns transport errors or rejects peers outside the current user.
    pub async fn accept(&self) -> io::Result<(AgentStream, PeerIdentity)> {
        let stream = self.inner.accept().await?;
        // A peer may exit before its credentials can be queried. Authentication
        // failures reject that connection without terminating the listener.
        let identity = verify_peer(&stream)
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
        Ok((AgentStream(stream), identity))
    }
}

pub(crate) fn canonical_database_path(path: &Path) -> io::Result<PathBuf> {
    let file = path
        .file_name()
        .ok_or_else(|| io::Error::other("missing database filename"))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(parent.canonicalize()?.join(file))
}

#[cfg(unix)]
use unix::{bind, endpoint_name, verify_peer};
#[cfg(target_os = "windows")]
use windows::{bind, endpoint_name, verify_peer};

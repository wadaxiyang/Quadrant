// SPDX-License-Identifier: GPL-3.0-only
//! Private directory and socket permissions, with kernel peer-UID verification.

use super::{AgentEndpoint, PeerIdentity};
use interprocess::{
    local_socket::{
        GenericFilePath, ListenerOptions, Name, ToFsName,
        tokio::{Listener, Stream, prelude::*},
    },
    os::unix::local_socket::ListenerOptionsExt,
};
use std::{
    fs, io,
    os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

fn socket_path(database: &Path) -> io::Result<PathBuf> {
    let parent = database
        .parent()
        .ok_or_else(|| io::Error::other("missing profile directory"))?;
    Ok(parent.join("ipc").join(format!(
        "agent-{:016x}.sock",
        crate::instance::instance_identity(database)
    )))
}

pub(super) fn endpoint_name(path: &Path) -> io::Result<Name<'static>> {
    socket_path(path)?.to_fs_name::<GenericFilePath>()
}

fn effective_uid() -> u32 {
    // SAFETY: geteuid takes no pointers and has no failure mode.
    unsafe { libc::geteuid() }
}

pub(super) fn bind(endpoint: &AgentEndpoint) -> io::Result<Listener> {
    let socket = socket_path(&endpoint.database_path)?;
    let directory = socket
        .parent()
        .ok_or_else(|| io::Error::other("missing socket directory"))?;
    match fs::DirBuilder::new().mode(0o700).create(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(directory)?;
    if !metadata.is_dir()
        || metadata.uid() != effective_uid()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "IPC directory must be private and owned by the current user",
        ));
    }
    // The caller holds the same profile lock used by both Agent and old app.
    // Remove only a stale socket owned by this user, never a symlink/regular file.
    match fs::symlink_metadata(&socket) {
        Ok(metadata) if metadata.file_type().is_socket() && metadata.uid() == effective_uid() => {
            fs::remove_file(&socket)?
        }
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe IPC socket path",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    ListenerOptions::new()
        .name(endpoint.name()?)
        .mode(0o600)
        .create_tokio()
}

pub(super) fn verify_peer(stream: &Stream) -> io::Result<PeerIdentity> {
    let credentials = stream.peer_creds()?;
    if credentials.euid() != Some(effective_uid()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "IPC peer is not the current user",
        ));
    }
    Ok(PeerIdentity {
        process_id: credentials.pid().and_then(|pid| u32::try_from(pid).ok()),
    })
}

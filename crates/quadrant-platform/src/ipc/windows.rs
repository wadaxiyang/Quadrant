// SPDX-License-Identifier: GPL-3.0-only
//! Protected named-pipe ACL granting only the current token user access.

use super::{AgentEndpoint, PeerIdentity};
use interprocess::{
    local_socket::{
        GenericNamespaced, ListenerOptions, Name, ToNsName,
        tokio::{Listener, Stream, prelude::*},
    },
    os::windows::{local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor},
};
use std::{io, path::Path};
use widestring::U16CString;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree},
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY, TOKEN_USER,
            TokenUser,
        },
        System::Threading::{
            GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
    core::PWSTR,
};

pub(super) async fn drain(stream: &Stream) {
    let Stream::NamedPipe(pipe) = stream;
    // Only a server can cancel a blocked native drain by disconnecting the
    // pipe. Client closure is immediate; its EOF also releases the GUI session.
    if !pipe.inner().is_server() {
        return;
    }
    // LocalSocket AsyncWrite::flush is a no-op. Explicitly drain the native
    // pipe before disconnecting, without letting an unread peer block shutdown.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), pipe.inner().flush()).await;
}

pub(super) fn disconnect(stream: &Stream) {
    use std::os::windows::io::{AsHandle, AsRawHandle};
    let Stream::NamedPipe(pipe) = stream;
    // Bypass interprocess's global blocking linger queue: an unread peer would
    // otherwise hold unrelated connections open. The explicit drain above is
    // used on graceful close; cancellation takes this immediate cleanup path.
    pipe.inner().assume_flushed();
    if pipe.inner().is_server() {
        // SAFETY: the borrowed handle remains owned by this live server stream.
        // Disconnect also releases a timed-out native FlushFileBuffers worker.
        let _ = unsafe {
            windows::Win32::System::Pipes::DisconnectNamedPipe(HANDLE(
                pipe.inner().as_handle().as_raw_handle(),
            ))
        };
    }
}

pub(super) fn endpoint_name(path: &Path) -> io::Result<Name<'static>> {
    let sid = current_user_sid()?;
    format!(
        "quadrant-agent-{sid}-{:016x}",
        crate::instance::instance_identity(path)
    )
    .to_ns_name::<GenericNamespaced>()
}

pub(super) fn bind(endpoint: &AgentEndpoint) -> io::Result<Listener> {
    let sid = current_user_sid()?;
    // Protected DACL: deny network logons, grant only this token user. The
    // interprocess local listener also sets PIPE_REJECT_REMOTE_CLIENTS by default.
    let sddl = U16CString::from_str(format!("D:P(D;;GA;;;NU)(A;;GA;;;{sid})"))
        .map_err(io::Error::other)?;
    let descriptor = SecurityDescriptor::deserialize(&sddl)?;
    ListenerOptions::new()
        .name(endpoint.name()?)
        .security_descriptor(descriptor)
        .create_tokio()
}

pub(super) fn verify_peer(stream: &Stream) -> io::Result<PeerIdentity> {
    let pid = stream
        .peer_creds()?
        .pid()
        .ok_or_else(|| io::Error::other("missing IPC peer PID"))?;
    // SAFETY: only query access is requested for the kernel-reported peer PID.
    let process = Token(
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map_err(io::Error::other)?,
    );
    // Check the token user, not just the claimed Hello PID. This also rejects
    // a foreign-user server impersonating our pipe name before Agent startup.
    if process_user_sid(process.0)? != current_user_sid()? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "IPC peer is not the current user",
        ));
    }
    Ok(PeerIdentity {
        process_id: Some(pid),
    })
}

struct Token(HANDLE);
impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: this wrapper exclusively owns an opened process/token handle.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn current_user_sid() -> io::Result<String> {
    // SAFETY: obtains this process's valid pseudo-handle, which must not be closed.
    process_user_sid(unsafe { GetCurrentProcess() })
}

fn process_user_sid(process: HANDLE) -> io::Result<String> {
    let mut handle = HANDLE::default();
    // SAFETY: caller supplies a live process handle; output is writable.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut handle) }.map_err(io::Error::other)?;
    let token = Token(handle);
    let mut length = 0;
    // SAFETY: the first call only obtains the required buffer size.
    let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &raw mut length) };
    if length == 0 {
        return Err(io::Error::last_os_error());
    }
    // Word alignment satisfies TOKEN_USER and its embedded SID representation.
    let mut buffer = vec![0_usize; (length as usize).div_ceil(size_of::<usize>())];
    // SAFETY: buffer is aligned, live, and has at least `length` writable bytes.
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            length,
            &raw mut length,
        )
    }
    .map_err(io::Error::other)?;
    // SAFETY: successful TokenUser query initialized this aligned TOKEN_USER.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut encoded = PWSTR::null();
    // SAFETY: SID points into the live token buffer; output is allocated by Win32.
    unsafe { ConvertSidToStringSidW(user.User.Sid, &raw mut encoded) }.map_err(io::Error::other)?;
    // SAFETY: successful conversion returns a terminated UTF-16 string.
    let result = unsafe { encoded.to_string() }.map_err(io::Error::other);
    // SAFETY: ConvertSidToStringSidW requires LocalFree for this allocation.
    let _ = unsafe { LocalFree(Some(HLOCAL(encoded.0.cast()))) };
    result
}

// SPDX-License-Identifier: GPL-3.0-only
//! Bounded GUI IPC execution, with session-scoped admission and no mutation replay.

mod connection;
#[cfg(test)]
mod tests;

use connection::{Connection, SessionEnd};
use quadrant_platform::AgentEndpoint;
use quadrant_protocol::{AppSnapshot, ClientUpdate, ConnectionState, GuiCommand, GuiLaunchMode};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{mpsc, oneshot};

/// Safe startup/transport failures. Raw transport details remain error sources.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Connection or transport I/O failed.
    #[error(
        "Quadrant could not connect to its background service. Restart Quadrant from the complete installation."
    )]
    Io(#[from] std::io::Error),
    /// Invalid or incomplete frame.
    #[error("The background connection was interrupted or returned an invalid message.")]
    Codec(#[from] quadrant_protocol::codec::CodecError),
    /// The peer uses a different wire contract.
    #[error(
        "The GUI and background service use incompatible versions. Restart both from the same installation."
    )]
    Incompatible,
    /// The peer broke message ordering/correlation invariants.
    #[error("The background service returned an unexpected response. Restart Quadrant.")]
    Protocol,
    /// A bounded operation expired.
    #[error("The background service did not respond in time.")]
    Timeout,
}

impl ClientError {
    /// Only an absent listener authorizes automatic Agent startup. Permission,
    /// protocol and response-time failures must never spawn a competing Agent.
    #[must_use]
    pub fn agent_absent(&self) -> bool {
        matches!(self, Self::Io(error) if matches!(error.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused))
    }
    fn recoverable(&self) -> bool {
        matches!(
            self,
            Self::Io(_) | Self::Timeout | Self::Codec(quadrant_protocol::codec::CodecError::Io(_))
        )
    }
}

/// Submission was not accepted; no success or persistence may be assumed.
#[derive(Debug, thiserror::Error)]
#[error("Wait for the current operation or reconnect before trying again.")]
pub struct SubmitRejected;

pub(crate) struct Submission {
    epoch: u64,
    command: GuiCommand,
}

/// Nonblocking callback port. At most one command is admitted at a time.
#[derive(Clone)]
pub struct ClientHandle {
    sender: mpsc::Sender<Submission>,
    // Low bits: 0 offline, 1 ready, 2 busy. High bits identify the connection.
    admission: Arc<AtomicU64>,
}

impl ClientHandle {
    /// Enqueues a command only for the currently ready connection.
    /// # Errors
    /// Rejects busy/disconnected/closed sessions without queuing offline work.
    pub fn submit(&self, command: GuiCommand) -> Result<(), SubmitRejected> {
        let token = self.admission.load(Ordering::SeqCst);
        if token & 3 != 1
            || self
                .admission
                .compare_exchange(token, token + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        {
            return Err(SubmitRejected);
        }
        if self
            .sender
            .try_send(Submission {
                epoch: token & !3,
                command,
            })
            .is_err()
        {
            let _ = self.admission.compare_exchange(
                token + 1,
                token,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            return Err(SubmitRejected);
        }
        Ok(())
    }
}

/// GUI-owned transport worker. Its runtime is supplied by the GUI bootstrap.
pub struct GuiClient {
    endpoint: AgentEndpoint,
    mode: GuiLaunchMode,
    connection: Connection,
    receiver: mpsc::Receiver<Submission>,
    admission: Arc<AtomicU64>,
}

pub(crate) type UpdateSink = Arc<dyn Fn(ClientUpdate) + Send + Sync>;

impl GuiClient {
    /// Starts a missing Agent once, then waits finitely for an accepted session.
    /// Agent-launched children pass `false` to prevent parent resurrection on Exit.
    /// The composition root owns any process handle created by `start`.
    /// # Errors
    /// Returns launch, incompatible protocol, denied access or bounded startup errors.
    pub async fn connect_or_start(
        endpoint: AgentEndpoint,
        mode: GuiLaunchMode,
        allow_start: bool,
        start: impl FnOnce() -> std::io::Result<()>,
    ) -> Result<Option<(Self, ClientHandle)>, ClientError> {
        connect_or_start_with(allow_start, start, || Self::connect(endpoint.clone(), mode)).await
    }

    /// Tries once to connect, negotiate, and load the snapshot before Slint construction.
    /// Startup and recovery callers own their separate bounded retry policies.
    /// `None` means the Agent activated an existing GUI and this invocation exits.
    /// # Errors
    /// Returns incompatible protocol, invalid response or bounded connection failure.
    pub async fn connect(
        endpoint: AgentEndpoint,
        mode: GuiLaunchMode,
    ) -> Result<Option<(Self, ClientHandle)>, ClientError> {
        let Some(connection) = Connection::open(&endpoint, mode).await? else {
            return Ok(None);
        };
        let (sender, receiver) = mpsc::channel(1);
        let admission = Arc::new(AtomicU64::new(4));
        Ok(Some((
            Self {
                endpoint,
                mode,
                connection,
                receiver,
                admission: admission.clone(),
            },
            ClientHandle { sender, admission },
        )))
    }

    /// Authoritative initial state; reading this never touches local storage.
    #[must_use]
    pub fn snapshot(&self) -> &AppSnapshot {
        &self.connection.snapshot
    }

    /// Runs until GUI shutdown, Agent full exit, redirect, or exhausted recovery.
    /// The caller initializes its shell from `snapshot()` before starting this worker;
    /// only a successful reconnect publishes a replacement snapshot to the sink.
    /// A dedicated shutdown signal works even when callbacks retain submission handles.
    pub async fn run(self, sink: UpdateSink, mut shutdown: oneshot::Receiver<()>) {
        let Self {
            endpoint,
            mode,
            mut connection,
            mut receiver,
            admission,
        } = self;
        let mut epoch = 4_u64;
        loop {
            admission.store(epoch | 1, Ordering::SeqCst);
            status(&sink, ConnectionState::Ready, "");
            let result = connection
                .run(&mut receiver, &admission, epoch, &sink, &mut shutdown)
                .await;
            admission.store(epoch, Ordering::SeqCst);
            match result {
                SessionEnd::Closed => return,
                SessionEnd::Lost(error) => {
                    status(
                        &sink,
                        ConnectionState::Reconnecting,
                        "Connection lost. An unconfirmed operation may have completed; it will not be sent again. Reconnecting…",
                    );
                    if !error.recoverable() {
                        status(&sink, ConnectionState::Unavailable, &error.to_string());
                        return;
                    }
                }
            }
            // Never replay old submissions, including a callback that completed
            // its enqueue after this drain. Epoch checking rejects that race.
            while receiver.try_recv().is_ok() {}
            let retry = async {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                connect_with_retry(&endpoint, mode).await
            };
            let result = tokio::select! {
                biased;
                _ = &mut shutdown => return,
                result = retry => result,
            };
            match result {
                Ok(Some(next)) => {
                    connection = next;
                    let Some(next_epoch) = epoch.checked_add(4) else {
                        return;
                    };
                    epoch = next_epoch;
                    // Bootstrap already initialized the shell. Only reconnect must
                    // hydrate the existing GUI, before Ready and queued peer events.
                    sink(ClientUpdate::Snapshot(connection.snapshot.clone()));
                }
                Ok(None) => {
                    if mode == GuiLaunchMode::QuickAdd {
                        // A newly opened Main may own capture now. This existing
                        // form can still hold an unsaved or unconfirmed draft.
                        status(
                            &sink,
                            ConnectionState::Unavailable,
                            "Another interface is active. This draft was not resent. Check the task list and copy any unsaved text before closing.",
                        );
                    } else {
                        sink(ClientUpdate::Event(quadrant_protocol::ServerEvent::ExitGui));
                    }
                    return;
                }
                Err(error) => {
                    status(
                        &sink,
                        ConnectionState::Unavailable,
                        &format!(
                            "{error} Close and reopen Quadrant to retry. Any unconfirmed operation was not resent."
                        ),
                    );
                    return;
                }
            }
        }
    }
}

pub(crate) fn status(sink: &UpdateSink, state: ConnectionState, message: &str) {
    sink(ClientUpdate::Connection {
        state,
        message: message.to_owned(),
    });
}

// The injected single attempt keeps startup policy testable with exact I/O errors
// and a controlled clock, without native ACL changes or real process spawning.
async fn connect_or_start_with<T, F>(
    allow_start: bool,
    start: impl FnOnce() -> std::io::Result<()>,
    mut connect: impl FnMut() -> F,
) -> Result<T, ClientError>
where
    F: std::future::Future<Output = Result<T, ClientError>>,
{
    match connect().await {
        Err(error) if allow_start && error.agent_absent() => start()?,
        result => return result,
    }
    // Spawn exactly once, immediately after a confirmed absent endpoint. Bound
    // the entire readiness wait, including Hello/snapshot negotiation time.
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut delay = 25;
        loop {
            match connect().await {
                Err(error) if error.agent_absent() => {
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    delay = (delay * 2).min(400);
                }
                result => return result,
            }
        }
    })
    .await
    .map_err(|_| ClientError::Timeout)?
}

// Established-session recovery only. Never use this backoff before deciding
// whether an explicit GUI launch needs to start a missing Agent.
async fn connect_with_retry(
    endpoint: &AgentEndpoint,
    mode: GuiLaunchMode,
) -> Result<Option<Connection>, ClientError> {
    let mut failure = ClientError::Timeout;
    for delay in [0, 250, 750] {
        if delay != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
        }
        match Connection::open(endpoint, mode).await {
            Ok(connection) => return Ok(connection),
            Err(error) if error.recoverable() => failure = error,
            Err(error) => return Err(error),
        }
    }
    Err(failure)
}

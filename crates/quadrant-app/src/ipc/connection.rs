// SPDX-License-Identifier: GPL-3.0-only
//! One negotiated stream and its ordered response loop.

use super::{ClientError, Submission, UpdateSink, status};
use quadrant_platform::{AgentEndpoint, AgentStream};
use quadrant_protocol::{
    AppSnapshot, ClientHello, ClientMessage, ClientUpdate, ConnectionState, GuiCommand,
    GuiDisposition, GuiLaunchMode, PROTOCOL_VERSION, RequestId, ServerEvent, ServerMessage,
    SessionId,
    codec::{read_message_async, write_message_async},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::WriteHalf,
    sync::{mpsc, oneshot},
    time::Instant,
};

pub(super) enum SessionEnd {
    Closed,
    Lost(ClientError),
}

pub(super) struct Connection {
    stream: AgentStream,
    session: SessionId,
    pub snapshot: Box<AppSnapshot>,
    early_events: Vec<ServerEvent>,
}

impl Connection {
    pub async fn open(
        endpoint: &AgentEndpoint,
        mode: GuiLaunchMode,
    ) -> Result<Option<Self>, ClientError> {
        tokio::time::timeout(Duration::from_secs(10), Self::negotiate(endpoint, mode))
            .await
            .map_err(|_| ClientError::Timeout)?
    }

    async fn negotiate(
        endpoint: &AgentEndpoint,
        mode: GuiLaunchMode,
    ) -> Result<Option<Self>, ClientError> {
        let mut stream = endpoint.connect().await?;
        write_message_async(
            &mut stream,
            &ClientMessage::Hello(ClientHello {
                protocol_version: PROTOCOL_VERSION,
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
                process_id: std::process::id(),
                mode,
            }),
        )
        .await?;
        let Some(ServerMessage::HelloAck(ack)) = read_message_async(&mut stream).await? else {
            return Err(ClientError::Protocol);
        };
        if ack.protocol_version != PROTOCOL_VERSION
            || ack.disposition == GuiDisposition::RejectIncompatibleVersion
        {
            return Err(ClientError::Incompatible);
        }
        if ack.disposition == GuiDisposition::ActivateExistingAndExit {
            return Ok(None);
        }
        let session = ack.session_id.ok_or(ClientError::Protocol)?;
        let request_id = RequestId::new(1).ok_or(ClientError::Protocol)?;
        write_message_async(
            &mut stream,
            &ClientMessage::GetInitialSnapshot { request_id },
        )
        .await?;
        let mut early_events = Vec::new();
        loop {
            match read_message_async(&mut stream).await? {
                Some(ServerMessage::InitialSnapshot {
                    request_id: actual,
                    snapshot,
                }) if actual == request_id => {
                    return Ok(Some(Self {
                        stream,
                        session,
                        snapshot,
                        early_events,
                    }));
                }
                Some(ServerMessage::Event(
                    event @ (ServerEvent::ActivateMainWindow | ServerEvent::OpenQuickAdd),
                )) if early_events.len() < 8 => early_events.push(event),
                _ => return Err(ClientError::Protocol),
            }
        }
    }

    pub async fn run(
        self,
        commands: &mut mpsc::Receiver<Submission>,
        admission: &Arc<AtomicU64>,
        epoch: u64,
        sink: &UpdateSink,
        shutdown: &mut oneshot::Receiver<()>,
    ) -> SessionEnd {
        let (mut reader, mut writer) = tokio::io::split(self.stream);
        let (messages, mut incoming) = mpsc::channel(8);
        // One persistent decoder owns partial reads. Selecting commands must not
        // cancel and resume a partially consumed frame.
        let reader_worker = tokio::spawn(async move {
            loop {
                let message = read_message_async(&mut reader).await;
                let terminal = !matches!(&message, Ok(Some(_)));
                if messages.send(message).await.is_err() || terminal {
                    break;
                }
            }
        });
        for event in self.early_events {
            sink(ClientUpdate::Event(event));
        }
        let result = session_loop(
            &mut writer,
            &mut incoming,
            commands,
            admission,
            epoch,
            self.session,
            sink,
            shutdown,
        )
        .await;
        reader_worker.abort();
        let _ = reader_worker.await;
        result
    }
}

struct Pending {
    id: RequestId,
    command: GuiCommand,
    deadline: Instant,
}
type Incoming = mpsc::Receiver<Result<Option<ServerMessage>, quadrant_protocol::codec::CodecError>>;

#[allow(clippy::too_many_arguments)] // Explicit session-owned channels and identity; no globals.
async fn session_loop(
    writer: &mut WriteHalf<AgentStream>,
    incoming: &mut Incoming,
    commands: &mut mpsc::Receiver<Submission>,
    admission: &AtomicU64,
    epoch: u64,
    session: SessionId,
    sink: &UpdateSink,
    shutdown: &mut oneshot::Receiver<()>,
) -> SessionEnd {
    let mut sequence = 1_u64;
    let mut pending: Option<Pending> = None;
    loop {
        let deadline = async {
            if let Some(pending) = &pending {
                tokio::time::sleep_until(pending.deadline).await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::select! {
            biased;
            _ = &mut *shutdown => {
                let _ = tokio::time::timeout(Duration::from_millis(500), write_message_async(writer, &ClientMessage::GuiClosing { session_id: session })).await;
                return SessionEnd::Closed;
            }
            () = deadline => return SessionEnd::Lost(ClientError::Timeout),
            message = incoming.recv() => match message {
                Some(Ok(Some(ServerMessage::Event(event)))) => {
                    let closed = matches!(event, ServerEvent::AgentShuttingDown | ServerEvent::ExitGui);
                    sink(ClientUpdate::Event(event));
                    if closed { return SessionEnd::Closed; }
                }
                Some(Ok(Some(ServerMessage::CommandResult { request_id, outcome }))) => {
                    let Some(request) = pending.take() else { return SessionEnd::Lost(ClientError::Protocol); };
                    if request.id != request_id { return SessionEnd::Lost(ClientError::Protocol); }
                    sink(ClientUpdate::CommandFinished { command: request.command, outcome });
                    admission.store(epoch | 1, Ordering::SeqCst);
                    status(sink, ConnectionState::Ready, "");
                }
                Some(Ok(None)) | None => return SessionEnd::Lost(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into()),
                Some(Err(error)) => return SessionEnd::Lost(error.into()),
                _ => return SessionEnd::Lost(ClientError::Protocol),
            },
            command = commands.recv(), if pending.is_none() => {
                let Some(submission) = command else { return SessionEnd::Closed; };
                if submission.epoch != epoch { continue; }
                let Some(next) = sequence.checked_add(1) else { return SessionEnd::Lost(ClientError::Protocol); };
                sequence = next;
                let Some(request_id) = RequestId::new(sequence) else { return SessionEnd::Lost(ClientError::Protocol); };
                status(sink, ConnectionState::Busy, "Waiting for the background service…");
                let message = ClientMessage::Command { request_id, command: submission.command.clone() };
                let write = tokio::time::timeout(Duration::from_secs(5), write_message_async(writer, &message));
                let result = tokio::select! {
                    biased;
                    _ = &mut *shutdown => return SessionEnd::Closed,
                    result = write => result,
                };
                match result {
                    Ok(Ok(())) => pending = Some(Pending { id: request_id, command: submission.command, deadline: Instant::now() + Duration::from_secs(30) }),
                    Ok(Err(error)) => return SessionEnd::Lost(error.into()),
                    Err(_) => return SessionEnd::Lost(ClientError::Timeout),
                }
            }
        }
    }
}

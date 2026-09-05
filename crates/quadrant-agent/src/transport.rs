// SPDX-License-Identifier: GPL-3.0-only
//! Bounded local connection workers on the one Agent runtime.

use quadrant_platform::{AgentListener, AgentStream, PeerIdentity};
use quadrant_protocol::{
    ClientMessage, ServerMessage, SessionId,
    codec::{read_message_async, write_message_async},
};
use std::time::Duration;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
};

pub(crate) enum Input {
    Connected {
        id: SessionId,
        peer: PeerIdentity,
        outgoing: mpsc::Sender<ServerMessage>,
    },
    Message {
        id: SessionId,
        message: ClientMessage,
    },
    Disconnected(SessionId),
    ListenerFailed,
}

pub(crate) async fn run(
    listener: AgentListener,
    sender: mpsc::Sender<Input>,
    mut stop: oneshot::Receiver<()>,
) {
    let mut clients = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = &mut stop => break,
            _ = clients.join_next(), if !clients.is_empty() => {},
            connection = listener.accept(), if clients.len() < 8 => {
                match connection {
                    Ok((stream, peer)) => { clients.spawn(connection_worker(stream, peer, sender.clone())); }
                    Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {},
                    Err(_) => { let _ = sender.send(Input::ListenerFailed).await; break; }
                }
            }
        }
    }
    drop(listener);
    // Broker drops output senders after queuing shutdown events. Writers drain,
    // then end their readers. A malicious peer cannot indefinitely delay Exit.
    if tokio::time::timeout(Duration::from_secs(3), async {
        while clients.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        clients.abort_all();
        while clients.join_next().await.is_some() {}
    }
}

async fn connection_worker(stream: AgentStream, peer: PeerIdentity, sender: mpsc::Sender<Input>) {
    let id = SessionId::generate();
    let (outgoing, mut output) = mpsc::channel(8);
    if sender
        .send(Input::Connected { id, peer, outgoing })
        .await
        .is_err()
    {
        return;
    }
    let (mut reader, mut writer) = tokio::io::split(stream);
    let read = async {
        let first =
            tokio::time::timeout(Duration::from_secs(10), read_message_async(&mut reader)).await;
        let Ok(Ok(Some(message))) = first else {
            return;
        };
        if sender.send(Input::Message { id, message }).await.is_err() {
            return;
        }
        while let Ok(Some(message)) = read_message_async(&mut reader).await {
            if sender.send(Input::Message { id, message }).await.is_err() {
                break;
            }
        }
    };
    let write = async {
        while let Some(message) = output.recv().await {
            if !matches!(
                tokio::time::timeout(
                    Duration::from_secs(10),
                    write_message_async(&mut writer, &message)
                )
                .await,
                Ok(Ok(()))
            ) {
                break;
            }
        }
    };
    // Never cancel and resume a partial frame. Either worker ending closes both halves.
    tokio::select! { () = read => {}, () = write => {} }
    let _ = sender.send(Input::Disconnected(id)).await;
    reader.unsplit(writer).close().await;
}

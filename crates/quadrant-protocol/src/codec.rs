// SPDX-License-Identifier: GPL-3.0-only
//! Transport-neutral framing: four-byte big-endian payload length, then UTF-8 JSON.
//!
//! Each payload is between one byte and [`MAX_MESSAGE_BYTES`]. A bad length,
//! malformed message, or truncated frame is fatal to the connection; do not try
//! to resynchronize. Clean EOF is reported separately from a partial frame.
//! Blocking helpers belong only on explicitly blocking transport workers, never
//! the Slint thread. Async `interprocess` adapters can use the same header check
//! and payload functions without creating a second wire format.

use std::io::{self, Read, Write};

use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

/// Maximum JSON payload size (8 MiB), checked before incoming allocation.
pub const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

/// Header size shared by blocking and future asynchronous local transports.
pub const HEADER_BYTES: usize = 4;

/// Reads a frame on the caller-owned runtime, with a deadline once framing starts.
///
/// Idle connections wait indefinitely; partial frames have a 30-second deadline.
/// This future is not cancellation-safe: cancel only when closing the stream.
/// # Errors
/// Returns length, decoding, I/O, or partial-frame timeout failures.
pub async fn read_message_async<R: tokio::io::AsyncRead + Unpin, T: DeserializeOwned>(
    reader: &mut R,
) -> Result<Option<T>, CodecError> {
    use tokio::io::AsyncReadExt;
    let mut header = [0; HEADER_BYTES];
    if reader.read(&mut header[..1]).await? == 0 {
        return Ok(None);
    }
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        reader.read_exact(&mut header[1..]).await?;
        let mut payload = vec![0; payload_length(header)?];
        reader.read_exact(&mut payload).await?;
        decode_payload(&payload).map(Some)
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "partial IPC frame timed out"))?
}

/// Writes a frame using the caller-owned async runtime and the same v1 encoding.
///
/// # Errors
/// Returns length, encoding, or stream failures. Cancellation closes the stream.
pub async fn write_message_async<W: tokio::io::AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    message: &T,
) -> Result<(), CodecError> {
    use tokio::io::AsyncWriteExt;
    let payload = encode_payload(message)?;
    let length =
        u32::try_from(payload.len()).map_err(|_| CodecError::InvalidLength(payload.len()))?;
    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(&payload).await?;
    writer.flush().await?;
    Ok(())
}

/// Framing, serialization, or stream failure.
#[derive(Debug, Error)]
pub enum CodecError {
    /// Empty or oversized payload, which must not be allocated/read.
    #[error("protocol message length {0} is outside the supported range")]
    InvalidLength(usize),
    /// Stream I/O failed, including EOF in the middle of a frame.
    #[error("local protocol stream I/O failed")]
    Io(#[from] io::Error),
    /// Payload is not a supported typed JSON message.
    #[error("local protocol message encoding is invalid")]
    Json(#[from] serde_json::Error),
}

/// Validates a header before an adapter allocates its payload buffer.
///
/// # Errors
/// Returns [`CodecError::InvalidLength`] for empty or oversized frames.
pub fn payload_length(header: [u8; HEADER_BYTES]) -> Result<usize, CodecError> {
    let length = u32::from_be_bytes(header) as usize;
    check_length(length)?;
    Ok(length)
}

fn check_length(length: usize) -> Result<(), CodecError> {
    if length == 0 || length > MAX_MESSAGE_BYTES {
        Err(CodecError::InvalidLength(length))
    } else {
        Ok(())
    }
}

/// Serializes a bounded payload, without a transport-specific header.
///
/// # Errors
/// Returns an encoding error or an oversized-message failure.
pub fn encode_payload<T: Serialize>(message: &T) -> Result<Vec<u8>, CodecError> {
    let payload = serde_json::to_vec(message)?;
    check_length(payload.len())?;
    Ok(payload)
}

/// Decodes one complete bounded JSON payload into its typed message.
///
/// # Errors
/// Returns a length or JSON error, including unknown enum variants and trailing data.
pub fn decode_payload<T: DeserializeOwned>(payload: &[u8]) -> Result<T, CodecError> {
    check_length(payload.len())?;
    Ok(serde_json::from_slice(payload)?)
}

/// Writes one complete frame, then flushes buffered stream bytes.
///
/// # Errors
/// Returns a length, encoding, or stream error. A partial write is connection-fatal.
pub fn write_message<W: Write, T: Serialize>(
    writer: &mut W,
    message: &T,
) -> Result<(), CodecError> {
    let payload = encode_payload(message)?;
    let length =
        u32::try_from(payload.len()).map_err(|_| CodecError::InvalidLength(payload.len()))?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Reads one frame, returning `None` only for EOF before any header byte.
///
/// # Errors
/// Returns a length, decoding, or stream error. Partial EOF is always an error.
pub fn read_message<R: Read, T: DeserializeOwned>(reader: &mut R) -> Result<Option<T>, CodecError> {
    let mut header = [0; HEADER_BYTES];
    loop {
        match reader.read(&mut header[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    reader.read_exact(&mut header[1..])?;
    let length = payload_length(header)?;
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload)?;
    decode_payload(&payload).map(Some)
}

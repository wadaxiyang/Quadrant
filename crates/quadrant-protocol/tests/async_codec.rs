// SPDX-License-Identifier: GPL-3.0-only
//! Async transport keeps the v1 byte encoding and fatal-frame semantics.

use quadrant_protocol::codec::{self, CodecError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn fragmented_async_frames_match_blocking_encoding_and_clean_eof() {
    let message = vec!["象限", "Focus"];
    let mut expected = Vec::new();
    codec::write_message(&mut expected, &message).unwrap();
    let (mut sender, mut receiver) = tokio::io::duplex(2);
    let reader = tokio::spawn(async move {
        let mut bytes = Vec::new();
        receiver.read_to_end(&mut bytes).await.unwrap();
        bytes
    });
    codec::write_message_async(&mut sender, &message)
        .await
        .unwrap();
    drop(sender);
    assert_eq!(reader.await.unwrap(), expected);

    let (mut sender, mut receiver) = tokio::io::duplex(2);
    let writer = tokio::spawn(async move {
        for byte in expected {
            sender.write_all(&[byte]).await.unwrap();
        }
    });
    let decoded: Option<Vec<String>> = codec::read_message_async(&mut receiver).await.unwrap();
    assert_eq!(decoded.unwrap(), message);
    assert!(
        codec::read_message_async::<_, Vec<String>>(&mut receiver)
            .await
            .unwrap()
            .is_none()
    );
    writer.await.unwrap();
}

#[tokio::test]
async fn async_decoder_rejects_partial_and_invalid_frames() {
    for bytes in [vec![0], vec![0, 0, 0, 2, b'{']] {
        let error = codec::read_message_async::<_, serde_json::Value>(&mut bytes.as_slice())
            .await
            .unwrap_err();
        assert!(
            matches!(error, CodecError::Io(error) if error.kind() == std::io::ErrorKind::UnexpectedEof)
        );
    }
    for length in [0_u32, u32::MAX] {
        let bytes = length.to_be_bytes();
        assert!(matches!(
            codec::read_message_async::<_, serde_json::Value>(&mut bytes.as_slice()).await,
            Err(CodecError::InvalidLength(_))
        ));
    }
}

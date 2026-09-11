use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

pub const VERSION: u16 = 1;
pub const MAX_FRAME: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub version: u16,
    pub kind: String,
    pub id: Uuid,
    pub timestamp: i64,
    pub payload: serde_json::Value,
}

pub fn frame(kind: &str, payload: serde_json::Value) -> Frame {
    Frame {
        version: VERSION,
        kind: kind.to_owned(),
        id: Uuid::new_v4(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        payload,
    }
}
pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, value: &Frame) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_FRAME {
        bail!("frame too large")
    }
    writer.write_u32(bytes.len() as u32).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Frame> {
    let len = reader.read_u32().await? as usize;
    if len == 0 || len > MAX_FRAME {
        bail!("invalid frame length")
    }
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes).await?;
    let frame: Frame = serde_json::from_slice(&bytes)?;
    if frame.version != VERSION {
        bail!("unsupported protocol version")
    }
    if !matches!(
        frame.kind.as_str(),
        "hello"
            | "heartbeat"
            | "heartbeat.ack"
            | "chat.message"
            | "chat.ack"
            | "control.test"
            | "control.ack"
            | "pair.request"
            | "pair.response"
            | "file.offer"
            | "file.accept"
            | "file.chunk"
            | "file.complete"
            | "file.complete.ack"
            | "file.cancel"
    ) {
        bail!("unknown protocol operation")
    }
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn frames_survive_fragmented_reads() {
        let (mut writer, mut reader) = duplex(1024);
        let expected = frame(
            "control.test",
            serde_json::json!({"messageId": "generated"}),
        );
        let bytes = serde_json::to_vec(&expected).unwrap();
        let mut encoded = (bytes.len() as u32).to_be_bytes().to_vec();
        encoded.extend(bytes);
        let write_task = tokio::spawn(async move {
            for byte in encoded {
                tokio::io::AsyncWriteExt::write_all(&mut writer, &[byte])
                    .await
                    .unwrap();
                tokio::task::yield_now().await;
            }
        });
        let actual = read_frame(&mut reader).await.unwrap();
        write_task.await.unwrap();
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.payload, expected.payload);
    }

    #[tokio::test]
    async fn unknown_operations_are_rejected() {
        let (mut writer, mut reader) = duplex(1024);
        let invalid = frame("unknown.critical", serde_json::json!({}));
        let bytes = serde_json::to_vec(&invalid).unwrap();
        tokio::io::AsyncWriteExt::write_u32(&mut writer, bytes.len() as u32)
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut writer, &bytes)
            .await
            .unwrap();
        assert!(read_frame(&mut reader).await.is_err());
    }
}

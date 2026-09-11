use super::{protocol, ChatMessage, LocalShareCore};
use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
pub async fn send(core: Arc<LocalShareCore>, peer_id: &str, body: &str) -> Result<ChatMessage> {
    if body.is_empty() || body.len() > 64 * 1024 {
        anyhow::bail!("invalid chat body")
    }
    if !core.is_peer_trusted(peer_id)? {
        anyhow::bail!("peer is not paired")
    }
    let msg = ChatMessage {
        id: uuid::Uuid::new_v4().to_string(),
        peer_id: peer_id.to_owned(),
        body: body.to_owned(),
        direction: "out".into(),
        state: "QUEUED".into(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    };
    core.store()
        .save_message(&msg.id, peer_id, body, "out", "QUEUED", msg.timestamp)?;
    core.emit(super::CoreEvent::Chat(msg.clone()));
    let send_result = core
        .send_frame(
            peer_id,
            protocol::frame("chat.message", json!({"messageId":msg.id,"body":body})),
        )
        .await;
    if send_result.is_err() {
        return Ok(msg);
    }
    core.store().update_message_state(&msg.id, "SENT")?;
    core.emit(super::CoreEvent::Chat(ChatMessage {
        state: "SENT".into(),
        ..msg.clone()
    }));
    Ok(msg)
}

pub async fn flush_queued(core: std::sync::Arc<LocalShareCore>, peer_id: &str) -> Result<()> {
    for msg in core.store().queued_messages(peer_id)? {
        if core
            .send_frame(
                peer_id,
                protocol::frame(
                    "chat.message",
                    json!({"messageId": msg.id, "body": msg.body}),
                ),
            )
            .await
            .is_err()
        {
            break;
        }
        core.store().update_message_state(&msg.id, "SENT")?;
        core.emit(super::CoreEvent::Chat(super::ChatMessage {
            state: "SENT".into(),
            ..msg
        }));
    }
    Ok(())
}

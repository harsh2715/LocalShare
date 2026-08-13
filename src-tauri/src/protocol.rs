use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello { protocol_version: u32, device_id: Uuid, device_name: String, fingerprint: String },
    HelloAck { protocol_version: u32 },
    Ping,
    Pong,
    Chat { id: Uuid, text: String, sent_at: i64 },
    TransferRequest { id: Uuid, files: Vec<FileMeta>, total_bytes: u64 },
    TransferAccept { id: Uuid },
    TransferReject { id: Uuid, reason: String },
    TransferCancel { id: Uuid },
    FileStart { transfer_id: Uuid, file: FileMeta, hash: String },
    FileChunk { transfer_id: Uuid, file_id: Uuid, offset: u64, data: Vec<u8> },
    FileEnd { transfer_id: Uuid, file_id: Uuid },
    TransferComplete { id: Uuid },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    pub id: Uuid,
    pub name: String,
    pub relative_path: Option<String>,
    pub size: u64,
}

pub fn encode(message: &Message) -> Result<Vec<u8>, serde_json::Error> { serde_json::to_vec(message) }

pub fn validate(message: &Message) -> Result<(), String> {
    match message {
        Message::Hello { protocol_version, device_name, fingerprint, .. } => {
            if *protocol_version != PROTOCOL_VERSION { return Err("Unsupported protocol version".into()); }
            if device_name.trim().is_empty() || device_name.chars().count() > 80 { return Err("Invalid device name".into()); }
            if fingerprint.len() > 128 { return Err("Invalid fingerprint".into()); }
        }
        Message::Chat { text, .. } if text.chars().count() > 16_000 => return Err("Message is too long".into()),
        Message::TransferRequest { files, total_bytes, .. } => {
            if files.is_empty() || files.len() > 10_000 { return Err("Invalid file manifest".into()); }
            let sum = files.iter().try_fold(0u64, |acc, f| acc.checked_add(f.size)).ok_or("Size overflow")?;
            if sum != *total_bytes { return Err("Manifest size mismatch".into()); }
            for file in files { validate_filename(&file.name)?; }
        }
        Message::HelloAck { protocol_version } if *protocol_version != PROTOCOL_VERSION => return Err("Unsupported protocol version".into()),
        Message::FileChunk { data, .. } if data.len() > 4 * 1024 * 1024 => return Err("Chunk too large".into()),
        _ => {}
    }
    Ok(())
}

pub fn validate_filename(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 255 || name == "." || name == ".." { return Err("Invalid filename".into()); }
    if name.contains('/') || name.contains('\\') || name.contains('\0') { return Err("Unsafe filename".into()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn rejects_path_traversal() { assert!(validate_filename("../secret").is_err()); assert!(validate_filename("C:\\Windows\\x").is_err()); }
    #[test] fn round_trips_unicode_chat() { let msg = Message::Chat { id: Uuid::new_v4(), text: "नमस्ते 👋".into(), sent_at: 1 }; let bytes = encode(&msg).unwrap(); let out: Message = serde_json::from_slice(&bytes).unwrap(); assert!(matches!(out, Message::Chat { text, .. } if text == "नमस्ते 👋")); }
    #[test] fn rejects_bad_manifest_and_version() { let msg = Message::TransferRequest { id: Uuid::new_v4(), files: vec![FileMeta { id: Uuid::new_v4(), name: "x".into(), relative_path: None, size: 4 }], total_bytes: 3 }; assert!(validate(&msg).is_err()); assert!(validate(&Message::HelloAck { protocol_version: 99 }).is_err()); }
    #[test] fn rejects_absolute_and_windows_paths() { for name in ["../../secret.txt", "/Users/test.txt", "C:\\Windows\\test.txt", "..\\..\\bad.txt"] { assert!(validate_filename(name).is_err(), "accepted {name}"); } }
}

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 2;
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TRANSFER_BYTES: u64 = 4 * 1024 * 1024 * 1024 * 1024;
pub const MAX_FILE_BYTES: u64 = MAX_TRANSFER_BYTES;
pub const MAX_NONCE_BYTES: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello {
        protocol_version: u32,
        device_id: Uuid,
        device_name: String,
        fingerprint: String,
        public_key: Vec<u8>,
        listening_port: u16,
        nonce: Vec<u8>,
        signature: Vec<u8>,
        #[serde(default)]
        capabilities: Vec<String>,
    },
    HelloAck {
        protocol_version: u32,
        device_id: Uuid,
        public_key: Vec<u8>,
        signature: Vec<u8>,
    },
    Ping,
    Pong,
    Chat {
        id: Uuid,
        text: String,
        sent_at: i64,
    },
    TransferRequest {
        id: Uuid,
        files: Vec<FileMeta>,
        #[serde(default)]
        directories: Vec<String>,
        total_bytes: u64,
    },
    TransferAccept {
        id: Uuid,
    },
    TransferReject {
        id: Uuid,
        reason: String,
    },
    TransferCancel {
        id: Uuid,
    },
    FileStart {
        transfer_id: Uuid,
        file: FileMeta,
        hash: String,
    },
    FileChunk {
        transfer_id: Uuid,
        file_id: Uuid,
        offset: u64,
        data: Vec<u8>,
    },
    FileEnd {
        transfer_id: Uuid,
        file_id: Uuid,
    },
    TransferComplete {
        id: Uuid,
        name: String,
        size: u64,
        transferred: u64,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMeta {
    pub id: Uuid,
    pub name: String,
    pub relative_path: Option<String>,
    pub size: u64,
}

pub fn encode(message: &Message) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(message)
}

fn signing_bytes(tag: &[u8], fields: &[&[u8]]) -> Vec<u8> {
    let mut result =
        Vec::with_capacity(tag.len() + fields.iter().map(|field| field.len() + 4).sum::<usize>());
    result.extend_from_slice(tag);
    for field in fields {
        result.extend_from_slice(&(field.len() as u32).to_be_bytes());
        result.extend_from_slice(field);
    }
    result
}

#[allow(clippy::too_many_arguments)]
pub fn hello_signing_bytes(
    protocol_version: u32,
    device_id: Uuid,
    device_name: &str,
    fingerprint: &str,
    public_key: &[u8],
    listening_port: u16,
    nonce: &[u8],
    capabilities: &[String],
) -> Vec<u8> {
    let capability_bytes = capabilities.join("\0");
    signing_bytes(
        b"LocalShare/hello/v2",
        &[
            &protocol_version.to_be_bytes(),
            device_id.as_bytes(),
            device_name.as_bytes(),
            fingerprint.as_bytes(),
            public_key,
            &listening_port.to_be_bytes(),
            nonce,
            capability_bytes.as_bytes(),
        ],
    )
}

pub fn ack_signing_bytes(
    protocol_version: u32,
    challenger_id: Uuid,
    challenger_nonce: &[u8],
    responder_id: Uuid,
    responder_nonce: &[u8],
) -> Vec<u8> {
    signing_bytes(
        b"LocalShare/ack/v2",
        &[
            &protocol_version.to_be_bytes(),
            challenger_id.as_bytes(),
            challenger_nonce,
            responder_id.as_bytes(),
            responder_nonce,
        ],
    )
}

pub fn validate(message: &Message) -> Result<(), String> {
    match message {
        Message::Hello {
            protocol_version,
            device_id,
            device_name,
            fingerprint,
            public_key,
            listening_port,
            nonce,
            signature,
            capabilities,
        } => {
            if *protocol_version != PROTOCOL_VERSION {
                return Err("Unsupported protocol version".into());
            }
            if device_name.trim().is_empty()
                || device_name.chars().count() > 80
                || device_name.chars().any(char::is_control)
            {
                return Err("Invalid device name".into());
            }
            if *device_id == Uuid::nil()
                || *listening_port == 0
                || public_key.len() != 32
                || nonce.len() != MAX_NONCE_BYTES
                || signature.len() != 64
                || capabilities.len() > 32
                || capabilities.iter().any(|capability| {
                    capability.is_empty()
                        || capability.len() > 64
                        || capability.chars().any(char::is_control)
                })
            {
                return Err("Invalid fingerprint".into());
            }
            let mut hasher = Sha256::new();
            hasher.update(public_key);
            if fingerprint.len() != 64
                || hex::decode(fingerprint).is_err()
                || hex::encode(hasher.finalize()) != *fingerprint
            {
                return Err("Invalid device identity".into());
            }
        }
        Message::HelloAck {
            protocol_version,
            device_id,
            public_key,
            signature,
        } => {
            if *protocol_version != PROTOCOL_VERSION
                || *device_id == Uuid::nil()
                || public_key.len() != 32
                || signature.len() != 64
            {
                return Err("Invalid handshake acknowledgement".into());
            }
        }
        Message::Chat { text, .. } => {
            if text.trim().is_empty() {
                return Err("Message is empty".into());
            }
            if text.chars().count() > 16_000 {
                return Err("Message is too long".into());
            }
        }
        Message::TransferRequest {
            files,
            directories,
            total_bytes,
            ..
        } => {
            if (files.is_empty() && directories.is_empty())
                || files.len() > 10_000
                || directories.len() > 10_000
                || *total_bytes > MAX_TRANSFER_BYTES
            {
                return Err("Invalid file manifest".into());
            }
            let sum = files
                .iter()
                .try_fold(0u64, |acc, f| acc.checked_add(f.size))
                .ok_or("Size overflow")?;
            if sum != *total_bytes {
                return Err("Manifest size mismatch".into());
            }
            let mut ids = HashSet::with_capacity(files.len());
            let mut paths = HashSet::with_capacity(files.len());
            for file in files {
                validate_file_meta(file)?;
                if file.size > MAX_FILE_BYTES
                    || !ids.insert(file.id)
                    || !paths.insert(file.relative_path.as_deref().unwrap_or(&file.name))
                {
                    return Err("Duplicate or oversized file manifest entry".into());
                }
            }
            for directory in directories {
                validate_relative_path(directory)?;
            }
        }
        Message::FileStart { file, hash, .. } => {
            validate_file_meta(file)?;
            if file.size > MAX_FILE_BYTES || hash.len() != 64 || hex::decode(hash).is_err() {
                return Err("Invalid SHA-256 hash".into());
            }
        }
        Message::FileChunk { data, .. } if data.is_empty() => return Err("Empty chunk".into()),
        Message::FileChunk { data, .. } if data.len() > 4 * 1024 * 1024 => {
            return Err("Chunk too large".into())
        }
        Message::FileChunk { offset, data, .. }
            if offset.checked_add(data.len() as u64).is_none() =>
        {
            return Err("Chunk offset overflow".into())
        }
        _ => {}
    }
    Ok(())
}

pub fn validate_filename(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 255
        || name == "."
        || name == ".."
        || name.chars().any(char::is_control)
        || name.ends_with('.')
        || name.ends_with(' ')
    {
        return Err("Invalid filename".into());
    }
    if name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || name
            .chars()
            .any(|ch| matches!(ch, '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return Err("Unsafe filename".into());
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    if matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || (stem.len() == 4
            && matches!(stem.get(..3), Some("com" | "lpt"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
    {
        return Err("Reserved filename".into());
    }
    Ok(())
}

/// Validate a manifest path without ever allowing it to escape the receive
/// root.  `name` is the display basename; `relative_path` is used only for
/// folder transfers and must remain a relative, normalized path.
pub fn validate_file_meta(file: &FileMeta) -> Result<(), String> {
    validate_filename(&file.name)?;
    if let Some(path) = &file.relative_path {
        validate_relative_path(path)?;
        let components = path.split('/').collect::<Vec<_>>();
        if components.last().copied() != Some(file.name.as_str()) {
            return Err("Relative path does not end with the file name".into());
        }
    }
    Ok(())
}

pub fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.len() > 4096 || path.starts_with('/') || path.starts_with('\\') {
        return Err("Unsafe relative path".into());
    }
    if path.contains('\0') || path.contains(':') || path.contains('\\') {
        return Err("Unsafe relative path".into());
    }
    if path
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("Unsafe relative path".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_path_traversal() {
        assert!(validate_filename("../secret").is_err());
        assert!(validate_filename("C:\\Windows\\x").is_err());
    }
    #[test]
    fn round_trips_unicode_chat() {
        let msg = Message::Chat {
            id: Uuid::new_v4(),
            text: "नमस्ते 👋".into(),
            sent_at: 1,
        };
        let bytes = encode(&msg).unwrap();
        let out: Message = serde_json::from_slice(&bytes).unwrap();
        assert!(matches!(out, Message::Chat { text, .. } if text == "नमस्ते 👋"));
    }
    #[test]
    fn rejects_bad_manifest_and_version() {
        let msg = Message::TransferRequest {
            id: Uuid::new_v4(),
            files: vec![FileMeta {
                id: Uuid::new_v4(),
                name: "x".into(),
                relative_path: None,
                size: 4,
            }],
            directories: Vec::new(),
            total_bytes: 3,
        };
        assert!(validate(&msg).is_err());
        assert!(validate(&Message::HelloAck {
            protocol_version: 99,
            device_id: Uuid::new_v4(),
            public_key: vec![0; 32],
            signature: vec![0; 64],
        })
        .is_err());
    }
    #[test]
    fn rejects_absolute_and_windows_paths() {
        for name in [
            "../../secret.txt",
            "/Users/test.txt",
            "C:\\Windows\\test.txt",
            "..\\..\\bad.txt",
        ] {
            assert!(validate_filename(name).is_err(), "accepted {name}");
        }
        assert!(validate_relative_path("Project/empty").is_ok());
        assert!(validate_relative_path("Project/../outside").is_err());
    }

    #[test]
    fn validates_folder_relative_paths() {
        let valid = FileMeta {
            id: Uuid::new_v4(),
            name: "report.pdf".into(),
            relative_path: Some("Project/docs/report.pdf".into()),
            size: 1,
        };
        assert!(validate_file_meta(&valid).is_ok());
        for path in [
            "../secret.txt",
            "/tmp/secret.txt",
            "C:\\Windows\\x",
            "Project/../secret.txt",
            "Project/docs/other.txt",
        ] {
            let file = FileMeta {
                id: Uuid::new_v4(),
                name: "report.pdf".into(),
                relative_path: Some(path.into()),
                size: 1,
            };
            assert!(validate_file_meta(&file).is_err(), "accepted {path}");
        }
    }

    #[test]
    fn rejects_invalid_file_hash_and_chunk_overflow() {
        let file = FileMeta {
            id: Uuid::new_v4(),
            name: "data.bin".into(),
            relative_path: None,
            size: 1,
        };
        assert!(validate(&Message::FileStart {
            transfer_id: Uuid::new_v4(),
            file: file.clone(),
            hash: "not-a-sha".into(),
        })
        .is_err());
        assert!(validate(&Message::FileChunk {
            transfer_id: Uuid::new_v4(),
            file_id: file.id,
            offset: u64::MAX,
            data: vec![1],
        })
        .is_err());
    }

    #[test]
    fn rejects_windows_reserved_and_ambiguous_names() {
        for name in [
            "CON.txt", "nul", "COM1.log", "LPT9", "file.", "file ", "bad?.txt",
        ] {
            assert!(validate_filename(name).is_err(), "accepted {name}");
        }
    }

    #[test]
    fn rejects_duplicate_manifest_entries() {
        let id = Uuid::new_v4();
        let files = vec![
            FileMeta {
                id,
                name: "a.txt".into(),
                relative_path: None,
                size: 1,
            },
            FileMeta {
                id,
                name: "b.txt".into(),
                relative_path: None,
                size: 1,
            },
        ];
        assert!(validate(&Message::TransferRequest {
            id: Uuid::new_v4(),
            files,
            directories: Vec::new(),
            total_bytes: 2,
        })
        .is_err());
    }
}

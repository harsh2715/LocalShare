use crate::{CoreEvent, LocalShareCore, TransferEvent};
use anyhow::{bail, Result};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use uuid::Uuid;

const CHUNK_SIZE: usize = 64 * 1024;

#[derive(Clone)]
pub struct OutgoingTransfer {
    pub peer_id: String,
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

pub struct IncomingOffer {
    pub peer_id: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

pub struct IncomingTransfer {
    pub peer_id: String,
    pub name: String,
    pub size: u64,
    pub expected_sha256: String,
    pub bytes: u64,
    pub temp_path: PathBuf,
    pub final_path: PathBuf,
    pub file: tokio::fs::File,
    pub hasher: Sha256,
}

fn modified_millis(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

pub async fn offer(core: &LocalShareCore, peer_id: &str, path: &Path) -> Result<String> {
    if !core.is_peer_trusted(peer_id)? {
        bail!("peer is not paired")
    }
    let metadata = tokio::fs::metadata(path).await?;
    if !metadata.is_file() {
        bail!("source is not a regular file")
    }
    let sha256 = hash_file(path).await?;
    let source_modified = modified_millis(&metadata);
    let transfer_id = Uuid::new_v4().to_string();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("source filename is invalid"))?
        .to_owned();
    core.outgoing_transfers.lock().await.insert(
        transfer_id.clone(),
        OutgoingTransfer {
            peer_id: peer_id.into(),
            path: path.into(),
            name: name.clone(),
            size: metadata.len(),
            sha256: sha256.clone(),
        },
    );
    core.store().record_transfer_offer(
        &transfer_id,
        peer_id,
        &name,
        metadata.len(),
        &sha256,
        "send",
    )?;
    core.store()
        .set_transfer_source(&transfer_id, path, metadata.len(), source_modified)?;
    core.send_frame(
        peer_id,
        crate::protocol::frame(
            "file.offer",
            serde_json::json!({"transferId":transfer_id,"name":name,"size":metadata.len(),"sha256":sha256}),
        ),
    )
    .await?;
    core.emit(CoreEvent::Transfer(TransferEvent {
        id: transfer_id.clone(),
        peer_id: peer_id.into(),
        name,
        size: metadata.len(),
        bytes: 0,
        state: "OFFERED".into(),
        verified: false,
        error: None,
        direction: "send".into(),
    }));
    Ok(transfer_id)
}

pub async fn receive_offer(
    core: &LocalShareCore,
    peer_id: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    let transfer_id = payload["transferId"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("file offer missing transferId"))?;
    let name = safe_name(payload["name"].as_str().unwrap_or_default())?;
    let size = payload["size"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("file offer missing size"))?;
    let sha256 = payload["sha256"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("file offer missing sha256"))?;
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("invalid file hash")
    }
    core.store()
        .record_transfer_offer(transfer_id, peer_id, &name, size, sha256, "receive")?;
    for path in core.store().completed_candidates(sha256, size)? {
        let candidate = PathBuf::from(&path);
        if tokio::fs::try_exists(&candidate).await?
            && tokio::fs::metadata(&candidate).await?.len() == size
            && hash_file(&candidate).await? == sha256
        {
            core.store()
                .cancel_transfer(transfer_id, "exact duplicate already exists")?;
            core.send_frame(
                peer_id,
                crate::protocol::frame(
                    "file.cancel",
                    serde_json::json!({"transferId": transfer_id, "reason": "exact duplicate already exists"}),
                ),
            )
            .await?;
            core.emit(CoreEvent::Transfer(TransferEvent {
                id: transfer_id.into(),
                peer_id: peer_id.into(),
                name,
                size,
                bytes: size,
                state: "CANCELLED".into(),
                verified: true,
                error: Some("exact duplicate already exists".into()),
                direction: "receive".into(),
            }));
            return Ok(());
        }
    }

    if let Some(stored) = core.store().transfer(transfer_id)? {
        if stored.direction == "receive" && stored.state == "ACTIVE" {
            if let (Some(temp), Some(final_path)) =
                (stored.temp_path.as_deref(), stored.local_path.as_deref())
            {
                let temp_path = PathBuf::from(temp);
                if tokio::fs::try_exists(&temp_path).await? {
                    let metadata = tokio::fs::metadata(&temp_path).await?;
                    if metadata.len() <= size {
                        let bytes = metadata.len();
                        let mut hasher = Sha256::new();
                        let mut existing = tokio::fs::File::open(&temp_path).await?;
                        let mut buf = vec![0_u8; CHUNK_SIZE];
                        loop {
                            let n = existing.read(&mut buf).await?;
                            if n == 0 {
                                break;
                            }
                            hasher.update(&buf[..n]);
                        }
                        let file = tokio::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&temp_path)
                            .await?;
                        core.active_incoming.lock().await.insert(
                            transfer_id.to_owned(),
                            IncomingTransfer {
                                peer_id: peer_id.to_owned(),
                                name: name.clone(),
                                size,
                                expected_sha256: sha256.to_owned(),
                                bytes,
                                temp_path: temp_path.clone(),
                                final_path: PathBuf::from(final_path),
                                file,
                                hasher,
                            },
                        );
                        core.store().update_transfer_progress(transfer_id, bytes)?;
                        core.send_frame(
                            peer_id,
                            crate::protocol::frame(
                                "file.accept",
                                serde_json::json!({"transferId": transfer_id, "offset": bytes}),
                            ),
                        )
                        .await?;
                        return Ok(());
                    }
                }
            }
        }
    }
    core.pending_incoming.lock().await.insert(
        transfer_id.into(),
        IncomingOffer {
            peer_id: peer_id.into(),
            name: name.clone(),
            size,
            sha256: sha256.into(),
        },
    );
    core.emit(CoreEvent::Transfer(TransferEvent {
        id: transfer_id.into(),
        peer_id: peer_id.into(),
        name,
        size,
        bytes: 0,
        state: "OFFERED".into(),
        verified: false,
        error: None,
        direction: "receive".into(),
    }));
    Ok(())
}

pub async fn accept(core: &LocalShareCore, transfer_id: &str) -> Result<()> {
    let offer = core
        .pending_incoming
        .lock()
        .await
        .remove(transfer_id)
        .ok_or_else(|| anyhow::anyhow!("unknown transfer offer"))?;
    let final_path = unique_destination(&core.config().download_dir, &offer.name).await?;
    let saved_name = final_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("destination filename is invalid"))?
        .to_owned();
    let temp_path = core.config().temp_dir.join(format!("{transfer_id}.part"));
    let file = tokio::fs::File::create(&temp_path).await?;
    core.active_incoming.lock().await.insert(
        transfer_id.into(),
        IncomingTransfer {
            peer_id: offer.peer_id.clone(),
            name: offer.name.clone(),
            size: offer.size,
            expected_sha256: offer.sha256.clone(),
            bytes: 0,
            temp_path: temp_path.clone(),
            final_path: final_path.clone(),
            file,
            hasher: Sha256::new(),
        },
    );
    core.store()
        .set_transfer_destination(transfer_id, &saved_name, &final_path, &temp_path)?;
    core.send_frame(
        &offer.peer_id,
        crate::protocol::frame("file.accept", serde_json::json!({"transferId":transfer_id})),
    )
    .await?;
    Ok(())
}

pub async fn decline(core: &LocalShareCore, transfer_id: &str) -> Result<()> {
    let offer = core
        .pending_incoming
        .lock()
        .await
        .remove(transfer_id)
        .ok_or_else(|| anyhow::anyhow!("unknown transfer offer"))?;
    core.store()
        .cancel_transfer(transfer_id, "peer declined file")?;
    core.send_frame(
        &offer.peer_id,
        crate::protocol::frame(
            "file.cancel",
            serde_json::json!({"transferId": transfer_id, "reason": "peer declined file"}),
        ),
    )
    .await?;
    core.emit(CoreEvent::Transfer(TransferEvent {
        id: transfer_id.into(),
        peer_id: offer.peer_id,
        name: offer.name,
        size: offer.size,
        bytes: 0,
        state: "CANCELLED".into(),
        verified: false,
        error: Some("peer declined file".into()),
        direction: "receive".into(),
    }));
    Ok(())
}

pub async fn send_chunks(
    core: std::sync::Arc<LocalShareCore>,
    transfer_id: &str,
    start_offset: u64,
) -> Result<()> {
    let transfer = core
        .outgoing_transfers
        .lock()
        .await
        .remove(transfer_id)
        .ok_or_else(|| anyhow::anyhow!("unknown outgoing transfer"))?;
    let result = send_chunks_inner(&core, transfer_id, &transfer, start_offset).await;
    if result.is_err() {
        core.outgoing_transfers
            .lock()
            .await
            .insert(transfer_id.to_owned(), transfer);
    }
    result
}

async fn send_chunks_inner(
    core: &std::sync::Arc<LocalShareCore>,
    transfer_id: &str,
    transfer: &OutgoingTransfer,
    start_offset: u64,
) -> Result<()> {
    let mut file = tokio::fs::File::open(&transfer.path).await?;
    let metadata = file.metadata().await?;
    let source_modified = modified_millis(&metadata);
    let stored = core
        .store()
        .transfer(transfer_id)?
        .ok_or_else(|| anyhow::anyhow!("transfer metadata missing"))?;
    if metadata.len() != transfer.size
        || stored.source_size != Some(transfer.size)
        || stored.source_modified != Some(source_modified)
    {
        core.store()
            .fail_transfer(transfer_id, "source file changed")?;
        bail!("source file changed")
    }
    if start_offset > transfer.size {
        bail!("invalid resume offset")
    }
    if start_offset > 0 {
        file.seek(std::io::SeekFrom::Start(start_offset)).await?;
    }
    let mut buf = vec![0_u8; CHUNK_SIZE];
    let mut offset = start_offset;
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        core.send_frame(&transfer.peer_id, crate::protocol::frame("file.chunk", serde_json::json!({
            "transferId":transfer_id,"offset":offset,"data":base64::engine::general_purpose::STANDARD.encode(&buf[..n])
        }))).await?;
        offset += n as u64;
        core.store().update_transfer_progress(transfer_id, offset)?;
    }
    core.send_frame(
        &transfer.peer_id,
        crate::protocol::frame(
            "file.complete",
            serde_json::json!({"transferId":transfer_id}),
        ),
    )
    .await?;
    core.store().update_transfer_progress(transfer_id, offset)?;
    Ok(())
}

pub async fn resume_outgoing(core: std::sync::Arc<LocalShareCore>, peer_id: &str) -> Result<()> {
    for stored in core.store().incomplete_outgoing(peer_id)? {
        let Some(source_path) = stored.source_path.as_deref() else {
            continue;
        };
        let source_path = PathBuf::from(source_path);
        let metadata = match tokio::fs::metadata(&source_path).await {
            Ok(metadata) => metadata,
            Err(_) => {
                core.store()
                    .fail_transfer(&stored.id, "source unavailable")?;
                continue;
            }
        };
        if !metadata.is_file()
            || metadata.len() != stored.size
            || stored.source_modified != Some(modified_millis(&metadata))
        {
            core.store()
                .fail_transfer(&stored.id, "source file changed")?;
            continue;
        }
        core.outgoing_transfers.lock().await.insert(
            stored.id.clone(),
            OutgoingTransfer {
                peer_id: peer_id.to_owned(),
                path: source_path,
                name: stored.name.clone(),
                size: stored.size,
                sha256: stored.sha256.clone(),
            },
        );
        core.send_frame(
            peer_id,
            crate::protocol::frame(
                "file.offer",
                serde_json::json!({"transferId": stored.id, "name": stored.name, "size": stored.size, "sha256": stored.sha256}),
            ),
        )
        .await?;
    }
    Ok(())
}

pub async fn receive_chunk(core: &LocalShareCore, payload: &serde_json::Value) -> Result<()> {
    let transfer_id = payload["transferId"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("chunk missing transferId"))?;
    let offset = payload["offset"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("chunk missing offset"))?;
    let data = base64::engine::general_purpose::STANDARD
        .decode(payload["data"].as_str().unwrap_or_default())?;
    let mut active = core.active_incoming.lock().await;
    let transfer = active
        .get_mut(transfer_id)
        .ok_or_else(|| anyhow::anyhow!("unknown incoming transfer"))?;
    if offset != transfer.bytes || transfer.bytes + data.len() as u64 > transfer.size {
        bail!("invalid chunk offset")
    }
    transfer.file.write_all(&data).await?;
    transfer.hasher.update(&data);
    transfer.bytes += data.len() as u64;
    core.store()
        .update_transfer_progress(transfer_id, transfer.bytes)?;
    core.emit(CoreEvent::Transfer(TransferEvent {
        id: transfer_id.into(),
        peer_id: transfer.peer_id.clone(),
        name: transfer.name.clone(),
        size: transfer.size,
        bytes: transfer.bytes,
        state: "RECEIVING".into(),
        verified: false,
        error: None,
        direction: "receive".into(),
    }));
    Ok(())
}

pub async fn complete(core: &LocalShareCore, transfer_id: &str) -> Result<()> {
    let mut transfer = core
        .active_incoming
        .lock()
        .await
        .remove(transfer_id)
        .ok_or_else(|| anyhow::anyhow!("unknown incoming transfer"))?;
    transfer.file.flush().await?;
    let actual = hex::encode(transfer.hasher.finalize());
    if transfer.bytes != transfer.size || actual != transfer.expected_sha256 {
        core.store()
            .fail_transfer(transfer_id, "file verification failed")?;
        bail!("file verification failed")
    }
    tokio::fs::rename(&transfer.temp_path, &transfer.final_path).await?;
    core.store()
        .complete_transfer(transfer_id, transfer.bytes, &actual)?;
    core.emit(CoreEvent::Transfer(TransferEvent {
        id: transfer_id.into(),
        peer_id: transfer.peer_id,
        name: transfer.name,
        size: transfer.size,
        bytes: transfer.bytes,
        state: "COMPLETE".into(),
        verified: true,
        error: None,
        direction: "receive".into(),
    }));
    Ok(())
}

fn safe_name(name: &str) -> Result<String> {
    let path = Path::new(name);
    if name.is_empty()
        || path.file_name().and_then(|v| v.to_str()) != Some(name)
        || name.contains('\0')
    {
        bail!("unsafe file name")
    }
    Ok(name.into())
}

async fn unique_destination(root: &Path, name: &str) -> Result<PathBuf> {
    let base = root.join(name);
    if tokio::fs::try_exists(&base).await? {
        let stem = Path::new(name)
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("file");
        let ext = Path::new(name)
            .extension()
            .and_then(|v| v.to_str())
            .map(|v| format!(".{v}"))
            .unwrap_or_default();
        for index in 1..10000 {
            let candidate = root.join(format!("{stem} ({index}){ext}"));
            if !tokio::fs::try_exists(&candidate).await? {
                return Ok(candidate);
            }
        }
        bail!("unable to choose destination")
    }
    Ok(base)
}

pub async fn hash_file(path: &Path) -> Result<String> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut buf = vec![0_u8; CHUNK_SIZE];
    let mut hash = Sha256::new();
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hex::encode(hash.finalize()))
}

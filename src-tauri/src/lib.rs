#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod identity;
pub mod protocol;
mod storage;

use ed25519_dalek::pkcs8::EncodePrivateKey;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use identity::Identity;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use protocol::{FileMeta, Message, MAX_FRAME_BYTES, PROTOCOL_VERSION};
use rand::{rngs::OsRng, RngCore};
use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    time::{sleep, timeout, Duration},
};
use tokio_rustls::{
    rustls::{
        self,
        client::danger::{ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
        ClientConfig, DigitallySignedStruct, ServerConfig,
    },
    TlsAcceptor, TlsConnector,
};
use uuid::Uuid;
use x509_parser::prelude::parse_x509_certificate;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const FRAME_WRITE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Serialize)]
struct Peer {
    id: String,
    name: String,
    address: String,
    fingerprint: String,
    state: String,
    platform: String,
    trusted: bool,
}
#[derive(Clone, Serialize)]
struct LocalInfo {
    version: String,
    device_id: String,
    device_name: String,
    platform: String,
    architecture: String,
    fingerprint: String,
    port: u16,
    addresses: Vec<String>,
    mdns: String,
    mdns_advertise: String,
    mdns_browse: String,
    listener_state: String,
    tls_state: String,
    database_state: String,
    receive_directory: String,
    updater: String,
    transport: String,
}
#[derive(Clone, Serialize)]
struct ChatEvent {
    peer_id: String,
    id: String,
    text: String,
    sent_at: i64,
    direction: String,
}
#[derive(Clone, Serialize)]
struct TransferEvent {
    peer_id: String,
    id: String,
    name: String,
    size: u64,
    transferred: u64,
    state: String,
    direction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    directories: Option<Vec<String>>,
}
#[derive(Clone, Serialize)]
struct ChatRecord {
    id: String,
    peer_id: String,
    direction: String,
    content: String,
    sent_at: i64,
    state: String,
}
#[derive(Clone, Serialize)]
struct HistoryRecord {
    id: String,
    peer_id: String,
    name: String,
    size: u64,
    direction: String,
    result: String,
    path: Option<String>,
    created_at: i64,
}
#[derive(Clone, Serialize)]
struct SettingsInfo {
    device_name: String,
    download_location: String,
    ask_before_receive: bool,
    notifications: bool,
    check_for_updates: bool,
}

struct Session {
    control_tx: mpsc::Sender<Message>,
    data_tx: mpsc::Sender<Message>,
}
struct AppState {
    identity: Identity,
    device_name: Mutex<String>,
    db: Mutex<Connection>,
    peers: Mutex<HashMap<String, Peer>>,
    sessions: Mutex<HashMap<String, Session>>,
    transfers: Mutex<HashMap<Uuid, Arc<AtomicBool>>>,
    transfer_peers: Mutex<HashMap<Uuid, String>>,
    incoming_cancellations: Mutex<HashMap<Uuid, Arc<AtomicBool>>>,
    pending_incoming: Mutex<HashMap<Uuid, oneshot::Sender<bool>>>,
    pending_outgoing: Mutex<HashMap<Uuid, oneshot::Sender<bool>>>,
    connecting: Mutex<HashMap<String, bool>>,
    reconnect_attempts: Mutex<HashMap<String, u8>>,
    mdns: Mutex<Option<ServiceDaemon>>,
    local: Mutex<LocalInfo>,
    tls_server: Arc<ServerConfig>,
    app: AppHandle,
}
struct IncomingFile {
    transfer_id: Uuid,
    file: FileMeta,
    expected_hash: String,
    part: PathBuf,
    final_path: PathBuf,
    out: tokio::fs::File,
    hasher: Sha256,
    received: u64,
}

struct IncomingTransferState {
    files: HashMap<Uuid, FileMeta>,
    received_files: HashSet<Uuid>,
    expected_bytes: u64,
    received_bytes: u64,
}

type ExpandedTransfer = (Vec<(PathBuf, FileMeta)>, Vec<String>);

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
fn emit<T: Serialize + Clone>(app: &AppHandle, name: &str, payload: T) {
    let _ = app.emit(name, payload);
}

fn update_local_status(state: &Arc<AppState>, update: impl FnOnce(&mut LocalInfo)) {
    update(&mut state.local.lock().unwrap());
}

fn tls_configs(identity: &Identity) -> Result<(Arc<ServerConfig>, Arc<ClientConfig>), String> {
    let private_der = identity
        .signing_key
        .to_pkcs8_der()
        .map_err(|e| e.to_string())?;
    let private_key = PrivatePkcs8KeyDer::from(private_der.as_bytes());
    let key_pair = rcgen::KeyPair::try_from(&private_key).map_err(|e| e.to_string())?;
    let params = rcgen::CertificateParams::new(vec![identity.device_id.to_string()])
        .map_err(|e| e.to_string())?;
    let cert = params.self_signed(&key_pair).map_err(|e| e.to_string())?;
    let cert_der = CertificateDer::from(cert.der().to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der()));
    let server = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key)
        .map_err(|e| e.to_string())?;
    let client = ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(IdentityCertificateVerifier))
        .with_no_client_auth();
    Ok((Arc::new(server), Arc::new(client)))
}

fn install_crypto_provider() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

#[derive(Debug)]
struct IdentityCertificateVerifier;
impl ServerCertVerifier for IdentityCertificateVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        ocsp_response: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if !intermediates.is_empty() || !ocsp_response.is_empty() {
            return Err(rustls::Error::General(
                "LocalShare requires one self-signed identity certificate".into(),
            ));
        }
        let (_, certificate) = parse_x509_certificate(end_entity.as_ref()).map_err(|_| {
            rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding)
        })?;
        if !certificate.validity().is_valid() {
            return Err(rustls::Error::General(
                "LocalShare identity certificate is outside its validity window".into(),
            ));
        }
        if certificate.public_key().algorithm.algorithm.to_string() != "1.3.101.112"
            || certificate.public_key().subject_public_key.data.len() != 32
        {
            return Err(rustls::Error::General(
                "LocalShare identity certificate must use Ed25519".into(),
            ));
        }
        certificate.verify_signature(None).map_err(|_| {
            rustls::Error::General("LocalShare identity certificate is not self-signed".into())
        })?;
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![rustls::SignatureScheme::ED25519]
    }
}

async fn write_message<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    msg: &Message,
) -> Result<(), String> {
    let bytes = protocol::encode(msg).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err("Message too large".into());
    }
    writer
        .write_u32(bytes.len() as u32)
        .await
        .map_err(|e| e.to_string())?;
    writer.write_all(&bytes).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())
}
async fn read_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Message, String> {
    let len = reader.read_u32().await.map_err(|e| e.to_string())? as usize;
    if len == 0 || len > MAX_FRAME_BYTES {
        return Err("Invalid frame length".into());
    }
    let mut buf = vec![0; len];
    reader
        .read_exact(&mut buf)
        .await
        .map_err(|e| e.to_string())?;
    let msg: Message = serde_json::from_slice(&buf).map_err(|e| e.to_string())?;
    protocol::validate(&msg)?;
    Ok(msg)
}

async fn read_session_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Message, String> {
    timeout(SESSION_IDLE_TIMEOUT, read_message(reader))
        .await
        .map_err(|_| "Peer session idle timeout".to_string())?
}

async fn handle_connection(
    state: Arc<AppState>,
    stream: TcpStream,
    outgoing: bool,
    addr: SocketAddr,
) -> Result<(), String> {
    let (_, client) = tls_configs(&state.identity)?;
    if outgoing {
        let connector = TlsConnector::from(client);
        let tls = timeout(
            HANDSHAKE_TIMEOUT,
            connector.connect(
                ServerName::try_from("localshare").map_err(|e| e.to_string())?,
                stream,
            ),
        )
        .await
        .map_err(|_| "TLS handshake timed out".to_string())?
        .map_err(|e| e.to_string())?;
        handle_tls_stream(state, tls, addr).await
    } else {
        let tls = timeout(
            HANDSHAKE_TIMEOUT,
            TlsAcceptor::from(state.tls_server.clone()).accept(stream),
        )
        .await
        .map_err(|_| "TLS handshake timed out".to_string())?
        .map_err(|e| e.to_string())?;
        handle_tls_stream(state, tls, addr).await
    }
}

async fn handle_tls_stream<S>(state: Arc<AppState>, tls: S, addr: SocketAddr) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut reader, mut writer) = tokio::io::split(tls);
    let device_name = state.device_name.lock().unwrap().clone();
    let local_port = state.local.lock().unwrap().port;
    let local_nonce = {
        let mut nonce = vec![0u8; protocol::MAX_NONCE_BYTES];
        OsRng.fill_bytes(&mut nonce);
        nonce
    };
    let capabilities = vec!["chat".into(), "files".into(), "multi_file".into()];
    let local_public_key = state
        .identity
        .signing_key
        .verifying_key()
        .to_bytes()
        .to_vec();
    let local_hello_payload = protocol::hello_signing_bytes(
        PROTOCOL_VERSION,
        state.identity.device_id,
        &device_name,
        &state.identity.fingerprint,
        &local_public_key,
        local_port,
        &local_nonce,
        &capabilities,
    );
    let local_hello = Message::Hello {
        protocol_version: PROTOCOL_VERSION,
        device_id: state.identity.device_id,
        device_name,
        fingerprint: state.identity.fingerprint.clone(),
        public_key: local_public_key.clone(),
        listening_port: local_port,
        nonce: local_nonce.clone(),
        signature: identity::sign(&state.identity.signing_key, &local_hello_payload),
        capabilities,
    };
    timeout(HANDSHAKE_TIMEOUT, write_message(&mut writer, &local_hello))
        .await
        .map_err(|_| "Handshake timed out".to_string())??;
    let hello = timeout(HANDSHAKE_TIMEOUT, read_message(&mut reader))
        .await
        .map_err(|_| "Handshake timed out".to_string())??;
    let (
        peer_id_uuid,
        peer_name,
        fingerprint,
        peer_public_key,
        peer_nonce,
        peer_port,
        peer_verifying_key,
    ) = match hello {
        Message::Hello {
            device_id,
            device_name,
            fingerprint,
            public_key,
            listening_port,
            nonce,
            signature,
            capabilities,
            protocol_version,
        } => {
            let payload = protocol::hello_signing_bytes(
                protocol_version,
                device_id,
                &device_name,
                &fingerprint,
                &public_key,
                listening_port,
                &nonce,
                &capabilities,
            );
            let key_bytes: [u8; 32] = public_key
                .as_slice()
                .try_into()
                .map_err(|_| "Invalid peer public key".to_string())?;
            let verifying_key = VerifyingKey::from_bytes(&key_bytes)
                .map_err(|_| "Invalid peer public key".to_string())?;
            let signature = Signature::from_slice(&signature)
                .map_err(|_| "Invalid peer handshake signature".to_string())?;
            verifying_key
                .verify(&payload, &signature)
                .map_err(|_| "Peer handshake signature verification failed".to_string())?;
            (
                device_id,
                device_name,
                fingerprint,
                public_key,
                nonce,
                listening_port,
                verifying_key,
            )
        }
        _ => return Err("Expected HELLO".into()),
    };
    let peer_id = peer_id_uuid.to_string();
    let ack_payload = protocol::ack_signing_bytes(
        PROTOCOL_VERSION,
        peer_id_uuid,
        &peer_nonce,
        state.identity.device_id,
        &local_nonce,
    );
    timeout(
        HANDSHAKE_TIMEOUT,
        write_message(
            &mut writer,
            &Message::HelloAck {
                protocol_version: PROTOCOL_VERSION,
                device_id: state.identity.device_id,
                public_key: local_public_key.clone(),
                signature: identity::sign(&state.identity.signing_key, &ack_payload),
            },
        ),
    )
    .await
    .map_err(|_| "Handshake timed out".to_string())??;
    let peer_ack = timeout(HANDSHAKE_TIMEOUT, read_message(&mut reader))
        .await
        .map_err(|_| "Handshake timed out".to_string())??;
    match peer_ack {
        Message::HelloAck {
            protocol_version,
            device_id,
            public_key,
            signature,
        } if device_id == peer_id_uuid && public_key == peer_public_key => {
            let payload = protocol::ack_signing_bytes(
                protocol_version,
                state.identity.device_id,
                &local_nonce,
                peer_id_uuid,
                &peer_nonce,
            );
            let signature = Signature::from_slice(&signature)
                .map_err(|_| "Invalid peer acknowledgement signature".to_string())?;
            peer_verifying_key
                .verify(&payload, &signature)
                .map_err(|_| "Peer acknowledgement verification failed".to_string())?;
        }
        _ => return Err("Invalid handshake acknowledgement".into()),
    }
    let trusted = {
        let db = state.db.lock().unwrap();
        match storage::trusted_fingerprint(&db, &peer_id) {
            Some(expected) if expected != fingerprint => {
                return Err("Known peer identity changed; connection rejected".into());
            }
            Some(_) => true,
            None => false,
        }
    };
    let peer = Peer {
        id: peer_id.clone(),
        name: peer_name.clone(),
        address: SocketAddr::new(addr.ip(), peer_port).to_string(),
        fingerprint: fingerprint.clone(),
        state: if trusted {
            "connected"
        } else {
            "trust_required"
        }
        .into(),
        platform: "LAN peer".into(),
        trusted,
    };
    state.connecting.lock().unwrap().remove(&peer_id);
    state
        .peers
        .lock()
        .unwrap()
        .insert(peer_id.clone(), peer.clone());
    emit(&state.app, "peer-updated", &peer);
    if !trusted {
        return Ok(());
    }
    let (control_tx, mut control_rx) = mpsc::channel::<Message>(128);
    let (data_tx, mut data_rx) = mpsc::channel::<Message>(16);
    {
        let mut sessions = state.sessions.lock().unwrap();
        if sessions.contains_key(&peer_id) {
            return Err("Duplicate peer session rejected".into());
        }
        sessions.insert(
            peer_id.clone(),
            Session {
                control_tx: control_tx.clone(),
                data_tx: data_tx.clone(),
            },
        );
    }
    state.reconnect_attempts.lock().unwrap().remove(&peer_id);
    let writer_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                Some(msg) = control_rx.recv() => {
                    if timeout(FRAME_WRITE_TIMEOUT, write_message(&mut writer, &msg)).await.is_err() { break; }
                },
                Some(msg) = data_rx.recv() => {
                    if timeout(FRAME_WRITE_TIMEOUT, write_message(&mut writer, &msg)).await.is_err() { break; }
                },
                else => break
            }
        }
    });
    let mut incoming: Option<IncomingFile> = None;
    let mut accepted_transfers: HashMap<Uuid, IncomingTransferState> = HashMap::new();
    let mut cancelled_transfers = HashSet::new();
    let mut pending_incoming_ids = HashSet::new();
    let mut orphan_part: Option<PathBuf> = None;
    let result: Result<(), String> = async {
    loop {
        match read_session_message(&mut reader).await {
            Ok(Message::Ping) => {
                let _ = control_tx.send(Message::Pong).await;
            }
            Ok(Message::Pong) => {}
            Ok(Message::Chat { id, text, sent_at }) => {
                let _ = state.db.lock().unwrap().execute("INSERT OR REPLACE INTO chat_messages VALUES (?1,?2,'received',?3,?4,'delivered')", rusqlite::params![id.to_string(), peer_id, text, sent_at]);
                emit(
                    &state.app,
                    "chat-message",
                    ChatEvent {
                        peer_id: peer_id.clone(),
                        id: id.to_string(),
                        text,
                        sent_at,
                        direction: "received".into(),
                    },
                );
            }
            Ok(Message::TransferRequest {
                id,
                files,
                directories,
                total_bytes,
            }) => {
                if accepted_transfers.contains_key(&id) || incoming.is_some() {
                    control_tx
                        .send(Message::TransferReject {
                            id,
                            reason: "Another transfer is already active".into(),
                        })
                        .await
                        .map_err(|e| e.to_string())?;
                    continue;
                }
                emit(
                    &state.app,
                    "transfer-request",
                    TransferEvent {
                        peer_id: peer_id.clone(),
                        id: id.to_string(),
                        name: format!("{} file(s)", files.len()),
                        size: total_bytes,
                        transferred: 0,
                        state: "pending".into(),
                        direction: "received".into(),
                        files: Some(files.iter().map(|file| file.name.clone()).collect()),
                        directories: Some(directories.clone()),
                    },
                );
                let ask = storage::setting(&state.db.lock().unwrap(), "ask_before_receive")
                    .map(|v| v != "false")
                    .unwrap_or(true);
                let accepted = if ask {
                    let (tx, rx) = oneshot::channel();
                    state.pending_incoming.lock().unwrap().insert(id, tx);
                    pending_incoming_ids.insert(id);
                    let approval = async {
                        let mut rx = rx;
                        loop {
                            tokio::select! {
                                answer = &mut rx => break answer.unwrap_or(false),
                                message = read_session_message(&mut reader) => match message {
                                    Ok(Message::Ping) => {
                                        let _ = control_tx.send(Message::Pong).await;
                                    }
                                    Ok(Message::TransferCancel { id: cancel_id }) if cancel_id == id => {
                                        cancelled_transfers.insert(id);
                                        break false;
                                    }
                                    Ok(_) | Err(_) => break false,
                                }
                            }
                        }
                    };
                    timeout(SESSION_IDLE_TIMEOUT, approval).await.unwrap_or(false)
                } else {
                    true
                };
                state.pending_incoming.lock().unwrap().remove(&id);
                pending_incoming_ids.remove(&id);
                if accepted {
                    let dir = download_dir(&state)?;
                    for directory in directories {
                        protocol::validate_relative_path(&directory)?;
                        fs::create_dir_all(safe_receive_path(&dir, &directory)?)
                            .map_err(|e| e.to_string())?;
                    }
                    accepted_transfers.insert(
                        id,
                        IncomingTransferState {
                            files: files.iter().map(|file| (file.id, file.clone())).collect(),
                            received_files: HashSet::new(),
                            expected_bytes: total_bytes,
                            received_bytes: 0,
                        },
                    );
                } else {
                    record_transfer_history(
                        &state,
                        &peer_id,
                        &format!("{} file(s)", files.len()),
                        total_bytes,
                        "received",
                        "rejected",
                        None,
                    );
                }
                let _ = control_tx
                    .send(if accepted {
                        Message::TransferAccept { id }
                    } else {
                        Message::TransferReject {
                            id,
                            reason: "Receiver declined the transfer".into(),
                        }
                    })
                    .await;
            }
            Ok(Message::TransferAccept { id }) => {
                if let Some(waiter) = state.pending_outgoing.lock().unwrap().remove(&id) {
                    let _ = waiter.send(true);
                }
            }
            Ok(Message::TransferReject { id, .. }) => {
                if let Some(waiter) = state.pending_outgoing.lock().unwrap().remove(&id) {
                    let _ = waiter.send(false);
                }
            }
            Ok(Message::FileStart {
                transfer_id,
                file,
                hash,
            }) => {
                if cancelled_transfers.contains(&transfer_id) {
                    continue;
                }
                if !accepted_transfers.contains_key(&transfer_id) || incoming.is_some() {
                    return Err("File start is not allowed for this transfer".into());
                }
                let dir = download_dir(&state)?;
                protocol::validate_file_meta(&file)?;
                let expected_file = accepted_transfers
                    .get(&transfer_id)
                    .and_then(|transfer| transfer.files.get(&file.id))
                    .cloned()
                    .ok_or("File is not present in the accepted manifest")?;
                if expected_file != file
                    || accepted_transfers
                        .get(&transfer_id)
                        .is_some_and(|transfer| transfer.received_files.contains(&file.id))
                {
                    return Err("File metadata does not match the accepted manifest".into());
                }
                let relative = file
                    .relative_path
                    .clone()
                    .unwrap_or_else(|| file.name.clone());
                let final_path = safe_receive_path(&dir, &relative)?;
                let final_path = unique_path_for_relative(&dir, &final_path)?;
                if let Some(parent) = final_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let part = final_path.with_file_name(format!(
                    "{}.localshare-part",
                    final_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or("Invalid receive filename")?
                ));
                let out = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&part)
                    .await
                    .map_err(|e| e.to_string())?;
                state
                    .incoming_cancellations
                    .lock()
                    .unwrap()
                    .insert(transfer_id, Arc::new(AtomicBool::new(false)));
                incoming = Some(IncomingFile {
                    transfer_id,
                    file,
                    expected_hash: hash,
                    part,
                    final_path,
                    out,
                    hasher: Sha256::new(),
                    received: 0,
                });
            }
            Ok(Message::FileChunk {
                file_id,
                offset,
                data,
                transfer_id,
            }) => {
                if cancelled_transfers.contains(&transfer_id) {
                    continue;
                }
                if !accepted_transfers.contains_key(&transfer_id) {
                    return Err("Chunk is not associated with an accepted transfer".into());
                }
                let cancelled = state
                    .incoming_cancellations
                    .lock()
                    .unwrap()
                    .get(&transfer_id)
                    .map(|flag| flag.load(Ordering::Relaxed))
                    .unwrap_or(false);
                if cancelled {
                    if let Some(file) = incoming.take() {
                        let _ = tokio::fs::remove_file(file.part).await;
                    }
                    state
                        .incoming_cancellations
                        .lock()
                        .unwrap()
                        .remove(&transfer_id);
                    let _ = control_tx
                        .send(Message::TransferCancel { id: transfer_id })
                        .await;
                    accepted_transfers.remove(&transfer_id);
                    record_transfer_history(
                        &state,
                        &peer_id,
                        "Transfer",
                        0,
                        "received",
                        "cancelled",
                        None,
                    );
                    emit(
                        &state.app,
                        "transfer-error",
                        serde_json::json!({"peer_id": peer_id, "id": transfer_id.to_string(), "state": "cancelled", "message": "Transfer cancelled"}),
                    );
                    continue;
                }
                let file = incoming.as_mut().ok_or("Chunk arrived before file start")?;
                if file.transfer_id != transfer_id
                    || file.file.id != file_id
                    || file.received != offset
                    || file
                        .received
                        .saturating_add(data.len() as u64)
                        > file.file.size
                {
                    return Err("Invalid file chunk offset or size".into());
                }
                file.out.write_all(&data).await.map_err(|e| e.to_string())?;
                file.hasher.update(&data);
                file.received += data.len() as u64;
                emit(
                    &state.app,
                    "transfer-progress",
                    TransferEvent {
                        peer_id: peer_id.clone(),
                        id: transfer_id.to_string(),
                        name: file.file.name.clone(),
                        size: file.file.size,
                        transferred: file.received,
                        state: "transferring".into(),
                        direction: "received".into(),
                        files: None,
                        directories: None,
                    },
                );
            }
            Ok(Message::FileEnd {
                transfer_id,
                file_id,
            }) => {
                if cancelled_transfers.contains(&transfer_id) {
                    continue;
                }
                if !accepted_transfers.contains_key(&transfer_id) {
                    return Err("File end is not associated with an accepted transfer".into());
                }
                let mut file = incoming
                    .take()
                    .ok_or("File end arrived before file start")?;
                orphan_part = Some(file.part.clone());
                if file.transfer_id != transfer_id || file.file.id != file_id {
                    let _ = tokio::fs::remove_file(&file.part).await;
                    orphan_part = None;
                    accepted_transfers.remove(&transfer_id);
                    return Err("File end does not match the active file".into());
                }
                file.out.flush().await.map_err(|e| e.to_string())?;
                file.out.sync_all().await.map_err(|e| e.to_string())?;
                let valid = file.received == file.file.size
                    && hex::encode(file.hasher.finalize()) == file.expected_hash;
                if valid {
                    let history_name = file.file.name.clone();
                    let history_size = file.file.size;
                    let history_path = file.final_path.to_string_lossy().into_owned();
                    ensure_no_symlink_components(&file.final_path)?;
                    tokio::fs::hard_link(&file.part, &file.final_path)
                        .await
                        .map_err(|e| format!("Unable to commit received file safely: {e}"))?;
                    tokio::fs::remove_file(&file.part)
                        .await
                        .map_err(|e| e.to_string())?;
                    orphan_part = None;
                    state
                        .incoming_cancellations
                        .lock()
                        .unwrap()
                        .remove(&transfer_id);
                    record_transfer_history(
                        &state,
                        &peer_id,
                        &history_name,
                        history_size,
                        "received",
                        "completed",
                        Some(Path::new(&history_path)),
                    );
                    if let Some(transfer) = accepted_transfers.get_mut(&transfer_id) {
                        transfer.received_files.insert(file.file.id);
                        transfer.received_bytes = transfer
                            .received_bytes
                            .checked_add(file.received)
                            .ok_or("Received byte count overflow")?;
                    }
                } else {
                    state
                        .incoming_cancellations
                        .lock()
                        .unwrap()
                        .remove(&transfer_id);
                    let _ = tokio::fs::remove_file(&file.part).await;
                    orphan_part = None;
                    emit(
                        &state.app,
                        "transfer-error",
                        serde_json::json!({"peer_id": peer_id, "id": transfer_id.to_string(), "state": "failed", "message": "Transfer failed integrity verification"}),
                    );
                    record_transfer_history(
                        &state,
                        &peer_id,
                        &file.file.name,
                        file.file.size,
                        "received",
                        "failed",
                        None,
                    );
                    accepted_transfers.remove(&transfer_id);
                }
            }
            Ok(Message::TransferCancel { id }) => {
                cancelled_transfers.insert(id);
                if let Some(flag) = state.transfers.lock().unwrap().get(&id) {
                    flag.store(true, Ordering::Relaxed);
                }
                if let Some(file) = incoming.take() {
                    if file.transfer_id == id {
                        let _ = tokio::fs::remove_file(file.part).await;
                    }
                }
                state.incoming_cancellations.lock().unwrap().remove(&id);
                accepted_transfers.remove(&id);
                if let Some(waiter) = state.pending_outgoing.lock().unwrap().remove(&id) {
                    let _ = waiter.send(false);
                }
                emit(
                    &state.app,
                    "transfer-error",
                    serde_json::json!({"peer_id": peer_id, "id": id.to_string(), "state": "cancelled", "message": "Transfer cancelled by peer"}),
                );
            }
            Ok(Message::TransferComplete {
                id,
                name,
                size,
                transferred,
            }) => {
                if let Some(transfer) = accepted_transfers.remove(&id) {
                    if incoming.is_some()
                        || transfer.received_files.len() != transfer.files.len()
                        || transfer.received_bytes != transfer.expected_bytes
                        || size != transfer.expected_bytes
                        || transferred != transfer.expected_bytes
                    {
                        return Err("Transfer completed before its manifest was satisfied".into());
                    }
                    emit(
                        &state.app,
                        "transfer-complete",
                        TransferEvent {
                            peer_id: peer_id.clone(),
                            id: id.to_string(),
                            name: if name.is_empty() {
                                format!("{} file(s)", transfer.files.len())
                            } else {
                                name.clone()
                            },
                            size,
                            transferred,
                            state: "completed".into(),
                            direction: "received".into(),
                            files: None,
                            directories: None,
                        },
                    );
                    let _ = data_tx
                        .send(Message::TransferComplete {
                            id,
                            name,
                            size,
                            transferred,
                        })
                        .await;
                } else {
                    let is_outgoing = state
                        .transfer_peers
                        .lock()
                        .unwrap()
                        .get(&id)
                        .is_some_and(|owner| owner == &peer_id);
                    if !is_outgoing {
                        return Err("Transfer completion is not associated with a local transfer".into());
                    }
                    emit(
                        &state.app,
                        "transfer-complete",
                        TransferEvent {
                            peer_id: peer_id.clone(),
                            id: id.to_string(),
                            name,
                            size,
                            transferred,
                            state: "completed".into(),
                            direction: "sent".into(),
                            files: None,
                            directories: None,
                        },
                    );
                    state.transfer_peers.lock().unwrap().remove(&id);
                }
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(peer = %peer_id, error = %error, "peer session ended");
                break;
            }
        }
    }
    Ok(())
    }.await;
    if let Some(file) = incoming.take() {
        let _ = tokio::fs::remove_file(file.part).await;
        state
            .incoming_cancellations
            .lock()
            .unwrap()
            .remove(&file.transfer_id);
    }
    if let Some(part) = orphan_part {
        let _ = tokio::fs::remove_file(part).await;
    }
    for id in accepted_transfers.keys().copied().collect::<Vec<_>>() {
        let _ = state.incoming_cancellations.lock().unwrap().remove(&id);
    }
    accepted_transfers.clear();
    for id in pending_incoming_ids {
        if let Some(waiter) = state.pending_incoming.lock().unwrap().remove(&id) {
            let _ = waiter.send(false);
        }
    }
    let outgoing_ids = state
        .transfer_peers
        .lock()
        .unwrap()
        .iter()
        .filter_map(|(id, owner)| (owner == &peer_id).then_some(*id))
        .collect::<Vec<_>>();
    for id in outgoing_ids {
        if let Some(flag) = state.transfers.lock().unwrap().get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
        if let Some(waiter) = state.pending_outgoing.lock().unwrap().remove(&id) {
            let _ = waiter.send(false);
        }
    }
    state.sessions.lock().unwrap().remove(&peer_id);
    state.connecting.lock().unwrap().remove(&peer_id);
    if let Some(p) = state.peers.lock().unwrap().get_mut(&peer_id) {
        p.state = "offline".into();
        emit(&state.app, "peer-updated", p.clone());
    }
    let reconnect_address = state
        .peers
        .lock()
        .unwrap()
        .get(&peer_id)
        .filter(|peer| peer.trusted)
        .and_then(|peer| peer.address.parse::<SocketAddr>().ok());
    if state.identity.device_id.to_string() < peer_id {
        if let Some(address) = reconnect_address {
            schedule_reconnect(state.clone(), peer_id.clone(), address);
        }
    }
    writer_task.abort();
    result
}

fn safe_receive_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || relative.contains('\0')
        || relative.contains(':')
        || relative.contains('\\')
        || relative.starts_with('/')
        || relative.starts_with('\\')
    {
        return Err("Unsafe receive path".into());
    }
    let root = fs::canonicalize(root).map_err(|e| format!("Invalid receive root: {e}"))?;
    let candidate = root.join(path);
    if !candidate.starts_with(&root)
        || candidate
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("Unsafe receive path".into());
    }
    ensure_no_symlink_components(&candidate)?;
    Ok(candidate)
}

fn ensure_no_symlink_components(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                return Err("Receive path contains a symbolic link".into());
            }
            if metadata.is_file() && current != path {
                return Err("Receive path contains a file ancestor".into());
            }
        }
    }
    Ok(())
}

fn unique_path_for_relative(root: &Path, candidate: &Path) -> Result<PathBuf, String> {
    if !candidate.exists()
        && !candidate
            .with_file_name(format!(
                "{}.localshare-part",
                candidate
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
            ))
            .exists()
    {
        return Ok(candidate.to_path_buf());
    }
    let parent = candidate.parent().ok_or("Invalid receive path")?;
    if !parent.starts_with(root) {
        return Err("Receive path escaped destination".into());
    }
    let filename = candidate
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid receive filename")?;
    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(filename);
    let extension = path.extension().and_then(|value| value.to_str());
    for index in 1..=10_000 {
        let renamed = match extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let next = parent.join(renamed);
        let part = next.with_file_name(format!(
            "{}.localshare-part",
            next.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
        ));
        if !next.exists() && !part.exists() {
            return Ok(next);
        }
    }
    Err("Unable to choose a safe duplicate filename".into())
}

fn expanded_transfer_inputs(paths: &[String]) -> Result<ExpandedTransfer, String> {
    let mut expanded = Vec::new();
    let mut directories = Vec::new();
    for raw in paths {
        let path = PathBuf::from(raw);
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("Symbolic links are not supported in transfers".into());
        }
        if metadata.is_file() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("Invalid filename")?
                .to_string();
            protocol::validate_filename(&name)?;
            expanded.push((
                path,
                FileMeta {
                    id: Uuid::new_v4(),
                    name,
                    relative_path: None,
                    size: metadata.len(),
                },
            ));
        } else if metadata.is_dir() {
            let root = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("Invalid folder name")?
                .to_string();
            protocol::validate_filename(&root)?;
            directories.push(root.clone());
            let canonical_root = fs::canonicalize(&path).map_err(|e| e.to_string())?;
            let mut stack = vec![path.clone()];
            while let Some(current) = stack.pop() {
                for entry in fs::read_dir(&current).map_err(|e| e.to_string())? {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let child = entry.path();
                    let child_meta = fs::symlink_metadata(&child).map_err(|e| e.to_string())?;
                    if child_meta.file_type().is_symlink() {
                        return Err(format!(
                            "Folder contains unsupported symbolic link: {}",
                            child.display()
                        ));
                    }
                    if child_meta.is_dir() {
                        let relative = child
                            .strip_prefix(&path)
                            .map_err(|_| "Unable to enumerate folder")?;
                        let relative = Path::new(&root).join(relative);
                        let relative = relative.to_string_lossy().replace('\\', "/");
                        protocol::validate_relative_path(&relative)?;
                        directories.push(relative);
                        stack.push(child);
                        continue;
                    }
                    if !child_meta.is_file() {
                        continue;
                    }
                    let canonical_child = fs::canonicalize(&child).map_err(|e| e.to_string())?;
                    if !canonical_child.starts_with(&canonical_root) {
                        return Err("Folder contains a path outside its selected root".into());
                    }
                    let relative = child
                        .strip_prefix(&path)
                        .map_err(|_| "Unable to enumerate folder")?;
                    let relative = Path::new(&root).join(relative);
                    let relative = relative.to_string_lossy().replace('\\', "/");
                    let name = child
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or("Invalid filename")?
                        .to_string();
                    let file = FileMeta {
                        id: Uuid::new_v4(),
                        name,
                        relative_path: Some(relative),
                        size: child_meta.len(),
                    };
                    protocol::validate_file_meta(&file)?;
                    expanded.push((child, file));
                }
            }
        } else {
            return Err(format!("Unsupported transfer path: {}", path.display()));
        }
    }
    if expanded.is_empty() && directories.is_empty() {
        return Err("The selected folder contains no files".into());
    }
    directories.sort();
    directories.dedup();
    Ok((expanded, directories))
}

fn download_dir(state: &AppState) -> Result<PathBuf, String> {
    let configured = state
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT value FROM settings WHERE key='download_location'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();
    let dir = configured
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or(storage::default_download_dir()?);
    if !dir.is_absolute() {
        return Err("Receive directory must be an absolute path".into());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::canonicalize(&dir).map_err(|e| e.to_string())
}

fn cleanup_stale_partials(root: &Path) -> Result<usize, String> {
    fn visit(path: &Path, removed: &mut usize) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let child = entry.path();
            let metadata = fs::symlink_metadata(&child).map_err(|e| e.to_string())?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                visit(&child, removed)?;
            } else if metadata.is_file()
                && child
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".localshare-part"))
            {
                fs::remove_file(&child).map_err(|e| e.to_string())?;
                *removed += 1;
            }
        }
        Ok(())
    }

    let mut removed = 0;
    visit(root, &mut removed)?;
    Ok(removed)
}

fn record_transfer_history(
    state: &AppState,
    peer_id: &str,
    name: &str,
    size: u64,
    direction: &str,
    result: &str,
    path: Option<&Path>,
) {
    let path = path.map(|value| value.to_string_lossy().into_owned());
    let _ = state.db.lock().unwrap().execute(
        "INSERT INTO transfer_history(id,peer_id,name,size,direction,result,path,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![Uuid::new_v4().to_string(), peer_id, name, size, direction, result, path, now()],
    );
}

#[tauri::command]
fn local_info(state: State<'_, Arc<AppState>>) -> LocalInfo {
    let mut info = state.local.lock().unwrap().clone();
    info.device_name = state.device_name.lock().unwrap().clone();
    info.receive_directory = download_dir(state.inner())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "Unavailable".into());
    info.updater = if storage::setting(&state.db.lock().unwrap(), "check_for_updates")
        .map(|value| value != "false")
        .unwrap_or(true)
    {
        "enabled · Internet only".into()
    } else {
        "disabled".into()
    };
    info.database_state = "healthy".into();
    info
}
#[tauri::command]
fn list_peers(state: State<'_, Arc<AppState>>) -> Vec<Peer> {
    state.peers.lock().unwrap().values().cloned().collect()
}
#[tauri::command]
fn chat_history(
    state: State<'_, Arc<AppState>>,
    peer_id: Option<String>,
) -> Result<Vec<ChatRecord>, String> {
    let db = state.db.lock().unwrap();
    let mut stmt = db.prepare("SELECT id, peer_id, direction, content, sent_at, state FROM chat_messages WHERE (?1 IS NULL OR peer_id=?1) ORDER BY sent_at ASC").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![peer_id], |r| {
            Ok(ChatRecord {
                id: r.get(0)?,
                peer_id: r.get(1)?,
                direction: r.get(2)?,
                content: r.get(3)?,
                sent_at: r.get(4)?,
                state: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn transfer_history(state: State<'_, Arc<AppState>>) -> Result<Vec<HistoryRecord>, String> {
    let db = state.db.lock().unwrap();
    let mut stmt = db.prepare("SELECT id, peer_id, name, size, direction, result, path, created_at FROM transfer_history ORDER BY created_at DESC").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(HistoryRecord {
                id: r.get(0)?,
                peer_id: r.get(1)?,
                name: r.get(2)?,
                size: r.get(3)?,
                direction: r.get(4)?,
                result: r.get(5)?,
                path: r.get(6)?,
                created_at: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn clear_transfer_history(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state
        .db
        .lock()
        .unwrap()
        .execute("DELETE FROM transfer_history", [])
        .map(|_| ())
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn settings(state: State<'_, Arc<AppState>>) -> Result<SettingsInfo, String> {
    let db = state.db.lock().unwrap();
    let download = storage::setting(&db, "download_location").unwrap_or_else(|| {
        storage::default_download_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    Ok(SettingsInfo {
        device_name: storage::setting(&db, "device_name")
            .unwrap_or_else(|| state.device_name.lock().unwrap().clone()),
        download_location: download,
        ask_before_receive: storage::setting(&db, "ask_before_receive")
            .map(|v| v != "false")
            .unwrap_or(true),
        notifications: storage::setting(&db, "notifications")
            .map(|v| v != "false")
            .unwrap_or(true),
        check_for_updates: storage::setting(&db, "check_for_updates")
            .map(|v| v != "false")
            .unwrap_or(true),
    })
}
#[tauri::command]
fn save_settings(
    state: State<'_, Arc<AppState>>,
    device_name: String,
    download_location: String,
    ask_before_receive: bool,
    notifications: bool,
    check_for_updates: bool,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    if device_name.trim().is_empty()
        || device_name.chars().count() > 80
        || device_name.chars().any(char::is_control)
    {
        return Err("Device name must be 1–80 visible characters".into());
    }
    storage::set_setting(&db, "device_name", device_name.trim())?;
    *state.device_name.lock().unwrap() = device_name.trim().to_string();
    state.local.lock().unwrap().device_name = device_name.trim().to_string();
    let destination = if download_location.trim().is_empty() {
        storage::default_download_dir()?
            .to_string_lossy()
            .into_owned()
    } else {
        let path = PathBuf::from(download_location.trim());
        if !path.is_absolute() {
            return Err("Download location must be an absolute path".into());
        }
        path.to_string_lossy().into_owned()
    };
    fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
    let destination = fs::canonicalize(&destination)
        .map_err(|e| format!("Invalid download location: {e}"))?
        .to_string_lossy()
        .into_owned();
    state.local.lock().unwrap().receive_directory = destination.clone();
    storage::set_setting(&db, "download_location", &destination)?;
    storage::set_setting(
        &db,
        "ask_before_receive",
        if ask_before_receive { "true" } else { "false" },
    )?;
    storage::set_setting(
        &db,
        "notifications",
        if notifications { "true" } else { "false" },
    )?;
    storage::set_setting(
        &db,
        "check_for_updates",
        if check_for_updates { "true" } else { "false" },
    )?;
    Ok(())
}
#[tauri::command]
fn open_location(path: String) -> Result<(), String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err("Location does not exist".into());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_file(path: String) -> Result<(), String> {
    let target = PathBuf::from(path);
    if !target.is_file() {
        return Err("File does not exist".into());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    let target = PathBuf::from(path);
    let folder = if target.is_dir() {
        target
    } else {
        target
            .parent()
            .map(Path::to_path_buf)
            .ok_or("No containing folder")?
    };
    if !folder.exists() {
        return Err("Folder does not exist".into());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&folder)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&folder)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&folder)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn accept_transfer(state: State<'_, Arc<AppState>>, transfer_id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&transfer_id).map_err(|e| e.to_string())?;
    state
        .pending_incoming
        .lock()
        .unwrap()
        .remove(&id)
        .and_then(|sender| sender.send(true).ok())
        .ok_or("Transfer request is no longer pending".to_string())
}
#[tauri::command]
fn reject_transfer(state: State<'_, Arc<AppState>>, transfer_id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&transfer_id).map_err(|e| e.to_string())?;
    state
        .pending_incoming
        .lock()
        .unwrap()
        .remove(&id)
        .and_then(|sender| sender.send(false).ok())
        .ok_or("Transfer request is no longer pending".to_string())
}

#[tauri::command]
async fn trust_peer(state: State<'_, Arc<AppState>>, peer_id: String) -> Result<(), String> {
    let peer = state
        .peers
        .lock()
        .unwrap()
        .get(&peer_id)
        .cloned()
        .ok_or("Peer is no longer available")?;
    if peer.fingerprint.trim().is_empty() {
        return Err("Peer did not provide an identity fingerprint".into());
    }
    {
        let db = state.db.lock().unwrap();
        storage::trust_peer(&db, &peer.id, &peer.name, &peer.fingerprint, now())?;
    }
    let addr = peer
        .address
        .parse::<SocketAddr>()
        .map_err(|e| format!("Invalid peer address: {e}"))?;
    let updated = Peer {
        trusted: true,
        state: "connecting".into(),
        ..peer
    };
    state
        .peers
        .lock()
        .unwrap()
        .insert(peer_id.clone(), updated.clone());
    emit(&state.app, "peer-updated", updated);
    if state.sessions.lock().unwrap().contains_key(&peer_id) {
        return Ok(());
    }
    if state
        .connecting
        .lock()
        .unwrap()
        .insert(peer_id.clone(), true)
        .is_some()
    {
        return Ok(());
    }
    let connect_state = state.inner().clone();
    tokio::spawn(async move {
        let result = match timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => handle_connection(connect_state.clone(), stream, true, addr).await,
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("Connection timed out".into()),
        };
        if result.is_err() {
            connect_state.connecting.lock().unwrap().remove(&peer_id);
        }
    });
    Ok(())
}
#[tauri::command]
async fn connect_manual(
    state: State<'_, Arc<AppState>>,
    ip: String,
    port: u16,
) -> Result<(), String> {
    let addr: SocketAddr = format!("{}:{}", ip.trim(), port)
        .parse()
        .map_err(|e| format!("Invalid address: {e}"))?;
    let stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| "Could not connect: timeout".to_string())?
        .map_err(|e| format!("Could not connect: {e}"))?;
    let cloned = state.inner().clone();
    tokio::spawn(async move {
        let _ = handle_connection(cloned, stream, true, addr).await;
    });
    Ok(())
}

fn schedule_reconnect(state: Arc<AppState>, peer_id: String, address: SocketAddr) {
    if state.sessions.lock().unwrap().contains_key(&peer_id) {
        state.reconnect_attempts.lock().unwrap().remove(&peer_id);
        return;
    }
    let attempt = {
        let mut attempts = state.reconnect_attempts.lock().unwrap();
        let attempt = attempts.entry(peer_id.clone()).or_insert(0);
        if *attempt >= 5 {
            return;
        }
        let current = *attempt;
        *attempt += 1;
        current
    };
    if state
        .connecting
        .lock()
        .unwrap()
        .insert(peer_id.clone(), true)
        .is_some()
    {
        return;
    }
    tokio::spawn(async move {
        sleep(Duration::from_secs(
            2u64.saturating_pow(attempt as u32).min(16),
        ))
        .await;
        let result = match timeout(CONNECT_TIMEOUT, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => handle_connection(state.clone(), stream, true, address).await,
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("Connection timed out".into()),
        };
        if result.is_err() {
            state.connecting.lock().unwrap().remove(&peer_id);
            schedule_reconnect(state, peer_id, address);
        }
    });
}
#[tauri::command]
async fn send_chat(
    state: State<'_, Arc<AppState>>,
    peer_id: String,
    text: String,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("Message is empty".into());
    }
    let id = Uuid::new_v4();
    let sent_at = now();
    let tx = state
        .sessions
        .lock()
        .unwrap()
        .get(&peer_id)
        .map(|s| s.control_tx.clone())
        .ok_or("Peer is not connected")?;
    let message = Message::Chat {
        id,
        text: text.clone(),
        sent_at,
    };
    protocol::validate(&message)?;
    tx.send(message)
        .await
        .map_err(|_| "Peer connection closed".to_string())?;
    state
        .db
        .lock()
        .unwrap()
        .execute(
            "INSERT OR REPLACE INTO chat_messages VALUES (?1,?2,'sent',?3,?4,'sent')",
            rusqlite::params![id.to_string(), peer_id, text, sent_at],
        )
        .map_err(|e| e.to_string())?;
    Ok(id.to_string())
}
#[tauri::command]
async fn send_files(
    state: State<'_, Arc<AppState>>,
    peer_id: String,
    paths: Vec<String>,
) -> Result<String, String> {
    let id = Uuid::new_v4();
    let (control_tx, data_tx) = state
        .sessions
        .lock()
        .unwrap()
        .get(&peer_id)
        .map(|s| (s.control_tx.clone(), s.data_tx.clone()))
        .ok_or("Peer is not connected")?;
    let (expanded, directories) = expanded_transfer_inputs(&paths)?;
    let files = expanded
        .iter()
        .map(|(_, file)| file.clone())
        .collect::<Vec<_>>();
    let total = files
        .iter()
        .try_fold(0u64, |total, file| total.checked_add(file.size))
        .ok_or("Transfer size overflow")?;
    let cancelled = Arc::new(AtomicBool::new(false));
    state
        .transfers
        .lock()
        .unwrap()
        .insert(id, cancelled.clone());
    state
        .transfer_peers
        .lock()
        .unwrap()
        .insert(id, peer_id.clone());
    let (accept_tx, accept_rx) = oneshot::channel();
    state.pending_outgoing.lock().unwrap().insert(id, accept_tx);
    let app = state.app.clone();
    let history_state = state.inner().clone();
    let peer_for_task = peer_id.clone();
    let display_name = if files.len() == 1 {
        files[0]
            .relative_path
            .clone()
            .unwrap_or_else(|| files[0].name.clone())
    } else {
        format!("{} file(s)", files.len())
    };
    emit(
        &app,
        "transfer-progress",
        TransferEvent {
            peer_id: peer_id.clone(),
            id: id.to_string(),
            name: display_name.clone(),
            size: total,
            transferred: 0,
            state: "waiting_approval".into(),
            direction: "sent".into(),
            files: Some(files.iter().map(|file| file.name.clone()).collect()),
            directories: None,
        },
    );
    tokio::spawn(async move {
        let result: Result<(), String> = async {
            let request = Message::TransferRequest {
                id,
                files: files.clone(),
                directories,
                total_bytes: total,
            };
            protocol::validate(&request)?;
            control_tx.send(request).await.map_err(|_| "Peer connection closed".to_string())?;
            if !matches!(timeout(SESSION_IDLE_TIMEOUT, accept_rx).await, Ok(Ok(true))) {
                if cancelled.load(Ordering::Relaxed) {
                    let _ = control_tx.send(Message::TransferCancel { id }).await;
                    return Err("Transfer cancelled".into());
                }
                return Err("Transfer was declined or timed out".into());
            }
            for (path, file) in expanded {
                let mut hasher = Sha256::new();
                let mut buffer = vec![0u8; 1024 * 1024];
                let mut input = tokio::fs::File::open(&path).await.map_err(|e| e.to_string())?;
                loop { let read = input.read(&mut buffer).await.map_err(|e| e.to_string())?; if read == 0 { break; } hasher.update(&buffer[..read]); }
                data_tx.send(Message::FileStart { transfer_id: id, file: file.clone(), hash: hex::encode(hasher.finalize()) }).await.map_err(|_| "Peer connection closed".to_string())?;
                let mut input = tokio::fs::File::open(&path).await.map_err(|e| e.to_string())?;
                let mut offset = 0u64;
                loop {
                    if cancelled.load(Ordering::Relaxed) {
                        let _ = control_tx.send(Message::TransferCancel { id }).await;
                        let display_name = if files.len() == 1 {
                            files[0].name.clone()
                        } else {
                            format!("{} file(s)", files.len())
                        };
                        record_transfer_history(
                            &history_state,
                            &peer_for_task,
                            &display_name,
                            total,
                            "sent",
                            "cancelled",
                            None,
                        );
                        emit(&app, "transfer-error", serde_json::json!({"peer_id": peer_for_task, "id": id.to_string(), "state": "cancelled", "message": "Transfer cancelled"}));
                        return Ok(());
                    }
                    let read = input.read(&mut buffer).await.map_err(|e| e.to_string())?;
                    if read == 0 { break; }
                    data_tx.send(Message::FileChunk { transfer_id: id, file_id: file.id, offset, data: buffer[..read].to_vec() }).await.map_err(|_| "Peer connection closed".to_string())?;
                    offset += read as u64;
                    emit(&app, "transfer-progress", TransferEvent { peer_id: peer_for_task.clone(), id: id.to_string(), name: file.name.clone(), size: file.size, transferred: offset, state: "transferring".into(), direction: "sent".into(), files: None, directories: None });
                }
                data_tx.send(Message::FileEnd { transfer_id: id, file_id: file.id }).await.map_err(|_| "Peer connection closed".to_string())?;
                let history_name = file.relative_path.clone().unwrap_or_else(|| file.name.clone());
                record_transfer_history(
                    &history_state,
                    &peer_for_task,
                    &history_name,
                    file.size,
                    "sent",
                    "completed",
                    Some(&path),
                );
            }
            data_tx.send(Message::TransferComplete { id, name: display_name.clone(), size: total, transferred: total }).await.map_err(|_| "Peer connection closed".to_string())?;
            Ok(())
        }.await;
        history_state.pending_outgoing.lock().unwrap().remove(&id);
        history_state.transfers.lock().unwrap().remove(&id);
        if let Err(message) = result {
            history_state.transfer_peers.lock().unwrap().remove(&id);
            let lower = message.to_ascii_lowercase();
            let result_state = if cancelled.load(Ordering::Relaxed) || lower.contains("cancel") {
                "cancelled"
            } else if lower.contains("declined") || lower.contains("timed out") {
                "rejected"
            } else {
                "failed"
            };
            let display_name = if files.len() == 1 {
                files[0]
                    .relative_path
                    .clone()
                    .unwrap_or_else(|| files[0].name.clone())
            } else {
                format!("{} file(s)", files.len())
            };
            record_transfer_history(
                &history_state,
                &peer_for_task,
                &display_name,
                total,
                "sent",
                result_state,
                None,
            );
            emit(
                &app,
                "transfer-error",
                serde_json::json!({"peer_id": peer_for_task, "id": id.to_string(), "state": result_state, "message": message}),
            );
        }
    });
    Ok(id.to_string())
}
#[tauri::command]
fn cancel_transfer(state: State<'_, Arc<AppState>>, transfer_id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&transfer_id).map_err(|e| e.to_string())?;
    if let Some(flag) = state.transfers.lock().unwrap().get(&id) {
        flag.store(true, Ordering::Relaxed);
        if let Some(waiter) = state.pending_outgoing.lock().unwrap().remove(&id) {
            let _ = waiter.send(false);
        }
        return Ok(());
    }
    if let Some(flag) = state.incoming_cancellations.lock().unwrap().get(&id) {
        flag.store(true, Ordering::Relaxed);
        return Ok(());
    }
    Err("Transfer not found".into())
}

fn private_addresses() -> Vec<String> {
    let Ok(items) = local_ip_address::list_afinet_netifas() else {
        return Vec::new();
    };
    let mut preferred = Vec::new();
    let mut fallback = Vec::new();
    for (name, ip) in items {
        let lower = name.to_ascii_lowercase();
        let excluded_interface = lower.contains("docker")
            || lower.contains("veth")
            || lower.contains("hyper-v")
            || lower.contains("hyperv")
            || lower.contains("wsl")
            || lower.contains("vmware")
            || lower.contains("virtualbox")
            || lower.contains("virtual")
            || lower.contains("vpn")
            || lower.contains("loopback");
        let preferred_interface = !excluded_interface
            && (lower == "en0"
                || lower == "en1"
                || lower.contains("wifi")
                || lower.contains("wi-fi")
                || lower.contains("ethernet")
                || lower.starts_with("eth"));
        let value = match ip {
            IpAddr::V4(v4) if v4.is_private() => Some(v4.to_string()),
            IpAddr::V6(v6)
                if !v6.is_loopback() && !v6.is_unspecified() && !v6.is_unicast_link_local() =>
            {
                Some(v6.to_string())
            }
            _ => None,
        };
        if let Some(value) = value {
            if preferred_interface {
                preferred.push(value);
            } else {
                fallback.push(value);
            }
        }
    }
    if preferred.is_empty() {
        fallback
    } else {
        preferred
    }
}

fn display_socket(ip: IpAddr, port: u16) -> String {
    SocketAddr::new(ip, port).to_string()
}

fn start_mdns(state: Arc<AppState>, port: u16) {
    let Ok(mdns) = ServiceDaemon::new() else {
        update_local_status(&state, |info| {
            info.mdns_advertise = "failed".into();
            info.mdns_browse = "failed".into();
            info.mdns = "unavailable".into();
        });
        return;
    };
    *state.mdns.lock().unwrap() = Some(mdns.clone());
    let addresses = private_addresses();
    let service_type = "_localshare._tcp.local.";
    let advertised_name = state.device_name.lock().unwrap().clone();
    let props = [
        ("protocol_version", PROTOCOL_VERSION.to_string()),
        ("device_id", state.identity.device_id.to_string()),
        ("device_name", advertised_name),
        ("platform", std::env::consts::OS.to_string()),
        ("fingerprint", state.identity.fingerprint.clone()),
        ("capabilities", "chat,files,multi_file".to_string()),
    ];
    let host = format!("{}.local.", state.identity.device_id);
    if !addresses.is_empty() {
        let advertise_state = if let Ok(info) = ServiceInfo::new(
            service_type,
            &state.identity.device_id.to_string(),
            &host,
            addresses.as_slice(),
            port,
            props.as_slice(),
        ) {
            if mdns.register(info).is_ok() {
                "active"
            } else {
                "failed"
            }
        } else {
            "failed"
        };
        update_local_status(&state, |local| {
            local.mdns_advertise = advertise_state.into();
            local.mdns = "advertising + browsing".into();
        });
    } else {
        update_local_status(&state, |local| {
            local.mdns_advertise = "no private address".into();
            local.mdns = "browsing only".into();
        });
    }
    if let Ok(receiver) = mdns.browse(service_type) {
        update_local_status(&state, |local| local.mdns_browse = "active".into());
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let props = info.get_properties();
                        let Some(id) = props
                            .get_property_val_str("device_id")
                            .map(ToString::to_string)
                        else {
                            continue;
                        };
                        if id == state.identity.device_id.to_string() {
                            continue;
                        }
                        let name = props
                            .get_property_val_str("device_name")
                            .unwrap_or("LAN device")
                            .to_string();
                        let fingerprint = props
                            .get_property_val_str("fingerprint")
                            .unwrap_or("")
                            .to_string();
                        let platform = props
                            .get_property_val_str("platform")
                            .unwrap_or("LAN")
                            .to_string();
                        for ip in info.get_addresses() {
                            let peer_id = id.clone();
                            let address = display_socket(*ip, info.get_port());
                            let trusted = state
                                .db
                                .lock()
                                .unwrap()
                                .query_row(
                                    "SELECT 1 FROM trusted_peers WHERE device_id=?1 AND fingerprint=?2",
                                    rusqlite::params![peer_id, fingerprint],
                                    |_| Ok(true),
                                )
                                .unwrap_or(false);
                            let peer = Peer {
                                id: peer_id.clone(),
                                name: name.clone(),
                                address: address.clone(),
                                fingerprint: fingerprint.clone(),
                                state: if trusted {
                                    "available"
                                } else {
                                    "trust_required"
                                }
                                .into(),
                                platform: platform.clone(),
                                trusted,
                            };
                            state
                                .peers
                                .lock()
                                .unwrap()
                                .insert(peer_id.clone(), peer.clone());
                            emit(&state.app, "peer-updated", peer.clone());
                            let should_connect = trusted
                                && state.identity.device_id.to_string() < peer_id
                                && !state.sessions.lock().unwrap().contains_key(&peer_id)
                                && state
                                    .connecting
                                    .lock()
                                    .unwrap()
                                    .insert(peer_id.clone(), true)
                                    .is_none();
                            if should_connect {
                                let connect_state = state.clone();
                                let connect_id = peer_id.clone();
                                std::thread::spawn(move || {
                                    let runtime = tokio::runtime::Builder::new_current_thread()
                                        .enable_all()
                                        .build();
                                    if let Ok(runtime) = runtime {
                                        let cleanup_state = connect_state.clone();
                                        let task_state = connect_state.clone();
                                        let result = runtime.block_on(async move {
                                            let addr = address
                                                .parse::<SocketAddr>()
                                                .map_err(|e| e.to_string())?;
                                            let stream =
                                                timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
                                                    .await
                                                    .map_err(|_| {
                                                        "Connection timed out".to_string()
                                                    })?
                                                    .map_err(|e| e.to_string())?;
                                            handle_connection(task_state, stream, true, addr).await
                                        });
                                        if result.is_err() {
                                            cleanup_state
                                                .connecting
                                                .lock()
                                                .unwrap()
                                                .remove(&connect_id);
                                        }
                                    } else {
                                        connect_state
                                            .connecting
                                            .lock()
                                            .unwrap()
                                            .remove(&connect_id);
                                    }
                                });
                            }
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        if let Some(id) = fullname.split('.').next() {
                            if state.peers.lock().unwrap().remove(id).is_some() {
                                let _ = state.app.emit("peer-removed", id);
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    } else {
        update_local_status(&state, |local| {
            local.mdns_browse = "failed".into();
            if local.mdns_advertise == "active" {
                local.mdns = "advertising only".into();
            }
        });
    }
}

pub fn run() {
    install_crypto_provider();
    tracing_subscriber::fmt()
        .with_env_filter("localshare=info")
        .init();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init());
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_process::init());
    builder
        .setup(|app| {
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            let data = storage::app_data_dir()?;
            let db = storage::open().map_err(Box::<dyn std::error::Error>::from)?;
            let name = storage::setting(&db, "device_name");
            let identity = identity::load_or_create(&data, name)
                .map_err(Box::<dyn std::error::Error>::from)?;
            let updater_enabled = cfg!(any(
                target_os = "macos",
                target_os = "windows",
                target_os = "linux"
            )) && storage::setting(&db, "check_for_updates")
                .map(|value| value != "false")
                .unwrap_or(true);
            let receive_directory = storage::default_download_dir()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "Unavailable".into());
            let (tls_server, _) =
                tls_configs(&identity).map_err(Box::<dyn std::error::Error>::from)?;
            let app_handle = app.handle().clone();
            let probe = std::net::TcpListener::bind(("0.0.0.0", 32123))
                .or_else(|_| std::net::TcpListener::bind(("0.0.0.0", 0)))
                .map_err(Box::<dyn std::error::Error>::from)?;
            let port = probe.local_addr()?.port();
            probe.set_nonblocking(true)?;
            let ipv6_probe = std::net::TcpListener::bind(("::", port))
                .ok()
                .and_then(|listener| listener.set_nonblocking(true).ok().map(|()| listener));
            let state = Arc::new(AppState {
                identity: identity.clone(),
                device_name: Mutex::new(identity.device_name.clone()),
                db: Mutex::new(db),
                peers: Mutex::new(HashMap::new()),
                sessions: Mutex::new(HashMap::new()),
                transfers: Mutex::new(HashMap::new()),
                transfer_peers: Mutex::new(HashMap::new()),
                incoming_cancellations: Mutex::new(HashMap::new()),
                pending_incoming: Mutex::new(HashMap::new()),
                pending_outgoing: Mutex::new(HashMap::new()),
                connecting: Mutex::new(HashMap::new()),
                reconnect_attempts: Mutex::new(HashMap::new()),
                mdns: Mutex::new(None),
                local: Mutex::new(LocalInfo {
                    version: env!("CARGO_PKG_VERSION").into(),
                    device_id: identity.device_id.to_string(),
                    device_name: identity.device_name.clone(),
                    platform: std::env::consts::OS.into(),
                    architecture: std::env::consts::ARCH.into(),
                    fingerprint: identity.fingerprint.clone(),
                    port,
                    addresses: private_addresses(),
                    mdns: "advertising + browsing".into(),
                    mdns_advertise: "starting".into(),
                    mdns_browse: "starting".into(),
                    listener_state: "starting".into(),
                    tls_state: "configured".into(),
                    database_state: "healthy".into(),
                    receive_directory,
                    updater: if updater_enabled {
                        "enabled · Internet only".into()
                    } else {
                        "disabled".into()
                    },
                    transport: "TLS 1.3 / direct LAN TCP".into(),
                }),
                tls_server,
                app: app_handle.clone(),
            });
            app.manage(state.clone());
            if let Ok(directory) = download_dir(&state) {
                if let Err(error) = cleanup_stale_partials(&directory) {
                    tracing::warn!(error = %error, "unable to clean stale partial transfers");
                }
            }
            start_mdns(state.clone(), port);
            let listener_state = state.clone();
            tauri::async_runtime::spawn(async move {
                let listener = match TcpListener::from_std(probe) {
                    Ok(v) => v,
                    Err(_) => {
                        update_local_status(&listener_state, |local| {
                            local.listener_state = "failed".into()
                        });
                        return;
                    }
                };
                update_local_status(&listener_state, |local| {
                    local.listener_state = "active".into()
                });
                while let Ok((stream, addr)) = listener.accept().await {
                    let state = listener_state.clone();
                    tokio::spawn(async move {
                        let _ = handle_connection(state, stream, false, addr).await;
                    });
                }
                update_local_status(&listener_state, |local| {
                    local.listener_state = "stopped".into()
                });
            });
            if let Some(ipv6_probe) = ipv6_probe {
                let state = state.clone();
                tauri::async_runtime::spawn(async move {
                    let Ok(listener) = TcpListener::from_std(ipv6_probe) else {
                        return;
                    };
                    while let Ok((stream, addr)) = listener.accept().await {
                        let state = state.clone();
                        tokio::spawn(async move {
                            let _ = handle_connection(state, stream, false, addr).await;
                        });
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            local_info,
            list_peers,
            chat_history,
            transfer_history,
            clear_transfer_history,
            settings,
            save_settings,
            open_location,
            open_file,
            open_folder,
            accept_transfer,
            reject_transfer,
            trust_peer,
            connect_manual,
            send_chat,
            send_files,
            cancel_transfer
        ])
        .run(tauri::generate_context!())
        .expect("error while running LocalShare");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        let root = std::env::var("LOCALSHARE_TEST_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".localshare-test-work/unit"));
        root.join(format!("{}-{}", name, Uuid::new_v4()))
    }

    #[test]
    fn receive_paths_stay_inside_root() {
        let root = test_root("paths");
        fs::create_dir_all(&root).unwrap();
        assert!(safe_receive_path(&root, "Project/docs/report.pdf").is_ok());
        for path in ["../outside.txt", "/tmp/outside.txt", "C:/Windows/x"] {
            assert!(safe_receive_path(&root, path).is_err(), "accepted {path}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_names_are_deterministic() {
        let root = test_root("duplicates");
        fs::create_dir_all(&root).unwrap();
        let original = root.join("photo.txt");
        fs::write(&original, b"one").unwrap();
        fs::write(root.join("photo (1).txt"), b"two").unwrap();
        assert_eq!(
            unique_path_for_relative(&root, &original).unwrap(),
            root.join("photo (2).txt")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn folder_expansion_preserves_nested_and_empty_directories() {
        let root = test_root("folder");
        let folder = root.join("Project");
        fs::create_dir_all(folder.join("nested/empty")).unwrap();
        fs::write(folder.join("nested/hello world.txt"), b"hello").unwrap();
        let (files, directories) =
            expanded_transfer_inputs(&[folder.to_string_lossy().into_owned()]).unwrap();
        assert_eq!(files.len(), 1);
        assert!(directories.iter().any(|path| path == "Project"));
        assert!(directories
            .iter()
            .any(|path| path == "Project/nested/empty"));
        assert_eq!(
            files[0].1.relative_path.as_deref(),
            Some("Project/nested/hello world.txt")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn receive_paths_reject_symlink_components() {
        use std::os::unix::fs::symlink;

        let root = test_root("symlink");
        let outside = test_root("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("redirect")).unwrap();
        assert!(safe_receive_path(&root, "redirect/escape.txt").is_err());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
}

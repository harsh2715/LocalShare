#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod identity; mod protocol; mod storage;

use identity::Identity;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use protocol::{FileMeta, Message, PROTOCOL_VERSION, MAX_FRAME_BYTES};
use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, net::{IpAddr, SocketAddr}, path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, time::{SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::mpsc};
use tokio_rustls::{rustls::{self, client::danger::{ServerCertVerified, ServerCertVerifier}, pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime}, DigitallySignedStruct, ClientConfig, ServerConfig}, TlsAcceptor, TlsConnector};
use uuid::Uuid;

#[derive(Clone, Serialize)] struct Peer { id: String, name: String, address: String, fingerprint: String, state: String, platform: String }
#[derive(Clone, Serialize)] struct LocalInfo { device_id: String, device_name: String, fingerprint: String, port: u16, addresses: Vec<String>, mdns: String, transport: String }
#[derive(Clone, Serialize)] struct ChatEvent { peer_id: String, id: String, text: String, sent_at: i64, direction: String }
#[derive(Clone, Serialize)] struct TransferEvent { peer_id: String, id: String, name: String, size: u64, transferred: u64, state: String, direction: String }

struct Session { control_tx: mpsc::Sender<Message>, data_tx: mpsc::Sender<Message> }
struct AppState { identity: Identity, db: Mutex<Connection>, peers: Mutex<HashMap<String, Peer>>, sessions: Mutex<HashMap<String, Session>>, transfers: Mutex<HashMap<Uuid, Arc<AtomicBool>>>, local: Mutex<LocalInfo>, tls_server: Arc<ServerConfig>, app: AppHandle }
struct IncomingFile { transfer_id: Uuid, file: FileMeta, expected_hash: String, part: PathBuf, final_path: PathBuf, out: tokio::fs::File, hasher: Sha256, received: u64 }

fn now() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64 }
fn emit<T: Serialize + Clone>(app: &AppHandle, name: &str, payload: T) { let _ = app.emit(name, payload); }

fn tls_configs(identity: &Identity) -> Result<(Arc<ServerConfig>, Arc<ClientConfig>), String> {
    let cert = rcgen::generate_simple_self_signed(vec![identity.device_id.to_string()]).map_err(|e| e.to_string())?;
    let cert_der = cert.cert.der().clone();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
    let server = ServerConfig::builder().with_no_client_auth().with_single_cert(vec![cert_der], key).map_err(|e| e.to_string())?;
    let client = ClientConfig::builder().dangerous().with_custom_certificate_verifier(Arc::new(NoVerifier)).with_no_client_auth();
    Ok((Arc::new(server), Arc::new(client)))
}

fn install_crypto_provider() { let _ = rustls::crypto::aws_lc_rs::default_provider().install_default(); }

#[derive(Debug)] struct NoVerifier;
impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(&self, _: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> { Ok(ServerCertVerified::assertion()) }
    fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> { Ok(rustls::client::danger::HandshakeSignatureValid::assertion()) }
    fn verify_tls13_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> { Ok(rustls::client::danger::HandshakeSignatureValid::assertion()) }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> { vec![rustls::SignatureScheme::ECDSA_NISTP256_SHA256, rustls::SignatureScheme::ED25519, rustls::SignatureScheme::RSA_PSS_SHA256] }
}

async fn write_message<W: AsyncWriteExt + Unpin>(writer: &mut W, msg: &Message) -> Result<(), String> { let bytes = protocol::encode(msg).map_err(|e| e.to_string())?; if bytes.len() > MAX_FRAME_BYTES { return Err("Message too large".into()); } writer.write_u32(bytes.len() as u32).await.map_err(|e| e.to_string())?; writer.write_all(&bytes).await.map_err(|e| e.to_string())?; writer.flush().await.map_err(|e| e.to_string()) }
async fn read_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Message, String> { let len = reader.read_u32().await.map_err(|e| e.to_string())? as usize; if len == 0 || len > MAX_FRAME_BYTES { return Err("Invalid frame length".into()); } let mut buf = vec![0; len]; reader.read_exact(&mut buf).await.map_err(|e| e.to_string())?; let msg: Message = serde_json::from_slice(&buf).map_err(|e| e.to_string())?; protocol::validate(&msg)?; Ok(msg) }

async fn handle_connection(state: Arc<AppState>, stream: TcpStream, outgoing: bool, addr: SocketAddr) -> Result<(), String> {
    let (_, client) = tls_configs(&state.identity)?;
    if outgoing { let connector = TlsConnector::from(client); let tls = connector.connect(ServerName::try_from("localshare").map_err(|e| e.to_string())?, stream).await.map_err(|e| e.to_string())?; handle_tls_stream(state, tls, addr).await }
    else { let tls = TlsAcceptor::from(state.tls_server.clone()).accept(stream).await.map_err(|e| e.to_string())?; handle_tls_stream(state, tls, addr).await }
}

async fn handle_tls_stream<S>(state: Arc<AppState>, tls: S, addr: SocketAddr) -> Result<(), String>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static {
    let (mut reader, mut writer) = tokio::io::split(tls);
    write_message(&mut writer, &Message::Hello { protocol_version: PROTOCOL_VERSION, device_id: state.identity.device_id, device_name: state.identity.device_name.clone(), fingerprint: state.identity.fingerprint.clone() }).await?;
    let hello = read_message(&mut reader).await?;
    let (peer_id, peer_name, fingerprint) = match hello { Message::Hello { device_id, device_name, fingerprint, .. } => (device_id.to_string(), device_name, fingerprint), _ => return Err("Expected HELLO".into()) };
    write_message(&mut writer, &Message::HelloAck { protocol_version: PROTOCOL_VERSION }).await?;
    let peer = Peer { id: peer_id.clone(), name: peer_name.clone(), address: addr.to_string(), fingerprint: fingerprint.clone(), state: "connected".into(), platform: "LAN peer".into() };
    state.peers.lock().unwrap().insert(peer_id.clone(), peer.clone()); emit(&state.app, "peer-updated", &peer);
    let (control_tx, mut control_rx) = mpsc::channel::<Message>(128); let (data_tx, mut data_rx) = mpsc::channel::<Message>(16);
    state.sessions.lock().unwrap().insert(peer_id.clone(), Session { control_tx: control_tx.clone(), data_tx: data_tx.clone() });
    let writer_task = tokio::spawn(async move { loop { tokio::select! { biased; Some(msg) = control_rx.recv() => { if write_message(&mut writer, &msg).await.is_err() { break; } }, Some(msg) = data_rx.recv() => { if write_message(&mut writer, &msg).await.is_err() { break; } }, else => break } } });
    let mut incoming: Option<IncomingFile> = None;
    loop {
        match read_message(&mut reader).await {
            Ok(Message::Ping) => { let _ = control_tx.send(Message::Pong).await; }
            Ok(Message::Pong) => {}
            Ok(Message::Chat { id, text, sent_at }) => {
                let _ = state.db.lock().unwrap().execute("INSERT OR REPLACE INTO chat_messages VALUES (?1,?2,'received',?3,?4,'delivered')", rusqlite::params![id.to_string(), peer_id, text, sent_at]);
                emit(&state.app, "chat-message", ChatEvent { peer_id: peer_id.clone(), id: id.to_string(), text, sent_at, direction: "received".into() });
            }
            Ok(Message::TransferRequest { id, files, total_bytes }) => {
                emit(&state.app, "transfer-request", TransferEvent { peer_id: peer_id.clone(), id: id.to_string(), name: format!("{} file(s)", files.len()), size: total_bytes, transferred: 0, state: "pending".into(), direction: "received".into() });
                let _ = control_tx.send(Message::TransferAccept { id }).await;
            }
            Ok(Message::FileStart { transfer_id, file, hash }) => {
                let dir = storage::app_data_dir()?.join("Downloads");
                fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                protocol::validate_filename(&file.name)?;
                let final_path = unique_path(&dir, &file.name);
                let part = final_path.with_extension(format!("{}localshare-part", final_path.extension().and_then(|e| e.to_str()).unwrap_or("")));
                let out = tokio::fs::File::create(&part).await.map_err(|e| e.to_string())?;
                incoming = Some(IncomingFile { transfer_id, file, expected_hash: hash, part, final_path, out, hasher: Sha256::new(), received: 0 });
            }
            Ok(Message::FileChunk { file_id, offset, data, transfer_id }) => {
                if let Some(file) = incoming.as_mut() {
                    if file.transfer_id == transfer_id && file.file.id == file_id && file.received == offset {
                        file.out.write_all(&data).await.map_err(|e| e.to_string())?;
                        file.hasher.update(&data);
                        file.received += data.len() as u64;
                        emit(&state.app, "transfer-progress", TransferEvent { peer_id: peer_id.clone(), id: transfer_id.to_string(), name: file.file.name.clone(), size: file.file.size, transferred: file.received, state: "transferring".into(), direction: "received".into() });
                    }
                }
            }
            Ok(Message::FileEnd { transfer_id, file_id }) => {
                if let Some(mut file) = incoming.take() {
                    if file.transfer_id == transfer_id && file.file.id == file_id {
                        file.out.flush().await.map_err(|e| e.to_string())?;
                        let valid = file.received == file.file.size && hex::encode(file.hasher.finalize()) == file.expected_hash;
                        if valid {
                            tokio::fs::rename(&file.part, &file.final_path).await.map_err(|e| e.to_string())?;
                            let _ = control_tx.send(Message::TransferComplete { id: transfer_id }).await;
                            emit(&state.app, "transfer-complete", TransferEvent { peer_id: peer_id.clone(), id: transfer_id.to_string(), name: file.file.name, size: file.file.size, transferred: file.received, state: "completed".into(), direction: "received".into() });
                        } else {
                            let _ = tokio::fs::remove_file(&file.part).await;
                            emit(&state.app, "transfer-error", serde_json::json!({"peer_id": peer_id, "id": transfer_id.to_string(), "message": "Transfer failed integrity verification"}));
                        }
                    }
                }
            }
            Ok(Message::TransferCancel { id }) => {
                if let Some(file) = incoming.take() {
                    if file.transfer_id == id { let _ = tokio::fs::remove_file(file.part).await; }
                }
                emit(&state.app, "transfer-error", serde_json::json!({"peer_id": peer_id, "id": id.to_string(), "message": "Transfer cancelled by peer"}));
            }
            Ok(Message::TransferComplete { id }) => emit(&state.app, "transfer-complete", serde_json::json!({"peer_id": peer_id, "id": id.to_string()})),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    state.sessions.lock().unwrap().remove(&peer_id); if let Some(p) = state.peers.lock().unwrap().get_mut(&peer_id) { p.state = "offline".into(); emit(&state.app, "peer-updated", p.clone()); }
    writer_task.abort(); Ok(())
}

fn unique_path(dir: &Path, name: &str) -> PathBuf { let base = Path::new(name); let stem = base.file_stem().and_then(|v| v.to_str()).unwrap_or("file"); let ext = base.extension().and_then(|v| v.to_str()); let mut candidate = dir.join(name); let mut i = 1; while candidate.exists() { candidate = dir.join(match ext { Some(e) => format!("{} ({}).{}", stem, i, e), None => format!("{} ({})", stem, i) }); i += 1; } candidate }

#[tauri::command] fn local_info(state: State<'_, Arc<AppState>>) -> LocalInfo { state.local.lock().unwrap().clone() }
#[tauri::command] fn list_peers(state: State<'_, Arc<AppState>>) -> Vec<Peer> { state.peers.lock().unwrap().values().cloned().collect() }
#[tauri::command] async fn connect_manual(state: State<'_, Arc<AppState>>, ip: String, port: u16) -> Result<(), String> { let addr: SocketAddr = format!("{}:{}", ip.trim(), port).parse().map_err(|e| format!("Invalid address: {e}"))?; let stream = TcpStream::connect(addr).await.map_err(|e| format!("Could not connect: {e}"))?; let cloned = state.inner().clone(); tokio::spawn(async move { let _ = handle_connection(cloned, stream, true, addr).await; }); Ok(()) }
#[tauri::command] async fn send_chat(state: State<'_, Arc<AppState>>, peer_id: String, text: String) -> Result<String, String> { if text.trim().is_empty() { return Err("Message is empty".into()); } let id = Uuid::new_v4(); let sent_at = now(); let tx = state.sessions.lock().unwrap().get(&peer_id).map(|s| s.control_tx.clone()).ok_or("Peer is not connected")?; tx.send(Message::Chat { id, text: text.clone(), sent_at }).await.map_err(|_| "Peer connection closed".to_string())?; state.db.lock().unwrap().execute("INSERT OR REPLACE INTO chat_messages VALUES (?1,?2,'sent',?3,?4,'sent')", rusqlite::params![id.to_string(), peer_id, text, sent_at]).map_err(|e| e.to_string())?; Ok(id.to_string()) }
#[tauri::command] async fn send_files(state: State<'_, Arc<AppState>>, peer_id: String, paths: Vec<String>) -> Result<String, String> { let id = Uuid::new_v4(); let (control_tx, data_tx) = state.sessions.lock().unwrap().get(&peer_id).map(|s| (s.control_tx.clone(), s.data_tx.clone())).ok_or("Peer is not connected")?; let mut files = Vec::new(); for path in &paths { let meta = fs::metadata(path).map_err(|e| e.to_string())?; let name = Path::new(path).file_name().and_then(|n| n.to_str()).ok_or("Invalid filename")?.to_string(); protocol::validate_filename(&name)?; files.push(FileMeta { id: Uuid::new_v4(), name, relative_path: None, size: meta.len() }); } let total = files.iter().map(|f| f.size).sum(); let cancelled = Arc::new(AtomicBool::new(false)); state.transfers.lock().unwrap().insert(id, cancelled.clone()); let app = state.app.clone(); let peer_for_task = peer_id.clone(); tokio::spawn(async move { let result: Result<(), String> = async { control_tx.send(Message::TransferRequest { id, files: files.clone(), total_bytes: total }).await.map_err(|_| "Peer connection closed".to_string())?; for (path, file) in paths.iter().zip(files) { let mut hasher = Sha256::new(); let mut buffer = vec![0u8; 1024 * 1024]; let mut input = tokio::fs::File::open(path).await.map_err(|e| e.to_string())?; loop { let read = input.read(&mut buffer).await.map_err(|e| e.to_string())?; if read == 0 { break; } hasher.update(&buffer[..read]); } data_tx.send(Message::FileStart { transfer_id: id, file: file.clone(), hash: hex::encode(hasher.finalize()) }).await.map_err(|_| "Peer connection closed".to_string())?; let mut input = tokio::fs::File::open(path).await.map_err(|e| e.to_string())?; let mut offset = 0u64; loop { if cancelled.load(Ordering::Relaxed) { let _ = control_tx.send(Message::TransferCancel { id }).await; emit(&app, "transfer-error", serde_json::json!({"peer_id": peer_for_task, "id": id.to_string(), "message": "Transfer cancelled"})); return Ok(()); } let read = input.read(&mut buffer).await.map_err(|e| e.to_string())?; if read == 0 { break; } data_tx.send(Message::FileChunk { transfer_id: id, file_id: file.id, offset, data: buffer[..read].to_vec() }).await.map_err(|_| "Peer connection closed".to_string())?; offset += read as u64; emit(&app, "transfer-progress", TransferEvent { peer_id: peer_for_task.clone(), id: id.to_string(), name: file.name.clone(), size: file.size, transferred: offset, state: "transferring".into(), direction: "sent".into() }); } data_tx.send(Message::FileEnd { transfer_id: id, file_id: file.id }).await.map_err(|_| "Peer connection closed".to_string())?; } control_tx.send(Message::TransferComplete { id }).await.map_err(|_| "Peer connection closed".to_string())?; Ok(()) }.await; if let Err(message) = result { emit(&app, "transfer-error", serde_json::json!({"peer_id": peer_for_task, "id": id.to_string(), "message": message})); } }); Ok(id.to_string()) }
#[tauri::command] fn cancel_transfer(state: State<'_, Arc<AppState>>, transfer_id: String) -> Result<(), String> { let id = Uuid::parse_str(&transfer_id).map_err(|e| e.to_string())?; if let Some(flag) = state.transfers.lock().unwrap().get(&id) { flag.store(true, Ordering::Relaxed); Ok(()) } else { Err("Transfer not found".into()) } }

fn private_addresses() -> Vec<String> { local_ip_address::list_afinet_netifas().ok().map(|items| items.into_iter().filter_map(|(_, ip)| match ip { IpAddr::V4(v4) if v4.is_private() => Some(v4.to_string()), IpAddr::V6(v6) if !v6.is_loopback() && !v6.is_unspecified() => Some(v6.to_string()), _ => None }).collect()).unwrap_or_default() }

fn start_mdns(state: Arc<AppState>, port: u16) {
    let Ok(mdns) = ServiceDaemon::new() else { return };
    let addresses = private_addresses(); if addresses.is_empty() { return; }
    let service_type = "_localshare._tcp.local.";
    let props = [("protocol_version", PROTOCOL_VERSION.to_string()), ("device_id", state.identity.device_id.to_string()), ("device_name", state.identity.device_name.clone()), ("platform", std::env::consts::OS.to_string()), ("fingerprint", state.identity.fingerprint.clone()), ("capabilities", "chat,files,multi_file".to_string())];
    let host = format!("{}.local.", state.identity.device_id);
    if let Ok(info) = ServiceInfo::new(service_type, &state.identity.device_id.to_string(), &host, addresses.as_slice(), port, props.as_slice()) { let _ = mdns.register(info); }
    if let Ok(receiver) = mdns.browse(service_type) { std::thread::spawn(move || { while let Ok(event) = receiver.recv() { if let ServiceEvent::ServiceResolved(info) = event { let props = info.get_properties(); let Some(id) = props.get_property_val_str("device_id").map(ToString::to_string) else { continue; }; if id == state.identity.device_id.to_string() { continue; } let name = props.get_property_val_str("device_name").unwrap_or("LAN device").to_string(); let fingerprint = props.get_property_val_str("fingerprint").unwrap_or("").to_string(); for ip in info.get_addresses() { let peer = Peer { id: id.clone(), name: name.clone(), address: format!("{}:{}", ip, info.get_port()), fingerprint: fingerprint.clone(), state: "available".into(), platform: props.get_property_val_str("platform").unwrap_or("LAN").into() }; state.peers.lock().unwrap().insert(id.clone(), peer.clone()); emit(&state.app, "peer-updated", peer); } } } }); }
}

fn main() { install_crypto_provider(); tracing_subscriber::fmt().with_env_filter("localshare=info").init(); tauri::Builder::default().plugin(tauri_plugin_dialog::init()).setup(|app| { let data = storage::app_data_dir()?; let db = storage::open().map_err(|e| Box::<dyn std::error::Error>::from(e))?; let name = storage::setting(&db, "device_name"); let identity = identity::load_or_create(&data, name).map_err(|e| Box::<dyn std::error::Error>::from(e))?; let (tls_server, _) = tls_configs(&identity).map_err(|e| Box::<dyn std::error::Error>::from(e))?; let app_handle = app.handle().clone(); let probe = std::net::TcpListener::bind(("0.0.0.0", 32123)).or_else(|_| std::net::TcpListener::bind(("0.0.0.0", 0))).map_err(|e| Box::<dyn std::error::Error>::from(e))?; let port = probe.local_addr()?.port(); drop(probe); let state = Arc::new(AppState { identity: identity.clone(), db: Mutex::new(db), peers: Mutex::new(HashMap::new()), sessions: Mutex::new(HashMap::new()), transfers: Mutex::new(HashMap::new()), local: Mutex::new(LocalInfo { device_id: identity.device_id.to_string(), device_name: identity.device_name.clone(), fingerprint: identity.fingerprint.clone(), port, addresses: private_addresses(), mdns: "advertising + browsing".into(), transport: "TLS 1.3 / direct LAN TCP".into() }), tls_server, app: app_handle.clone() }); app.manage(state.clone()); start_mdns(state.clone(), port); tauri::async_runtime::spawn(async move { let listener = match TcpListener::bind(("0.0.0.0", port)).await { Ok(v) => v, Err(_) => return }; while let Ok((stream, addr)) = listener.accept().await { let state = state.clone(); tokio::spawn(async move { let _ = handle_connection(state, stream, false, addr).await; }); } }); Ok(()) }).invoke_handler(tauri::generate_handler![local_info, list_peers, connect_manual, send_chat, send_files, cancel_transfer]).run(tauri::generate_context!()).expect("error while running LocalShare"); }

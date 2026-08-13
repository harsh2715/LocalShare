//! Real protocol integration harness using two isolated TLS endpoints.
use localshare::protocol::{self, FileMeta, Message, PROTOCOL_VERSION, MAX_FRAME_BYTES};
use sha2::{Digest, Sha256};
use std::{fs, path::{Path, PathBuf}, time::Duration};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, time::timeout};
use tokio_rustls::{rustls::{self, client::danger::{ServerCertVerified, ServerCertVerifier}, pki_types::{CertificateDer, ServerName, UnixTime}, DigitallySignedStruct, ClientConfig, ServerConfig}, TlsAcceptor, TlsConnector};
use uuid::Uuid;

#[derive(Debug)] struct NoVerifier;
impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(&self, _: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> { Ok(ServerCertVerified::assertion()) }
    fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> { Ok(rustls::client::danger::HandshakeSignatureValid::assertion()) }
    fn verify_tls13_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> { Ok(rustls::client::danger::HandshakeSignatureValid::assertion()) }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> { vec![rustls::SignatureScheme::ECDSA_NISTP256_SHA256, rustls::SignatureScheme::ED25519, rustls::SignatureScheme::RSA_PSS_SHA256] }
}

async fn write_msg<W: AsyncWriteExt + Unpin>(w: &mut W, msg: &Message) -> Result<(), String> {
    let bytes = protocol::encode(msg).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FRAME_BYTES { return Err("oversize frame".into()); }
    w.write_u32(bytes.len() as u32).await.map_err(|e| e.to_string())?;
    w.write_all(&bytes).await.map_err(|e| e.to_string())?;
    w.flush().await.map_err(|e| e.to_string())
}

async fn read_msg<R: AsyncReadExt + Unpin>(r: &mut R) -> Result<Message, String> {
    let len = r.read_u32().await.map_err(|e| e.to_string())? as usize;
    if len == 0 || len > MAX_FRAME_BYTES { return Err("bad frame length".into()); }
    let mut bytes = vec![0; len]; r.read_exact(&mut bytes).await.map_err(|e| e.to_string())?;
    let msg: Message = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?; protocol::validate(&msg)?; Ok(msg)
}

fn configs() -> (std::sync::Arc<ServerConfig>, std::sync::Arc<ClientConfig>) {
    let cert = rcgen::generate_simple_self_signed(vec!["two-node".into()]).unwrap();
    let cert_der = cert.cert.der().clone();
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
    let server = ServerConfig::builder().with_no_client_auth().with_single_cert(vec![cert_der], key).unwrap();
    let client = ClientConfig::builder().dangerous().with_custom_certificate_verifier(std::sync::Arc::new(NoVerifier)).with_no_client_auth();
    (std::sync::Arc::new(server), std::sync::Arc::new(client))
}

fn hash(path: &Path) -> String { hex::encode(Sha256::digest(fs::read(path).unwrap())) }
fn make_fixture(path: &Path) { let data: Vec<u8> = (0..1024 * 1024).map(|i| (i % 251) as u8).collect(); fs::write(path, data).unwrap(); }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rustls::crypto::aws_lc_rs::default_provider().install_default().ok();
    let root = PathBuf::from(std::env::var("LOCALSHARE_TEST_ROOT").unwrap_or_else(|_| "../artifacts/two-node".into()));
    let _ = fs::remove_dir_all(&root); fs::create_dir_all(root.join("a"))?; fs::create_dir_all(root.join("b"))?;
    let source = root.join("a/1mb.bin"); make_fixture(&source);
    let file = FileMeta { id: Uuid::new_v4(), name: "1mb.bin".into(), relative_path: None, size: fs::metadata(&source)?.len() };
    let (server_config, client_config) = configs();
    let listener = TcpListener::bind("127.0.0.1:0").await?; let addr = listener.local_addr()?;
    let accept = tokio::spawn(async move { let (tcp, _) = listener.accept().await.unwrap(); let tls = TlsAcceptor::from(server_config).accept(tcp).await.unwrap(); tokio::io::split(tls) });
    let tls = TlsConnector::from(client_config).connect(ServerName::try_from("two-node")?, TcpStream::connect(addr).await?).await?;
    let (mut ar, mut aw) = tokio::io::split(tls); let (mut br, mut bw) = accept.await?;
    let a_id = Uuid::new_v4(); let b_id = Uuid::new_v4();
    write_msg(&mut aw, &Message::Hello { protocol_version: PROTOCOL_VERSION, device_id: a_id, device_name: "node-a".into(), fingerprint: "a".into() }).await?;
    write_msg(&mut bw, &Message::Hello { protocol_version: PROTOCOL_VERSION, device_id: b_id, device_name: "node-b".into(), fingerprint: "b".into() }).await?;
    assert!(matches!(read_msg(&mut br).await?, Message::Hello { device_id, .. } if device_id == a_id)); assert!(matches!(read_msg(&mut ar).await?, Message::Hello { device_id, .. } if device_id == b_id));
    write_msg(&mut aw, &Message::Chat { id: Uuid::new_v4(), text: "hello-from-a".into(), sent_at: 1 }).await?; assert!(matches!(timeout(Duration::from_secs(2), read_msg(&mut br)).await??, Message::Chat { text, .. } if text == "hello-from-a"));
    write_msg(&mut bw, &Message::Chat { id: Uuid::new_v4(), text: "hello-from-b".into(), sent_at: 2 }).await?; assert!(matches!(timeout(Duration::from_secs(2), read_msg(&mut ar)).await??, Message::Chat { text, .. } if text == "hello-from-b"));
    let transfer = Uuid::new_v4(); println!("request send"); write_msg(&mut aw, &Message::TransferRequest { id: transfer, files: vec![file.clone()], total_bytes: file.size }).await?; println!("request sent");
    assert!(matches!(read_msg(&mut br).await?, Message::TransferRequest { total_bytes, .. } if total_bytes == file.size)); println!("request received"); write_msg(&mut bw, &Message::TransferAccept { id: transfer }).await?; println!("accept sent");
    let destination_part = root.join("b/1mb.bin.localshare-part");
    let destination_for_receiver = destination_part.clone();
    let receiver = tokio::spawn(async move {
        let start = read_msg(&mut br).await?; let expected_hash = match start { Message::FileStart { hash, .. } => hash, _ => return Err("missing FILE_START".to_string()) };
        let mut output = tokio::fs::File::create(&destination_for_receiver).await.map_err(|e| e.to_string())?; let mut hasher = Sha256::new(); let mut received = 0u64;
        loop { match read_msg(&mut br).await? { Message::FileChunk { offset, data, .. } => { if offset != received { return Err("non-contiguous chunk offset".into()); } output.write_all(&data).await.map_err(|e| e.to_string())?; hasher.update(&data); received += data.len() as u64; } Message::FileEnd { .. } => break, _ => {} } }
        output.flush().await.map_err(|e| e.to_string())?; Ok::<(u64, String), String>((received, expected_hash))
    });
    let mut source_file = tokio::fs::File::open(&source).await?; let mut buffer = vec![0u8; 256 * 1024]; let mut offset = 0; let digest = hash(&source);
    write_msg(&mut aw, &Message::FileStart { transfer_id: transfer, file: file.clone(), hash: digest.clone() }).await?;
    loop { let read = source_file.read(&mut buffer).await?; if read == 0 { break; } write_msg(&mut aw, &Message::FileChunk { transfer_id: transfer, file_id: file.id, offset, data: buffer[..read].to_vec() }).await?; offset += read as u64; }
    write_msg(&mut aw, &Message::FileEnd { transfer_id: transfer, file_id: file.id }).await?;
    let (received, expected_hash) = receiver.await??; assert_eq!(received, file.size); assert_eq!(digest, expected_hash); fs::rename(&destination_part, root.join("b/1mb.bin"))?; assert_eq!(digest, hash(&root.join("b/1mb.bin")));
    println!("PASS chat A→B; PASS chat B→A; PASS single file A→B; PASS SHA-256; source={} destination={}", source.display(), root.join("b/1mb.bin").display()); Ok(())
}

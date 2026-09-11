//! Real protocol integration harness using two isolated TLS endpoints.
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use localshare::protocol::{self, FileMeta, Message, MAX_FRAME_BYTES, PROTOCOL_VERSION};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_rustls::{
    rustls::{
        self,
        client::danger::{ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, ServerName, UnixTime},
        ClientConfig, DigitallySignedStruct, ServerConfig,
    },
    TlsAcceptor, TlsConnector,
};
use uuid::Uuid;

#[derive(Debug)]
struct NoVerifier;
impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::RSA_PSS_SHA256,
        ]
    }
}

async fn write_msg<W: AsyncWriteExt + Unpin>(w: &mut W, msg: &Message) -> Result<(), String> {
    let bytes = protocol::encode(msg).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err("oversize frame".into());
    }
    w.write_u32(bytes.len() as u32)
        .await
        .map_err(|e| e.to_string())?;
    w.write_all(&bytes).await.map_err(|e| e.to_string())?;
    w.flush().await.map_err(|e| e.to_string())
}

async fn read_msg<R: AsyncReadExt + Unpin>(r: &mut R) -> Result<Message, String> {
    let len = r.read_u32().await.map_err(|e| e.to_string())? as usize;
    if len == 0 || len > MAX_FRAME_BYTES {
        return Err("bad frame length".into());
    }
    let mut bytes = vec![0; len];
    r.read_exact(&mut bytes).await.map_err(|e| e.to_string())?;
    let msg: Message = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    protocol::validate(&msg)?;
    Ok(msg)
}

fn configs() -> (std::sync::Arc<ServerConfig>, std::sync::Arc<ClientConfig>) {
    let cert = rcgen::generate_simple_self_signed(vec!["two-node".into()]).unwrap();
    let cert_der = cert.cert.der().clone();
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
        cert.key_pair.serialize_der(),
    ));
    let server = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key)
        .unwrap();
    let client = ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .dangerous()
        .with_custom_certificate_verifier(std::sync::Arc::new(NoVerifier))
        .with_no_client_auth();
    (std::sync::Arc::new(server), std::sync::Arc::new(client))
}

fn fingerprint(key: &ed25519_dalek::SigningKey) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.verifying_key().to_bytes());
    hex::encode(hasher.finalize())
}

fn make_hello(
    id: Uuid,
    name: &str,
    key: &ed25519_dalek::SigningKey,
    port: u16,
    nonce: &[u8],
) -> Message {
    let public_key = key.verifying_key().to_bytes().to_vec();
    let capabilities = vec!["chat".into(), "files".into(), "multi_file".into()];
    let signing_bytes = protocol::hello_signing_bytes(
        PROTOCOL_VERSION,
        id,
        name,
        &fingerprint(key),
        &public_key,
        port,
        nonce,
        &capabilities,
    );
    Message::Hello {
        protocol_version: PROTOCOL_VERSION,
        device_id: id,
        device_name: name.into(),
        fingerprint: fingerprint(key),
        public_key,
        listening_port: port,
        nonce: nonce.to_vec(),
        signature: key.sign(&signing_bytes).to_bytes().to_vec(),
        capabilities,
    }
}

fn verify_hello(message: Message) -> Result<(Uuid, Vec<u8>, Vec<u8>), String> {
    let Message::Hello {
        protocol_version,
        device_id,
        device_name,
        fingerprint,
        public_key,
        listening_port,
        nonce,
        signature,
        capabilities,
    } = message
    else {
        return Err("expected hello".into());
    };
    let key_bytes: [u8; 32] = public_key.as_slice().try_into().map_err(|_| "bad key")?;
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| "bad key")?;
    let bytes = protocol::hello_signing_bytes(
        protocol_version,
        device_id,
        &device_name,
        &fingerprint,
        &public_key,
        listening_port,
        &nonce,
        &capabilities,
    );
    let signature = Signature::from_slice(&signature).map_err(|_| "bad signature")?;
    key.verify(&bytes, &signature)
        .map_err(|_| "bad signature")?;
    Ok((device_id, public_key, nonce))
}

fn hash(path: &Path) -> String {
    let mut input = fs::File::open(path).unwrap();
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = input.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    hex::encode(hasher.finalize())
}
fn make_fixture(path: &Path, size_mb: u64) {
    let mut output = fs::File::create(path).unwrap();
    let buffer: Vec<u8> = (0..1024 * 1024).map(|i| (i % 251) as u8).collect();
    for _ in 0..size_mb {
        output.write_all(&buffer).unwrap();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();
    let root = PathBuf::from(
        std::env::var("LOCALSHARE_TEST_ROOT")
            .unwrap_or_else(|_| "../.localshare-test-work/two-node".into()),
    );
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("a"))?;
    fs::create_dir_all(root.join("b"))?;
    let source = root.join("a/1mb.bin");
    let size_mb = std::env::var("LOCALSHARE_TEST_SIZE_MB")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1);
    make_fixture(&source, size_mb);
    let file = FileMeta {
        id: Uuid::new_v4(),
        name: "1mb.bin".into(),
        relative_path: None,
        size: fs::metadata(&source)?.len(),
    };
    let (server_config, client_config) = configs();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let accept = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let tls = TlsAcceptor::from(server_config).accept(tcp).await.unwrap();
        tokio::io::split(tls)
    });
    let tls = TlsConnector::from(client_config)
        .connect(
            ServerName::try_from("two-node")?,
            TcpStream::connect(addr).await?,
        )
        .await?;
    let (mut ar, mut aw) = tokio::io::split(tls);
    let (mut br, mut bw) = accept.await?;
    let a_id = Uuid::new_v4();
    let b_id = Uuid::new_v4();
    let a_key = ed25519_dalek::SigningKey::generate(&mut OsRng);
    let b_key = ed25519_dalek::SigningKey::generate(&mut OsRng);
    let mut a_nonce = vec![0u8; 32];
    let mut b_nonce = vec![0u8; 32];
    OsRng.fill_bytes(&mut a_nonce);
    OsRng.fill_bytes(&mut b_nonce);
    write_msg(
        &mut aw,
        &make_hello(a_id, "node-a", &a_key, addr.port(), &a_nonce),
    )
    .await?;
    write_msg(
        &mut bw,
        &make_hello(b_id, "node-b", &b_key, addr.port(), &b_nonce),
    )
    .await?;
    let (received_a_id, received_a_key, received_a_nonce) = verify_hello(read_msg(&mut br).await?)?;
    let (received_b_id, received_b_key, received_b_nonce) = verify_hello(read_msg(&mut ar).await?)?;
    assert_eq!(received_a_id, a_id);
    assert_eq!(received_b_id, b_id);
    assert_eq!(received_a_nonce, a_nonce);
    assert_eq!(received_b_nonce, b_nonce);
    write_msg(
        &mut aw,
        &Message::HelloAck {
            protocol_version: PROTOCOL_VERSION,
            device_id: a_id,
            public_key: a_key.verifying_key().to_bytes().to_vec(),
            signature: a_key
                .sign(&protocol::ack_signing_bytes(
                    PROTOCOL_VERSION,
                    b_id,
                    &b_nonce,
                    a_id,
                    &a_nonce,
                ))
                .to_bytes()
                .to_vec(),
        },
    )
    .await?;
    write_msg(
        &mut bw,
        &Message::HelloAck {
            protocol_version: PROTOCOL_VERSION,
            device_id: b_id,
            public_key: b_key.verifying_key().to_bytes().to_vec(),
            signature: b_key
                .sign(&protocol::ack_signing_bytes(
                    PROTOCOL_VERSION,
                    a_id,
                    &a_nonce,
                    b_id,
                    &b_nonce,
                ))
                .to_bytes()
                .to_vec(),
        },
    )
    .await?;
    assert!(
        matches!(read_msg(&mut br).await?, Message::HelloAck { device_id, .. } if device_id == a_id)
    );
    assert!(
        matches!(read_msg(&mut ar).await?, Message::HelloAck { device_id, .. } if device_id == b_id)
    );
    assert_eq!(received_a_key, a_key.verifying_key().to_bytes());
    assert_eq!(received_b_key, b_key.verifying_key().to_bytes());
    write_msg(
        &mut aw,
        &Message::Chat {
            id: Uuid::new_v4(),
            text: "hello-from-a".into(),
            sent_at: 1,
        },
    )
    .await?;
    assert!(
        matches!(timeout(Duration::from_secs(2), read_msg(&mut br)).await??, Message::Chat { text, .. } if text == "hello-from-a")
    );
    write_msg(
        &mut bw,
        &Message::Chat {
            id: Uuid::new_v4(),
            text: "hello-from-b".into(),
            sent_at: 2,
        },
    )
    .await?;
    assert!(
        matches!(timeout(Duration::from_secs(2), read_msg(&mut ar)).await??, Message::Chat { text, .. } if text == "hello-from-b")
    );
    let transfer = Uuid::new_v4();
    println!("request send");
    write_msg(
        &mut aw,
        &Message::TransferRequest {
            id: transfer,
            files: vec![file.clone()],
            directories: Vec::new(),
            total_bytes: file.size,
        },
    )
    .await?;
    println!("request sent");
    assert!(
        matches!(read_msg(&mut br).await?, Message::TransferRequest { total_bytes, .. } if total_bytes == file.size)
    );
    println!("request received");
    write_msg(&mut bw, &Message::TransferAccept { id: transfer }).await?;
    println!("accept sent");
    let destination_part = root.join("b/1mb.bin.localshare-part");
    let destination_for_receiver = destination_part.clone();
    let receiver = tokio::spawn(async move {
        let start = read_msg(&mut br).await?;
        let expected_hash = match start {
            Message::FileStart { hash, .. } => hash,
            _ => return Err("missing FILE_START".to_string()),
        };
        let mut output = tokio::fs::File::create(&destination_for_receiver)
            .await
            .map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut received = 0u64;
        let mut saw_chat_during_transfer = false;
        let mut saw_file_end = false;
        loop {
            match read_msg(&mut br).await? {
                Message::Chat { text, .. } if text == "hello-from-a-during-transfer" => {
                    saw_chat_during_transfer = true;
                }
                Message::FileChunk { offset, data, .. } => {
                    if offset != received {
                        return Err("non-contiguous chunk offset".into());
                    }
                    output.write_all(&data).await.map_err(|e| e.to_string())?;
                    hasher.update(&data);
                    received += data.len() as u64;
                }
                Message::FileEnd { .. } => saw_file_end = true,
                Message::TransferComplete { .. } => {
                    if !saw_file_end {
                        return Err("completion overtook file end".to_string());
                    }
                    break;
                }
                _ => {}
            }
        }
        output.flush().await.map_err(|e| e.to_string())?;
        Ok::<(u64, String, bool), String>((received, expected_hash, saw_chat_during_transfer))
    });
    let mut b_chat_reader = tokio::spawn(async move {
        loop {
            match read_msg(&mut ar).await? {
                Message::Chat { text, .. } if text == "hello-from-b-during-transfer" => {
                    return Ok::<bool, String>(true);
                }
                _ => {}
            }
        }
    });
    let mut source_file = tokio::fs::File::open(&source).await?;
    let mut buffer = vec![0u8; 256 * 1024];
    let mut offset = 0;
    let mut sent_during_transfer_chat = false;
    let digest = hash(&source);
    write_msg(
        &mut aw,
        &Message::FileStart {
            transfer_id: transfer,
            file: file.clone(),
            hash: digest.clone(),
        },
    )
    .await?;
    loop {
        let read = source_file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        write_msg(
            &mut aw,
            &Message::FileChunk {
                transfer_id: transfer,
                file_id: file.id,
                offset,
                data: buffer[..read].to_vec(),
            },
        )
        .await?;
        offset += read as u64;
        if !sent_during_transfer_chat {
            write_msg(
                &mut aw,
                &Message::Chat {
                    id: Uuid::new_v4(),
                    text: "hello-from-a-during-transfer".into(),
                    sent_at: 3,
                },
            )
            .await?;
            write_msg(
                &mut bw,
                &Message::Chat {
                    id: Uuid::new_v4(),
                    text: "hello-from-b-during-transfer".into(),
                    sent_at: 4,
                },
            )
            .await?;
            let joined_b_chat = timeout(Duration::from_secs(2), &mut b_chat_reader)
                .await
                .map_err(|error| error.to_string())?;
            let b_seen_result = joined_b_chat.map_err(|error| error.to_string())?;
            let b_seen = b_seen_result?;
            assert!(b_seen);
            sent_during_transfer_chat = true;
        }
    }
    write_msg(
        &mut aw,
        &Message::FileEnd {
            transfer_id: transfer,
            file_id: file.id,
        },
    )
    .await?;
    write_msg(
        &mut aw,
        &Message::TransferComplete {
            id: transfer,
            name: file.name.clone(),
            size: file.size,
            transferred: file.size,
        },
    )
    .await?;
    let (received, expected_hash, saw_chat_during_transfer) = receiver.await??;
    assert_eq!(received, file.size);
    assert_eq!(digest, expected_hash);
    assert!(saw_chat_during_transfer);
    fs::rename(&destination_part, root.join("b/1mb.bin"))?;
    assert_eq!(digest, hash(&root.join("b/1mb.bin")));
    println!("PASS chat A→B; PASS chat B→A; PASS chat during transfer A↔B; PASS streaming single file A→B ({} MiB); PASS SHA-256; source={} destination={}", size_mb, source.display(), root.join("b/1mb.bin").display());
    Ok(())
}

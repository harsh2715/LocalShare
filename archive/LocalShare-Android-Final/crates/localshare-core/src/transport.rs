use crate::{protocol, CoreEvent, LocalShareCore, Peer};
use anyhow::{anyhow, bail, Result};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
    server::danger::{ClientCertVerified, ClientCertVerifier},
    ClientConfig, DigitallySignedStruct, DistinguishedName, ServerConfig, SignatureScheme,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{self, Duration, Instant};
use tokio_rustls::{TlsAcceptor, TlsConnector};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);

pub async fn spawn_listener(core: Arc<LocalShareCore>) -> Result<()> {
    let cfg = core.config().clone();
    let listener = TcpListener::bind((cfg.bind_host.as_str(), cfg.port)).await?;
    core.set_listener_port(listener.local_addr()?.port() as u16);
    let acceptor = TlsAcceptor::from(server_config(core.identity())?);
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let acceptor = acceptor.clone();
                    let core = core.clone();
                    tokio::spawn(async move {
                        match acceptor.accept(stream).await {
                            Ok(tls) => {
                                if let Err(error) =
                                    handle_server(core.clone(), tls, addr.ip().to_string()).await
                                {
                                    core.emit(CoreEvent::Error(error.to_string()));
                                }
                            }
                            Err(error) => {
                                core.emit(CoreEvent::Error(format!("TLS accept failed: {error}")))
                            }
                        }
                    });
                }
                Err(error) => {
                    core.emit(CoreEvent::Error(format!("listener failed: {error}")));
                    break;
                }
            }
        }
    });
    Ok(())
}

fn server_config(identity: &crate::identity::Identity) -> Result<Arc<ServerConfig>> {
    let certs = rustls_pemfile::certs(&mut identity.certificate_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::pkcs8_private_keys(&mut identity.private_key_pem.as_bytes())
        .next()
        .ok_or_else(|| anyhow!("missing private key"))??;
    Ok(Arc::new(
        ServerConfig::builder()
            .with_client_cert_verifier(Arc::new(InsecureClientVerifier))
            .with_single_cert(certs, PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)))?,
    ))
}

async fn handle_server(
    core: Arc<LocalShareCore>,
    mut stream: tokio_rustls::server::TlsStream<TcpStream>,
    host: String,
) -> Result<()> {
    let hello = protocol::read_frame(&mut stream).await?;
    if hello.kind != "hello" {
        bail!("expected hello")
    }
    let certificate_fingerprint = server_certificate_fingerprint(&stream)?;
    let peer = peer_from_hello(&hello.payload, host)?;
    if peer.fingerprint != certificate_fingerprint {
        bail!("peer hello fingerprint does not match TLS certificate")
    }
    if core.store().is_blocked(&peer.id)? {
        bail!("peer is blocked")
    }
    if let Some(trusted) = core.store().trusted_fingerprint(&peer.id)? {
        if trusted != peer.fingerprint {
            bail!("device identity changed; re-pair required")
        }
    }
    let peer = Peer {
        state: if core.store().trusted_fingerprint(&peer.id)?.as_deref()
            == Some(peer.fingerprint.as_str())
        {
            "TRUSTED"
        } else {
            "UNTRUSTED"
        }
        .into(),
        ..peer
    };
    let peer_id = peer.id.clone();
    let (sender, receiver) = mpsc::channel(64);
    let (close_sender, close_receiver) = oneshot::channel();
    if !core
        .register_session(peer_id.clone(), sender.clone(), close_sender)
        .await
    {
        bail!("duplicate peer session")
    }
    core.store().save_peer(&peer)?;
    core.emit(CoreEvent::Peer(peer));
    protocol::write_frame(&mut stream, &protocol::frame("hello", hello_payload(&core))).await?;
    let _ = crate::chat::flush_queued(core.clone(), &peer_id).await;
    let _ = crate::transfer::resume_outgoing(core.clone(), &peer_id).await;
    let result = run_session(
        core.clone(),
        stream,
        peer_id.clone(),
        sender,
        receiver,
        close_receiver,
    )
    .await;
    core.unregister_session(&peer_id).await;
    result
}

fn peer_from_hello(payload: &serde_json::Value, host: String) -> Result<Peer> {
    let id = payload["deviceId"]
        .as_str()
        .ok_or_else(|| anyhow!("hello missing deviceId"))?;
    let name = payload["deviceName"]
        .as_str()
        .ok_or_else(|| anyhow!("hello missing deviceName"))?;
    let fingerprint = payload["fingerprint"]
        .as_str()
        .ok_or_else(|| anyhow!("hello missing fingerprint"))?;
    Ok(Peer {
        id: id.into(),
        name: name.into(),
        host,
        port: payload["port"].as_u64().unwrap_or_default() as u16,
        fingerprint: fingerprint.into(),
        protocol_version: payload["versions"]
            .as_array()
            .and_then(|versions| versions.first())
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default() as u16,
        capabilities: payload["capabilities"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        state: "CONNECTED".into(),
    })
}

fn hello_payload(core: &LocalShareCore) -> serde_json::Value {
    serde_json::json!({"deviceId":core.identity().id,"deviceName":core.identity().name,"fingerprint":core.identity().fingerprint,"port":core.status().listener_port,"versions":[protocol::VERSION],"capabilities":["chat","files","resume"]})
}

async fn run_session<S>(
    core: Arc<LocalShareCore>,
    stream: S,
    peer_id: String,
    sender: mpsc::Sender<protocol::Frame>,
    mut outbound: mpsc::Receiver<protocol::Frame>,
    mut close_receiver: oneshot::Receiver<()>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    let writer_core = core.clone();
    let writer_task = tokio::spawn(async move {
        let mut heartbeat =
            time::interval_at(Instant::now() + HEARTBEAT_INTERVAL, HEARTBEAT_INTERVAL);
        loop {
            tokio::select! {
                frame = outbound.recv() => match frame {
                    Some(frame) => protocol::write_frame(&mut writer, &frame).await?,
                    None => return Ok::<(), anyhow::Error>(()),
                },
                _ = heartbeat.tick() => {
                    writer_core.emit(CoreEvent::Heartbeat { direction: "ping".into(), acknowledged: false });
                    protocol::write_frame(&mut writer, &protocol::frame("heartbeat", serde_json::json!({}))).await?;
                },
                _ = &mut close_receiver => {
                    let _ = writer.shutdown().await;
                    return Ok::<(), anyhow::Error>(());
                }
            }
        }
    });
    loop {
        match protocol::read_frame(&mut reader).await {
            Ok(frame) if frame.kind == "heartbeat" => {
                core.emit(CoreEvent::Heartbeat {
                    direction: "pong".into(),
                    acknowledged: true,
                });
                sender
                    .send(protocol::frame("heartbeat.ack", serde_json::json!({})))
                    .await?;
            }
            Ok(frame) if frame.kind == "heartbeat.ack" => {
                core.emit(CoreEvent::Heartbeat {
                    direction: "ack".into(),
                    acknowledged: true,
                });
            }
            Ok(frame) if frame.kind == "chat.message" => {
                let body = frame.payload["body"].as_str().unwrap_or_default();
                let message_id = frame.payload["messageId"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| frame.id.to_string());
                core.store().save_message(
                    &message_id,
                    &peer_id,
                    body,
                    "in",
                    "DELIVERED",
                    frame.timestamp,
                )?;
                core.emit(CoreEvent::Chat(crate::ChatMessage {
                    id: message_id.clone(),
                    peer_id: peer_id.clone(),
                    body: body.into(),
                    direction: "in".into(),
                    state: "DELIVERED".into(),
                    timestamp: frame.timestamp,
                }));
                sender
                    .send(protocol::frame(
                        "chat.ack",
                        serde_json::json!({"messageId":message_id}),
                    ))
                    .await?;
            }
            Ok(frame) if frame.kind == "pair.request" => {
                let fingerprint = frame.payload["fingerprint"].as_str().unwrap_or_default();
                if fingerprint
                    != core
                        .store()
                        .peer(&peer_id)?
                        .map(|peer| peer.fingerprint)
                        .unwrap_or_default()
                {
                    bail!("pair request identity mismatch")
                }
                if let Some(peer) = core.store().peer(&peer_id)? {
                    core.emit(CoreEvent::Peer(Peer {
                        state: "PAIRING".into(),
                        ..peer
                    }));
                }
            }
            Ok(frame) if frame.kind == "pair.response" => {
                if frame.payload["accepted"].as_bool().unwrap_or(false) {
                    if let Some(peer) = core.store().peer(&peer_id)? {
                        let fingerprint = frame.payload["fingerprint"]
                            .as_str()
                            .unwrap_or(&peer.fingerprint);
                        if fingerprint != peer.fingerprint {
                            bail!("pair response identity mismatch")
                        }
                        core.store().trust_peer(&peer)?;
                        core.emit(CoreEvent::Peer(Peer {
                            state: "TRUSTED".into(),
                            ..peer
                        }));
                    }
                }
            }
            Ok(frame) if frame.kind == "chat.ack" => {
                let message_id = frame.payload["messageId"].as_str().unwrap_or_default();
                core.store().update_message_state(message_id, "DELIVERED")?;
                core.emit(CoreEvent::Chat(crate::ChatMessage {
                    id: message_id.into(),
                    peer_id: peer_id.clone(),
                    body: String::new(),
                    direction: "out".into(),
                    state: "DELIVERED".into(),
                    timestamp: frame.timestamp,
                }));
            }
            Ok(frame) if frame.kind == "control.test" => {
                let message_id = frame.payload["messageId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                core.emit(crate::CoreEvent::Control {
                    kind: "received".into(),
                    message_id: message_id.clone(),
                });
                sender
                    .send(protocol::frame(
                        "control.ack",
                        serde_json::json!({"messageId": message_id}),
                    ))
                    .await?;
            }
            Ok(frame) if frame.kind == "control.ack" => {
                core.emit(crate::CoreEvent::Control {
                    kind: "ack".into(),
                    message_id: frame.payload["messageId"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                });
            }
            Ok(frame) if frame.kind == "file.offer" => {
                crate::transfer::receive_offer(&core, &peer_id, &frame.payload).await?;
            }
            Ok(frame) if frame.kind == "file.accept" => {
                let transfer_id = frame.payload["transferId"]
                    .as_str()
                    .ok_or_else(|| anyhow!("file accept missing transferId"))?
                    .to_owned();
                let offset = frame.payload["offset"].as_u64().unwrap_or(0);
                let send_core = core.clone();
                tokio::spawn(async move {
                    if let Err(error) =
                        crate::transfer::send_chunks(send_core.clone(), &transfer_id, offset).await
                    {
                        send_core.emit(CoreEvent::Error(format!("file send failed: {error}")));
                    }
                });
            }
            Ok(frame) if frame.kind == "file.chunk" => {
                crate::transfer::receive_chunk(&core, &frame.payload).await?;
            }
            Ok(frame) if frame.kind == "file.complete" => {
                let transfer_id = frame.payload["transferId"]
                    .as_str()
                    .ok_or_else(|| anyhow!("file complete missing transferId"))?;
                crate::transfer::complete(&core, transfer_id).await?;
                sender
                    .send(protocol::frame(
                        "file.complete.ack",
                        serde_json::json!({"transferId": transfer_id}),
                    ))
                    .await?;
            }
            Ok(frame) if frame.kind == "file.complete.ack" => {
                let transfer_id = frame.payload["transferId"]
                    .as_str()
                    .ok_or_else(|| anyhow!("file complete ack missing transferId"))?;
                if let Some(stored) = core.store().transfer(transfer_id)? {
                    core.store()
                        .complete_transfer(transfer_id, stored.size, &stored.sha256)?;
                    core.emit(crate::CoreEvent::Transfer(crate::TransferEvent {
                        id: transfer_id.into(),
                        peer_id: stored.peer_id,
                        name: stored.name,
                        size: stored.size,
                        bytes: stored.size,
                        state: "COMPLETED".into(),
                        verified: true,
                        error: None,
                        direction: stored.direction,
                    }));
                }
            }
            Ok(frame) if frame.kind == "file.cancel" => {
                let transfer_id = frame.payload["transferId"]
                    .as_str()
                    .ok_or_else(|| anyhow!("file cancel missing transferId"))?;
                let reason = frame.payload["reason"].as_str().unwrap_or("cancelled");
                let direction = core
                    .store()
                    .transfer(transfer_id)?
                    .map(|transfer| transfer.direction)
                    .unwrap_or_else(|| "receive".into());
                core.outgoing_transfers.lock().await.remove(transfer_id);
                core.store().cancel_transfer(transfer_id, reason)?;
                core.emit(crate::CoreEvent::Transfer(crate::TransferEvent {
                    id: transfer_id.into(),
                    peer_id: peer_id.clone(),
                    name: String::new(),
                    size: 0,
                    bytes: 0,
                    state: "CANCELLED".into(),
                    verified: false,
                    error: Some(reason.into()),
                    direction,
                }));
            }
            Ok(_) => {}
            Err(error) => {
                writer_task.abort();
                return Err(error);
            }
        }
    }
}

#[derive(Debug)]
struct InsecureVerifier;
impl ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::ED25519,
        ]
    }
}

#[derive(Debug)]
struct InsecureClientVerifier;

impl ClientCertVerifier for InsecureClientVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> std::result::Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::ED25519,
        ]
    }
}

fn identity_certificates(
    identity: &crate::identity::Identity,
) -> Result<Vec<CertificateDer<'static>>> {
    Ok(
        rustls_pemfile::certs(&mut identity.certificate_pem.as_bytes())
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn identity_private_key(identity: &crate::identity::Identity) -> Result<PrivateKeyDer<'static>> {
    let key = rustls_pemfile::pkcs8_private_keys(&mut identity.private_key_pem.as_bytes())
        .next()
        .ok_or_else(|| anyhow!("missing private key"))??;
    Ok(PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)))
}

fn server_certificate_fingerprint(
    stream: &tokio_rustls::server::TlsStream<TcpStream>,
) -> Result<String> {
    let (_, connection) = stream.get_ref();
    let cert = connection
        .peer_certificates()
        .and_then(|certs| certs.first())
        .ok_or_else(|| anyhow!("peer did not present a client certificate"))?;
    Ok(hex::encode(Sha256::digest(cert.as_ref())))
}

fn certificate_fingerprint(stream: &tokio_rustls::client::TlsStream<TcpStream>) -> Result<String> {
    let (_, connection) = stream.get_ref();
    let cert = connection
        .peer_certificates()
        .and_then(|certs| certs.first())
        .ok_or_else(|| anyhow!("peer did not present a certificate"))?;
    Ok(hex::encode(Sha256::digest(cert.as_ref())))
}

pub async fn connect_manual(
    core: Arc<LocalShareCore>,
    host: &str,
    port: u16,
    expected_fingerprint: Option<&str>,
) -> Result<()> {
    let stream = TcpStream::connect((host, port)).await?;
    let client_certificates = identity_certificates(core.identity())?;
    let client_key = identity_private_key(core.identity())?;
    let cfg = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(InsecureVerifier))
        .with_client_auth_cert(client_certificates, client_key)?;
    let connector = TlsConnector::from(Arc::new(cfg));
    let name = ServerName::try_from("localshare").map_err(|_| anyhow!("bad server name"))?;
    let mut tls = connector.connect(name, stream).await?;
    let fingerprint = certificate_fingerprint(&tls)?;
    if let Some(expected) = expected_fingerprint {
        if expected != fingerprint {
            bail!("peer fingerprint mismatch")
        }
    }
    protocol::write_frame(&mut tls, &protocol::frame("hello", hello_payload(&core))).await?;
    let hello = protocol::read_frame(&mut tls).await?;
    if hello.kind != "hello" {
        bail!("expected hello response")
    }
    let peer = peer_from_hello(&hello.payload, host.into())?;
    if core.store().is_blocked(&peer.id)? {
        bail!("peer is blocked")
    }
    if let Some(trusted) = core.store().trusted_fingerprint(&peer.id)? {
        if trusted != peer.fingerprint {
            bail!("device identity changed; re-pair required")
        }
    }
    let peer = Peer {
        state: if core.store().trusted_fingerprint(&peer.id)?.as_deref()
            == Some(peer.fingerprint.as_str())
        {
            "TRUSTED"
        } else {
            "UNTRUSTED"
        }
        .into(),
        ..peer
    };
    let peer_id = peer.id.clone();
    let (sender, receiver) = mpsc::channel(64);
    let (close_sender, close_receiver) = oneshot::channel();
    if !core
        .register_session(peer_id.clone(), sender.clone(), close_sender)
        .await
    {
        bail!("duplicate peer session")
    }
    core.store().save_peer(&peer)?;
    core.emit(CoreEvent::Peer(peer));
    let session_core = core.clone();
    let session_peer_id = peer_id.clone();
    let queued_core = session_core.clone();
    tokio::spawn(async move {
        let _ = crate::chat::flush_queued(queued_core, &session_peer_id).await;
    });
    let resume_core = core.clone();
    let resume_peer_id = peer_id.clone();
    tokio::spawn(async move {
        let _ = crate::transfer::resume_outgoing(resume_core, &resume_peer_id).await;
    });
    tokio::spawn(async move {
        let _ = run_session(
            session_core.clone(),
            tls,
            peer_id.clone(),
            sender,
            receiver,
            close_receiver,
        )
        .await;
        session_core.unregister_session(&peer_id).await;
    });
    Ok(())
}

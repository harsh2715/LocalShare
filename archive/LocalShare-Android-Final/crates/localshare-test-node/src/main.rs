use anyhow::Result;
use clap::Parser;
use localshare_core::{LocalShareCore, NodeConfig};
use serde::Deserialize;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

#[derive(Debug, Deserialize)]
struct ConnectRequest {
    host: String,
    port: u16,
    fingerprint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    peer_id: String,
    body: String,
}

#[derive(Debug, Deserialize)]
struct ControlRequest {
    peer_id: String,
    message_id: String,
}

#[derive(Debug, Deserialize)]
struct FileOfferRequest {
    peer_id: String,
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct FileAcceptRequest {
    transfer_id: String,
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "virtual")]
    profile: String,
    #[arg(long, default_value = "Virtual-Device")]
    device_name: String,
    #[arg(long)]
    data_dir: PathBuf,
    #[arg(long)]
    download_dir: PathBuf,
    #[arg(long)]
    temp_dir: Option<PathBuf>,
    #[arg(long, default_value = "127.0.0.1")]
    bind_host: String,
    #[arg(long, default_value_t = 33201)]
    port: u16,
    #[arg(long)]
    peer_fingerprint: Option<String>,
    #[arg(long)]
    health_port: Option<u16>,
    /// Keep the node alive without requiring an interactive stdin session.
    #[arg(long, default_value_t = false)]
    daemon: bool,
    #[arg(long, default_value_t = false)]
    no_discovery: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let temp_dir = args.temp_dir.unwrap_or_else(|| args.data_dir.join("temp"));
    let config = NodeConfig {
        profile: args.profile,
        device_name: args.device_name,
        data_dir: args.data_dir,
        download_dir: args.download_dir,
        temp_dir,
        bind_host: args.bind_host,
        port: args.port,
        ask_before_receiving: true,
        secure_identity: false,
        enable_discovery: !args.no_discovery,
    };
    let core = LocalShareCore::start(config).await?;
    println!("{}", serde_json::to_string(&core.status())?);
    if let Some(health_port) = args.health_port {
        let health_core = core.clone();
        tokio::spawn(async move {
            if let Ok(listener) = tokio::net::TcpListener::bind(("127.0.0.1", health_port)).await {
                loop {
                    let Ok((mut stream, _)) = listener.accept().await else {
                        break;
                    };
                    let mut request = Vec::with_capacity(16 * 1024);
                    let mut chunk = [0_u8; 4096];
                    loop {
                        let read = stream.read(&mut chunk).await.unwrap_or(0);
                        if read == 0 {
                            break;
                        }
                        request.extend_from_slice(&chunk[..read]);
                        if let Some(header_end) =
                            request.windows(4).position(|window| window == b"\r\n\r\n")
                        {
                            let headers = std::str::from_utf8(&request[..header_end]).unwrap_or("");
                            let content_length = headers
                                .lines()
                                .find_map(|line| {
                                    line.strip_prefix("Content-Length:")
                                        .and_then(|value| value.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if request.len() >= header_end + 4 + content_length {
                                break;
                            }
                        }
                        if request.len() >= 16 * 1024 {
                            break;
                        }
                    }
                    let (method, path, request_body) = parse_http_request(&request);
                    let (status_code, payload) = if method == "POST" && path == "/connect" {
                        match serde_json::from_slice::<ConnectRequest>(request_body) {
                            Ok(connect) => match health_core
                                .connect_manual(
                                    &connect.host,
                                    connect.port,
                                    connect.fingerprint.as_deref(),
                                )
                                .await
                            {
                                Ok(()) => (200, serde_json::json!({"connected": true})),
                                Err(error) => (
                                    502,
                                    serde_json::json!({"connected": false, "error": error.to_string()}),
                                ),
                            },
                            Err(error) => (
                                400,
                                serde_json::json!({"connected": false, "error": error.to_string()}),
                            ),
                        }
                    } else if method == "POST" && path == "/chat" {
                        match serde_json::from_slice::<ChatRequest>(request_body) {
                            Ok(chat) => match localshare_core::chat::send(
                                health_core.clone(),
                                &chat.peer_id,
                                &chat.body,
                            )
                            .await
                            {
                                Ok(message) => (200, serde_json::json!({"message": message})),
                                Err(error) => (
                                    502,
                                    serde_json::json!({"sent": false, "error": error.to_string()}),
                                ),
                            },
                            Err(error) => (
                                400,
                                serde_json::json!({"sent": false, "error": error.to_string()}),
                            ),
                        }
                    } else if method == "POST" && path == "/pair/request" {
                        match serde_json::from_slice::<serde_json::Value>(request_body)
                            .ok()
                            .and_then(|body| body["peer_id"].as_str().map(str::to_owned))
                        {
                            Some(peer_id) => match health_core.request_pair(&peer_id).await {
                                Ok(()) => (200, serde_json::json!({"requested": true})),
                                Err(error) => (
                                    502,
                                    serde_json::json!({"requested": false, "error": error.to_string()}),
                                ),
                            },
                            None => (
                                400,
                                serde_json::json!({"requested": false, "error": "peer_id is required"}),
                            ),
                        }
                    } else if method == "POST" && path == "/pair/accept" {
                        match serde_json::from_slice::<serde_json::Value>(request_body)
                            .ok()
                            .and_then(|body| body["peer_id"].as_str().map(str::to_owned))
                        {
                            Some(peer_id) => match health_core.accept_pair(&peer_id).await {
                                Ok(()) => (200, serde_json::json!({"accepted": true})),
                                Err(error) => (
                                    502,
                                    serde_json::json!({"accepted": false, "error": error.to_string()}),
                                ),
                            },
                            None => (
                                400,
                                serde_json::json!({"accepted": false, "error": "peer_id is required"}),
                            ),
                        }
                    } else if method == "POST" && path == "/pair/reject" {
                        match serde_json::from_slice::<serde_json::Value>(request_body)
                            .ok()
                            .and_then(|body| body["peer_id"].as_str().map(str::to_owned))
                        {
                            Some(peer_id) => match health_core.reject_pair(&peer_id).await {
                                Ok(()) => (200, serde_json::json!({"rejected": true})),
                                Err(error) => (
                                    502,
                                    serde_json::json!({"rejected": false, "error": error.to_string()}),
                                ),
                            },
                            None => (
                                400,
                                serde_json::json!({"rejected": false, "error": "peer_id is required"}),
                            ),
                        }
                    } else if method == "POST" && path == "/peer/forget" {
                        match serde_json::from_slice::<serde_json::Value>(request_body)
                            .ok()
                            .and_then(|body| body["peer_id"].as_str().map(str::to_owned))
                        {
                            Some(peer_id) => match health_core.forget_peer(&peer_id) {
                                Ok(()) => (200, serde_json::json!({"forgotten": true})),
                                Err(error) => (
                                    502,
                                    serde_json::json!({"forgotten": false, "error": error.to_string()}),
                                ),
                            },
                            None => (
                                400,
                                serde_json::json!({"forgotten": false, "error": "peer_id is required"}),
                            ),
                        }
                    } else if method == "POST" && path == "/peer/block" {
                        match serde_json::from_slice::<serde_json::Value>(request_body)
                            .ok()
                            .and_then(|body| {
                                Some((
                                    body["peer_id"].as_str()?.to_owned(),
                                    body["blocked"].as_bool()?,
                                ))
                            }) {
                            Some((peer_id, blocked)) => {
                                match health_core.block_peer(&peer_id, blocked) {
                                    Ok(()) => (200, serde_json::json!({"blocked": blocked})),
                                    Err(error) => (
                                        502,
                                        serde_json::json!({"updated": false, "error": error.to_string()}),
                                    ),
                                }
                            }
                            None => (
                                400,
                                serde_json::json!({"updated": false, "error": "peer_id and blocked are required"}),
                            ),
                        }
                    } else if method == "GET" && path == "/state" {
                        let status = health_core.status();
                        match (health_core.store().list_peers(), health_core.transfers()) {
                            (Ok(peers), Ok(transfers)) => (
                                200,
                                serde_json::json!({"status": status, "peers": peers, "transfers": transfers}),
                            ),
                            (Err(error), _) | (_, Err(error)) => {
                                (500, serde_json::json!({"error": error.to_string()}))
                            }
                        }
                    } else if method == "GET" && path.starts_with("/messages/") {
                        let peer_id = path.trim_start_matches("/messages/");
                        match health_core.messages(peer_id) {
                            Ok(messages) => (200, serde_json::json!({"messages": messages})),
                            Err(error) => (500, serde_json::json!({"error": error.to_string()})),
                        }
                    } else if method == "POST" && path == "/disconnect" {
                        match serde_json::from_slice::<serde_json::Value>(request_body)
                            .ok()
                            .and_then(|body| body["peer_id"].as_str().map(str::to_owned))
                        {
                            Some(peer_id) => (
                                200,
                                serde_json::json!({"disconnected": health_core.disconnect_peer(&peer_id).await}),
                            ),
                            None => (400, serde_json::json!({"disconnected": false})),
                        }
                    } else if method == "POST" && path == "/control" {
                        match serde_json::from_slice::<ControlRequest>(request_body) {
                            Ok(control) => match health_core
                                .send_frame(
                                    &control.peer_id,
                                    localshare_core::protocol::frame(
                                        "control.test",
                                        serde_json::json!({"messageId": control.message_id}),
                                    ),
                                )
                                .await
                            {
                                Ok(()) => (200, serde_json::json!({"sent": true})),
                                Err(error) => (
                                    502,
                                    serde_json::json!({"sent": false, "error": error.to_string()}),
                                ),
                            },
                            Err(error) => (
                                400,
                                serde_json::json!({"sent": false, "error": error.to_string()}),
                            ),
                        }
                    } else if method == "POST" && path == "/file/offer" {
                        match serde_json::from_slice::<FileOfferRequest>(request_body) {
                            Ok(offer) => {
                                match health_core.offer_file(&offer.peer_id, &offer.path).await {
                                    Ok(transfer_id) => {
                                        (200, serde_json::json!({"transfer_id": transfer_id}))
                                    }
                                    Err(error) => (
                                        502,
                                        serde_json::json!({"offered": false, "error": error.to_string()}),
                                    ),
                                }
                            }
                            Err(error) => (
                                400,
                                serde_json::json!({"offered": false, "error": error.to_string()}),
                            ),
                        }
                    } else if method == "POST" && path == "/file/accept" {
                        match serde_json::from_slice::<FileAcceptRequest>(request_body) {
                            Ok(accept) => {
                                match health_core.accept_file(&accept.transfer_id).await {
                                    Ok(()) => (200, serde_json::json!({"accepted": true})),
                                    Err(error) => (
                                        502,
                                        serde_json::json!({"accepted": false, "error": error.to_string()}),
                                    ),
                                }
                            }
                            Err(error) => (
                                400,
                                serde_json::json!({"accepted": false, "error": error.to_string()}),
                            ),
                        }
                    } else {
                        let status = health_core.status();
                        (
                            200,
                            serde_json::json!({
                                "running": status.running,
                                "deviceId": status.device_id,
                                "deviceName": status.device_name,
                                "listener": {
                                    "host": status.listener_host,
                                    "port": status.listener_port,
                                    "ready": status.listener_ready
                                },
                                "database": if status.database_ready { "ready" } else { "not_ready" },
                                "identity": if status.identity_ready { "ready" } else { "not_ready" },
                                "peerCount": status.peer_count
                            }),
                        )
                    };
                    let body = serde_json::to_vec(&payload).unwrap_or_default();
                    let mut response = format!("HTTP/1.1 {status_code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
                    response.extend_from_slice(&body);
                    use tokio::io::AsyncWriteExt;
                    let _ = stream.write_all(&response).await;
                    let _ = stream.shutdown().await;
                }
            }
        });
    }
    let mut events = core.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            let event = match event {
                localshare_core::CoreEvent::Chat(mut message) => {
                    message.body = format!("[redacted:{} bytes]", message.body.len());
                    localshare_core::CoreEvent::Chat(message)
                }
                other => other,
            };
            println!(
                "{}",
                serde_json::to_string(&event)
                    .unwrap_or_else(|_| "{\"event\":\"Error\",\"data\":\"serialization\"}".into())
            );
        }
    });
    if args.daemon {
        tokio::signal::ctrl_c().await?;
        return Ok(());
    }
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    while let Some(line) = lines.next_line().await? {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("status") => println!("{}", serde_json::to_string(&core.status())?),
            Some("connect") => {
                let host = parts.next().unwrap_or("127.0.0.1");
                let port = parts.next().unwrap_or("33202").parse()?;
                match core
                    .connect_manual(host, port, args.peer_fingerprint.as_deref())
                    .await
                {
                    Ok(()) => println!("{}", "{\"event\":\"connected\"}"),
                    Err(e) => println!(
                        "{}",
                        serde_json::json!({"event":"error","data":e.to_string()})
                    ),
                }
            }
            Some("quit") => break,
            Some(_) | None => println!("{}", "{\"event\":\"error\",\"data\":\"unknown command\"}"),
        }
    }
    Ok(())
}

fn parse_http_request(request: &[u8]) -> (&str, &str, &[u8]) {
    let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
        return ("", "", &[]);
    };
    let headers = std::str::from_utf8(&request[..header_end]).unwrap_or("");
    let mut first_line = headers.lines().next().unwrap_or("").split_whitespace();
    let method = first_line.next().unwrap_or("");
    let path = first_line.next().unwrap_or("");
    (method, path, &request[header_end + 4..])
}

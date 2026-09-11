use localshare_core::chat;
use localshare_core::{LocalShareCore, NodeConfig};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::str::FromStr;
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

struct AppState {
    core: std::sync::Arc<tokio::sync::Mutex<Option<std::sync::Arc<LocalShareCore>>>>,
    startup_error: std::sync::Arc<tokio::sync::Mutex<Option<String>>>,
}

#[tauri::command]
async fn get_state(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let guard = state.core.lock().await;
    let Some(core) = guard.as_ref() else {
        let error = state.startup_error.lock().await.clone();
        return Err(error.unwrap_or_else(|| "LocalShare core is still starting".into()));
    };
    state_value(core).map_err(|error| {
        eprintln!("LocalShare get_state failed: {error:#}");
        error.to_string()
    })
}

fn state_value(core: &LocalShareCore) -> anyhow::Result<serde_json::Value> {
    let status = core.status();
    let interfaces = local_interfaces(&core.config().bind_host, status.listener_port);
    let peers = core
        .store()
        .list_peers()?
        .into_iter()
        .map(|peer| {
            serde_json::json!({
                "id": peer.id,
                "name": peer.name,
                "platform": std::env::consts::OS,
                "host": peer.host,
                "port": peer.port,
                "state": peer.state,
                "fingerprint": peer.fingerprint,
                "capabilities": peer.capabilities
            })
        })
        .collect::<Vec<_>>();
    let transfers = core
        .transfers()?
        .into_iter()
        .map(|transfer| {
            serde_json::json!({
                "id": transfer.id,
                "peerId": transfer.peer_id,
                "direction": transfer.direction,
                "name": transfer.name,
                "size": transfer.size,
                "bytesTransferred": transfer.bytes,
                "state": transfer.state,
                "speed": 0,
                "verified": transfer.verified,
                "error": transfer.error
            })
        })
        .collect::<Vec<_>>();
    let device_name = core
        .setting("deviceName")?
        .unwrap_or_else(|| core.identity().name.clone());
    Ok(serde_json::json!({
        "self": {"id": core.identity().id, "name": device_name, "fingerprint": core.identity().fingerprint, "platform": std::env::consts::OS},
        "peers": peers,
        "transfers": transfers,
        "settings": {"deviceName": device_name, "discoverable": true, "downloadFolder": core.config().download_dir, "askBeforeReceiving": core.config().ask_before_receiving, "networkInterface": "Automatic", "transferConcurrency": 2},
        "diagnostics": {"interfaces": interfaces, "listenerPort": status.listener_port, "mdns": if status.discovery_ready {"ready"} else {"disabled"}, "peerCount": status.peer_count, "protocolVersion": localshare_core::protocol::VERSION, "internet": "unknown"}
    }))
}

fn local_interfaces(bind_host: &str, listener_port: u16) -> Vec<serde_json::Value> {
    let address = if bind_host == "0.0.0.0" || bind_host == "::" {
        UdpSocket::bind("0.0.0.0:0")
            .and_then(|socket| {
                socket.connect("192.0.2.1:80")?;
                socket.local_addr()
            })
            .map(|local| local.ip().to_string())
            .unwrap_or_else(|_| bind_host.to_owned())
    } else {
        bind_host.to_owned()
    };
    let internal = address == "127.0.0.1" || address == "::1";
    vec![serde_json::json!({
        "name": "LocalShare listener",
        "address": format!("{address}:{listener_port}"),
        "family": if address.contains(':') { "ipv6" } else { "ipv4" },
        "internal": internal
    })]
}

#[tauri::command]
async fn connect_manual(
    state: State<'_, AppState>,
    host: String,
    port: u16,
    fingerprint: String,
) -> Result<(), String> {
    if fingerprint.trim().is_empty() {
        return Err("peer certificate fingerprint is required".into());
    }
    let guard = state.core.lock().await;
    guard
        .as_ref()
        .ok_or("core not started")?
        .connect_manual(&host, port, Some(fingerprint.trim()))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_peers(state: State<'_, AppState>) -> Result<Vec<localshare_core::Peer>, String> {
    let guard = state.core.lock().await;
    Ok(guard
        .as_ref()
        .ok_or("core not started")?
        .store()
        .list_peers()
        .map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn pair(peer_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.core.lock().await;
    guard
        .as_ref()
        .ok_or("core not started")?
        .accept_pair(&peer_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn forget_peer(peer_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.core.lock().await;
    guard
        .as_ref()
        .ok_or("core not started")?
        .forget_peer(&peer_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn block_peer(
    peer_id: String,
    blocked: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let guard = state.core.lock().await;
    guard
        .as_ref()
        .ok_or("core not started")?
        .block_peer(&peer_id, blocked)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn send_message(
    peer_id: String,
    body: String,
    state: State<'_, AppState>,
) -> Result<localshare_core::ChatMessage, String> {
    let core = { state.core.lock().await.clone().ok_or("core not started")? };
    chat::send(core, &peer_id, &body)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_messages(
    peer_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<localshare_core::ChatMessage>, String> {
    let guard = state.core.lock().await;
    guard
        .as_ref()
        .ok_or("core not started")?
        .messages(&peer_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn send_file(
    peer_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let core = { state.core.lock().await.clone().ok_or("core not started")? };
    core.offer_file(&peer_id, &PathBuf::from(path))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn accept_transfer(transfer_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.core.lock().await;
    guard
        .as_ref()
        .ok_or("core not started")?
        .accept_file(&transfer_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn decline_transfer(transfer_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.core.lock().await;
    guard
        .as_ref()
        .ok_or("core not started")?
        .decline_file(&transfer_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_transfers(
    state: State<'_, AppState>,
) -> Result<Vec<localshare_core::TransferEvent>, String> {
    let guard = state.core.lock().await;
    guard
        .as_ref()
        .ok_or("core not started")?
        .transfers()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn choose_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    Ok(app
        .dialog()
        .file()
        .set_title("Choose files to send")
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .map(|path| path.to_string())
        .collect())
}

fn selected_file_name(location: &str, index: usize) -> String {
    let decoded = percent_encoding::percent_decode_str(location)
        .decode_utf8_lossy();
    let candidate = decoded
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or_default()
        .trim();
    let sanitized = candidate
        .chars()
        .map(|character| {
            if character.is_control() || matches!(character, '/' | '\\') {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        format!("shared-file-{}", index + 1)
    } else {
        sanitized
    }
}

#[tauri::command]
async fn send_files(
    peer_id: String,
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let core = state.core.lock().await.clone().ok_or("core not started")?;
    let mut ids = Vec::with_capacity(paths.len());
    for (index, location) in paths.into_iter().enumerate() {
        let file_path = FilePath::from_str(&location).map_err(|error| error.to_string())?;
        let path = match file_path {
            FilePath::Path(path) => path,
            selected @ FilePath::Url(_) => {
                let upload_dir = core
                    .config()
                    .temp_dir
                    .join("outgoing")
                    .join(uuid::Uuid::new_v4().to_string());
                std::fs::create_dir_all(&upload_dir).map_err(|error| error.to_string())?;
                let destination = upload_dir.join(selected_file_name(&location, index));
                let mut options = OpenOptions::new();
                options.read(true);
                let mut source = app
                    .fs()
                    .open(selected, options)
                    .map_err(|error| error.to_string())?;
                let mut output = std::fs::File::create(&destination)
                    .map_err(|error| error.to_string())?;
                std::io::copy(&mut source, &mut output).map_err(|error| error.to_string())?;
                destination
            }
        };
        ids.push(
            core.offer_file(&peer_id, &path)
                .await
                .map_err(|e| e.to_string())?,
        );
    }
    Ok(ids)
}

#[tauri::command]
async fn cancel_transfer(transfer_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.core.lock().await;
    let core = guard.as_ref().ok_or("core not started")?;
    core.store()
        .cancel_transfer(&transfer_id, "cancelled by user")
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn pause_transfer(_transfer_id: String) -> Result<(), String> {
    Err("pause is not yet available for this transfer".into())
}

#[tauri::command]
async fn resume_transfer(_transfer_id: String) -> Result<(), String> {
    Err("resume is not yet available for this transfer".into())
}

#[tauri::command]
async fn update_settings(
    settings: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let guard = state.core.lock().await;
    let core = guard.as_ref().ok_or("core not started")?;
    if let Some(name) = settings.get("deviceName").and_then(|v| v.as_str()) {
        core.set_setting("deviceName", name)
            .map_err(|e| e.to_string())?;
    }
    if let Some(folder) = settings.get("downloadFolder").and_then(|v| v.as_str()) {
        core.set_setting("downloadFolder", folder)
            .map_err(|e| e.to_string())?;
    }
    if let Some(ask) = settings.get("askBeforeReceiving").and_then(|v| v.as_bool()) {
        core.set_setting("askBeforeReceiving", &ask.to_string())
            .map_err(|e| e.to_string())?;
    }
    state_value(core).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            core: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            startup_error: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            connect_manual,
            get_peers,
            pair,
            forget_peer,
            block_peer,
            send_message,
            get_messages,
            send_file,
            accept_transfer,
            decline_transfer,
            get_transfers,
            update_settings,
            choose_files,
            send_files,
            cancel_transfer,
            pause_transfer,
            resume_transfer
        ])
        .setup(|app| {
            let app_data = app.path().app_data_dir().expect("app data");
            // Development builds must not compete with an installed release for the
            // same macOS Keychain item. Keep their identity and data isolated while
            // retaining Keychain-backed identity storage in release builds.
            let data = if cfg!(debug_assertions) {
                app_data.join("dev")
            } else {
                app_data
            };
            let config = NodeConfig {
                profile: if cfg!(debug_assertions) {
                    "desktop-dev".into()
                } else {
                    "desktop".into()
                },
                device_name: "LocalShare Device".into(),
                data_dir: data.clone(),
                download_dir: data.join("downloads"),
                temp_dir: data.join("temp"),
                bind_host: "0.0.0.0".into(),
                port: if cfg!(debug_assertions) { 0 } else { 32123 },
                ask_before_receiving: true,
                secure_identity: !cfg!(debug_assertions),
                enable_discovery: true,
            };
            let core_slot = app.state::<AppState>().core.clone();
            let startup_error = app.state::<AppState>().startup_error.clone();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(60),
                    LocalShareCore::start(config),
                )
                .await
                .map_err(|_| {
                    anyhow::anyhow!(
                        "startup timed out while initializing identity, storage, networking, or discovery"
                    )
                })
                .and_then(|result| result);
                match result {
                    Ok(core) => {
                        let mut events = core.subscribe();
                        *core_slot.lock().await = Some(core.clone());
                        while let Ok(_event) = events.recv().await {
                            if let Ok(value) = state_value(&core) {
                                let _ = app_handle.emit("core-state", value);
                            }
                        }
                    }
                    Err(error) => {
                        let message = format!("LocalShare core startup failed: {error:#}");
                        eprintln!("{message}");
                        *startup_error.lock().await = Some(message);
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running LocalShare");
}

#[cfg(not(mobile))]
fn main() {
    run();
}

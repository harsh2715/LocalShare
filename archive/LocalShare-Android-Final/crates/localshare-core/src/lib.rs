pub mod chat;
pub mod discovery;
pub mod identity;
pub mod protocol;
pub mod storage;
pub mod transfer;
pub mod transport;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU16, AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub profile: String,
    pub device_name: String,
    pub data_dir: PathBuf,
    pub download_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub bind_host: String,
    pub port: u16,
    pub ask_before_receiving: bool,
    pub secure_identity: bool,
    pub enable_discovery: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    pub running: bool,
    pub device_id: String,
    pub device_name: String,
    pub listener_host: String,
    pub listener_port: u16,
    pub listener_ready: bool,
    pub database_ready: bool,
    pub identity_ready: bool,
    pub peer_count: usize,
    pub discovery_ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum CoreEvent {
    Status(NodeStatus),
    Peer(Peer),
    Heartbeat {
        direction: String,
        acknowledged: bool,
    },
    Control {
        kind: String,
        message_id: String,
    },
    Chat(ChatMessage),
    Transfer(TransferEvent),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub protocol_version: u16,
    pub capabilities: Vec<String>,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub peer_id: String,
    pub body: String,
    pub direction: String,
    pub state: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferEvent {
    pub id: String,
    pub peer_id: String,
    pub name: String,
    pub size: u64,
    pub bytes: u64,
    pub state: String,
    pub verified: bool,
    pub error: Option<String>,
    pub direction: String,
}

pub struct LocalShareCore {
    config: NodeConfig,
    identity: identity::Identity,
    store: storage::Store,
    events: broadcast::Sender<CoreEvent>,
    listener_port: AtomicU16,
    active_sessions: AtomicUsize,
    sessions: Mutex<HashMap<String, SessionHandle>>,
    pub(crate) outgoing_transfers: Mutex<HashMap<String, transfer::OutgoingTransfer>>,
    pub(crate) pending_incoming: Mutex<HashMap<String, transfer::IncomingOffer>>,
    pub(crate) active_incoming: Mutex<HashMap<String, transfer::IncomingTransfer>>,
    discovery: Mutex<Option<discovery::DiscoveryHandle>>,
}

struct SessionHandle {
    sender: mpsc::Sender<protocol::Frame>,
    close: oneshot::Sender<()>,
}

impl LocalShareCore {
    pub async fn start(config: NodeConfig) -> Result<Arc<Self>> {
        std::fs::create_dir_all(&config.data_dir)?;
        std::fs::create_dir_all(&config.download_dir)?;
        std::fs::create_dir_all(&config.temp_dir)?;
        let identity_dir = config.data_dir.clone();
        let identity_name = config.device_name.clone();
        let identity_profile = config.profile.clone();
        let secure_identity = config.secure_identity;
        let identity = tokio::task::spawn_blocking(move || {
            identity::load_or_create(
                &identity_dir,
                &identity_name,
                secure_identity,
                &identity_profile,
            )
        })
        .await??;
        let store = storage::Store::open(&config.data_dir.join("localshare.sqlite"))?;
        let (events, _) = broadcast::channel(256);
        let core = Arc::new(Self {
            config,
            identity,
            store,
            events,
            listener_port: AtomicU16::new(0),
            active_sessions: AtomicUsize::new(0),
            sessions: Mutex::new(HashMap::new()),
            outgoing_transfers: Mutex::new(HashMap::new()),
            pending_incoming: Mutex::new(HashMap::new()),
            active_incoming: Mutex::new(HashMap::new()),
            discovery: Mutex::new(None),
        });
        transport::spawn_listener(core.clone()).await?;
        if core.config.enable_discovery {
            let handle = discovery::start(core.clone()).await?;
            *core.discovery.lock().await = Some(handle);
        }
        Ok(core)
    }
    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.events.subscribe()
    }
    pub fn status(&self) -> NodeStatus {
        NodeStatus {
            running: true,
            device_id: self.identity.id.clone(),
            device_name: self.identity.name.clone(),
            listener_host: self.config.bind_host.clone(),
            listener_port: self.listener_port.load(Ordering::Relaxed),
            listener_ready: self.listener_port.load(Ordering::Relaxed) != 0,
            database_ready: true,
            identity_ready: true,
            peer_count: self.active_sessions.load(Ordering::Relaxed),
            discovery_ready: self
                .discovery
                .try_lock()
                .ok()
                .and_then(|guard| guard.as_ref().map(|_| true))
                .unwrap_or(false),
        }
    }
    pub fn config(&self) -> &NodeConfig {
        &self.config
    }
    pub fn identity(&self) -> &identity::Identity {
        &self.identity
    }
    pub fn store(&self) -> &storage::Store {
        &self.store
    }
    pub fn emit(&self, event: CoreEvent) {
        let _ = self.events.send(event);
    }
    pub fn set_listener_port(&self, port: u16) {
        self.listener_port.store(port, Ordering::Relaxed);
    }
    pub async fn connect_manual(
        self: &Arc<Self>,
        host: &str,
        port: u16,
        expected_fingerprint: Option<&str>,
    ) -> Result<()> {
        transport::connect_manual(self.clone(), host, port, expected_fingerprint).await
    }
    pub fn is_peer_trusted(&self, peer_id: &str) -> Result<bool> {
        let Some(peer) = self.store.peer(peer_id)? else {
            return Ok(false);
        };
        Ok(self.store.trusted_fingerprint(peer_id)?.as_deref() == Some(peer.fingerprint.as_str()))
    }
    pub(crate) async fn register_session(
        &self,
        peer_id: String,
        sender: mpsc::Sender<protocol::Frame>,
        close: oneshot::Sender<()>,
    ) -> bool {
        let inserted = self
            .sessions
            .lock()
            .await
            .insert(peer_id, SessionHandle { sender, close })
            .is_none();
        if inserted {
            self.active_sessions.fetch_add(1, Ordering::Relaxed);
        }
        inserted
    }
    pub(crate) async fn unregister_session(&self, peer_id: &str) {
        if self.sessions.lock().await.remove(peer_id).is_some() {
            self.active_sessions.fetch_sub(1, Ordering::Relaxed);
            let _ = self.store.update_peer_state(peer_id, "OFFLINE");
        }
    }
    pub async fn send_frame(&self, peer_id: &str, frame: protocol::Frame) -> Result<()> {
        let sender = self
            .sessions
            .lock()
            .await
            .get(peer_id)
            .map(|session| session.sender.clone())
            .ok_or_else(|| anyhow::anyhow!("peer is not connected"))?;
        sender
            .send(frame)
            .await
            .map_err(|_| anyhow::anyhow!("peer session closed"))
    }
    pub async fn disconnect_peer(&self, peer_id: &str) -> bool {
        if let Some(session) = self.sessions.lock().await.remove(peer_id) {
            let _ = session.close.send(());
            self.active_sessions.fetch_sub(1, Ordering::Relaxed);
            let _ = self.store.update_peer_state(peer_id, "OFFLINE");
            true
        } else {
            false
        }
    }
    pub async fn offer_file(&self, peer_id: &str, path: &std::path::Path) -> Result<String> {
        transfer::offer(self, peer_id, path).await
    }
    pub async fn accept_file(&self, transfer_id: &str) -> Result<()> {
        transfer::accept(self, transfer_id).await
    }
    pub async fn decline_file(&self, transfer_id: &str) -> Result<()> {
        transfer::decline(self, transfer_id).await
    }
    pub async fn pair_peer(&self, peer_id: &str) -> Result<()> {
        let peer = self
            .store()
            .peer(peer_id)?
            .ok_or_else(|| anyhow::anyhow!("peer not found"))?;
        if self.store().is_blocked(peer_id)? {
            anyhow::bail!("peer is blocked")
        }
        self.store().trust_peer(&peer)?;
        self.emit(CoreEvent::Peer(Peer {
            state: "TRUSTED".into(),
            ..peer
        }));
        Ok(())
    }
    pub async fn request_pair(&self, peer_id: &str) -> Result<()> {
        let peer = self
            .store()
            .peer(peer_id)?
            .ok_or_else(|| anyhow::anyhow!("peer not found"))?;
        if self.store().is_blocked(peer_id)? {
            anyhow::bail!("peer is blocked");
        }
        self.send_frame(peer_id, protocol::frame("pair.request", serde_json::json!({"deviceId": self.identity.id, "deviceName": self.identity.name, "fingerprint": self.identity.fingerprint}))).await?;
        self.emit(CoreEvent::Peer(Peer {
            state: "PAIRING".into(),
            ..peer
        }));
        Ok(())
    }
    pub async fn accept_pair(&self, peer_id: &str) -> Result<()> {
        self.pair_peer(peer_id).await?;
        self.send_frame(
            peer_id,
            protocol::frame(
                "pair.response",
                serde_json::json!({"accepted": true, "fingerprint": self.identity.fingerprint}),
            ),
        )
        .await?;
        Ok(())
    }
    pub async fn reject_pair(&self, peer_id: &str) -> Result<()> {
        self.send_frame(
            peer_id,
            protocol::frame("pair.response", serde_json::json!({"accepted": false})),
        )
        .await?;
        Ok(())
    }
    pub fn forget_peer(&self, peer_id: &str) -> Result<()> {
        self.store().forget_peer(peer_id)?;
        Ok(())
    }
    pub fn block_peer(&self, peer_id: &str, blocked: bool) -> Result<()> {
        let peer = self
            .store()
            .peer(peer_id)?
            .ok_or_else(|| anyhow::anyhow!("peer not found"))?;
        self.store().set_blocked(&peer, blocked)?;
        Ok(())
    }
    pub fn messages(&self, peer_id: &str) -> Result<Vec<ChatMessage>> {
        self.store().messages_for(peer_id)
    }
    pub fn transfers(&self) -> Result<Vec<TransferEvent>> {
        self.store().transfer_rows()
    }
    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        self.store().setting(key)
    }
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.store().set_setting(key, value)
    }
}

impl Drop for LocalShareCore {
    fn drop(&mut self) {}
}

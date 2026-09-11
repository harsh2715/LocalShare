use crate::Peer;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Store {
    conn: std::sync::Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct StoredTransfer {
    pub id: String,
    pub peer_id: String,
    pub name: String,
    pub size: u64,
    pub bytes: u64,
    pub state: String,
    pub sha256: String,
    pub saved_name: Option<String>,
    pub local_path: Option<String>,
    pub temp_path: Option<String>,
    pub source_path: Option<String>,
    pub source_size: Option<u64>,
    pub source_modified: Option<i64>,
    pub direction: String,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS peers(id TEXT PRIMARY KEY,name TEXT NOT NULL,host TEXT NOT NULL,port INTEGER NOT NULL,fingerprint TEXT NOT NULL,state TEXT NOT NULL,last_seen INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS trusted_peers(peer_id TEXT PRIMARY KEY,fingerprint TEXT NOT NULL,name TEXT NOT NULL,paired_at INTEGER NOT NULL,last_seen INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS blocked_peers(peer_id TEXT PRIMARY KEY,fingerprint TEXT,name TEXT,blocked_at INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS messages(id TEXT PRIMARY KEY,peer_id TEXT NOT NULL,body TEXT NOT NULL,direction TEXT NOT NULL,state TEXT NOT NULL,timestamp INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS transfers(id TEXT PRIMARY KEY,peer_id TEXT NOT NULL,name TEXT NOT NULL,size INTEGER NOT NULL,bytes INTEGER NOT NULL,state TEXT NOT NULL,verified INTEGER NOT NULL DEFAULT 0,error TEXT,original_name TEXT,saved_name TEXT,sha256 TEXT,local_path TEXT,temp_path TEXT,source_path TEXT,source_size INTEGER,source_modified INTEGER,direction TEXT,created_at INTEGER,completed_at INTEGER); CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);")?;
        Self::migrate_transfer_columns(&conn)?;
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_transfers_sha256_size ON transfers(sha256,size); CREATE INDEX IF NOT EXISTS idx_transfers_peer ON transfers(peer_id);")?;
        Ok(Self {
            conn: std::sync::Mutex::new(conn),
        })
    }

    fn migrate_transfer_columns(conn: &Connection) -> Result<()> {
        let mut statement = conn.prepare("PRAGMA table_info(transfers)")?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (name, definition) in [
            ("original_name", "TEXT"),
            ("saved_name", "TEXT"),
            ("sha256", "TEXT"),
            ("local_path", "TEXT"),
            ("temp_path", "TEXT"),
            ("source_path", "TEXT"),
            ("source_size", "INTEGER"),
            ("source_modified", "INTEGER"),
            ("direction", "TEXT"),
            ("created_at", "INTEGER"),
            ("completed_at", "INTEGER"),
        ] {
            if !columns.iter().any(|column| column == name) {
                conn.execute(
                    &format!("ALTER TABLE transfers ADD COLUMN {name} {definition}"),
                    [],
                )?;
            }
        }
        Ok(())
    }
    pub fn peer_count(&self) -> usize {
        self.conn
            .lock()
            .ok()
            .and_then(|c| {
                c.query_row("SELECT COUNT(*) FROM peers", [], |r| r.get::<_, i64>(0))
                    .ok()
            })
            .unwrap_or(0) as usize
    }
    pub fn save_message(
        &self,
        id: &str,
        peer_id: &str,
        body: &str,
        direction: &str,
        state: &str,
        timestamp: i64,
    ) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT OR IGNORE INTO messages VALUES(?,?,?,?,?,?)",
            params![id, peer_id, body, direction, state, timestamp],
        )?;
        Ok(())
    }
    pub fn save_peer(&self, peer: &Peer) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO peers(id,name,host,port,fingerprint,state,last_seen) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,host=excluded.host,port=excluded.port,fingerprint=excluded.fingerprint,state=excluded.state,last_seen=excluded.last_seen",
            params![peer.id, peer.name, peer.host, peer.port, peer.fingerprint, peer.state, chrono::Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn update_peer_state(&self, peer_id: &str, state: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE peers SET state=?,last_seen=? WHERE id=?",
            params![state, now_millis(), peer_id],
        )?;
        Ok(())
    }

    pub fn peer(&self, peer_id: &str) -> Result<Option<Peer>> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,name,host,port,fingerprint,state FROM peers WHERE id=?",
                params![peer_id],
                |row| {
                    Ok(Peer {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        host: row.get(2)?,
                        port: row.get(3)?,
                        fingerprint: row.get(4)?,
                        protocol_version: crate::protocol::VERSION,
                        capabilities: Vec::new(),
                        state: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn trusted_fingerprint(&self, peer_id: &str) -> Result<Option<String>> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT fingerprint FROM trusted_peers WHERE peer_id=?",
                params![peer_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn is_blocked(&self, peer_id: &str) -> Result<bool> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT 1 FROM blocked_peers WHERE peer_id=?",
                params![peer_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    pub fn trust_peer(&self, peer: &Peer) -> Result<()> {
        let now = now_millis();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO trusted_peers(peer_id,fingerprint,name,paired_at,last_seen) VALUES(?,?,?,?,?) ON CONFLICT(peer_id) DO UPDATE SET fingerprint=excluded.fingerprint,name=excluded.name,last_seen=excluded.last_seen",
            params![peer.id, peer.fingerprint, peer.name, now, now],
        )?;
        conn.execute(
            "UPDATE peers SET state='TRUSTED',last_seen=? WHERE id=?",
            params![now, peer.id],
        )?;
        Ok(())
    }

    pub fn forget_peer(&self, peer_id: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "DELETE FROM trusted_peers WHERE peer_id=?",
            params![peer_id],
        )?;
        self.update_peer_state(peer_id, "UNTRUSTED")?;
        Ok(())
    }

    pub fn set_blocked(&self, peer: &Peer, blocked: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if blocked {
            conn.execute(
                "INSERT INTO blocked_peers(peer_id,fingerprint,name,blocked_at) VALUES(?,?,?,?) ON CONFLICT(peer_id) DO UPDATE SET fingerprint=excluded.fingerprint,name=excluded.name,blocked_at=excluded.blocked_at",
                params![peer.id, peer.fingerprint, peer.name, now_millis()],
            )?;
            conn.execute(
                "DELETE FROM trusted_peers WHERE peer_id=?",
                params![peer.id],
            )?;
            conn.execute(
                "UPDATE peers SET state='BLOCKED',last_seen=? WHERE id=?",
                params![now_millis(), peer.id],
            )?;
        } else {
            conn.execute(
                "DELETE FROM blocked_peers WHERE peer_id=?",
                params![peer.id],
            )?;
            conn.execute(
                "UPDATE peers SET state='UNTRUSTED',last_seen=? WHERE id=?",
                params![now_millis(), peer.id],
            )?;
        }
        Ok(())
    }

    pub fn list_peers(&self) -> Result<Vec<Peer>> {
        let conn = self.conn.lock().unwrap();
        let mut statement =
            conn.prepare("SELECT id,name,host,port,fingerprint,state FROM peers ORDER BY name")?;
        let peers = statement
            .query_map([], |row| {
                Ok(Peer {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    host: row.get(2)?,
                    port: row.get(3)?,
                    fingerprint: row.get(4)?,
                    protocol_version: crate::protocol::VERSION,
                    capabilities: Vec::new(),
                    state: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(peers)
    }

    pub fn messages_for(&self, peer_id: &str) -> Result<Vec<crate::ChatMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut statement = conn.prepare("SELECT id,peer_id,body,direction,state,timestamp FROM messages WHERE peer_id=? ORDER BY timestamp,id")?;
        let messages = statement
            .query_map(params![peer_id], |row| {
                Ok(crate::ChatMessage {
                    id: row.get(0)?,
                    peer_id: row.get(1)?,
                    body: row.get(2)?,
                    direction: row.get(3)?,
                    state: row.get(4)?,
                    timestamp: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(messages)
    }

    pub fn queued_messages(&self, peer_id: &str) -> Result<Vec<crate::ChatMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut statement = conn.prepare("SELECT id,peer_id,body,direction,state,timestamp FROM messages WHERE peer_id=? AND direction='out' AND state='QUEUED' ORDER BY timestamp,id")?;
        let messages = statement
            .query_map(params![peer_id], |row| {
                Ok(crate::ChatMessage {
                    id: row.get(0)?,
                    peer_id: row.get(1)?,
                    body: row.get(2)?,
                    direction: row.get(3)?,
                    state: row.get(4)?,
                    timestamp: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(messages)
    }

    pub fn transfer_rows(&self) -> Result<Vec<crate::TransferEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut statement = conn.prepare("SELECT id,peer_id,COALESCE(original_name,name),size,bytes,state,verified,error,COALESCE(direction,'receive') FROM transfers ORDER BY created_at,id")?;
        let transfers = statement
            .query_map([], |row| {
                Ok(crate::TransferEvent {
                    id: row.get(0)?,
                    peer_id: row.get(1)?,
                    name: row.get(2)?,
                    size: row.get::<_, i64>(3)? as u64,
                    bytes: row.get::<_, i64>(4)? as u64,
                    state: row.get(5)?,
                    verified: row.get::<_, i64>(6)? != 0,
                    error: row.get(7)?,
                    direction: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(transfers)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT value FROM settings WHERE key=?",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.lock().unwrap().execute("INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
        Ok(())
    }
    pub fn update_message_state(&self, id: &str, state: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("UPDATE messages SET state=? WHERE id=?", params![state, id])?;
        Ok(())
    }

    pub fn record_transfer_offer(
        &self,
        id: &str,
        peer_id: &str,
        original_name: &str,
        size: u64,
        sha256: &str,
        direction: &str,
    ) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO transfers(id,peer_id,name,size,bytes,state,verified,error,original_name,saved_name,sha256,local_path,direction,created_at,completed_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,NULL) ON CONFLICT(id) DO UPDATE SET sha256=excluded.sha256,original_name=excluded.original_name,direction=excluded.direction",
            params![id, peer_id, original_name, size as i64, 0_i64, "OFFERED", 0_i64, Option::<String>::None, original_name, Option::<String>::None, sha256, Option::<String>::None, direction, now_millis()],
        )?;
        Ok(())
    }

    pub fn set_transfer_destination(
        &self,
        id: &str,
        saved_name: &str,
        local_path: &Path,
        temp_path: &Path,
    ) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE transfers SET saved_name=?,local_path=?,temp_path=?,state='ACTIVE' WHERE id=?",
            params![
                saved_name,
                local_path.to_string_lossy().to_string(),
                temp_path.to_string_lossy().to_string(),
                id
            ],
        )?;
        Ok(())
    }

    pub fn set_transfer_source(
        &self,
        id: &str,
        source_path: &Path,
        source_size: u64,
        source_modified: i64,
    ) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE transfers SET source_path=?,source_size=?,source_modified=? WHERE id=?",
            params![
                source_path.to_string_lossy().to_string(),
                source_size as i64,
                source_modified,
                id
            ],
        )?;
        Ok(())
    }

    pub fn transfer(&self, id: &str) -> Result<Option<StoredTransfer>> {
        self.conn.lock().unwrap().query_row(
            "SELECT id,peer_id,COALESCE(original_name,name),size,bytes,state,COALESCE(sha256,''),saved_name,local_path,temp_path,source_path,source_size,source_modified,COALESCE(direction,'') FROM transfers WHERE id=?",
            params![id],
            |row| Ok(StoredTransfer {
                id: row.get(0)?, peer_id: row.get(1)?, name: row.get(2)?, size: row.get::<_, i64>(3)? as u64, bytes: row.get::<_, i64>(4)? as u64, state: row.get(5)?, sha256: row.get(6)?, saved_name: row.get(7)?, local_path: row.get(8)?, temp_path: row.get(9)?, source_path: row.get(10)?, source_size: row.get::<_, Option<i64>>(11)?.map(|v| v as u64), source_modified: row.get(12)?, direction: row.get(13)?,
            }),
        ).optional().map_err(Into::into)
    }

    pub fn incomplete_outgoing(&self, peer_id: &str) -> Result<Vec<StoredTransfer>> {
        let ids = {
            let conn = self.conn.lock().unwrap();
            let mut statement = conn.prepare("SELECT id FROM transfers WHERE peer_id=? AND direction='send' AND state NOT IN ('COMPLETED','CANCELLED')")?;
            let ids = statement
                .query_map(params![peer_id], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            ids
        };
        ids.into_iter()
            .map(|id| {
                self.transfer(&id)
                    .and_then(|row| row.ok_or_else(|| anyhow::anyhow!("transfer disappeared")))
            })
            .collect()
    }

    pub fn update_transfer_progress(&self, id: &str, bytes: u64) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE transfers SET bytes=?,state='ACTIVE' WHERE id=?",
            params![bytes as i64, id],
        )?;
        Ok(())
    }

    pub fn complete_transfer(&self, id: &str, bytes: u64, sha256: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE transfers SET bytes=?,sha256=?,state='COMPLETED',verified=1,completed_at=? WHERE id=?",
            params![bytes as i64, sha256, now_millis(), id],
        )?;
        Ok(())
    }

    pub fn fail_transfer(&self, id: &str, error: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE transfers SET state='FAILED',error=? WHERE id=?",
            params![error, id],
        )?;
        Ok(())
    }

    pub fn cancel_transfer(&self, id: &str, reason: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE transfers SET state='CANCELLED',error=? WHERE id=?",
            params![reason, id],
        )?;
        Ok(())
    }

    pub fn completed_candidates(&self, sha256: &str, size: u64) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut statement = conn.prepare("SELECT local_path FROM transfers WHERE state='COMPLETED' AND verified=1 AND sha256=? AND size=? AND local_path IS NOT NULL")?;
        let paths = statement
            .query_map(params![sha256, size as i64], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(paths)
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_and_block_state_are_persistent() {
        let path = std::env::temp_dir().join(format!(
            "localshare-storage-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let store = Store::open(&path).unwrap();
        let peer = Peer {
            id: "peer-a".into(),
            name: "Generated Peer".into(),
            host: "127.0.0.1".into(),
            port: 33301,
            fingerprint: "a".repeat(64),
            protocol_version: crate::protocol::VERSION,
            capabilities: vec![],
            state: "UNTRUSTED".into(),
        };
        store.save_peer(&peer).unwrap();
        store.trust_peer(&peer).unwrap();
        assert_eq!(
            store.trusted_fingerprint("peer-a").unwrap().as_deref(),
            Some(peer.fingerprint.as_str())
        );
        store.set_blocked(&peer, true).unwrap();
        assert!(store.is_blocked("peer-a").unwrap());
        assert_eq!(store.trusted_fingerprint("peer-a").unwrap(), None);
        store.set_blocked(&peer, false).unwrap();
        assert!(!store.is_blocked("peer-a").unwrap());
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }
}

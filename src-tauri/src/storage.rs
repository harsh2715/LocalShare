use rusqlite::{params, Connection};
use std::{fs, path::PathBuf};

pub fn open() -> Result<Connection, String> {
    let dir = dirs::data_local_dir().ok_or("Unable to locate application data directory")?.join("LocalShare");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db = Connection::open(dir.join("localshare.sqlite")).map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS trusted_peers (device_id TEXT PRIMARY KEY, name TEXT NOT NULL, fingerprint TEXT NOT NULL, trusted_at INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS chat_messages (id TEXT PRIMARY KEY, peer_id TEXT NOT NULL, direction TEXT NOT NULL, content TEXT NOT NULL, sent_at INTEGER NOT NULL, state TEXT NOT NULL); CREATE TABLE IF NOT EXISTS transfer_history (id TEXT PRIMARY KEY, peer_id TEXT NOT NULL, name TEXT NOT NULL, size INTEGER NOT NULL, direction TEXT NOT NULL, result TEXT NOT NULL, path TEXT, created_at INTEGER NOT NULL);").map_err(|e| e.to_string())?;
    Ok(db)
}

pub fn app_data_dir() -> Result<PathBuf, String> { dirs::data_local_dir().map(|p| p.join("LocalShare")).ok_or("Unable to locate application data directory".into()) }
pub fn setting(db: &Connection, key: &str) -> Option<String> { db.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get(0)).ok() }
pub fn set_setting(db: &Connection, key: &str, value: &str) -> Result<(), String> { db.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value]).map(|_| ()).map_err(|e| e.to_string()) }

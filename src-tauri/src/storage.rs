use rusqlite::{params, Connection};
use std::{fs, path::PathBuf};

pub fn open() -> Result<Connection, String> {
    let dir = dirs::data_local_dir()
        .ok_or("Unable to locate application data directory")?
        .join("LocalShare");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db = Connection::open(dir.join("localshare.sqlite")).map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS trusted_peers (device_id TEXT PRIMARY KEY, name TEXT NOT NULL, fingerprint TEXT NOT NULL, trusted_at INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS chat_messages (id TEXT PRIMARY KEY, peer_id TEXT NOT NULL, direction TEXT NOT NULL, content TEXT NOT NULL, sent_at INTEGER NOT NULL, state TEXT NOT NULL); CREATE TABLE IF NOT EXISTS transfer_history (id TEXT PRIMARY KEY, peer_id TEXT NOT NULL, name TEXT NOT NULL, size INTEGER NOT NULL, direction TEXT NOT NULL, result TEXT NOT NULL, path TEXT, created_at INTEGER NOT NULL);").map_err(|e| e.to_string())?;
    migrate_defaults(&db)?;
    Ok(db)
}

pub fn app_data_dir() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|p| p.join("LocalShare"))
        .ok_or("Unable to locate application data directory".into())
}

/// The user-visible destination for incoming files.  Keep application state
/// and received content separate: deleting LocalShare's database must not
/// unexpectedly delete a user's files.
pub fn default_download_dir() -> Result<PathBuf, String> {
    dirs::download_dir()
        .map(|p| p.join("LocalShare"))
        .ok_or("Unable to locate the platform Downloads directory".into())
}

pub fn setting(db: &Connection, key: &str) -> Option<String> {
    db.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
        r.get(0)
    })
    .ok()
}
pub fn set_setting(db: &Connection, key: &str, value: &str) -> Result<(), String> {
    db.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value]).map(|_| ()).map_err(|e| e.to_string())
}

fn migrate_defaults(db: &Connection) -> Result<(), String> {
    let default_dir = default_download_dir()?;
    let default_value = default_dir.to_string_lossy();
    let legacy_value = app_data_dir()?
        .join("Downloads")
        .to_string_lossy()
        .into_owned();
    let current = setting(db, "download_location");
    if current.as_deref().is_none() || current.as_deref() == Some(legacy_value.as_str()) {
        set_setting(db, "download_location", &default_value)?;
    }
    if setting(db, "ask_before_receive").is_none() {
        set_setting(db, "ask_before_receive", "true")?;
    }
    if setting(db, "notifications").is_none() {
        set_setting(db, "notifications", "true")?;
    }
    if setting(db, "check_for_updates").is_none() {
        set_setting(db, "check_for_updates", "true")?;
    }
    Ok(())
}

pub fn trusted_fingerprint(db: &Connection, device_id: &str) -> Option<String> {
    db.query_row(
        "SELECT fingerprint FROM trusted_peers WHERE device_id=?1",
        [device_id],
        |row| row.get(0),
    )
    .ok()
}

pub fn trust_peer(
    db: &Connection,
    device_id: &str,
    name: &str,
    fingerprint: &str,
    trusted_at: i64,
) -> Result<(), String> {
    db.execute(
        "INSERT INTO trusted_peers(device_id,name,fingerprint,trusted_at) VALUES(?1,?2,?3,?4) ON CONFLICT(device_id) DO UPDATE SET name=excluded.name, fingerprint=excluded.fingerprint, trusted_at=excluded.trusted_at",
        params![device_id, name, fingerprint, trusted_at],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

use rusqlite::{params, Connection};
use std::{fs, path::PathBuf, sync::OnceLock};
use tauri::Manager;

static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static DOWNLOAD_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn configure(app: &tauri::AppHandle) -> Result<(), String> {
    let data = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    #[cfg(target_os = "android")]
    let download = data.join("Downloads");
    #[cfg(not(target_os = "android"))]
    let download = app
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?
        .join("LocalShare");
    let _ = APP_DATA_DIR.set(data);
    let _ = DOWNLOAD_DIR.set(download);
    Ok(())
}

pub fn open() -> Result<Connection, String> {
    let dir = app_data_dir()?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db = Connection::open(dir.join("localshare.sqlite")).map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS trusted_peers (device_id TEXT PRIMARY KEY, name TEXT NOT NULL, fingerprint TEXT NOT NULL, trusted_at INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS chat_messages (id TEXT PRIMARY KEY, peer_id TEXT NOT NULL, direction TEXT NOT NULL, content TEXT NOT NULL, sent_at INTEGER NOT NULL, state TEXT NOT NULL); CREATE TABLE IF NOT EXISTS transfer_history (id TEXT PRIMARY KEY, peer_id TEXT NOT NULL, name TEXT NOT NULL, size INTEGER NOT NULL, direction TEXT NOT NULL, result TEXT NOT NULL, path TEXT, created_at INTEGER NOT NULL);").map_err(|e| e.to_string())?;
    migrate_schema(&db)?;
    migrate_defaults(&db)?;
    Ok(db)
}

fn migrate_schema(db: &Connection) -> Result<(), String> {
    let mut columns = db
        .prepare("PRAGMA table_info(trusted_peers)")
        .map_err(|e| e.to_string())?;
    let names = columns
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if names.iter().any(|name| name == "peer_id") && !names.iter().any(|name| name == "device_id") {
        db.execute_batch(
            "BEGIN IMMEDIATE;
             DROP TABLE IF EXISTS trusted_peers_legacy;
             ALTER TABLE trusted_peers RENAME TO trusted_peers_legacy;
             CREATE TABLE trusted_peers (
               device_id TEXT PRIMARY KEY,
               name TEXT NOT NULL,
               fingerprint TEXT NOT NULL,
               trusted_at INTEGER NOT NULL
             );
             INSERT OR REPLACE INTO trusted_peers(device_id, name, fingerprint, trusted_at)
               SELECT peer_id, name, fingerprint, paired_at FROM trusted_peers_legacy;
             DROP TABLE trusted_peers_legacy;
             COMMIT;",
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn app_data_dir() -> Result<PathBuf, String> {
    APP_DATA_DIR.get().cloned().map(Ok).unwrap_or_else(|| {
        dirs::data_local_dir()
            .map(|p| p.join("LocalShare"))
            .ok_or("Unable to locate application data directory".into())
    })
}

/// The user-visible destination for incoming files.  Keep application state
/// and received content separate: deleting LocalShare's database must not
/// unexpectedly delete a user's files.
pub fn default_download_dir() -> Result<PathBuf, String> {
    DOWNLOAD_DIR.get().cloned().map(Ok).unwrap_or_else(|| {
        dirs::download_dir()
            .map(|p| p.join("LocalShare"))
            .ok_or("Unable to locate the platform Downloads directory".into())
    })
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
        set_setting(db, "ask_before_receive", "false")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_trusted_peers_without_losing_identity() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE trusted_peers(
               peer_id TEXT PRIMARY KEY,
               fingerprint TEXT NOT NULL,
               name TEXT NOT NULL,
               paired_at INTEGER NOT NULL,
               last_seen INTEGER NOT NULL
             );
             INSERT INTO trusted_peers VALUES('peer-1', 'abc123', 'Office Mac', 42, 99);",
        )
        .unwrap();

        migrate_schema(&db).unwrap();

        let migrated: (String, String, String, i64) = db
            .query_row(
                "SELECT device_id, name, fingerprint, trusted_at FROM trusted_peers",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            migrated,
            ("peer-1".into(), "Office Mac".into(), "abc123".into(), 42)
        );
    }
}

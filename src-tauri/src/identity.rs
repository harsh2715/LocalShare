use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use uuid::Uuid;

#[derive(Clone)]
pub struct Identity {
    pub device_id: Uuid,
    pub device_name: String,
    pub fingerprint: String,
}

pub fn load_or_create(
    data_dir: &Path,
    configured_name: Option<String>,
) -> Result<Identity, String> {
    fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let path = data_dir.join("device-id");
    let device_id = match fs::read_to_string(&path) {
        Ok(value) => Uuid::parse_str(value.trim()).map_err(|e| e.to_string())?,
        Err(_) => {
            let id = Uuid::new_v4();
            fs::write(&path, id.to_string()).map_err(|e| e.to_string())?;
            id
        }
    };
    let mut hasher = Sha256::new();
    hasher.update(device_id.as_bytes());
    let fingerprint = hex::encode(hasher.finalize());
    let device_name = configured_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let hostname = hostname::get()
                .ok()
                .and_then(|v| v.into_string().ok())
                .unwrap_or_default();
            if hostname.trim().is_empty() || hostname.eq_ignore_ascii_case("localhost") {
                #[cfg(target_os = "android")]
                return "Android device".into();
                #[cfg(target_os = "ios")]
                return "iPhone or iPad".into();
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                return "This device".into();
            }
            hostname
        });
    Ok(Identity {
        device_id,
        device_name,
        fingerprint,
    })
}

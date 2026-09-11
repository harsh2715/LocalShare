use ed25519_dalek::{
    pkcs8::{DecodePrivateKey, EncodePrivateKey},
    Signer, SigningKey,
};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path};
use uuid::Uuid;

const KEYRING_SERVICE: &str = "com.localshare.desktop.identity";
const FALLBACK_KEY_FILE: &str = "device-key.pk8";

#[derive(Clone)]
pub struct Identity {
    pub device_id: Uuid,
    pub device_name: String,
    pub fingerprint: String,
    pub signing_key: SigningKey,
}

fn decode_key(bytes: &[u8]) -> Result<SigningKey, String> {
    SigningKey::from_pkcs8_der(bytes).map_err(|e| format!("Invalid stored device identity: {e}"))
}

fn fallback_key_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join(FALLBACK_KEY_FILE)
}

fn read_fallback_key(data_dir: &Path) -> Result<Option<SigningKey>, String> {
    match fs::read(fallback_key_path(data_dir)) {
        Ok(bytes) => decode_key(&bytes).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Unable to read device identity: {error}")),
    }
}

fn write_fallback_key(data_dir: &Path, key: &SigningKey) -> Result<SigningKey, String> {
    let document = key
        .to_pkcs8_der()
        .map_err(|e| format!("Unable to encode device identity: {e}"))?;
    let path = fallback_key_path(data_dir);
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return read_fallback_key(data_dir)?.ok_or_else(|| {
                "Device identity appeared during creation but could not be read".into()
            })
        }
        Err(error) => return Err(format!("Unable to persist device identity: {error}")),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Unable to protect device identity: {e}"))?;
    }
    file.write_all(document.as_bytes())
        .map_err(|e| format!("Unable to persist device identity: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("Unable to persist device identity: {e}"))?;
    Ok(key.clone())
}

fn load_or_create_signing_key(data_dir: &Path, device_id: Uuid) -> Result<SigningKey, String> {
    let username = device_id.to_string();
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &username) {
        if let Ok(bytes) = entry.get_secret() {
            return decode_key(&bytes);
        }
        if let Some(key) = read_fallback_key(data_dir)? {
            return Ok(key);
        }
        let key = SigningKey::generate(&mut OsRng);
        let document = key
            .to_pkcs8_der()
            .map_err(|e| format!("Unable to encode device identity: {e}"))?;
        if entry.set_secret(document.as_bytes()).is_ok() {
            return Ok(key);
        }
        return write_fallback_key(data_dir, &key);
    }
    if let Some(key) = read_fallback_key(data_dir)? {
        return Ok(key);
    }
    let key = SigningKey::generate(&mut OsRng);
    write_fallback_key(data_dir, &key)
}

pub fn public_key_fingerprint(public_key: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(public_key);
    hex::encode(hasher.finalize())
}

pub fn load_or_create(
    data_dir: &Path,
    configured_name: Option<String>,
) -> Result<Identity, String> {
    fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let path = data_dir.join("device-id");
    let device_id = match fs::read_to_string(&path) {
        Ok(value) => Uuid::parse_str(value.trim()).map_err(|e| e.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let id = Uuid::new_v4();
            fs::write(&path, id.to_string()).map_err(|e| e.to_string())?;
            id
        }
        Err(error) => return Err(format!("Unable to read device ID: {error}")),
    };
    let signing_key = load_or_create_signing_key(data_dir, device_id)?;
    let fingerprint = public_key_fingerprint(&signing_key.verifying_key().to_bytes());
    let device_name = configured_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            hostname::get()
                .ok()
                .and_then(|v| v.into_string().ok())
                .unwrap_or_else(|| "This device".into())
        });
    Ok(Identity {
        device_id,
        device_name,
        fingerprint,
        signing_key,
    })
}

pub fn sign(key: &SigningKey, payload: &[u8]) -> Vec<u8> {
    key.sign(payload).to_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Verifier;

    #[test]
    fn identity_is_stable_and_signs() {
        let data_dir = std::env::temp_dir().join(format!("localshare-identity-{}", Uuid::new_v4()));
        let first = load_or_create(&data_dir, Some("Test device".into())).unwrap();
        let second = load_or_create(&data_dir, Some("Test device".into())).unwrap();
        assert_eq!(first.device_id, second.device_id);
        assert_eq!(first.fingerprint, second.fingerprint);
        let payload = b"handshake test";
        let signature = sign(&first.signing_key, payload);
        let signature = ed25519_dalek::Signature::from_slice(&signature).unwrap();
        first
            .signing_key
            .verifying_key()
            .verify(payload, &signature)
            .unwrap();
        let _ = fs::remove_dir_all(data_dir);
    }
}

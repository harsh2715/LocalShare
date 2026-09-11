use anyhow::Result;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: String,
    pub name: String,
    pub certificate_pem: String,
    #[serde(default)]
    pub private_key_pem: String,
    pub fingerprint: String,
}

pub fn load_or_create(dir: &Path, name: &str, secure: bool, profile: &str) -> Result<Identity> {
    let file = dir.join("identity.json");
    if file.exists() {
        let mut identity: Identity = serde_json::from_slice(&fs::read(&file)?)?;
        identity.private_key_pem = if secure {
            keyring::Entry::new("com.localshare.desktop.identity", profile)?
                .get_password()
                .map_err(|error| anyhow::anyhow!("identity keychain read failed: {error}"))?
        } else {
            serde_json::from_slice::<serde_json::Value>(&fs::read(&file)?)?["private_key_pem"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("identity private key missing"))?
                .to_owned()
        };
        restrict_file(&file)?;
        return Ok(identity);
    }
    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(vec!["localshare".into()])?;
    let cert_pem = cert.pem();
    let private_key_pem = key_pair.serialize_pem();
    let fingerprint = hex::encode(Sha256::digest(cert.der().as_ref()));
    let identity = Identity {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_owned(),
        certificate_pem: cert_pem,
        private_key_pem: private_key_pem.clone(),
        fingerprint,
    };
    if secure {
        keyring::Entry::new("com.localshare.desktop.identity", profile)?
            .set_password(&private_key_pem)
            .map_err(|error| anyhow::anyhow!("identity keychain write failed: {error}"))?;
    }
    if secure {
        let public_identity = serde_json::json!({
            "id": identity.id,
            "name": identity.name,
            "certificate_pem": identity.certificate_pem,
            "fingerprint": identity.fingerprint
        });
        fs::write(&file, serde_json::to_vec_pretty(&public_identity)?)?;
    } else {
        fs::write(&file, serde_json::to_vec_pretty(&identity)?)?;
    }
    restrict_file(&file)?;
    Ok(identity)
}

fn restrict_file(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

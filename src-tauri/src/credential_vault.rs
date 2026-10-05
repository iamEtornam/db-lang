//! Authenticated local secrets, with the encryption key held by the OS credential store.
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload, rand_core::RngCore},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "qs-secret-v1:";
const MAX_BACKUP_BYTES: usize = 10 * 1024 * 1024;

pub struct CredentialVault(Aes256Gcm);

impl CredentialVault {
    pub fn from_key(key: &[u8]) -> Result<Self, String> {
        Aes256Gcm::new_from_slice(key).map(Self).map_err(|_| "Invalid credential key length".into())
    }

    pub fn open_os_key(id: &str, create: bool) -> Result<Self, String> {
        let entry = keyring::Entry::new("dev.etornam.QueryStudio.credentials", id)
            .map_err(|e| format!("Cannot access the OS credential store: {e}"))?;
        let key = match entry.get_secret() {
            Ok(key) => key,
            Err(keyring::Error::NoEntry) if create => {
                let mut key = [0u8; 32];
                OsRng.fill_bytes(&mut key);
                entry.set_secret(&key).map_err(|e| format!("Cannot save the credential key: {e}"))?;
                // Do not commit encrypted data until persistence has been verified.
                let saved = entry.get_secret().map_err(|e| format!("Cannot verify the credential key: {e}"))?;
                if saved != key { return Err("Credential key verification failed".into()); }
                saved
            }
            Err(keyring::Error::NoEntry) => return Err("The OS credential key is missing. Restore the original keychain or import an encrypted connection backup into a fresh installation. The existing database has not been modified.".into()),
            Err(e) => return Err(format!("Unlock or enable your OS credential store and retry: {e}")),
        };
        Self::from_key(&key)
    }

    pub fn seal(&self, value: &str, context: &str) -> Result<String, String> {
        if value.is_empty() { return Ok(String::new()); }
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self.0.encrypt(&nonce, Payload { msg: value.as_bytes(), aad: context.as_bytes() })
            .map_err(|_| "Could not encrypt a credential".to_string())?;
        let mut bytes = nonce.to_vec();
        bytes.extend(ciphertext);
        Ok(format!("{PREFIX}{}", STANDARD.encode(bytes)))
    }

    pub fn unseal(&self, value: &str, context: &str) -> Result<String, String> {
        if value.is_empty() { return Ok(String::new()); }
        let encoded = value.strip_prefix(PREFIX).ok_or("Credential is not encrypted")?;
        let bytes = STANDARD.decode(encoded).map_err(|_| "Invalid encrypted credential")?;
        if bytes.len() < 28 { return Err("Truncated encrypted credential".into()); }
        let plaintext = self.0.decrypt(Nonce::from_slice(&bytes[..12]), Payload { msg: &bytes[12..], aad: context.as_bytes() })
            .map_err(|_| "Credential authentication failed. Check the keychain or restore a backup.".to_string())?;
        String::from_utf8(plaintext).map_err(|_| "Invalid credential encoding".into())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupEnvelope {
    format: String,
    version: u8,
    salt: String,
    payload: String,
}

fn backup_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    if passphrase.chars().count() < 12 || passphrase.len() > 1024 {
        return Err("Use a backup passphrase with 12 or more characters (at most 1024 bytes)".into());
    }
    let mut key = [0u8; 32];
    argon2::Argon2::default().hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|_| "Could not derive the backup key".to_string())?;
    Ok(key)
}

pub fn encrypt_backup(plaintext: &str, passphrase: &str) -> Result<String, String> {
    if plaintext.len() > MAX_BACKUP_BYTES / 2 { return Err("Connection backup is too large".into()); }
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let key = backup_key(passphrase, &salt)?;
    let payload = CredentialVault::from_key(&key)?.seal(plaintext, "QueryStudio connection backup v1")?;
    serde_json::to_string_pretty(&BackupEnvelope { format: "QueryStudio connections".into(), version: 1, salt: STANDARD.encode(salt), payload })
        .map_err(|_| "Could not encode connection backup".into())
}

pub fn decrypt_backup(backup: &str, passphrase: &str) -> Result<String, String> {
    if backup.len() > MAX_BACKUP_BYTES { return Err("Connection backup is too large".into()); }
    let envelope: BackupEnvelope = serde_json::from_str(backup).map_err(|_| "Invalid connection backup file")?;
    if envelope.format != "QueryStudio connections" || envelope.version != 1 {
        return Err("Unsupported connection backup format".into());
    }
    let salt = STANDARD.decode(envelope.salt).map_err(|_| "Invalid backup salt")?;
    if salt.len() != 16 { return Err("Invalid backup salt length".into()); }
    let key = backup_key(passphrase, &salt)?;
    CredentialVault::from_key(&key)?.unseal(&envelope.payload, "QueryStudio connection backup v1")
        .map_err(|_| "Incorrect passphrase or damaged connection backup".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn randomized_authenticated_secrets_reject_wrong_key_and_context() {
        let vault = CredentialVault::from_key(&[7; 32]).unwrap();
        let value = "paßword 🔑";
        let sealed = vault.seal(value, "connection/a/password").unwrap();
        assert_ne!(sealed, vault.seal(value, "connection/a/password").unwrap());
        assert!(!sealed.contains(value));
        assert_eq!(vault.unseal(&sealed, "connection/a/password").unwrap(), value);
        assert!(vault.unseal(&sealed, "connection/b/password").is_err());
        assert!(CredentialVault::from_key(&[8; 32]).unwrap().unseal(&sealed, "connection/a/password").is_err());
        assert!(vault.unseal("qs-secret-v1:AA==", "connection/a/password").is_err());
        assert!(vault.unseal("unencrypted", "connection/a/password").is_err());
        assert_eq!(vault.unseal(&vault.seal("", "empty").unwrap(), "empty").unwrap(), "");
    }

    #[test]
    fn encrypted_backups_are_portable_and_fail_closed() {
        let backup = encrypt_backup("[{\"password\":\"test-secret\"}]", "a long test passphrase").unwrap();
        assert!(!backup.contains("test-secret"));
        assert_eq!(decrypt_backup(&backup, "a long test passphrase").unwrap(), "[{\"password\":\"test-secret\"}]");
        assert!(decrypt_backup(&backup, "wrong long passphrase").is_err());
        assert!(encrypt_backup("[]", "short").is_err());
        let mut envelope: BackupEnvelope = serde_json::from_str(&backup).unwrap();
        envelope.version = 99;
        assert!(decrypt_backup(&serde_json::to_string(&envelope).unwrap(), "a long test passphrase").is_err());
        assert!(decrypt_backup(&"x".repeat(MAX_BACKUP_BYTES + 1), "a long test passphrase").is_err());
    }
}

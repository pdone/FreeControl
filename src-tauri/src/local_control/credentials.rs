use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;

const CREDENTIAL_VERSION: u32 = 1;
const CREDENTIAL_FILE: &str = "external-control.json";
const TMP_PREFIX: &str = "external-control.json.";

#[derive(Serialize, Deserialize)]
pub(crate) struct StoredCredential {
    version: u32,
    token_hash: String,
    created_at: u64,
}

impl StoredCredential {
    pub(crate) fn verify(&self, token: &str) -> bool {
        if self.version != CREDENTIAL_VERSION {
            return false;
        }
        let Some(expected) = decode_hash(&self.token_hash) else {
            return false;
        };
        let actual = hash_token(token);
        actual.ct_eq(&expected).into()
    }
}

pub fn path_in(root: &Path) -> PathBuf {
    root.join(CREDENTIAL_FILE)
}

pub fn load(path: &Path) -> Option<StoredCredential> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            crate::logger::warn(
                "local-control",
                format!("读取凭据文件失败，按未配对处理：{e}"),
            );
            return None;
        }
    };
    match serde_json::from_str::<StoredCredential>(&raw) {
        Ok(stored)
            if stored.version == CREDENTIAL_VERSION
                && decode_hash(&stored.token_hash).is_some() =>
        {
            Some(stored)
        }
        _ => {
            crate::logger::warn("local-control", "凭据文件损坏或版本不支持，按未配对处理");
            None
        }
    }
}

pub fn save(path: &Path, token_hash: &[u8; 32], created_at: u64) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let stored = StoredCredential {
        version: CREDENTIAL_VERSION,
        token_hash: encode_hash(token_hash),
        created_at,
    };
    let bytes = serde_json::to_vec_pretty(&stored).map_err(|e| e.to_string())?;
    let tmp = temp_path(path);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).map_err(|e| e.to_string())?;
    if let Err(e) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    drop(file);
    if let Err(e) = replace_file(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn revoke(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => save(path, &[0u8; 32], 0),
    }
}

pub fn cleanup_stale(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(TMP_PREFIX) && name.ends_with(".tmp") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64url(&bytes)
}

pub fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn encode_hash(hash: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in hash {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hash(raw: &str) -> Option<[u8; 32]> {
    if raw.len() != 64 || !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        let high = (raw.as_bytes()[index * 2] as char).to_digit(16)?;
        let low = (raw.as_bytes()[index * 2 + 1] as char).to_digit(16)?;
        *byte = ((high << 4) | low) as u8;
    }
    Some(out)
}

fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4 + 2) / 3);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(ALPHABET[(b0 >> 2) as usize] as char);
        out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(b2 & 0x3f) as usize] as char);
        }
    }
    out
}

fn temp_path(path: &Path) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path.with_file_name(format!(
        "{}.{}-{}-{}.tmp",
        TMP_PREFIX,
        std::process::id(),
        nanos,
        seq
    ))
}

fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
    let mut last = String::new();
    for attempt in 0..3 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e.to_string();
                if attempt < 2 {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "fcn-credential-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn token_is_url_safe_and_hash_is_stable() {
        let token = generate_token();
        assert_eq!(token.len(), 43);
        assert!(token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        assert_eq!(hash_token(&token), hash_token(&token));
        assert_ne!(hash_token(&token), hash_token("different"));
    }

    #[test]
    fn credential_file_never_contains_plaintext_and_verifies() {
        let dir = temp_dir("roundtrip");
        let path = path_in(&dir);
        let token = generate_token();
        let hash = hash_token(&token);
        save(&path, &hash, 123).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains(&token));
        assert!(raw.contains(&encode_hash(&hash)));
        let stored = load(&path).unwrap();
        assert!(stored.verify(&token));
        assert!(!stored.verify("wrong"));
        assert_eq!(stored.created_at, 123);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_credential_is_ignored_and_revoke_is_idempotent() {
        let dir = temp_dir("corrupt");
        let path = path_in(&dir);
        fs::write(&path, b"not json").unwrap();
        assert!(load(&path).is_none());
        revoke(&path).unwrap();
        revoke(&path).unwrap();
        assert!(!path.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_write_cleans_temporary_file() {
        let dir = temp_dir("failed");
        let blocker = dir.join("blocker");
        fs::write(&blocker, b"x").unwrap();
        let path = blocker.join(CREDENTIAL_FILE);
        assert!(save(&path, &hash_token("secret"), 1).is_err());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(dir);
    }
}

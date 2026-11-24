use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const DISCOVERY_VERSION: u32 = 1;
const SERVICE_NAME: &str = "freecontrol-fuyu";
const DISCOVERY_FILE: &str = "fuyu-local.json";
const TMP_PREFIX: &str = "fuyu-local.json.";

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryRecord {
    pub version: u32,
    pub service: String,
    pub base_url: String,
    pub token: String,
    pub issued_at: u64,
}

pub fn path_in(root: &Path) -> PathBuf {
    root.join(DISCOVERY_FILE)
}

pub fn write(path: &Path, base_url: &str, token: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let record = DiscoveryRecord {
        version: DISCOVERY_VERSION,
        service: SERVICE_NAME.to_string(),
        base_url: base_url.to_string(),
        token: token.to_string(),
        issued_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis() as u64)
            .unwrap_or(0),
    };
    let bytes = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
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

#[cfg(test)]
pub fn read(path: &Path) -> Result<Option<DiscoveryRecord>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let record = serde_json::from_str::<DiscoveryRecord>(&raw).map_err(|e| e.to_string())?;
    if record.version != DISCOVERY_VERSION
        || record.service != SERVICE_NAME
        || record.token.is_empty()
        || record.token.len() > 512
    {
        return Err("invalid discovery record".to_string());
    }
    Ok(Some(record))
}

pub fn remove(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
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

fn temp_path(path: &Path) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "fcn-discovery-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discovery_round_trips_and_removes() {
        let dir = temp_dir("roundtrip");
        let path = path_in(&dir);
        write(&path, "http://127.0.0.1:1234", "secret-token").unwrap();
        let record = read(&path).unwrap().unwrap();
        assert_eq!(record.version, 1);
        assert_eq!(record.service, "freecontrol-fuyu");
        assert_eq!(record.base_url, "http://127.0.0.1:1234");
        assert_eq!(record.token, "secret-token");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        remove(&path).unwrap();
        remove(&path).unwrap();
        assert!(read(&path).unwrap().is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_discovery_is_rejected() {
        let dir = temp_dir("corrupt");
        let path = path_in(&dir);
        fs::write(&path, b"not json").unwrap();
        assert!(read(&path).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_write_cleans_temporary_file() {
        let dir = temp_dir("failed");
        let blocker = dir.join("blocker");
        fs::write(&blocker, b"x").unwrap();
        let path = blocker.join(DISCOVERY_FILE);
        assert!(write(&path, "http://127.0.0.1:1234", "token").is_err());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(dir);
    }
}

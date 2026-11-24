use crate::local_control::credentials::{self, StoredCredential};
use crate::local_control::errors::ControlError;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AuthManager {
    path: PathBuf,
    stored: Mutex<Option<StoredCredential>>,
}

impl AuthManager {
    pub fn new(path: PathBuf) -> Self {
        let stored = credentials::load(&path);
        Self {
            path,
            stored: Mutex::new(stored),
        }
    }

    pub fn verify(&self, token: &str) -> bool {
        self.stored
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|stored| stored.verify(token))
    }

    pub fn has_credential(&self) -> bool {
        self.stored.lock().unwrap().is_some()
    }

    pub fn issue(&self) -> Result<String, ControlError> {
        let token = credentials::generate_token();
        let hash = credentials::hash_token(&token);
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_millis() as u64)
            .unwrap_or(0);
        credentials::save(&self.path, &hash, created_at).map_err(|_| ControlError::Internal)?;
        *self.stored.lock().unwrap() = credentials::load(&self.path);
        Ok(token)
    }

    pub fn revoke(&self) -> Result<(), ControlError> {
        credentials::revoke(&self.path).map_err(|_| ControlError::Internal)?;
        *self.stored.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fcn-auth-{tag}-{}-{nanos}.json",
            std::process::id()
        ))
    }

    #[test]
    fn issue_verify_and_revoke() {
        let path = temp_path("roundtrip");
        let auth = AuthManager::new(path.clone());
        let token = auth.issue().unwrap();
        assert!(auth.verify(&token));
        assert!(!auth.verify("wrong"));
        assert!(!fs::read_to_string(&path).unwrap().contains(&token));
        auth.revoke().unwrap();
        assert!(!auth.verify(&token));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn write_failure_does_not_issue_token() {
        let dir = temp_path("blocked");
        fs::write(&dir, b"x").unwrap();
        let auth = AuthManager::new(dir.join("credential.json"));
        assert_eq!(auth.issue(), Err(ControlError::Internal));
        let _ = fs::remove_file(dir);
    }
}

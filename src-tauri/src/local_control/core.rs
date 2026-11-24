use crate::device;
use crate::local_control::dto::{ExternalActionResult, ExternalState};
use crate::local_control::errors::ControlError;
use crate::session;
use crate::AppState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

pub struct DeviceControlService {
    app: AppHandle,
    device_locks: DeviceLocks,
}

impl DeviceControlService {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            device_locks: DeviceLocks::default(),
        }
    }

    pub async fn state(&self) -> ExternalState {
        ExternalState::from_payload(&device::build_payload(&self.app))
    }

    pub async fn connect(&self, device_id: &str) -> Result<ExternalActionResult, ControlError> {
        let lock = self.device_locks.get(device_id);
        let _guard = lock.lock().await;
        let was_active = {
            let state = self.app.state::<AppState>();
            session::active_record(&state, device_id).is_some()
        };
        let app = self.app.clone();
        let id = device_id.to_string();
        let result = crate::download::run_blocking(move || session::connect_saved(&app, &id))
            .await
            .map_err(|error| ControlError::from_user_error(&error));
        let result = match result {
            Ok(result) => result,
            Err(ControlError::Conflict) => return self.action_result(device_id, false, None).await,
            Err(error) => return Err(error),
        };
        let warning = result
            .warning
            .as_ref()
            .map(|warning| warning.code.clone())
            .or_else(|| {
                result
                    .outcomes
                    .first()
                    .and_then(|outcome| outcome.warning.as_ref())
                    .map(|warning| warning.code.clone())
            });
        self.action_result(device_id, !was_active, warning).await
    }

    pub async fn disconnect(&self, device_id: &str) -> Result<ExternalActionResult, ControlError> {
        let lock = self.device_locks.get(device_id);
        let _guard = lock.lock().await;
        let was_active = {
            let state = self.app.state::<AppState>();
            !session::active_records(&state, device_id).is_empty()
        };
        let app = self.app.clone();
        let id = device_id.to_string();
        crate::download::run_blocking(move || session::disconnect_saved(&app, &id))
            .await
            .map_err(|error| ControlError::from_user_error(&error))?;
        self.action_result(device_id, was_active, None).await
    }

    async fn action_result(
        &self,
        device_id: &str,
        changed: bool,
        warning: Option<String>,
    ) -> Result<ExternalActionResult, ControlError> {
        let status = {
            let state = self.app.state::<AppState>();
            device::status_of_device(&state, device_id)
        }
        .ok_or(ControlError::NotFound)?;
        Ok(ExternalActionResult {
            device_id: device_id.to_string(),
            status,
            changed,
            warning,
        })
    }
}

#[derive(Default)]
struct DeviceLocks(Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>);

impl DeviceLocks {
    fn get(&self, device_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.0.lock().unwrap();
        locks
            .entry(device_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn same_device_operations_are_serialized() {
        let locks = DeviceLocks::default();
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let lock = locks.get("device-a");
            let active = active.clone();
            let max_active = max_active.clone();
            tasks.push(tokio::spawn(async move {
                let _guard = lock.lock().await;
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn different_devices_have_independent_locks() {
        let locks = DeviceLocks::default();
        assert!(!Arc::ptr_eq(&locks.get("a"), &locks.get("b")));
        assert!(Arc::ptr_eq(&locks.get("a"), &locks.get("a")));
    }
}

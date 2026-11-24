use crate::local_control::api::ControlOperations;
use crate::local_control::auth::AuthManager;
use crate::local_control::core::DeviceControlService;
use crate::local_control::discovery;
use crate::local_control::server::RunningServer;
use crate::AppState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

pub const EVENT_LOCAL_CONTROL_STATUS: &str = "local-control-status";
const FUYU_PROJECT_URL: &str = "https://github.com/TixXin/Fuyu";

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Disabled,
    Starting,
    Running,
    Error,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub status: ServiceStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_docs_url: Option<String>,
}

struct RuntimeState {
    status: ServiceStatus,
    message: Option<String>,
    server: Option<RunningServer>,
}

struct RuntimeManager {
    app: AppHandle,
    auth: Option<Arc<AuthManager>>,
    discovery_path: Option<PathBuf>,
    reconcile_lock: tokio::sync::Mutex<()>,
    state: Mutex<RuntimeState>,
}

#[derive(Clone)]
pub struct LocalControlRuntime {
    inner: Arc<RuntimeManager>,
}

impl LocalControlRuntime {
    pub fn new(
        app: AppHandle,
        credential_path: Option<PathBuf>,
        discovery_path: Option<PathBuf>,
    ) -> Self {
        let auth = credential_path.map(|path| Arc::new(AuthManager::new(path)));
        Self {
            inner: Arc::new(RuntimeManager {
                app,
                auth,
                discovery_path,
                reconcile_lock: tokio::sync::Mutex::new(()),
                state: Mutex::new(RuntimeState {
                    status: ServiceStatus::Disabled,
                    message: None,
                    server: None,
                }),
            }),
        }
    }

    pub fn status(&self) -> RuntimeStatus {
        let state = self.inner.state.lock().unwrap();
        let (base_url, api_docs_url) = if state.status == ServiceStatus::Running {
            state.server.as_ref().map_or((None, None), |server| {
                (Some(server.base_url.clone()), Some(server.api_docs_url()))
            })
        } else {
            (None, None)
        };
        RuntimeStatus {
            status: state.status,
            message: state.message.clone(),
            base_url,
            api_docs_url,
        }
    }

    pub async fn reconcile(&self, enabled: bool) {
        let _reconcile = self.inner.reconcile_lock.lock().await;
        if enabled {
            if self.status().status == ServiceStatus::Running {
                return;
            }
            if self.status().status == ServiceStatus::Starting {
                return;
            }
            self.set_status(ServiceStatus::Starting, None);
            let Some(auth) = self.inner.auth.clone() else {
                self.set_status(
                    ServiceStatus::Error,
                    Some("无法确定本地控制凭据位置。".to_string()),
                );
                return;
            };
            let Some(discovery_path) = self.inner.discovery_path.clone() else {
                self.set_status(
                    ServiceStatus::Error,
                    Some("无法确定本地发现文件位置。".to_string()),
                );
                return;
            };
            let operations: Arc<dyn ControlOperations> =
                Arc::new(DeviceControlService::new(self.inner.app.clone()));
            let preferred_port = {
                let state = self.inner.app.state::<AppState>();
                let port = state.config.lock().unwrap().external_control.last_port;
                port
            };
            match RunningServer::start(operations, auth, discovery_path, preferred_port).await
            {
                Ok(server) => {
                    crate::config::remember_local_control_port(&self.inner.app, server.port);
                    let mut state = self.inner.state.lock().unwrap();
                    state.server = Some(server);
                    state.status = ServiceStatus::Running;
                    state.message = None;
                    drop(state);
                    self.emit_status();
                }
                Err(error) => {
                    let message = error.body().message;
                    crate::logger::error(
                        "local-control",
                        format!("本地控制服务启动失败：{message}"),
                    );
                    self.set_status(ServiceStatus::Error, Some(message));
                }
            }
            return;
        }

        let server = {
            let mut state = self.inner.state.lock().unwrap();
            state.status = ServiceStatus::Disabled;
            state.message = None;
            state.server.take()
        };
        self.emit_status();
        if let Some(server) = server {
            if let Err(error) = server.shutdown().await {
                let message = error.body().message;
                crate::logger::error("local-control", format!("本地控制服务关闭失败：{message}"));
                self.set_status(ServiceStatus::Error, Some(message));
            }
        } else {
            if let Some(path) = &self.inner.discovery_path {
                if let Err(error) = discovery::remove(path) {
                    crate::logger::error("local-control", format!("删除本地发现文件失败：{error}"));
                }
            }
            if let Some(auth) = &self.inner.auth {
                if auth.has_credential() {
                    if let Err(error) = auth.revoke() {
                        let message = error.body().message;
                        crate::logger::error(
                            "local-control",
                            format!("撤销本地控制凭据失败：{message}"),
                        );
                        self.set_status(ServiceStatus::Error, Some(message));
                    }
                }
            }
        }
    }

    fn set_status(&self, status: ServiceStatus, message: Option<String>) {
        {
            let mut state = self.inner.state.lock().unwrap();
            state.status = status;
            state.message = message;
        }
        self.emit_status();
    }

    fn emit_status(&self) {
        let _ = self
            .inner
            .app
            .emit(EVENT_LOCAL_CONTROL_STATUS, self.status());
    }
}

pub fn shutdown(app: &AppHandle) {
    let runtime = app.state::<LocalControlRuntime>().clone();
    tauri::async_runtime::block_on(runtime.reconcile(false));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_exposes_only_current_addresses_and_never_credentials() {
        let value = serde_json::to_value(RuntimeStatus {
            status: ServiceStatus::Running,
            message: None,
            base_url: Some("http://127.0.0.1:54321".to_string()),
            api_docs_url: Some("http://127.0.0.1:54321/api-docs".to_string()),
        })
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "status": "running",
                "baseUrl": "http://127.0.0.1:54321",
                "apiDocsUrl": "http://127.0.0.1:54321/api-docs"
            })
        );
        let raw = value.to_string();
        assert!(!raw.contains("Bearer"));
        assert!(!raw.contains("token"));
        assert!(!raw.contains("discovery"));
    }

    #[test]
    fn non_running_status_omits_addresses() {
        for status in [
            ServiceStatus::Disabled,
            ServiceStatus::Starting,
            ServiceStatus::Error,
        ] {
            let value = serde_json::to_value(RuntimeStatus {
                status,
                message: None,
                base_url: None,
                api_docs_url: None,
            })
            .unwrap();
            assert!(value.get("baseUrl").is_none());
            assert!(value.get("apiDocsUrl").is_none());
        }
    }

    #[test]
    fn fuyu_project_page_url_is_https() {
        assert_eq!(FUYU_PROJECT_URL, "https://github.com/TixXin/Fuyu");
    }

    #[test]
    fn api_docs_open_target_requires_running_service() {
        let unavailable = [
            RuntimeStatus {
                status: ServiceStatus::Disabled,
                message: None,
                base_url: None,
                api_docs_url: None,
            },
            RuntimeStatus {
                status: ServiceStatus::Starting,
                message: None,
                base_url: None,
                api_docs_url: None,
            },
            RuntimeStatus {
                status: ServiceStatus::Error,
                message: Some("启动失败".to_string()),
                base_url: None,
                api_docs_url: None,
            },
        ];
        for status in unavailable {
            assert!(api_docs_open_target(&status).is_err());
        }

        let available = RuntimeStatus {
            status: ServiceStatus::Running,
            message: None,
            base_url: Some("http://127.0.0.1:54321".to_string()),
            api_docs_url: Some("http://127.0.0.1:54321/api-docs".to_string()),
        };
        assert_eq!(
            api_docs_open_target(&available).unwrap(),
            "http://127.0.0.1:54321/api-docs"
        );
    }
}

fn api_docs_open_target(status: &RuntimeStatus) -> Result<String, String> {
    if status.status != ServiceStatus::Running {
        return Err("本地服务未运行。".to_string());
    }
    status
        .api_docs_url
        .clone()
        .filter(|url| url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:"))
        .ok_or_else(|| "接口文档地址不可用。".to_string())
}

#[tauri::command]
pub fn get_local_control_status(runtime: State<'_, LocalControlRuntime>) -> RuntimeStatus {
    runtime.status()
}

#[tauri::command]
pub fn open_local_control_docs(runtime: State<'_, LocalControlRuntime>) -> Result<(), String> {
    let url = api_docs_open_target(&runtime.status())?;
    crate::shell::open_url(&url)
}

#[tauri::command]
pub fn open_fuyu_project_page() -> Result<(), String> {
    crate::shell::open_url(FUYU_PROJECT_URL)
}

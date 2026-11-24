use crate::local_control::api::{router, ApiState, ControlOperations, API_DOCS_PATH};
use crate::local_control::auth::AuthManager;
use crate::local_control::discovery;
use crate::local_control::errors::ControlError;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, RwLock};
use tokio::task::JoinHandle;

pub struct RunningServer {
    pub base_url: String,
    pub port: u16,
    auth: Arc<AuthManager>,
    discovery_path: PathBuf,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
    accepting: Arc<AtomicBool>,
    action_gate: Arc<RwLock<()>>,
}

impl RunningServer {
    pub async fn start(
        operations: Arc<dyn ControlOperations>,
        auth: Arc<AuthManager>,
        discovery_path: PathBuf,
        preferred_port: Option<u16>,
    ) -> Result<Self, ControlError> {
        let (listener, port) = bind_loopback(preferred_port).await?;
        let base_url = format!("http://127.0.0.1:{port}");
        let token = auth.issue().map_err(|_| ControlError::Internal)?;
        if let Err(error) = discovery::write(&discovery_path, &base_url, &token) {
            let _ = auth.revoke();
            crate::logger::error("local-control", format!("写入本地发现文件失败：{error}"));
            return Err(ControlError::Internal);
        }
        let state = ApiState::new(operations, auth.clone(), port);
        let accepting = state.accepting.clone();
        let action_gate = state.action_gate.clone();
        let app = router(state);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await;
        });
        Ok(Self {
            base_url,
            port,
            auth,
            discovery_path,
            shutdown_tx: Some(shutdown_tx),
            task,
            accepting,
            action_gate,
        })
    }

    pub fn api_docs_url(&self) -> String {
        format!("{}{}", self.base_url, API_DOCS_PATH)
    }

    pub async fn shutdown(mut self) -> Result<(), ControlError> {
        self.accepting.store(false, Ordering::SeqCst);
        let _action_guard = self.action_gate.write().await;
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        let _ = self.task.await;
        let discovery_result =
            discovery::remove(&self.discovery_path).map_err(|_| ControlError::Internal);
        let auth_result = self.auth.revoke();
        discovery_result.and(auth_result)
    }
}

async fn bind_loopback(preferred_port: Option<u16>) -> Result<(TcpListener, u16), ControlError> {
    if let Some(port) = preferred_port.filter(|port| *port != 0) {
        match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(listener) => {
                let port = listener
                    .local_addr()
                    .map_err(|_| ControlError::ServiceUnavailable)?
                    .port();
                return Ok((listener, port));
            }
            Err(_) => {
                crate::logger::warn(
                    "local-control",
                    format!("上次端口 {port} 不可用，改用新端口"),
                );
            }
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| ControlError::ServiceUnavailable)?;
    let port = listener
        .local_addr()
        .map_err(|_| ControlError::ServiceUnavailable)?
        .port();
    Ok((listener, port))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_control::dto::{ExternalActionResult, ExternalState};
    use async_trait::async_trait;
    use reqwest::StatusCode;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::sync::Notify;

    struct MockOperations {
        connects: Arc<AtomicUsize>,
        started: Arc<Notify>,
        release: Arc<Notify>,
    }

    #[async_trait]
    impl ControlOperations for MockOperations {
        async fn state(&self) -> ExternalState {
            ExternalState {
                protocol_version: 1,
                status_sequence: 1,
                probed: true,
                stale: false,
                error: None,
                devices: Vec::new(),
            }
        }

        async fn connect(&self, device_id: &str) -> Result<ExternalActionResult, ControlError> {
            self.connects.fetch_add(1, Ordering::SeqCst);
            self.started.notify_one();
            self.release.notified().await;
            Ok(ExternalActionResult {
                device_id: device_id.to_string(),
                status: "connected".to_string(),
                changed: true,
                warning: None,
            })
        }

        async fn disconnect(&self, device_id: &str) -> Result<ExternalActionResult, ControlError> {
            Ok(ExternalActionResult {
                device_id: device_id.to_string(),
                status: "idle".to_string(),
                changed: false,
                warning: None,
            })
        }
    }

    fn paths(tag: &str) -> (Arc<AuthManager>, PathBuf, PathBuf, PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("fcn-server-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let auth_path = root.join("external-control.json");
        let discovery_path = discovery::path_in(&root);
        (
            Arc::new(AuthManager::new(auth_path)),
            discovery_path.clone(),
            discovery_path,
            root,
        )
    }

    fn operations() -> Arc<MockOperations> {
        Arc::new(MockOperations {
            connects: Arc::new(AtomicUsize::new(0)),
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        })
    }

    fn token(discovery_path: &PathBuf) -> String {
        discovery::read(discovery_path).unwrap().unwrap().token
    }

    #[test]
    fn shutdown_does_not_touch_sessions() {
        let raw = include_str!("server.rs");
        for symbol in ["shutdown_all", "stop_sessions_of_device"] {
            let forbidden = format!("session::{symbol}");
            assert!(!raw.contains(&forbidden));
        }
    }

    #[tokio::test]
    async fn dynamic_loopback_server_discovers_serves_and_closes() {
        let (auth, discovery_path, _discovery, root) = paths("lifecycle");
        let server = RunningServer::start(operations(), auth, discovery_path.clone(), None)
            .await
            .unwrap();
        assert!(server.base_url.starts_with("http://127.0.0.1:"));
        let record = discovery::read(&discovery_path).unwrap().unwrap();
        assert_eq!(record.service, "freecontrol-fuyu");
        assert_eq!(record.base_url, server.base_url);
        let token = record.token;
        let response = reqwest::Client::new()
            .get(format!("{}/v1/state", server.base_url))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<serde_json::Value>().await.unwrap()["protocolVersion"],
            1
        );
        let port = server
            .base_url
            .rsplit_once(':')
            .unwrap()
            .1
            .parse::<u16>()
            .unwrap();
        server.shutdown().await.unwrap();
        assert!(discovery::read(&discovery_path).unwrap().is_none());
        assert!(reqwest::Client::new()
            .get(format!("http://127.0.0.1:{port}/v1/state"))
            .send()
            .await
            .is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn api_docs_are_read_only_and_separated_from_device_auth() {
        let (auth, discovery_path, _discovery, root) = paths("api-docs");
        let server = RunningServer::start(operations(), auth, discovery_path.clone(), None)
            .await
            .unwrap();
        let base_url = server.base_url.clone();
        let token = token(&discovery_path);
        let client = reqwest::Client::new();

        let docs = client
            .get(format!("{base_url}/api-docs"))
            .send()
            .await
            .unwrap();
        assert_eq!(docs.status(), StatusCode::OK);
        let docs_html = docs.text().await.unwrap();
        assert!(docs_html.contains("FreeControl API"));
        assert!(docs_html.contains("/api-docs/openapi.json"));
        assert!(docs_html.contains("只读接口文档"));
        assert!(!docs_html.contains("Try it out"));
        assert!(!docs_html.contains("https://"));
        assert!(!docs_html.contains(&token));

        let spec_response = client
            .get(format!("{base_url}/api-docs/openapi.json"))
            .send()
            .await
            .unwrap();
        assert_eq!(spec_response.status(), StatusCode::OK);
        let spec = spec_response.json::<serde_json::Value>().await.unwrap();
        assert!(spec["paths"].get("/v1/state").is_some());
        assert!(spec["paths"]
            .get("/v1/devices/{deviceId}/connect")
            .is_some());
        assert!(spec["paths"]
            .get("/v1/devices/{deviceId}/disconnect")
            .is_some());
        assert!(!spec.to_string().contains(&token));

        assert_eq!(
            client
                .get(format!("{base_url}/api-docs/openapi.json"))
                .header(reqwest::header::ORIGIN, "https://evil.example")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            client
                .get(format!("{base_url}/v1/state"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );

        server.shutdown().await.unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn restart_rotates_token_and_discovery() {
        let (auth, discovery_path, _discovery, root) = paths("restart");
        let first = RunningServer::start(
            operations(),
            auth.clone(),
            discovery_path.clone(),
            None,
        )
        .await
        .unwrap();
        let old_token = token(&discovery_path);
        first.shutdown().await.unwrap();

        let second = RunningServer::start(operations(), auth, discovery_path.clone(), None)
            .await
            .unwrap();
        let new_token = token(&discovery_path);
        assert_ne!(old_token, new_token);
        assert_eq!(
            reqwest::Client::new()
                .get(format!("{}/v1/state", second.base_url))
                .bearer_auth(old_token)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            reqwest::Client::new()
                .get(format!("{}/v1/state", second.base_url))
                .bearer_auth(new_token)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        second.shutdown().await.unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn preferred_port_is_reused_and_falls_back_when_occupied() {
        let (auth, discovery_path, _discovery, root) = paths("preferred-port");
        let first = RunningServer::start(
            operations(),
            auth.clone(),
            discovery_path.clone(),
            None,
        )
        .await
        .unwrap();
        let port = first.port;
        first.shutdown().await.unwrap();

        let second = RunningServer::start(
            operations(),
            auth.clone(),
            discovery_path.clone(),
            Some(port),
        )
        .await
        .unwrap();
        assert_eq!(second.port, port);
        second.shutdown().await.unwrap();

        let blocker = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let occupied = blocker.local_addr().unwrap().port();
        let third = RunningServer::start(operations(), auth, discovery_path, Some(occupied))
            .await
            .unwrap();
        assert_ne!(third.port, occupied);
        third.shutdown().await.unwrap();
        drop(blocker);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn preferred_port_survives_quick_restart_after_request() {
        let (auth, discovery_path, _discovery, root) = paths("preferred-port-restart");
        let first = RunningServer::start(
            operations(),
            auth.clone(),
            discovery_path.clone(),
            None,
        )
        .await
        .unwrap();
        let port = first.port;
        let token = token(&discovery_path);
        assert_eq!(
            reqwest::Client::new()
                .get(format!("{}/v1/state", first.base_url))
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        first.shutdown().await.unwrap();

        let second = RunningServer::start(
            operations(),
            auth,
            discovery_path,
            Some(port),
        )
        .await
        .unwrap();
        assert_eq!(second.port, port);
        second.shutdown().await.unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rejects_unsafe_requests_and_nonempty_action_bodies() {
        let (auth, discovery_path, _discovery, root) = paths("security");
        let operations = operations();
        let connects = operations.connects.clone();
        let release = operations.release.clone();
        let server = RunningServer::start(operations, auth, discovery_path.clone(), None)
            .await
            .unwrap();
        let base_url = server.base_url.clone();
        let token = token(&discovery_path);
        let client = reqwest::Client::new();

        assert_eq!(
            client
                .get(format!("{base_url}/v1/state"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client
                .get(format!("{base_url}/v1/state"))
                .bearer_auth(&token)
                .header(reqwest::header::HOST, "evil.example")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            client
                .get(format!("{base_url}/v1/state"))
                .bearer_auth(&token)
                .header(reqwest::header::ORIGIN, "https://evil.example")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            client
                .get(format!("{base_url}/v1/state?token={token}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            client
                .get(format!("{base_url}/v1/state"))
                .bearer_auth(&token)
                .header(reqwest::header::ORIGIN, format!("{base_url}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            client
                .post(format!("{base_url}/v1/devices/a/connect"))
                .bearer_auth(&token)
                .body("{}")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            client
                .post(format!("{base_url}/v1/devices/a/connect"))
                .bearer_auth(&token)
                .body("x".repeat(17 * 1024))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(connects.load(Ordering::SeqCst), 0);

        release.notify_one();
        let response = client
            .post(format!("{base_url}/v1/devices/a/connect"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value = response.json::<serde_json::Value>().await.unwrap();
        assert_eq!(value["deviceId"], "a");
        assert_eq!(value["status"], "connected");
        assert_eq!(value["changed"], true);
        assert_eq!(connects.load(Ordering::SeqCst), 1);
        server.shutdown().await.unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn shutdown_rejects_new_actions_and_waits_for_in_flight_action() {
        let (auth, discovery_path, _discovery, root) = paths("gate");
        let operations = operations();
        let connects = operations.connects.clone();
        let started = operations.started.clone();
        let release = operations.release.clone();
        let server = RunningServer::start(operations, auth, discovery_path, None)
            .await
            .unwrap();
        let base_url = server.base_url.clone();
        let token = token(&server.discovery_path);
        let client = reqwest::Client::new();
        let first = {
            let client = client.clone();
            let base_url = base_url.clone();
            let token = token.clone();
            tokio::spawn(async move {
                client
                    .post(format!("{base_url}/v1/devices/a/connect"))
                    .bearer_auth(token)
                    .send()
                    .await
                    .unwrap()
                    .status()
            })
        };
        started.notified().await;
        let shutdown = tokio::spawn(async move { server.shutdown().await });
        let second = client
            .post(format!("{base_url}/v1/devices/b/connect"))
            .bearer_auth(token)
            .send()
            .await;
        if let Ok(response) = second {
            assert_ne!(response.status(), StatusCode::OK);
        }
        assert_eq!(connects.load(Ordering::SeqCst), 1);
        release.notify_one();
        assert_eq!(first.await.unwrap(), StatusCode::OK);
        shutdown.await.unwrap().unwrap();
        let _ = fs::remove_dir_all(root);
    }
}

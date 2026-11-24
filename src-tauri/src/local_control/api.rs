use crate::local_control::auth::AuthManager;
use crate::local_control::core::DeviceControlService;
use crate::local_control::dto::{ExternalActionResult, ExternalState};
use crate::local_control::errors::ControlError;
use crate::local_control::limits::RateLimiter;
use async_trait::async_trait;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{header, HeaderMap};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Semaphore};

const MAX_BODY_BYTES: usize = 16 * 1024;
const GLOBAL_REQUEST_LIMIT: usize = 60;
const GLOBAL_REQUEST_WINDOW: Duration = Duration::from_secs(10);
const MAX_CONCURRENT_REQUESTS: usize = 32;
pub const API_DOCS_PATH: &str = "/api-docs";
pub const OPENAPI_PATH: &str = "/api-docs/openapi.json";
const DOCS_HTML: &str = include_str!("docs.html");
// 静态 OpenAPI 契约：接口或 DTO 变化时必须同步更新 openapi.json。
const OPENAPI_JSON: &str = include_str!("openapi.json");

#[async_trait]
pub trait ControlOperations: Send + Sync {
    async fn state(&self) -> ExternalState;
    async fn connect(&self, device_id: &str) -> Result<ExternalActionResult, ControlError>;
    async fn disconnect(&self, device_id: &str) -> Result<ExternalActionResult, ControlError>;
}

#[async_trait]
impl ControlOperations for DeviceControlService {
    async fn state(&self) -> ExternalState {
        DeviceControlService::state(self).await
    }

    async fn connect(&self, device_id: &str) -> Result<ExternalActionResult, ControlError> {
        DeviceControlService::connect(self, device_id).await
    }

    async fn disconnect(&self, device_id: &str) -> Result<ExternalActionResult, ControlError> {
        DeviceControlService::disconnect(self, device_id).await
    }
}

#[derive(Clone)]
pub struct ApiState {
    pub operations: Arc<dyn ControlOperations>,
    pub auth: Arc<AuthManager>,
    pub port: u16,
    rate_limiter: Arc<RateLimiter>,
    concurrency: Arc<Semaphore>,
    pub accepting: Arc<AtomicBool>,
    pub action_gate: Arc<RwLock<()>>,
}

impl ApiState {
    pub fn new(operations: Arc<dyn ControlOperations>, auth: Arc<AuthManager>, port: u16) -> Self {
        Self {
            operations,
            auth,
            port,
            rate_limiter: Arc::new(RateLimiter::default()),
            concurrency: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            accepting: Arc::new(AtomicBool::new(true)),
            action_gate: Arc::new(RwLock::new(())),
        }
    }
}

pub fn router(state: ApiState) -> Router {
    let device_router = Router::new()
        .route("/v1/state", get(state_handler))
        .route("/v1/devices/{device_id}/connect", post(connect))
        .route("/v1/devices/{device_id}/disconnect", post(disconnect))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), request_guard));
    let docs_router: Router<ApiState> = Router::new()
        .route(API_DOCS_PATH, get(docs_page))
        .route(OPENAPI_PATH, get(openapi_json));
    let docs_router = docs_router.layer(middleware::from_fn_with_state(state.clone(), docs_guard));
    Router::new()
        .merge(device_router)
        .merge(docs_router)
        .with_state(state)
}

async fn docs_page() -> axum::response::Html<&'static str> {
    axum::response::Html(DOCS_HTML)
}

async fn openapi_json() -> Response {
    ([(header::CONTENT_TYPE, "application/json")], OPENAPI_JSON).into_response()
}

async fn docs_guard(
    State(state): State<ApiState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if let Err(error) = validate_host_and_origin(&state, request.headers()) {
        return error.into_response();
    }
    next.run(request).await
}

async fn request_guard(
    State(state): State<ApiState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if request.uri().query().is_some() {
        return ControlError::InvalidRequest.into_response();
    }
    if let Err(error) = validate_host_and_origin(&state, request.headers()) {
        return error.into_response();
    }
    if !state
        .rate_limiter
        .allow(GLOBAL_REQUEST_LIMIT, GLOBAL_REQUEST_WINDOW)
    {
        return ControlError::RateLimited.into_response();
    }
    let _permit = match Arc::clone(&state.concurrency).try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return ControlError::RateLimited.into_response(),
    };
    let path = request.uri().path().to_string();
    if !bearer_valid(&state, request.headers()) {
        return ControlError::Unauthorized.into_response();
    }
    let _action_guard = if path.ends_with("/connect") || path.ends_with("/disconnect") {
        if !state.accepting.load(Ordering::SeqCst) {
            return ControlError::ServiceUnavailable.into_response();
        }
        let guard = state.action_gate.read().await;
        if !state.accepting.load(Ordering::SeqCst) {
            return ControlError::ServiceUnavailable.into_response();
        }
        Some(guard)
    } else {
        None
    };
    next.run(request).await
}

fn validate_host_and_origin(state: &ApiState, headers: &HeaderMap) -> Result<(), ControlError> {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .ok_or(ControlError::Forbidden)?;
    let ip_host = format!("127.0.0.1:{}", state.port);
    let local_host = format!("localhost:{}", state.port);
    if host != ip_host && host != local_host {
        return Err(ControlError::Forbidden);
    }
    if let Some(origin) = headers.get(header::ORIGIN) {
        let origin = origin.to_str().map_err(|_| ControlError::Forbidden)?;
        if origin != format!("http://{ip_host}") && origin != format!("http://{local_host}") {
            return Err(ControlError::Forbidden);
        }
    }
    Ok(())
}

fn bearer_valid(state: &ApiState, headers: &HeaderMap) -> bool {
    let Some(value) = headers.get(header::AUTHORIZATION) else {
        return false;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };
    let Some((scheme, token)) = value.split_once(' ') else {
        return false;
    };
    scheme.eq_ignore_ascii_case("Bearer") && !token.is_empty() && state.auth.verify(token)
}

async fn state_handler(State(state): State<ApiState>) -> Result<Json<ExternalState>, ControlError> {
    let state = tokio::time::timeout(Duration::from_secs(2), state.operations.state())
        .await
        .map_err(|_| ControlError::ServiceUnavailable)?;
    Ok(Json(state))
}

async fn connect(
    State(state): State<ApiState>,
    Path(device_id): Path<String>,
    body: Bytes,
) -> Result<Json<ExternalActionResult>, ControlError> {
    ensure_empty_body(&body)?;
    state.operations.connect(&device_id).await.map(Json)
}

async fn disconnect(
    State(state): State<ApiState>,
    Path(device_id): Path<String>,
    body: Bytes,
) -> Result<Json<ExternalActionResult>, ControlError> {
    ensure_empty_body(&body)?;
    state.operations.disconnect(&device_id).await.map(Json)
}

fn ensure_empty_body(body: &Bytes) -> Result<(), ControlError> {
    if body.is_empty() {
        Ok(())
    } else {
        Err(ControlError::InvalidRequest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_openapi_covers_device_routes_and_bearer_scheme() {
        let value: serde_json::Value = serde_json::from_str(OPENAPI_JSON).unwrap();
        let paths = value["paths"].as_object().unwrap();
        assert!(paths.contains_key("/v1/state"));
        assert!(paths.contains_key("/v1/devices/{deviceId}/connect"));
        assert!(paths.contains_key("/v1/devices/{deviceId}/disconnect"));
        assert_eq!(value["security"][0]["bearer_auth"], serde_json::json!([]));
        assert_eq!(
            value["components"]["securitySchemes"]["bearer_auth"]["type"],
            "http"
        );
        assert_eq!(
            value["components"]["securitySchemes"]["bearer_auth"]["scheme"],
            "bearer"
        );
        assert!(value["components"]["schemas"]
            .get("ExternalState")
            .is_some());
        assert!(value["components"]["schemas"]
            .get("ExternalActionResult")
            .is_some());
        assert!(value["components"]["schemas"].get("ErrorBody").is_some());
    }
}

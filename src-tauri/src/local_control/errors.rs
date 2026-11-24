use crate::ui_message::{codes, UserError};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlError {
    InvalidRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    DeviceOffline,
    Conflict,
    OperationFailed,
    RateLimited,
    ServiceUnavailable,
    Internal,
}

impl ControlError {
    pub fn status(self) -> StatusCode {
        match self {
            Self::InvalidRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::DeviceOffline => StatusCode::CONFLICT,
            Self::Conflict => StatusCode::CONFLICT,
            Self::OperationFailed => StatusCode::UNPROCESSABLE_ENTITY,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn body(self) -> ErrorBody {
        let (code, message, retryable) = match self {
            Self::InvalidRequest => ("invalid_request", "请求格式不正确。", false),
            Self::Unauthorized => ("unauthorized", "访问凭据无效或已撤销。", false),
            Self::Forbidden => ("forbidden", "当前来源无权访问本地控制服务。", false),
            Self::NotFound => ("device_not_found", "设备不存在。", false),
            Self::DeviceOffline => ("device_unavailable", "设备当前不可连接。", true),
            Self::Conflict => ("operation_conflict", "设备操作正在处理中。", true),
            Self::OperationFailed => (
                "operation_failed",
                "设备操作失败，请查看 FreeControl 运行日志。",
                true,
            ),
            Self::RateLimited => ("rate_limited", "请求过于频繁，请稍后重试。", true),
            Self::ServiceUnavailable => ("service_unavailable", "本地控制服务暂不可用。", true),
            Self::Internal => ("internal_error", "本地控制服务发生内部错误。", true),
        };
        ErrorBody {
            code: code.to_string(),
            message: message.to_string(),
            retryable,
        }
    }

    pub fn from_user_error(error: &UserError) -> Self {
        let mapped = match error.code.as_str() {
            codes::DEVICE_NOT_FOUND | codes::SESSION_DEVICE_NOT_FOUND => Self::NotFound,
            codes::SESSION_DEVICE_OFFLINE | codes::SESSION_CONNECTION_UNKNOWN => {
                Self::DeviceOffline
            }
            codes::SESSION_ALREADY_RUNNING => Self::Conflict,
            codes::SESSION_ENVIRONMENT_NOT_READY
            | codes::SESSION_LINK_FAILED
            | codes::SESSION_START_FAILED
            | codes::SESSION_STOP_FAILED => Self::OperationFailed,
            _ => Self::Internal,
        };
        if let Some(detail) = &error.detail {
            crate::logger::warn(
                "local-control",
                format!("设备操作失败（{}）：{detail}", error.code),
            );
        }
        mapped
    }
}

impl IntoResponse for ControlError {
    fn into_response(self) -> Response {
        let mut response = (self.status(), Json(self.body())).into_response();
        if self == Self::Unauthorized {
            response.headers_mut().insert(
                axum::http::header::WWW_AUTHENTICATE,
                axum::http::HeaderValue::from_static("Bearer"),
            );
        }
        response
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn user_error_details_never_reach_external_body() {
        let error = UserError::new(codes::SESSION_START_FAILED)
            .with_detail("C:/private/scrcpy --secret-token");
        let mapped = ControlError::from_user_error(&error);
        assert_eq!(mapped, ControlError::OperationFailed);
        let body = serde_json::to_value(mapped.body()).unwrap();
        assert_eq!(body["code"], "operation_failed");
        assert!(!body.to_string().contains("private"));
        assert!(!body.to_string().contains("secret-token"));
    }

    #[test]
    fn device_and_environment_failures_use_stable_codes() {
        for (code, expected) in [
            (codes::DEVICE_NOT_FOUND, ControlError::NotFound),
            (codes::SESSION_DEVICE_OFFLINE, ControlError::DeviceOffline),
            (
                codes::SESSION_ENVIRONMENT_NOT_READY,
                ControlError::OperationFailed,
            ),
            (codes::SESSION_LINK_FAILED, ControlError::OperationFailed),
            (codes::SESSION_START_FAILED, ControlError::OperationFailed),
        ] {
            let error = UserError::new(code).with_detail("adb C:/private/path failed");
            assert_eq!(ControlError::from_user_error(&error), expected);
            assert!(!ControlError::from_user_error(&error)
                .body()
                .message
                .contains("private"));
        }
    }

    #[test]
    fn stable_error_codes_and_statuses() {
        assert_eq!(ControlError::NotFound.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            ControlError::Unauthorized.body(),
            ErrorBody {
                code: "unauthorized".to_string(),
                message: "访问凭据无效或已撤销。".to_string(),
                retryable: false,
            }
        );
        assert_eq!(
            serde_json::to_value(ControlError::RateLimited.body()).unwrap()["retryable"],
            json!(true)
        );
    }
}

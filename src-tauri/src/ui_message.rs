//! 后端 → 前端的用户可见消息。
//!
//! 只携带稳定的 code 与参数；文案由前端字典按当前语言出。技术细节放 `detail`，
//! 原样展示；日志文件、会话输出与 scrcpy / adb 原文不走这里。
//!
//! `params` 的形状由各 code 自行约定（通常是 `{ version, link }` 这类短参数），
//! 前端按同一个 code 查字典并插值。

use serde::Serialize;

/// 用户可见消息：稳定 code + 可选参数。
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserMessage {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl UserMessage {
    pub fn new(code: impl Into<String>) -> Self {
        UserMessage {
            code: code.into(),
            params: None,
        }
    }

    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }
}

/// 命令失败：消息 + 可选技术细节。
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserError {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl UserError {
    pub fn new(code: impl Into<String>) -> Self {
        UserError {
            code: code.into(),
            params: None,
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl From<String> for UserError {
    fn from(detail: String) -> Self {
        UserError::new(codes::INTERNAL).with_detail(detail)
    }
}

/// 下载通道的展示形状：官方通道没有地址，代理通道带自己的前缀地址。
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChannelView {
    Official,
    Proxy { url: String },
}

/// 后端上屏消息的稳定 code。前端字典按同一组键出文案。
pub mod codes {
    pub const INTERNAL: &str = "error.internal";
    pub const CONFIG_SAVE_FAILED: &str = "config.saveFailed";

    pub const DEVICE_INVALID: &str = "device.invalid";
    pub const DEVICE_NOT_FOUND: &str = "device.notFound";
    pub const DEVICE_STATUS_PROBE_FAILED: &str = "device.statusProbeFailed";

    // 无线配对与发现（spec `device-management` 的三条新增要求）。
    pub const WIRELESS_ADDRESS_INVALID: &str = "wireless.addressInvalid";
    pub const WIRELESS_CODE_REQUIRED: &str = "wireless.codeRequired";
    pub const WIRELESS_PAIR_FAILED: &str = "wireless.pairFailed";
    pub const WIRELESS_PORT_UNRESOLVED: &str = "wireless.portUnresolved";
    pub const WIRELESS_CONNECT_FAILED: &str = "wireless.connectFailed";
    pub const WIRELESS_MDNS_FAILED: &str = "wireless.mdnsFailed";
    pub const WIRELESS_SWEEP_UNAVAILABLE: &str = "wireless.sweepUnavailable";
    pub const WIRELESS_NOT_WIRELESS: &str = "wireless.notWireless";

    pub const SESSION_DEVICE_NOT_FOUND: &str = "session.deviceNotFound";
    pub const SESSION_DEVICE_OFFLINE: &str = "session.deviceOffline";
    pub const SESSION_CONNECTION_UNKNOWN: &str = "session.connectionUnknown";
    pub const SESSION_ALREADY_RUNNING: &str = "session.alreadyRunning";
    pub const SESSION_ENVIRONMENT_NOT_READY: &str = "session.environmentNotReady";
    pub const SESSION_LINK_FAILED: &str = "session.linkFailed";
    pub const SESSION_START_FAILED: &str = "session.startFailed";
    pub const SESSION_NO_SESSION: &str = "session.noSession";
    pub const SESSION_NO_RECORD: &str = "session.noRecord";
    pub const SESSION_STOP_FAILED: &str = "session.stopFailed";
    pub const SESSION_WINDOW_ARRANGEMENT_DEGRADED: &str = "session.windowArrangementDegraded";

    pub const ENV_INVALID_DIR: &str = "environment.invalidDir";
    pub const ENV_DIR_LOCKED: &str = "environment.dirLocked";

    pub const INSTALL_FAILED: &str = "install.failed";
    pub const INSTALL_CONFIRM_FAILED: &str = "install.confirmFailed";
    pub const INSTALL_NO_INSTALL: &str = "install.noInstall";
    pub const INSTALL_UNVERIFIED: &str = "install.unverified";
    pub const INSTALL_FALLBACK_TO_BEST_FIT: &str = "install.fallbackToBestFit";

    pub const UPDATE_CHECK_FAILED: &str = "update.checkFailed";

    pub const PROXY_INVALID: &str = "proxy.invalid";
    pub const PROXY_DUPLICATE: &str = "proxy.duplicate";
    pub const PROXY_NOT_FOUND: &str = "proxy.notFound";

    pub const LINK_UNKNOWN: &str = "link.unknown";
    pub const LINK_OPEN_FAILED: &str = "link.openFailed";
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn user_error_omits_absent_optional_fields() {
        let value = serde_json::to_value(UserError::new(codes::SESSION_NO_SESSION)).unwrap();
        assert_eq!(value, json!({ "code": "session.noSession" }));

        let value = serde_json::to_value(
            UserError::new(codes::INSTALL_FAILED).with_detail("boom"),
        )
        .unwrap();
        assert_eq!(
            value,
            json!({ "code": "install.failed", "detail": "boom" })
        );
    }

    #[test]
    fn user_message_carries_params_and_serializes_channel_view() {
        let value = serde_json::to_value(
            UserMessage::new(codes::INSTALL_FALLBACK_TO_BEST_FIT)
                .with_params(json!({ "version": "5.0.1" })),
        )
        .unwrap();
        assert_eq!(
            value,
            json!({ "code": "install.fallbackToBestFit", "params": { "version": "5.0.1" } })
        );

        assert_eq!(
            serde_json::to_value(ChannelView::Official).unwrap(),
            json!({ "kind": "official" })
        );
        assert_eq!(
            serde_json::to_value(ChannelView::Proxy {
                url: "https://gh-proxy.org".to_string()
            })
            .unwrap(),
            json!({ "kind": "proxy", "url": "https://gh-proxy.org" })
        );
    }
}

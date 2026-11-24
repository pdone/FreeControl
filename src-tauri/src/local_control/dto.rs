use crate::config::Device;
use crate::device::DeviceStatusPayload;
use serde::Serialize;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalDevice {
    pub id: String,
    pub alias: String,
    pub kind: String,
    pub connection_id: String,
    pub status: String,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalState {
    pub protocol_version: u32,
    pub status_sequence: u64,
    pub probed: bool,
    pub stale: bool,
    pub error: Option<String>,
    pub devices: Vec<ExternalDevice>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalActionResult {
    pub device_id: String,
    pub status: String,
    pub changed: bool,
    pub warning: Option<String>,
}

impl ExternalState {
    pub fn from_payload(payload: &DeviceStatusPayload) -> Self {
        let devices = payload
            .devices
            .iter()
            .map(|device| ExternalDevice {
                id: device.id.clone(),
                alias: device.alias.clone(),
                kind: device.kind.clone(),
                connection_id: device.connection_id(),
                status: external_status(payload, device),
            })
            .collect();
        Self {
            protocol_version: PROTOCOL_VERSION,
            status_sequence: payload.status_sequence,
            probed: payload.probed,
            stale: payload.stale,
            error: payload
                .error
                .as_ref()
                .map(|_| "设备状态暂不可用".to_string()),
            devices,
        }
    }
}

fn external_status(payload: &DeviceStatusPayload, device: &Device) -> String {
    match payload.statuses.get(&device.id).map(String::as_str) {
        Some(crate::device::STATUS_CONNECTED) => crate::device::STATUS_CONNECTED.to_string(),
        _ if !payload.probed => "unknown".to_string(),
        Some(status) => status.to_string(),
        None => crate::device::STATUS_OFFLINE.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{KIND_USB, KIND_WIFI};
    use crate::device::{STATUS_CONNECTED, STATUS_IDLE, STATUS_OFFLINE};
    use crate::ui_message::{codes, UserError};
    use serde_json::json;
    use std::collections::HashMap;

    fn device(id: &str, kind: &str, connection_id: &str) -> Device {
        Device {
            id: id.to_string(),
            alias: format!("别名-{id}"),
            kind: kind.to_string(),
            serial: if kind == KIND_USB {
                connection_id.to_string()
            } else {
                String::new()
            },
            ip: if kind == KIND_WIFI {
                "192.0.2.10".to_string()
            } else {
                String::new()
            },
            port: if kind == KIND_WIFI { 5555 } else { 0 },
            last_session_started_at: Some(42),
        }
    }

    #[test]
    fn projects_only_external_device_fields_and_sanitizes_probe_error() {
        let devices = vec![
            device("a", KIND_USB, "SERIAL-A"),
            device("b", KIND_WIFI, ""),
            device("c", KIND_USB, "SERIAL-C"),
        ];
        let payload = DeviceStatusPayload {
            statuses: HashMap::from([
                ("a".to_string(), STATUS_CONNECTED.to_string()),
                ("b".to_string(), STATUS_IDLE.to_string()),
                ("c".to_string(), STATUS_OFFLINE.to_string()),
            ]),
            stale: true,
            probed: true,
            error: Some(
                UserError::new(codes::DEVICE_STATUS_PROBE_FAILED)
                    .with_detail("C:/secret/adb failed"),
            ),
            devices,
            device_list: crate::config::DeviceList::default(),
            status_sequence: 9,
        };
        let state = ExternalState::from_payload(&payload);
        assert_eq!(state.protocol_version, PROTOCOL_VERSION);
        assert_eq!(state.status_sequence, 9);
        assert!(state.stale);
        assert_eq!(state.error.as_deref(), Some("设备状态暂不可用"));
        assert_eq!(state.devices[0].status, STATUS_CONNECTED);
        assert_eq!(state.devices[1].status, STATUS_IDLE);
        assert_eq!(state.devices[2].status, STATUS_OFFLINE);

        let value = serde_json::to_value(state).unwrap();
        let raw = value.to_string();
        assert!(!raw.contains("secret"));
        assert!(!raw.contains("lastSessionStartedAt"));
        assert!(!raw.contains("deviceList"));
        assert_eq!(value["devices"][0]["connectionId"], "SERIAL-A");
        assert_eq!(value["devices"][1]["connectionId"], "192.0.2.10:5555");
        assert!(value["devices"][0].get("serial").is_none());
    }

    #[test]
    fn unprobed_devices_are_unknown_but_running_sessions_stay_connected() {
        let payload = DeviceStatusPayload {
            statuses: HashMap::from([
                ("a".to_string(), STATUS_CONNECTED.to_string()),
                ("b".to_string(), STATUS_OFFLINE.to_string()),
            ]),
            stale: false,
            probed: false,
            error: None,
            devices: vec![device("a", KIND_USB, "A"), device("b", KIND_USB, "B")],
            device_list: crate::config::DeviceList::default(),
            status_sequence: 1,
        };
        let state = ExternalState::from_payload(&payload);
        assert_eq!(state.devices[0].status, STATUS_CONNECTED);
        assert_eq!(state.devices[1].status, "unknown");
        assert_eq!(serde_json::to_value(state).unwrap()["probed"], json!(false));
    }
}

use crate::config::TelldusCredentials;
use reqwest::blocking::Client;
use reqwest_oauth1::{OAuthClientProvider, Secrets};
use serde_json::Value;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

const BASE_URL: &str = "https://pa-api.telldus.com";
const MIN_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("OAuth request failed: {0}")]
    OAuth(#[from] reqwest_oauth1::Error),
    #[error("unexpected Telldus response: {0}")]
    Unexpected(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Controller,
    Device,
    Sensor,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Controller => "controller",
            Category::Device => "device",
            Category::Sensor => "sensor",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub category: Category,
    pub id: String,
    pub name: String,
    pub details: Option<String>,
}

pub struct TelldusApi<'a> {
    client: &'a Client,
    credentials: &'a TelldusCredentials,
}

pub struct AddDeviceRequest<'a> {
    pub client_id: &'a str,
    pub name: &'a str,
    pub protocol: &'a str,
    pub model: &'a str,
}

impl<'a> TelldusApi<'a> {
    pub fn new(client: &'a Client, credentials: &'a TelldusCredentials) -> Self {
        Self {
            client,
            credentials,
        }
    }

    pub fn list_controllers(&self) -> Result<Vec<Entry>, ApiError> {
        let payload = self.get_json("/json/clients/list", &[])?;
        let items = array_from(&payload, &["client", "clients"]);
        Ok(items
            .into_iter()
            .map(|client| {
                let id = pick_string(&client, &["id", "clientId"]).unwrap_or_else(|| "?".into());
                let name = pick_string(&client, &["name", "clientName"])
                    .unwrap_or_else(|| "(controller)".into());
                let mut details = Vec::new();
                if let Some(online) = pick_string(&client, &["online"]) {
                    if matches!(online.as_str(), "1" | "true" | "True" | "TRUE") {
                        details.push("online".into());
                    } else if matches!(online.as_str(), "0" | "false" | "False" | "FALSE") {
                        details.push("offline".into());
                    }
                }
                if let Some(last_seen) = pick_string(&client, &["lastSeen", "lastseen"]) {
                    if !last_seen.is_empty() && last_seen != "0" {
                        details.push(format!("lastSeen={last_seen}"));
                    }
                }
                if let Some(firmware) = pick_string(&client, &["firmware", "firmwareVersion"]) {
                    details.push(format!("fw={firmware}"));
                }
                Entry {
                    category: Category::Controller,
                    id,
                    name,
                    details: details_to_string(details),
                }
            })
            .collect())
    }

    pub fn list_devices(&self) -> Result<Vec<Entry>, ApiError> {
        let payload = self.get_json("/json/devices/list", &[])?;
        let items = array_from(&payload, &["device", "devices"]);
        Ok(items
            .into_iter()
            .map(|device| {
                let id = pick_string(&device, &["id", "deviceId"]).unwrap_or_else(|| "?".into());
                let name =
                    pick_string(&device, &["name"]).unwrap_or_else(|| "(unnamed device)".into());
                let mut details = Vec::new();
                if let Some(model) = pick_string(&device, &["model", "deviceType", "type"]) {
                    details.push(model);
                }
                if let Some(state) = pick_string(&device, &["statevalue", "state", "stateValue"]) {
                    if !state.is_empty() {
                        details.push(format!("state={state}"));
                    }
                }
                if let Some(client_name) = pick_string(&device, &["clientName"]) {
                    details.push(format!("client={client_name}"));
                }
                Entry {
                    category: Category::Device,
                    id,
                    name,
                    details: details_to_string(details),
                }
            })
            .collect())
    }

    pub fn list_sensors(&self) -> Result<Vec<Entry>, ApiError> {
        let payload = self.get_json(
            "/json/sensors/list",
            &[
                ("includeIgnored", "0"),
                ("includeValues", "1"),
                ("includeScale", "1"),
            ],
        )?;
        let items = array_from(&payload, &["sensor", "sensors"]);
        Ok(items
            .into_iter()
            .map(|sensor| {
                let id = pick_string(&sensor, &["id", "sensorId"]).unwrap_or_else(|| "?".into());
                let name =
                    pick_string(&sensor, &["name"]).unwrap_or_else(|| "(unnamed sensor)".into());
                let mut details = Vec::new();
                if let Some(model) = pick_string(&sensor, &["model"]) {
                    details.push(model);
                }
                if let Some(protocol) = pick_string(&sensor, &["protocol"]) {
                    details.push(format!("protocol={protocol}"));
                }
                if let Some(data) = sensor.get("data").and_then(Value::as_array) {
                    let mut values = Vec::new();
                    for entry in data {
                        if let Some(name) = pick_string(entry, &["name"]) {
                            let value = pick_string(entry, &["value"]).unwrap_or_default();
                            let scale = pick_string(entry, &["scale"]).unwrap_or_default();
                            let mut sample = format!("{name}={value}");
                            if !scale.is_empty() {
                                sample.push_str(&format!("@{scale}"));
                            }
                            values.push(sample);
                        }
                    }
                    if !values.is_empty() {
                        details.push(values.join(", "));
                    }
                }
                Entry {
                    category: Category::Sensor,
                    id,
                    name,
                    details: details_to_string(details),
                }
            })
            .collect())
    }

    pub fn device_turn_on(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/turnOn", id, vec![])
    }

    pub fn device_turn_off(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/turnOff", id, vec![])
    }

    pub fn device_dim(&self, id: &str, level: u8) -> Result<(), ApiError> {
        self.device_action(
            "/json/device/dim",
            id,
            vec![("level".into(), level.to_string())],
        )
    }

    pub fn device_bell(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/bell", id, vec![])
    }

    pub fn device_up(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/up", id, vec![])
    }

    pub fn device_down(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/down", id, vec![])
    }

    pub fn device_stop(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/stop", id, vec![])
    }

    pub fn device_execute(&self, id: &str, command: i32) -> Result<(), ApiError> {
        self.device_action(
            "/json/device/execute",
            id,
            vec![("command".into(), command.to_string())],
        )
    }

    pub fn device_learn(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/learn", id, vec![])
    }

    pub fn set_device_name(&self, id: &str, name: &str) -> Result<(), ApiError> {
        let payload = self.get_json_owned(
            "/json/device/setName",
            vec![("id".into(), id.into()), ("name".into(), name.into())],
        )?;
        ensure_success(&payload)
    }

    pub fn set_device_model(&self, id: &str, model: &str) -> Result<(), ApiError> {
        let payload = self.get_json_owned(
            "/json/device/setModel",
            vec![("id".into(), id.into()), ("model".into(), model.into())],
        )?;
        ensure_success(&payload)
    }

    pub fn set_device_protocol(&self, id: &str, protocol: &str) -> Result<(), ApiError> {
        let payload = self.get_json_owned(
            "/json/device/setProtocol",
            vec![
                ("id".into(), id.into()),
                ("protocol".into(), protocol.into()),
            ],
        )?;
        ensure_success(&payload)
    }

    pub fn set_device_parameter(
        &self,
        id: &str,
        parameter: &str,
        value: &str,
    ) -> Result<(), ApiError> {
        let payload = self.get_json_owned(
            "/json/device/setDeviceParameter",
            vec![
                ("id".into(), id.into()),
                ("parameter".into(), parameter.into()),
                ("value".into(), value.into()),
            ],
        )?;
        ensure_success(&payload)
    }

    pub fn get_device_parameter(
        &self,
        id: &str,
        parameter: &str,
    ) -> Result<Option<String>, ApiError> {
        let payload = self.get_json_owned(
            "/json/device/getDeviceParameter",
            vec![
                ("id".into(), id.into()),
                ("parameter".into(), parameter.into()),
            ],
        )?;
        Ok(payload
            .get("value")
            .and_then(Value::as_str)
            .map(|s| s.to_string()))
    }

    pub fn device_info(&self, id: &str) -> Result<Value, ApiError> {
        self.get_json("/json/device/info", &[("id", id)])
    }

    pub fn device_history(&self, id: &str, limit: Option<u32>) -> Result<Vec<Value>, ApiError> {
        let mut params = vec![("id".into(), id.into())];
        if let Some(limit) = limit {
            params.push(("limit".into(), limit.to_string()));
        }
        let payload = self.get_json_owned("/json/device/history", params)?;
        Ok(array_from(&payload, &["history"]))
    }

    pub fn sensor_info(&self, id: &str, scale: Option<i32>) -> Result<Value, ApiError> {
        let mut params = vec![("id".into(), id.into())];
        if let Some(scale) = scale {
            params.push(("scale".into(), scale.to_string()));
        }
        self.get_json_owned("/json/sensor/info", params)
    }

    pub fn sensor_history(
        &self,
        id: &str,
        scale: i32,
        limit: Option<u32>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut params = vec![
            ("id".into(), id.into()),
            ("scale".into(), scale.to_string()),
        ];
        if let Some(limit) = limit {
            params.push(("limit".into(), limit.to_string()));
        }
        let payload = self.get_json_owned("/json/sensor/history", params)?;
        Ok(array_from(&payload, &["history"]))
    }

    pub fn device_turn_on(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/turnOn", id, Vec::new())
    }

    pub fn device_turn_off(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/turnOff", id, Vec::new())
    }

    pub fn device_dim(&self, id: &str, level: u8) -> Result<(), ApiError> {
        self.device_action(
            "/json/device/dim",
            id,
            vec![("level".into(), level.to_string())],
        )
    }

    pub fn device_bell(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/bell", id, Vec::new())
    }

    pub fn device_up(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/up", id, Vec::new())
    }

    pub fn device_down(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/down", id, Vec::new())
    }

    pub fn device_stop(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/stop", id, Vec::new())
    }

    pub fn device_execute(&self, id: &str, command: i32) -> Result<(), ApiError> {
        self.device_action(
            "/json/device/execute",
            id,
            vec![("command".into(), command.to_string())],
        )
    }

    pub fn device_learn(&self, id: &str) -> Result<(), ApiError> {
        self.device_action("/json/device/learn", id, Vec::new())
    }

    pub fn set_device_parameter(
        &self,
        id: &str,
        parameter: &str,
        value: &str,
    ) -> Result<(), ApiError> {
        let payload = self.get_json_owned(
            "/json/device/setDeviceParameter",
            vec![
                ("id".into(), id.into()),
                ("parameter".into(), parameter.into()),
                ("value".into(), value.into()),
            ],
        )?;
        ensure_success(&payload)
    }

    pub fn get_device_parameter(
        &self,
        id: &str,
        parameter: &str,
    ) -> Result<Option<String>, ApiError> {
        let payload = self.get_json_owned(
            "/json/device/getDeviceParameter",
            vec![
                ("id".into(), id.into()),
                ("parameter".into(), parameter.into()),
            ],
        )?;
        Ok(payload
            .get("value")
            .and_then(Value::as_str)
            .map(|s| s.to_string()))
    }

    pub fn add_device(&self, request: AddDeviceRequest<'_>) -> Result<String, ApiError> {
        let payload = self.get_json_owned(
            "/json/device/add",
            vec![
                ("clientId".into(), request.client_id.into()),
                ("name".into(), request.name.into()),
                ("protocol".into(), request.protocol.into()),
                ("model".into(), request.model.into()),
            ],
        )?;
        ensure_success(&payload)?;
        payload
            .get("id")
            .or_else(|| payload.get("deviceId"))
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or_else(|| ApiError::Unexpected("device/add did not return an id".into()))
    }

    pub fn remove_device(&self, id: &str) -> Result<(), ApiError> {
        let payload = self.get_json("/json/device/remove", &[("id", id)])?;
        ensure_success(&payload)
    }

    pub fn device_info(&self, id: &str) -> Result<Value, ApiError> {
        self.get_json("/json/device/info", &[("id", id)])
    }

    pub fn device_history(&self, id: &str, limit: Option<u32>) -> Result<Vec<Value>, ApiError> {
        let mut params = vec![("id".into(), id.into())];
        if let Some(limit) = limit {
            params.push(("limit".into(), limit.to_string()));
        }
        let payload = self.get_json_owned("/json/device/history", params)?;
        Ok(array_from(&payload, &["history"]))
    }

    pub fn sensor_info(&self, id: &str, scale: Option<i32>) -> Result<Value, ApiError> {
        let mut params = vec![("id".into(), id.into())];
        if let Some(scale) = scale {
            params.push(("scale".into(), scale.to_string()));
        }
        self.get_json_owned("/json/sensor/info", params)
    }

    pub fn sensor_history(
        &self,
        id: &str,
        scale: i32,
        limit: Option<u32>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut params = vec![
            ("id".into(), id.into()),
            ("scale".into(), scale.to_string()),
        ];
        if let Some(limit) = limit {
            params.push(("limit".into(), limit.to_string()));
        }
        let payload = self.get_json_owned("/json/sensor/history", params)?;
        Ok(array_from(&payload, &["history"]))
    }

    fn get_json(&self, path: &str, params: &[(&str, &str)]) -> Result<Value, ApiError> {
        let url = format!("{BASE_URL}{path}");
        let secrets = Secrets::new(&self.credentials.public_key, &self.credentials.private_key)
            .token(&self.credentials.token, &self.credentials.token_secret);

        let mut request = self.client.clone().oauth1(secrets).get(&url);
        if !params.is_empty() {
            request = request.query(&params);
        }
        wait_for_rate_limit();
        let response = request.send()?.error_for_status()?.text()?;
        serde_json::from_str(&response).map_err(|err| ApiError::Unexpected(err.to_string()))
    }

    fn get_json_owned(&self, path: &str, params: Vec<(String, String)>) -> Result<Value, ApiError> {
        let pairs = params_to_slice(&params);
        self.get_json(path, &pairs)
    }

    fn device_action(
        &self,
        path: &str,
        id: &str,
        mut extra: Vec<(String, String)>,
    ) -> Result<(), ApiError> {
        extra.insert(0, ("id".into(), id.into()));
        let payload = self.get_json_owned(path, extra)?;
        ensure_success(&payload)
    }
}

fn wait_for_rate_limit() {
    static LAST_REQUEST: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    let lock = LAST_REQUEST.get_or_init(|| Mutex::new(None));

    let mut guard = lock.lock().expect("rate limiter poisoned");
    if let Some(last) = *guard {
        let elapsed = last.elapsed();
        if elapsed < MIN_REQUEST_INTERVAL {
            thread::sleep(MIN_REQUEST_INTERVAL - elapsed);
        }
    }
    *guard = Some(Instant::now());
}

fn array_from(value: &Value, keys: &[&str]) -> Vec<Value> {
    if let Some(array) = value.as_array() {
        return array.clone();
    }
    for key in keys {
        if let Some(array) = value.get(*key).and_then(Value::as_array) {
            return array.clone();
        }
    }
    Vec::new()
}

fn pick_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(found) = value.get(*key) {
            if let Some(text) = value_as_string(found) {
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn details_to_string(mut parts: Vec<String>) -> Option<String> {
    parts.retain(|part| !part.trim().is_empty());
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn ensure_success(value: &Value) -> Result<(), ApiError> {
    if let Some(status) = value.get("status").and_then(Value::as_str) {
        if status.eq_ignore_ascii_case("success") {
            return Ok(());
        }
        let detail = value
            .get("error")
            .or_else(|| value.get("message"))
            .map(Value::to_string)
            .unwrap_or_else(|| value.to_string());
        return Err(ApiError::Unexpected(format!("{status}: {detail}")));
    }

    if value.get("error").is_some() || value.get("message").is_some() {
        return Err(ApiError::Unexpected(value.to_string()));
    }

    Ok(())
}

fn params_to_slice(params: &[(String, String)]) -> Vec<(&str, &str)> {
    params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

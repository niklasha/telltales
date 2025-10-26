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

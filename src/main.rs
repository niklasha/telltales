mod api;
mod auth;
mod config;
mod http_client;

use api::{AddDeviceRequest, TelldusApi};
use clap::{Parser, Subcommand, ValueEnum};
use config::{TelldusCredentials, credentials_path, ensure_credentials, save_credentials};
use http_client::build_http_client;
use serde_json::to_string_pretty;
use std::process::ExitCode;
use thiserror::Error;

#[derive(Parser)]
#[command(name = "telltales", version, about = "Telldus Live CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage Telldus Live authentication
    Auth {
        #[command(subcommand)]
        command: Option<AuthCommand>,
    },
    /// Interact with Telldus Live devices
    Devices {
        #[command(subcommand)]
        command: Option<DeviceCommand>,
    },
    /// Inspect Telldus Live sensors
    Sensors {
        #[command(subcommand)]
        command: Option<SensorCommand>,
    },
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Ensure credentials are present and valid locally
    Validate,
}

#[derive(Subcommand)]
enum DeviceCommand {
    /// List Telldus Live resources
    List {
        /// Filter to a specific resource category
        #[arg(short, long, value_enum, default_value_t = DeviceKind::All)]
        kind: DeviceKind,
    },
    /// Update Telldus Live device metadata
    Edit {
        /// Telldus device identifier
        #[arg(long = "id")]
        device_id: String,
        /// New display name
        #[arg(long)]
        name: Option<String>,
        /// New protocol value
        #[arg(long)]
        protocol: Option<String>,
        /// New model value
        #[arg(long)]
        model: Option<String>,
    },
<<<<<<< HEAD
=======
    /// Register a new Telldus Live device
    Add {
        /// Controller (client) identifier that will own the device
        #[arg(long = "client-id")]
        client_id: String,
        /// Human readable name
        #[arg(long)]
        name: String,
        /// Device protocol (e.g. "selflearning" or "zwave")
        #[arg(long)]
        protocol: String,
        /// Device model identifier
        #[arg(long)]
        model: String,
        /// Optional TellStick parameter values in key=value form
        #[arg(long = "parameter", value_parser = parse_key_value)]
        parameters: Vec<KeyValue>,
        /// Immediately trigger Learn mode after creating the device
        #[arg(long)]
        learn: bool,
    },
    /// Remove a device from Telldus Live
    Remove {
        #[arg(long = "id")]
        device_id: String,
    },
>>>>>>> 2252d5c (Support Telldus device registration and removal)
    /// Turn on a device
    On {
        #[arg(long = "id")]
        device_id: String,
    },
    /// Turn off a device
    Off {
        #[arg(long = "id")]
        device_id: String,
    },
    /// Dim a device to a level (0-255)
    Dim {
        #[arg(long = "id")]
        device_id: String,
        #[arg(long, value_parser = clap::value_parser!(u8).range(0..=255))]
        level: u8,
    },
    /// Trigger a doorbell action
    Bell {
        #[arg(long = "id")]
        device_id: String,
    },
    /// Execute a Telldus command number
    Execute {
        #[arg(long = "id")]
        device_id: String,
        #[arg(long)]
        command: i32,
    },
    /// Start an upwards movement
    Up {
        #[arg(long = "id")]
        device_id: String,
    },
    /// Stop movement
    Stop {
        #[arg(long = "id")]
        device_id: String,
    },
    /// Start a downwards movement
    Down {
        #[arg(long = "id")]
        device_id: String,
    },
    /// Put device into learning mode
    Learn {
        #[arg(long = "id")]
        device_id: String,
    },
    /// Inspect device details
    Info {
        #[arg(long = "id")]
        device_id: String,
    },
    /// Show recent device history
    History {
        #[arg(long = "id")]
        device_id: String,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Persist a device parameter key/value
    SetParameter {
        #[arg(long = "id")]
        device_id: String,
        #[arg(long)]
        parameter: String,
        #[arg(long)]
        value: String,
    },
    /// Retrieve a device parameter value
    GetParameter {
        #[arg(long = "id")]
        device_id: String,
        #[arg(long)]
        parameter: String,
    },
<<<<<<< HEAD
=======
}

#[derive(Subcommand)]
enum SensorCommand {
    /// Show sensor metadata
    Info {
        #[arg(long = "id")]
        sensor_id: String,
        /// Optional scale (e.g. 0 for temperature, 1 for humidity)
        #[arg(long)]
        scale: Option<i32>,
    },
    /// Show historic sensor readings
    History {
        #[arg(long = "id")]
        sensor_id: String,
        /// Telldus scale identifier
        #[arg(long)]
        scale: i32,
        /// Optional number of entries
        #[arg(long)]
        limit: Option<u32>,
    },
>>>>>>> 2252d5c (Support Telldus device registration and removal)
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum DeviceKind {
    All,
    Controllers,
    Devices,
    Sensors,
}

#[derive(Subcommand)]
enum SensorCommand {
    /// Show sensor metadata
    Info {
        #[arg(long = "id")]
        sensor_id: String,
        /// Optional scale (e.g. 0 for temperature, 1 for humidity)
        #[arg(long)]
        scale: Option<i32>,
    },
    /// Show historic sensor readings
    History {
        #[arg(long = "id")]
        sensor_id: String,
        /// Telldus scale identifier
        #[arg(long)]
        scale: i32,
        /// Optional number of entries
        #[arg(long)]
        limit: Option<u32>,
    },
}

#[derive(Debug, Error)]
enum AppError {
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Auth(#[from] auth::AuthError),
    #[error(transparent)]
    Api(#[from] api::ApiError),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Usage(String),
}

#[derive(Clone, Debug)]
struct KeyValue {
    key: String,
    value: String,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        eprintln!("Error: {err}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run(cli: Cli) -> Result<(), AppError> {
    match cli.command.unwrap_or(Commands::Auth {
        command: Some(AuthCommand::Validate),
    }) {
        Commands::Auth { command } => match command.unwrap_or(AuthCommand::Validate) {
            AuthCommand::Validate => handle_validate(),
        },
        Commands::Devices { command } => match command.unwrap_or(DeviceCommand::List {
            kind: DeviceKind::All,
        }) {
            DeviceCommand::List { kind } => handle_devices_list(kind),
            DeviceCommand::Edit {
                device_id,
                name,
                protocol,
                model,
            } => handle_devices_edit(&device_id, name, protocol, model),
<<<<<<< HEAD
=======
            DeviceCommand::Add {
                client_id,
                name,
                protocol,
                model,
                parameters,
                learn,
            } => handle_device_add(&client_id, &name, &protocol, &model, parameters, learn),
            DeviceCommand::Remove { device_id } => handle_device_remove(&device_id),
>>>>>>> 2252d5c (Support Telldus device registration and removal)
            DeviceCommand::On { device_id } => handle_device_simple(
                &device_id,
                |api, id| api.device_turn_on(id),
                || "Turned device on.".into(),
            ),
            DeviceCommand::Off { device_id } => handle_device_simple(
                &device_id,
                |api, id| api.device_turn_off(id),
                || "Turned device off.".into(),
            ),
            DeviceCommand::Dim { device_id, level } => handle_device_simple(
                &device_id,
                move |api, id| api.device_dim(id, level),
                move || format!("Dimmed device to level {level}."),
            ),
            DeviceCommand::Bell { device_id } => handle_device_simple(
                &device_id,
                |api, id| api.device_bell(id),
                || "Triggered bell.".into(),
            ),
            DeviceCommand::Execute { device_id, command } => handle_device_simple(
                &device_id,
                move |api, id| api.device_execute(id, command),
                move || format!("Executed command {command}."),
            ),
            DeviceCommand::Up { device_id } => handle_device_simple(
                &device_id,
                |api, id| api.device_up(id),
                || "Sent up command.".into(),
            ),
            DeviceCommand::Stop { device_id } => handle_device_simple(
                &device_id,
                |api, id| api.device_stop(id),
                || "Sent stop command.".into(),
            ),
            DeviceCommand::Down { device_id } => handle_device_simple(
                &device_id,
                |api, id| api.device_down(id),
                || "Sent down command.".into(),
            ),
            DeviceCommand::Learn { device_id } => handle_device_simple(
                &device_id,
                |api, id| api.device_learn(id),
                || "Device put into learn mode.".into(),
            ),
            DeviceCommand::Info { device_id } => handle_device_info(&device_id),
            DeviceCommand::History { device_id, limit } => handle_device_history(&device_id, limit),
            DeviceCommand::SetParameter {
                device_id,
                parameter,
                value,
            } => handle_device_set_parameter(&device_id, &parameter, &value),
            DeviceCommand::GetParameter {
                device_id,
                parameter,
            } => handle_device_get_parameter(&device_id, &parameter),
        },
        Commands::Sensors { command } => match command {
            Some(SensorCommand::Info { sensor_id, scale }) => handle_sensor_info(&sensor_id, scale),
            Some(SensorCommand::History {
                sensor_id,
                scale,
                limit,
            }) => handle_sensor_history(&sensor_id, scale, limit),
            None => Err(AppError::Usage(
                "Specify a sensors subcommand (info/history).".into(),
            )),
        },
    }
}

fn handle_validate() -> Result<(), AppError> {
    let mut credentials = ensure_credentials()?;
    let location = credentials_path()?;
    println!("Using credentials file at {}", location.to_string_lossy());

    let outcome = auth::validate(&mut credentials)?;
    if outcome.tokens_refreshed {
        save_credentials(&credentials)?;
        println!("Stored refreshed OAuth access token.");
    }

    if let Some(name) = outcome.account_name {
        println!("Authenticated as {name}.");
    } else {
        println!("Credentials verified with Telldus Live.");
    }
    Ok(())
}

fn handle_devices_list(kind: DeviceKind) -> Result<(), AppError> {
    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);
    let mut entries = match kind {
        DeviceKind::All => {
            let mut combined = Vec::new();
            combined.extend(api.list_controllers()?);
            combined.extend(api.list_devices()?);
            combined.extend(api.list_sensors()?);
            combined
        }
        DeviceKind::Controllers => api.list_controllers()?,
        DeviceKind::Devices => api.list_devices()?,
        DeviceKind::Sensors => api.list_sensors()?,
    };

    if entries.is_empty() {
        println!("No resources returned for the selected filter.");
        return Ok(());
    }

    entries.sort_by(|a, b| {
        a.category
            .as_str()
            .cmp(b.category.as_str())
            .then(a.name.cmp(&b.name))
            .then(a.id.cmp(&b.id))
    });

    println!();
    println!("{:<12} {:<12} {:<32} {}", "TYPE", "ID", "NAME", "DETAILS");
    for entry in entries {
        let details = entry.details.unwrap_or_else(|| "-".into());
        println!(
            "{:<12} {:<12} {:<32} {}",
            entry.category.as_str(),
            entry.id,
            entry.name,
            details
        );
    }

    Ok(())
}

fn handle_devices_edit(
    device_id: &str,
    name: Option<String>,
    protocol: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    if name.is_none() && protocol.is_none() && model.is_none() {
        return Err(AppError::Usage(
            "Nothing to update; supply at least one of --name, --protocol, or --model.".into(),
        ));
    }

    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);

    if let Some(ref new_name) = name {
        api.set_device_name(device_id, new_name)?;
        println!("Updated device {device_id} name to '{new_name}'.");
    }
    if let Some(ref protocol) = protocol {
        api.set_device_protocol(device_id, protocol)?;
        println!("Updated device {device_id} protocol to '{protocol}'.");
    }
    if let Some(ref model) = model {
        api.set_device_model(device_id, model)?;
        println!("Updated device {device_id} model to '{model}'.");
    }

    println!("Device update complete.");
    Ok(())
}

<<<<<<< HEAD
fn handle_device_simple<F, S>(device_id: &str, action: F, message: S) -> Result<(), AppError>
where
    F: FnOnce(&TelldusApi, &str) -> Result<(), api::ApiError>,
    S: FnOnce() -> String,
=======
fn handle_device_add(
    client_id: &str,
    name: &str,
    protocol: &str,
    model: &str,
    parameters: Vec<KeyValue>,
    learn: bool,
) -> Result<(), AppError> {
    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);

    let new_id = api.add_device(AddDeviceRequest {
        client_id,
        name,
        protocol,
        model,
    })?;
    println!("Created device {new_id} on client {client_id}.");

    for kv in parameters {
        api.set_device_parameter(&new_id, &kv.key, &kv.value)?;
        println!(
            "Set parameter '{key}' = '{value}'",
            key = kv.key,
            value = kv.value
        );
    }

    if learn {
        println!(
            "Triggering learn mode for device {new_id}. Activate the remote within the Telldus timeout window."
        );
        api.device_learn(&new_id)?;
    }

    Ok(())
}

fn handle_device_remove(device_id: &str) -> Result<(), AppError> {
    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);
    api.remove_device(device_id)?;
    println!("Removed device {device_id}.");
    Ok(())
}

fn handle_device_simple<F, M>(device_id: &str, action: F, message: M) -> Result<(), AppError>
where
    F: FnOnce(&TelldusApi, &str) -> Result<(), api::ApiError>,
    M: FnOnce() -> String,
>>>>>>> 2252d5c (Support Telldus device registration and removal)
{
    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);
    action(&api, device_id)?;
<<<<<<< HEAD
    println!("{}", message());
=======
    let text = message();
    println!("{text}");
>>>>>>> 2252d5c (Support Telldus device registration and removal)
    Ok(())
}

fn handle_device_info(device_id: &str) -> Result<(), AppError> {
    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);
    let info = api.device_info(device_id)?;
    print_json(&info);
    Ok(())
}

fn handle_device_history(device_id: &str, limit: Option<u32>) -> Result<(), AppError> {
    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);
    let entries = api.device_history(device_id, limit)?;
    if entries.is_empty() {
        println!("No history entries found.");
    } else {
        for (idx, entry) in entries.iter().enumerate() {
            println!("-- Event {} --", idx + 1);
            print_json(entry);
        }
    }
    Ok(())
}

fn handle_device_set_parameter(
    device_id: &str,
    parameter: &str,
    value: &str,
) -> Result<(), AppError> {
    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);
    api.set_device_parameter(device_id, parameter, value)?;
    println!("Set parameter '{parameter}' for device {device_id} to '{value}'.");
    Ok(())
}

fn handle_device_get_parameter(device_id: &str, parameter: &str) -> Result<(), AppError> {
    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);
    match api.get_device_parameter(device_id, parameter)? {
        Some(value) => println!("Parameter '{parameter}' = '{value}'"),
        None => println!("Parameter '{parameter}' not set for device {device_id}."),
    }
    Ok(())
}

fn handle_sensor_info(sensor_id: &str, scale: Option<i32>) -> Result<(), AppError> {
    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);
    let info = api.sensor_info(sensor_id, scale)?;
    print_json(&info);
    Ok(())
}

fn handle_sensor_history(sensor_id: &str, scale: i32, limit: Option<u32>) -> Result<(), AppError> {
    let session = authenticate()?;
    let api = TelldusApi::new(&session.client, &session.credentials);
    let entries = api.sensor_history(sensor_id, scale, limit)?;
    if entries.is_empty() {
        println!("No sensor history entries found.");
    } else {
        for (idx, entry) in entries.iter().enumerate() {
            println!("-- Reading {} --", idx + 1);
            print_json(entry);
        }
    }
    Ok(())
}

struct Session {
    client: reqwest::blocking::Client,
    credentials: TelldusCredentials,
}

fn authenticate() -> Result<Session, AppError> {
    let mut credentials = ensure_credentials()?;
    let location = credentials_path()?;
    println!("Using credentials file at {}", location.to_string_lossy());

    let client = build_http_client()?;
    let outcome = auth::validate_with_client(&client, &mut credentials)?;
    if outcome.tokens_refreshed {
        save_credentials(&credentials)?;
        println!("Stored refreshed OAuth access token.");
    }
    if let Some(name) = outcome.account_name {
        println!("Authenticated as {name}.");
    }

    Ok(Session {
        client,
        credentials,
    })
}

fn print_json(value: &serde_json::Value) {
    match to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        Err(_) => println!("{value}"),
    }
}
<<<<<<< HEAD
=======

fn parse_key_value(arg: &str) -> Result<KeyValue, String> {
    let mut parts = arg.splitn(2, '=');
    let key = parts
        .next()
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .ok_or_else(|| "parameter must be in key=value format".to_string())?;
    let value = parts
        .next()
        .map(|v| v.trim())
        .ok_or_else(|| "parameter must be in key=value format".to_string())?;
    Ok(KeyValue {
        key: key.to_string(),
        value: value.to_string(),
    })
}
>>>>>>> 2252d5c (Support Telldus device registration and removal)

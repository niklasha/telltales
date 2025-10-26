mod api;
mod auth;
mod config;
mod http_client;

use api::TelldusApi;
use clap::{Parser, Subcommand, ValueEnum};
use config::{credentials_path, ensure_credentials, save_credentials};
use http_client::build_http_client;
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
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum DeviceKind {
    All,
    Controllers,
    Devices,
    Sensors,
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

    let api = TelldusApi::new(&client, &credentials);
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

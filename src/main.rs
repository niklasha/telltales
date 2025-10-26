mod config;

use clap::{Parser, Subcommand};
use config::{credentials_path, ensure_credentials};
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
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Ensure credentials are present and valid locally
    Validate,
}

#[derive(Debug, Error)]
enum AppError {
    #[error(transparent)]
    Config(#[from] config::ConfigError),
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
    }
}

fn handle_validate() -> Result<(), AppError> {
    let credentials = ensure_credentials()?;
    let location = credentials_path()?;
    println!(
        "Found Telldus Live credentials at {}",
        location.to_string_lossy()
    );
    if credentials.is_complete() {
        println!("Credentials are present and ready for API requests.");
    }
    Ok(())
}

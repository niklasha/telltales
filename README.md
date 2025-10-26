# Telltales CLI

Telldus Live command-line interface scaffold written in Rust.

## Authentication

Run `cargo run -- auth validate` to ensure local credentials are present. The command looks for YAML credentials at `~/.config/telltales/credentials.yaml`. If the public or private key fields are missing you’ll be prompted to supply them; OAuth access tokens remain optional until an OAuth flow is implemented.

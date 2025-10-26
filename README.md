# Telltales CLI

Telldus Live command-line interface scaffold written in Rust.

## Authentication

Run `cargo run -- auth validate` to ensure local credentials are present. The command will look for YAML credentials at `~/.config/telltales/credentials.yaml`. If any fields are missing it will prompt for the Telldus Live public key, private key, access token, and access token secret, then persist them back to the same file.

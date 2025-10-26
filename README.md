# Telltales CLI

Telldus Live command-line interface scaffold written in Rust.

## Authentication

Run `cargo run -- auth validate` to ensure local credentials are present and usable. The command looks for YAML credentials at `~/.config/telltales/credentials.yaml`. If the Telldus Live public or private key fields are missing you’ll be prompted to supply them.

When no OAuth access token is stored—or when the stored token is rejected—the CLI initiates an OAuth 1.0a flow using `oauth_callback=oob`, prints an authorization URL, and asks for either the verification code shown by Telldus Live or the full redirect URL you land on after pressing **Confirm**. The CLI extracts the `oauth_verifier` value, exchanges it for an access token and secret, persists those values, and immediately validates them against the `user/profile` endpoint.

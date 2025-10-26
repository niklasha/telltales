# Telltales CLI

Telldus Live command-line interface scaffold written in Rust.

## Authentication

Run `cargo run -- auth validate` to ensure local credentials are present and usable. The command looks for YAML credentials at `~/.config/telltales/credentials.yaml`. If the Telldus Live public or private key fields are missing you’ll be prompted to supply them.

When no OAuth access token is stored—or when the stored token is rejected—the CLI spins up a temporary HTTP listener on `http://127.0.0.1:<port>/telltales/callback`, prints an authorization URL, and waits for the browser to redirect back. If the redirect reaches the local listener the CLI captures the `oauth_verifier` automatically. Otherwise, copy the final redirect URL (or the code shown) and paste it back into the CLI prompt. On success the access token and secret are persisted and verified against the `user/profile` endpoint.

# Telltales CLI

Telldus Live command-line interface scaffold written in Rust.

## Authentication

Run `cargo run -- auth validate` to ensure local credentials are present and usable. The command looks for YAML credentials at `~/.config/telltales/credentials.yaml`. If the Telldus Live public or private key fields are missing you’ll be prompted to supply them.

When no OAuth access token is stored—or when the stored token is rejected—the CLI spins up a temporary HTTP listener on `http://127.0.0.1:<port>/telltales/callback`, prints an authorization URL, and waits for the browser to redirect back. If the redirect reaches the local listener the CLI captures the `oauth_verifier` automatically. Otherwise, copy the final redirect URL (or the code shown) and paste it back into the CLI prompt. On success the access token and secret are persisted and verified against the `user/profile` endpoint.

## Listing devices

List all discovered resources (controllers, devices, sensors) with:

```
cargo run -- devices list
```

Filter to a specific category with the `--kind` flag:

```
# only controllers
cargo run -- devices list --kind controllers

# only sensors
cargo run -- devices list --kind sensors
```

Each row shows the resource type, numeric identifier, display name, and a short summary of known attributes.

## Editing devices

Rename or adjust metadata for a device:

```
cargo run -- devices edit --id 6942590 --name "Kitchen Counter"
```

Provide additional fields to update the protocol or model in the same call:

```
cargo run -- devices edit --id 6942590 --protocol zwave --model switches
```

At least one of `--name`, `--protocol`, or `--model` must be supplied. The command calls the Telldus Live `device/setName`, `device/setProtocol`, and `device/setModel` endpoints under the hood to persist your changes.

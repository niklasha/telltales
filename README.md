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

At least one of `--name`, `--protocol`, or `--model` must be supplied. The command calls the Telldus Live `device/setName`, `device/setProtocol`, and `device/setModel` endpoints to persist your changes.

## Controlling devices

Invoke Telldus Live actions directly from the CLI:

```
# Turn switches on/off
cargo run -- devices on --id 6942590
cargo run -- devices off --id 6942590

# Dim to a level (0-255)
cargo run -- devices dim --id 6942590 --level 128

# Trigger bell/scene/relay actions
cargo run -- devices bell --id 6942590
cargo run -- devices execute --id 6942590 --command 15

# Motorised shades or relays with directional controls
cargo run -- devices up --id 6942590
cargo run -- devices stop --id 6942590
cargo run -- devices down --id 6942590

# Manage TellStick parameters
cargo run -- devices set-parameter --id 6942590 --parameter house --value A
cargo run -- devices get-parameter --id 6942590 --parameter house

# Device learn mode
cargo run -- devices learn --id 6942590
```

Introspection helpers:

```
cargo run -- devices info --id 6942590
cargo run -- devices history --id 6942590 --limit 10
```

## Inspecting sensors

Fetch sensor metadata and historic values (scales follow Telldus Live conventions, for example `0` for temperature and `1` for humidity on combined sensors):

```
cargo run -- sensors info --id 1534643827 --scale 0
cargo run -- sensors history --id 1534643827 --scale 0 --limit 20
```

All network interactions reuse the shared OAuth session and respect a one-second rate limit window to comply with Telldus Live throttling.

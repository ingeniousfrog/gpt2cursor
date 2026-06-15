# gpt2cursor

`gpt2cursor` is a local macOS menu-bar Codex-to-Cursor bridge. It starts an
OpenAI-compatible endpoint on your machine so Cursor can call your locally
logged-in Codex CLI through a custom Base URL.

This project is for personal local development experiments only. It is not a
cloud relay, not an account-sharing service, and not a replacement for the
official OpenAI API.

## What It Does

- Runs as a Tauri menu-bar app with a compact local control panel.
- Exposes `GET /v1/models`, `GET /healthz`, and `POST /v1/chat/completions`.
- Converts Cursor chat-completion requests into `codex exec --json` calls.
- Uses your existing local Codex CLI login.
- Protects the local service with a user-set or randomly generated bearer key.
- Shows the Base URL, port status, bridge status, and per-session Codex usage.
- Supports launch-at-login on macOS.

## Local API Key

The API key shown in the app protects this local service only. Cursor sends it
as `Authorization: Bearer <key>`, and `gpt2cursor` checks it before a request can
reach your local Codex CLI.

It is not an OpenAI API key, is not sent to OpenAI by this project, and should
not be described or treated as an official OpenAI credential.

## Cursor Setup

Start the bridge from the menu-bar app, then add an OpenAI-compatible provider
in Cursor:

- Base URL: `http://127.0.0.1:8787/v1` by default, or the Base URL shown in the app
- API Key: the key shown in the app
- Model: `codex-local` by default

The app always binds the bridge to `127.0.0.1`; it does not expose the service
on your LAN or the public internet.

## App Controls

- **Port**: defaults to `8787`; the app checks whether the port is available
  before saving and starting.
- **API Key**: paste your own local key or generate a random `g2c_...` key.
- **Start / Stop**: starts or stops the native Rust HTTP bridge.
- **Usage**: shows request count, active requests, latest duration, and
  cumulative tokens for this app session.
- **Codex Account**: best-effort local CLI status; account quota is shown as
  unavailable when the CLI does not expose a stable quota API.
- **Launch at login**: writes/removes a macOS LaunchAgent for the app.

## Development

Requirements:

- Node.js 20 or newer.
- Rust 1.78 or newer.
- Codex CLI installed and logged in on the same machine.

Useful commands:

```sh
npm install
npm run build
npm run tauri
cargo test --manifest-path src-tauri/Cargo.toml
```

The Rust tests bind temporary localhost ports for integration coverage.

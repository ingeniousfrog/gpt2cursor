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
- Model: click **+ Add Custom Model** in Cursor Settings → Models and add
  `gpt2cursor-local`

Cursor does not automatically fetch models from a custom Base URL. You must add
the model name manually.

The app always binds the bridge to `127.0.0.1` by default. For Cursor Agent mode,
enable **Public Tunnel** in the app and provide your own ngrok authtoken so the
panel can start a public HTTPS URL for Cursor.

## ngrok Tunnel (Cursor Agent)

Cursor Agent routes requests through Cursor's cloud, which cannot reach
`127.0.0.1`. To use gpt2cursor with Agent mode:

1. Install [ngrok](https://ngrok.com/download) on the same machine.
2. In gpt2cursor, enable **Public Tunnel**. If you already ran `ngrok config add-authtoken`, the app reuses that login automatically.
3. Click **Start**. The app starts the local bridge and an ngrok tunnel.
4. Copy the **public** Base URL shown in the panel into Cursor Settings.
5. Paste the gpt2cursor API key and add custom model `gpt2cursor-local`.

Notes:

- Each user needs their own ngrok account and authtoken.
- Free ngrok URLs may change when the tunnel restarts.
- The public endpoint is protected by your gpt2cursor API key, but exposing a
  local Codex bridge to the internet still carries risk. Use only for personal
  experiments.

## App Controls

- **Port**: defaults to `8787`; the app checks whether the port is available
  before saving and starting.
- **API Key**: paste your own local key or generate a random `g2c_...` key.
- **Start / Stop**: starts or stops the native Rust HTTP bridge.
- **Usage**: shows request count, active requests, latest duration, and
  cumulative tokens for this app session.
- **Codex Account**: best-effort local CLI status; account quota is shown as
  unavailable when the CLI does not expose a stable quota API.
- **Public Tunnel**: optional ngrok integration for Cursor Agent; reuses your local ngrok login or accepts an override token,
  start the bridge, and copy the public Base URL.
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

## macOS Install (Release DMG)

Release builds are adhoc-signed for local use. After downloading from GitHub:

1. Open the DMG and drag **gpt2cursor** onto the **Applications** folder shortcut.
2. Launch **gpt2cursor** from `/Applications`.
3. If macOS says the app cannot be opened because the developer cannot be verified,
   right-click **gpt2cursor** in Applications and choose **Open**, then confirm
   **Open** again. You can also go to **System Settings → Privacy & Security**
   and click **Open Anyway**.

If macOS still blocks the app, or shows **“gpt2cursor is damaged and can't be
opened”**, remove the quarantine attribute in Terminal:

```sh
xattr -cr /Applications/gpt2cursor.app
```

Then open **gpt2cursor** again from Applications.

This is expected for local adhoc-signed builds that are not notarized by Apple.
Builds from `npm run tauri:build` still run an extra signing step so the DMG is
not rejected for a broken resource seal.

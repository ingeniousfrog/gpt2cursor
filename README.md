<p align="center">
  <img src="src-tauri/icons/icon.png" alt="gpt2cursor logo" width="112" height="112">
</p>

<h1 align="center">gpt2cursor</h1>

<p align="center">
  A native local bridge that lets Cursor talk to your locally logged-in Codex CLI through an OpenAI-compatible endpoint.
</p>

<p align="center">
  <a href="README-CN.md">简体中文</a>
  ·
  <a href="docs/HOW_TO_USE.md">How to Use</a>
  ·
  <a href="https://github.com/ingeniousfrog/gpt2cursor/releases">Releases</a>
</p>

<p align="center">
  <a href="https://github.com/ingeniousfrog/gpt2cursor/releases"><img alt="release" src="https://img.shields.io/badge/release-v0.4.3-orange?style=flat-square"></a>
  <a href="LICENSE"><img alt="license" src="https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square"></a>
</p>

<p align="center">
  Last updated: 2026-06-17
</p>

## Quick Start

1. Download the latest [Release](https://github.com/ingeniousfrog/gpt2cursor/releases).
2. Install and open **gpt2cursor**, then click **Start**.
3. Enable **Public Tunnel** (ngrok) and copy the public HTTPS Base URL.
4. In Cursor, add model `gpt2cursor-local` with that Base URL and the API key from the panel.
5. Use **Ask** or **Agent** — both need the public ngrok URL (Cursor cannot reach `127.0.0.1`).

Full walkthrough: [docs/HOW_TO_USE.md](docs/HOW_TO_USE.md)

## Downloads

| Platform | Artifact | Status |
| --- | --- | --- |
| macOS (Apple Silicon) | `gpt2cursor_0.4.3_aarch64.dmg` | Stable |
| Windows (x64) | `gpt2cursor_0.4.3_x64-setup.exe` | Experimental |

Runtime requirements: **Codex CLI logged in on the same machine**. Node.js is **not** required for end users.

## Why It Exists

Cursor supports OpenAI-compatible providers. Codex CLI already uses your local login.
`gpt2cursor` sits between them: Cursor sends chat-completion requests to a local
endpoint, and the app turns them into `codex exec --json` over PTY with streaming
SSE back to Cursor.

No cloud relay. No account-sharing service. Not a replacement for the official OpenAI API.

## Highlights

- Native Tauri app (macOS menu-bar + Windows tray).
- **Single-instance** — no duplicate tray icons or port conflicts on launch.
- OpenAI-compatible endpoints: `GET /v1/models`, `GET /healthz`, `POST /v1/chat/completions`.
- PTY streaming bridge for Cursor Ask / Agent compatibility.
- **Live Codex progress** — reasoning, tool/file activity in the panel and streamed to Cursor.
- **Dev mode** — raw request / JSONL / result logs in the activity popover.
- **Dark & light themes** — switch from the footer theme toggle.
- Local bearer key (`g2c_...`) protecting the bridge.
- Activity panel with live request logs.
- ngrok public tunnel so Cursor cloud can reach your local bridge (Ask & Agent).
- macOS **Launch at login** (optional).
- Configurable Codex timeout and context trimming.

## Supported Cursor Modes

| Cursor mode | Status | Base URL |
| --- | --- | --- |
| Ask | Supported | Public ngrok HTTPS URL |
| Agent | Supported | Public ngrok HTTPS URL |
| Other modes | Planned | Not supported yet |

<p align="center">
  <img src="docs/images/cursor-ask-mode.png" alt="Ask mode" width="720">
</p>

<p align="center">
  <img src="docs/images/cursor-agent-mode.jpg" alt="Agent mode" width="720">
</p>

## Architecture

```mermaid
flowchart LR
  Cursor["Cursor Cloud"]
  Ngrok["ngrok HTTPS tunnel"]
  LocalEndpoint["gpt2cursor"]
  CodexCLI["Codex CLI"]
  CodexSession["Local Codex session"]

  Cursor -->|"OpenAI-compatible SSE"| Ngrok
  Ngrok --> LocalEndpoint
  LocalEndpoint -->|"codex exec --json via PTY"| CodexCLI
  CodexCLI --> CodexSession
  CodexSession --> CodexCLI
  CodexCLI --> LocalEndpoint
  LocalEndpoint --> Ngrok
  Ngrok --> Cursor
```

## Cursor Setup (Summary)

| Setting | Value |
| --- | --- |
| Base URL | Public HTTPS URL from gpt2cursor panel (ngrok) |
| API Key | Local key from gpt2cursor panel |
| Model | `gpt2cursor-local` (add manually in Cursor Settings → Models) |

Details: [docs/HOW_TO_USE.md](docs/HOW_TO_USE.md)

## Install (Summary)

### macOS

1. Open DMG → drag to **Applications** → launch.
2. If blocked: right-click **Open**, or run `xattr -cr /Applications/gpt2cursor.app`.

### Windows

1. Run `gpt2cursor_0.4.3_x64-setup.exe`.
2. If SmartScreen warns: **More info → Run anyway**.

Install and troubleshooting details: [docs/HOW_TO_USE.md](docs/HOW_TO_USE.md)

## Documentation

| Doc | Description |
| --- | --- |
| [HOW_TO_USE.md](docs/HOW_TO_USE.md) | Step-by-step setup for Ask / Agent, ngrok, troubleshooting |
| [HOW_TO_USE_CN.md](docs/HOW_TO_USE_CN.md) | 中文使用指南 |
| [README-CN.md](README-CN.md) | 中文项目说明 |

## Development

```sh
npm install
npm run tauri          # dev
npm test               # Rust integration tests
npm run tauri:build    # macOS DMG + sign script
```

Requirements: Node.js 20+, Rust 1.78+, Codex CLI. Windows installers are built via
[`.github/workflows/build-windows.yml`](.github/workflows/build-windows.yml).

## Project Status

Personal local development experiments only. Current focus: Cursor Ask/Agent,
Codex CLI bridging, and cross-platform packaging.

# How to Use gpt2cursor

Step-by-step guide for installing gpt2cursor, connecting Cursor, and using it in
**Ask** or **Agent** mode.

> **Scope:** Only Cursor **Ask** and **Agent** are supported today. Other Cursor
> modes are not supported yet.

## Before You Start

### What you need

| Requirement | Required? | Notes |
| --- | --- | --- |
| [Codex CLI](https://github.com/openai/codex) installed and logged in | Yes | gpt2cursor reuses your local Codex session |
| [Cursor](https://cursor.com/) | Yes | Custom OpenAI-compatible provider |
| Node.js | No | Only needed if you build from source |
| ngrok | Yes (for Cursor) | Cursor routes custom-model requests through its cloud and cannot reach `127.0.0.1` on your machine. Enable **Public Tunnel** for both **Ask** and **Agent**. |

### What gpt2cursor provides

- A local OpenAI-compatible endpoint (`/v1/models`, `/v1/chat/completions`)
- A menu-bar / tray control panel
- **PTY streaming**: runs `codex exec --json` inside a pseudo-terminal (PTY), reads JSONL output line by line, and streams SSE chunks back to Cursor
- **Public Tunnel** via ngrok so Cursor can reach your local bridge
- **Launch at login** on macOS (optional; keeps the bridge available after reboot)
- Live Activity logs for bridge requests

---

## 1. Install the App

Download the latest release:
[github.com/ingeniousfrog/gpt2cursor/releases](https://github.com/ingeniousfrog/gpt2cursor/releases)

| Platform | File | Notes |
| --- | --- | --- |
| macOS (Apple Silicon) | `gpt2cursor_*_aarch64.dmg` | Drag to Applications |
| Windows (x64) | `gpt2cursor_*_x64-setup.exe` | NSIS installer |

### macOS install

1. Open the DMG.
2. Drag **gpt2cursor** onto **Applications**.
3. Launch from `/Applications`.

If macOS blocks the app:

- Right-click **gpt2cursor** → **Open**, or
- **System Settings → Privacy & Security → Open Anyway**, or
- Run in Terminal:

```sh
xattr -cr /Applications/gpt2cursor.app
```

### Windows install

1. Run `gpt2cursor_*_x64-setup.exe`.
2. Finish the installer.
3. Launch **gpt2cursor** from the Start menu or system tray.

If SmartScreen warns about an unknown publisher: **More info → Run anyway**.

> Windows support is experimental. Launch-at-login and some macOS-only UI polish
> are not available on Windows yet.

---

## 2. Configure gpt2cursor

1. Click the **gpt2cursor** tray / menu-bar icon.
2. Set or generate an **API Key** (`g2c_...`). This protects your local bridge only.
3. Confirm **Port** (default `8787`).
4. Click **Start**.
5. Copy the **Base URL** shown in the panel.

Recommended defaults:

- **Timeout**: 300s for longer sessions
- **Context msgs**: 12 (lower if Cursor sends huge histories)
- **Launch at login** (macOS): optional; auto-starts gpt2cursor after sign-in

---

## 3. Enable Public Tunnel (Required for Cursor)

Cursor does not call `127.0.0.1` on your machine directly when you use a custom
OpenAI-compatible provider. **Both Ask and Agent** need a public HTTPS Base URL.

1. Install [ngrok](https://ngrok.com/download) on the same machine.
2. In gpt2cursor, enable **Public Tunnel**.
3. Paste your ngrok authtoken if needed (or reuse `ngrok config add-authtoken`).
4. Click **Start** and wait for the **public HTTPS Base URL** in the panel.
5. Copy that URL into Cursor Settings (not `http://127.0.0.1:8787/v1`).

The bridge still runs locally; ngrok only exposes it to Cursor's cloud.

---

## 4. Add the Model in Cursor

Cursor does **not** auto-discover models from custom Base URLs.

1. Open **Cursor Settings → Models**.
2. Click **+ Add Custom Model**.
3. Enter: `gpt2cursor-local`
4. Add an OpenAI-compatible provider (or edit your existing one):
   - **Base URL**: the **public HTTPS** URL from gpt2cursor (with `/v1` suffix)
   - **API Key**: from the gpt2cursor panel
   - **Model**: `gpt2cursor-local`

---

## 5. Use Ask Mode

1. In Cursor chat, switch mode to **Ask**.
2. Select model **`gpt2cursor-local`**.
3. Send a short message (for example: `What model are you?`).

Use the same **public ngrok Base URL** as in Cursor Settings.

![Ask mode example](images/cursor-ask-mode.png)

---

## 6. Use Agent Mode

1. In Cursor chat, switch mode to **Agent**.
2. Select model **`gpt2cursor-local`**.
3. Send a task (for example: explain a file or run a small change).

Agent mode also uses the **public ngrok Base URL**. The main difference is payload
size: Agent sends much larger chat history, so keep **Context msgs** reasonable.

![Agent mode example](images/cursor-agent-mode.jpg)

### Connection checklist (Ask & Agent)

- [ ] Bridge is **Running** in gpt2cursor
- [ ] **Public Tunnel** is enabled and shows an **https://** Base URL
- [ ] Cursor Settings use the **public** Base URL (not `127.0.0.1`)
- [ ] Model is `gpt2cursor-local`
- [ ] Codex CLI is logged in locally

---

## 7. Verify It Works

### In gpt2cursor Activity

Check the gpt2cursor **Activity** panel. Healthy logs look like:

```text
codex exec via pty, ...
stream ok ...
```

| Log | Meaning |
| --- | --- |
| `trimmed history N -> M messages` | Large Cursor history was trimmed for Codex |
| `codex exec via pty` | Codex CLI started in PTY; JSONL is streaming |
| `stream ok` | Response streamed successfully |
| `client disconnected` | Cursor closed the connection early (often timeout) |

---

## 8. Troubleshooting

### macOS: "gpt2cursor is damaged and can't be opened"

```sh
xattr -cr /Applications/gpt2cursor.app
```

Then open again from Applications.

### Cursor shows "Unable to read body" or disconnects

- Increase **Timeout** in gpt2cursor (try 300s).
- Lower **Context msgs** (try 8–12).
- Prefer **Ask** for quick Q&A; Agent sends much larger payloads.

### Cursor cannot connect (Ask or Agent)

- Confirm **Public Tunnel** is on and the panel shows a public **https://** URL.
- Update Cursor Base URL to that public URL (not `127.0.0.1`).
- Restart bridge + tunnel after ngrok URL changes.

### "User API Key Rate limit exceeded"

This usually comes from Cursor retrying, not from Codex itself. Wait, reduce
parallel requests, or switch back to Ask mode to test.

### Codex CLI not found

Install and log in to Codex CLI on the same machine:

```sh
codex login
```

Then click **Refresh** in the gpt2cursor panel.

---

## 9. FAQ

**Do I need Node.js?**  
No. Release builds bundle the UI. Node is only for development.

**Is my OpenAI API key used?**  
No. The key in gpt2cursor is a local bearer token for the bridge.

**Can I use Edit / Composer / other Cursor modes?**  
Not yet. Only **Ask** and **Agent** are supported.

**Why PTY?**  
Codex CLI hangs or stalls when run with plain pipes. gpt2cursor runs it in a PTY
and streams JSONL events to Cursor as SSE deltas.

**Do Ask and Agent both need ngrok?**  
Yes, when using Cursor. Custom Base URL requests are routed through Cursor's
cloud, which cannot reach your local `127.0.0.1`.

**Is launch at login supported?**  
Yes on macOS. Toggle **Launch at login** in the gpt2cursor panel.

**Is the public tunnel safe?**  
It is protected by your gpt2cursor API key, but exposing a local Codex bridge to
the internet has risk. Use for personal experiments only.

---

[← Back to README](../README.md)

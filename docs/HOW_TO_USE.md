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
| ngrok | Agent mode only | Cursor cloud cannot reach `127.0.0.1` |

### What gpt2cursor provides

- A local OpenAI-compatible endpoint (`/v1/models`, `/v1/chat/completions`)
- A menu-bar / tray control panel
- Optional ngrok public tunnel for Cursor Agent mode
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

- **Timeout**: 300s for longer Agent sessions
- **Context msgs**: 12 (lower if Agent sends huge histories)

---

## 3. Add the Model in Cursor

Cursor does **not** auto-discover models from custom Base URLs.

1. Open **Cursor Settings → Models**.
2. Click **+ Add Custom Model**.
3. Enter: `gpt2cursor-local`
4. Add an OpenAI-compatible provider (or edit your existing one):
   - **Base URL**: from the gpt2cursor panel
   - **API Key**: from the gpt2cursor panel
   - **Model**: `gpt2cursor-local`

---

## 4. Use Ask Mode

Ask mode talks to your local Base URL directly.

1. In Cursor chat, switch mode to **Ask**.
2. Select model **`gpt2cursor-local`**.
3. Send a short message (for example: `What model are you?`).

Expected Base URL:

```text
http://127.0.0.1:8787/v1
```

You do **not** need ngrok for Ask mode.

![Ask mode example](images/cursor-ask-mode.png)

Check the gpt2cursor **Activity** panel. You should see logs like:

```text
codex exec via pty, ...
stream ok ...
```

---

## 5. Use Agent Mode

Agent mode routes requests through Cursor's cloud, so it cannot reach
`127.0.0.1` on your machine.

### Enable the public tunnel

1. Install [ngrok](https://ngrok.com/download) on the same machine.
2. In gpt2cursor, enable **Public Tunnel**.
3. Paste your ngrok authtoken if needed (or reuse `ngrok config add-authtoken`).
4. Click **Start** and wait for the **public HTTPS Base URL**.
5. Put that public URL into Cursor Settings (not `127.0.0.1`).
6. Keep the same gpt2cursor API key and model `gpt2cursor-local`.

![Agent mode example](images/cursor-agent-mode.jpg)

### Agent mode checklist

- [ ] Bridge is **Running** in gpt2cursor
- [ ] Public tunnel shows an **https://** Base URL
- [ ] Cursor uses the **public** Base URL
- [ ] Model is `gpt2cursor-local`
- [ ] Codex CLI is logged in locally

---

## 6. Verify It Works

### In Cursor

- Ask mode: reply streams within ~15–60s for normal questions
- Agent mode: same, after tunnel is active

### In gpt2cursor Activity

| Log | Meaning |
| --- | --- |
| `trimmed history N -> M messages` | Large Cursor history was trimmed for Codex |
| `codex exec via pty` | Codex CLI started correctly |
| `stream ok` | Response streamed successfully |
| `client disconnected` | Cursor closed the connection early (often timeout) |

---

## 7. Troubleshooting

### macOS: "gpt2cursor is damaged and can't be opened"

```sh
xattr -cr /Applications/gpt2cursor.app
```

Then open again from Applications.

### Cursor shows "Unable to read body" or disconnects

- Increase **Timeout** in gpt2cursor (try 300s).
- Lower **Context msgs** (try 8–12).
- Prefer **Ask** for quick Q&A; Agent sends much larger payloads.

### Agent mode cannot connect

- Confirm ngrok is running and the panel shows a public URL.
- Update Cursor Base URL to the **https** tunnel URL.
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

## 8. FAQ

**Do I need Node.js?**  
No. Release builds bundle the UI. Node is only for development.

**Is my OpenAI API key used?**  
No. The key in gpt2cursor is a local bearer token for the bridge.

**Can I use Edit / Composer / other Cursor modes?**  
Not yet. Only **Ask** and **Agent** are supported.

**Is the public tunnel safe?**  
It is protected by your gpt2cursor API key, but exposing a local Codex bridge to
the internet has risk. Use for personal experiments only.

---

[← Back to README](../README.md)

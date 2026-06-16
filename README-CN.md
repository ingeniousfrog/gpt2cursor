<p align="center">
  <img src="src-tauri/icons/icon.png" alt="gpt2cursor logo" width="112" height="112">
</p>

<h1 align="center">gpt2cursor</h1>

<p align="center">
  原生本地桥接工具，让 Cursor 通过 OpenAI-compatible endpoint 调用你本机已登录的 Codex CLI。
</p>

<p align="center">
  <a href="README.md">English</a>
  ·
  <a href="docs/HOW_TO_USE_CN.md">使用指南</a>
  ·
  <a href="https://github.com/ingeniousfrog/gpt2cursor/releases">Releases</a>
</p>

<p align="center">
  <a href="https://github.com/ingeniousfrog/gpt2cursor/releases"><img alt="release" src="https://img.shields.io/badge/release-v0.4.2-orange?style=flat-square"></a>
  <a href="LICENSE"><img alt="license" src="https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square"></a>
</p>

<p align="center">
  Last updated: 2026-06-16
</p>

## 快速开始

1. 下载最新 [Release](https://github.com/ingeniousfrog/gpt2cursor/releases)。
2. 安装并打开 **gpt2cursor**，点击 **Start**。
3. 开启 **Public Tunnel**（ngrok），复制公网 HTTPS Base URL。
4. 在 Cursor 中添加模型 `gpt2cursor-local`，填入该 Base URL 与面板中的 API Key。
5. 使用 **Ask** 或 **Agent** —— 两者都需要公网 ngrok 地址（Cursor 无法访问 `127.0.0.1`）。

完整步骤见 [docs/HOW_TO_USE_CN.md](docs/HOW_TO_USE_CN.md)

## 下载

| 平台 | 安装包 | 状态 |
| --- | --- | --- |
| macOS（Apple Silicon） | `gpt2cursor_0.4.2_aarch64.dmg` | 稳定 |
| Windows（x64） | `gpt2cursor_0.4.2_x64-setup.exe` | 实验性 |
| Linux（x64） | `gpt2cursor_0.4.2_amd64.AppImage` | 实验性 |

运行依赖：同机 **Codex CLI 已登录**。终端用户**不需要** Node.js。

## 它解决什么问题

Cursor 支持 OpenAI-compatible provider，Codex CLI 则使用你本机的登录态。
`gpt2cursor` 在中间做本地桥接：Cursor 把请求发到本地 endpoint，应用通过 PTY
调用 `codex exec --json`，再以 SSE 流式返回给 Cursor。

不是云端转发，不是账号共享，也不是官方 OpenAI API 的替代品。

## 亮点

- Tauri 原生应用（macOS 菜单栏 + Windows 托盘）。
- OpenAI-compatible 接口：`GET /v1/models`、`GET /healthz`、`POST /v1/chat/completions`。
- PTY 流式桥接，兼容 Cursor Ask / Agent。
- 本地 bearer key（`g2c_...`）保护 bridge。
- Activity 面板实时请求日志。
- ngrok 公网隧道，让 Cursor 云端访问本机 bridge（Ask 与 Agent 均需）。
- macOS **开机自登录**（可选）。
- 可配置 Codex 超时与上下文裁剪。

## 当前支持的 Cursor 模式

| Cursor 模式 | 状态 | Base URL |
| --- | --- | --- |
| Ask | 已支持 | ngrok 公网 HTTPS |
| Agent | 已支持 | ngrok 公网 HTTPS |
| 其他模式 | 待开发 | 暂不支持 |

<p align="center">
  <img src="docs/images/cursor-ask-mode.png" alt="Ask 模式" width="720">
</p>

<p align="center">
  <img src="docs/images/cursor-agent-mode.jpg" alt="Agent 模式" width="720">
</p>

## 工作方式

```mermaid
flowchart LR
  Cursor["Cursor 云端"]
  Ngrok["ngrok HTTPS 隧道"]
  LocalEndpoint["gpt2cursor"]
  CodexCLI["Codex CLI"]
  CodexSession["本地 Codex 会话"]

  Cursor -->|"OpenAI-compatible SSE"| Ngrok
  Ngrok --> LocalEndpoint
  LocalEndpoint -->|"codex exec --json via PTY"| CodexCLI
  CodexCLI --> CodexSession
  CodexSession --> CodexCLI
  CodexCLI --> LocalEndpoint
  LocalEndpoint --> Ngrok
  Ngrok --> Cursor
```

## Cursor 配置（摘要）

| 配置项 | 值 |
| --- | --- |
| Base URL | gpt2cursor 面板中的公网 HTTPS 地址（ngrok） |
| API Key | 面板中的本地 key |
| Model | `gpt2cursor-local`（需在 Cursor Settings → Models 手动添加） |

详细说明：[docs/HOW_TO_USE_CN.md](docs/HOW_TO_USE_CN.md)

## 安装（摘要）

### macOS

1. 打开 DMG → 拖到 **Applications** → 启动。
2. 若被拦截：右键 **打开**，或执行 `xattr -cr /Applications/gpt2cursor.app`。

### Windows

1. 运行 `gpt2cursor_0.4.2_x64-setup.exe`。
2. 若 SmartScreen 提示：选择 **更多信息 → 仍要运行**。

安装与排障详见 [docs/HOW_TO_USE_CN.md](docs/HOW_TO_USE_CN.md)

## 文档

| 文档 | 说明 |
| --- | --- |
| [HOW_TO_USE_CN.md](docs/HOW_TO_USE_CN.md) | 中文分步指南（Ask / Agent、ngrok、排障） |
| [HOW_TO_USE.md](docs/HOW_TO_USE.md) | English how-to guide |
| [README.md](README.md) | English project overview |

## 开发

```sh
npm install
npm run tauri          # 开发
npm test               # Rust 集成测试
npm run tauri:build    # macOS DMG + 签名脚本
```

环境：Node.js 20+、Rust 1.78+、Codex CLI。Windows 安装包由
[`.github/workflows/build-windows.yml`](.github/workflows/build-windows.yml) 构建。

## 项目状态

面向个人本地开发实验。当前聚焦 Cursor Ask/Agent、Codex CLI 桥接与跨平台打包。

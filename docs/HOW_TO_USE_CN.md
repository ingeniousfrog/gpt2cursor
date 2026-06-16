# gpt2cursor 使用指南

本文是安装、配置 Cursor，以及在 **Ask** / **Agent** 模式下使用 gpt2cursor 的分步说明。

> **支持范围：** 目前仅支持 Cursor 的 **Ask** 和 **Agent** 模式，其他模式待开发。

## 开始之前

### 你需要准备什么

| 项目 | 是否必需 | 说明 |
| --- | --- | --- |
| 已安装并登录的 [Codex CLI](https://github.com/openai/codex) | 是 | gpt2cursor 复用本机 Codex 登录态 |
| [Cursor](https://cursor.com/) | 是 | 配置 OpenAI-compatible provider |
| Node.js | 否 | 仅源码构建时需要 |
| ngrok | Agent 模式需要 | Cursor 云端无法访问 `127.0.0.1` |

### gpt2cursor 提供什么

- 本地 OpenAI-compatible 接口（`/v1/models`、`/v1/chat/completions`）
- 菜单栏 / 托盘控制面板
- 可选 ngrok 公网隧道（供 Cursor Agent 使用）
- Activity 实时请求日志

---

## 1. 安装应用

下载最新 Release：
[github.com/ingeniousfrog/gpt2cursor/releases](https://github.com/ingeniousfrog/gpt2cursor/releases)

| 平台 | 文件 | 说明 |
| --- | --- | --- |
| macOS（Apple Silicon） | `gpt2cursor_*_aarch64.dmg` | 拖到 Applications |
| Windows（x64） | `gpt2cursor_*_x64-setup.exe` | NSIS 安装程序 |

### macOS 安装

1. 打开 DMG。
2. 将 **gpt2cursor** 拖到 **Applications**。
3. 从 `/Applications` 启动。

若 macOS 拦截启动：

- 右键 **gpt2cursor** → **打开**，或
- **系统设置 → 隐私与安全性 → 仍要打开**，或
- 在终端执行：

```sh
xattr -cr /Applications/gpt2cursor.app
```

### Windows 安装

1. 运行 `gpt2cursor_*_x64-setup.exe`。
2. 完成安装向导。
3. 从开始菜单或系统托盘启动 **gpt2cursor**。

若 SmartScreen 提示未知发布者：选择 **更多信息 → 仍要运行**。

> Windows 支持仍为实验性质。开机启动等部分 macOS 专属能力在 Windows 上暂不可用。

---

## 2. 配置 gpt2cursor

1. 点击 **gpt2cursor** 菜单栏 / 托盘图标。
2. 设置或生成 **API Key**（`g2c_...`），仅用于保护本地 bridge。
3. 确认 **Port**（默认 `8787`）。
4. 点击 **Start**。
5. 复制面板中显示的 **Base URL**。

推荐默认值：

- **Timeout**：300s（Agent 长会话更稳）
- **Context msgs**：12（Agent 历史过大时可再降低）

---

## 3. 在 Cursor 中添加模型

Cursor **不会**自动从自定义 Base URL 拉取模型列表。

1. 打开 **Cursor Settings → Models**。
2. 点击 **+ Add Custom Model**。
3. 输入：`gpt2cursor-local`
4. 添加或编辑 OpenAI-compatible provider：
   - **Base URL**：gpt2cursor 面板中的地址
   - **API Key**：gpt2cursor 面板中的 key
   - **Model**：`gpt2cursor-local`

---

## 4. 使用 Ask 模式

Ask 模式直接访问本机 Base URL。

1. 在 Cursor 聊天区切换到 **Ask**。
2. 选择模型 **`gpt2cursor-local`**。
3. 发送一条简单消息（例如：`你是什么模型？`）。

常用 Base URL：

```text
http://127.0.0.1:8787/v1
```

Ask 模式**不需要** ngrok。

![Ask 模式示例](images/cursor-ask-mode.png)

在 gpt2cursor **Activity** 面板中，正常日志类似：

```text
codex exec via pty, ...
stream ok ...
```

---

## 5. 使用 Agent 模式

Agent 模式会经过 Cursor 云端，无法直接访问你电脑上的 `127.0.0.1`。

### 开启公网隧道

1. 在同一台机器安装 [ngrok](https://ngrok.com/download)。
2. 在 gpt2cursor 中开启 **Public Tunnel**。
3. 如需可粘贴 ngrok authtoken（或复用 `ngrok config add-authtoken`）。
4. 点击 **Start**，等待面板出现 **公网 HTTPS Base URL**。
5. 将该公网地址填入 Cursor Settings（不要用 `127.0.0.1`）。
6. API Key 与模型 `gpt2cursor-local` 保持不变。

![Agent 模式示例](images/cursor-agent-mode.jpg)

### Agent 模式检查清单

- [ ] gpt2cursor 中 bridge 为 **Running**
- [ ] 公网隧道显示 **https://** Base URL
- [ ] Cursor 使用的是**公网** Base URL
- [ ] 模型为 `gpt2cursor-local`
- [ ] 本机 Codex CLI 已登录

---

## 6. 验证是否正常工作

### Cursor 侧

- Ask：普通问题通常 15–60 秒内开始流式回复
- Agent：隧道就绪后表现类似

### gpt2cursor Activity 日志

| 日志 | 含义 |
| --- | --- |
| `trimmed history N -> M messages` | 已裁剪过大的 Cursor 历史 |
| `codex exec via pty` | Codex CLI 已正确启动 |
| `stream ok` | 流式回复成功 |
| `client disconnected` | Cursor 提前断开（常见于超时） |

---

## 7. 故障排查

### macOS：「gpt2cursor 已损坏，无法打开」

```sh
xattr -cr /Applications/gpt2cursor.app
```

然后重新从 Applications 打开。

### Cursor 报 Unable to read body 或中途断开

- 将 gpt2cursor **Timeout** 提高到 300s。
- 降低 **Context msgs**（建议 8–12）。
- 简单问答优先用 **Ask**；Agent 会发送更大上下文。

### Agent 模式连不上

- 确认 ngrok 已启动，面板有公网 URL。
- Cursor Base URL 必须是 **https** 隧道地址。
- ngrok URL 变化后需重启 bridge + 隧道。

### User API Key Rate limit exceeded

通常来自 Cursor 侧重试，不一定是 Codex 本身。可等待、减少并发，或先用 Ask 模式验证。

### 找不到 Codex CLI

在本机安装并登录 Codex CLI：

```sh
codex login
```

然后在 gpt2cursor 面板点击 **Refresh**。

---

## 8. 常见问题

**需要安装 Node.js 吗？**  
不需要。Release 包已内置界面；Node 仅开发构建时需要。

**会用我的 OpenAI API Key 吗？**  
不会。gpt2cursor 里的 key 只是本地 bridge 的 bearer token。

**Edit / Composer 等其他 Cursor 模式能用吗？**  
暂不支持，目前仅 **Ask** 和 **Agent**。

**公网隧道安全吗？**  
有 gpt2cursor API Key 保护，但把本地 Codex bridge 暴露到公网仍有风险，建议仅个人实验使用。

---

[← 返回 README](../README-CN.md)

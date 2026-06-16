use crate::settings::AppSettings;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
}

#[derive(Clone, Debug)]
pub struct CodexResult {
    pub text: String,
    pub usage: TokenUsage,
    pub duration_ms: u64,
}

pub trait CodexExecutor: Send + Sync {
    fn execute(&self, settings: &AppSettings, prompt: &str) -> Result<CodexResult, String>;
}

pub struct RealCodexExecutor;

impl CodexExecutor for RealCodexExecutor {
    fn execute(&self, settings: &AppSettings, prompt: &str) -> Result<CodexResult, String> {
        let started = Instant::now();
        let stdout = run_codex_in_pty(settings, prompt)?;
        let duration_ms = started.elapsed().as_millis() as u64;
        let (text, usage) = parse_codex_jsonl(&stdout);
        if text.trim().is_empty() && !stdout.trim().is_empty() {
            return Err(format!(
                "Codex CLI returned no assistant text. Raw output: {}",
                truncate_for_error(&stdout)
            ));
        }
        Ok(CodexResult {
            text,
            usage,
            duration_ms,
        })
    }
}

fn run_codex_in_pty(settings: &AppSettings, prompt: &str) -> Result<String, String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| format!("Unable to open PTY for Codex CLI: {err}"))?;

    let (use_stdin, command) = build_codex_command(settings, prompt);
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|err| format!("Unable to start Codex CLI: {err}"))?;
    drop(pair.slave);

    if use_stdin {
        let mut writer = pair
            .master
            .take_writer()
            .map_err(|err| format!("Unable to write prompt to Codex CLI: {err}"))?;
        writer
            .write_all(prompt.as_bytes())
            .map_err(|err| format!("Unable to write prompt to Codex CLI: {err}"))?;
    }

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| format!("Unable to read Codex CLI output: {err}"))?;
    let reader_handle = thread::spawn(move || {
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output);
        output
    });

    let timeout = Duration::from_millis(settings.codex_timeout_ms);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = reader_handle
                    .join()
                    .map_err(|_| "Codex CLI reader thread panicked".to_string())?;
                if !status.success() {
                    return Err(format!(
                        "Codex CLI failed: {}",
                        truncate_for_error(&stdout)
                    ));
                }
                return Ok(stdout);
            }
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = reader_handle.join();
                return Err(format!(
                    "Codex CLI request timed out after {}s",
                    timeout.as_secs().max(1)
                ));
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(err) => return Err(format!("Unable to wait for Codex CLI: {err}")),
        }
    }
}

fn build_codex_command(settings: &AppSettings, prompt: &str) -> (bool, CommandBuilder) {
    let executable = resolve_codex_command(&settings.codex_command);
    let mut cmd = CommandBuilder::new(executable);
    cmd.arg("exec");
    cmd.arg("--json");
    cmd.arg("--color");
    cmd.arg("never");
    cmd.arg("-s");
    cmd.arg(&settings.codex_sandbox);
    cmd.arg("--skip-git-repo-check");
    cmd.arg("--ephemeral");

    if !settings.codex_model.trim().is_empty() {
        cmd.arg("-m");
        cmd.arg(settings.codex_model.trim());
    }
    if !settings.codex_profile.trim().is_empty() {
        cmd.arg("-p");
        cmd.arg(settings.codex_profile.trim());
    }
    if !settings.codex_workdir.trim().is_empty() {
        cmd.arg("-C");
        cmd.arg(settings.codex_workdir.trim());
    }

    match settings.codex_approval.as_str() {
        "never" => {
            cmd.arg("-c");
            cmd.arg("approval_policy=never");
        }
        "untrusted" => {
            cmd.arg("-c");
            cmd.arg("approval_policy=on-failure");
        }
        _ => {}
    }

    if settings.codex_sandbox == "danger-full-access" && settings.codex_approval == "never" {
        cmd.arg("--dangerously-bypass-approvals-and-sandbox");
    }

    let use_stdin = prompt.len() > 120_000;
    if use_stdin {
        cmd.arg("-");
    } else {
        cmd.arg(prompt);
    }

    (use_stdin, cmd)
}

fn truncate_for_error(text: &str) -> String {
    const LIMIT: usize = 400;
    if text.len() <= LIMIT {
        return text.trim().to_string();
    }
    format!("{}...", text.chars().take(LIMIT).collect::<String>().trim())
}

pub fn resolve_codex_command(command: &str) -> PathBuf {
    if command.contains('/') || command.contains('\\') {
        return PathBuf::from(command);
    }

    if Command::new("codex")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        return PathBuf::from("codex");
    }

    #[cfg(windows)]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            for relative in [
                "Programs\\codex\\codex.exe",
                "codex\\codex.exe",
                "OpenAI\\Codex\\codex.exe",
            ] {
                let path = PathBuf::from(&local_app_data).join(relative);
                if path.is_file() {
                    return path;
                }
            }
        }
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            for relative in [".local\\bin\\codex.exe", ".codex\\bin\\codex.exe"] {
                let path = PathBuf::from(&user_profile).join(relative);
                if path.is_file() {
                    return path;
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        for candidate in [
            "/opt/homebrew/bin/codex",
            "/usr/local/bin/codex",
            "/Applications/Codex.app/Contents/Resources/codex",
        ] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return path;
            }
        }
    }

    PathBuf::from(command)
}

pub const MAX_CODEX_PROMPT_CHARS: usize = 32_000;
const MAX_MESSAGE_CHARS: usize = 4_000;

pub fn trim_messages_for_codex(
    messages: &[ChatMessage],
    max_messages: usize,
) -> (Vec<ChatMessage>, usize) {
    let original_len = messages.len();
    if original_len == 0 {
        return (Vec::new(), 0);
    }

    let max_messages = max_messages.max(1);
    if original_len <= max_messages && format_prompt(messages).len() <= MAX_CODEX_PROMPT_CHARS {
        return (messages.to_vec(), original_len);
    }

    let system = messages
        .iter()
        .filter(|message| message.role == "system")
        .cloned()
        .collect::<Vec<_>>();
    let non_system = messages
        .iter()
        .filter(|message| message.role != "system")
        .cloned()
        .collect::<Vec<_>>();

    let tail_budget = max_messages.saturating_sub(system.len()).max(1);
    let mut tail = non_system
        .into_iter()
        .rev()
        .take(tail_budget)
        .collect::<Vec<_>>();
    tail.reverse();

    let mut trimmed = system;
    trimmed.extend(tail);
    trimmed = trimmed
        .into_iter()
        .map(|mut message| {
            message.content = truncate_message_content(&message.content);
            message
        })
        .collect();

    while trimmed.len() > 1 && format_prompt(&trimmed).len() > MAX_CODEX_PROMPT_CHARS {
        let Some(index) = trimmed.iter().position(|message| message.role != "system") else {
            break;
        };
        trimmed.remove(index);
    }

    (trimmed, original_len)
}

fn truncate_message_content(content: &str) -> String {
    if content.len() <= MAX_MESSAGE_CHARS {
        return content.to_string();
    }
    format!(
        "{}... [truncated {} chars]",
        &content[..MAX_MESSAGE_CHARS],
        content.len() - MAX_MESSAGE_CHARS
    )
}

pub enum CodexStreamEvent<'a> {
    TextDelta(&'a str),
    Keepalive,
}

pub fn run_codex_in_pty_streaming<F>(
    settings: &AppSettings,
    prompt: &str,
    mut on_event: F,
) -> Result<CodexResult, String>
where
    F: FnMut(CodexStreamEvent<'_>) -> Result<(), String>,
{
    let started = Instant::now();
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| format!("Unable to open PTY for Codex CLI: {err}"))?;

    let (use_stdin, command) = build_codex_command(settings, prompt);
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|err| format!("Unable to start Codex CLI: {err}"))?;
    drop(pair.slave);

    if use_stdin {
        let mut writer = pair
            .master
            .take_writer()
            .map_err(|err| format!("Unable to write prompt to Codex CLI: {err}"))?;
        writer
            .write_all(prompt.as_bytes())
            .map_err(|err| format!("Unable to write prompt to Codex CLI: {err}"))?;
    }

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| format!("Unable to read Codex CLI output: {err}"))?;
    let (line_tx, line_rx) = std::sync::mpsc::channel();
    let reader_handle = thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let mut buffered = BufReader::new(&mut reader);
        let mut line = String::new();
        loop {
            line.clear();
            match buffered.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if line_tx.send(line.clone()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let timeout = Duration::from_millis(settings.codex_timeout_ms);
    let mut stdout = String::new();
    let mut message_texts = Vec::new();
    let mut usage = TokenUsage::default();
    let mut last_sent_text = String::new();
    let mut last_keepalive = Instant::now();

    loop {
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = reader_handle.join();
            return Err(format!(
                "Codex CLI request timed out after {}s",
                timeout.as_secs().max(1)
            ));
        }

        match line_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(line) => {
                stdout.push_str(&line);
                if let Ok(event) = serde_json::from_str::<Value>(line.trim()) {
                    if let Some(text) = extract_message_text(&event) {
                        message_texts.push(text.clone());
                        let delta = if text.starts_with(&last_sent_text) {
                            text[last_sent_text.len()..].to_string()
                        } else {
                            text.clone()
                        };
                        if !delta.is_empty() {
                            if let Err(err) = on_event(CodexStreamEvent::TextDelta(&delta)) {
                                let _ = child.kill();
                                let _ = reader_handle.join();
                                return Err(err);
                            }
                            last_sent_text = text;
                        }
                    }
                    if let Some(next_usage) = extract_usage(&event) {
                        usage = next_usage;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if last_keepalive.elapsed() >= Duration::from_secs(10) {
                    if let Err(err) = on_event(CodexStreamEvent::Keepalive) {
                        let _ = child.kill();
                        let _ = reader_handle.join();
                        return Err(err);
                    }
                    last_keepalive = Instant::now();
                }
                if matches!(child.try_wait(), Ok(Some(_))) {
                    while let Ok(line) = line_rx.try_recv() {
                        stdout.push_str(&line);
                        if let Ok(event) = serde_json::from_str::<Value>(line.trim()) {
                            if let Some(text) = extract_message_text(&event) {
                                message_texts.push(text.clone());
                                let delta = if text.starts_with(&last_sent_text) {
                                    text[last_sent_text.len()..].to_string()
                                } else {
                                    text.clone()
                                };
                                if !delta.is_empty() {
                                    if let Err(err) = on_event(CodexStreamEvent::TextDelta(&delta)) {
                                        let _ = child.kill();
                                        let _ = reader_handle.join();
                                        return Err(err);
                                    }
                                    last_sent_text = text;
                                }
                            }
                            if let Some(next_usage) = extract_usage(&event) {
                                usage = next_usage;
                            }
                        }
                    }
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    break;
                }
                thread::sleep(Duration::from_millis(25));
            }
        }
    }

    let _ = reader_handle.join();
    let status = child
        .wait()
        .map_err(|err| format!("Unable to wait for Codex CLI: {err}"))?;
    if !status.success() {
        return Err(format!(
            "Codex CLI failed: {}",
            truncate_for_error(&stdout)
        ));
    }

    let duration_ms = started.elapsed().as_millis() as u64;
    let text = message_texts
        .last()
        .cloned()
        .unwrap_or_else(|| stdout.trim().to_string());
    if text.trim().is_empty() && !stdout.trim().is_empty() {
        return Err(format!(
            "Codex CLI returned no assistant text. Raw output: {}",
            truncate_for_error(&stdout)
        ));
    }

    Ok(CodexResult {
        text,
        usage,
        duration_ms,
    })
}

pub fn format_prompt(messages: &[ChatMessage]) -> String {
    let mut system_notes = Vec::new();
    let mut history = Vec::new();
    let mut latest_user = String::new();

    for message in messages {
        match message.role.as_str() {
            "system" => system_notes.push(message.content.clone()),
            "user" => {
                if !latest_user.is_empty() {
                    history.push(format!("User:\n{latest_user}"));
                }
                latest_user = message.content.clone();
            }
            role => history.push(format!("{role}:\n{}", message.content)),
        }
    }

    let mut prompt = String::from(
        "You are answering through a Cursor chat bridge powered by Codex. \
Reply directly to the latest user message in the same language they used. \
Do not run shell commands or edit files unless the latest user message clearly asks for code changes.\n\n",
    );

    if !system_notes.is_empty() {
        prompt.push_str(&format!("System notes:\n{}\n\n", system_notes.join("\n")));
    }
    if !history.is_empty() {
        prompt.push_str(&format!("Earlier conversation:\n{}\n\n", history.join("\n\n")));
    }
    prompt.push_str(&format!("Latest user message:\n{latest_user}"));
    prompt
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub fn parse_codex_jsonl(stdout: &str) -> (String, TokenUsage) {
    let mut messages = Vec::new();
    let mut usage = TokenUsage::default();

    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        if let Some(text) = extract_message_text(&event) {
            messages.push(text);
        }
        if let Some(next_usage) = extract_usage(&event) {
            usage = next_usage;
        }
    }

    let text = messages
        .last()
        .cloned()
        .unwrap_or_else(|| stdout.trim().to_string());
    (text, usage)
}

fn extract_message_text(event: &Value) -> Option<String> {
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or_default();
    if matches!(event_type, "agent_message" | "message" | "assistant_message") {
        for key in ["message", "text", "content"] {
            if let Some(value) = event.get(key).and_then(Value::as_str) {
                return Some(value.to_string());
            }
        }
    }

    let item = event.get("item")?;
    for key in ["message", "text", "content"] {
        if let Some(value) = item.get(key).and_then(Value::as_str) {
            return Some(value.to_string());
        }
    }

    item.get("content")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.is_empty())
}

fn extract_usage(event: &Value) -> Option<TokenUsage> {
    let usage = event.get("usage")?;
    Some(TokenUsage {
        input_tokens: usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0),
        cached_input_tokens: usage
            .get("cached_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0),
        reasoning_output_tokens: usage
            .get("reasoning_output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct CodexModelOption {
    pub id: String,
    pub label: String,
}

pub fn list_codex_models() -> Result<Vec<CodexModelOption>, String> {
    let output = Command::new(resolve_codex_command("codex"))
        .args(["debug", "models"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("Unable to run codex debug models: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "codex debug models failed".to_string()
        } else {
            stderr
        });
    }

    parse_codex_models(&String::from_utf8_lossy(&output.stdout))
}

pub fn parse_codex_models(stdout: &str) -> Result<Vec<CodexModelOption>, String> {
    let value: Value = serde_json::from_str(stdout.trim())
        .map_err(|err| format!("Unable to parse codex model catalog: {err}"))?;
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| "Codex model catalog is missing a models array".to_string())?;

    let mut options = Vec::new();
    for model in models {
        let Some(id) = model
            .get("id")
            .or_else(|| model.get("slug"))
            .or_else(|| model.get("name"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let label = model
            .get("display_name")
            .and_then(Value::as_str)
            .or_else(|| model.get("name").and_then(Value::as_str))
            .unwrap_or(id);
        options.push(CodexModelOption {
            id: id.to_string(),
            label: label.to_string(),
        });
    }

    if options.is_empty() {
        return Err(
            "Codex model catalog is empty. Run `codex login` in Terminal, then try again."
                .to_string(),
        );
    }

    Ok(options)
}

#[derive(Clone, Debug, Serialize)]
pub struct CodexAccountStatus {
    pub cli_installed: bool,
    pub authenticated: bool,
    pub summary: String,
    pub detail: String,
    pub checked_at_ms: u64,
}

pub fn probe_codex_status() -> CodexAccountStatus {
    let checked_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let executable = resolve_codex_command("codex");
    let cli_installed = executable.exists() || which_codex_on_path();

    if !cli_installed {
        return CodexAccountStatus {
            cli_installed: false,
            authenticated: false,
            summary: "Codex CLI not found".to_string(),
            detail: "Install the Codex CLI and sign in locally. gpt2cursor reuses that session.".to_string(),
            checked_at_ms,
        };
    }

    let authenticated = has_local_codex_auth();
    let summary = if authenticated {
        "Codex CLI is authenticated".to_string()
    } else {
        "Codex CLI found; sign in required".to_string()
    };
    let detail = if authenticated {
        "Local CLI session is ready. Per-session token usage updates below; account quota is not exposed by this CLI.".to_string()
    } else {
        "Run `codex login` in Terminal, then refresh. Account quota is unavailable through the local CLI.".to_string()
    };

    CodexAccountStatus {
        cli_installed: true,
        authenticated,
        summary,
        detail,
        checked_at_ms,
    }
}

fn which_codex_on_path() -> bool {
    #[cfg(windows)]
    let program = "where";
    #[cfg(not(windows))]
    let program = "which";

    Command::new(program)
        .arg("codex")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn has_local_codex_auth() -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    for relative in [
        ".codex/auth.json",
        ".codex/credentials.json",
        ".config/codex/auth.json",
    ] {
        if home.join(relative).is_file() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_model_catalog() {
        let json = r#"{"models":[{"slug":"gpt-5.5","display_name":"GPT-5.5"},{"id":"gpt-5.4","display_name":"GPT-5.4"}]}"#;
        let models = parse_codex_models(json).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-5.5");
        assert_eq!(models[1].label, "GPT-5.4");
    }

    #[test]
    fn trims_long_chat_history_for_codex() {
        let messages = (0..120)
            .map(|index| ChatMessage {
                role: if index % 2 == 0 { "user" } else { "assistant" }.to_string(),
                content: format!("message {index}"),
            })
            .collect::<Vec<_>>();
        let (trimmed, original) = trim_messages_for_codex(&messages, 32);
        assert_eq!(original, 120);
        assert_eq!(trimmed.len(), 32);
        assert_eq!(trimmed.first().unwrap().content, "message 88");
        assert_eq!(trimmed.last().unwrap().content, "message 119");
    }

    #[test]
    fn preserves_system_messages_when_trimming() {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "Be concise.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "first".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "second".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "latest".to_string(),
            },
        ];
        let (trimmed, original) = trim_messages_for_codex(&messages, 2);
        assert_eq!(original, 4);
        assert_eq!(trimmed.len(), 2);
        assert_eq!(trimmed[0].role, "system");
        assert_eq!(trimmed[1].content, "latest");
    }

    #[test]
    fn formats_prompt_from_chat_messages() {
        let prompt = format_prompt(&[
            ChatMessage {
                role: "system".to_string(),
                content: "Be concise.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
        ]);
        assert!(prompt.contains("System notes:\nBe concise."));
        assert!(prompt.contains("Latest user message:\nHello"));
    }

    #[test]
    fn parses_current_codex_item_text_and_usage() {
        let jsonl = r#"{"type":"item.completed","item":{"type":"agent_message","text":"OK"}}"#.to_string()
            + "\n"
            + r#"{"type":"turn.completed","usage":{"input_tokens":10,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":1}}"#;
        let (text, usage) = parse_codex_jsonl(&jsonl);
        assert_eq!(text, "OK");
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 3);
    }

    #[test]
    fn preserves_explicit_codex_path() {
        assert_eq!(
            resolve_codex_command("/tmp/codex"),
            std::path::PathBuf::from("/tmp/codex")
        );
    }
}

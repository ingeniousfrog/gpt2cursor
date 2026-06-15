use crate::settings::AppSettings;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
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
        let executable = resolve_codex_command(&settings.codex_command);
        let mut command = Command::new(executable);
        command
            .arg("--ask-for-approval")
            .arg(&settings.codex_approval)
            .arg("exec")
            .arg("--json")
            .arg("--color")
            .arg("never")
            .arg("--sandbox")
            .arg(&settings.codex_sandbox)
            .arg("-");

        if !settings.codex_profile.trim().is_empty() {
            command.arg("--profile").arg(&settings.codex_profile);
        }
        if !settings.codex_model.trim().is_empty() {
            command.arg("-m").arg(&settings.codex_model);
        }

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("Unable to start Codex CLI: {err}"))?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(prompt.as_bytes())
                .map_err(|err| format!("Unable to write prompt to Codex CLI: {err}"))?;
        }

        let output = wait_with_timeout(child, Duration::from_millis(settings.codex_timeout_ms))?;
        let duration_ms = started.elapsed().as_millis() as u64;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(format!("Codex CLI failed: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let (text, usage) = parse_codex_jsonl(&stdout);
        Ok(CodexResult {
            text,
            usage,
            duration_ms,
        })
    }
}

pub fn resolve_codex_command(command: &str) -> PathBuf {
    if command.contains('/') {
        return PathBuf::from(command);
    }

    for candidate in [
        "/opt/homebrew/bin/codex",
        "/usr/local/bin/codex",
        "/Applications/Codex.app/Contents/Resources/codex",
    ] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return path;
        }
    }

    PathBuf::from(command)
}

pub fn format_prompt(messages: &[ChatMessage]) -> String {
    let transcript = messages
        .iter()
        .map(|message| format!("{}:\n{}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("{transcript}\n\nassistant:")
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

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|err| format!("Unable to read Codex output: {err}"));
            }
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                return Err("Codex CLI request timed out".to_string());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(err) => return Err(format!("Unable to wait for Codex CLI: {err}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(prompt, "system:\nBe concise.\n\nuser:\nHello\n\nassistant:");
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

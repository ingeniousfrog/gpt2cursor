use crate::{
    codex::{
        format_prompt, run_codex_in_pty_streaming, trim_messages_for_codex, truncate_message_content,
        ChatMessage, CodexExecutor, CodexStreamEvent, TokenUsage, MAX_CODEX_PROMPT_CHARS,
    },
    settings::AppSettings,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const HOST: &str = "127.0.0.1";
// Hard cap for the raw HTTP body Cursor sends. Parsed messages are trimmed long before Codex.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const ERR_BODY_TOO_LARGE: &str = "request_body_too_large";
const USER_BODY_TOO_LARGE_HINT: &str = "Request body too large. Lower Context msgs in gpt2cursor, or start a new Cursor Agent chat.";
const PROMPT_TOO_LARGE_HINT: &str = "Prompt too large after trimming. Lower Context msgs in gpt2cursor, or start a new Cursor Agent chat.";
const ERR_PROMPT_TOO_LARGE: &str = "prompt_too_large";
const PARSE_MESSAGE_HEADROOM: usize = 4;
const MAX_RECENT_LOGS: usize = 40;
const MAX_RECENT_LOGS_DEV: usize = 200;
const MAX_LOG_LINE_CHARS: usize = 2_048;
const MAX_DEV_BODY_LOG_CHARS: usize = 4_096;
pub const CURSOR_MODEL_ID: &str = "gpt2cursor-local";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriteOutcome {
    Ok,
    ClientDisconnected,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UsageSnapshot {
    pub request_count: u64,
    pub active_requests: u64,
    pub last_duration_ms: u64,
    pub total_duration_ms: u64,
    pub last_usage: TokenUsage,
    pub total_usage: TokenUsage,
    pub last_error: Option<String>,
    #[serde(default)]
    pub recent_logs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BridgeStatus {
    pub running: bool,
    pub port: u16,
    pub base_url: String,
    pub usage: UsageSnapshot,
}

pub struct BridgeRuntime {
    pub port: u16,
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl BridgeRuntime {
    pub fn is_alive(&self) -> bool {
        !self
            .join
            .as_ref()
            .map(|handle| handle.is_finished())
            .unwrap_or(true)
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect((HOST, self.port));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub fn base_url(port: u16) -> String {
    format!("http://{HOST}:{port}/v1")
}

pub fn is_port_available(port: u16) -> bool {
    TcpListener::bind((HOST, port)).is_ok()
}

pub fn start_bridge(
    settings: Arc<Mutex<AppSettings>>,
    usage: Arc<Mutex<UsageSnapshot>>,
    executor: Arc<dyn CodexExecutor>,
) -> Result<BridgeRuntime, String> {
    let port = {
        let settings_guard = settings
            .lock()
            .map_err(|_| "Settings state is unavailable".to_string())?;
        settings_guard.validate()?;
        settings_guard.port
    };
    let listener = TcpListener::bind((HOST, port))
        .map_err(|err| format!("Port {port} is not available: {err}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("Unable to configure listener: {err}"))?;

    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let listener_settings = Arc::clone(&settings);
    let join = thread::spawn(move || {
        while !thread_stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let request_settings = match listener_settings.lock() {
                        Ok(settings_guard) => settings_guard.clone(),
                        Err(_) => continue,
                    };
                    let request_usage = Arc::clone(&usage);
                    let request_executor = Arc::clone(&executor);
                    thread::spawn(move || {
                        handle_stream(stream, request_settings, request_usage, request_executor);
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(25));
                }
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
                Err(err) => {
                    eprintln!("bridge accept error: {err}");
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    });

    Ok(BridgeRuntime {
        port,
        stop,
        join: Some(join),
    })
}

fn handle_stream(
    mut stream: TcpStream,
    settings: AppSettings,
    usage: Arc<Mutex<UsageSnapshot>>,
    executor: Arc<dyn CodexExecutor>,
) {
    let _ = stream.set_nonblocking(false);
    let started = Instant::now();
    increment_active(&usage);
    let result = handle_request(
        &mut stream,
        &settings,
        Arc::clone(&usage),
        Arc::clone(&executor),
    );
    let duration_ms = started.elapsed().as_millis() as u64;
    decrement_active(&usage, duration_ms, result.as_ref().err().cloned());
}

fn handle_request(
    stream: &mut TcpStream,
    settings: &AppSettings,
    usage: Arc<Mutex<UsageSnapshot>>,
    executor: Arc<dyn CodexExecutor>,
) -> Result<(), String> {
    let request = match read_request(stream) {
        Ok(value) => value,
        Err(message) if message == ERR_BODY_TOO_LARGE => {
            append_log(&usage, USER_BODY_TOO_LARGE_HINT.to_string(), false);
            write_json(stream, 413, body_too_large_error_json())?;
            return Err(USER_BODY_TOO_LARGE_HINT.to_string());
        }
        Err(message) => return Err(message),
    };
    let route = normalize_route(&request.method, &request.path);
    append_log(
        &usage,
        format!("{} {} ({})", request.method, request.path, route.label()),
        settings.dev_mode,
    );

    if !is_authorized(&request.headers, &settings.api_key) {
        write_json(
            stream,
            401,
            json!({"error":{"message":"Missing or invalid bearer token","type":"authentication_error"}}),
        )?;
        return Err("authentication failed".to_string());
    }

    match route {
        Route::Healthz => write_json(stream, 200, json!({"ok":true})),
        Route::Models => write_json(stream, 200, models_payload()),
        Route::ChatCompletions => handle_chat(stream, settings, usage, executor, request.body),
        Route::NotFound => write_json(
            stream,
            404,
            json!({"error":{"message":"Route not found","type":"not_found"}}),
        ),
    }
}

fn handle_chat(
    stream: &mut TcpStream,
    settings: &AppSettings,
    usage: Arc<Mutex<UsageSnapshot>>,
    executor: Arc<dyn CodexExecutor>,
    body: Vec<u8>,
) -> Result<(), String> {
    let input: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => {
            write_json(
                stream,
                400,
                json!({"error":{"message":"Request body must be valid JSON","type":"invalid_request_error"}}),
            )?;
            return Err("invalid json".to_string());
        }
    };
    let incoming_message_count = count_incoming_messages(&input);
    let max_raw_messages = settings
        .codex_max_messages
        .saturating_mul(PARSE_MESSAGE_HEADROOM)
        .max(16);
    let chat = match parse_chat_request(&input, max_raw_messages) {
        Ok(value) => value,
        Err(message) => {
            append_log(&usage, format!("chat parse error: {message}"), settings.dev_mode);
            write_json(
                stream,
                400,
                json!({"error":{"message":message,"type":"invalid_request_error"}}),
            )?;
            return Err("invalid chat request".to_string());
        }
    };
    if incoming_message_count > chat.messages.len() {
        append_log(
            &usage,
            format!(
                "pre-trimmed {} -> {} messages ({} KB body; lower Context msgs or start a new chat if this keeps happening)",
                incoming_message_count,
                chat.messages.len(),
                body.len() / 1024
            ),
            true,
        );
    }
    append_log(
        &usage,
        format!(
            "chat {} stream={} messages={} (body {} KB)",
            chat.model,
            chat.stream,
            chat.messages.len(),
            body.len() / 1024
        ),
        settings.dev_mode,
    );
    if settings.dev_mode {
        append_log(
            &usage,
            format!(
                "dev request body: {}",
                truncate_log_text(
                    String::from_utf8_lossy(&body).trim(),
                    MAX_DEV_BODY_LOG_CHARS,
                )
            ),
            true,
        );
    }

    let (prompt, prompt_meta) = build_codex_prompt(&chat.messages, settings);
    if prompt_meta.prompt_chars > MAX_CODEX_PROMPT_CHARS {
        append_log(
            &usage,
            format!(
                "{} ({} chars after trim)",
                PROMPT_TOO_LARGE_HINT, prompt_meta.prompt_chars
            ),
            true,
        );
        write_json(
            stream,
            413,
            json!({
                "error": {
                    "message": PROMPT_TOO_LARGE_HINT,
                    "type": "invalid_request_error",
                    "code": ERR_PROMPT_TOO_LARGE
                }
            }),
        )?;
        return Err(PROMPT_TOO_LARGE_HINT.to_string());
    }
    append_log(
        &usage,
        format!(
            "codex exec via pty, {} chars, {} msgs, timeout {}s",
            prompt_meta.prompt_chars,
            prompt_meta.sent_messages,
            settings.codex_timeout_ms / 1000
        ),
        true,
    );
    if settings.dev_mode {
        append_log(
            &usage,
            format!("dev codex prompt: {}", truncate_log_text(&prompt, MAX_LOG_LINE_CHARS)),
            true,
        );
    }
    if prompt_meta.original_messages > prompt_meta.sent_messages {
        append_log(
            &usage,
            format!(
                "trimmed history {} -> {} messages, {} chars prompt",
                prompt_meta.original_messages,
                prompt_meta.sent_messages,
                prompt_meta.prompt_chars
            ),
            true,
        );
    }

    if chat.stream {
        return handle_chat_streaming(stream, settings, usage, executor, &chat, &prompt);
    }

    let codex = match executor.execute(settings, &prompt) {
        Ok(value) => value,
        Err(message) => {
            append_log(&usage, format!("codex failed: {message}"), settings.dev_mode);
            write_json(
                stream,
                502,
                json!({"error":{"message":"Codex CLI request failed","type":"upstream_error","detail":message}}),
            )?;
            return Err("codex failed".to_string());
        }
    };
    let created = now_epoch_seconds();

    let result = write_json(
        stream,
        200,
        json!({
            "id": format!("chatcmpl-{created}"),
            "object":"chat.completion",
            "created": created,
            "model": chat.model,
            "choices":[{
                "index":0,
                "message":{"role":"assistant","content":codex.text},
                "finish_reason":"stop"
            }],
            "usage": {
                "prompt_tokens": codex.usage.input_tokens,
                "completion_tokens": codex.usage.output_tokens,
                "total_tokens": codex.usage.input_tokens + codex.usage.output_tokens
            },
            "gpt2cursor": {
                "duration_ms": codex.duration_ms,
                "reasoning_output_tokens": codex.usage.reasoning_output_tokens,
                "cached_input_tokens": codex.usage.cached_input_tokens
            }
        }),
    );
    record_success(&usage, &codex.usage, codex.duration_ms);
    append_log(
        &usage,
        format!(
            "chat ok {}ms out={} tok",
            codex.duration_ms, codex.usage.output_tokens
        ),
        settings.dev_mode,
    );
    if settings.dev_mode {
        append_log(
            &usage,
            format!(
                "dev codex result: {}",
                truncate_log_text(&codex.text, MAX_LOG_LINE_CHARS)
            ),
            true,
        );
    }
    result
}

fn handle_chat_streaming(
    stream: &mut TcpStream,
    settings: &AppSettings,
    usage: Arc<Mutex<UsageSnapshot>>,
    _executor: Arc<dyn CodexExecutor>,
    chat: &ChatRequest,
    prompt: &str,
) -> Result<(), String> {
    let created = now_epoch_seconds();
    let id = format!("chatcmpl-{created}");
    let model = chat.model.clone();
    let dev_mode = settings.dev_mode;
    let usage_for_stream = Arc::clone(&usage);

    if matches!(start_sse(stream)?, WriteOutcome::ClientDisconnected) {
        append_log(
            &usage,
            "client disconnected before stream started".to_string(),
            dev_mode,
        );
        return Ok(());
    }
    if matches!(
        write_sse_data(
            stream,
            &json!({
                "id": id,
                "object": "chat.completion.chunk",
                "created": created,
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": {"role": "assistant"},
                    "finish_reason": null
                }]
            }),
        )?,
        WriteOutcome::ClientDisconnected
    ) {
        append_log(
            &usage,
            "client disconnected before codex started".to_string(),
            dev_mode,
        );
        return Ok(());
    }

    let codex = match run_codex_in_pty_streaming(settings, prompt, |event| {
        match event {
            CodexStreamEvent::TextDelta(delta) => match write_sse_data(
                stream,
                &json!({
                    "id": id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": model,
                    "choices": [{
                        "index": 0,
                        "delta": {"content": delta},
                        "finish_reason": null
                    }]
                }),
            )? {
                WriteOutcome::Ok => Ok(()),
                WriteOutcome::ClientDisconnected => Err("client disconnected".to_string()),
            },
            CodexStreamEvent::Reasoning(text) | CodexStreamEvent::Activity(text) => {
                append_log(
                    &usage_for_stream,
                    format!("codex: {text}"),
                    dev_mode,
                );
                match write_sse_reasoning(stream, &id, created, &model, &text)? {
                    WriteOutcome::Ok => Ok(()),
                    WriteOutcome::ClientDisconnected => Err("client disconnected".to_string()),
                }
            }
            CodexStreamEvent::Raw(text) => {
                append_log(
                    &usage_for_stream,
                    format!("dev codex jsonl: {text}"),
                    true,
                );
                Ok(())
            }
            CodexStreamEvent::Keepalive { elapsed_secs } => {
                let heartbeat = format!("still working... {elapsed_secs}s");
                append_log(
                    &usage_for_stream,
                    format!("codex: {heartbeat}"),
                    dev_mode,
                );
                match write_sse_reasoning(stream, &id, created, &model, &heartbeat)? {
                    WriteOutcome::Ok => Ok(()),
                    WriteOutcome::ClientDisconnected => Err("client disconnected".to_string()),
                }
            }
        }
    }) {
        Ok(value) => value,
        Err(message) if message == "client disconnected" => {
            append_log(
                &usage,
                "client disconnected during codex stream".to_string(),
                dev_mode,
            );
            return Ok(());
        }
        Err(message) => {
            append_log(
                &usage,
                format!("codex failed (stream): {message}"),
                dev_mode,
            );
            if try_write_stream_error(stream, &id, created, &model, &message)? {
                return Ok(());
            }
            return Err("codex failed".to_string());
        }
    };

    if matches!(
        write_sse_finish(stream, &id, created, &model)?,
        WriteOutcome::ClientDisconnected
    ) {
        append_log(
            &usage,
            format!(
                "client disconnected after stream finished ({}ms)",
                codex.duration_ms
            ),
            dev_mode,
        );
        record_success(&usage, &codex.usage, codex.duration_ms);
        return Ok(());
    }
    record_success(&usage, &codex.usage, codex.duration_ms);
    append_log(
        &usage,
        format!(
            "stream ok {}ms out={} tok",
            codex.duration_ms, codex.usage.output_tokens
        ),
        dev_mode,
    );
    if dev_mode {
        append_log(
            &usage,
            format!(
                "dev codex result: {}",
                truncate_log_text(&codex.text, MAX_LOG_LINE_CHARS)
            ),
            true,
        );
    }
    Ok(())
}

fn try_write_stream_error(
    stream: &mut TcpStream,
    id: &str,
    created: u64,
    model: &str,
    message: &str,
) -> Result<bool, String> {
    if matches!(
        write_sse_data(
            stream,
            &json!({
                "id": id,
                "object": "chat.completion.chunk",
                "created": created,
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": {"content": format!("Codex CLI failed: {message}")},
                    "finish_reason": null
                }]
            }),
        )?,
        WriteOutcome::ClientDisconnected
    ) {
        return Ok(true);
    }
    Ok(matches!(
        write_sse_finish(stream, id, created, model)?,
        WriteOutcome::ClientDisconnected
    ))
}

struct PromptMeta {
    original_messages: usize,
    sent_messages: usize,
    prompt_chars: usize,
}

fn build_codex_prompt(messages: &[ChatMessage], settings: &AppSettings) -> (String, PromptMeta) {
    let (trimmed, original_messages) =
        trim_messages_for_codex(messages, settings.codex_max_messages);
    let prompt = format_prompt(&trimmed);
    let meta = PromptMeta {
        original_messages,
        sent_messages: trimmed.len(),
        prompt_chars: prompt.len(),
    };
    (prompt, meta)
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    stream
        .set_nonblocking(false)
        .map_err(|err| format!("Unable to configure socket: {err}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|err| format!("Unable to set read timeout: {err}"))?;
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 2048];
    let header_end;

    loop {
        let count = read_with_retry(stream, &mut chunk, "request")?;
        if count == 0 {
            return Err("Request ended before headers".to_string());
        }
        buffer.extend_from_slice(&chunk[..count]);
        if buffer.len() > MAX_BODY_BYTES {
            return Err(ERR_BODY_TOO_LARGE.to_string());
        }
        if let Some(index) = find_header_end(&buffer) {
            header_end = index;
            break;
        }
    }

    let headers_raw = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = headers_raw.split("\r\n");
    let request_line = lines.next().ok_or_else(|| "Missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let path = request_parts.next().unwrap_or_default().to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect::<HashMap<_, _>>();
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    if content_length > MAX_BODY_BYTES {
        return Err(ERR_BODY_TOO_LARGE.to_string());
    }

    let body_start = header_end + 4;
    let mut body = buffer.get(body_start..).unwrap_or_default().to_vec();
    while body.len() < content_length {
        let count = read_with_retry(stream, &mut chunk, "body")?;
        if count == 0 {
            return Err(format!(
                "Request body incomplete: received {} of {} bytes",
                body.len(),
                content_length
            ));
        }
        body.extend_from_slice(&chunk[..count]);
        if body.len() > MAX_BODY_BYTES {
            return Err(ERR_BODY_TOO_LARGE.to_string());
        }
    }
    body.truncate(content_length);

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn body_too_large_error_json() -> Value {
    json!({
        "error": {
            "message": USER_BODY_TOO_LARGE_HINT,
            "type": "invalid_request_error",
            "code": ERR_BODY_TOO_LARGE
        }
    })
}

fn count_incoming_messages(input: &Value) -> usize {
    if let Some(messages) = input.get("messages").and_then(Value::as_array) {
        return messages.len();
    }
    input
        .get("input")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0)
}

fn tail_raw_json_messages(messages: Vec<Value>, max_messages: usize) -> Vec<Value> {
    if messages.len() <= max_messages {
        return messages;
    }

    let system = messages
        .iter()
        .filter(|message| message_role(message) == Some("system"))
        .cloned()
        .collect::<Vec<_>>();
    let non_system = messages
        .into_iter()
        .filter(|message| message_role(message) != Some("system"))
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
    trimmed
}

fn message_role(message: &Value) -> Option<&str> {
    message.get("role").and_then(Value::as_str)
}

fn parse_chat_request(input: &Value, max_raw_messages: usize) -> Result<ChatRequest, String> {
    let model = input
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(CURSOR_MODEL_ID)
        .to_string();
    let stream = input.get("stream").and_then(Value::as_bool).unwrap_or(false);
    let raw_messages = if let Some(messages) = input.get("messages").and_then(Value::as_array) {
        messages.clone()
    } else if let Some(items) = input.get("input").and_then(Value::as_array) {
        convert_input_to_messages(items)?
    } else {
        return Err("messages must include at least one message".to_string());
    };
    if raw_messages.is_empty() {
        return Err("messages must include at least one message".to_string());
    }
    let raw_messages = tail_raw_json_messages(raw_messages, max_raw_messages);

    let mut parsed = Vec::new();
    for (index, message) in raw_messages.iter().enumerate() {
        let parsed_message = parse_message(index, message);
        if !parsed_message.content.trim().is_empty() {
            parsed.push(parsed_message);
        }
    }
    if parsed.is_empty() {
        return Err("messages must include at least one message with text".to_string());
    }
    Ok(ChatRequest {
        model,
        messages: parsed,
        stream,
    })
}

fn convert_input_to_messages(items: &[Value]) -> Result<Vec<Value>, String> {
    let mut messages = Vec::new();
    for item in items {
        let item_type = item
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match item_type {
            "message" => {
                let role = item
                    .get("role")
                    .and_then(Value::as_str)
                    .unwrap_or("user");
                let content = item
                    .get("content")
                    .cloned()
                    .or_else(|| item.get("text").cloned())
                    .unwrap_or(Value::String(String::new()));
                messages.push(json!({"role": role, "content": content}));
            }
            "function_call" | "tool_call" | "tool_use" => {
                let name = item
                    .get("name")
                    .or_else(|| item.get("tool_name"))
                    .and_then(Value::as_str)
                    .unwrap_or("tool");
                let arguments = item
                    .get("arguments")
                    .or_else(|| item.get("input"))
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "{}".to_string());
                messages.push(json!({
                    "role": "assistant",
                    "content": format!("[tool_call {name}]: {arguments}")
                }));
            }
            "function_call_output" | "tool_result" => {
                let content = item
                    .get("output")
                    .or_else(|| item.get("content"))
                    .map(ToString::to_string)
                    .unwrap_or_else(|| item.to_string());
                messages.push(json!({
                    "role": "tool",
                    "content": content
                }));
            }
            _ => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    messages.push(json!({"role":"user","content": text}));
                }
            }
        }
    }
    Ok(messages)
}

fn parse_message(index: usize, message: &Value) -> ChatMessage {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let content = if let Some(content) = extract_message_content(message) {
        content
    } else if is_explicitly_empty_content(message) {
        String::new()
    } else {
        serde_json::to_string(message)
            .map(|serialized| format!("[message {index}]: {serialized}"))
            .unwrap_or_default()
    };
    ChatMessage {
        role,
        content: truncate_message_content(&content),
    }
}

fn is_explicitly_empty_content(message: &Value) -> bool {
    match message.get("content") {
        None | Some(Value::Null) => true,
        Some(Value::String(text)) => text.trim().is_empty(),
        Some(Value::Array(parts)) => parts.is_empty(),
        _ => false,
    }
}

fn extract_message_content(message: &Value) -> Option<String> {
    if let Some(content) = message.get("content") {
        if let Some(text) = extract_content_value(content) {
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
    }

    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        if !tool_calls.is_empty() {
            return Some(format_tool_calls(tool_calls));
        }
    }

    if let Some(function_call) = message.get("function_call") {
        if !function_call.is_null() {
            return Some(format_function_call(function_call));
        }
    }

    if message.get("role").and_then(Value::as_str) == Some("tool") {
        let name = message
            .get("name")
            .or_else(|| message.get("tool_call_id"))
            .and_then(Value::as_str)
            .unwrap_or("tool");
        if let Some(content) = message
            .get("content")
            .and_then(extract_content_value)
            .filter(|text| !text.trim().is_empty())
        {
            return Some(format!("[{name}]: {content}"));
        }
    }

    message
        .get("refusal")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| text.to_string())
}

fn extract_content_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Null => None,
        Value::Array(parts) => {
            let texts = parts
                .iter()
                .filter_map(extract_content_part)
                .collect::<Vec<_>>();
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        }
        Value::Object(_) => extract_content_part(value),
        _ => None,
    }
}

fn extract_content_part(part: &Value) -> Option<String> {
    for key in ["text", "input_text", "output_text", "content"] {
        if let Some(text) = part.get(key).and_then(Value::as_str) {
            if !text.trim().is_empty() {
                return Some(text.to_string());
            }
        }
    }

    let part_type = part.get("type").and_then(Value::as_str).unwrap_or_default();
    match part_type {
        "text" | "input_text" | "output_text" => part
            .get("text")
            .or_else(|| part.get("input_text"))
            .or_else(|| part.get("output_text"))
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(|text| text.to_string()),
        "image_url" | "image" | "input_image" => Some("[image]".to_string()),
        "tool_use" | "tool_call" | "function_call" => {
            let name = part
                .get("name")
                .or_else(|| part.get("tool_name"))
                .and_then(Value::as_str)
                .unwrap_or("tool");
            let input = part
                .get("input")
                .or_else(|| part.get("arguments"))
                .map(ToString::to_string)
                .unwrap_or_default();
            Some(format!("[tool_call {name}]: {input}"))
        }
        "tool_result" | "function_call_output" => {
            let content = part
                .get("content")
                .or_else(|| part.get("output"))
                .and_then(extract_content_value)
                .unwrap_or_else(|| part.to_string());
            Some(format!("[tool_result]: {content}"))
        }
        _ => None,
    }
}

fn format_tool_calls(tool_calls: &[Value]) -> String {
    tool_calls
        .iter()
        .map(|call| {
            let name = call
                .get("function")
                .and_then(|value| value.get("name"))
                .or_else(|| call.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("tool");
            let arguments = call
                .get("function")
                .and_then(|value| value.get("arguments"))
                .or_else(|| call.get("arguments"))
                .or_else(|| call.get("input"))
                .map(ToString::to_string)
                .unwrap_or_else(|| "{}".to_string());
            format!("[tool_call {name}]: {arguments}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_function_call(function_call: &Value) -> String {
    let name = function_call
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("function");
    let arguments = function_call
        .get("arguments")
        .map(ToString::to_string)
        .unwrap_or_else(|| "{}".to_string());
    format!("[function_call {name}]: {arguments}")
}

fn models_payload() -> Value {
    json!({
        "object": "list",
        "data": [{
            "id": CURSOR_MODEL_ID,
            "object": "model",
            "created": 0,
            "owned_by": "local-codex"
        }]
    })
}

fn is_authorized(headers: &HashMap<String, String>, api_key: &str) -> bool {
    headers
        .get("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| constant_time_eq(token.as_bytes(), api_key.as_bytes()))
        .unwrap_or(false)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .fold(0_u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn write_json(stream: &mut TcpStream, status: u16, payload: Value) -> Result<(), String> {
    let body = payload.to_string();
    write_response(stream, status, "application/json; charset=utf-8", body.as_bytes())
}

fn start_sse(stream: &mut TcpStream) -> Result<WriteOutcome, String> {
    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream; charset=utf-8\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n";
    write_all(stream, head.as_bytes(), "start SSE stream")
}

fn write_sse_data(stream: &mut TcpStream, payload: &Value) -> Result<WriteOutcome, String> {
    let body = format!("data: {payload}\n\n");
    write_all(stream, body.as_bytes(), "write SSE chunk")
}

fn write_sse_reasoning(
    stream: &mut TcpStream,
    id: &str,
    created: u64,
    model: &str,
    text: &str,
) -> Result<WriteOutcome, String> {
    write_sse_data(
        stream,
        &json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {"reasoning_content": text},
                "finish_reason": null
            }]
        }),
    )
}

fn write_sse_finish(
    stream: &mut TcpStream,
    id: &str,
    created: u64,
    model: &str,
) -> Result<WriteOutcome, String> {
    write_sse_data(
        stream,
        &json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
        }),
    )?;
    write_all(stream, b"data: [DONE]\n\n", "write SSE done marker")
}

fn write_all(stream: &mut TcpStream, bytes: &[u8], action: &str) -> Result<WriteOutcome, String> {
    match stream.write_all(bytes) {
        Ok(()) => Ok(WriteOutcome::Ok),
        Err(err) if is_client_disconnect(&err) => Ok(WriteOutcome::ClientDisconnected),
        Err(err) => Err(format!("Unable to {action}: {err}")),
    }
}

fn is_client_disconnect(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::BrokenPipe
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::NotConnected
    )
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        413 => "Payload Too Large",
        502 => "Bad Gateway",
        _ => "Internal Server Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {status_text}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|_| stream.write_all(body))
        .map_err(|err| {
            if is_client_disconnect(&err) {
                return String::new();
            }
            format!("Unable to write response: {err}")
        })?;
    Ok(())
}

fn increment_active(usage: &Arc<Mutex<UsageSnapshot>>) {
    if let Ok(mut snapshot) = usage.lock() {
        snapshot.active_requests += 1;
    }
}

fn decrement_active(usage: &Arc<Mutex<UsageSnapshot>>, duration_ms: u64, error: Option<String>) {
    if let Ok(mut snapshot) = usage.lock() {
        snapshot.request_count += 1;
        snapshot.active_requests = snapshot.active_requests.saturating_sub(1);
        snapshot.last_duration_ms = duration_ms;
        snapshot.total_duration_ms += duration_ms;
        snapshot.last_error = error.filter(|message| !message.is_empty());
    }
}

pub fn record_success(usage: &Arc<Mutex<UsageSnapshot>>, token_usage: &TokenUsage, duration_ms: u64) {
    if let Ok(mut snapshot) = usage.lock() {
        snapshot.last_duration_ms = duration_ms;
        snapshot.last_usage = token_usage.clone();
        snapshot.last_error = None;
        snapshot.total_usage.input_tokens += token_usage.input_tokens;
        snapshot.total_usage.cached_input_tokens += token_usage.cached_input_tokens;
        snapshot.total_usage.output_tokens += token_usage.output_tokens;
        snapshot.total_usage.reasoning_output_tokens += token_usage.reasoning_output_tokens;
    }
}

pub fn append_bridge_log(
    usage: &Arc<Mutex<UsageSnapshot>>,
    line: impl Into<String>,
    dev_mode: bool,
) {
    append_log(usage, line.into(), dev_mode);
}

fn append_log(usage: &Arc<Mutex<UsageSnapshot>>, line: String, dev_mode: bool) {
    let line = truncate_log_text(&line, MAX_LOG_LINE_CHARS);
    let stamped = format!("{} {}", format_log_timestamp(), line);
    eprintln!("[gpt2cursor] {stamped}");
    if let Ok(mut snapshot) = usage.lock() {
        snapshot.recent_logs.push(stamped);
        let max_logs = if dev_mode {
            MAX_RECENT_LOGS_DEV
        } else {
            MAX_RECENT_LOGS
        };
        if snapshot.recent_logs.len() > max_logs {
            let overflow = snapshot.recent_logs.len() - max_logs;
            snapshot.recent_logs.drain(0..overflow);
        }
    }
}

fn truncate_log_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max_chars).collect();
    format!("{truncated}... [truncated]")
}

fn format_log_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hours = (seconds / 3600) % 24;
    let minutes = (seconds / 60) % 60;
    let secs = seconds % 60;
    format!("{hours:02}:{minutes:02}:{secs:02}")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    Healthz,
    Models,
    ChatCompletions,
    NotFound,
}

impl Route {
    fn label(self) -> &'static str {
        match self {
            Self::Healthz => "healthz",
            Self::Models => "models",
            Self::ChatCompletions => "chat",
            Self::NotFound => "404",
        }
    }
}

fn normalize_route(method: &str, path: &str) -> Route {
    let path = path.split('?').next().unwrap_or(path).trim_end_matches('/');
    match (method, path) {
        ("GET", "/healthz") => Route::Healthz,
        ("GET", "/v1/models") | ("GET", "/models") => Route::Models,
        ("POST", "/v1/chat/completions") | ("POST", "/chat/completions") => Route::ChatCompletions,
        _ => Route::NotFound,
    }
}

fn read_with_retry(stream: &mut TcpStream, chunk: &mut [u8], phase: &str) -> Result<usize, String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match stream.read(chunk) {
            Ok(count) => return Ok(count),
            Err(err) if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::Interrupted => {
                if Instant::now() >= deadline {
                    return Err(format!(
                        "Timed out while reading {phase}: {err}"
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => {
                return Err(format!("Unable to read {phase}: {err}"));
            }
        }
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::CodexResult;

    struct MockExecutor;

    impl CodexExecutor for MockExecutor {
        fn execute(&self, _settings: &AppSettings, _prompt: &str) -> Result<CodexResult, String> {
            Ok(CodexResult {
                text: "mocked".to_string(),
                usage: TokenUsage {
                    input_tokens: 4,
                    output_tokens: 2,
                    ..TokenUsage::default()
                },
                duration_ms: 12,
            })
        }
    }

    struct FailingExecutor;

    impl CodexExecutor for FailingExecutor {
        fn execute(&self, _settings: &AppSettings, _prompt: &str) -> Result<CodexResult, String> {
            Err("boom".to_string())
        }
    }

    #[test]
    fn rejects_invalid_bearer_token() {
        let mut headers = HashMap::new();
        headers.insert("authorization".to_string(), "Bearer wrong".to_string());
        assert!(!is_authorized(&headers, "secret"));
    }

    #[test]
    fn accepts_valid_bearer_token() {
        let mut headers = HashMap::new();
        headers.insert("authorization".to_string(), "Bearer secret".to_string());
        assert!(is_authorized(&headers, "secret"));
    }

    #[test]
    fn parses_chat_request_with_text_parts() {
        let input = json!({
            "model":"gpt2cursor-local",
            "stream":true,
            "messages":[{"role":"user","content":[{"type":"text","text":"hello"}]}]
        });
        let chat = parse_chat_request(&input, 64).unwrap();
        assert!(chat.stream);
        assert_eq!(chat.messages[0].content, "hello");
    }

    #[test]
    fn parses_assistant_tool_calls_without_content() {
        let input = json!({
            "model":"gpt2cursor-local",
            "messages":[
                {"role":"user","content":"read file"},
                {
                    "role":"assistant",
                    "content": null,
                    "tool_calls":[{
                        "id":"call_1",
                        "type":"function",
                        "function":{"name":"Read","arguments":"{\"path\":\"main.rs\"}"}
                    }]
                }
            ]
        });
        let chat = parse_chat_request(&input, 64).unwrap();
        assert_eq!(chat.messages.len(), 2);
        assert!(chat.messages[1].content.contains("[tool_call Read]"));
    }

    #[test]
    fn parses_tool_role_messages() {
        let input = json!({
            "model":"gpt2cursor-local",
            "messages":[
                {"role":"user","content":"run"},
                {
                    "role":"tool",
                    "tool_call_id":"call_1",
                    "name":"Shell",
                    "content":"done"
                }
            ]
        });
        let chat = parse_chat_request(&input, 64).unwrap();
        assert_eq!(chat.messages[1].content, "done");
    }

    #[test]
    fn parses_input_text_content_parts() {
        let input = json!({
            "model":"gpt2cursor-local",
            "messages":[{
                "role":"user",
                "content":[{"type":"input_text","text":"hello from cursor"}]
            }]
        });
        let chat = parse_chat_request(&input, 64).unwrap();
        assert_eq!(chat.messages[0].content, "hello from cursor");
    }

    #[test]
    fn skips_empty_assistant_messages() {
        let input = json!({
            "model":"gpt2cursor-local",
            "messages":[
                {"role":"user","content":"hello"},
                {"role":"assistant","content":""}
            ]
        });
        let chat = parse_chat_request(&input, 64).unwrap();
        assert_eq!(chat.messages.len(), 1);
    }

    #[test]
    fn parses_responses_api_input_field() {
        let input = json!({
            "model":"gpt2cursor-local",
            "input":[
                {"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}
            ]
        });
        let chat = parse_chat_request(&input, 64).unwrap();
        assert_eq!(chat.messages[0].content, "hello");
    }

    #[test]
    fn detects_available_ephemeral_port() {
        assert!(is_port_available(0));
    }

    #[test]
    fn can_start_and_stop_bridge() {
        let listener = TcpListener::bind((HOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let settings = AppSettings {
            port,
            api_key: "secret".to_string(),
            ..AppSettings::default()
        };
        let usage = Arc::new(Mutex::new(UsageSnapshot::default()));
        let runtime = start_bridge(Arc::new(Mutex::new(settings)), usage, Arc::new(MockExecutor)).unwrap();
        runtime.stop();
    }

    #[test]
    fn serves_models_over_http() {
        let (runtime, port, _usage) = test_runtime(Arc::new(MockExecutor));
        let response = send_raw(port, "GET /v1/models HTTP/1.1\r\nauthorization: Bearer secret\r\n\r\n");
        runtime.stop();
        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("gpt2cursor-local"));
        assert!(!response.contains("codex-local"));
    }

    #[test]
    fn rejects_missing_bearer_over_http() {
        let (runtime, port, _usage) = test_runtime(Arc::new(MockExecutor));
        let response = send_raw(port, "GET /v1/models HTTP/1.1\r\n\r\n");
        runtime.stop();
        assert!(response.contains("HTTP/1.1 401 Unauthorized"));
        assert!(response.contains("authentication_error"));
    }

    #[test]
    fn serves_non_streaming_chat_over_http() {
        let (runtime, port, usage) = test_runtime(Arc::new(MockExecutor));
        let body = r#"{"model":"gpt2cursor-local","messages":[{"role":"user","content":"hello"}]}"#;
        let response = send_json(port, "/v1/chat/completions", body);
        runtime.stop();
        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("mocked"));
        assert_eq!(usage.lock().unwrap().total_usage.output_tokens, 2);
    }

    #[test]
    fn serves_streaming_chat_over_http() {
        let (runtime, port, _usage) = test_runtime(Arc::new(MockExecutor));
        let body = r#"{"model":"gpt2cursor-local","stream":true,"messages":[{"role":"user","content":"hello"}]}"#;
        let response = send_json(port, "/v1/chat/completions", body);
        runtime.stop();
        assert!(response.contains("text/event-stream"));
        assert!(response.contains("data: [DONE]"));
    }

    #[test]
    fn pre_trims_large_incoming_messages_before_codex() {
        let messages: Vec<_> = (0..100)
            .map(|index| json!({"role":"user","content":format!("msg {index}")}))
            .collect();
        let input = json!({"model":"gpt2cursor-local","messages": messages});
        let chat = parse_chat_request(&input, 16).unwrap();
        assert_eq!(chat.messages.len(), 16);
    }

    #[test]
    fn returns_413_for_oversized_body() {
        let (runtime, port, usage) = test_runtime(Arc::new(MockExecutor));
        let request = format!(
            "POST /v1/chat/completions HTTP/1.1\r\nauthorization: Bearer secret\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        let response = send_raw(port, &request);
        runtime.stop();
        assert!(response.contains("HTTP/1.1 413 Payload Too Large") || response.contains("HTTP/1.1 413"));
        assert!(response.contains("Request body too large"));
        assert!(response.contains(ERR_BODY_TOO_LARGE));
        assert_eq!(
            usage.lock().unwrap().last_error.as_deref(),
            Some(USER_BODY_TOO_LARGE_HINT)
        );
    }

    #[test]
    fn returns_400_for_bad_json_over_http() {
        let (runtime, port, _usage) = test_runtime(Arc::new(MockExecutor));
        let response = send_json(port, "/v1/chat/completions", "{");
        runtime.stop();
        assert!(response.contains("HTTP/1.1 400 Bad Request"));
        assert!(response.contains("invalid_request_error"));
    }

    #[test]
    fn returns_502_when_codex_fails_over_http() {
        let (runtime, port, _usage) = test_runtime(Arc::new(FailingExecutor));
        let body = r#"{"model":"gpt2cursor-local","messages":[{"role":"user","content":"hello"}]}"#;
        let response = send_json(port, "/v1/chat/completions", body);
        runtime.stop();
        assert!(response.contains("HTTP/1.1 502 Bad Gateway"));
        assert!(response.contains("upstream_error"));
    }

    fn test_runtime(
        executor: Arc<dyn CodexExecutor>,
    ) -> (BridgeRuntime, u16, Arc<Mutex<UsageSnapshot>>) {
        let listener = TcpListener::bind((HOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let settings = AppSettings {
            port,
            api_key: "secret".to_string(),
            ..AppSettings::default()
        };
        let usage = Arc::new(Mutex::new(UsageSnapshot::default()));
        let runtime = start_bridge(Arc::new(Mutex::new(settings)), Arc::clone(&usage), executor).unwrap();
        thread::sleep(Duration::from_millis(60));
        (runtime, port, usage)
    }

    fn send_json(port: u16, path: &str, body: &str) -> String {
        send_raw(
            port,
            &format!(
                "POST {path} HTTP/1.1\r\nauthorization: Bearer secret\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    fn send_raw(port: u16, raw: &str) -> String {
        let mut stream = TcpStream::connect((HOST, port)).unwrap();
        stream.write_all(raw.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }
}

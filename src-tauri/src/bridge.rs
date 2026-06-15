use crate::{
    codex::{format_prompt, ChatMessage, CodexExecutor, TokenUsage},
    settings::AppSettings,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const HOST: &str = "127.0.0.1";
const MAX_BODY_BYTES: usize = 1_048_576;
pub const CURSOR_MODEL_IDS: &[&str] = &["codex-local", "gpt2cursor-local"];

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UsageSnapshot {
    pub request_count: u64,
    pub active_requests: u64,
    pub last_duration_ms: u64,
    pub total_duration_ms: u64,
    pub last_usage: TokenUsage,
    pub total_usage: TokenUsage,
    pub last_error: Option<String>,
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
    settings: AppSettings,
    usage: Arc<Mutex<UsageSnapshot>>,
    executor: Arc<dyn CodexExecutor>,
) -> Result<BridgeRuntime, String> {
    settings.validate()?;
    let listener = TcpListener::bind((HOST, settings.port))
        .map_err(|err| format!("Port {} is not available: {err}", settings.port))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("Unable to configure listener: {err}"))?;

    let port = settings.port;
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let join = thread::spawn(move || {
        while !thread_stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let request_settings = settings.clone();
                    let request_usage = Arc::clone(&usage);
                    let request_executor = Arc::clone(&executor);
                    thread::spawn(move || {
                        handle_stream(stream, request_settings, request_usage, request_executor);
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(25));
                }
                Err(_) => break,
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
    let request = read_request(stream)?;
    if !is_authorized(&request.headers, &settings.api_key) {
        write_json(
            stream,
            401,
            json!({"error":{"message":"Missing or invalid bearer token","type":"authentication_error"}}),
        )?;
        return Err("authentication failed".to_string());
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/healthz") => write_json(stream, 200, json!({"ok":true})),
        ("GET", "/v1/models") => write_json(stream, 200, models_payload()),
        ("POST", "/v1/chat/completions") => {
            handle_chat(stream, settings, usage, executor, request.body)
        }
        _ => write_json(
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
    let chat = match parse_chat_request(&input) {
        Ok(value) => value,
        Err(message) => {
            write_json(
                stream,
                400,
                json!({"error":{"message":message,"type":"invalid_request_error"}}),
            )?;
            return Err("invalid chat request".to_string());
        }
    };
    let prompt = format_prompt(&chat.messages);
    let codex = match executor.execute(settings, &prompt) {
        Ok(value) => value,
        Err(message) => {
            write_json(
                stream,
                502,
                json!({"error":{"message":"Codex CLI request failed","type":"upstream_error","detail":message}}),
            )?;
            return Err("codex failed".to_string());
        }
    };
    let created = now_epoch_seconds();

    if chat.stream {
        let result = write_sse(
            stream,
            &settings.model,
            &codex.text,
            created,
            codex.usage.clone(),
            codex.duration_ms,
        );
        record_success(&usage, &codex.usage, codex.duration_ms);
        result
    } else {
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
        result
    }
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
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| format!("Unable to set read timeout: {err}"))?;
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 2048];
    let header_end;

    loop {
        let count = stream.read(&mut chunk).map_err(|err| format!("Unable to read request: {err}"))?;
        if count == 0 {
            return Err("Request ended before headers".to_string());
        }
        buffer.extend_from_slice(&chunk[..count]);
        if buffer.len() > MAX_BODY_BYTES {
            return Err("Request body is too large".to_string());
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
        return Err("Request body is too large".to_string());
    }

    let body_start = header_end + 4;
    let mut body = buffer.get(body_start..).unwrap_or_default().to_vec();
    while body.len() < content_length {
        let count = stream.read(&mut chunk).map_err(|err| format!("Unable to read body: {err}"))?;
        if count == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..count]);
    }
    body.truncate(content_length);

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn parse_chat_request(input: &Value) -> Result<ChatRequest, String> {
    let model = input
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("codex-local")
        .to_string();
    let stream = input.get("stream").and_then(Value::as_bool).unwrap_or(false);
    let messages = input
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "messages must include at least one message".to_string())?;
    if messages.is_empty() {
        return Err("messages must include at least one message".to_string());
    }

    let parsed = messages
        .iter()
        .enumerate()
        .map(|(index, message)| parse_message(index, message))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ChatRequest {
        model,
        messages: parsed,
        stream,
    })
}

fn parse_message(index: usize, message: &Value) -> Result<ChatMessage, String> {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("messages[{index}].role must be a string"))?;
    let content = match message.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return Err(format!("messages[{index}].content must include text")),
    };
    if content.trim().is_empty() {
        return Err(format!("messages[{index}].content must include text"));
    }
    Ok(ChatMessage {
        role: role.to_string(),
        content,
    })
}

fn models_payload() -> Value {
    json!({
        "object": "list",
        "data": CURSOR_MODEL_IDS
            .iter()
            .map(|id| {
                json!({
                    "id": id,
                    "object": "model",
                    "created": 0,
                    "owned_by": "local-codex"
                })
            })
            .collect::<Vec<_>>()
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

fn write_sse(
    stream: &mut TcpStream,
    model: &str,
    content: &str,
    created: u64,
    usage: TokenUsage,
    duration_ms: u64,
) -> Result<(), String> {
    let chunk = json!({
        "id": format!("chatcmpl-{created}"),
        "object":"chat.completion.chunk",
        "created": created,
        "model": model,
        "choices":[{
            "index":0,
            "delta":{"role":"assistant","content":content},
            "finish_reason": null
        }],
        "gpt2cursor": {"usage": usage, "duration_ms": duration_ms}
    });
    let done = json!({
        "id": format!("chatcmpl-{created}"),
        "object":"chat.completion.chunk",
        "created": created,
        "model": model,
        "choices":[{"index":0,"delta":{},"finish_reason":"stop"}]
    });
    let body = format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n");
    write_response(stream, 200, "text/event-stream; charset=utf-8", body.as_bytes())
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
        .map_err(|err| format!("Unable to write response: {err}"))
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
        snapshot.last_error = error;
    }
}

pub fn record_success(usage: &Arc<Mutex<UsageSnapshot>>, token_usage: &TokenUsage, duration_ms: u64) {
    if let Ok(mut snapshot) = usage.lock() {
        snapshot.last_duration_ms = duration_ms;
        snapshot.last_usage = token_usage.clone();
        snapshot.total_usage.input_tokens += token_usage.input_tokens;
        snapshot.total_usage.cached_input_tokens += token_usage.cached_input_tokens;
        snapshot.total_usage.output_tokens += token_usage.output_tokens;
        snapshot.total_usage.reasoning_output_tokens += token_usage.reasoning_output_tokens;
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
            "model":"codex-local",
            "stream":true,
            "messages":[{"role":"user","content":[{"type":"text","text":"hello"}]}]
        });
        let chat = parse_chat_request(&input).unwrap();
        assert!(chat.stream);
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
        let runtime = start_bridge(settings, usage, Arc::new(MockExecutor)).unwrap();
        runtime.stop();
    }

    #[test]
    fn serves_models_over_http() {
        let (runtime, port, _usage) = test_runtime(Arc::new(MockExecutor));
        let response = send_raw(port, "GET /v1/models HTTP/1.1\r\nauthorization: Bearer secret\r\n\r\n");
        runtime.stop();
        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("codex-local"));
        assert!(response.contains("gpt2cursor-local"));
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
        let body = r#"{"model":"codex-local","messages":[{"role":"user","content":"hello"}]}"#;
        let response = send_json(port, "/v1/chat/completions", body);
        runtime.stop();
        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("mocked"));
        assert_eq!(usage.lock().unwrap().total_usage.output_tokens, 2);
    }

    #[test]
    fn serves_streaming_chat_over_http() {
        let (runtime, port, _usage) = test_runtime(Arc::new(MockExecutor));
        let body = r#"{"model":"codex-local","stream":true,"messages":[{"role":"user","content":"hello"}]}"#;
        let response = send_json(port, "/v1/chat/completions", body);
        runtime.stop();
        assert!(response.contains("text/event-stream"));
        assert!(response.contains("data: [DONE]"));
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
        let body = r#"{"model":"codex-local","messages":[{"role":"user","content":"hello"}]}"#;
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
        let runtime = start_bridge(settings, Arc::clone(&usage), executor).unwrap();
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

use serde::Serialize;
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpStream,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const DEFAULT_API_PORT: u16 = 4040;
const TUNNEL_WAIT_MS: u64 = 15_000;

#[derive(Clone, Debug, Serialize)]
pub struct TunnelStatus {
    pub installed: bool,
    pub running: bool,
    pub local_url: String,
    pub public_url: Option<String>,
    pub error: Option<String>,
}

pub struct NgrokRuntime {
    child: Child,
    pub public_url: String,
}

pub fn is_ngrok_installed() -> bool {
    Command::new("which")
        .arg("ngrok")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn configure_authtoken(token: &str) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("ngrok authtoken is required".to_string());
    }

    let output = Command::new("ngrok")
        .args(["config", "add-authtoken", token])
        .output()
        .map_err(|err| format!("Unable to run ngrok config: {err}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
        "ngrok config add-authtoken failed".to_string()
    } else {
        stderr
    })
}

pub fn start_tunnel(local_port: u16, authtoken: &str) -> Result<NgrokRuntime, String> {
    if !is_ngrok_installed() {
        return Err("ngrok is not installed. Install it from https://ngrok.com/download".to_string());
    }

    configure_authtoken(authtoken)?;

    let child = Command::new("ngrok")
        .args([
            "http",
            &local_port.to_string(),
            "--log=stdout",
            "--log-format=json",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("Unable to start ngrok: {err}"))?;

    let public_url = wait_for_public_url(DEFAULT_API_PORT, TUNNEL_WAIT_MS)?
        .ok_or_else(|| "Timed out waiting for ngrok public URL".to_string())?;

    Ok(NgrokRuntime { child, public_url })
}

pub fn wait_for_public_url(api_port: u16, timeout_ms: u64) -> Result<Option<String>, String> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(timeout_ms) {
        if let Some(url) = fetch_public_url(api_port)? {
            return Ok(Some(url));
        }
        thread::sleep(Duration::from_millis(250));
    }
    Ok(None)
}

pub fn fetch_public_url(api_port: u16) -> Result<Option<String>, String> {
    let body = http_get(&format!("127.0.0.1:{api_port}"), "/api/tunnels")?;
    Ok(parse_tunnels_response(&body))
}

pub fn parse_tunnels_response(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let tunnels = value.get("tunnels")?.as_array()?;
    let mut fallback = None;

    for tunnel in tunnels {
        let public_url = tunnel.get("public_url")?.as_str()?;
        let openai_url = to_openai_base_url(public_url);
        if public_url.starts_with("https://") {
            return Some(openai_url);
        }
        if fallback.is_none() {
            fallback = Some(openai_url);
        }
    }

    fallback
}

pub fn to_openai_base_url(public_url: &str) -> String {
    format!("{}/v1", public_url.trim_end_matches('/'))
}

pub fn tunnel_status(
    local_port: u16,
    runtime: Option<&NgrokRuntime>,
    error: Option<String>,
) -> TunnelStatus {
    let local_url = format!("http://127.0.0.1:{local_port}/v1");
    TunnelStatus {
        installed: is_ngrok_installed(),
        running: runtime.is_some(),
        local_url,
        public_url: runtime.map(|runtime| runtime.public_url.clone()),
        error,
    }
}

impl NgrokRuntime {
    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn http_get(host_port: &str, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(host_port)
        .map_err(|err| format!("Unable to connect to ngrok API: {err}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|err| format!("Unable to configure ngrok API timeout: {err}"))?;

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("Unable to query ngrok API: {err}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|err| format!("Unable to read ngrok API response: {err}"))?;

    response
        .split("\r\n\r\n")
        .nth(1)
        .map(str::to_string)
        .ok_or_else(|| "ngrok API response was empty".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https_tunnel_url() {
        let body = r#"{"tunnels":[{"public_url":"https://abc.ngrok-free.app","config":{"addr":"http://localhost:8787"}}]}"#;
        assert_eq!(
            parse_tunnels_response(body),
            Some("https://abc.ngrok-free.app/v1".to_string())
        );
    }

    #[test]
    fn prefers_https_over_http_tunnel() {
        let body = r#"{"tunnels":[
            {"public_url":"http://abc.ngrok-free.app"},
            {"public_url":"https://xyz.ngrok-free.app"}
        ]}"#;
        assert_eq!(
            parse_tunnels_response(body),
            Some("https://xyz.ngrok-free.app/v1".to_string())
        );
    }

    #[test]
    fn formats_openai_base_url() {
        assert_eq!(
            to_openai_base_url("https://abc.ngrok-free.app/"),
            "https://abc.ngrok-free.app/v1"
        );
    }
}

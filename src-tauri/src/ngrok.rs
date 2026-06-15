use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const DEFAULT_API_PORT: u16 = 4040;
const TUNNEL_WAIT_MS: u64 = 15_000;

const NGROK_CANDIDATE_PATHS: &[&str] = &[
    "/opt/homebrew/bin/ngrok",
    "/usr/local/bin/ngrok",
    "/opt/local/bin/ngrok",
];

#[derive(Clone, Debug, Serialize)]
pub struct TunnelStatus {
    pub installed: bool,
    pub configured: bool,
    pub running: bool,
    pub local_url: String,
    pub public_url: Option<String>,
    pub error: Option<String>,
}

pub struct NgrokRuntime {
    child: Option<Child>,
    pub public_url: String,
    owned: bool,
}

pub fn resolve_ngrok_binary() -> Option<PathBuf> {
    if Command::new("ngrok")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        return Some(PathBuf::from("ngrok"));
    }

    NGROK_CANDIDATE_PATHS
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .map(|path| path.to_path_buf())
}

pub fn is_ngrok_installed() -> bool {
    resolve_ngrok_binary().is_some()
}

pub fn ngrok_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join("Library/Application Support/ngrok/ngrok.yml"));
        paths.push(home.join(".config/ngrok/ngrok.yml"));
        paths.push(home.join(".ngrok2/ngrok.yml"));
    }
    paths
}

pub fn parse_authtoken_from_config(content: &str) -> Option<String> {
    let mut in_agent = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "agent:" {
            in_agent = true;
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            in_agent = false;
        }
        let key = if in_agent {
            trimmed.strip_prefix("authtoken:")?
        } else if let Some(rest) = trimmed.strip_prefix("authtoken:") {
            rest
        } else {
            continue;
        };
        let token = key.trim().trim_matches('"').trim_matches('\'');
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }
    None
}

pub fn read_configured_authtoken() -> Option<String> {
    ngrok_config_paths()
        .iter()
        .find_map(|path| fs::read_to_string(path).ok().and_then(|raw| parse_authtoken_from_config(&raw)))
}

pub fn has_configured_authtoken() -> bool {
    read_configured_authtoken().is_some()
}

pub fn is_ngrok_ready(authtoken: &str) -> bool {
    !authtoken.trim().is_empty() || has_configured_authtoken()
}

pub fn configure_authtoken(token: &str) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("ngrok authtoken is required".to_string());
    }

    let binary = resolve_ngrok_binary().ok_or_else(|| {
        "ngrok is not installed. Install it from https://ngrok.com/download".to_string()
    })?;

    let output = Command::new(&binary)
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
    if let Ok(Some(public_url)) = fetch_tunnel_for_port(DEFAULT_API_PORT, local_port) {
        return Ok(NgrokRuntime {
            child: None,
            public_url,
            owned: false,
        });
    }

    let binary = resolve_ngrok_binary().ok_or_else(|| {
        "ngrok is not installed. Install it from https://ngrok.com/download".to_string()
    })?;

    let token = authtoken.trim();
    if !token.is_empty() {
        configure_authtoken(token)?;
    } else if !has_configured_authtoken() {
        return Err(
            "ngrok authtoken is not configured. Log in with ngrok or paste your authtoken."
                .to_string(),
        );
    }

    let mut command = Command::new(&binary);
    command
        .args([
            "http",
            &local_port.to_string(),
            "--log=stdout",
            "--log-format=json",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    let child = command
        .spawn()
        .map_err(|err| format!("Unable to start ngrok: {err}"))?;

    let public_url = wait_for_tunnel_url(DEFAULT_API_PORT, local_port, TUNNEL_WAIT_MS)?
        .ok_or_else(|| "Timed out waiting for ngrok public URL".to_string())?;

    Ok(NgrokRuntime {
        child: Some(child),
        public_url,
        owned: true,
    })
}

pub fn wait_for_tunnel_url(
    api_port: u16,
    local_port: u16,
    timeout_ms: u64,
) -> Result<Option<String>, String> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(timeout_ms) {
        if let Some(url) = fetch_tunnel_for_port(api_port, local_port)? {
            return Ok(Some(url));
        }
        thread::sleep(Duration::from_millis(250));
    }
    Ok(None)
}

pub fn fetch_tunnel_for_port(api_port: u16, local_port: u16) -> Result<Option<String>, String> {
    let body = match http_get(&format!("127.0.0.1:{api_port}"), "/api/tunnels") {
        Ok(body) => body,
        Err(_) => return Ok(None),
    };
    Ok(parse_tunnel_for_port(&body, local_port))
}

pub fn parse_tunnel_for_port(body: &str, local_port: u16) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let tunnels = value.get("tunnels")?.as_array()?;
    let mut fallback = None;

    for tunnel in tunnels {
        if !tunnel_matches_port(tunnel, local_port) {
            continue;
        }
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

fn tunnel_matches_port(tunnel: &Value, local_port: u16) -> bool {
    let addr = tunnel
        .get("config")
        .and_then(|config| config.get("addr"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if addr.is_empty() {
        return false;
    }

    addr.rsplit(':')
        .next()
        .and_then(|port| port.parse::<u16>().ok())
        .is_some_and(|port| port == local_port)
}

pub fn to_openai_base_url(public_url: &str) -> String {
    format!("{}/v1", public_url.trim_end_matches('/'))
}

pub fn tunnel_status(
    local_port: u16,
    runtime: Option<&NgrokRuntime>,
    authtoken: &str,
    error: Option<String>,
) -> TunnelStatus {
    let local_url = format!("http://127.0.0.1:{local_port}/v1");
    TunnelStatus {
        installed: is_ngrok_installed(),
        configured: is_ngrok_ready(authtoken),
        running: runtime.is_some(),
        local_url,
        public_url: runtime.map(|runtime| runtime.public_url.clone()),
        error,
    }
}

impl NgrokRuntime {
    pub fn refresh(&mut self, local_port: u16) -> Result<(), String> {
        if let Ok(Some(url)) = fetch_tunnel_for_port(DEFAULT_API_PORT, local_port) {
            self.public_url = url;
        }

        if self.owned {
            if let Some(ref mut child) = self.child {
                if child
                    .try_wait()
                    .map_err(|err| format!("Unable to check ngrok process: {err}"))?
                    .is_some()
                {
                    return Err("ngrok tunnel exited unexpectedly".to_string());
                }
            }
            return Ok(());
        }

        if self.public_url.is_empty() {
            return Err("ngrok tunnel is no longer available".to_string());
        }

        Ok(())
    }

    pub fn stop(mut self) {
        if !self.owned {
            return;
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
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
    fn parses_top_level_authtoken() {
        let content = "version: \"3\"\nauthtoken: test_token_123\n";
        assert_eq!(
            parse_authtoken_from_config(content),
            Some("test_token_123".to_string())
        );
    }

    #[test]
    fn parses_agent_authtoken() {
        let content = "version: \"3\"\nagent:\n  authtoken: nested_token\n";
        assert_eq!(
            parse_authtoken_from_config(content),
            Some("nested_token".to_string())
        );
    }

    #[test]
    fn parses_https_tunnel_url_for_port() {
        let body = r#"{"tunnels":[{"public_url":"https://abc.ngrok-free.app","config":{"addr":"http://localhost:8787"}}]}"#;
        assert_eq!(
            parse_tunnel_for_port(body, 8787),
            Some("https://abc.ngrok-free.app/v1".to_string())
        );
    }

    #[test]
    fn ignores_tunnel_for_other_port() {
        let body = r#"{"tunnels":[{"public_url":"https://abc.ngrok-free.app","config":{"addr":"http://localhost:8787"}}]}"#;
        assert_eq!(parse_tunnel_for_port(body, 3000), None);
    }

    #[test]
    fn prefers_https_over_http_tunnel() {
        let body = r#"{"tunnels":[
            {"public_url":"http://abc.ngrok-free.app","config":{"addr":"http://localhost:8787"}},
            {"public_url":"https://xyz.ngrok-free.app","config":{"addr":"http://127.0.0.1:8787"}}
        ]}"#;
        assert_eq!(
            parse_tunnel_for_port(body, 8787),
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

    #[test]
    fn ngrok_ready_with_inline_token() {
        assert!(is_ngrok_ready("token"));
    }
}

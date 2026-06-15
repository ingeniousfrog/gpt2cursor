use std::{env, fs, path::PathBuf};

const LAUNCH_AGENT_ID: &str = "com.gpt2cursor.app";

#[cfg(target_os = "macos")]
pub fn set_launch_at_login(enabled: bool) -> Result<(), String> {
    let path = launch_agent_path()?;
    if enabled {
        let exe = env::current_exe()
            .map_err(|err| format!("Unable to locate current executable: {err}"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("Unable to create LaunchAgents directory: {err}"))?;
        }
        fs::write(&path, launch_agent_plist(&exe))
            .map_err(|err| format!("Unable to save launch agent: {err}"))?;
    } else if path.exists() {
        fs::remove_file(&path).map_err(|err| format!("Unable to remove launch agent: {err}"))?;
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_launch_at_login(_enabled: bool) -> Result<(), String> {
    Err("Launch at login is only implemented for macOS".to_string())
}

#[cfg(target_os = "macos")]
pub fn is_launch_at_login_enabled() -> bool {
    launch_agent_path().map(|path| path.exists()).unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub fn is_launch_at_login_enabled() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn launch_agent_path() -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCH_AGENT_ID}.plist")))
}

#[cfg(target_os = "macos")]
fn launch_agent_plist(exe: &PathBuf) -> String {
    let exe = escape_xml(&exe.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LAUNCH_AGENT_ID}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#
    )
}

#[cfg(target_os = "macos")]
fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

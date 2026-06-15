#![allow(deprecated, unexpected_cfgs)]

mod bridge;
mod codex;
mod launch;
mod settings;
mod tray_icon;

use bridge::{
    base_url, is_port_available, start_bridge as start_bridge_server, BridgeRuntime, BridgeStatus,
    UsageSnapshot,
};
use codex::{resolve_codex_command, RealCodexExecutor};
use serde::{Deserialize, Serialize};
use settings::{load_settings, save_settings as persist_settings, settings_path, AppSettings};
use std::{
    fs::File,
    io::Read,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, PhysicalPosition, WindowEvent, Wry};

struct ManagedState {
    settings: Mutex<AppSettings>,
    bridge: Mutex<Option<BridgeRuntime>>,
    usage: Arc<Mutex<UsageSnapshot>>,
}

#[derive(Clone, Debug, Serialize)]
struct AppViewState {
    settings: AppSettings,
    bridge: BridgeStatus,
    codex: CodexAccountStatus,
}

#[derive(Clone, Debug, Serialize)]
struct PortValidation {
    port: u16,
    available: bool,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
struct CodexAccountStatus {
    available: bool,
    summary: String,
    detail: String,
}

#[derive(Clone, Debug, Deserialize)]
struct SaveSettingsInput {
    settings: AppSettings,
}

const PANEL_CORNER_RADIUS: f64 = 26.0;

#[cfg(target_os = "macos")]
fn round_macos_window(window: &tauri::WebviewWindow<Wry>) {
    use cocoa::appkit::NSColor;
    use cocoa::base::{id, nil, NO, YES};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    use tauri::utils::config::WindowEffectsConfig;
    use tauri::window::{Effect, EffectState};

    let _ = window.set_effects(Some(WindowEffectsConfig {
        effects: vec![Effect::WindowBackground],
        state: Some(EffectState::Active),
        radius: Some(PANEL_CORNER_RADIUS),
        color: None,
    }));

    let Ok(ns_window_ptr) = window.ns_window() else {
        return;
    };
    let ns_window = ns_window_ptr as id;

    unsafe {
        let _: () = msg_send![ns_window, setOpaque: NO];
        let _: () = msg_send![ns_window, setBackgroundColor: NSColor::clearColor(nil)];
        let _: () = msg_send![ns_window, setHasShadow: YES];

        let content_view: id = msg_send![ns_window, contentView];
        round_macos_view_tree(content_view, PANEL_CORNER_RADIUS);
    }

    let _ = window.with_webview(move |platform_webview| {
        let webview = platform_webview.inner() as id;
        if webview.is_null() {
            return;
        }
        unsafe {
            let _: () = msg_send![webview, setOpaque: NO];
            let no: id = msg_send![class!(NSNumber), numberWithBool:0];
            let draws_key = NSString::alloc(nil).init_str("drawsBackground");
            let _: () = msg_send![webview, setValue:no forKey:draws_key];
            round_macos_view_tree(webview, PANEL_CORNER_RADIUS);
        }
    });
}

#[cfg(target_os = "macos")]
unsafe fn round_macos_view_tree(view: cocoa::base::id, radius: f64) {
    use cocoa::appkit::NSColor;
    use cocoa::base::{id, nil, YES};
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }

    let _: () = msg_send![view, setWantsLayer: YES];
    let layer: id = msg_send![view, layer];
    if layer != nil {
        let _: () = msg_send![layer, setCornerRadius: radius];
        let _: () = msg_send![layer, setMasksToBounds: YES];
        let clear: id = msg_send![NSColor::clearColor(nil), CGColor];
        let _: () = msg_send![layer, setBackgroundColor: clear];
    }

    let subviews: id = msg_send![view, subviews];
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        round_macos_view_tree(child, radius);
    }
}

#[tauri::command]
fn get_app_state(app: AppHandle, state: tauri::State<ManagedState>) -> Result<AppViewState, String> {
    app_state(&app, &state)
}

#[tauri::command]
fn validate_port(port: u16, state: tauri::State<ManagedState>) -> PortValidation {
    if settings::validate_port(port).is_err() {
        return PortValidation {
            port,
            available: false,
            message: "Port must be between 1 and 65535".to_string(),
        };
    }

    if let Ok(bridge) = state.bridge.lock() {
        if bridge.as_ref().map(|runtime| runtime.port) == Some(port) {
            return PortValidation {
                port,
                available: true,
                message: "Port is used by the running bridge".to_string(),
            };
        }
    }

    let available = is_port_available(port);
    PortValidation {
        port,
        available,
        message: if available {
            "Port is available".to_string()
        } else {
            "Port is already in use".to_string()
        },
    }
}

#[tauri::command]
fn generate_api_key() -> Result<String, String> {
    let mut bytes = [0_u8; 24];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|err| format!("Unable to generate secure API key: {err}"))?;
    Ok(format!("g2c_{}", to_hex(&bytes)))
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: tauri::State<ManagedState>,
    input: SaveSettingsInput,
) -> Result<AppViewState, String> {
    input.settings.validate()?;
    let mut next = input.settings;
    next.launch_at_login = launch::is_launch_at_login_enabled();
    persist_settings(&settings_path(&app)?, &next)?;
    *state
        .settings
        .lock()
        .map_err(|_| "Settings state is unavailable".to_string())? = next;
    app_state(&app, &state)
}

#[tauri::command]
fn start_bridge(
    app: AppHandle,
    state: tauri::State<ManagedState>,
) -> Result<AppViewState, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "Settings state is unavailable".to_string())?
        .clone();
    settings.validate()?;

    {
        let bridge = state
            .bridge
            .lock()
            .map_err(|_| "Bridge state is unavailable".to_string())?;
        if bridge.is_some() {
            return app_state(&app, &state);
        }
    }

    if !is_port_available(settings.port) {
        return Err(format!("Port {} is already in use", settings.port));
    }

    let runtime = start_bridge_server(
        settings,
        Arc::clone(&state.usage),
        Arc::new(RealCodexExecutor),
    )?;
    *state
        .bridge
        .lock()
        .map_err(|_| "Bridge state is unavailable".to_string())? = Some(runtime);
    app_state(&app, &state)
}

#[tauri::command]
fn stop_bridge(
    app: AppHandle,
    state: tauri::State<ManagedState>,
) -> Result<AppViewState, String> {
    let runtime = state
        .bridge
        .lock()
        .map_err(|_| "Bridge state is unavailable".to_string())?
        .take();
    if let Some(runtime) = runtime {
        runtime.stop();
    }
    app_state(&app, &state)
}

#[tauri::command]
fn set_launch_at_login(
    app: AppHandle,
    state: tauri::State<ManagedState>,
    enabled: bool,
) -> Result<AppViewState, String> {
    launch::set_launch_at_login(enabled)?;
    {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Settings state is unavailable".to_string())?;
        settings.launch_at_login = enabled;
        persist_settings(&settings_path(&app)?, &settings)?;
    }
    app_state(&app, &state)
}

#[tauri::command]
fn refresh_codex_status() -> CodexAccountStatus {
    codex_status()
}

#[tauri::command]
fn quit_app(app: AppHandle<Wry>, state: tauri::State<ManagedState>) {
    if let Ok(mut bridge) = state.bridge.lock() {
        if let Some(runtime) = bridge.take() {
            runtime.stop();
        }
    }
    app.exit(0);
}

fn app_state(app: &AppHandle, state: &tauri::State<ManagedState>) -> Result<AppViewState, String> {
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "Settings state is unavailable".to_string())?
        .clone();
    settings.launch_at_login = launch::is_launch_at_login_enabled();
    let bridge = state
        .bridge
        .lock()
        .map_err(|_| "Bridge state is unavailable".to_string())?;
    let running = bridge.is_some();
    let port = bridge.as_ref().map(|runtime| runtime.port).unwrap_or(settings.port);
    let usage = state
        .usage
        .lock()
        .map_err(|_| "Usage state is unavailable".to_string())?
        .clone();

    let _ = app;
    Ok(AppViewState {
        settings,
        bridge: BridgeStatus {
            running,
            port,
            base_url: base_url(port),
            usage,
        },
        codex: CodexAccountStatus {
            available: false,
            summary: "Codex account quota not refreshed".to_string(),
            detail: "Use Refresh to check local Codex CLI status. Per-session usage updates automatically.".to_string(),
        },
    })
}

fn codex_status() -> CodexAccountStatus {
    match run_codex_doctor_summary() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{stdout}{stderr}");
            CodexAccountStatus {
                available: false,
                summary: if combined.contains("auth is configured") || combined.contains("✓ auth") {
                    "Codex CLI is authenticated; account quota is not exposed by this CLI.".to_string()
                } else {
                    "Codex CLI detected; account quota is unavailable.".to_string()
                },
                detail: "gpt2cursor shows reliable per-session token usage. Subscription or account quota is not exposed through a stable local Codex CLI interface.".to_string(),
            }
        }
        Err(err) => CodexAccountStatus {
            available: false,
            summary: "Codex CLI is unavailable".to_string(),
            detail: format!("Unable to run codex doctor: {err}"),
        },
    }
}

fn run_codex_doctor_summary() -> Result<std::process::Output, String> {
    let mut child = Command::new(resolve_codex_command("codex"))
        .arg("doctor")
        .arg("--summary")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Unable to run codex doctor: {err}"))?;
    let started = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|err| format!("Unable to read codex doctor output: {err}"));
            }
            Ok(None) if started.elapsed() > Duration::from_secs(15) => {
                let _ = child.kill();
                return Err("codex doctor timed out".to_string());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(err) => return Err(format!("Unable to wait for codex doctor: {err}")),
        }
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let settings_file = settings_path(app.handle())?;
            let mut settings = load_settings(&settings_file);
            settings.launch_at_login = launch::is_launch_at_login_enabled();
            app.manage(ManagedState {
                settings: Mutex::new(settings),
                bridge: Mutex::new(None),
                usage: Arc::new(Mutex::new(UsageSnapshot::default())),
            });

            let icon = tray_icon::load_tray_icon()?;
            TrayIconBuilder::with_id("gpt2cursor")
                .icon(icon)
                .tooltip("gpt2cursor")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        toggle_panel(tray.app_handle(), position);
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                round_macos_window(&window);
                let panel = window.clone();
                window.on_window_event(move |event| {
                    if matches!(event, WindowEvent::Focused(false)) {
                        let _ = panel.hide();
                    }
                });
                let _ = window.hide();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            validate_port,
            generate_api_key,
            save_settings,
            start_bridge,
            stop_bridge,
            set_launch_at_login,
            refresh_codex_status,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("failed to run gpt2cursor");
}

fn toggle_panel(app: &AppHandle<Wry>, position: PhysicalPosition<f64>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    let panel_width = 440.0;
    let panel_height = 640.0;
    let mut x = (position.x - panel_width / 2.0).max(8.0);
    let mut y = (position.y + 10.0).max(8.0);

    if let Ok(Some(monitor)) = window.current_monitor() {
        let monitor_origin = monitor.position();
        let monitor_size = monitor.size();
        let max_x = monitor_origin.x as f64 + monitor_size.width as f64 - panel_width - 8.0;
        let max_y = monitor_origin.y as f64 + monitor_size.height as f64 - panel_height - 8.0;
        x = x.min(max_x.max(8.0));
        y = y.min(max_y.max(8.0));
    }

    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.show();
    let _ = window.set_focus();
}

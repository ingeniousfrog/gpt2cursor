import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  CheckCircle2,
  ChevronDown,
  CircleHelp,
  Clipboard,
  Eye,
  EyeOff,
  Globe,
  KeyRound,
  Loader2,
  LogOut,
  PlugZap,
  Power,
  RefreshCw,
  Shuffle,
  SlidersHorizontal,
  ToggleLeft,
  ToggleRight,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import appIcon from "../src-tauri/icons/icon.png";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
    __TAURI__?: unknown;
  }
}

const CURSOR_MODEL = "gpt2cursor-local";

type TokenUsage = {
  input_tokens: number;
  cached_input_tokens: number;
  output_tokens: number;
  reasoning_output_tokens: number;
};

type UsageSnapshot = {
  request_count: number;
  active_requests: number;
  last_duration_ms: number;
  total_duration_ms: number;
  last_usage: TokenUsage;
  total_usage: TokenUsage;
  last_error: string | null;
  recent_logs: string[];
};

type AppSettings = {
  port: number;
  api_key: string;
  model: string;
  codex_command: string;
  codex_model: string;
  codex_profile: string;
  codex_sandbox: string;
  codex_approval: string;
  codex_timeout_ms: number;
  codex_max_messages: number;
  launch_at_login: boolean;
  ngrok_enabled: boolean;
  ngrok_authtoken: string;
};

type TunnelStatus = {
  installed: boolean;
  configured: boolean;
  running: boolean;
  local_url: string;
  public_url: string | null;
  error: string | null;
};

type BridgeStatus = {
  running: boolean;
  port: number;
  base_url: string;
  usage: UsageSnapshot;
};

type CodexModelOption = {
  id: string;
  label: string;
};

type CodexStatus = {
  cli_installed: boolean;
  authenticated: boolean;
  summary: string;
  detail: string;
  checked_at_ms: number;
};

type AppViewState = {
  settings: AppSettings;
  bridge: BridgeStatus;
  tunnel: TunnelStatus;
  codex: CodexStatus;
};

type PortValidation = {
  port: number;
  available: boolean;
  message: string;
};

const defaultUsage: TokenUsage = {
  input_tokens: 0,
  cached_input_tokens: 0,
  output_tokens: 0,
  reasoning_output_tokens: 0,
};

const defaultCodexModelOptions: Array<{ value: string; label: string }> = [
  { value: "gpt-5.5", label: "GPT-5.5" },
];

const profileOptions = [
  { value: "", label: "Default profile" },
  { value: "work", label: "work" },
  { value: "personal", label: "personal" },
];

let mockSettings: AppSettings | null = null;
let mockBridgeRunning = false;
let tauriInvokeAvailable: boolean | null = null;

const isTauri = () =>
  typeof window !== "undefined"
  && Boolean(window.__TAURI_INTERNALS__ || window.__TAURI__);

async function probeTauriInvoke(): Promise<boolean> {
  if (tauriInvokeAvailable !== null) {
    return tauriInvokeAvailable;
  }
  if (isTauri()) {
    tauriInvokeAvailable = true;
    return true;
  }
  try {
    await invoke("get_app_state");
    tauriInvokeAvailable = true;
    return true;
  } catch {
    tauriInvokeAvailable = false;
    return false;
  }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const canInvoke = await probeTauriInvoke();
  if (canInvoke) {
    return invoke<T>(command, args);
  }
  if (import.meta.env.DEV) {
    return mockCommand<T>(command, args);
  }
  throw new Error("Tauri IPC is not available");
}

async function mockCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const defaults: AppSettings = {
    port: 8787,
    api_key: "g2c_preview_5db6f88baf29d2c8",
    model: CURSOR_MODEL,
    codex_command: "codex",
    codex_model: "gpt-5.5",
    codex_profile: "",
    codex_sandbox: "read-only",
    codex_approval: "never",
    codex_timeout_ms: 300000,
    codex_max_messages: 12,
    launch_at_login: false,
    ngrok_enabled: false,
    ngrok_authtoken: "",
  };
  const settings = mockSettings ?? defaults;

  if (command === "save_settings" && args?.input && typeof args.input === "object") {
    const input = args.input as { settings?: AppSettings };
    mockSettings = input.settings ?? settings;
  }

  const effective = mockSettings ?? settings;
  const running = command === "start_bridge" || (command === "get_app_state" && mockBridgeRunning);
  if (command === "start_bridge") {
    mockBridgeRunning = true;
  }
  if (command === "stop_bridge") {
    mockBridgeRunning = false;
  }

  const state: AppViewState = {
    settings: effective,
    bridge: {
      running: command === "get_app_state" ? mockBridgeRunning : running,
      port: effective.port,
      base_url: `http://127.0.0.1:${effective.port}/v1`,
      usage: {
        request_count: running ? 18 : 0,
        active_requests: running ? 1 : 0,
        last_duration_ms: running ? 1840 : 0,
        total_duration_ms: running ? 42100 : 0,
        last_usage: { input_tokens: 812, cached_input_tokens: 128, output_tokens: 244, reasoning_output_tokens: 36 },
        total_usage: { input_tokens: 8840, cached_input_tokens: 1600, output_tokens: 2301, reasoning_output_tokens: 412 },
        last_error: null,
        recent_logs: running ? ["12:01:02 bridge started", "12:01:18 POST /v1/chat/completions (chat)"] : [],
      },
    },
    tunnel: {
      installed: true,
      configured: true,
      running: effective.ngrok_enabled && (command === "get_app_state" ? mockBridgeRunning : running),
      local_url: `http://127.0.0.1:${effective.port}/v1`,
      public_url:
        effective.ngrok_enabled && (command === "get_app_state" ? mockBridgeRunning : running)
          ? "https://preview.ngrok-free.app/v1"
          : null,
      error: null,
    },
    codex: {
      cli_installed: true,
      authenticated: true,
      summary: "Codex CLI is authenticated",
      detail: "Browser preview only. Run npm run tauri for real ngrok.",
      checked_at_ms: Date.now(),
    },
  };

  if (command === "validate_port") {
    const port = Number(args?.port ?? 0);
    return { port, available: port > 0, message: port > 0 ? "Port is available" : "Port must be between 1 and 65535" } as T;
  }
  if (command === "generate_api_key") {
    return "g2c_preview_6f7a3d88e16c4baf9120" as T;
  }
  if (command === "list_codex_model_options") {
    return [
      { id: "gpt-5.5", label: "GPT-5.5" },
      { id: "gpt-5.4", label: "GPT-5.4" },
      { id: "gpt-5.4-mini", label: "GPT-5.4-Mini" },
    ] as T;
  }
  if (command === "refresh_codex_status") {
    return {
      cli_installed: true,
      authenticated: true,
      summary: "Codex CLI is authenticated",
      detail: "Browser preview only.",
      checked_at_ms: Date.now(),
    } as T;
  }
  if (command === "save_settings") {
    return state as T;
  }
  return state as T;
}

function totalTokens(usage: TokenUsage) {
  return usage.input_tokens + usage.output_tokens + usage.reasoning_output_tokens;
}

function formatDuration(ms: number) {
  if (ms <= 0) return "0 ms";
  if (ms < 1000) return `${ms} ms`;
  return `${(ms / 1000).toFixed(ms > 10_000 ? 0 : 1)} s`;
}

function formatCheckedAt(ms: number) {
  if (!ms) return "Not checked yet";
  return new Date(ms).toLocaleTimeString();
}

function shortKey(key: string) {
  if (!key) return "Not set";
  return `${key.slice(0, 7)}...${key.slice(-5)}`;
}

function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  return typeof error === "string" ? error : "Operation failed";
}

export default function App() {
  const [state, setState] = useState<AppViewState | null>(null);
  const [draft, setDraft] = useState<AppSettings | null>(null);
  const [portValidation, setPortValidation] = useState<PortValidation | null>(null);
  const [apiKeyVisible, setApiKeyVisible] = useState(false);
  const [ngrokTokenVisible, setNgrokTokenVisible] = useState(false);
  const [ngrokTokenOverride, setNgrokTokenOverride] = useState(false);
  const [setupOpen, setSetupOpen] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [activityOpen, setActivityOpen] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [codexRefreshing, setCodexRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [codex, setCodex] = useState<CodexStatus | null>(null);
  const [codexModelOptions, setCodexModelOptions] = useState(defaultCodexModelOptions);

  const applyCodexModels = useCallback((models: CodexModelOption[], currentModel?: string) => {
    const options = models.map((model) => ({ value: model.id, label: model.label }));
    setCodexModelOptions(options);
    const ids = new Set(models.map((model) => model.id));
    const latest = models[0]?.id ?? "";
    const nextModel = currentModel && ids.has(currentModel) ? currentModel : latest;
    if (nextModel) {
      setDraft((current) => current ? { ...current, codex_model: nextModel } : current);
    }
    return nextModel;
  }, []);

  const refreshCodexModels = useCallback(async (currentModel?: string) => {
    const models = await call<CodexModelOption[]>("list_codex_model_options");
    return applyCodexModels(models, currentModel);
  }, [applyCodexModels]);

  const loadState = useCallback(async () => {
    const next = await call<AppViewState>("get_app_state");
    setState((prev) => {
      if (prev?.bridge.running && !next.bridge.running && next.tunnel.error) {
        setError(next.tunnel.error);
      } else if (next.bridge.running && next.tunnel.error) {
        setError(next.tunnel.error);
      }
      return next;
    });
    setDraft((current) => {
      if (!current) return next.settings;
      if (next.bridge.running) {
        return { ...current, ngrok_enabled: next.settings.ngrok_enabled };
      }
      return current;
    });
  }, []);

  useEffect(() => {
    void loadState().catch((err) => setError(errorMessage(err)));
  }, [loadState]);

  useEffect(() => {
    if (!state?.bridge.running) return undefined;
    const timer = window.setInterval(() => {
      void loadState().catch((err) => setError(errorMessage(err)));
    }, 3000);
    return () => window.clearInterval(timer);
  }, [loadState, state?.bridge.running]);

  useEffect(() => {
    if (!draft?.port) return undefined;
    const timer = window.setTimeout(() => {
      call<PortValidation>("validate_port", { port: Number(draft.port) })
        .then(setPortValidation)
        .catch((err) => setError(errorMessage(err)));
    }, 250);
    return () => window.clearTimeout(timer);
  }, [draft?.port]);

  const bridge = state?.bridge;
  const tunnel = state?.tunnel;
  const settings = draft;
  const running = Boolean(bridge?.running);
  const tunnelEnabled = running
    ? (state?.settings.ngrok_enabled ?? settings?.ngrok_enabled ?? false)
    : (settings?.ngrok_enabled ?? false);
  const usage = bridge?.usage;
  const canStart = Boolean(
    settings?.api_key
    && portValidation?.available !== false
    && (!settings.ngrok_enabled || settings.ngrok_authtoken.trim() || tunnel?.configured),
  );
  const localBaseUrl = bridge?.base_url ?? tunnel?.local_url ?? `http://127.0.0.1:${settings?.port ?? 8787}/v1`;
  const cursorBaseUrl = tunnel?.public_url ?? localBaseUrl;

  const saveDraft = useCallback(async () => {
    if (!settings) return;
    setBusy("save");
    setError(null);
    try {
      const next = await call<AppViewState>("save_settings", { input: { settings } });
      setState(next);
      setDraft(next.settings);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [settings]);

  const refreshCodex = useCallback(async () => {
    if (codexRefreshing) return;
    setCodexRefreshing(true);
    try {
      setCodex(await call<CodexStatus>("refresh_codex_status"));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setCodexRefreshing(false);
    }
  }, [codexRefreshing]);

  const start = useCallback(async () => {
    if (!settings) return;
    setBusy("start");
    setError(null);
    try {
      const nextModel = await refreshCodexModels(settings.codex_model);
      const effectiveSettings = {
        ...settings,
        codex_model: nextModel || settings.codex_model || codexModelOptions[0]?.value || "gpt-5.5",
      };
      await call<AppViewState>("save_settings", { input: { settings: effectiveSettings } });
      const next = await call<AppViewState>("start_bridge");
      setState(next);
      setDraft(next.settings);
      setSetupOpen(true);
      void refreshCodex();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [refreshCodex, refreshCodexModels, settings, codexModelOptions]);

  const stop = useCallback(async () => {
    setBusy("stop");
    setError(null);
    setActivityOpen(false);
    try {
      const next = await call<AppViewState>("stop_bridge");
      setState(next);
      setDraft(next.settings);
      setCodex(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  }, []);

  const generateKey = useCallback(async () => {
    setBusy("key");
    setError(null);
    try {
      const apiKey = await call<string>("generate_api_key");
      setDraft((current) => current ? { ...current, api_key: apiKey } : current);
      setApiKeyVisible(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  }, []);

  const toggleLaunch = useCallback(async () => {
    if (!settings) return;
    setBusy("launch");
    setError(null);
    try {
      const next = await call<AppViewState>("set_launch_at_login", {
        enabled: !settings.launch_at_login,
      });
      setState(next);
      setDraft(next.settings);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [settings]);

  const copy = useCallback(async (label: string, value: string) => {
    await navigator.clipboard.writeText(value);
    setCopied(label);
    window.setTimeout(() => setCopied(null), 1300);
  }, []);

  const updateDraft = useCallback(<K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    setDraft((current) => current ? { ...current, [key]: value } : current);
  }, []);

  const usageCards = useMemo(() => {
    const snapshot = usage ?? {
      request_count: 0,
      active_requests: 0,
      last_duration_ms: 0,
      total_duration_ms: 0,
      last_usage: defaultUsage,
      total_usage: defaultUsage,
      last_error: null,
      recent_logs: [],
    };
    return [
      ["Requests", snapshot.request_count.toString()],
      ["Active", snapshot.active_requests.toString()],
      ["Last", formatDuration(snapshot.last_duration_ms)],
      ["Tokens", totalTokens(snapshot.total_usage).toString()],
    ];
  }, [usage]);

  const sessionBars = useMemo(() => {
    const snapshot = usage?.total_usage ?? defaultUsage;
    const total = Math.max(totalTokens(snapshot), 1);
    return [
      { label: "Input", value: snapshot.input_tokens, color: "bg-sky-400" },
      { label: "Cached", value: snapshot.cached_input_tokens, color: "bg-violet-400" },
      { label: "Output", value: snapshot.output_tokens, color: "bg-emerald-400" },
      { label: "Reasoning", value: snapshot.reasoning_output_tokens, color: "bg-amber-400" },
    ].map((item) => ({
      ...item,
      pct: Math.min(100, Math.round((item.value / total) * 100)),
    }));
  }, [usage]);

  if (!settings || !state) {
    return (
      <main className="flex h-full items-center justify-center rounded-[26px] bg-panel text-sky-500">
        <Loader2 className="h-6 w-6 animate-spin" />
      </main>
    );
  }

  const startDisabled = busy === "start" || busy === "stop" || (!running && !canStart);
  const activityLogs = usage?.recent_logs ?? [];
  const hasActivity = activityLogs.length > 0 || Boolean(usage?.last_error);

  return (
    <main className="relative h-full overflow-hidden rounded-[26px] bg-panel text-slate-800 shadow-panel">
      <div className="pointer-events-none absolute inset-0 bg-mesh opacity-35" />
      <div className="panel-scroll panel-shell">
        <section className={`hero-card ${activityOpen ? "hero-card-raised" : ""}`}>
          <div className="flex min-w-0 items-center gap-2.5">
            <div className="logo-shell">
              <img src={appIcon} alt="" className="logo-image" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="label-accent">gpt2cursor</div>
              <div className="mt-1 flex items-center gap-2">
                <span className={running ? "status-pill-live" : "status-pill-idle"}>
                  <span className={`h-1.5 w-1.5 rounded-full ${running ? "bg-emerald-500" : "bg-slate-300"}`} />
                  {running ? "Live" : "Idle"}
                </span>
                <span className="truncate text-[15px] font-bold tracking-tight text-slate-900">
                  {running ? `Port ${bridge?.port}` : "Local bridge"}
                </span>
              </div>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {running && (
              <button
                type="button"
                className={`icon-btn relative h-10 w-10 ${activityOpen ? "border-sky-300/80 bg-sky-50 text-sky-600" : ""}`}
                onClick={() => setActivityOpen((value) => !value)}
                title="Request activity"
                aria-label="Request activity"
                aria-expanded={activityOpen}
              >
                <Activity className="h-4 w-4" />
                {hasActivity && !activityOpen && <span className="activity-dot" />}
              </button>
            )}
            <button
              className={running ? "stop-btn" : "primary-btn"}
              disabled={startDisabled}
              onClick={running ? stop : start}
            >
              {busy === "start" || busy === "stop" ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Power className="h-3.5 w-3.5" />}
              {running ? "Stop" : "Start"}
            </button>
          </div>
        </section>

        <section className="url-card">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span className="label">Cursor Base URL</span>
              <button
                className="inline-flex h-5 w-5 items-center justify-center rounded-full text-slate-400 transition hover:bg-sky-100 hover:text-sky-600"
                onClick={() => setSetupOpen(true)}
                title="Cursor setup guide"
                aria-label="Cursor setup guide"
              >
                <CircleHelp className="h-3 w-3" />
              </button>
            </div>
            <div className="url-value">{cursorBaseUrl}</div>
            {tunnel?.public_url && (
              <div className="mt-1 font-mono text-[10px] text-slate-500">Local · {localBaseUrl}</div>
            )}
          </div>
          <button className="icon-btn" onClick={() => void copy("base", cursorBaseUrl)} title="Copy Base URL">
            {copied === "base" ? <CheckCircle2 className="h-3.5 w-3.5" /> : <Clipboard className="h-3.5 w-3.5" />}
          </button>
        </section>

        <section className="surface-card p-3">
          <div className="section-head">
            <div className="flex items-center gap-1.5">
              <Globe className="h-3.5 w-3.5 text-sky-500" />
              <span className="label">Public Tunnel</span>
            </div>
            <SegmentCapsule
              enabled={tunnelEnabled}
              disabled={running}
              onChange={(enabled) => updateDraft("ngrok_enabled", enabled)}
            />
          </div>

          {tunnelEnabled ? (
            <>
              <p className="mt-2 text-[11px] leading-relaxed text-slate-500">
                ngrok exposes port <span className="font-mono font-semibold text-slate-700">{settings.port}</span> so Cursor (Ask &amp; Agent) can reach this bridge.
              </p>
              {tunnel?.configured && !settings.ngrok_authtoken.trim() && !ngrokTokenOverride ? (
                <div className="info-card mt-2">Using saved ngrok login on this Mac.</div>
              ) : (
                <div className="mt-2">
                  <span className="mb-1 block text-[9px] font-bold uppercase tracking-[0.16em] text-slate-400">Authtoken</span>
                  <div className="flex gap-1.5">
                    <input
                      className="field min-w-0 font-mono"
                      type={ngrokTokenVisible ? "text" : "password"}
                      value={settings.ngrok_authtoken}
                      placeholder={tunnel?.configured ? "Optional override" : "Paste ngrok authtoken"}
                      disabled={running}
                      onChange={(event) => updateDraft("ngrok_authtoken", event.target.value)}
                    />
                    <button
                      className="icon-btn"
                      onClick={() => setNgrokTokenVisible((value) => !value)}
                      title={ngrokTokenVisible ? "Hide authtoken" : "Show authtoken"}
                    >
                      {ngrokTokenVisible ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                    </button>
                  </div>
                </div>
              )}
              {tunnel?.configured && !settings.ngrok_authtoken.trim() && !ngrokTokenOverride && (
                <button
                  className="ghost-btn mt-2"
                  onClick={() => setNgrokTokenOverride(true)}
                  disabled={running}
                >
                  Override token
                </button>
              )}
              <div className="mt-2.5 flex flex-wrap gap-1.5">
                <StatusChip label="ngrok" ok={tunnel?.installed ?? false} />
                <StatusChip label="login" ok={tunnel?.configured ?? false} />
                <StatusChip label="tunnel" ok={tunnel?.running ?? false} />
              </div>
              {tunnelEnabled && !tunnel?.installed && (
                <p className="mt-2 text-[11px] text-amber-700">Install ngrok from ngrok.com/download</p>
              )}
              {tunnelEnabled && tunnel?.installed && !tunnel?.configured && (
                <p className="mt-2 text-[11px] text-amber-700">Run ngrok config add-authtoken or paste token above.</p>
              )}
              {tunnel?.error && <p className="mt-2 text-[11px] text-rose-600">{tunnel.error}</p>}
            </>
          ) : (
            <p className="mt-2 text-[11px] leading-relaxed text-slate-500">
              Cursor cannot reach 127.0.0.1. Enable Public Tunnel for Ask &amp; Agent.
            </p>
          )}
        </section>

        {error && <div className="error-card">{error}</div>}
        {!error && tunnel?.error && running && <div className="warning-card">{tunnel.error}</div>}

        <section className="grid grid-cols-2 gap-2">
          <div className="surface-card p-2.5">
            <div className="mb-1.5 flex items-center gap-1.5">
              <PlugZap className="h-3.5 w-3.5 text-sky-500" />
              <span className="label">Port</span>
            </div>
            <input
              className="field font-mono"
              disabled={running}
              inputMode="numeric"
              value={settings.port}
              onChange={(event) => updateDraft("port", Number(event.target.value || 0))}
            />
            <p className={`mt-1.5 text-[10px] ${portValidation?.available === false ? "text-rose-500" : "text-slate-500"}`}>
              {portValidation?.message ?? "Checking..."}
            </p>
          </div>

          <div className="surface-card p-2.5">
            <div className="mb-1.5 flex items-center gap-1.5">
              <KeyRound className="h-3.5 w-3.5 text-sky-500" />
              <span className="label">API Key</span>
            </div>
            <div className="flex gap-1.5">
              <input
                className="field min-w-0 flex-1 font-mono"
                type={apiKeyVisible ? "text" : "password"}
                value={settings.api_key}
                placeholder="Generate key"
                onChange={(event) => updateDraft("api_key", event.target.value)}
              />
              <button className="icon-btn shrink-0" onClick={generateKey} title="Generate key">
                {busy === "key" ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Shuffle className="h-3.5 w-3.5" />}
              </button>
            </div>
            <div className="mt-1.5 flex items-center justify-between gap-2">
              <span className="truncate text-[10px] text-slate-500">
                {apiKeyVisible ? "Key visible" : shortKey(settings.api_key)}
              </span>
              <div className="flex shrink-0 items-center gap-1">
                <button
                  className="icon-btn h-7 w-7"
                  onClick={() => settings.api_key && void copy("api-key", settings.api_key)}
                  disabled={!settings.api_key}
                  title="Copy API key"
                >
                  {copied === "api-key" ? <CheckCircle2 className="h-3.5 w-3.5" /> : <Clipboard className="h-3.5 w-3.5" />}
                </button>
                <button
                  className="icon-btn h-7 w-7"
                  onClick={() => setApiKeyVisible((value) => !value)}
                  title={apiKeyVisible ? "Hide API key" : "Show API key"}
                >
                  {apiKeyVisible ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                </button>
              </div>
            </div>
          </div>
        </section>

        <section className="surface-card p-2.5">
          <button className="flex w-full items-center justify-between" onClick={() => setAdvancedOpen((value) => !value)}>
            <span className="flex items-center gap-1.5">
              <SlidersHorizontal className="h-3.5 w-3.5 text-sky-500" />
              <span className="label">Defaults</span>
            </span>
            <ChevronDown className={`h-3.5 w-3.5 text-slate-400 transition ${advancedOpen ? "rotate-180" : ""}`} />
          </button>

          <div className="mt-2 grid grid-cols-2 gap-2">
            <div>
              <span className="mb-1 block text-[9px] font-bold uppercase tracking-[0.16em] text-slate-400">Cursor</span>
              <div className="field flex items-center font-mono text-[12px]">{CURSOR_MODEL}</div>
            </div>
            <SelectField
              label="Codex"
              value={settings.codex_model}
              options={codexModelOptions}
              onChange={(value) => updateDraft("codex_model", value)}
            />
          </div>

          {advancedOpen && (
            <div className="mt-2 grid grid-cols-2 gap-2 border-t border-slate-200/60 pt-2">
              <SelectField label="Profile" value={settings.codex_profile} options={profileOptions} onChange={(v) => updateDraft("codex_profile", v)} />
              <SelectField
                label="Sandbox"
                value={settings.codex_sandbox}
                options={[
                  { value: "read-only", label: "read-only" },
                  { value: "workspace-write", label: "workspace-write" },
                ]}
                onChange={(v) => updateDraft("codex_sandbox", v)}
              />
              <SelectField
                label="Approval"
                value={settings.codex_approval}
                options={[
                  { value: "never", label: "never" },
                  { value: "on-request", label: "on-request" },
                  { value: "untrusted", label: "untrusted" },
                ]}
                onChange={(v) => updateDraft("codex_approval", v)}
              />
              <div>
                <span className="mb-1 block text-[9px] font-bold uppercase tracking-[0.16em] text-slate-400">Timeout (s)</span>
                <input
                  className="field font-mono"
                  type="number"
                  min={60}
                  step={30}
                  value={Math.round(settings.codex_timeout_ms / 1000)}
                  onChange={(event) => updateDraft("codex_timeout_ms", Math.max(60, Number(event.target.value || 0)) * 1000)}
                />
              </div>
              <div>
                <span className="mb-1 block text-[9px] font-bold uppercase tracking-[0.16em] text-slate-400">Context msgs</span>
                <input
                  className="field font-mono"
                  type="number"
                  min={4}
                  max={128}
                  step={1}
                  value={settings.codex_max_messages}
                  onChange={(event) => updateDraft("codex_max_messages", Math.max(4, Number(event.target.value || 0)))}
                />
              </div>
              <button className="ghost-btn self-end" onClick={saveDraft} disabled={busy === "save"}>
                {busy === "save" ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <CheckCircle2 className="h-3.5 w-3.5" />}
                Save
              </button>
            </div>
          )}
        </section>

        {running && (
          <>
            <section className="grid grid-cols-4 gap-1.5">
              {usageCards.map(([label, value]) => (
                <div className="metric-card" key={label}>
                  <div className="text-[8px] uppercase tracking-[0.14em] text-slate-400">{label}</div>
                  <div className="mt-0.5 truncate font-mono text-[13px] font-bold text-slate-900">{value}</div>
                </div>
              ))}
            </section>

            <section className="surface-card p-2.5">
              <div className="section-head">
                <div className="flex items-center gap-1.5">
                  <Activity className="h-3.5 w-3.5 text-sky-500" />
                  <span className="label">Codex</span>
                </div>
                <button className="icon-btn h-7 w-7" onClick={() => void refreshCodex()} disabled={codexRefreshing} title="Refresh">
                  <RefreshCw className={`h-3.5 w-3.5 ${codexRefreshing ? "animate-spin" : ""}`} />
                </button>
              </div>

              <div className="mt-2 flex flex-wrap gap-1.5">
                <StatusChip label="CLI" ok={codex?.cli_installed ?? false} />
                <StatusChip label="Auth" ok={codex?.authenticated ?? false} />
              </div>

              <div className="mt-2 text-[13px] font-semibold text-slate-900">
                {codex?.summary ?? "Tap refresh to check Codex CLI"}
              </div>
              <p className="mt-0.5 text-[11px] leading-relaxed text-slate-500">
                {codex?.detail ?? "Session usage updates while running."}
              </p>
              <p className="mt-1.5 text-[9px] uppercase tracking-[0.14em] text-slate-400">
                {formatCheckedAt(codex?.checked_at_ms ?? 0)}
              </p>

              <div className="mt-2.5 space-y-1.5">
                {sessionBars.map((bar) => (
                  <div key={bar.label}>
                    <div className="mb-0.5 flex items-center justify-between text-[10px] text-slate-600">
                      <span>{bar.label}</span>
                      <span className="font-mono font-semibold text-slate-800">{bar.value}</span>
                    </div>
                    <div className="h-1 overflow-hidden rounded-full bg-slate-100">
                      <div className={`h-full rounded-full ${bar.color}`} style={{ width: `${bar.pct}%` }} />
                    </div>
                  </div>
                ))}
              </div>
            </section>
          </>
        )}

        <section className="flex items-center gap-2 pt-0.5">
          <button className="ghost-btn flex-1 justify-start" onClick={toggleLaunch} disabled={busy === "launch"}>
            {settings.launch_at_login ? <ToggleRight className="h-3.5 w-3.5 text-emerald-500" /> : <ToggleLeft className="h-3.5 w-3.5" />}
            Login item
          </button>
          <button className="ghost-btn" onClick={() => void call("quit_app")}>
            <LogOut className="h-3.5 w-3.5" />
            Quit
          </button>
        </section>

        {usage?.last_error && running && !activityOpen && (
          <div className="warning-card">Last request: {usage.last_error}</div>
        )}
      </div>

      {activityOpen && running && (
        <ActivityPopover
          logs={activityLogs}
          lastError={usage?.last_error}
          onClose={() => setActivityOpen(false)}
        />
      )}

      {setupOpen && (
        <CursorSetupModal
          baseUrl={cursorBaseUrl}
          apiKey={settings.api_key}
          model={CURSOR_MODEL}
          usePublicUrl={Boolean(tunnelEnabled && tunnel?.public_url)}
          copied={copied}
          onCopy={(label, value) => void copy(label, value)}
          onClose={() => setSetupOpen(false)}
        />
      )}
    </main>
  );
}

function ActivityPopover({
  logs,
  lastError,
  onClose,
}: {
  logs: string[];
  lastError?: string | null;
  onClose: () => void;
}) {
  return (
    <>
      <button
        type="button"
        className="activity-popover-backdrop"
        onClick={onClose}
        aria-label="Close activity"
      />
      <div className="activity-popover" role="dialog" aria-label="Request activity">
        <div className="activity-popover-head">
          <div className="flex items-center gap-1.5">
            <Activity className="h-3.5 w-3.5 text-sky-500" />
            <span className="text-[12px] font-bold text-slate-800">Activity</span>
          </div>
          <button
            type="button"
            className="icon-btn h-7 w-7"
            onClick={onClose}
            title="Collapse activity"
            aria-label="Collapse activity"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
        {lastError && <div className="activity-error">{lastError}</div>}
        <div className="activity-log">
          {logs.length > 0 ? (
            logs.slice(-20).map((line, index) => (
              <div key={`${line}-${index}`} className="activity-log-line">{line}</div>
            ))
          ) : (
            <div className="activity-log-empty">Waiting for requests…</div>
          )}
        </div>
      </div>
    </>
  );
}

function SegmentCapsule({
  enabled,
  disabled,
  onChange,
}: {
  enabled: boolean;
  disabled?: boolean;
  onChange: (enabled: boolean) => void;
}) {
  return (
    <div className="segment-capsule" role="group" aria-label="Public tunnel mode">
      <button
        type="button"
        className={`segment-option ${enabled ? "segment-option-active" : ""}`}
        disabled={disabled}
        onClick={() => onChange(true)}
      >
        Enable
      </button>
      <button
        type="button"
        className={`segment-option ${!enabled ? "segment-option-active" : ""}`}
        disabled={disabled}
        onClick={() => onChange(false)}
      >
        Disable
      </button>
    </div>
  );
}

function CursorSetupModal({
  baseUrl,
  apiKey,
  model,
  usePublicUrl,
  copied,
  onCopy,
  onClose,
}: {
  baseUrl: string;
  apiKey: string;
  model: string;
  usePublicUrl: boolean;
  copied: string | null;
  onCopy: (label: string, value: string) => void;
  onClose: () => void;
}) {
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-card" onClick={(event) => event.stopPropagation()}>
        <div className="mb-3 flex items-start justify-between gap-3">
          <div>
            <div className="label">Cursor Setup</div>
            <h2 className="mt-1 text-lg font-black tracking-tight text-slate-900">Connect Cursor to gpt2cursor</h2>
          </div>
          <button className="icon-btn h-8 w-8 shrink-0" onClick={onClose} title="Close">
            <X className="h-4 w-4" />
          </button>
        </div>
        <ol className="space-y-3 text-sm leading-relaxed text-slate-600">
          <li>
            <span className="font-semibold text-slate-800">1. Base URL</span>
            <p className="mt-1">In Cursor Settings → Models, enable Override OpenAI Base URL and paste the {usePublicUrl ? "public" : "local"} Base URL.</p>
            <button className="ghost-btn mt-2 h-8 px-2.5 text-xs" onClick={() => onCopy("setup-base", baseUrl)}>
              {copied === "setup-base" ? <CheckCircle2 className="h-3.5 w-3.5" /> : <Clipboard className="h-3.5 w-3.5" />}
              <span className="max-w-[260px] truncate font-mono">{baseUrl}</span>
            </button>
          </li>
          <li>
            <span className="font-semibold text-slate-800">2. API Key</span>
            <p className="mt-1">Paste the gpt2cursor API key into OpenAI API Key.</p>
            <button className="ghost-btn mt-2 h-8 px-2.5 text-xs" onClick={() => onCopy("setup-key", apiKey)}>
              {copied === "setup-key" ? <CheckCircle2 className="h-3.5 w-3.5" /> : <Clipboard className="h-3.5 w-3.5" />}
              Copy API key
            </button>
          </li>
          <li>
            <span className="font-semibold text-slate-800">3. Custom model</span>
            <p className="mt-1">Click + Add Custom Model and add the model name below.</p>
            <button className="ghost-btn mt-2 h-8 px-2.5 text-xs" onClick={() => onCopy("setup-model", model)}>
              {copied === "setup-model" ? <CheckCircle2 className="h-3.5 w-3.5" /> : <Clipboard className="h-3.5 w-3.5" />}
              {model}
            </button>
          </li>
        </ol>
        <button className="primary-btn mt-4 w-full" onClick={onClose}>Got it</button>
      </div>
    </div>
  );
}

function StatusChip({ label, ok }: { label: string; ok: boolean }) {
  return (
    <span className={ok ? "status-chip-ok" : "status-chip-off"}>
      <span className={`h-1 w-1 rounded-full ${ok ? "bg-emerald-500" : "bg-slate-300"}`} />
      {label}
    </span>
  );
}

function SelectField({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: Array<{ value: string; label: string }>;
  onChange: (value: string) => void;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-[9px] font-bold uppercase tracking-[0.16em] text-slate-400">{label}</span>
      <select className="field appearance-auto" value={value} onChange={(event) => onChange(event.target.value)}>
        {options.map((option) => (
          <option key={option.label} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

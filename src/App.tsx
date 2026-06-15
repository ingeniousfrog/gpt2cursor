import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  CheckCircle2,
  ChevronDown,
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
  Wifi,
  WifiOff,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import appIcon from "../src-tauri/icons/icon.png";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
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
  launch_at_login: boolean;
  ngrok_enabled: boolean;
  ngrok_authtoken: string;
};

type TunnelStatus = {
  installed: boolean;
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

const isTauri = () => typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri() && import.meta.env.DEV) {
    return mockCommand<T>(command, args);
  }
  return invoke<T>(command, args);
}

async function mockCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const settings: AppSettings = {
    port: 8787,
    api_key: "g2c_preview_5db6f88baf29d2c8",
    model: CURSOR_MODEL,
    codex_command: "codex",
    codex_model: "gpt-5.5",
    codex_profile: "",
    codex_sandbox: "read-only",
    codex_approval: "never",
    codex_timeout_ms: 120000,
    launch_at_login: false,
    ngrok_enabled: false,
    ngrok_authtoken: "",
  };
  const running = command === "start_bridge";
  const state: AppViewState = {
    settings,
    bridge: {
      running,
      port: settings.port,
      base_url: "http://127.0.0.1:8787/v1",
      usage: {
        request_count: running ? 18 : 0,
        active_requests: running ? 1 : 0,
        last_duration_ms: running ? 1840 : 0,
        total_duration_ms: running ? 42100 : 0,
        last_usage: { input_tokens: 812, cached_input_tokens: 128, output_tokens: 244, reasoning_output_tokens: 36 },
        total_usage: { input_tokens: 8840, cached_input_tokens: 1600, output_tokens: 2301, reasoning_output_tokens: 412 },
        last_error: null,
      },
    },
    tunnel: {
      installed: true,
      running,
      local_url: "http://127.0.0.1:8787/v1",
      public_url: running ? "https://preview.ngrok-free.app/v1" : null,
      error: null,
    },
    codex: {
      cli_installed: true,
      authenticated: true,
      summary: "Codex CLI is authenticated",
      detail: "Local CLI session is ready.",
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
      detail: "Local CLI session is ready. Per-session token usage updates below.",
      checked_at_ms: Date.now(),
    } as T;
  }
  if (command === "save_settings" && args?.input && typeof args.input === "object") {
    const input = args.input as { settings?: AppSettings };
    return { ...state, settings: input.settings ?? settings } as T;
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
  const [advancedOpen, setAdvancedOpen] = useState(false);
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
    setState(next);
    setDraft((current) => current ?? next.settings);
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
  const usage = bridge?.usage;
  const canStart = Boolean(
    settings?.api_key
    && portValidation?.available !== false
    && (!settings.ngrok_enabled || settings.ngrok_authtoken.trim()),
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

  const running = Boolean(bridge?.running);
  const startDisabled = busy === "start" || busy === "stop" || (!running && !canStart);

  return (
    <main className="h-full overflow-hidden rounded-[26px] bg-panel text-slate-800 shadow-panel">
      <div className="pointer-events-none absolute inset-0 bg-mesh opacity-40" />
      <div className="panel-scroll relative flex h-full flex-col gap-3 overflow-y-auto p-4">
        <section className="hero-card">
          <div className="flex min-w-0 items-center gap-3">
            <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-[18px] bg-white/35 p-1.5 shadow-logo">
              <img src={appIcon} alt="" className="h-full w-full object-contain" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="label">Local Bridge</div>
              <div className="mt-1 flex items-center gap-2">
                {running ? <Wifi className="h-4 w-4 text-sky-500" /> : <WifiOff className="h-4 w-4 text-slate-400" />}
                <h1 className="truncate text-[22px] font-black tracking-tight text-slate-900">
                  {running ? "Running" : "Ready"}
                </h1>
              </div>
              <p className="mt-1 truncate text-xs text-slate-500">
                {running ? `Listening on port ${bridge?.port}` : "Choose a port and start the local endpoint"}
              </p>
            </div>
          </div>
          <button
            className={running ? "stop-btn" : "primary-btn"}
            disabled={startDisabled}
            onClick={running ? stop : start}
          >
            {busy === "start" || busy === "stop" ? <Loader2 className="h-4 w-4 animate-spin" /> : <Power className="h-4 w-4" />}
            {running ? "Stop" : "Start"}
          </button>
        </section>

        <section className="base-card">
          <div className="min-w-0">
            <div className="label">Cursor Base URL</div>
            <div className="mt-1 break-all font-mono text-[13px] font-semibold text-sky-700">{cursorBaseUrl}</div>
            {tunnel?.public_url && (
              <div className="mt-1 break-all font-mono text-[11px] text-slate-500">
                Local: {localBaseUrl}
              </div>
            )}
          </div>
          <button className="icon-btn" onClick={() => void copy("base", cursorBaseUrl)} title="Copy Base URL">
            {copied === "base" ? <CheckCircle2 className="h-4 w-4" /> : <Clipboard className="h-4 w-4" />}
          </button>
        </section>

        <section className="soft-card p-3">
          <div className="mb-2 flex items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <Globe className="h-4 w-4 text-sky-500" />
              <span className="label">Public Tunnel</span>
            </div>
            <button
              className="ghost-btn h-8 px-2.5 text-xs"
              onClick={() => updateDraft("ngrok_enabled", !settings.ngrok_enabled)}
              disabled={running}
            >
              {settings.ngrok_enabled ? <ToggleRight className="h-4 w-4 text-sky-500" /> : <ToggleLeft className="h-4 w-4" />}
              {settings.ngrok_enabled ? "Enabled" : "Disabled"}
            </button>
          </div>
          <p className="text-xs leading-relaxed text-slate-500">
            Expose the local bridge through ngrok so Cursor Agent can reach your endpoint from the cloud.
          </p>
          <div className="mt-3">
            <span className="mb-1 block text-[10px] font-bold uppercase tracking-[0.14em] text-slate-400">ngrok Authtoken</span>
            <div className="flex gap-2">
              <input
                className="field min-w-0 font-mono"
                type={ngrokTokenVisible ? "text" : "password"}
                value={settings.ngrok_authtoken}
                placeholder="Paste your ngrok authtoken"
                disabled={running}
                onChange={(event) => updateDraft("ngrok_authtoken", event.target.value)}
              />
              <button
                className="icon-btn shrink-0"
                onClick={() => setNgrokTokenVisible((value) => !value)}
                title={ngrokTokenVisible ? "Hide authtoken" : "Show authtoken"}
              >
                {ngrokTokenVisible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
          </div>
          <div className="mt-3 flex flex-wrap gap-2">
            <StatusChip label="ngrok" ok={tunnel?.installed ?? false} />
            <StatusChip label="Tunnel" ok={tunnel?.running ?? false} />
          </div>
          {settings.ngrok_enabled && !tunnel?.installed && (
            <p className="mt-2 text-xs text-amber-700">Install ngrok from ngrok.com/download before starting.</p>
          )}
          {tunnel?.error && (
            <p className="mt-2 text-xs text-rose-600">{tunnel.error}</p>
          )}
          {tunnel?.public_url && (
            <p className="mt-2 text-xs text-emerald-700">Public URL is ready. Copy the Base URL above into Cursor.</p>
          )}
        </section>

        <section className="soft-card p-3">
          <div className="label">Cursor Setup</div>
          <ol className="mt-2 space-y-1.5 text-xs leading-relaxed text-slate-600">
            <li>1. In Cursor Settings → Models, enable <span className="font-semibold text-slate-800">Override OpenAI Base URL</span> and paste the {settings.ngrok_enabled ? "public" : "local"} Base URL above.</li>
            <li>2. Paste the API Key from this app into <span className="font-semibold text-slate-800">OpenAI API Key</span>.</li>
            <li>3. Click <span className="font-semibold text-slate-800">+ Add Custom Model</span> and add <span className="font-mono font-semibold text-sky-700">{CURSOR_MODEL}</span>.</li>
          </ol>
          <button
            className="ghost-btn mt-3 h-8 px-2.5 text-xs"
            onClick={() => void copy("model", CURSOR_MODEL)}
            title="Copy model name"
          >
            {copied === "model" ? <CheckCircle2 className="h-3.5 w-3.5" /> : <Clipboard className="h-3.5 w-3.5" />}
            {CURSOR_MODEL}
          </button>
        </section>

        {error && <div className="error-card">{error}</div>}

        <section className="grid grid-cols-[0.82fr_1.18fr] gap-3">
          <div className="soft-card p-3">
            <div className="mb-2 flex items-center gap-2">
              <PlugZap className="h-4 w-4 text-sky-500" />
              <span className="label">Port</span>
            </div>
            <input
              className="field font-mono"
              disabled={running}
              inputMode="numeric"
              value={settings.port}
              onChange={(event) => updateDraft("port", Number(event.target.value || 0))}
            />
            <p className={`mt-2 text-xs ${portValidation?.available === false ? "text-rose-500" : "text-slate-500"}`}>
              {portValidation?.message ?? "Checking port..."}
            </p>
          </div>

          <div className="soft-card p-3">
            <div className="mb-2 flex items-center gap-2">
              <KeyRound className="h-4 w-4 text-sky-500" />
              <span className="label">API Key</span>
            </div>
            <div className="flex gap-2">
              <input
                className="field min-w-0 font-mono"
                type={apiKeyVisible ? "text" : "password"}
                value={settings.api_key}
                placeholder="Generate or paste key"
                onChange={(event) => updateDraft("api_key", event.target.value)}
              />
              <button className="icon-btn shrink-0" onClick={generateKey} title="Generate key">
                {busy === "key" ? <Loader2 className="h-4 w-4 animate-spin" /> : <Shuffle className="h-4 w-4" />}
              </button>
            </div>
            <div className="mt-2 flex items-center justify-between text-xs text-slate-500">
              <span>{apiKeyVisible ? "Key is visible" : shortKey(settings.api_key)}</span>
              <button
                className="inline-flex h-7 w-7 items-center justify-center rounded-full text-slate-500 transition hover:bg-sky-100 hover:text-sky-600"
                onClick={() => setApiKeyVisible((value) => !value)}
                title={apiKeyVisible ? "Hide API key" : "Show API key"}
                aria-label={apiKeyVisible ? "Hide API key" : "Show API key"}
              >
                {apiKeyVisible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
          </div>
        </section>

        <section className="soft-card p-3">
          <button
            className="flex w-full items-center justify-between"
            onClick={() => setAdvancedOpen((value) => !value)}
          >
            <span className="flex items-center gap-2">
              <SlidersHorizontal className="h-4 w-4 text-sky-500" />
              <span className="label">Defaults</span>
            </span>
            <ChevronDown className={`h-4 w-4 text-slate-500 transition ${advancedOpen ? "rotate-180" : ""}`} />
          </button>

          <div className="mt-3 grid grid-cols-2 gap-2">
            <div className="block">
              <span className="mb-1 block text-[10px] font-bold uppercase tracking-[0.14em] text-slate-400">Cursor model</span>
              <div className="field flex items-center font-mono text-[13px]">{CURSOR_MODEL}</div>
            </div>
            <SelectField
              label="Codex model"
              value={settings.codex_model}
              options={codexModelOptions}
              onChange={(value) => updateDraft("codex_model", value)}
            />
          </div>

          {advancedOpen && (
            <div className="mt-2 grid grid-cols-2 gap-2">
              <SelectField
                label="Profile"
                value={settings.codex_profile}
                options={profileOptions}
                onChange={(value) => updateDraft("codex_profile", value)}
              />
              <SelectField
                label="Sandbox"
                value={settings.codex_sandbox}
                options={[
                  { value: "read-only", label: "read-only" },
                  { value: "workspace-write", label: "workspace-write" },
                ]}
                onChange={(value) => updateDraft("codex_sandbox", value)}
              />
              <SelectField
                label="Approval"
                value={settings.codex_approval}
                options={[
                  { value: "never", label: "never" },
                  { value: "on-request", label: "on-request" },
                  { value: "untrusted", label: "untrusted" },
                ]}
                onChange={(value) => updateDraft("codex_approval", value)}
              />
              <button className="ghost-btn self-end" onClick={saveDraft} disabled={busy === "save"}>
                {busy === "save" ? <Loader2 className="h-4 w-4 animate-spin" /> : <CheckCircle2 className="h-4 w-4" />}
                Save
              </button>
            </div>
          )}
        </section>

        {running && (
          <>
            <section className="grid grid-cols-4 gap-2">
              {usageCards.map(([label, value]) => (
                <div className="metric-card" key={label}>
                  <div className="text-[9px] uppercase tracking-[0.13em] text-slate-400">{label}</div>
                  <div className="mt-1 truncate font-mono text-sm font-black text-slate-900">{value}</div>
                </div>
              ))}
            </section>

            <section className="soft-card p-3">
              <div className="mb-2 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Activity className="h-4 w-4 text-sky-500" />
                  <span className="label">Codex Status</span>
                </div>
                <button
                  className="icon-btn h-8 w-8"
                  onClick={() => void refreshCodex()}
                  disabled={codexRefreshing}
                  title="Refresh Codex status"
                >
                  <RefreshCw className={`h-4 w-4 ${codexRefreshing ? "animate-spin" : ""}`} />
                </button>
              </div>

              <div className="mb-3 flex flex-wrap gap-2">
                <StatusChip label="CLI" ok={codex?.cli_installed ?? false} />
                <StatusChip label="Auth" ok={codex?.authenticated ?? false} />
              </div>

              <div className="text-sm font-bold text-slate-900">
                {codex?.summary ?? "Tap refresh to check Codex CLI"}
              </div>
              <p className="mt-1 text-xs leading-relaxed text-slate-500">
                {codex?.detail ?? "Session token usage updates automatically while the bridge is running."}
              </p>
              <p className="mt-2 text-[10px] uppercase tracking-[0.14em] text-slate-400">
                Checked {formatCheckedAt(codex?.checked_at_ms ?? 0)}
              </p>

              <div className="mt-3 space-y-2">
                <div className="text-[10px] font-bold uppercase tracking-[0.14em] text-slate-400">Session tokens</div>
                {sessionBars.map((bar) => (
                  <div key={bar.label}>
                    <div className="mb-1 flex items-center justify-between text-[11px] text-slate-600">
                      <span>{bar.label}</span>
                      <span className="font-mono font-semibold text-slate-800">{bar.value}</span>
                    </div>
                    <div className="h-1.5 overflow-hidden rounded-full bg-slate-100">
                      <div className={`h-full rounded-full ${bar.color}`} style={{ width: `${bar.pct}%` }} />
                    </div>
                  </div>
                ))}
              </div>
            </section>
          </>
        )}

        <section className="flex items-center justify-between gap-2">
          <button className="ghost-btn flex-1 justify-start" onClick={toggleLaunch} disabled={busy === "launch"}>
            {settings.launch_at_login ? <ToggleRight className="h-4 w-4 text-sky-500" /> : <ToggleLeft className="h-4 w-4" />}
            Launch at login
          </button>
          <button className="ghost-btn" onClick={() => void call("quit_app")}>
            <LogOut className="h-4 w-4" />
            Quit
          </button>
        </section>

        {usage?.last_error && running && <div className="warning-card">Last request: {usage.last_error}</div>}
      </div>
    </main>
  );
}

function StatusChip({ label, ok }: { label: string; ok: boolean }) {
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[11px] font-semibold ${ok ? "bg-emerald-50 text-emerald-700" : "bg-slate-100 text-slate-500"}`}>
      <span className={`h-1.5 w-1.5 rounded-full ${ok ? "bg-emerald-500" : "bg-slate-300"}`} />
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
      <span className="mb-1 block text-[10px] font-bold uppercase tracking-[0.14em] text-slate-400">{label}</span>
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

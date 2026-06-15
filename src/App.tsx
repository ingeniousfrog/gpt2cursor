import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  CheckCircle2,
  ChevronDown,
  Clipboard,
  Eye,
  EyeOff,
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

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

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
};

type BridgeStatus = {
  running: boolean;
  port: number;
  base_url: string;
  usage: UsageSnapshot;
};

type CodexStatus = {
  available: boolean;
  summary: string;
  detail: string;
};

type AppViewState = {
  settings: AppSettings;
  bridge: BridgeStatus;
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

const cursorModelOptions = [
  { value: "codex-local", label: "codex-local" },
  { value: "gpt2cursor-local", label: "gpt2cursor-local" },
];

const codexModelOptions = [
  { value: "", label: "Use Codex default" },
  { value: "gpt-5.5", label: "GPT-5.5" },
  { value: "gpt-5", label: "GPT-5" },
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
    model: "codex-local",
    codex_command: "codex",
    codex_model: "",
    codex_profile: "",
    codex_sandbox: "read-only",
    codex_approval: "never",
    codex_timeout_ms: 120000,
    launch_at_login: false,
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
    codex: {
      available: false,
      summary: "Codex status appears after the bridge starts",
      detail: "The packaged Tauri app talks to the native Rust bridge.",
    },
  };

  if (command === "validate_port") {
    const port = Number(args?.port ?? 0);
    return { port, available: port > 0, message: port > 0 ? "Port is available" : "Port must be between 1 and 65535" } as T;
  }
  if (command === "generate_api_key") {
    return "g2c_preview_6f7a3d88e16c4baf9120" as T;
  }
  if (command === "refresh_codex_status") {
    return {
      available: false,
      summary: "Codex CLI is authenticated; account quota is not exposed by this CLI.",
      detail: "Per-session token usage is shown reliably. Account quota uses best-effort local CLI status after startup.",
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
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [codex, setCodex] = useState<CodexStatus | null>(null);

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
    }, 1500);
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
  const settings = draft;
  const usage = bridge?.usage;
  const canStart = Boolean(settings?.api_key && portValidation?.available !== false);
  const baseUrl = bridge?.base_url ?? `http://127.0.0.1:${settings?.port ?? 8787}/v1`;

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
    setBusy("codex");
    setError(null);
    try {
      setCodex(await call<CodexStatus>("refresh_codex_status"));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  }, []);

  const start = useCallback(async () => {
    if (!settings) return;
    setBusy("start");
    setError(null);
    try {
      await call<AppViewState>("save_settings", { input: { settings } });
      const next = await call<AppViewState>("start_bridge");
      setState(next);
      setDraft(next.settings);
      void refreshCodex();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [refreshCodex, settings]);

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
      <div className="pointer-events-none absolute inset-0 bg-mesh opacity-80" />
      <div className="panel-scroll relative flex h-full flex-col gap-3 overflow-y-auto p-4">
        <section className="hero-card">
          <div className="flex min-w-0 items-center gap-3">
            <img src="/src-tauri/icons/icon.png" alt="" className="h-14 w-14 rounded-[18px] object-cover shadow-logo" />
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
            <div className="mt-1 break-all font-mono text-[13px] font-semibold text-sky-700">{baseUrl}</div>
          </div>
          <button className="icon-btn" onClick={() => void copy("base", baseUrl)} title="Copy Base URL">
            {copied === "base" ? <CheckCircle2 className="h-4 w-4" /> : <Clipboard className="h-4 w-4" />}
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
            <SelectField
              label="Cursor model"
              value={settings.model}
              options={cursorModelOptions}
              onChange={(value) => updateDraft("model", value)}
            />
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
                <button className="icon-btn h-8 w-8" onClick={refreshCodex} title="Refresh Codex status">
                  {busy === "codex" ? <Loader2 className="h-4 w-4 animate-spin" /> : <RefreshCw className="h-4 w-4" />}
                </button>
              </div>
              <div className="text-sm font-bold text-slate-900">{codex?.summary ?? "Checking Codex CLI..."}</div>
              <p className="mt-1 text-xs leading-relaxed text-slate-500">
                {codex?.detail ?? "This appears after the local bridge is started."}
              </p>
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

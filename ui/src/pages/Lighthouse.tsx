import { useEffect, useState, useRef, useCallback } from "react";
import { Loader2, Globe, Trash2, ArrowDown, Shield, Eye, EyeOff, Copy, Check, Radio, Download } from "lucide-react";
import { useGatewayLogs } from "../contexts/LogContext";
import {
  getStatus,
  startGateway,
  stopGateway,
  gatewayStatus,
  getGatewaySettings,
  setGatewaySettings,
  reloadGateway,
  type HarborStatus,
  type GatewaySettingsInfo,
} from "../lib/tauri";

const levelColor: Record<string, string> = {
  ERROR: "text-red",
  WARN: "text-yellow",
  INFO: "text-text-primary",
  DEBUG: "text-text-muted",
  TRACE: "text-text-muted",
};

function Lighthouse() {
  const [status, setStatus] = useState<HarborStatus | null>(null);
  const [running, setRunning] = useState(false);
  const [loading, setLoading] = useState(false);
  const [msg, setMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const { logs, clearLogs } = useGatewayLogs();
  const [autoScroll, setAutoScroll] = useState(true);
  const logEndRef = useRef<HTMLDivElement>(null);
  const msgTimerRef = useRef<ReturnType<typeof setTimeout>>(null);

  // Gateway settings
  const [settings, setSettings] = useState<GatewaySettingsInfo | null>(null);
  const [exposed, setExposed] = useState(false);
  const [token, setToken] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [savingSettings, setSavingSettings] = useState(false);
  const [copied, setCopied] = useState(false);
  const [tokenSaved, setTokenSaved] = useState(false);

  useEffect(() => {
    getStatus().then(setStatus).catch(() => {});
    gatewayStatus().then(setRunning).catch(() => {});
    getGatewaySettings().then((s) => {
      setSettings(s);
      setExposed(s.host === "0.0.0.0");
      setToken(s.token ?? "");
    }).catch(() => {});
    return () => {
      if (msgTimerRef.current) clearTimeout(msgTimerRef.current);
    };
  }, []);

  // Auto-scroll (debounced to avoid queueing animations under high log volume)
  useEffect(() => {
    if (!autoScroll || !logEndRef.current) return;
    const id = requestAnimationFrame(() => {
      logEndRef.current?.scrollIntoView({ behavior: "instant" });
    });
    return () => cancelAnimationFrame(id);
  }, [logs, autoScroll]);

  const showMsg = useCallback((text: string, isError: boolean) => {
    if (msgTimerRef.current) clearTimeout(msgTimerRef.current);
    setMsg({ text, isError });
    msgTimerRef.current = setTimeout(() => setMsg(null), 3000);
  }, []);

  const saveTokenIfDirty = async () => {
    const currentToken = settings?.token ?? "";
    if (token === currentToken) return;
    const host = exposed ? "0.0.0.0" : "127.0.0.1";
    await setGatewaySettings(host, token || null);
    setSettings((prev) => prev ? { ...prev, token: token || null } : prev);
  };

  const handleToggle = async () => {
    setLoading(true);
    setMsg(null);
    try {
      if (running) {
        const result = await stopGateway();
        setRunning(false);
        showMsg(result, false);
      } else {
        await saveTokenIfDirty();
        const result = await startGateway();
        setRunning(true);
        showMsg(result, false);
        const s = await getStatus();
        setStatus(s);
      }
    } catch (e) {
      showMsg(String(e), true);
      gatewayStatus().then(setRunning).catch(() => {});
    } finally {
      setLoading(false);
    }
  };

  const restartGateway = async () => {
    setLoading(true);
    try {
      await stopGateway();
      setRunning(false);
      await startGateway();
      setRunning(true);
    } finally {
      setLoading(false);
    }
  };

  const handleToggleExpose = async () => {
    const newExposed = !exposed;
    setExposed(newExposed);
    setSavingSettings(true);
    try {
      const host = newExposed ? "0.0.0.0" : "127.0.0.1";
      await setGatewaySettings(host, token || null);

      if (running) {
        await restartGateway();
      }

      const s = await getStatus();
      setStatus(s);
    } catch (e) {
      showMsg(String(e), true);
      if (running) gatewayStatus().then(setRunning).catch(() => {});
    } finally {
      setSavingSettings(false);
    }
  };

  const handleTokenSave = async () => {
    const currentToken = settings?.token ?? "";
    if (token === currentToken) return;

    setSavingSettings(true);
    try {
      await saveTokenIfDirty();

      if (running) {
        await reloadGateway();
      }

      setTokenSaved(true);
      setTimeout(() => setTokenSaved(false), 2000);
    } catch (e) {
      showMsg(String(e), true);
    } finally {
      setSavingSettings(false);
    }
  };

  const [exported, setExported] = useState(false);

  const exportLogs = () => {
    if (logs.length === 0) return;
    const content = logs.map((e) => `${e.timestamp} ${e.level.padEnd(5)} ${e.message}`).join("\n");
    const blob = new Blob([content], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `harbor-signal-log-${new Date().toISOString().slice(0, 19).replace(/:/g, "-")}.log`;
    a.click();
    URL.revokeObjectURL(url);
    setExported(true);
    setTimeout(() => setExported(false), 1500);
  };

  const displayPort = status?.gateway_port ?? 3100;
  const displayHost = exposed ? (status?.local_ip ?? "0.0.0.0") : "127.0.0.1";

  return (
    <div className="p-8 max-w-4xl h-full flex flex-col">
      {/* Header with status */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-lg font-semibold text-text-primary">Lighthouse</h1>
          <p className="text-[13px] text-text-secondary mt-0.5">
            Light the beacon and watch the signal fires
          </p>
        </div>
        <button
          onClick={handleToggle}
          disabled={loading}
          className={`relative w-9 h-5 rounded-full shrink-0 transition-colors duration-300 ${
            running ? "bg-green" : "bg-text-muted/30"
          } ${loading ? "opacity-40 cursor-not-allowed" : ""}`}
        >
          {loading ? (
            <Loader2 className="absolute top-0.5 left-1/2 -translate-x-1/2 w-4 h-4 text-text-muted animate-spin" />
          ) : (
            <span
              className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform duration-300 ${
                running ? "translate-x-4" : "translate-x-0"
              }`}
            />
          )}
        </button>
      </div>

      {/* Toast */}
      {msg && (
        <div className={`mb-4 px-3 py-2 rounded-md text-[12px] border animate-fade-in ${
          msg.isError
            ? "bg-red-muted text-red border-red/20"
            : "bg-green-muted text-green border-green/20"
        }`}>
          {msg.text}
        </div>
      )}

      {/* Connection info — only when running */}
      {running && (
        <section className="rounded-lg bg-bg-element border border-border-subtle mb-4 shrink-0 animate-fade-in">
          {/* Endpoint row */}
          <div className="flex items-center justify-between px-4 py-3 text-[12px]">
            <span className="text-text-secondary">Endpoint</span>
            <button
              onClick={() => {
                const url = `http://${displayHost}:${displayPort}/mcp`;
                navigator.clipboard.writeText(url);
                setCopied(true);
                setTimeout(() => setCopied(false), 1500);
              }}
              className="group inline-flex items-center gap-1.5 px-2 py-0.5 rounded bg-bg-app hover:bg-bg-hover transition-colors duration-150"
            >
              <code className="font-mono text-[11px] text-text-primary">
                http://{displayHost}:{displayPort}/mcp
              </code>
              {copied
                ? <Check className="w-3 h-3 text-green" />
                : <Copy className="w-3 h-3 text-text-muted group-hover:text-text-secondary transition-colors" />
              }
            </button>
          </div>

          {/* Network settings */}
          <div className="border-t border-border-subtle px-4 py-3 space-y-2.5 text-[12px]">
            <div className="flex justify-between items-center">
              <div className="flex items-center gap-1.5">
                <Globe className="w-3 h-3 text-text-muted" />
                <span className="text-text-secondary">Expose to network</span>
              </div>
              <button
                onClick={handleToggleExpose}
                disabled={savingSettings || loading}
                className={`relative w-7 h-4 rounded-full shrink-0 transition-colors duration-300 ${
                  exposed ? "bg-yellow" : "bg-text-muted/30"
                } ${savingSettings || loading ? "opacity-40 cursor-not-allowed" : ""}`}
              >
                <span
                  className={`absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-white shadow-sm transition-transform duration-300 ${
                    exposed ? "translate-x-3" : "translate-x-0"
                  }`}
                />
              </button>
            </div>

            {exposed && (
              <div className="space-y-1.5 animate-fade-in">
                <div className="flex items-center gap-1.5">
                  <Shield className="w-3 h-3 text-text-muted" />
                  <span className="text-text-secondary">Bearer token</span>
                </div>
                <div className="relative">
                  <input
                    type={showToken ? "text" : "password"}
                    placeholder="Set a token for remote access"
                    value={token}
                    onChange={(e) => setToken(e.target.value)}
                    onBlur={handleTokenSave}
                    onKeyDown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); }}
                    disabled={savingSettings || loading}
                    className={`w-full px-3 py-1.5 pr-8 rounded-md text-[12px] font-mono bg-bg-app border text-text-primary placeholder:text-text-muted focus:outline-none focus:ring-1 transition-colors duration-150 ${
                      tokenSaved
                        ? "border-green focus:border-green focus:ring-green/30"
                        : "border-border-default focus:border-accent focus:ring-accent/30"
                    } ${savingSettings || loading ? "opacity-40 cursor-not-allowed" : ""}`}
                  />
                  <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1.5">
                    {tokenSaved && <Check className="w-3 h-3 text-green animate-fade-in" />}
                    <button
                      onClick={() => setShowToken(!showToken)}
                      className="text-text-muted hover:text-text-secondary transition-colors"
                    >
                      {showToken ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
                    </button>
                  </div>
                </div>
                {!token && !tokenSaved && (
                  <div className="px-2 py-1.5 rounded bg-red/10 border border-red/20 text-[11px] text-red">
                    No token set — anyone on the network can access your tools.
                  </div>
                )}
              </div>
            )}
          </div>
        </section>
      )}

      {/* Log Viewer */}
      <section className="flex-1 flex flex-col rounded-lg bg-bg-element border border-border-subtle overflow-hidden min-h-0">
        <div className="flex items-center justify-between px-4 py-2 border-b border-border-subtle shrink-0">
          <span className="text-[12px] font-medium text-text-primary">Signal Log</span>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setAutoScroll(!autoScroll)}
              className={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] transition-colors ${
                autoScroll
                  ? "bg-accent-muted text-accent"
                  : "text-text-muted hover:text-text-secondary"
              }`}
            >
              <ArrowDown className="w-3 h-3" />
              Auto-scroll
            </button>
            <button
              onClick={exportLogs}
              disabled={logs.length === 0}
              className={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] transition-colors disabled:opacity-30 disabled:cursor-not-allowed ${
                exported ? "text-green" : "text-text-muted hover:text-text-secondary"
              }`}
            >
              {exported ? <Check className="w-3 h-3" /> : <Download className="w-3 h-3" />}
              {exported ? "Saved" : "Export"}
            </button>
            <button
              onClick={clearLogs}
              className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] text-text-muted hover:text-text-secondary transition-colors"
            >
              <Trash2 className="w-3 h-3" />
              Clear
            </button>
            <span className="text-[11px] text-text-muted tabular-nums">{logs.length} entries</span>
          </div>
        </div>
        <div className="flex-1 overflow-auto p-3 font-mono text-[11px] leading-relaxed bg-bg-app">
          {logs.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full animate-fade-in">
              <div className="w-10 h-10 rounded-xl bg-bg-element border border-border-subtle flex items-center justify-center mb-3">
                <Radio className={`w-5 h-5 ${running ? "text-green animate-pulse" : "text-text-muted"}`} />
              </div>
              <p className="text-[13px] font-medium text-text-primary mb-0.5">
                {running ? "Listening for signals" : "No signals yet"}
              </p>
              <p className="text-[12px] text-text-secondary">
                {running ? "Log entries will appear here as they arrive" : "Light the beacon to start capturing logs"}
              </p>
            </div>
          ) : (
            logs.map((entry, i) => (
              <div key={i} className="flex gap-2 hover:bg-bg-hover rounded px-1 py-0.5">
                <span className="text-text-muted shrink-0">{entry.timestamp}</span>
                <span className={`shrink-0 w-12 ${levelColor[entry.level] ?? "text-text-secondary"}`}>
                  {entry.level}
                </span>
                <span className="text-text-secondary break-all">{entry.message}</span>
              </div>
            ))
          )}
          <div ref={logEndRef} />
        </div>
      </section>
    </div>
  );
}

export default Lighthouse;

import { useEffect, useState, useRef, useCallback } from "react";
import { Power, PowerOff, Loader2, Globe, Trash2, ArrowDown } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { getStatus, startGateway, stopGateway, gatewayStatus, type HarborStatus } from "../lib/tauri";

interface LogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
}

const MAX_LOG_ENTRIES = 500;

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
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const logEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getStatus().then(setStatus).catch(() => {});
    gatewayStatus().then(setRunning).catch(() => {});
  }, []);

  // Listen for gateway log events
  useEffect(() => {
    let cancelled = false;
    const setup = listen<LogEntry>("gateway-log", (event) => {
      if (!cancelled) {
        setLogs((prev) => {
          const next = [...prev, event.payload];
          return next.length > MAX_LOG_ENTRIES ? next.slice(-MAX_LOG_ENTRIES) : next;
        });
      }
    });
    return () => {
      cancelled = true;
      setup.then((unlisten) => unlisten());
    };
  }, []);

  // Auto-scroll
  useEffect(() => {
    if (autoScroll && logEndRef.current) {
      logEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [logs, autoScroll]);

  const handleToggle = async () => {
    setLoading(true);
    setMsg(null);
    try {
      if (running) {
        const result = await stopGateway();
        setRunning(false);
        setMsg({ text: result, isError: false });
      } else {
        const result = await startGateway();
        setRunning(true);
        setMsg({ text: result, isError: false });
      }
      setTimeout(() => setMsg(null), 3000);
    } catch (e) {
      setMsg({ text: String(e), isError: true });
      gatewayStatus().then(setRunning).catch(() => {});
      setTimeout(() => setMsg(null), 4000);
    } finally {
      setLoading(false);
    }
  };

  const clearLogs = useCallback(() => setLogs([]), []);

  return (
    <div className="p-8 max-w-4xl h-full flex flex-col">
      {/* Header */}
      <div className="mb-6">
        <h1 className="text-lg font-semibold text-text-primary">Lighthouse</h1>
        <p className="text-[13px] text-text-secondary mt-0.5">
          Light the beacon and watch the signal fires
        </p>
      </div>

      {/* Gateway Controls */}
      <section className="p-4 rounded-lg bg-bg-element border border-border-subtle mb-4 shrink-0">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Globe className="w-4 h-4 text-text-muted" />
            <h2 className="text-[13px] font-medium text-text-primary">Beacon</h2>
          </div>
          <span className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[11px] font-medium ${
            running ? "bg-green-muted text-green" : "bg-bg-app text-text-muted"
          }`}>
            <span className={`w-1.5 h-1.5 rounded-full ${running ? "bg-green" : "bg-text-muted"}`} />
            {running ? "Lit" : "Dark"}
          </span>
        </div>

        {msg && (
          <div className={`mb-3 px-3 py-2 rounded-md text-[12px] border animate-fade-in ${
            msg.isError
              ? "bg-red-muted text-red border-red/20"
              : "bg-green-muted text-green border-green/20"
          }`}>
            {msg.text}
          </div>
        )}

        <div className="text-[12px] space-y-2.5">
          <div className="flex justify-between items-center">
            <span className="text-text-secondary">Beacon port</span>
            <span className="text-text-primary tabular-nums">{status?.gateway_port ?? 3100}</span>
          </div>
          <div className="flex justify-between items-center">
            <span className="text-text-secondary">Endpoint</span>
            <code className="px-1.5 py-0.5 rounded bg-bg-app font-mono text-[11px] text-text-primary">
              http://127.0.0.1:{status?.gateway_port ?? 3100}/mcp
            </code>
          </div>
          <button
            onClick={handleToggle}
            disabled={loading}
            className={`w-full flex items-center justify-center gap-2 px-3 py-2 rounded-md text-[12px] font-medium border transition-colors duration-150 mt-1 ${
              running
                ? "bg-bg-app border-red/30 text-red hover:bg-red-muted"
                : "bg-accent border-accent text-white hover:bg-accent-hover"
            } disabled:opacity-40 disabled:cursor-not-allowed`}
          >
            {loading ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : running ? (
              <PowerOff className="w-3.5 h-3.5" />
            ) : (
              <Power className="w-3.5 h-3.5" />
            )}
            {loading ? "Working..." : running ? "Extinguish" : "Light the Beacon"}
          </button>
        </div>
      </section>

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
            <div className="flex items-center justify-center h-full text-text-muted text-[12px]">
              {running ? "Listening for signals..." : "Light the beacon to see signals"}
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

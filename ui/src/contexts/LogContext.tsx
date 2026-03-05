import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";

export interface LogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
}

const MAX_LOG_ENTRIES = 500;

interface LogContextValue {
  logs: LogEntry[];
  clearLogs: () => void;
}

const LogContext = createContext<LogContextValue>({
  logs: [],
  clearLogs: () => {},
});

export function useGatewayLogs() {
  return useContext(LogContext);
}

export function LogProvider({ children }: { children: ReactNode }) {
  const [logs, setLogs] = useState<LogEntry[]>([]);

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

  const clearLogs = useCallback(() => setLogs([]), []);

  return (
    <LogContext.Provider value={{ logs, clearLogs }}>
      {children}
    </LogContext.Provider>
  );
}

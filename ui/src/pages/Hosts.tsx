import { useEffect, useState } from "react";
import StatusBadge from "../components/StatusBadge";
import type { Status } from "../components/StatusBadge";
import { getStatus, connectHost, disconnectHost, type HostStatus } from "../lib/tauri";

function Hosts() {
  const [hosts, setHosts] = useState<HostStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const status = await getStatus();
      setHosts(status.hosts);
    } catch {
      setHosts([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { refresh(); }, []);

  const handleToggleConnect = async (host: string, connected: boolean) => {
    try {
      if (connected) {
        await disconnectHost(host);
      } else {
        await connectHost(host);
      }
      refresh();
    } catch (e) {
      setError(String(e));
      setTimeout(() => setError(null), 4000);
    }
  };

  const hostStatus = (h: HostStatus): Status => {
    if (h.connected) return "connected";
    if (h.config_exists) return "detected";
    return "not_found";
  };

  if (loading) {
    return (
      <div className="p-8">
        <div className="space-y-3">
          {[1, 2, 3, 4].map((i) => (
            <div key={i} className="h-16 rounded-lg bg-bg-element animate-pulse" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="p-8 max-w-4xl">
      {/* Header */}
      <div className="flex items-center justify-between mb-8">
        <div>
          <h1 className="text-lg font-semibold text-text-primary">Ports</h1>
          <p className="text-[13px] text-text-secondary mt-0.5">
            Link hosts to sync your fleet through the lighthouse
          </p>
        </div>
      </div>

      {/* Error toast */}
      {error && (
        <div className="mb-4 px-3 py-2 rounded-md text-[13px] bg-red-muted text-red border border-red/20 animate-fade-in">
          {error}
        </div>
      )}

      {/* Host list */}
      <div className="space-y-2">
        {hosts.map((h) => (
          <div
            key={h.name}
            className="stagger-item flex items-center justify-between p-4 rounded-lg bg-bg-element border border-border-subtle hover:border-border-default transition-colors duration-150"
          >
            <div>
              <div className="text-[13px] font-medium text-text-primary">{h.display_name}</div>
              <div className="text-[12px] text-text-muted font-mono mt-0.5">
                {h.config_path}
              </div>
              {h.connected && (
                <div className="text-[12px] text-text-secondary mt-1">
                  {h.server_count} server{h.server_count !== 1 ? "s" : ""} synced
                </div>
              )}
            </div>
            <div className="flex items-center gap-3 shrink-0">
              <StatusBadge status={hostStatus(h)} />
              <button
                onClick={() => handleToggleConnect(h.name, h.connected)}
                className="px-2.5 py-1 rounded-md text-[12px] border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors duration-150 whitespace-nowrap"
              >
                {h.connected ? "Cast Off" : "Link"}
              </button>
            </div>
          </div>
        ))}
      </div>

      {/* How sync works */}
      <div className="mt-10 p-4 rounded-lg bg-bg-element border border-border-subtle">
        <h2 className="text-[13px] font-medium text-text-primary mb-2.5">How syncing works</h2>
        <ul className="text-[12px] text-text-secondary space-y-1.5 list-none">
          <li className="flex gap-2">
            <span className="text-text-muted select-none">-</span>
            Harbor reads the manifest at <code className="px-1 py-0.5 rounded bg-bg-app font-mono text-[11px]">~/.harbor/config.toml</code> as the source of truth
          </li>
          <li className="flex gap-2">
            <span className="text-text-muted select-none">-</span>
            Linked hosts receive a single <code className="px-1 py-0.5 rounded bg-bg-app font-mono text-[11px]">harbor relay</code> entry that routes through the lighthouse
          </li>
          <li className="flex gap-2">
            <span className="text-text-muted select-none">-</span>
            Servers are automatically synced when you dock, undock, or toggle
          </li>
          <li className="flex gap-2">
            <span className="text-text-muted select-none">-</span>
            Tool filters, vault secrets, and hot reload are handled by the lighthouse at runtime
          </li>
          <li className="flex gap-2">
            <span className="text-text-muted select-none">-</span>
            Existing non-Harbor entries in host configs are preserved
          </li>
        </ul>
      </div>
    </div>
  );
}

export default Hosts;

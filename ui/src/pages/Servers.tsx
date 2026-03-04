import { useEffect, useState } from "react";
import { Plus, X, Trash2, Zap, Lock, ChevronDown, ChevronRight, RefreshCw } from "lucide-react";
import StatusBadge from "../components/StatusBadge";
import type { Status } from "../components/StatusBadge";
import {
  getStatus,
  addServer,
  removeServer,
  toggleServer,
  getToolFilters,
  setToolAllowlist,
  setToolBlocklist,
  discoverTools,
  type ServerStatus,
  type ToolFilterInfo,
  type DiscoveredTool,
} from "../lib/tauri";

function ToolFilterPanel({ serverName }: { serverName: string }) {
  const [filters, setFilters] = useState<ToolFilterInfo | null>(null);
  const [tools, setTools] = useState<DiscoveredTool[]>([]);
  const [loading, setLoading] = useState(true);
  const [discovering, setDiscovering] = useState(false);
  const [discoveryError, setDiscoveryError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadFilters = async () => {
    try {
      const f = await getToolFilters(serverName);
      setFilters(f);
    } catch (e) {
      showError(String(e));
    }
  };

  const loadTools = async () => {
    setDiscovering(true);
    setDiscoveryError(null);
    try {
      const discovered = await discoverTools(serverName);
      setTools(discovered);
    } catch (e) {
      setDiscoveryError(String(e));
      setTools([]);
    } finally {
      setDiscovering(false);
    }
  };

  const load = async () => {
    setLoading(true);
    await Promise.all([loadFilters(), loadTools()]);
    setLoading(false);
  };

  useEffect(() => { load(); }, [serverName]);

  const showError = (msg: string) => {
    setError(msg);
    setTimeout(() => setError(null), 4000);
  };

  const isToolBlocked = (toolName: string): boolean => {
    const blocklist = filters?.tool_blocklist ?? [];
    return blocklist.includes(toolName);
  };

  const isToolAllowed = (toolName: string): boolean => {
    const allowlist = filters?.tool_allowlist;
    if (!allowlist) return true; // no allowlist = all allowed
    return allowlist.includes(toolName);
  };

  const handleToggleTool = async (toolName: string) => {
    const blocked = isToolBlocked(toolName);
    try {
      if (blocked) {
        // Unblock: remove from blocklist
        const updated = (filters?.tool_blocklist ?? []).filter((t) => t !== toolName);
        await setToolBlocklist(serverName, updated.length > 0 ? updated : null);
      } else {
        // Block: add to blocklist
        const existing = filters?.tool_blocklist ?? [];
        await setToolBlocklist(serverName, [...existing, toolName]);
      }
      await loadFilters();
    } catch (e) {
      showError(String(e));
    }
  };

  const handleBlockAll = async () => {
    try {
      const allNames = tools.map((t) => t.name);
      await setToolBlocklist(serverName, allNames);
      await loadFilters();
    } catch (e) {
      showError(String(e));
    }
  };

  const handleUnblockAll = async () => {
    try {
      await setToolBlocklist(serverName, null);
      await loadFilters();
    } catch (e) {
      showError(String(e));
    }
  };

  if (loading) {
    return <div className="h-8 rounded bg-bg-app animate-pulse mt-2" />;
  }

  const blockedCount = tools.filter((t) => isToolBlocked(t.name)).length;
  const totalCount = tools.length;

  return (
    <div className="mt-3 pt-3 border-t border-border-subtle">
      {error && (
        <div className="mb-2 px-2 py-1 rounded text-[11px] bg-red-muted text-red border border-red/20">
          {error}
        </div>
      )}

      {/* Header with refresh */}
      <div className="flex items-center justify-between mb-2">
        <div className="text-[11px] text-text-muted font-medium">
          Tools{totalCount > 0 && ` (${totalCount - blockedCount}/${totalCount} active)`}
        </div>
        <div className="flex items-center gap-2">
          {totalCount > 0 && (
            <>
              <button
                onClick={handleUnblockAll}
                className="text-[11px] text-text-muted hover:text-green transition-colors"
              >
                Enable all
              </button>
              <span className="text-text-muted text-[11px]">/</span>
              <button
                onClick={handleBlockAll}
                className="text-[11px] text-text-muted hover:text-red transition-colors"
              >
                Disable all
              </button>
            </>
          )}
          <button
            onClick={loadTools}
            disabled={discovering}
            className="p-0.5 rounded text-text-muted hover:text-text-primary transition-colors disabled:opacity-40"
            title="Refresh tools from gateway"
          >
            <RefreshCw className={`w-3 h-3 ${discovering ? "animate-spin" : ""}`} />
          </button>
        </div>
      </div>

      {/* Discovery error */}
      {discoveryError && (
        <div className="mb-2 px-2 py-1.5 rounded text-[11px] bg-bg-app border border-border-default text-text-secondary">
          Could not discover tools: {discoveryError}
        </div>
      )}

      {/* Tool list */}
      {totalCount > 0 ? (
        <div className="space-y-0.5">
          {tools.map((tool) => {
            const blocked = isToolBlocked(tool.name);
            return (
              <button
                key={tool.name}
                onClick={() => handleToggleTool(tool.name)}
                className={`w-full flex items-start gap-2 px-2 py-1.5 rounded text-left transition-colors duration-100 ${
                  blocked
                    ? "opacity-50 hover:opacity-70"
                    : "hover:bg-bg-hover"
                }`}
              >
                <div
                  className={`mt-0.5 w-3.5 h-3.5 rounded border shrink-0 flex items-center justify-center transition-colors ${
                    blocked
                      ? "border-border-default bg-bg-app"
                      : "border-accent bg-accent"
                  }`}
                >
                  {!blocked && (
                    <svg className="w-2.5 h-2.5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                    </svg>
                  )}
                </div>
                <div className="min-w-0">
                  <div className={`text-[12px] font-mono ${blocked ? "text-text-muted line-through" : "text-text-primary"}`}>
                    {tool.name}
                  </div>
                  {tool.description && (
                    <div className="text-[11px] text-text-muted truncate">
                      {tool.description}
                    </div>
                  )}
                </div>
              </button>
            );
          })}
        </div>
      ) : !discoveryError ? (
        <div className="text-[11px] text-text-muted">
          No tools discovered for this server.
        </div>
      ) : null}

      {/* Host overrides summary */}
      {Object.keys(filters?.tool_hosts ?? {}).length > 0 && (
        <div className="mt-2 pt-2 border-t border-border-subtle">
          <div className="text-[11px] text-text-muted font-medium mb-1">Host overrides</div>
          {Object.entries(filters!.tool_hosts).map(([host, hostTools]) => (
            <div key={host} className="text-[11px] text-text-secondary font-mono">
              <span className="text-yellow">{host}</span>: {hostTools.join(", ")}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function Servers() {
  const [servers, setServers] = useState<ServerStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAdd, setShowAdd] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedServer, setExpandedServer] = useState<string | null>(null);

  // Add form state
  const [newName, setNewName] = useState("");
  const [newCommand, setNewCommand] = useState("");
  const [newArgs, setNewArgs] = useState("");
  const [newEnvKey, setNewEnvKey] = useState("");
  const [newEnvVal, setNewEnvVal] = useState("");
  const [envPairs, setEnvPairs] = useState<[string, string][]>([]);

  const refresh = async () => {
    try {
      const status = await getStatus();
      setServers(status.servers);
    } catch {
      setServers([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { refresh(); }, []);

  const showError = (msg: string) => {
    setError(msg);
    setTimeout(() => setError(null), 4000);
  };

  const handleAdd = async () => {
    if (!newName || !newCommand) return;
    const args = newArgs.split(/\s+/).filter(Boolean);
    const env: Record<string, string> = {};
    envPairs.forEach(([k, v]) => { env[k] = v; });

    try {
      await addServer(newName, newCommand, args, env);
      setShowAdd(false);
      setNewName(""); setNewCommand(""); setNewArgs("");
      setEnvPairs([]);
      refresh();
    } catch (e) {
      showError(String(e));
    }
  };

  const handleRemove = async (name: string) => {
    try {
      await removeServer(name);
      if (expandedServer === name) setExpandedServer(null);
      refresh();
    } catch (e) {
      showError(String(e));
    }
  };

  const handleToggle = async (name: string, enabled: boolean) => {
    try {
      await toggleServer(name, !enabled);
      refresh();
    } catch (e) {
      showError(String(e));
    }
  };

  const addEnvPair = () => {
    if (newEnvKey) {
      setEnvPairs([...envPairs, [newEnvKey, newEnvVal]]);
      setNewEnvKey(""); setNewEnvVal("");
    }
  };

  if (loading) {
    return (
      <div className="p-8">
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
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
          <h1 className="text-lg font-semibold text-text-primary">Fleet</h1>
          <p className="text-[13px] text-text-secondary mt-0.5">
            Manage your fleet of docked MCP ships
          </p>
        </div>
        <button
          onClick={() => setShowAdd(!showAdd)}
          className={`inline-flex items-center gap-2 px-3 py-1.5 rounded-md text-[13px] font-medium transition-colors duration-150 ${
            showAdd
              ? "bg-bg-element text-text-secondary border border-border-default hover:bg-bg-hover"
              : "bg-accent text-white hover:bg-accent-hover"
          }`}
        >
          {showAdd ? (
            <>
              <X className="w-3.5 h-3.5" />
              Cancel
            </>
          ) : (
            <>
              <Plus className="w-3.5 h-3.5" />
              Dock Ship
            </>
          )}
        </button>
      </div>

      {/* Error toast */}
      {error && (
        <div className="mb-4 px-3 py-2 rounded-md text-[13px] bg-red-muted text-red border border-red/20 animate-fade-in">
          {error}
        </div>
      )}

      {/* Add server form */}
      {showAdd && (
        <div className="mb-6 p-4 rounded-lg bg-bg-element border border-border-subtle animate-fade-in">
          <div className="grid grid-cols-2 gap-3 mb-3">
            <input
              placeholder="Ship name"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              className="px-3 py-2 rounded-md text-[13px] bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150"
            />
            <input
              placeholder="Command (e.g. npx)"
              value={newCommand}
              onChange={(e) => setNewCommand(e.target.value)}
              className="px-3 py-2 rounded-md text-[13px] bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150"
            />
          </div>
          <input
            placeholder="Cargo (space-separated, e.g. -y @mcp/server-github)"
            value={newArgs}
            onChange={(e) => setNewArgs(e.target.value)}
            className="w-full px-3 py-2 rounded-md text-[13px] bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150 mb-3"
          />

          {/* Env vars */}
          <div className="flex gap-2 mb-2">
            <input
              placeholder="ENV_KEY"
              value={newEnvKey}
              onChange={(e) => setNewEnvKey(e.target.value)}
              className="flex-1 px-3 py-2 rounded-md text-[13px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150"
            />
            <input
              placeholder="provision (or vault:key_name)"
              value={newEnvVal}
              onChange={(e) => setNewEnvVal(e.target.value)}
              className="flex-1 px-3 py-2 rounded-md text-[13px] bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150"
            />
            <button
              onClick={addEnvPair}
              className="px-3 py-2 rounded-md text-[13px] bg-bg-app border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors duration-150"
            >
              + Env
            </button>
          </div>
          {envPairs.length > 0 && (
            <div className="flex flex-wrap gap-2 mb-3">
              {envPairs.map(([k, v], i) => (
                <span key={i} className="inline-flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-full bg-bg-active text-text-secondary font-mono">
                  {k}={v.startsWith("vault:") ? <Lock className="w-3 h-3" /> : "***"}
                </span>
              ))}
            </div>
          )}
          <button
            onClick={handleAdd}
            disabled={!newName || !newCommand}
            className="px-4 py-2 rounded-md text-[13px] font-medium bg-accent text-white hover:bg-accent-hover disabled:opacity-40 disabled:cursor-not-allowed transition-colors duration-150"
          >
            Dock Ship
          </button>
        </div>
      )}

      {/* Server list */}
      {servers.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 animate-fade-in">
          <div className="w-12 h-12 rounded-xl bg-bg-element border border-border-subtle flex items-center justify-center mb-4">
            <Zap className="w-6 h-6 text-text-muted" />
          </div>
          <p className="text-sm font-medium text-text-primary mb-1">The docks are empty</p>
          <p className="text-[13px] text-text-secondary max-w-xs text-center">
            Dock your first ship to start managing your fleet across all ports.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {servers.map((s) => (
            <div
              key={s.name}
              className="stagger-item rounded-lg bg-bg-element border border-border-subtle hover:border-border-default transition-colors duration-150"
            >
              <div className="flex items-center justify-between p-4">
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => setExpandedServer(expandedServer === s.name ? null : s.name)}
                    className="p-0.5 rounded text-text-muted hover:text-text-primary transition-colors"
                  >
                    {expandedServer === s.name ? (
                      <ChevronDown className="w-3.5 h-3.5" />
                    ) : (
                      <ChevronRight className="w-3.5 h-3.5" />
                    )}
                  </button>
                  <div>
                    <div className="text-[13px] font-medium text-text-primary">{s.name}</div>
                    <div className="text-[12px] text-text-muted font-mono mt-0.5">
                      {s.command}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-3">
                  <button
                    onClick={() => handleToggle(s.name, s.enabled)}
                    title={s.enabled ? "Enabled" : "Disabled"}
                    className={`relative w-7 h-4 rounded-full shrink-0 transition-colors duration-300 ${
                      s.enabled ? "bg-emerald-400" : "bg-text-muted/30"
                    }`}
                  >
                    <span
                      className={`absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-white shadow-sm transition-transform duration-300 ${
                        s.enabled ? "translate-x-3" : "translate-x-0"
                      }`}
                    />
                  </button>
                  <button
                    onClick={() => handleRemove(s.name)}
                    className="p-1 rounded-md text-text-muted hover:text-red hover:bg-red-muted transition-colors duration-150"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>

              {/* Tool filter panel */}
              {expandedServer === s.name && (
                <div className="px-4 pb-4 animate-fade-in">
                  <ToolFilterPanel serverName={s.name} />
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default Servers;

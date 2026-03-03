import { useEffect, useState } from "react";
import { Plus, X, Trash2, Zap, Lock } from "lucide-react";
import StatusBadge from "../components/StatusBadge";
import type { Status } from "../components/StatusBadge";
import { getStatus, addServer, removeServer, toggleServer, type ServerStatus } from "../lib/tauri";

function Servers() {
  const [servers, setServers] = useState<ServerStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAdd, setShowAdd] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
              className="stagger-item flex items-center justify-between p-4 rounded-lg bg-bg-element border border-border-subtle hover:border-border-default transition-colors duration-150"
            >
              <div>
                <div className="text-[13px] font-medium text-text-primary">{s.name}</div>
                <div className="text-[12px] text-text-muted font-mono mt-0.5">
                  {s.command}
                </div>
              </div>
              <div className="flex items-center gap-3">
                <StatusBadge status={(s.running ? "running" : s.enabled ? "enabled" : "disabled") as Status} />
                <button
                  onClick={() => handleToggle(s.name, s.enabled)}
                  className="px-2.5 py-1 rounded-md text-[12px] border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors duration-150"
                >
                  {s.enabled ? "Moor" : "Rig"}
                </button>
                <button
                  onClick={() => handleRemove(s.name)}
                  className="p-1 rounded-md text-text-muted hover:text-red hover:bg-red-muted transition-colors duration-150"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default Servers;

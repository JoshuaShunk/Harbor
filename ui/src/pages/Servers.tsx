import { useEffect, useState } from "react";
import { Plus, X, Trash2, Zap, Lock, ChevronDown, ChevronRight, RefreshCw, Globe, Monitor, FolderOpen, FileText, Search } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
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
  getServerExtraArgs,
  setServerExtraArgs,
  getServerArgs,
  setServerArgs,
  getConfigSchema,
  vaultSet,
  type ServerStatus,
  type ToolFilterInfo,
  type DiscoveredTool,
  type ServerExtraArgsInfo,
  type ConfigSchemaResponse,
  type ConfigSchemaArg,
  type ConfigSchemaEnvVar,
} from "../lib/tauri";

function ToolFilterPanel({ serverName }: { serverName: string }) {
  const [filters, setFilters] = useState<ToolFilterInfo | null>(null);
  const [tools, setTools] = useState<DiscoveredTool[]>([]);
  const [loading, setLoading] = useState(true);
  const [discovering, setDiscovering] = useState(false);
  const [discoveryError, setDiscoveryError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [toolSearch, setToolSearch] = useState("");

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

  const TOOL_DISPLAY_LIMIT = 10;

  const blockedCount = tools.filter((t) => isToolBlocked(t.name)).length;
  const totalCount = tools.length;

  const filteredTools = toolSearch
    ? tools.filter((t) =>
        t.name.toLowerCase().includes(toolSearch.toLowerCase()) ||
        t.description?.toLowerCase().includes(toolSearch.toLowerCase())
      )
    : tools;
  const displayedTools = filteredTools.slice(0, TOOL_DISPLAY_LIMIT);
  const hiddenCount = filteredTools.length - displayedTools.length;

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

      {/* Search (shown when >10 tools) */}
      {totalCount > TOOL_DISPLAY_LIMIT && (
        <div className="relative mb-2">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-text-muted" />
          <input
            type="text"
            value={toolSearch}
            onChange={(e) => setToolSearch(e.target.value)}
            placeholder={`Search ${totalCount} tools...`}
            className="w-full pl-6 pr-2 py-1.5 rounded bg-bg-app border border-border-default text-[11px] text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent"
          />
        </div>
      )}

      {/* Tool list */}
      {totalCount > 0 ? (
        <div className="space-y-0.5">
          {displayedTools.map((tool) => {
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
          {hiddenCount > 0 && (
            <div className="px-2 py-1.5 text-[11px] text-text-muted">
              +{hiddenCount} more tool{hiddenCount !== 1 ? "s" : ""}{toolSearch ? " matching search" : ""}
            </div>
          )}
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

/** Args that are runtime boilerplate, not user-configurable */
function isBoilerplateArg(arg: string): boolean {
  if (arg === "-y" || arg === "--yes") return true;
  if (arg.startsWith("@") && arg.includes("/")) return true; // npm scoped package
  if (arg.startsWith("mcp-server-") || arg.startsWith("mcp_server_")) return true; // pypi package
  return false;
}

function ServerArgsPanel({ serverName }: { serverName: string }) {
  const [editableArgs, setEditableArgs] = useState<string[]>([]);
  const [boilerplateArgs, setBoilerplateArgs] = useState<string[]>([]);
  const [nativeInfo, setNativeInfo] = useState<ServerExtraArgsInfo | null>(null);
  const [isNative, setIsNative] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [newArg, setNewArg] = useState("");
  const [textValue, setTextValue] = useState("");

  const load = async () => {
    try {
      const extraInfo = await getServerExtraArgs(serverName).catch(() => null);
      setNativeInfo(extraInfo);

      const native = extraInfo && extraInfo.extra_args_kind !== "none";
      setIsNative(!!native);

      if (native) {
        // Native server: only show the extra args (user-configurable)
        setEditableArgs(extraInfo!.extra_args);
        setBoilerplateArgs([]);
        if (extraInfo!.extra_args_kind === "text") {
          setTextValue(extraInfo!.extra_args[0] ?? "");
        }
      } else {
        // Non-native: filter out boilerplate (runtime flags, package names)
        const allArgs = await getServerArgs(serverName);
        const boilerplate = allArgs.filter(isBoilerplateArg);
        const userArgs = allArgs.filter(a => !isBoilerplateArg(a));
        setBoilerplateArgs(boilerplate);
        setEditableArgs(userArgs);
      }
      setDirty(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { load(); }, [serverName]);

  if (loading) return <div className="h-6 rounded bg-bg-app animate-pulse mt-2" />;

  const kind = nativeInfo?.extra_args_kind ?? "none";

  const handlePickFolder = async () => {
    const result = await open({ directory: true, multiple: true });
    if (result) {
      const selected = Array.isArray(result) ? result : [result];
      const updated = [...editableArgs, ...selected.filter((p) => !editableArgs.includes(p))];
      setEditableArgs(updated);
      setDirty(true);
    }
  };

  const handlePickFile = async () => {
    const result = await open({ directory: false, multiple: false });
    if (result) {
      const file = Array.isArray(result) ? result[0] : result;
      setEditableArgs([file]);
      setDirty(true);
    }
  };

  const removeArg = (idx: number) => {
    setEditableArgs(editableArgs.filter((_, i) => i !== idx));
    setDirty(true);
  };

  const addArg = () => {
    if (newArg.trim()) {
      setEditableArgs([...editableArgs, newArg.trim()]);
      setNewArg("");
      setDirty(true);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      if (isNative) {
        // Save only the extra args portion (backend preserves catalog defaults)
        const args = kind === "text" ? (textValue ? [textValue] : []) : editableArgs;
        await setServerExtraArgs(serverName, args);
      } else {
        // Prepend boilerplate args back before saving
        await setServerArgs(serverName, [...boilerplateArgs, ...editableArgs]);
      }
      setDirty(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  // Determine the section label
  const label = isNative
    ? (nativeInfo?.extra_args_label ?? "Configuration")
    : "Arguments";

  return (
    <div className="mt-3 pt-3 border-t border-border-subtle">
      <div className="text-[11px] text-text-muted font-medium mb-2">
        {label}
      </div>

      {error && (
        <div className="mb-2 px-2 py-1 rounded text-[11px] bg-red-muted text-red border border-red/20">
          {error}
        </div>
      )}

      {/* Text input mode (e.g. postgres connection string) */}
      {isNative && kind === "text" && (
        <input
          value={textValue}
          onChange={(e) => { setTextValue(e.target.value); setDirty(true); }}
          placeholder={nativeInfo?.extra_args_placeholder ?? ""}
          className="w-full px-2.5 py-1.5 rounded-md text-[12px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors"
        />
      )}

      {/* List mode (directories, files, or generic args) */}
      {kind !== "text" && (
        <>
          {editableArgs.length > 0 && (
            <div className="space-y-1">
              {editableArgs.map((arg, i) => (
                <div key={i} className="flex items-center gap-2 px-2 py-1.5 rounded bg-bg-app border border-border-default group">
                  {kind === "directories" ? (
                    <FolderOpen className="w-3.5 h-3.5 text-accent shrink-0" />
                  ) : kind === "file" ? (
                    <FileText className="w-3.5 h-3.5 text-accent shrink-0" />
                  ) : (
                    <span className="text-[10px] text-text-muted w-3.5 text-center shrink-0 font-mono">{i}</span>
                  )}
                  <span className="text-[12px] font-mono text-text-secondary truncate flex-1">{arg}</span>
                  <button
                    onClick={() => removeArg(i)}
                    className="p-0.5 rounded text-text-muted hover:text-red transition-colors shrink-0 opacity-0 group-hover:opacity-100"
                  >
                    <X className="w-3 h-3" />
                  </button>
                </div>
              ))}
            </div>
          )}

          <div className="flex items-center gap-2 mt-2">
            {kind === "directories" && (
              <button
                onClick={handlePickFolder}
                className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-[12px] font-medium bg-bg-app border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors"
              >
                <FolderOpen className="w-3.5 h-3.5" />
                Add Folder
              </button>
            )}
            {kind === "file" && (
              <button
                onClick={handlePickFile}
                className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-[12px] font-medium bg-bg-app border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors"
              >
                <FileText className="w-3.5 h-3.5" />
                {editableArgs.length > 0 ? "Change File" : "Choose File"}
              </button>
            )}
            <div className="flex items-center gap-1.5 flex-1">
              <input
                value={newArg}
                onChange={(e) => setNewArg(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addArg()}
                placeholder="Add argument..."
                className="flex-1 px-2 py-1.5 rounded-md text-[12px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors"
              />
              <button
                onClick={addArg}
                disabled={!newArg.trim()}
                className="px-2 py-1.5 rounded-md text-[12px] font-medium bg-bg-app border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover disabled:opacity-40 transition-colors"
              >
                <Plus className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        </>
      )}

      {dirty && (
        <button
          onClick={handleSave}
          disabled={saving}
          className="mt-2 px-3 py-1.5 rounded-md text-[12px] font-medium bg-accent text-white hover:bg-accent-hover disabled:opacity-40 transition-colors"
        >
          {saving ? "Saving..." : "Save"}
        </button>
      )}
    </div>
  );
}

function RegistryConfigPanel({ serverName }: { serverName: string }) {
  const [schema, setSchema] = useState<ConfigSchemaResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [argValues, setArgValues] = useState<Record<string, string[]>>({});
  const [envValues, setEnvValues] = useState<Record<string, string>>({});
  const [currentArgs, setCurrentArgs] = useState<string[]>([]);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const [schemaRes, args] = await Promise.all([
          getConfigSchema(serverName),
          getServerArgs(serverName),
        ]);
        setSchema(schemaRes);
        setCurrentArgs(args);

        // Initialize arg values from current args
        if (schemaRes.args) {
          const vals: Record<string, string[]> = {};
          for (const spec of schemaRes.args) {
            if (spec.arg_type === "positional") {
              // Positional args come after the package identifier in the args array
              // Find matching values from current args
              const matches = args.filter(a =>
                !a.startsWith("-") && a !== "-y" &&
                !a.startsWith("@") && !a.startsWith("mcp-server-") && !a.startsWith("mcp_server_")
              );
              if (matches.length > 0) vals[spec.name] = matches;
            } else if (spec.arg_type === "named") {
              const prefix = `--${spec.name}=`;
              const matches = args
                .filter(a => a.startsWith(prefix))
                .map(a => a.slice(prefix.length));
              if (matches.length > 0) vals[spec.name] = matches;
            }
          }
          setArgValues(vals);
        }
      } catch {
        setSchema(null);
      } finally {
        setLoading(false);
      }
    })();
  }, [serverName]);

  if (loading) return <div className="h-6 rounded bg-bg-app animate-pulse mt-2" />;
  if (!schema || (!schema.args?.length && !schema.env_vars?.length)) return null;

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      // Rebuild args: keep existing non-schema args, replace schema-defined ones
      const schemaArgNames = new Set((schema.args ?? []).map(a => a.name));
      // Keep args that aren't managed by schema (e.g., "-y", package identifier)
      const kept = currentArgs.filter(a => {
        // Keep flags and package identifiers
        if (a === "-y") return true;
        if (a.startsWith("@") || a.startsWith("mcp-server-") || a.startsWith("mcp_server_")) return true;
        // Remove named args that match schema
        for (const name of schemaArgNames) {
          if (a.startsWith(`--${name}=`) || a === `--${name}`) return false;
        }
        // Keep unrecognized args
        return true;
      });

      // Add schema-managed args back with new values
      const newArgs = [...kept];
      for (const spec of (schema.args ?? [])) {
        const vals = argValues[spec.name] ?? [];
        if (spec.arg_type === "named") {
          for (const v of vals) {
            if (v) newArgs.push(`--${spec.name}=${v}`);
          }
        } else if (spec.arg_type === "positional") {
          newArgs.push(...vals.filter(Boolean));
        }
      }

      await setServerArgs(serverName, newArgs);

      // Save env vars to vault or config
      for (const [key, value] of Object.entries(envValues)) {
        if (value) {
          // Store secret env vars in vault
          const spec = schema.env_vars?.find(e => e.name === key);
          if (spec?.is_secret) {
            await vaultSet(key.toLowerCase(), value);
          }
        }
      }

      setDirty(false);
      setCurrentArgs(newArgs);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const updateArgValue = (name: string, index: number, value: string) => {
    setArgValues(prev => {
      const arr = [...(prev[name] ?? [])];
      arr[index] = value;
      return { ...prev, [name]: arr };
    });
    setDirty(true);
  };

  const addArgValue = (name: string) => {
    setArgValues(prev => ({
      ...prev,
      [name]: [...(prev[name] ?? []), ""],
    }));
    setDirty(true);
  };

  const removeArgValue = (name: string, index: number) => {
    setArgValues(prev => ({
      ...prev,
      [name]: (prev[name] ?? []).filter((_, i) => i !== index),
    }));
    setDirty(true);
  };

  return (
    <div className="mt-3 pt-3 border-t border-border-subtle">
      <div className="flex items-center gap-2 mb-2">
        <span className="text-[11px] text-text-muted font-medium">Configuration</span>
        {schema.registry_name && (
          <span className="text-[10px] text-text-muted px-1.5 py-0.5 rounded bg-bg-app border border-border-default">
            via {schema.registry_name}
          </span>
        )}
      </div>

      {error && (
        <div className="mb-2 px-2 py-1 rounded text-[11px] bg-red-muted text-red border border-red/20">
          {error}
        </div>
      )}

      {/* Package arguments */}
      {(schema.args ?? []).length > 0 && (
        <div className="space-y-3 mb-3">
          {(schema.args ?? []).map((spec) => (
            <ArgField
              key={spec.name}
              spec={spec}
              values={argValues[spec.name] ?? []}
              onChange={(idx, val) => updateArgValue(spec.name, idx, val)}
              onAdd={() => addArgValue(spec.name)}
              onRemove={(idx) => removeArgValue(spec.name, idx)}
            />
          ))}
        </div>
      )}

      {/* Environment variables */}
      {(schema.env_vars ?? []).length > 0 && (
        <div className="space-y-2">
          <div className="text-[11px] text-text-muted font-medium">Environment Variables</div>
          {(schema.env_vars ?? []).map((ev) => (
            <EnvVarField
              key={ev.name}
              spec={ev}
              value={envValues[ev.name] ?? ""}
              onChange={(val) => {
                setEnvValues(prev => ({ ...prev, [ev.name]: val }));
                setDirty(true);
              }}
            />
          ))}
        </div>
      )}

      {dirty && (
        <button
          onClick={handleSave}
          disabled={saving}
          className="mt-2 px-3 py-1.5 rounded-md text-[12px] font-medium bg-accent text-white hover:bg-accent-hover disabled:opacity-40 transition-colors"
        >
          {saving ? "Saving..." : "Save"}
        </button>
      )}
    </div>
  );
}

function ArgField({
  spec,
  values,
  onChange,
  onAdd,
  onRemove,
}: {
  spec: ConfigSchemaArg;
  values: string[];
  onChange: (index: number, value: string) => void;
  onAdd: () => void;
  onRemove: (index: number) => void;
}) {
  const effectiveValues = values.length > 0 ? values : [""];

  const handlePickFolder = async (index: number) => {
    const result = await open({ directory: true, multiple: false });
    if (result) {
      const path = Array.isArray(result) ? result[0] : result;
      if (path) onChange(index, path);
    }
  };

  const handlePickFile = async (index: number) => {
    const result = await open({ directory: false, multiple: false });
    if (result) {
      const path = Array.isArray(result) ? result[0] : result;
      if (path) onChange(index, path);
    }
  };

  const isFilePath = spec.format === "filepath" || spec.format === "path";

  return (
    <div>
      <label className="flex items-center gap-1.5 text-[12px] font-medium text-text-primary mb-1">
        {spec.name}
        {spec.is_required && <span className="text-red text-[10px]">required</span>}
        {spec.is_secret && <Lock className="w-3 h-3 text-text-muted" />}
      </label>
      {spec.description && (
        <p className="text-[11px] text-text-muted mb-1">{spec.description}</p>
      )}

      {spec.choices ? (
        // Dropdown for choices
        <select
          value={effectiveValues[0] ?? spec.default ?? ""}
          onChange={(e) => onChange(0, e.target.value)}
          className="w-full px-2.5 py-1.5 rounded-md text-[12px] bg-bg-app border border-border-default text-text-primary focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors"
        >
          <option value="">Select...</option>
          {spec.choices.map(c => (
            <option key={c} value={c}>{c}</option>
          ))}
        </select>
      ) : spec.is_repeated ? (
        // Multi-value list
        <div className="space-y-1">
          {effectiveValues.map((val, i) => (
            <div key={i} className="flex items-center gap-1.5">
              <input
                value={val}
                onChange={(e) => onChange(i, e.target.value)}
                placeholder={spec.placeholder ?? spec.value_hint ?? `${spec.name}...`}
                className="flex-1 px-2.5 py-1.5 rounded-md text-[12px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors"
              />
              {isFilePath && (
                <button
                  onClick={() => spec.format === "filepath" ? handlePickFile(i) : handlePickFolder(i)}
                  className="p-1.5 rounded-md bg-bg-app border border-border-default text-text-muted hover:text-text-primary hover:border-border-hover transition-colors"
                >
                  {spec.format === "filepath" ? <FileText className="w-3.5 h-3.5" /> : <FolderOpen className="w-3.5 h-3.5" />}
                </button>
              )}
              {effectiveValues.length > 1 && (
                <button
                  onClick={() => onRemove(i)}
                  className="p-1 rounded text-text-muted hover:text-red transition-colors"
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>
          ))}
          <button
            onClick={onAdd}
            className="inline-flex items-center gap-1 text-[11px] text-text-muted hover:text-text-secondary transition-colors"
          >
            <Plus className="w-3 h-3" />
            Add another
          </button>
        </div>
      ) : (
        // Single value input
        <div className="flex items-center gap-1.5">
          <input
            type={spec.is_secret ? "password" : "text"}
            value={effectiveValues[0] ?? ""}
            onChange={(e) => onChange(0, e.target.value)}
            placeholder={spec.placeholder ?? spec.value_hint ?? spec.default ?? `${spec.name}...`}
            className="flex-1 px-2.5 py-1.5 rounded-md text-[12px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors"
          />
          {isFilePath && (
            <button
              onClick={() => spec.format === "filepath" ? handlePickFile(0) : handlePickFolder(0)}
              className="p-1.5 rounded-md bg-bg-app border border-border-default text-text-muted hover:text-text-primary hover:border-border-hover transition-colors"
            >
              {spec.format === "filepath" ? <FileText className="w-3.5 h-3.5" /> : <FolderOpen className="w-3.5 h-3.5" />}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function EnvVarField({
  spec,
  value,
  onChange,
}: {
  spec: ConfigSchemaEnvVar;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div>
      <label className="flex items-center gap-1.5 text-[12px] font-medium text-text-primary mb-0.5">
        {spec.name}
        {spec.is_required && <span className="text-red text-[10px]">required</span>}
        {spec.is_secret && <Lock className="w-3 h-3 text-text-muted" />}
      </label>
      <input
        type={spec.is_secret ? "password" : "text"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={spec.is_secret ? `vault:${spec.name.toLowerCase()}` : (spec.default ?? spec.name)}
        className="w-full px-2.5 py-1.5 rounded-md text-[12px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors"
      />
      {spec.description && (
        <p className="text-[11px] text-text-muted mt-0.5">{spec.description}</p>
      )}
    </div>
  );
}

function ServerConfigPanel({ serverName, source }: { serverName: string; source: string | null }) {
  const isNative = source?.startsWith("native:");

  return (
    <>
      {/* Registry config schema (available for all servers) */}
      <RegistryConfigSchemaSection serverName={serverName} />
      {/* Native servers get the ExtraArgs system, others get generic args editor */}
      {isNative ? (
        <ServerArgsPanel serverName={serverName} />
      ) : (
        <ServerArgsPanel serverName={serverName} />
      )}
    </>
  );
}

function RegistryConfigSchemaSection({ serverName }: { serverName: string }) {
  const [hasSchema, setHasSchema] = useState<boolean | null>(null);

  useEffect(() => {
    getConfigSchema(serverName)
      .then(res => {
        const has = !!((res.args && res.args.length > 0) || (res.env_vars && res.env_vars.length > 0));
        setHasSchema(has);
      })
      .catch(() => setHasSchema(false));
  }, [serverName]);

  if (hasSchema === null) return <div className="h-6 rounded bg-bg-app animate-pulse mt-2" />;
  if (!hasSchema) return null;

  return <RegistryConfigPanel serverName={serverName} />;
}

const inputClass = "px-3 py-2 rounded-md text-[13px] bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150";

function Servers() {
  const [servers, setServers] = useState<ServerStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAdd, setShowAdd] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedServer, setExpandedServer] = useState<string | null>(null);

  // Add form state
  const [addMode, setAddMode] = useState<"local" | "remote">("local");
  const [newName, setNewName] = useState("");
  // Local mode
  const [newCommand, setNewCommand] = useState("");
  const [newArgs, setNewArgs] = useState("");
  const [newEnvKey, setNewEnvKey] = useState("");
  const [newEnvVal, setNewEnvVal] = useState("");
  const [envPairs, setEnvPairs] = useState<[string, string][]>([]);
  // Remote mode
  const [newUrl, setNewUrl] = useState("");
  const [newHeaderKey, setNewHeaderKey] = useState("");
  const [newHeaderVal, setNewHeaderVal] = useState("");
  const [headerPairs, setHeaderPairs] = useState<[string, string][]>([]);

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

  const resetForm = () => {
    setNewName(""); setNewCommand(""); setNewArgs("");
    setNewEnvKey(""); setNewEnvVal(""); setEnvPairs([]);
    setNewUrl(""); setNewHeaderKey(""); setNewHeaderVal(""); setHeaderPairs([]);
  };

  const handleAdd = async () => {
    if (!newName) return;

    if (addMode === "local") {
      if (!newCommand) return;
      const args = newArgs.split(/\s+/).filter(Boolean);
      const env: Record<string, string> = {};
      envPairs.forEach(([k, v]) => { env[k] = v; });

      try {
        await addServer(newName, newCommand, args, env);
        setShowAdd(false);
        resetForm();
        refresh();
      } catch (e) {
        showError(String(e));
      }
    } else {
      if (!newUrl) return;
      const headers: Record<string, string> = {};
      headerPairs.forEach(([k, v]) => { headers[k] = v; });

      try {
        await addServer(
          newName,
          null,
          [],
          {},
          newUrl,
          Object.keys(headers).length > 0 ? headers : null,
        );
        setShowAdd(false);
        resetForm();
        refresh();
      } catch (e) {
        showError(String(e));
      }
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

  const addHeaderPair = () => {
    if (newHeaderKey) {
      setHeaderPairs([...headerPairs, [newHeaderKey, newHeaderVal]]);
      setNewHeaderKey(""); setNewHeaderVal("");
    }
  };

  const canSubmit = addMode === "local"
    ? !!(newName && newCommand)
    : !!(newName && newUrl);

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
          {/* Local / Remote toggle */}
          <div className="flex items-center gap-1 mb-4 p-0.5 rounded-md bg-bg-app border border-border-default w-fit">
            <button
              onClick={() => setAddMode("local")}
              className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded text-[12px] font-medium transition-colors duration-150 ${
                addMode === "local"
                  ? "bg-bg-element text-text-primary shadow-sm"
                  : "text-text-muted hover:text-text-secondary"
              }`}
            >
              <Monitor className="w-3.5 h-3.5" />
              Local
            </button>
            <button
              onClick={() => setAddMode("remote")}
              className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded text-[12px] font-medium transition-colors duration-150 ${
                addMode === "remote"
                  ? "bg-bg-element text-text-primary shadow-sm"
                  : "text-text-muted hover:text-text-secondary"
              }`}
            >
              <Globe className="w-3.5 h-3.5" />
              Remote
            </button>
          </div>

          {addMode === "local" ? (
            <>
              {/* Local server fields */}
              <div className="grid grid-cols-2 gap-3 mb-3">
                <input
                  placeholder="Ship name"
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  className={inputClass}
                />
                <input
                  placeholder="Command (e.g. npx)"
                  value={newCommand}
                  onChange={(e) => setNewCommand(e.target.value)}
                  className={inputClass}
                />
              </div>
              <input
                placeholder="Cargo (space-separated, e.g. -y @mcp/server-github)"
                value={newArgs}
                onChange={(e) => setNewArgs(e.target.value)}
                className={`w-full ${inputClass} mb-3`}
              />

              {/* Env vars */}
              <div className="flex gap-2 mb-2">
                <input
                  placeholder="ENV_KEY"
                  value={newEnvKey}
                  onChange={(e) => setNewEnvKey(e.target.value)}
                  className={`flex-1 font-mono ${inputClass}`}
                />
                <input
                  placeholder="provision (or vault:key_name)"
                  value={newEnvVal}
                  onChange={(e) => setNewEnvVal(e.target.value)}
                  className={`flex-1 ${inputClass}`}
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
            </>
          ) : (
            <>
              {/* Remote server fields */}
              <div className="grid grid-cols-2 gap-3 mb-3">
                <input
                  placeholder="Ship name"
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  className={inputClass}
                />
                <input
                  placeholder="URL (e.g. https://mcp.example.com/mcp)"
                  value={newUrl}
                  onChange={(e) => setNewUrl(e.target.value)}
                  className={inputClass}
                />
              </div>

              {/* Headers */}
              <div className="flex gap-2 mb-2">
                <input
                  placeholder="Header name (e.g. Authorization)"
                  value={newHeaderKey}
                  onChange={(e) => setNewHeaderKey(e.target.value)}
                  className={`flex-1 ${inputClass}`}
                />
                <input
                  placeholder="Value (or vault:key_name)"
                  value={newHeaderVal}
                  onChange={(e) => setNewHeaderVal(e.target.value)}
                  className={`flex-1 ${inputClass}`}
                />
                <button
                  onClick={addHeaderPair}
                  className="px-3 py-2 rounded-md text-[13px] bg-bg-app border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors duration-150"
                >
                  + Header
                </button>
              </div>
              {headerPairs.length > 0 && (
                <div className="flex flex-wrap gap-2 mb-3">
                  {headerPairs.map(([k, v], i) => (
                    <span key={i} className="inline-flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-full bg-bg-active text-text-secondary font-mono">
                      {k}: {v.startsWith("vault:") ? <Lock className="w-3 h-3" /> : "***"}
                    </span>
                  ))}
                </div>
              )}
            </>
          )}

          <button
            onClick={handleAdd}
            disabled={!canSubmit}
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
                  <div className="flex items-center gap-2">
                    {s.is_remote && (
                      <Globe className="w-3.5 h-3.5 text-accent shrink-0" />
                    )}
                    <div>
                      <div className="text-[13px] font-medium text-text-primary">{s.name}</div>
                      <div className="text-[12px] text-text-muted font-mono mt-0.5">
                        {s.command}
                      </div>
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

              {/* Expanded panels */}
              {expandedServer === s.name && (
                <div className="px-4 pb-4 animate-fade-in">
                  <ServerConfigPanel serverName={s.name} source={s.source} />
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

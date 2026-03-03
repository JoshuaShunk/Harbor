import { useEffect, useState } from "react";
import { Trash2, Shield, Globe, FolderCog, Info, RefreshCw, ArrowDownCircle, CheckCircle2, AlertCircle, Loader2, Anchor, ChevronDown, ChevronUp, Sun, Moon, Monitor } from "lucide-react";
import { getStatus, vaultSet, vaultDelete, vaultList, oauthListProviders, oauthRevokeCharter, oauthSetCustomCredentials, type HarborStatus, type OAuthProviderInfo } from "../lib/tauri";
import StatusBadge from "../components/StatusBadge";
import type { Status } from "../components/StatusBadge";
import { useUpdate } from "../contexts/UpdateContext";
import { useTheme, type ThemeChoice } from "../contexts/ThemeContext";

const themeOptions: { value: ThemeChoice; label: string; icon: typeof Sun }[] = [
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
  { value: "system", label: "System", icon: Monitor },
];

function AppearanceSection() {
  const { theme, setTheme } = useTheme();

  return (
    <section className="p-4 rounded-lg bg-bg-element border border-border-subtle">
      <div className="flex items-center gap-2 mb-3">
        <Sun className="w-4 h-4 text-text-muted" />
        <h2 className="text-[13px] font-medium text-text-primary">Appearance</h2>
      </div>
      <div className="flex gap-2">
        {themeOptions.map(({ value, label, icon: Icon }) => (
          <button
            key={value}
            onClick={() => setTheme(value)}
            className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-md text-[12px] font-medium border transition-colors duration-150 ${
              theme === value
                ? "bg-accent-muted border-accent text-accent"
                : "bg-bg-app border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover"
            }`}
          >
            <Icon className="w-3.5 h-3.5" />
            {label}
          </button>
        ))}
      </div>
    </section>
  );
}

function Settings() {
  const [status, setStatus] = useState<HarborStatus | null>(null);
  const [vaultKeys, setVaultKeys] = useState<string[]>([]);
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");
  const [vaultMsg, setVaultMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const [providers, setProviders] = useState<OAuthProviderInfo[]>([]);
  const [expandedProvider, setExpandedProvider] = useState<string | null>(null);
  const [customClientId, setCustomClientId] = useState("");
  const [customSecret, setCustomSecret] = useState("");

  const {
    status: updateStatus,
    currentVersion,
    availableVersion,
    progress: updateProgress,
    error: updateError,
    checkForUpdate,
    downloadAndInstall,
  } = useUpdate();

  useEffect(() => {
    getStatus().then(setStatus).catch(() => {});
    refreshVault();
    refreshProviders();
  }, []);

  const refreshProviders = async () => {
    try {
      setProviders(await oauthListProviders());
    } catch {
      setProviders([]);
    }
  };

  const handleRevoke = async (providerId: string) => {
    try {
      await oauthRevokeCharter(providerId);
      refreshProviders();
    } catch (e) {
      setVaultMsg({ text: String(e), isError: true });
      setTimeout(() => setVaultMsg(null), 4000);
    }
  };

  const handleSaveCustomCreds = async (providerId: string) => {
    if (!customClientId.trim()) return;
    try {
      await oauthSetCustomCredentials(providerId, customClientId.trim(), customSecret.trim() || undefined);
      setCustomClientId("");
      setCustomSecret("");
      setExpandedProvider(null);
      setVaultMsg({ text: `Custom papers for ${providerId} stowed`, isError: false });
      setTimeout(() => setVaultMsg(null), 3000);
    } catch (e) {
      setVaultMsg({ text: String(e), isError: true });
      setTimeout(() => setVaultMsg(null), 4000);
    }
  };

  const refreshVault = async () => {
    try {
      const keys = await vaultList();
      setVaultKeys(keys);
    } catch {
      setVaultKeys([]);
    }
  };

  const handleVaultSet = async () => {
    if (!newKey.trim()) return;
    try {
      await vaultSet(newKey.trim(), newValue);
      setNewKey("");
      setNewValue("");
      setVaultMsg({ text: `Secret "${newKey.trim()}" stowed in the chest`, isError: false });
      setTimeout(() => setVaultMsg(null), 3000);
      refreshVault();
    } catch (e) {
      setVaultMsg({ text: String(e), isError: true });
      setTimeout(() => setVaultMsg(null), 4000);
    }
  };

  const handleVaultDelete = async (key: string) => {
    try {
      await vaultDelete(key);
      setVaultMsg({ text: `Secret "${key}" tossed overboard`, isError: false });
      setTimeout(() => setVaultMsg(null), 3000);
      refreshVault();
    } catch (e) {
      setVaultMsg({ text: String(e), isError: true });
      setTimeout(() => setVaultMsg(null), 4000);
    }
  };

  return (
    <div className="p-8 max-w-4xl">
      {/* Header */}
      <div className="mb-8">
        <h1 className="text-lg font-semibold text-text-primary">Helm</h1>
        <p className="text-[13px] text-text-secondary mt-0.5">
          Chart your course and manage the treasure chest
        </p>
      </div>

      <div className="space-y-4">
        {/* Appearance */}
        <AppearanceSection />

        {/* Gateway */}
        <section className="p-4 rounded-lg bg-bg-element border border-border-subtle">
          <div className="flex items-center gap-2 mb-3">
            <Globe className="w-4 h-4 text-text-muted" />
            <h2 className="text-[13px] font-medium text-text-primary">Lighthouse</h2>
          </div>
          <div className="text-[12px] space-y-2.5">
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">Beacon port</span>
              <span className="text-text-primary tabular-nums">{status?.gateway_port ?? 3100}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">Beacon</span>
              <code className="px-1.5 py-0.5 rounded bg-bg-app font-mono text-[11px] text-text-primary">
                http://127.0.0.1:{status?.gateway_port ?? 3100}/mcp
              </code>
            </div>
          </div>
        </section>

        {/* Config location */}
        <section className="p-4 rounded-lg bg-bg-element border border-border-subtle">
          <div className="flex items-center gap-2 mb-3">
            <FolderCog className="w-4 h-4 text-text-muted" />
            <h2 className="text-[13px] font-medium text-text-primary">Charts</h2>
          </div>
          <div className="text-[12px] space-y-2.5">
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">Manifest</span>
              <code className="px-1.5 py-0.5 rounded bg-bg-app font-mono text-[11px] text-text-primary">
                ~/.harbor/config.toml
              </code>
            </div>
          </div>
        </section>

        {/* Updates — Dry Dock */}
        <section className="p-4 rounded-lg bg-bg-element border border-border-subtle">
          <div className="flex items-center gap-2 mb-3">
            <RefreshCw className="w-4 h-4 text-text-muted" />
            <h2 className="text-[13px] font-medium text-text-primary">Dry Dock</h2>
          </div>
          <div className="text-[12px] space-y-3">
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">Current version</span>
              <span className="text-text-primary tabular-nums">v{currentVersion}</span>
            </div>

            {updateStatus === "idle" && (
              <button
                onClick={checkForUpdate}
                className="w-full flex items-center justify-center gap-2 px-3 py-2 rounded-md text-[12px] font-medium bg-bg-app border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors duration-150"
              >
                <RefreshCw className="w-3.5 h-3.5" />
                Check for Updates
              </button>
            )}

            {updateStatus === "checking" && (
              <div className="flex items-center justify-center gap-2 px-3 py-2 text-text-secondary">
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                <span>Scanning the horizon...</span>
              </div>
            )}

            {updateStatus === "up-to-date" && (
              <div className="flex items-center gap-2 px-3 py-2 rounded-md bg-green-muted border border-green/20">
                <CheckCircle2 className="w-3.5 h-3.5 text-green" />
                <span className="text-green text-[12px]">Ship is seaworthy — you're on the latest</span>
              </div>
            )}

            {updateStatus === "available" && (
              <>
                <div className="flex justify-between items-center">
                  <span className="text-text-secondary">Available version</span>
                  <span className="text-accent font-medium tabular-nums">v{availableVersion}</span>
                </div>
                <button
                  onClick={downloadAndInstall}
                  className="w-full flex items-center justify-center gap-2 px-3 py-2 rounded-md text-[12px] font-medium bg-accent text-white hover:bg-accent-hover transition-colors duration-150"
                >
                  <ArrowDownCircle className="w-3.5 h-3.5" />
                  Update & Relaunch
                </button>
              </>
            )}

            {updateStatus === "downloading" && (
              <div className="space-y-2">
                <div className="flex items-center gap-2 text-text-secondary">
                  <Loader2 className="w-3.5 h-3.5 animate-spin text-accent" />
                  <span>Hauling cargo...</span>
                  {updateProgress && updateProgress.total > 0 && (
                    <span className="ml-auto tabular-nums text-text-muted">
                      {Math.round((updateProgress.downloaded / updateProgress.total) * 100)}%
                    </span>
                  )}
                </div>
                {updateProgress && updateProgress.total > 0 && (
                  <div className="w-full h-1.5 rounded-full bg-bg-app overflow-hidden">
                    <div
                      className="h-full bg-accent rounded-full transition-all duration-300"
                      style={{ width: `${Math.round((updateProgress.downloaded / updateProgress.total) * 100)}%` }}
                    />
                  </div>
                )}
              </div>
            )}

            {updateStatus === "ready" && (
              <div className="flex items-center gap-2 px-3 py-2 text-accent">
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                <span>Setting sail on new course...</span>
              </div>
            )}

            {updateStatus === "error" && (
              <div className="space-y-2">
                <div className="flex items-center gap-2 px-3 py-2 rounded-md bg-red-muted border border-red/20">
                  <AlertCircle className="w-3.5 h-3.5 text-red shrink-0" />
                  <span className="text-red text-[12px]">{updateError}</span>
                </div>
                <button
                  onClick={checkForUpdate}
                  className="w-full flex items-center justify-center gap-2 px-3 py-2 rounded-md text-[12px] font-medium bg-bg-app border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors duration-150"
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                  Try Again
                </button>
              </div>
            )}
          </div>
        </section>

        {/* Auth Vault */}
        <section className="p-4 rounded-lg bg-bg-element border border-border-subtle">
          <div className="flex items-center gap-2 mb-3">
            <Shield className="w-4 h-4 text-text-muted" />
            <h2 className="text-[13px] font-medium text-text-primary">Treasure Chest</h2>
          </div>
          <p className="text-[12px] text-text-secondary mb-3">
            Stow API keys and tokens in your OS keychain.
            Reference them with <code className="px-1 py-0.5 rounded bg-bg-app font-mono text-[11px]">vault:KEY_NAME</code>.
          </p>

          {/* Vault feedback */}
          {vaultMsg && (
            <div className={`mb-3 px-3 py-2 rounded-md text-[12px] border animate-fade-in ${
              vaultMsg.isError
                ? "bg-red-muted text-red border-red/20"
                : "bg-green-muted text-green border-green/20"
            }`}>
              {vaultMsg.text}
            </div>
          )}

          {/* Add secret form */}
          <div className="flex gap-2 mb-3">
            <input
              placeholder="KEY_NAME"
              value={newKey}
              onChange={(e) => setNewKey(e.target.value)}
              className="flex-1 px-2.5 py-1.5 rounded-md text-[12px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150"
            />
            <input
              placeholder="secret value"
              type="password"
              value={newValue}
              onChange={(e) => setNewValue(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleVaultSet()}
              className="flex-1 px-2.5 py-1.5 rounded-md text-[12px] bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150"
            />
            <button
              onClick={handleVaultSet}
              disabled={!newKey.trim()}
              className="px-3 py-1.5 rounded-md text-[12px] font-medium bg-accent text-white hover:bg-accent-hover disabled:opacity-40 disabled:cursor-not-allowed transition-colors duration-150"
            >
              Stow
            </button>
          </div>

          {/* Stored keys */}
          {vaultKeys.length > 0 ? (
            <div className="space-y-1">
              {vaultKeys.map((key) => (
                <div key={key} className="flex items-center justify-between px-2.5 py-1.5 rounded-md bg-bg-app group">
                  <span className="font-mono text-[12px] text-text-primary">{key}</span>
                  <button
                    onClick={() => handleVaultDelete(key)}
                    className="p-1 rounded text-text-muted opacity-0 group-hover:opacity-100 hover:text-red hover:bg-red-muted transition-all duration-150"
                  >
                    <Trash2 className="w-3 h-3" />
                  </button>
                </div>
              ))}
              <div className="text-[11px] text-text-muted mt-2 pt-2 border-t border-border-subtle">
                {vaultKeys.length} secret{vaultKeys.length !== 1 ? "s" : ""} stowed
              </div>
            </div>
          ) : (
            <div className="text-[12px] text-text-muted">
              The chest is empty.
            </div>
          )}
        </section>

        {/* Papers (OAuth) */}
        <section className="p-4 rounded-lg bg-bg-element border border-border-subtle">
          <div className="flex items-center gap-2 mb-3">
            <Anchor className="w-4 h-4 text-text-muted" />
            <h2 className="text-[13px] font-medium text-text-primary">Papers</h2>
          </div>
          <p className="text-[12px] text-text-secondary mb-3">
            Manage your OAuth charters. Harbor carries its own papers by default,
            or you can supply your own credentials.
          </p>

          {providers.length > 0 ? (
            <div className="space-y-1.5">
              {providers.map((p) => {
                const badgeStatus: Status = p.has_token
                  ? "chartered"
                  : p.token_expired
                    ? "expired"
                    : "unchartered";
                const isExpanded = expandedProvider === p.id;

                return (
                  <div key={p.id} className="rounded-md bg-bg-app">
                    <div className="flex items-center justify-between px-3 py-2">
                      <div className="flex items-center gap-2">
                        <span className="text-[12px] font-medium text-text-primary">{p.display_name}</span>
                        <StatusBadge status={badgeStatus} />
                      </div>
                      <div className="flex items-center gap-2">
                        {p.has_token && (
                          <button
                            onClick={() => handleRevoke(p.id)}
                            className="px-2 py-0.5 rounded text-[11px] border border-red/30 text-red hover:bg-red-muted transition-colors"
                          >
                            Revoke
                          </button>
                        )}
                        <button
                          onClick={() => {
                            setExpandedProvider(isExpanded ? null : p.id);
                            setCustomClientId("");
                            setCustomSecret("");
                          }}
                          className="flex items-center gap-1 px-2 py-0.5 rounded text-[11px] text-text-muted hover:text-text-secondary transition-colors"
                        >
                          {isExpanded ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
                          Own Papers
                        </button>
                      </div>
                    </div>
                    {isExpanded && (
                      <div className="px-3 pb-3 pt-1 space-y-2 border-t border-border-subtle animate-fade-in">
                        <input
                          placeholder="Client ID"
                          value={customClientId}
                          onChange={(e) => setCustomClientId(e.target.value)}
                          className="w-full px-2.5 py-1.5 rounded-md text-[12px] font-mono bg-bg-element border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30"
                        />
                        <input
                          placeholder="Client Secret (optional)"
                          type="password"
                          value={customSecret}
                          onChange={(e) => setCustomSecret(e.target.value)}
                          className="w-full px-2.5 py-1.5 rounded-md text-[12px] font-mono bg-bg-element border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30"
                        />
                        <button
                          onClick={() => handleSaveCustomCreds(p.id)}
                          disabled={!customClientId.trim()}
                          className="px-3 py-1.5 rounded-md text-[11px] font-medium bg-accent text-white hover:bg-accent-hover disabled:opacity-40 transition-colors"
                        >
                          Stow Papers
                        </button>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="text-[12px] text-text-muted">Loading providers...</div>
          )}
        </section>

        {/* About */}
        <section className="p-4 rounded-lg bg-bg-element border border-border-subtle">
          <div className="flex items-center gap-2 mb-3">
            <Info className="w-4 h-4 text-text-muted" />
            <h2 className="text-[13px] font-medium text-text-primary">About</h2>
          </div>
          <div className="text-[12px] text-text-secondary space-y-1">
            <div>Harbor — Your Fleet Commander for MCP Ships</div>
            <div className="text-text-muted">Version {currentVersion}</div>
            <div className="mt-2 pt-2 border-t border-border-subtle text-text-secondary leading-relaxed">
              Command your MCP fleet across Claude Code, Codex, VS Code, and Cursor from one harbor.
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

export default Settings;

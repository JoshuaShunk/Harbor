import { useEffect, useState } from "react";
import { Trash2, Shield, FolderCog, Info, RefreshCw, ArrowDownCircle, CheckCircle2, AlertCircle, Loader2, Anchor, ChevronDown, ChevronUp, Sun, Moon, Monitor } from "lucide-react";
import { vaultSet, vaultDelete, vaultList, oauthListProviders, oauthRevokeCharter, oauthSetCustomCredentials, oauthStartCharter, type OAuthProviderInfo } from "../lib/tauri";
import StatusBadge from "../components/StatusBadge";
import type { Status } from "../components/StatusBadge";
import { useUpdate } from "../contexts/UpdateContext";
import { useTheme, type ThemeChoice } from "../contexts/ThemeContext";

const PROVIDER_META: Record<string, { icon: React.ReactNode; description: string }> = {
  github: {
    icon: (
      <svg className="w-5 h-5 text-text-primary" viewBox="0 0 24 24" fill="currentColor">
        <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
      </svg>
    ),
    description: "Repos, issues, pull requests, and code search",
  },
  google: {
    icon: (
      <svg className="w-5 h-5" viewBox="0 0 24 24">
        <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 01-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z" fill="#4285F4" />
        <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853" />
        <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#FBBC05" />
        <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335" />
      </svg>
    ),
    description: "Drive, Docs, Gmail, and Calendar access",
  },
  slack: {
    icon: (
      <svg className="w-5 h-5" viewBox="0 0 24 24">
        <path d="M5.042 15.165a2.528 2.528 0 01-2.52 2.523A2.528 2.528 0 010 15.165a2.527 2.527 0 012.522-2.52h2.52v2.52zm1.271 0a2.527 2.527 0 012.521-2.52 2.527 2.527 0 012.521 2.52v6.313A2.528 2.528 0 018.834 24a2.528 2.528 0 01-2.521-2.522v-6.313z" fill="#E01E5A" />
        <path d="M8.834 5.042a2.528 2.528 0 01-2.521-2.52A2.528 2.528 0 018.834 0a2.528 2.528 0 012.521 2.522v2.52H8.834zm0 1.271a2.528 2.528 0 012.521 2.521 2.528 2.528 0 01-2.521 2.521H2.522A2.528 2.528 0 010 8.834a2.528 2.528 0 012.522-2.521h6.312z" fill="#36C5F0" />
        <path d="M18.956 8.834a2.528 2.528 0 012.522-2.521A2.528 2.528 0 0124 8.834a2.528 2.528 0 01-2.522 2.521h-2.522V8.834zm-1.271 0a2.528 2.528 0 01-2.521 2.521 2.528 2.528 0 01-2.521-2.521V2.522A2.528 2.528 0 0115.164 0a2.528 2.528 0 012.521 2.522v6.312z" fill="#2EB67D" />
        <path d="M15.164 18.956a2.528 2.528 0 012.521 2.522A2.528 2.528 0 0115.164 24a2.528 2.528 0 01-2.521-2.522v-2.522h2.521zm0-1.271a2.528 2.528 0 01-2.521-2.521 2.528 2.528 0 012.521-2.521h6.314A2.528 2.528 0 0124 15.164a2.528 2.528 0 01-2.522 2.521h-6.314z" fill="#ECB22E" />
      </svg>
    ),
    description: "Channels, messages, and workspace access",
  },
};

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

  const [chartering, setChartering] = useState<string | null>(null);
  const handleCharter = async (providerId: string) => {
    setChartering(providerId);
    try {
      await oauthStartCharter(providerId);
      refreshProviders();
    } catch (e) {
      setVaultMsg({ text: String(e), isError: true });
      setTimeout(() => setVaultMsg(null), 4000);
    } finally {
      setChartering(null);
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
            <div className="space-y-2">
              {providers.map((p) => {
                const badgeStatus: Status = p.has_token
                  ? "chartered"
                  : p.token_expired
                    ? "expired"
                    : "unchartered";
                const isExpanded = expandedProvider === p.id;
                const meta = PROVIDER_META[p.id];

                return (
                  <div key={p.id} className="stagger-item rounded-lg bg-bg-app border border-border-subtle hover:border-border-default transition-colors duration-150">
                    <div className="flex items-center justify-between p-4">
                      <div className="flex items-center gap-3">
                        <div className="w-9 h-9 rounded-lg bg-bg-element border border-border-subtle flex items-center justify-center shrink-0">
                          {meta?.icon ?? <Anchor className="w-4 h-4 text-text-muted" />}
                        </div>
                        <div>
                          <div className="flex items-center gap-2">
                            <span className="text-[13px] font-medium text-text-primary">{p.display_name}</span>
                            <StatusBadge status={badgeStatus} />
                          </div>
                          <div className="text-[12px] text-text-muted mt-0.5">
                            {meta?.description ?? "OAuth provider"}
                          </div>
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        {!p.has_token && (
                          <button
                            onClick={() => handleCharter(p.id)}
                            disabled={chartering === p.id}
                            className="px-2.5 py-1 rounded-md text-[12px] font-medium bg-accent text-white hover:bg-accent-hover disabled:opacity-40 transition-colors duration-150"
                          >
                            {chartering === p.id ? "Chartering..." : "Charter"}
                          </button>
                        )}
                        {p.has_token && (
                          <button
                            onClick={() => handleRevoke(p.id)}
                            className="px-2.5 py-1 rounded-md text-[12px] border border-red/30 text-red hover:bg-red-muted transition-colors duration-150"
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
                          className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[12px] text-text-muted hover:text-text-secondary border border-border-default hover:border-border-hover transition-colors duration-150"
                        >
                          {isExpanded ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
                          Own Papers
                        </button>
                      </div>
                    </div>
                    {isExpanded && (
                      <div className="px-4 pb-4 pt-0 space-y-2 border-t border-border-subtle animate-fade-in">
                        <div className="pt-3 space-y-2">
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

import { useEffect, useState, useRef } from "react";
import { Trash2, Shield, FolderCog, Info, RefreshCw, ArrowDownCircle, CheckCircle2, AlertCircle, Loader2, Anchor, ChevronDown, ChevronUp, Sun, Moon, Monitor, Settings2 } from "lucide-react";
import { vaultSet, vaultDelete, vaultList, oauthListProviders, oauthRevokeCharter, oauthSetCustomCredentials, oauthStartCharter, type OAuthProviderInfo } from "../lib/tauri";
import StatusBadge from "../components/StatusBadge";
import type { Status } from "../components/StatusBadge";
import { useUpdate } from "../contexts/UpdateContext";
import { useTheme, type ThemeChoice } from "../contexts/ThemeContext";
import { SiGithub, SiAtlassian, SiLinear, SiNotion, SiSentry, SiStripe } from "react-icons/si";

const PROVIDER_META: Record<string, { icon: React.ReactNode; description: string }> = {
  github: {
    icon: <SiGithub className="w-5 h-5 text-text-primary" />,
    description: "Repos, issues, pull requests, and code search",
  },
  google: {
    icon: (
      <svg className="w-5 h-5" viewBox="0 0 16 16" fill="none">
        <g fillRule="evenodd" clipRule="evenodd">
          <path fill="#F44336" d="M7.209 1.061c.725-.081 1.154-.081 1.933 0a6.57 6.57 0 0 1 3.65 1.82a100 100 0 0 0-1.986 1.93q-1.876-1.59-4.188-.734q-1.696.78-2.362 2.528a78 78 0 0 1-2.148-1.658a.26.26 0 0 0-.16-.027q1.683-3.245 5.26-3.86" opacity=".987"/>
          <path fill="#FFC107" d="M1.946 4.92q.085-.013.161.027a78 78 0 0 0 2.148 1.658A7.6 7.6 0 0 0 4.04 7.99q.037.678.215 1.331L2 11.116Q.527 8.038 1.946 4.92" opacity=".997"/>
          <path fill="#448AFF" d="M12.685 13.29a26 26 0 0 0-2.202-1.74q1.15-.812 1.396-2.228H8.122V6.713q3.25-.027 6.497.055q.616 3.345-1.423 6.032a7 7 0 0 1-.51.49" opacity=".999"/>
          <path fill="#43A047" d="M4.255 9.322q1.23 3.057 4.51 2.854a3.94 3.94 0 0 0 1.718-.626q1.148.812 2.202 1.74a6.62 6.62 0 0 1-4.027 1.684a6.4 6.4 0 0 1-1.02 0Q3.82 14.524 2 11.116z" opacity=".993"/>
        </g>
      </svg>
    ),
    description: "Drive, Docs, Gmail, and Calendar access",
  },
  slack: {
    icon: (
      <svg className="w-5 h-5" viewBox="0 0 128 128">
        <path fill="#de1c59" d="M27.255 80.719c0 7.33-5.978 13.317-13.309 13.317C6.616 94.036.63 88.049.63 80.719s5.987-13.317 13.317-13.317h13.309zm6.709 0c0-7.33 5.987-13.317 13.317-13.317s13.317 5.986 13.317 13.317v33.335c0 7.33-5.986 13.317-13.317 13.317c-7.33 0-13.317-5.987-13.317-13.317zm0 0"/>
        <path fill="#35c5f0" d="M47.281 27.255c-7.33 0-13.317-5.978-13.317-13.309C33.964 6.616 39.951.63 47.281.63s13.317 5.987 13.317 13.317v13.309zm0 6.709c7.33 0 13.317 5.987 13.317 13.317s-5.986 13.317-13.317 13.317H13.946C6.616 60.598.63 54.612.63 47.281c0-7.33 5.987-13.317 13.317-13.317zm0 0"/>
        <path fill="#2eb57d" d="M100.745 47.281c0-7.33 5.978-13.317 13.309-13.317c7.33 0 13.317 5.987 13.317 13.317s-5.987 13.317-13.317 13.317h-13.309zm-6.709 0c0 7.33-5.987 13.317-13.317 13.317s-13.317-5.986-13.317-13.317V13.946C67.402 6.616 73.388.63 80.719.63c7.33 0 13.317 5.987 13.317 13.317zm0 0"/>
        <path fill="#ebb02e" d="M80.719 100.745c7.33 0 13.317 5.978 13.317 13.309c0 7.33-5.987 13.317-13.317 13.317s-13.317-5.987-13.317-13.317v-13.309zm0-6.709c-7.33 0-13.317-5.987-13.317-13.317s5.986-13.317 13.317-13.317h33.335c7.33 0 13.317 5.986 13.317 13.317c0 7.33-5.987 13.317-13.317 13.317zm0 0"/>
      </svg>
    ),
    description: "Channels, messages, and workspace access",
  },
  atlassian: {
    icon: <SiAtlassian className="w-5 h-5" color="#0052CC" />,
    description: "Jira, Confluence & Compass — issues, pages, search",
  },
  linear: {
    icon: <SiLinear className="w-5 h-5" color="#5E6AD2" />,
    description: "Issues, projects, cycles, and comments",
  },
  notion: {
    icon: <SiNotion className="w-5 h-5 text-text-primary" />,
    description: "Pages, databases, docs, and tasks",
  },
  sentry: {
    icon: <SiSentry className="w-5 h-5" color="#362D59" />,
    description: "Errors, issues, and performance monitoring",
  },
  figma: {
    icon: (
      <svg className="w-5 h-5" viewBox="0 0 256 384">
        <path fill="#0ACF83" d="M64 384c35.328 0 64-28.672 64-64v-64H64c-35.328 0-64 28.672-64 64s28.672 64 64 64Z"/>
        <path fill="#A259FF" d="M0 192c0-35.328 28.672-64 64-64h64v128H64c-35.328 0-64-28.672-64-64Z"/>
        <path fill="#F24E1E" d="M0 64C0 28.672 28.672 0 64 0h64v128H64C28.672 128 0 99.328 0 64Z"/>
        <path fill="#FF7262" d="M128 0h64c35.328 0 64 28.672 64 64s-28.672 64-64 64h-64V0Z"/>
        <path fill="#1ABCFE" d="M256 192c0 35.328-28.672 64-64 64s-64-28.672-64-64s28.672-64 64-64s64 28.672 64 64Z"/>
      </svg>
    ),
    description: "Design inspection, Dev Mode, and components",
  },
  stripe: {
    icon: <SiStripe className="w-5 h-5" color="#635BFF" />,
    description: "Payments, customers, subscriptions, and webhooks",
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
  const [showVaultKeys, setShowVaultKeys] = useState(false);
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");
  const [vaultMsg, setVaultMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const [providers, setProviders] = useState<OAuthProviderInfo[]>([]);
  const [expandedProvider, setExpandedProvider] = useState<string | null>(null);
  const [customClientId, setCustomClientId] = useState("");
  const [customSecret, setCustomSecret] = useState("");
  const [openMenu, setOpenMenu] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setOpenMenu(null);
      }
    };
    if (openMenu) {
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [openMenu]);

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
            <div>
              <button
                onClick={() => setShowVaultKeys(!showVaultKeys)}
                className="flex items-center gap-1.5 text-[12px] text-text-muted hover:text-text-secondary transition-colors duration-150"
              >
                {showVaultKeys ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
                {vaultKeys.length} secret{vaultKeys.length !== 1 ? "s" : ""} stowed
              </button>
              {showVaultKeys && (
                <div className="space-y-1 mt-2 animate-fade-in">
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
                </div>
              )}
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
                      <div className="relative" ref={openMenu === p.id ? menuRef : undefined}>
                        <button
                          onClick={() => setOpenMenu(openMenu === p.id ? null : p.id)}
                          className="p-1.5 rounded-md text-text-muted hover:text-text-secondary hover:bg-bg-hover transition-colors duration-150"
                        >
                          <Settings2 className="w-4 h-4" />
                        </button>
                        {openMenu === p.id && (
                          <div className="absolute right-0 top-full mt-1 w-48 rounded-lg bg-bg-element border border-border-subtle shadow-lg z-10 py-1 animate-fade-in">
                            <button
                              onClick={() => {
                                setOpenMenu(null);
                                handleCharter(p.id);
                              }}
                              disabled={chartering === p.id}
                              className="w-full text-left px-3 py-2 text-[12px] text-text-secondary hover:text-text-primary hover:bg-bg-hover transition-colors duration-150 disabled:opacity-40"
                            >
                              {chartering === p.id ? "Chartering..." : p.has_token ? "Re-charter" : "Charter"}
                            </button>
                            {p.has_token && (
                              <button
                                onClick={() => {
                                  setOpenMenu(null);
                                  handleRevoke(p.id);
                                }}
                                className="w-full text-left px-3 py-2 text-[12px] text-red hover:bg-red-muted transition-colors duration-150"
                              >
                                Revoke
                              </button>
                            )}
                            <div className="h-px bg-border-subtle my-1" />
                            <button
                              onClick={() => {
                                setOpenMenu(null);
                                setExpandedProvider(isExpanded ? null : p.id);
                                setCustomClientId("");
                                setCustomSecret("");
                              }}
                              className="w-full text-left px-3 py-2 text-[12px] text-text-secondary hover:text-text-primary hover:bg-bg-hover transition-colors duration-150"
                            >
                              Own Papers
                            </button>
                          </div>
                        )}
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
              Command your MCP fleet across Claude Code, Claude Desktop, Codex, VS Code, and Cursor from one harbor.
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

export default Settings;

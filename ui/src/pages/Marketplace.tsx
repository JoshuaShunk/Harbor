import { useEffect, useState } from "react";
import { Search, ExternalLink, BadgeCheck, Package, Anchor, ChevronDown, Lock } from "lucide-react";
import { marketplaceSearch, oauthGetStatus, addServer, getGdriveCredentialPaths, type MarketplaceServer, type OAuthProviderInfo } from "../lib/tauri";
import StatusBadge from "../components/StatusBadge";
import type { Status } from "../components/StatusBadge";
import OAuthCharterModal from "../components/OAuthCharterModal";

function Marketplace() {
  const [query, setQuery] = useState("");
  const [servers, setServers] = useState<MarketplaceServer[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);
  const [searchGen, setSearchGen] = useState(0);

  const handleSearch = async (cursor?: string) => {
    if (!cursor && !query.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const result = await marketplaceSearch(query.trim(), cursor, 10);
      if (cursor) {
        setServers((prev) => [...prev, ...result.servers]);
      } else {
        setServers(result.servers);
      }
      setNextCursor(result.next_cursor);
      setHasSearched(true);
      if (!cursor) setSearchGen((g) => g + 1);
    } catch (e) {
      setError(String(e));
      if (!cursor) {
        setServers([]);
        setNextCursor(null);
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="p-8 max-w-4xl">
      {/* Header */}
      <div className="mb-8">
        <h1 className="text-lg font-semibold text-text-primary">Scout the Seas</h1>
        <p className="text-[13px] text-text-secondary mt-0.5">
          Discover MCP ships from the official registry
        </p>
      </div>

      {/* Search bar */}
      <div className="flex gap-2 mb-6">
        <div className="flex-1 relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text-muted pointer-events-none" />
          <input
            placeholder="Scout the waters (e.g. github, database, memory)..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            className="w-full pl-9 pr-3 py-2 rounded-md text-[13px] bg-bg-element border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150"
          />
        </div>
        <button
          onClick={() => handleSearch()}
          disabled={loading || !query.trim()}
          className="px-4 py-2 rounded-md text-[13px] font-medium bg-accent text-white hover:bg-accent-hover disabled:opacity-40 disabled:cursor-not-allowed transition-colors duration-150"
        >
          {loading ? "Scouting..." : "Scout"}
        </button>
      </div>

      {/* Error */}
      {error && (
        <div className="mb-4 px-3 py-2 rounded-md text-[13px] bg-red-muted text-red border border-red/20 animate-fade-in">
          {error}
        </div>
      )}

      {/* Results */}
      {hasSearched ? (
        <div className="animate-fade-in">
          <div className="text-[12px] text-text-muted mb-3">
            {servers.length} sighting{servers.length !== 1 ? "s" : ""} loaded
          </div>

          {servers.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16">
              <div className="w-12 h-12 rounded-xl bg-bg-element border border-border-subtle flex items-center justify-center mb-4">
                <Search className="w-6 h-6 text-text-muted" />
              </div>
              <p className="text-sm font-medium text-text-primary mb-1">No ships spotted on the horizon</p>
              <p className="text-[13px] text-text-secondary">Try scouting different waters</p>
            </div>
          ) : (
            <div className="space-y-2">
              {servers.map((s) => (
                <ServerResult key={s.name} server={s} searchGen={searchGen} />
              ))}
            </div>
          )}

          {/* Load More */}
          {nextCursor && (
            <div className="flex justify-center mt-6">
              <button
                onClick={() => handleSearch(nextCursor)}
                disabled={loading}
                className="inline-flex items-center gap-1 px-4 py-1.5 rounded-md text-[12px] font-medium border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover disabled:opacity-30 disabled:cursor-not-allowed transition-colors duration-150"
              >
                {loading ? "Loading..." : "Load More"}
              </button>
            </div>
          )}
        </div>
      ) : !loading && (
        <div className="flex flex-col items-center justify-center py-20 animate-fade-in">
          <div className="w-12 h-12 rounded-xl bg-bg-element border border-border-subtle flex items-center justify-center mb-4">
            <Package className="w-6 h-6 text-text-muted" />
          </div>
          <p className="text-sm font-medium text-text-primary mb-1">Scout the Registry</p>
          <p className="text-[13px] text-text-secondary max-w-xs text-center">
            Spot new MCP ships by name, category, or description.
          </p>
        </div>
      )}
    </div>
  );
}

// Correct npm packages and env vars for OAuth-backed MCP servers.
// The official registry name may not match the actual npm package, so we override here.
// Slack uses HTTPS redirect proxy at harbormcp.ai for OAuth.
const OAUTH_SERVER_CONFIG: Record<string, { pkg: string; envVar: string }> = {
  github: { pkg: "@modelcontextprotocol/server-github", envVar: "GITHUB_PERSONAL_ACCESS_TOKEN" },
  google: { pkg: "@modelcontextprotocol/server-gdrive", envVar: "GOOGLE_ACCESS_TOKEN" },
  slack: { pkg: "@modelcontextprotocol/server-slack", envVar: "SLACK_BOT_TOKEN" },
};

// Match an OAuth provider by checking env var names first, then falling back
// to the server slug (segment after `/`) for servers that don't declare env vars.
const ENV_VAR_PROVIDER_PATTERNS: Record<string, string> = {
  GITHUB: "github",
  GOOGLE: "google",
  GDRIVE: "google",
  GMAIL: "google",
  SLACK: "slack",
};

function detectProvider(
  qualifiedName: string,
  envVars: { name: string }[],
): string | null {
  // 1. Check env var names (most reliable signal).
  for (const ev of envVars) {
    const upper = ev.name.toUpperCase();
    for (const [keyword, provider] of Object.entries(ENV_VAR_PROVIDER_PATTERNS)) {
      if (upper.includes(keyword)) return provider;
    }
  }
  // 2. Fall back to slug (after last `/`) to catch servers with no declared env vars
  //    like `ai.smithery/smithery-ai-github`. We intentionally skip the namespace
  //    prefix to avoid `io.github.*` false positives.
  const slug = (qualifiedName.split("/").pop() ?? qualifiedName).toLowerCase();
  if (slug.includes("github")) return "github";
  if (slug.includes("google") || slug.includes("gdrive") || slug.includes("gmail")) return "google";
  if (slug.includes("slack")) return "slack";
  return null;
}

function ServerResult({ server, searchGen }: { server: MarketplaceServer; searchGen: number }) {
  const [providerId, setProviderId] = useState<string | null>(null);
  const [oauthStatus, setOauthStatus] = useState<OAuthProviderInfo | null>(null);
  const [showCharter, setShowCharter] = useState(false);
  const [dockMsg, setDockMsg] = useState<string | null>(null);
  const [showEnvForm, setShowEnvForm] = useState(false);
  const [envPairs, setEnvPairs] = useState<[string, string][]>([]);
  const [envKey, setEnvKey] = useState("");
  const [envVal, setEnvVal] = useState("");
  const [docking, setDocking] = useState(false);

  const displayName = server.title ?? server.name;
  const registryEnvVars = server.package?.environment_variables ?? [];
  const hasRequiredEnvVars = registryEnvVars.some((e) => e.is_required);

  useEffect(() => {
    const id = detectProvider(server.name, registryEnvVars);
    setProviderId(id);
    if (id) {
      oauthGetStatus(id).then(setOauthStatus);
    }
  }, [server.name, searchGen]);

  // Initialize envPairs from registry metadata.
  useEffect(() => {
    if (registryEnvVars.length > 0) {
      const initial: [string, string][] = registryEnvVars.map((ev) => [
        ev.name,
        ev.is_secret ? `vault:${ev.name.toLowerCase()}` : (ev.default ?? ""),
      ]);
      setEnvPairs(initial);
      if (hasRequiredEnvVars) {
        setShowEnvForm(true);
      }
    }
  }, [server.name]);

  const isOAuthServer = providerId !== null && oauthStatus !== null;

  const refreshOAuth = () => {
    if (providerId) {
      oauthGetStatus(providerId).then(setOauthStatus);
    }
  };

  const handleDockWithOAuth = async () => {
    if (!providerId) return;
    try {
      const config = OAUTH_SERVER_CONFIG[providerId];
      const pkg = config?.pkg ?? (server.package?.identifier ?? server.name);
      const name = displayName.toLowerCase().replace(/[^a-z0-9-]/g, "-");

      let env: Record<string, string>;
      if (providerId === "google") {
        const [oauthPath, credsPath] = await getGdriveCredentialPaths();
        env = { GDRIVE_OAUTH_PATH: oauthPath, GDRIVE_CREDENTIALS_PATH: credsPath };
      } else if (providerId === "slack") {
        env = {
          SLACK_BOT_TOKEN: `vault:oauth:slack:access_token`,
          SLACK_TEAM_ID: `vault:oauth:slack:team_id`,
        };
      } else {
        const envVar = config?.envVar ?? `${providerId.toUpperCase()}_TOKEN`;
        env = { [envVar]: `vault:oauth:${providerId}:access_token` };
      }

      await addServer(name, "npx", ["-y", pkg], env);
      setDockMsg("Ship docked!");
      setTimeout(() => setDockMsg(null), 3000);
    } catch (e) {
      setDockMsg(String(e));
      setTimeout(() => setDockMsg(null), 4000);
    }
  };

  const handleGenericDock = async () => {
    setDocking(true);
    try {
      const name = displayName.toLowerCase().replace(/[^a-z0-9-]/g, "-");
      const pkg = server.package;

      let command: string;
      let args: string[];
      if (pkg) {
        const runtime = pkg.runtime_hint
          ?? (pkg.registry_type === "pypi" ? "uvx" : "npx");
        command = runtime;
        args = runtime === "npx" ? ["-y", pkg.identifier] : [pkg.identifier];
      } else {
        command = "npx";
        args = ["-y", server.name];
      }

      const env: Record<string, string> = {};
      envPairs.forEach(([k, v]) => { if (v) env[k] = v; });

      await addServer(name, command, args, env);
      setDockMsg("Ship docked!");
      setShowEnvForm(false);
      setTimeout(() => setDockMsg(null), 3000);
    } catch (e) {
      setDockMsg(String(e));
      setTimeout(() => setDockMsg(null), 4000);
    } finally {
      setDocking(false);
    }
  };

  const updateEnvPair = (key: string, value: string) => {
    setEnvPairs((prev) => {
      const idx = prev.findIndex(([k]) => k === key);
      if (idx >= 0) {
        const next = [...prev];
        next[idx] = [key, value];
        return next;
      }
      return [...prev, [key, value]];
    });
  };

  const addEnvPair = () => {
    if (envKey.trim()) {
      setEnvPairs([...envPairs, [envKey.trim(), envVal]]);
      setEnvKey("");
      setEnvVal("");
    }
  };

  const oauthBadgeStatus: Status | null = oauthStatus
    ? oauthStatus.has_token
      ? "chartered"
      : oauthStatus.token_expired
        ? "expired"
        : "unchartered"
    : null;

  return (
    <>
      <div className="stagger-item p-4 rounded-lg bg-bg-element border border-border-subtle hover:border-border-default transition-colors duration-150">
        <div className="flex items-start justify-between gap-4">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-[13px] font-medium text-text-primary truncate">{displayName}</span>
              {server.is_official && (
                <span className="inline-flex items-center gap-1 text-[11px] font-medium px-1.5 py-0.5 rounded-sm bg-green-muted text-green">
                  <BadgeCheck className="w-3 h-3" />
                  official
                </span>
              )}
              {oauthBadgeStatus && <StatusBadge status={oauthBadgeStatus} />}
            </div>
            <div className="text-[12px] text-text-muted font-mono mt-0.5 truncate">
              {server.name}
            </div>
            <div className="text-[12px] text-text-secondary mt-1.5 line-clamp-2 leading-relaxed">
              {server.description}
            </div>
            {server.package && (
              <div className="text-[11px] text-text-muted mt-1 font-mono">
                {server.package.runtime_hint ?? (server.package.registry_type === "pypi" ? "uvx" : "npx")} {server.package.identifier}
              </div>
            )}
            {dockMsg && (
              <div className="text-[11px] text-green mt-1.5">{dockMsg}</div>
            )}
          </div>
          <div className="flex items-center gap-2 shrink-0 pt-0.5">
            {/* OAuth: Charter or Dock Ship button */}
            {isOAuthServer ? (
              oauthStatus.has_token ? (
                <button
                  onClick={handleDockWithOAuth}
                  className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md text-[12px] font-medium bg-accent text-white hover:bg-accent-hover transition-colors duration-150"
                >
                  <Anchor className="w-3 h-3" />
                  Dock Ship
                </button>
              ) : (
                <button
                  onClick={() => setShowCharter(true)}
                  className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md text-[12px] font-medium border border-accent/40 text-accent hover:bg-accent-muted transition-colors duration-150"
                >
                  <Anchor className="w-3 h-3" />
                  Charter
                </button>
              )
            ) : (
              <div className="flex items-center gap-1">
                <button
                  onClick={handleGenericDock}
                  disabled={docking}
                  className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md text-[12px] font-medium bg-accent text-white hover:bg-accent-hover disabled:opacity-40 transition-colors duration-150"
                >
                  <Anchor className="w-3 h-3" />
                  {docking ? "Docking..." : "Dock Ship"}
                </button>
                <button
                  onClick={() => setShowEnvForm(!showEnvForm)}
                  className="p-1 rounded-md text-text-muted hover:text-text-secondary hover:bg-bg-hover transition-colors duration-150"
                  title="Configure environment variables"
                >
                  <ChevronDown className={`w-3.5 h-3.5 transition-transform duration-150 ${showEnvForm ? "rotate-180" : ""}`} />
                </button>
              </div>
            )}
            {server.website_url && (
              <a
                href={server.website_url}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-1 px-2 py-1 rounded-md text-[12px] border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors duration-150"
              >
                View
                <ExternalLink className="w-3 h-3" />
              </a>
            )}
          </div>
        </div>

        {/* Env var form for generic servers */}
        {showEnvForm && !isOAuthServer && (
          <div className="mt-3 pt-3 border-t border-border-subtle animate-fade-in">
            {/* Pre-populated env vars from registry */}
            {registryEnvVars.length > 0 && (
              <div className="space-y-2 mb-3">
                {registryEnvVars.map((ev) => (
                  <div key={ev.name}>
                    <label className="flex items-center gap-1 text-[12px] font-medium text-text-primary mb-0.5">
                      {ev.name}
                      {ev.is_required && <span className="text-red">*</span>}
                      {ev.is_secret && <Lock className="w-3 h-3 text-text-muted" />}
                    </label>
                    <input
                      value={envPairs.find(([k]) => k === ev.name)?.[1] ?? ""}
                      onChange={(e) => updateEnvPair(ev.name, e.target.value)}
                      placeholder={ev.is_secret ? `vault:${ev.name.toLowerCase()}` : (ev.default ?? "")}
                      className="w-full px-3 py-1.5 rounded-md text-[12px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150"
                    />
                    {ev.description && (
                      <p className="text-[11px] text-text-muted mt-0.5">{ev.description}</p>
                    )}
                  </div>
                ))}
              </div>
            )}
            {/* Manual env var entry */}
            <div className="flex gap-2">
              <input
                placeholder="ENV_KEY"
                value={envKey}
                onChange={(e) => setEnvKey(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addEnvPair()}
                className="flex-1 px-3 py-1.5 rounded-md text-[12px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150"
              />
              <input
                placeholder="value (or vault:key_name)"
                value={envVal}
                onChange={(e) => setEnvVal(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addEnvPair()}
                className="flex-1 px-3 py-1.5 rounded-md text-[12px] bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-colors duration-150"
              />
              <button
                onClick={addEnvPair}
                className="px-2.5 py-1.5 rounded-md text-[12px] bg-bg-app border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors duration-150"
              >
                + Env
              </button>
            </div>
            {/* Extra manual env vars (beyond registry-defined ones) */}
            {envPairs.filter(([k]) => !registryEnvVars.some((ev) => ev.name === k)).length > 0 && (
              <div className="flex flex-wrap gap-1.5 mt-2">
                {envPairs
                  .filter(([k]) => !registryEnvVars.some((ev) => ev.name === k))
                  .map(([k, v], i) => (
                    <span
                      key={`${k}-${i}`}
                      className="inline-flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-full bg-bg-active text-text-secondary font-mono"
                    >
                      {k}={v.startsWith("vault:") ? <Lock className="w-3 h-3" /> : "***"}
                      <button
                        onClick={() => setEnvPairs(envPairs.filter(([ek]) => ek !== k))}
                        className="ml-0.5 text-text-muted hover:text-red transition-colors"
                      >
                        x
                      </button>
                    </span>
                  ))}
              </div>
            )}
            {registryEnvVars.length === 0 && (
              <p className="text-[11px] text-text-muted mt-2">
                Add env vars this ship needs (API keys, tokens, etc.)
              </p>
            )}
          </div>
        )}
      </div>

      {/* Charter modal */}
      {showCharter && oauthStatus && (
        <OAuthCharterModal
          provider={oauthStatus}
          serverName={displayName}
          serverRegistryName={server.name}
          onComplete={() => {
            setShowCharter(false);
            refreshOAuth();
          }}
          onClose={() => {
            setShowCharter(false);
            refreshOAuth();
          }}
        />
      )}
    </>
  );
}

export default Marketplace;

import { useState } from "react";
import { Anchor, Loader2, CheckCircle2, AlertCircle, ChevronDown, ChevronUp } from "lucide-react";
import {
  oauthStartCharter,
  oauthSetCustomCredentials,
  addServer,
  getGdriveCredentialPaths,
  type OAuthProviderInfo,
} from "../lib/tauri";

type FlowState = "idle" | "waiting" | "success" | "error";

interface OAuthCharterModalProps {
  provider: OAuthProviderInfo;
  serverName: string;
  serverRegistryName: string;
  onComplete: () => void;
  onClose: () => void;
}

const OAUTH_SERVER_CONFIG: Record<string, { pkg: string; envVar: string }> = {
  github: { pkg: "@modelcontextprotocol/server-github", envVar: "GITHUB_PERSONAL_ACCESS_TOKEN" },
  google: { pkg: "@modelcontextprotocol/server-gdrive", envVar: "GOOGLE_ACCESS_TOKEN" },
  slack: { pkg: "@modelcontextprotocol/server-slack", envVar: "SLACK_BOT_TOKEN" },
};

function OAuthCharterModal({
  provider,
  serverName,
  serverRegistryName,
  onComplete,
  onClose,
}: OAuthCharterModalProps) {
  const [state, setState] = useState<FlowState>("idle");
  const [error, setError] = useState<string | null>(null);
  const [showCustom, setShowCustom] = useState(false);
  const [customClientId, setCustomClientId] = useState("");
  const [customSecret, setCustomSecret] = useState("");
  const [docking, setDocking] = useState(false);

  const handleCharter = async () => {
    setState("waiting");
    setError(null);
    try {
      // Save custom credentials first if provided
      if (customClientId.trim()) {
        await oauthSetCustomCredentials(
          provider.id,
          customClientId.trim(),
          customSecret.trim() || undefined,
        );
      }
      await oauthStartCharter(provider.id);
      setState("success");
    } catch (e) {
      setState("error");
      setError(String(e));
    }
  };

  const handleDock = async () => {
    setDocking(true);
    try {
      const config = OAUTH_SERVER_CONFIG[provider.id];
      const pkg = config?.pkg ?? serverRegistryName;
      const name = serverName.toLowerCase().replace(/[^a-z0-9-]/g, "-");

      let env: Record<string, string>;
      if (provider.id === "google") {
        const [oauthPath, credsPath] = await getGdriveCredentialPaths();
        env = { GDRIVE_OAUTH_PATH: oauthPath, GDRIVE_CREDENTIALS_PATH: credsPath };
      } else if (provider.id === "slack") {
        env = {
          SLACK_BOT_TOKEN: `vault:oauth:slack:access_token`,
          SLACK_TEAM_ID: `vault:oauth:slack:team_id`,
        };
      } else {
        const envVar = config?.envVar ?? `${provider.id.toUpperCase()}_TOKEN`;
        env = { [envVar]: `vault:oauth:${provider.id}:access_token` };
      }

      await addServer(name, "npx", ["-y", pkg], env);
      onComplete();
    } catch (e) {
      setError(String(e));
    } finally {
      setDocking(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 animate-fade-in">
      <div className="w-full max-w-md mx-4 rounded-xl bg-bg-element border border-border-subtle shadow-xl">
        {/* Header */}
        <div className="px-6 pt-6 pb-4 border-b border-border-subtle">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-accent-muted flex items-center justify-center">
              <Anchor className="w-5 h-5 text-accent" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-text-primary">
                Charter {provider.display_name} Papers
              </h2>
              <p className="text-[12px] text-text-secondary mt-0.5">
                for {serverName}
              </p>
            </div>
          </div>
        </div>

        {/* Body */}
        <div className="px-6 py-5">
          {state === "idle" && (
            <div className="space-y-4">
              <p className="text-[13px] text-text-secondary">
                Harbor will open your browser to authorize with {provider.display_name}.
                The following permissions will be requested:
              </p>
              <div className="flex flex-wrap gap-1.5">
                {provider.scopes.map((scope) => (
                  <span
                    key={scope}
                    className="text-[11px] px-2 py-0.5 rounded-full bg-bg-active text-text-secondary font-mono"
                  >
                    {scope}
                  </span>
                ))}
              </div>

              {/* Custom credentials toggle */}
              <button
                onClick={() => setShowCustom(!showCustom)}
                className="flex items-center gap-1 text-[12px] text-text-muted hover:text-text-secondary transition-colors"
              >
                {showCustom ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
                Use own papers
              </button>
              {showCustom && (
                <div className="space-y-2 animate-fade-in">
                  <input
                    placeholder="Client ID"
                    value={customClientId}
                    onChange={(e) => setCustomClientId(e.target.value)}
                    className="w-full px-3 py-2 rounded-md text-[12px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30"
                  />
                  <input
                    placeholder="Client Secret (optional)"
                    type="password"
                    value={customSecret}
                    onChange={(e) => setCustomSecret(e.target.value)}
                    className="w-full px-3 py-2 rounded-md text-[12px] font-mono bg-bg-app border border-border-default text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/30"
                  />
                </div>
              )}
            </div>
          )}

          {state === "waiting" && (
            <div className="flex flex-col items-center py-6 animate-fade-in">
              <Loader2 className="w-8 h-8 text-accent animate-spin mb-4" />
              <p className="text-[13px] font-medium text-text-primary">
                Awaiting papers from {provider.display_name}...
              </p>
              <p className="text-[12px] text-text-muted mt-1">
                Complete the authorization in your browser
              </p>
            </div>
          )}

          {state === "success" && (
            <div className="flex flex-col items-center py-6 animate-fade-in">
              <CheckCircle2 className="w-8 h-8 text-green mb-4" />
              <p className="text-[13px] font-medium text-text-primary">Charted!</p>
              <p className="text-[12px] text-text-secondary mt-1">
                {provider.display_name} papers received and stowed
              </p>
            </div>
          )}

          {state === "error" && (
            <div className="flex flex-col items-center py-6 animate-fade-in">
              <AlertCircle className="w-8 h-8 text-red mb-4" />
              <p className="text-[13px] font-medium text-text-primary">Charter Failed</p>
              <p className="text-[12px] text-text-secondary mt-1 text-center max-w-xs">
                {error}
              </p>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-6 pb-6 flex justify-end gap-2">
          {state === "idle" && (
            <>
              <button
                onClick={onClose}
                className="px-3 py-1.5 rounded-md text-[12px] border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleCharter}
                className="px-4 py-1.5 rounded-md text-[12px] font-medium bg-accent text-white hover:bg-accent-hover transition-colors"
              >
                Begin Charter
              </button>
            </>
          )}

          {state === "success" && (
            <>
              <button
                onClick={onClose}
                className="px-3 py-1.5 rounded-md text-[12px] border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors"
              >
                Close
              </button>
              <button
                onClick={handleDock}
                disabled={docking}
                className="px-4 py-1.5 rounded-md text-[12px] font-medium bg-accent text-white hover:bg-accent-hover disabled:opacity-40 transition-colors"
              >
                {docking ? "Docking..." : "Dock Ship"}
              </button>
            </>
          )}

          {state === "error" && (
            <>
              <button
                onClick={onClose}
                className="px-3 py-1.5 rounded-md text-[12px] border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => setState("idle")}
                className="px-4 py-1.5 rounded-md text-[12px] font-medium bg-accent text-white hover:bg-accent-hover transition-colors"
              >
                Try Again
              </button>
            </>
          )}

          {state === "waiting" && (
            <button
              onClick={onClose}
              className="px-3 py-1.5 rounded-md text-[12px] border border-border-default text-text-secondary hover:text-text-primary hover:border-border-hover transition-colors"
            >
              Cancel
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export default OAuthCharterModal;

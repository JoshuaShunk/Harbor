import { invoke } from "@tauri-apps/api/core";

// --- Types matching the Rust structs ---

export interface ServerConfig {
  source: string | null;
  command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
  auto_start: boolean;
  hosts: Record<string, boolean>;
}

export interface ServerStatus {
  name: string;
  enabled: boolean;
  running: boolean;
  pid: number | null;
  command: string;
}

export interface HostStatus {
  name: string;
  display_name: string;
  connected: boolean;
  config_exists: boolean;
  config_path: string;
  server_count: number;
}

export interface HarborStatus {
  servers: ServerStatus[];
  hosts: HostStatus[];
  gateway_port: number;
}

// --- Tauri command wrappers ---

export async function getStatus(): Promise<HarborStatus> {
  return invoke<HarborStatus>("get_status");
}

export async function addServer(
  name: string,
  command: string,
  args: string[],
  env: Record<string, string>,
): Promise<void> {
  return invoke("add_server", { name, command, args, env });
}

export async function removeServer(name: string): Promise<void> {
  return invoke("remove_server", { name });
}

export async function toggleServer(name: string, enabled: boolean): Promise<void> {
  return invoke("toggle_server", { name, enabled });
}

export async function connectHost(host: string): Promise<void> {
  return invoke("connect_host", { host });
}

export async function disconnectHost(host: string): Promise<void> {
  return invoke("disconnect_host", { host });
}

// --- Vault ---

export async function vaultSet(key: string, value: string): Promise<void> {
  return invoke("vault_set", { key, value });
}

export async function vaultGet(key: string): Promise<string> {
  return invoke<string>("vault_get", { key });
}

export async function vaultDelete(key: string): Promise<void> {
  return invoke("vault_delete", { key });
}

export async function vaultList(): Promise<string[]> {
  return invoke<string[]>("vault_list");
}

// --- Tool Discovery ---

export interface DiscoveredTool {
  name: string;
  description: string | null;
}

export async function discoverTools(server: string): Promise<DiscoveredTool[]> {
  return invoke<DiscoveredTool[]>("discover_tools", { server });
}

// --- Tool Filters ---

export interface ToolFilterInfo {
  tool_allowlist: string[] | null;
  tool_blocklist: string[] | null;
  tool_hosts: Record<string, string[]>;
}

export async function getToolFilters(server: string): Promise<ToolFilterInfo> {
  return invoke<ToolFilterInfo>("get_tool_filters", { server });
}

export async function setToolAllowlist(server: string, tools: string[] | null): Promise<void> {
  return invoke("set_tool_allowlist", { server, tools });
}

export async function setToolBlocklist(server: string, tools: string[] | null): Promise<void> {
  return invoke("set_tool_blocklist", { server, tools });
}

export async function setToolHostOverride(
  server: string,
  host: string,
  tools: string[] | null,
): Promise<void> {
  return invoke("set_tool_host_override", { server, host, tools });
}

// --- Gateway ---

export async function startGateway(): Promise<string> {
  return invoke<string>("start_gateway");
}

export async function stopGateway(): Promise<string> {
  return invoke<string>("stop_gateway");
}

export async function gatewayStatus(): Promise<boolean> {
  return invoke<boolean>("gateway_status");
}

// --- Marketplace ---

export interface MarketplaceEnvVar {
  name: string;
  description: string | null;
  is_required: boolean;
  is_secret: boolean;
  default: string | null;
}

export interface MarketplacePackage {
  registry_type: string;
  identifier: string;
  version: string | null;
  runtime_hint: string | null;
  environment_variables: MarketplaceEnvVar[];
}

export interface MarketplaceServer {
  name: string;
  title: string | null;
  description: string;
  website_url: string | null;
  is_official: boolean;
  repository_url: string | null;
  package: MarketplacePackage | null;
}

export interface MarketplaceSearchResult {
  servers: MarketplaceServer[];
  next_cursor: string | null;
}

export async function marketplaceSearch(
  query: string,
  cursor?: string,
  limit?: number,
): Promise<MarketplaceSearchResult> {
  return invoke<MarketplaceSearchResult>("marketplace_search", {
    query,
    cursor: cursor ?? null,
    limit: limit ?? null,
  });
}

// --- OAuth ---

export interface OAuthProviderInfo {
  id: string;
  display_name: string;
  has_token: boolean;
  token_expired: boolean;
  scopes: string[];
}

export async function oauthListProviders(): Promise<OAuthProviderInfo[]> {
  return invoke<OAuthProviderInfo[]>("oauth_list_providers");
}

export async function oauthProviderForServer(qualifiedName: string): Promise<string | null> {
  return invoke<string | null>("oauth_provider_for_server", { qualifiedName });
}

export async function oauthStartCharter(providerId: string): Promise<void> {
  return invoke("oauth_start_charter", { providerId });
}

export async function oauthGetStatus(providerId: string): Promise<OAuthProviderInfo> {
  return invoke<OAuthProviderInfo>("oauth_get_status", { providerId });
}

export async function oauthRevokeCharter(providerId: string): Promise<void> {
  return invoke("oauth_revoke_charter", { providerId });
}

export async function oauthSetCustomCredentials(
  providerId: string,
  clientId: string,
  clientSecret?: string,
): Promise<void> {
  return invoke("oauth_set_custom_credentials", {
    providerId,
    clientId,
    clientSecret: clientSecret ?? null,
  });
}

export async function getGdriveCredentialPaths(): Promise<[string, string]> {
  return invoke<[string, string]>("gdrive_credential_paths");
}

// --- Native Catalog ---

export interface NativeServerInfo {
  id: string;
  display_name: string;
  description: string;
  auth_kind: string;
  has_auth: boolean;
  manual_vault_key: string | null;
}

export async function catalogList(): Promise<NativeServerInfo[]> {
  return invoke<NativeServerInfo[]>("catalog_list");
}

export async function dockNative(id: string, name?: string): Promise<void> {
  return invoke("dock_native", { id, name: name ?? null });
}

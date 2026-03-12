import { invoke } from "@tauri-apps/api/core";

// --- Types matching the Rust structs ---

export interface ServerConfig {
  source: string | null;
  command: string | null;
  args: string[];
  env: Record<string, string>;
  url: string | null;
  headers: Record<string, string> | null;
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
  is_remote: boolean;
  source: string | null;
  locally_modified: boolean;
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
  gateway_host: string;
  local_ip: string | null;
}

// --- Tauri command wrappers ---

export async function getStatus(): Promise<HarborStatus> {
  return invoke<HarborStatus>("get_status");
}

export async function addServer(
  name: string,
  command: string | null,
  args: string[],
  env: Record<string, string>,
  url?: string | null,
  headers?: Record<string, string> | null,
  source?: string | null,
): Promise<void> {
  return invoke("add_server", {
    name,
    command,
    args,
    env,
    url: url ?? null,
    headers: headers ?? null,
    source: source ?? null,
  });
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

export interface GatewaySettingsInfo {
  host: string;
  token: string | null;
}

export async function getGatewaySettings(): Promise<GatewaySettingsInfo> {
  return invoke<GatewaySettingsInfo>("get_gateway_settings");
}

export async function setGatewaySettings(host: string, token: string | null): Promise<void> {
  return invoke("set_gateway_settings", { host, token });
}

export async function reloadGateway(): Promise<void> {
  return invoke("reload_gateway");
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
  is_remote: boolean;
  manual_vault_key: string | null;
  extra_args_kind: string;
  extra_args_label: string | null;
  extra_args_placeholder: string | null;
}

export async function catalogList(): Promise<NativeServerInfo[]> {
  return invoke<NativeServerInfo[]>("catalog_list");
}

export async function dockNative(
  id: string,
  name?: string,
  extraArgs?: string[],
): Promise<void> {
  return invoke("dock_native", {
    id,
    name: name ?? null,
    extraArgs: extraArgs ?? null,
  });
}

// --- Server Extra Args ---

export interface ServerExtraArgsInfo {
  extra_args: string[];
  extra_args_kind: string;
  extra_args_label: string | null;
  extra_args_placeholder: string | null;
}

export async function getServerExtraArgs(name: string): Promise<ServerExtraArgsInfo> {
  return invoke<ServerExtraArgsInfo>("get_server_extra_args", { name });
}

export async function setServerExtraArgs(name: string, extraArgs: string[]): Promise<void> {
  return invoke("set_server_extra_args", { name, extraArgs });
}

// --- Server Args (general, any server) ---

export async function getServerArgs(name: string): Promise<string[]> {
  return invoke<string[]>("get_server_args", { name });
}

export async function setServerArgs(name: string, args: string[]): Promise<void> {
  return invoke("set_server_args", { name, args });
}

// --- Config Schema (from MCP Registry) ---

export interface ConfigSchemaArg {
  arg_type: string;
  name: string;
  description: string | null;
  is_required: boolean;
  format: string;
  default: string | null;
  is_secret: boolean;
  is_repeated: boolean;
  choices: string[] | null;
  placeholder: string | null;
  value_hint: string | null;
}

export interface ConfigSchemaEnvVar {
  name: string;
  description: string | null;
  is_required: boolean;
  is_secret: boolean;
  default: string | null;
}

export interface ConfigSchemaResponse {
  args: ConfigSchemaArg[] | null;
  env_vars: ConfigSchemaEnvVar[] | null;
  registry_name: string | null;
}

export async function getConfigSchema(name: string): Promise<ConfigSchemaResponse> {
  return invoke<ConfigSchemaResponse>("get_config_schema", { name });
}

// --- Request Log ---

export type RequestStatus = "success" | "error";

export interface RequestLogEntry {
  id: number;
  timestamp: string; // ISO 8601 UTC
  server: string;
  tool: string;
  input: unknown;
  status: RequestStatus;
  latency_ms: number;
  error: string | null;
  output: unknown | null;
}

export async function getRequestLogs(limit?: number): Promise<RequestLogEntry[]> {
  return invoke<RequestLogEntry[]>("get_request_logs", { limit: limit ?? null });
}

export async function clearRequestLogs(): Promise<void> {
  return invoke("clear_request_logs");
}

// --- Fleet (Crew) ---

export interface FleetStatusResponse {
  initialized: boolean;
  remote_url: string | null;
  ahead: number;
  behind: number;
}

export interface FleetPullResult {
  added: string[];
  updated: string[];
  locally_modified: string[];
  conflicts: string[];
}

export async function fleetStatus(): Promise<FleetStatusResponse> {
  return invoke<FleetStatusResponse>("fleet_status");
}

export async function fleetPull(): Promise<FleetPullResult> {
  return invoke<FleetPullResult>("fleet_pull");
}

// --- Publish ---

export interface PublishInfoResponse {
  url: string;
  token: string;
  transport: string;
}

export interface PublishStatusResponse {
  publishing: boolean;
  info: PublishInfoResponse | null;
}

export async function startPublish(
  subdomain?: string | null,
  relay?: string | null,
  tools?: string[] | null,
): Promise<PublishInfoResponse> {
  return invoke<PublishInfoResponse>("start_publish", {
    subdomain: subdomain ?? null,
    relay: relay ?? null,
    tools: tools ?? null,
  });
}

export async function stopPublish(): Promise<string> {
  return invoke<string>("stop_publish");
}

export async function publishStatus(): Promise<PublishStatusResponse> {
  return invoke<PublishStatusResponse>("publish_status");
}

// --- App behaviour ---

export async function getHideOnClose(): Promise<boolean> {
  return invoke<boolean>("get_hide_on_close");
}

export async function setHideOnClose(enabled: boolean): Promise<void> {
  return invoke("set_hide_on_close", { enabled });
}

export async function autostartIsEnabled(): Promise<boolean> {
  return invoke<boolean>("autostart_is_enabled");
}

export async function autostartEnable(): Promise<void> {
  return invoke("autostart_enable");
}

export async function autostartDisable(): Promise<void> {
  return invoke("autostart_disable");
}

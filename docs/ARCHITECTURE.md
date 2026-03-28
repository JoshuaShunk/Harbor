# Architecture

This document provides a high-level overview of Harbor's architecture and key design decisions.

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Harbor Desktop                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                     React Frontend (Tauri v2)                        │   │
│  │   Servers │ Marketplace │ Hosts │ Settings                          │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                    │ IPC                                     │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                     Tauri Commands (harbor-desktop)                  │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              harbor-core                                     │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ │
│  │   Config   │ │ Connectors │ │  Gateway   │ │   Vault    │ │   Fleet    │ │
│  │            │ │            │ │(Lighthouse)│ │            │ │            │ │
│  │ config.toml│ │ Claude     │ │ HTTP/SSE   │ │ OS Keychain│ │ Git Sync   │ │
│  │ ServerMgr  │ │ Codex      │ │ StdioBridge│ │ vault:refs │ │ Team Share │ │
│  │            │ │ VS Code    │ │ BridgeMgr  │ │            │ │            │ │
│  │            │ │ Cursor     │ │            │ │            │ │            │ │
│  │            │ │ Cline...   │ │            │ │            │ │            │ │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘ └────────────┘ │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                           Marketplace                                    ││
│  │                      MCP Registry Integration                            ││
│  └─────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              harbor-cli                                      │
│  dock │ undock │ fleet │ launch │ port │ sync │ lighthouse │ crew │ ...    │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Crate Structure

### `harbor-core`

The core library containing all business logic. Used by both CLI and desktop.

| Module | Purpose |
|--------|---------|
| `config` | Config file management (`~/.harbor/config.toml`), server definitions |
| `connector` | Host integrations — reads/writes configs for Claude, Codex, VS Code, etc. |
| `gateway` | HTTP/SSE server, StdioBridge for JSON-RPC, BridgeManager for connections |
| `vault` | OS keychain integration via `keyring` crate, `vault:` reference resolution |
| `marketplace` | MCP Registry search and server discovery |
| `fleet` | Team fleet sync — git operations, merge logic, drift detection |

### `harbor-cli`

Clap-based CLI binary. Thin wrapper around `harbor-core`.

### `harbor-desktop`

Tauri v2 application. Bundles the React frontend and exposes Tauri commands that call into `harbor-core`.

## Data Flow

### Server Sync

```
User docks server
        │
        ▼
┌───────────────┐
│ Config Update │  ~/.harbor/config.toml
└───────────────┘
        │
        ▼
┌───────────────┐
│  Connectors   │  Read host configs, merge Harbor servers
└───────────────┘
        │
        ├──────────────────────────────────────────┐
        ▼                  ▼                       ▼
┌─────────────┐    ┌─────────────┐         ┌─────────────┐
│ ~/.claude.json│  │~/.cursor/mcp│  ...    │.vscode/mcp  │
└─────────────┘    └─────────────┘         └─────────────┘
```

### Vault Resolution

```
Server config: API_KEY=vault:MY_KEY
                    │
                    ▼
┌───────────────────────────────┐
│      Gateway (Lighthouse)      │
│  1. Server starts              │
│  2. Resolve vault: refs        │
│  3. Inject real values         │
│  4. Pass to MCP server process │
└───────────────────────────────┘
                    │
                    ▼
        MCP Server receives API_KEY=sk-...
```

Secrets are resolved at runtime, never written to disk.

### Fleet Sync

```
┌─────────────────┐         ┌─────────────────┐
│  Local Config   │         │  Fleet Repo     │
│ config.toml     │◄───────►│ harbor-fleet.toml│
└─────────────────┘  merge  └─────────────────┘
        │                           │
        │                           │
        ▼                           ▼
┌─────────────────┐         ┌─────────────────┐
│ FleetServerDef  │         │ Git Operations  │
│ (no local state)│         │ push/pull/status│
└─────────────────┘         └─────────────────┘
```

- `FleetServerDef` is the shareable subset: command, args, env (with vault refs)
- Local state (`enabled`, `auto_start`, `hosts`) is excluded from fleet
- SHA-256 hash tracks drift since last pull

## Key Design Decisions

### Safe Merges

Connectors never overwrite host configs. They:
1. Read the existing config
2. Parse existing servers
3. Merge Harbor's servers in
4. Write back the combined result

If a user manually edits their host config, those changes persist.

### Vault References

We chose `vault:KEY_NAME` syntax for several reasons:
- Clear visual distinction from literal values
- Works in TOML without escaping
- Easy to parse and resolve
- Makes secrets obvious in config reviews

### Gateway Architecture

The lighthouse starts its HTTP server before initializing MCP servers. This ensures:
- Endpoints are available immediately
- Server initialization is non-blocking
- Hot reload can restart servers without dropping connections

### Fleet State Separation

Fleet sync only shares server definitions, not machine state:
- **Shared:** command, args, env (vault refs only)
- **Local:** enabled, auto_start, hosts, tool_hosts

This lets each team member customize which servers run on their machine.

### Git Shell-out

Fleet git operations shell out to the `git` binary rather than using libgit2. This:
- Leverages user's existing SSH keys and credential helpers
- Avoids linking OpenSSL
- Matches user expectations for git behavior

## Security Model

1. **Secrets in Keychain** — Never in plain text configs
2. **Vault Resolution at Runtime** — Gateway resolves just before spawning servers
3. **Fleet Refs Only** — `vault:KEY` committed, not actual values
4. **Per-Machine Provisioning** — Each user provisions their own secrets

See [SECURITY.md](../SECURITY.md) for vulnerability reporting.

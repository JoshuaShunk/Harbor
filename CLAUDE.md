# Harbor

Universal MCP Hub — desktop app + CLI that manages MCP servers across Claude Code, Codex, VS Code, and Cursor.

## Tech Stack

Rust + Tauri v2 + React 19 + TypeScript + Tailwind CSS + Axum

## Project Structure

- `crates/harbor-core` — Core library (config, connectors, gateway, vault, marketplace, fleet)
- `crates/harbor-cli` — CLI binary (clap)
- `crates/harbor-desktop` — Tauri v2 desktop app
- `ui/` — React frontend (Vite + Tailwind)

## Build & Test

```sh
# Frontend
cd ui && npm ci && npm run build

# CLI
cargo build -p harbor-cli

# Desktop (requires Tauri prerequisites)
cd crates/harbor-desktop && cargo tauri dev

# Tests
cargo test --workspace

# Linting
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## Key Conventions

- Nautical naming theme for CLI commands (dock, undock, fleet, launch, anchor, port, sync, lighthouse, scout, chest, cargo, crew)
- Config stored at `~/.harbor/config.toml`
- Connectors do safe merges into host configs (never overwrite)
- Syncing routes through the lighthouse gateway for tool filtering, vault resolution, and hot reload
- Host configs are auto-synced on dock/undock/toggle/connect
- Desktop app auto-starts the lighthouse on launch
- Users opt in to each host individually via `harbor port link <host>` or the Link button in the UI
- `vault:` prefix in env vars references OS keychain secrets (resolved at runtime by the gateway)
- Gateway starts HTTP server before initializing MCP servers (non-blocking)

## Fleet Sync (harbor crew)

- Fleet repo lives at `~/.harbor/fleet/` — a git repo containing `harbor-fleet.toml`
- Fleet state (pull hashes) persisted at `~/.harbor/fleet-state.json`
- `harbor_core::fleet` module: `config`, `git`, `merge`, `provision`, `state`
- `FleetServerDef` is the shareable subset of `ServerConfig` — omits `enabled`, `auto_start`, `hosts`, `tool_hosts`
- Servers with `source = "fleet"` in local config are fleet-managed
- Hash-based drift detection: SHA-256 of `FleetServerDef` stored after each pull; mismatches produce `LocallyModified` on the next pull (user changes are never silently overwritten)
- Fleet git operations shell out to the `git` binary (no libgit2) — leverages user's existing SSH keys and credential helpers
- Desktop: `fleet_status` and `fleet_pull` Tauri commands; server cards show `fleet` / `modified` badges; Settings has a Crew section with Pull button

# Harbor

Universal MCP Hub — desktop app + CLI that manages MCP servers across Claude Code, Codex, VS Code, and Cursor.

## Tech Stack

Rust + Tauri v2 + React 19 + TypeScript + Tailwind CSS + Axum

## Project Structure

- `crates/harbor-core` — Core library (config, connectors, gateway, vault, marketplace)
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

- Nautical naming theme for CLI commands (dock, undock, fleet, launch, anchor, signal, lighthouse, scout, chest)
- Config stored at `~/.harbor/config.toml`
- Connectors do safe merges into host configs (never overwrite)
- `vault:` prefix in env vars references OS keychain secrets (resolved at sync time)
- Gateway starts HTTP server before initializing MCP servers (non-blocking)

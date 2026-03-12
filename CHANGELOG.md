# Changelog

All notable changes to Harbor will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.6] - 2026-03-11

### Added

- `harbor port link` now supports Cline, Roo Code, and Windsurf (previously UI-only)
- `harbor launch --detach` / `-d` — start a server in the background and return immediately
- `harbor anchor <name>` — stop a detached server by sending SIGTERM to its stored PID (macOS/Linux) or via `taskkill` (Windows)
- PID file store at `~/.harbor/run/<name>.pid` tracks detached processes across CLI invocations
- "Keep running when closed" setting — hides the app to the system tray instead of quitting, so the lighthouse stays running (default: on)
- "Start at login" setting — registers Harbor as a login item so it launches automatically on startup (default: off)
- "Open Harbor" item in the system tray menu to restore the window when hidden
- Settings toggles for both new behaviours in the General section

## [0.5.5] - 2026-03-11

### Added

- `harbor crew` command group for git-backed team fleet sync (`init`, `join`, `push`, `pull`, `status`, `provision`)
- `harbor crew init [--git <url>]` — initialize `~/.harbor/fleet/` as a git repo with optional remote
- `harbor crew join <git-url>` — clone a team fleet and auto-merge on first pull
- `harbor crew push [<servers>] [-m <msg>]` — mark servers as fleet-managed and commit/push to the shared repo
- `harbor crew pull [--dry-run]` — fetch upstream and 3-way merge fleet servers into local config
- `harbor crew status` — show git divergence (ahead/behind) and per-server state
- `harbor crew provision [--dry-run]` — scan fleet for missing vault secrets and prompt to stow them
- Hash-based drift detection: SHA-256 of `FleetServerDef` tracks hand-edits since the last pull; locally modified servers are skipped on pull with clear resolution hints
- Per-machine state (enabled, auto_start, host connections) is excluded from the fleet definition and never overwritten by a pull
- Fleet source badge on server cards in the desktop app (blue "fleet" chip)
- "modified" badge on server cards when local edits drift from the last pull
- Crew section in Settings page: fleet remote URL, git ahead/behind status, Pull button
- 44 new unit tests across all five fleet modules (config round-trips, merge logic, git subprocess smoke tests, provision integration)

## [0.3.3] - 2026-03-04

### Added

- `harbor update` command — self-update CLI via GitHub releases with SHA256 verification
- Startup version check — passive "update available" notice on every command (cached 24h)

## [0.2.4] - 2026-03-03

### Added

- `harbor scuttle` (alias: `uninstall`) command with `--purge`, `--dry-run`, and `--yes` flags
- `--uninstall` flag for install script (`curl -fsSL https://harbormcp.ai/install.sh | sh -s -- --uninstall`)

## [0.2.3] - 2026-03-03

### Added

- CLI binary builds in release workflow (macOS x86/arm, Linux x86/arm)
- CLI binary bundled inside macOS .app for symlink-based install
- Install script at harbormcp.ai/install.sh (`curl -fsSL https://harbormcp.ai/install.sh | sh`)

## [0.2.2] - 2026-03-03

### Added

- Dark mode logo and theme-aware sidebar styling
- README, community files, and GitHub repository configuration

### Changed

- Settings page updated with theme-aware styling

### Removed

- Logo from Dry Dock settings section

## [0.2.1] - 2026-03-03

### Changed

- New app icon and sidebar logo with cargo ship branding

## [0.2.0] - 2026-03-03

### Added

- Light/dark/system appearance toggle in Settings with full CSS variable support and localStorage persistence

### Changed

- Vault consolidated to a single keychain entry (JSON blob) so only one OS prompt is needed after binary signature changes, with lazy migration from the old per-key format
- Bridge env resolution delegated to Vault.resolve_env instead of duplicating logic inline

### Fixed

- Redundant border on version footer removed

## [0.1.0] - 2026-03-03

### Added

- Core library with config management and server manager
- CLI with add, remove, list, start, stop, status, and sync commands
- Connectors for Claude Code, Codex, VS Code, and Cursor (safe merge into host configs)
- Gateway with HTTP/SSE endpoint, StdioBridge JSON-RPC, and BridgeManager
- Desktop app built with Tauri v2 and React, featuring sidebar layout and 4 pages
- Auth vault using OS keychain via `keyring` crate
- Smithery marketplace search integration
- `vault:` references in env vars resolved at sync time

[Unreleased]: https://github.com/JoshuaShunk/Harbor/compare/v0.5.6...HEAD
[0.5.6]: https://github.com/JoshuaShunk/Harbor/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/JoshuaShunk/Harbor/compare/v0.3.3...v0.5.5
[0.2.4]: https://github.com/JoshuaShunk/Harbor/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/JoshuaShunk/Harbor/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/JoshuaShunk/Harbor/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/JoshuaShunk/Harbor/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/JoshuaShunk/Harbor/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/JoshuaShunk/Harbor/releases/tag/v0.1.0

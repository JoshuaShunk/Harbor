# Changelog

All notable changes to Harbor will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/JoshuaShunk/Harbor/compare/v0.2.4...HEAD
[0.2.4]: https://github.com/JoshuaShunk/Harbor/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/JoshuaShunk/Harbor/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/JoshuaShunk/Harbor/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/JoshuaShunk/Harbor/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/JoshuaShunk/Harbor/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/JoshuaShunk/Harbor/releases/tag/v0.1.0

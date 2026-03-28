# Frequently Asked Questions

## General

### What is Harbor?

Harbor is a universal MCP (Model Context Protocol) hub that manages MCP servers across multiple AI coding assistants. Instead of manually editing config files for each tool, you configure servers once in Harbor and sync them everywhere.

### Which hosts does Harbor support?

Harbor supports:
- **Claude Code** — `~/.claude.json`
- **Codex** — `~/.codex/config.toml`
- **VS Code** — `.vscode/mcp.json`
- **Cursor** — `~/.cursor/mcp.json`
- **Cline** — VS Code extension config
- **Roo Code** — VS Code extension config
- **Windsurf** — `~/.windsurf/mcp.json`

### Does Harbor replace my existing configs?

No. Harbor performs safe merges — it adds its servers alongside your existing ones and never overwrites your manual configurations.

## Vault & Secrets

### How do I store a secret?

```sh
harbor chest set MY_API_KEY sk-...
```

The secret is stored in your OS keychain (macOS Keychain, Windows Credential Manager, or Linux Secret Service).

### How do vault references work?

When you set an environment variable to `vault:SECRET_NAME`, Harbor resolves it at runtime:

```sh
harbor dock --name my-server --command npx --args my-mcp-server \
  --env API_KEY=vault:MY_API_KEY
```

Your secrets never appear in plain-text config files.

### Why am I getting keychain prompts?

Your OS may prompt for keychain access when:
- Harbor is first installed or updated (new binary signature)
- After a system restart
- If you have strict keychain settings

This is normal security behavior.

### Are my secrets shared in fleet sync?

No. Fleet sync only commits vault references (e.g., `vault:MY_API_KEY`), not actual secret values. Each team member must provision their own secrets with `harbor crew provision`.

## Host Sync

### Why aren't my servers appearing in Claude Code / VS Code / etc?

1. **Check if the host is linked:** `harbor port` shows linked hosts
2. **Link the host:** `harbor port link claude`
3. **Verify sync:** `harbor sync` to manually trigger a sync
4. **Restart the host app** — some hosts require a restart to pick up config changes

### Can I sync different servers to different hosts?

Yes. Use the `hosts` section in your server config:

```toml
[servers.github.hosts]
claude = true
codex = true
vscode = false
cursor = false
```

Or via CLI flags when docking.

### Why does VS Code use a different config location?

VS Code uses `.vscode/mcp.json` in your workspace root (project-level config) while most other hosts use global user configs. This is by design in the MCP specification for VS Code.

## Fleet Sync

### What is fleet sync?

Fleet sync (`harbor crew`) lets teams share MCP server configurations via a git repository. Server definitions are synced while secrets stay local.

### How do I set up fleet sync for my team?

1. **Initialize:** `harbor crew init --git git@github.com:your-org/fleet.git`
2. **Push servers:** `harbor crew push github linear slack`
3. **Share the repo URL with teammates**
4. **Teammates join:** `harbor crew join git@github.com:your-org/fleet.git`
5. **Provision secrets:** `harbor crew provision`

### What does "locally modified" mean?

If you manually edit a fleet-managed server in your local config, Harbor detects the drift and marks it as "locally modified". On the next `harbor crew pull`, that server won't be overwritten — you'll see a warning with resolution hints.

### How do I reset a locally modified server?

To accept the upstream version and discard your local changes:
1. Delete the server: `harbor undock server-name`
2. Pull again: `harbor crew pull`

## Gateway (Lighthouse)

### What is the lighthouse?

The lighthouse is Harbor's gateway server. It provides:
- HTTP/SSE endpoints for tool discovery and remote access
- Tool filtering and access control
- Vault resolution at runtime
- Hot reload when configs change

### How do I start the lighthouse?

The desktop app starts it automatically. For CLI-only usage:

```sh
harbor lighthouse --port 3100
```

### What endpoints does the lighthouse expose?

- `GET /tools` — List available tools
- `POST /mcp` — JSON-RPC endpoint for MCP
- `GET /sse` — Server-sent events for streaming

## Troubleshooting

### Harbor can't find my config file

Ensure `~/.harbor/config.toml` exists. You can create it with:

```sh
harbor fleet  # creates config if missing
```

### Commands hang or timeout

If commands hang, the gateway might be stuck. Try:

1. Stop any running lighthouse: `harbor anchor lighthouse`
2. Check for orphaned processes: `ps aux | grep harbor`
3. Clear the run directory: `rm ~/.harbor/run/*.pid`

### How do I completely uninstall Harbor?

```sh
harbor scuttle --purge --yes
```

This removes the CLI, configs, and keychain entries. Add `--dry-run` first to preview what will be removed.

# Contributing to Harbor

Thanks for your interest in contributing to Harbor! Here's how to get started.

## Development Setup

### Prerequisites

- [Rust](https://rustup.rs/) (1.75+)
- [Node.js](https://nodejs.org/) (20+)
- [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)

### Getting Started

```sh
# Clone the repo
git clone https://github.com/JoshuaShunk/Harbor.git
cd Harbor

# Install frontend dependencies
cd ui && npm ci && cd ..

# Run the desktop app in development
cd crates/harbor-desktop && cargo tauri dev

# Or just build the CLI
cargo build -p harbor-cli
```

### Project Structure

```
Harbor/
├── crates/
│   ├── harbor-core       # Core library (config, connectors, gateway, vault)
│   ├── harbor-cli        # CLI binary
│   └── harbor-desktop    # Tauri desktop app
└── ui/                   # React frontend
    └── src/
        ├── pages/        # Servers, Hosts, Marketplace, Settings
        ├── components/   # Shared components
        └── contexts/     # Theme, updates
```

## Making Changes

1. **Fork the repo** and create a branch from `main`
2. **Make your changes** — keep them focused and minimal
3. **Test your changes** — run `cargo build --workspace` at minimum
4. **Submit a pull request** — describe what you changed and why

## Guidelines

- Keep PRs focused on a single change
- Follow existing code style and patterns
- Write descriptive commit messages
- If adding a new CLI command, follow the nautical naming theme
- If modifying connector behavior, test against the affected host config format

## Reporting Bugs

Open an [issue](https://github.com/JoshuaShunk/Harbor/issues/new?template=bug-report.yml) with:

- Steps to reproduce
- Expected vs actual behavior
- Your OS and Harbor version

## Suggesting Features

Open a [discussion](https://github.com/JoshuaShunk/Harbor/discussions) or [feature request](https://github.com/JoshuaShunk/Harbor/issues/new?template=feature-request.yml) describing:

- The problem you're trying to solve
- How you'd like it to work
- Any alternatives you've considered

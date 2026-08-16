# Contributing to CronBox

CronBox is a local-first CLI service with a Vue control panel and Rust scheduler. Keep changes practical, inspectable, and focused.

## Development Setup

Install Bun and Rust stable, then run:

```bash
bun install
bun run build
bun run server
```

The server is available at `http://127.0.0.1:4317`. For frontend hot reload, run `bun run dev` in a second terminal and use `http://127.0.0.1:5188`; Vite proxies `/api` to the Rust server.

Before opening a pull request, run:

```bash
bun run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
```

## Pull Requests

- Keep changes focused and reviewable.
- Add or update Rust tests when scheduler, API, database, CLI, or executor behavior changes.
- Manually verify affected browser workflows.
- Update documentation for user-visible behavior.
- Avoid dependencies that do not remove meaningful complexity.
- Do not commit `dist`, build outputs, local databases, logs, or secrets.

## Product Direction

Prefer changes that improve recurring local work: reliable scheduling, clear command status, useful logs, explicit script discovery, and straightforward recovery. Production startup uses a per-user macOS LaunchAgent or Linux systemd user service; the management interface is a local Web console. Do not add desktop-window or tray dependencies.

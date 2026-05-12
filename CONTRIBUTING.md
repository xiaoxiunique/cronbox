# Contributing to CronBox

Thanks for helping improve CronBox. This project is a local-first desktop app, so changes should stay practical, inspectable, and conservative by default.

## Development Setup

Requirements:

- Bun
- Rust stable
- Platform dependencies required by Tauri 2

Install dependencies and start the app:

```bash
bun install
bun run tauri dev
```

Run the frontend build:

```bash
bun run build
```

Run Rust tests:

```bash
cd src-tauri
cargo test
```

Run formatting checks before opening a pull request:

```bash
cd src-tauri
cargo fmt -- --check
```

## Pull Requests

- Keep changes focused and reviewable.
- Add or update tests when behavior changes.
- Update README or other docs when user-visible behavior changes.
- Avoid new dependencies unless they remove meaningful complexity.
- Do not commit build outputs, local databases, logs, or generated package artifacts.

## Product Direction

CronBox is intended to feel like a compact local operations tool:

- local-first scheduler
- menu bar runtime
- script entrypoint scanning
- history-first log views
- CLI parity for core management
- coding-agent tasks that run from explicit local workspaces

Prefer changes that make recurring local work easier to run, inspect, and recover from.

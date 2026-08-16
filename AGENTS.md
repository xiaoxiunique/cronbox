# Repository Guidelines

## Project Structure & Module Organization

CronBox is a CLI-hosted local service with a Vue 3/TypeScript control panel and Rust backend. Frontend code lives in `src/`: views are under `src/views`, reusable components under `src/components`, and HTTP bindings and helpers under `src/lib`. Rust code remains in `src-tauri/src`; executors are grouped in `src-tauri/src/executor`, the local HTTP server is `src-tauri/src/web.rs`, and integration tests are in `src-tauri/tests`. Supporting scripts, site files, and the bundled agent skill live in `scripts/`, `sites/cronbox`, and `skills/`.

## Build, Test, and Development Commands

- `bun install`: install frontend dependencies.
- `bun run build`: type-check Vue and build assets embedded by Rust.
- `bun run server`: start the scheduler and Web console at `127.0.0.1:4317`.
- `bun run dev`: run Vite with `/api` proxied to the Rust server.
- `cronbox service status`: inspect the macOS LaunchAgent or Linux systemd user service.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust unit and integration tests.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: verify Rust formatting.
- `bun run install-cli`: build and install the local binary.

Build the frontend before compiling Rust from a clean checkout because `rust-embed` packages `dist` into the executable.

## Coding Style & Naming Conventions

Follow `.editorconfig`: UTF-8, LF endings, final newline, trimmed trailing whitespace, two-space indentation by default, and four spaces for Rust. Use strict TypeScript and Vue `<script setup lang="ts">`. Name Vue files in PascalCase, functions and refs in camelCase, Rust modules in `snake_case`, and Rust types in PascalCase. Format Rust with `cargo fmt`.

## Testing Guidelines

Add tests for scheduler, HTTP API, database, CLI, and executor behavior. Put cross-module scenarios in `src-tauri/tests/integration.rs`; keep focused unit tests beside their modules. Use descriptive `test_...` names. Frontend changes require `bun run build` plus manual browser checks at desktop and mobile widths. No coverage threshold is enforced.

## Commit & Pull Request Guidelines

Use short imperative subjects, such as `Add one-shot schedules`. Keep pull requests focused; include a description, linked issues when applicable, tests run, and screenshots for UI changes. Do not commit build outputs, local databases, logs, generated packages, secrets, or AI co-author lines.

## Security Tips

The HTTP API can execute local scripts. Keep it bound to loopback, validate paths and arguments, and avoid adding permissive CORS. LaunchAgent and systemd changes must remain per-user and require no root access. Report vulnerabilities through `SECURITY.md`.

# Repository Guidelines

## Project Structure & Module Organization

CronBox is a Tauri 2 desktop app with a Vue 3/TypeScript frontend and Rust backend. Frontend source lives in `src/`: views are in `src/views`, reusable UI in `src/components`, and shared helpers/API bindings in `src/lib`. Rust application code lives in `src-tauri/src`, with executor implementations under `src-tauri/src/executor`. Rust integration tests are in `src-tauri/tests`. Static app metadata, icons, capabilities, and packaging config live under `src-tauri/`. Supporting assets and docs include `sites/cronbox`, `skills/cronbox.md`, and `scripts/install-cli.sh`.

## Build, Test, and Development Commands

- `bun install`: install frontend and Tauri CLI dependencies.
- `bun run tauri dev`: start the desktop app in development mode.
- `bun run build`: type-check the Vue/TypeScript frontend with `vue-tsc` and build with Vite.
- `cd src-tauri && cargo test`: run Rust unit and integration tests.
- `cd src-tauri && cargo fmt -- --check`: verify Rust formatting.
- `bun run install-cli`: run the helper that installs the `cronbox` CLI link.

## Coding Style & Naming Conventions

Follow `.editorconfig`: UTF-8, LF endings, final newline, trimmed trailing whitespace, two-space indentation by default, and four-space indentation for Rust. Use strict TypeScript and Vue single-file components with `<script setup lang="ts">` as seen in existing views. Keep frontend filenames in PascalCase for components/views and camelCase for functions and refs. Rust modules use `snake_case`; types and enums use `PascalCase`.

## Testing Guidelines

Add Rust tests for scheduler, database, CLI, and executor behavior changes. Place integration tests in `src-tauri/tests` and name tests descriptively, for example `test_bash_schedule_env_is_injected`. Frontend changes currently rely on `bun run build` for type and compile validation; manually verify affected Tauri UI flows when behavior changes.

## Commit & Pull Request Guidelines

Use short imperative commit subjects matching project history, such as `Add one-shot schedules` or `Tighten the Schedules page row`. Keep pull requests focused and reviewable. Include a clear description, linked issues when relevant, screenshots or short recordings for UI changes, and notes on tests run. Update README or related docs when user-visible behavior changes. Do not commit build outputs, local databases, logs, generated package artifacts, secrets, or AI co-author lines.

## Security & Configuration Tips

CronBox is local-first and executes user scripts, so treat paths, environment variables, and command arguments carefully. Avoid broad permissions or new dependencies unless they remove meaningful complexity. Report security issues through `SECURITY.md` rather than public issues.

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
bun install                  # install frontend deps
bun run tauri dev            # run the full desktop app (Vite + Tauri)
bun run dev                  # Vite dev server only (port 5188, strict)
bun run build                # vue-tsc --noEmit typecheck + vite build → dist/

cargo test   --manifest-path src-tauri/Cargo.toml            # all Rust tests
cargo test   --manifest-path src-tauri/Cargo.toml <name>     # single test by name
cargo fmt    --manifest-path src-tauri/Cargo.toml -- --check # CI enforces this
```

CI (`bun run build` → `cargo fmt --check` → `cargo test`) builds the frontend
**before** running Rust tests, because `tauri::generate_context!()` needs `dist/`.
Run `bun run build` once locally before `cargo test` if `dist/` is stale.

## Architecture

CronBox is a Tauri 2 desktop app: Rust backend (`src-tauri/`), Vue 3 + TypeScript
frontend (`src/`). It is a local-first menu-bar scheduler that runs script files on
cron schedules.

### One binary, two modes
The compiled `cronbox` binary is **both** the GUI app and the `cronbox` CLI.
`src-tauri/src/main.rs::should_run_cli` picks the mode at startup from argv0 / args /
`TAURI_ENV_TARGET_TRIPLE`: launched from an `.app` bundle → GUI; invoked from a
terminal → CLI. GUI mode calls `lib::run()`; CLI mode calls `cli::run_from_env()`.

Both modes operate on the **same SQLite database** at
`<data_dir>/com.cronbox.app/cronbox.db`. The DB is the single source of truth — the
CLI never talks to the running GUI, it just reads/writes the shared DB.

### Backend layers (`src-tauri/src/`)
- `lib.rs` — Tauri app entry. On `setup` it spawns two Tokio loops: the scheduler and
  a 2-second tray-refresh loop. Registers all IPC commands in `invoke_handler`. Window
  close is intercepted (`prevent_close` → `hide`) so schedules keep running in the tray.
- `db.rs` — `Database` wraps a `rusqlite` connection (WAL mode, `foreign_keys=ON`).
  Schema + idempotent `migrate()` live here. `open_in_memory()` is for tests.
- `models.rs` — shared structs/enums (`ScriptLanguage`, `JobStatus`, `Schedule`, `Job`,
  …), all `serde`-serialized for IPC. Mirrored by TypeScript interfaces in `src/lib/api.ts`.
- `state.rs` — `AppState { db, db_path }` + filesystem script discovery. `scan_scripts`
  walks registered work dirs; a `scan_mode` of `"auto"` recurses, `"manual"` surfaces
  only rows in `script_entries`. `is_entry_script` is the heuristic gate (shebang /
  Python `__main__` guard / JS `import.meta.main` / executable bit / non-empty SQL).
- `scheduler.rs` — `run_scheduler` ticks every 1s, finds due schedules, and for each:
  skips (records a `skipped` job) if a prior run is still active, else creates a job,
  recomputes `next_run_at` via `croner`, and spawns `execute_job` as a detached task.
  Each detached job opens its **own** `Database` connection (no shared lock held).
- `executor/` — one module per language (`bash`, `bun`, `python`, `pgsql`) behind
  `executor::execute_with_log_callback`. `LogCallback` streams stdout/stderr chunks
  into the job's `logs` column live. `python::json_to_cli_args` is shared by bash/bun.
- `commands.rs` — `#[tauri::command]` IPC handlers (the GUI's API surface).
- `cli.rs` — CLI subcommands (`add`, `dirs`, `scripts`, `schedules`, `jobs`, `run`),
  reimplementing the same operations as `commands.rs` against the shared DB.
- `tray.rs` — system tray menu/icon.

### Argument passing
Schedules store `args` as a JSON string. Executors pass it three ways at once: the raw
JSON in `CRONBOX_ARGS`, converted CLI flags (`json_to_cli_args`), and per-key env vars
(`ARG_<KEY>` for Python, bare `$KEY` for bash). Scripts can consume whichever they prefer.

### Frontend (`src/`)
Vue 3 `<script setup>` + `vue-router` (`main.ts`). Views in `src/views/`,
shared components in `src/components/`. `src/lib/api.ts` wraps every Tauri command via
`invoke()` and re-declares the backend types — **keep it in sync with `models.rs`**.

### Agent tasks
`create_codex_task` / `create_claude_task` generate executable `.sh` task scripts under
the `~/.cronbox` workspace (`cli.rs::default_agent_workspace_path`), which ships its own
`AGENTS.md` / `CLAUDE.md` for durable scheduled-agent rules.

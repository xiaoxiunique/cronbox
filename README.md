# CronBox

CronBox is a local-first CLI scheduler for scripts and coding-agent tasks. The `cronbox` process runs the scheduler and serves a browser-based control panel on your computer.

```bash
cronbox
```

This opens `http://127.0.0.1:4317`, where you can inspect scripts, schedules, run history, live logs, and current command status. The server binds to loopback only.

## Features

- Discover executable Bash, Python, Bun, and PostgreSQL entry scripts.
- Run scripts manually or on recurring and one-shot cron schedules.
- View queued, running, successful, failed, skipped, and cancelled jobs.
- Stream stdout and stderr into persistent SQLite-backed job logs.
- Skip overlapping runs for the same schedule.
- Create Codex and Claude Code tasks in explicit local workspaces.
- Manage directories, scripts, schedules, jobs, skills, export, and import from the CLI.

## Installation

Install the latest macOS binary and LaunchAgent:

```bash
curl -fsSL https://raw.githubusercontent.com/xiaoxiunique/cronbox/main/scripts/install-macos.sh | bash
```

The installer selects the correct Apple Silicon or Intel release, installs `cronbox` into a writable bin directory, and registers a per-user LaunchAgent. CronBox starts at login and restarts after unexpected exits. Set `CRONBOX_VERSION=v0.1.0` to install a specific tag or `CRONBOX_CLI_TARGET` to choose the destination.

Install the Linux x64 binary and systemd user service:

```bash
curl -fsSL https://raw.githubusercontent.com/xiaoxiunique/cronbox/main/scripts/install-linux.sh | bash
```

The Linux installer writes `~/.config/systemd/user/cronbox.service`, enables it for the user's default target, and starts it immediately. On headless machines that must keep running after logout, enable systemd user lingering once with `loginctl enable-linger "$USER"` if permitted by the host.

## Development

Requirements: Bun and Rust stable.

```bash
bun install
bun run build
bun run server
```

The production frontend is embedded in the Rust binary. For frontend hot reload, keep `bun run server` running and start `bun run dev` in another terminal, then open `http://127.0.0.1:5188`.

Run checks:

```bash
bun run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
```

## CLI

Running `cronbox` without arguments starts the scheduler and Web console. Use `cronbox serve --port 4400 --no-open` to choose a port or suppress automatic browser launch. `cronbox daemon` runs the scheduler without the Web console.

Manage the background service with the same commands on macOS and Linux:

```bash
cronbox service install
cronbox service status
cronbox service restart
cronbox service logs --follow
cronbox service stop
cronbox service uninstall
```

macOS uses `launchd`; Linux uses `systemd --user`. On Linux, `cronbox service logs --follow` reads the user journal.

```bash
cronbox add ~/scripts/cleanup.sh --alias "Clean temporary files"
cronbox add ~/scripts --include daily-report.sh
cronbox scripts list
cronbox schedules add ./daily-report.sh "0 9 * * *" --tz Asia/Shanghai
cronbox schedules add ./reminder.sh "* * * * *" --once
cronbox jobs list
cronbox run ~/scripts daily-report.sh --args '{"date":"today"}'
cronbox export --out cronbox.json
```

The CLI, LaunchAgent, and Web console share the same local database. A scheduler lock prevents duplicate execution; a standby Web process automatically takes over when the active scheduler exits.

## Agent Tasks

CronBox can generate task scripts for Codex and Claude Code. The default workspace is `~/.cronbox` and includes `AGENTS.md` and `CLAUDE.md` for durable task instructions.

```bash
cronbox skills install
```

## Packaging

Release builds embed the Vue application into a single Rust binary. Tags matching `v*` create draft releases with macOS, Linux, and Windows archives.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md). Report security issues through the private channel in [SECURITY.md](SECURITY.md).

## License

MIT. See [LICENSE](LICENSE).

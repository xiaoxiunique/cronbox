# CronBox

CronBox is a local-first menu bar scheduler for scripts and coding-agent tasks.

It scans folders for executable entry scripts, lets you run them manually or on recurring and one-shot schedules, and keeps run history with live logs in one place. Scheduled jobs keep running while the main window is closed, as long as CronBox is still active in the menu bar.

## Status

CronBox is early-stage software. Expect rough edges around packaging, platform-specific permissions, and coding-agent task execution. Issues and focused pull requests are welcome.

## Features

- Add script directories and only surface files with executable entrypoints.
- Schedule Bash, Python, Bun, PostgreSQL, Codex, and Claude task scripts as recurring jobs or one-shot runs.
- Keep aliases, cron expressions, timezone, JSON args, enabled state, one-shot state, and history per script.
- View run history and logs in a two-pane detail view.
- Stream stdout and stderr while a job is running.
- Skip a scheduled run when the previous run for the same schedule is still queued or running.
- Send macOS notifications when scheduled jobs complete.
- Manage directories, selected script entries, schedules, jobs, agent skills, and manual runs from the `cronbox` CLI.
- Create Codex and Claude Code task scripts in the default `~/.cronbox` workspace.

## Development

CronBox is built with Tauri 2, Vue 3, TypeScript, and Rust.

Requirements:

- Bun
- Rust stable
- Tauri 2 platform prerequisites

```bash
bun install
bun run tauri dev
```

Build the frontend:

```bash
bun run build
```

Run Rust checks and tests:

```bash
cd src-tauri
cargo test
```

## Installation

Install the latest published macOS release:

```bash
curl -fsSL https://raw.githubusercontent.com/xiaoxiunique/cronbox/main/scripts/install-macos.sh | bash
```

The installer copies `CronBox.app` to `/Applications` and links the `cronbox` CLI into a writable bin directory such as `/opt/homebrew/bin`, `/usr/local/bin`, or `~/.local/bin`. To install a specific tag, set `CRONBOX_VERSION`, for example:

```bash
curl -fsSL https://raw.githubusercontent.com/xiaoxiunique/cronbox/main/scripts/install-macos.sh | env CRONBOX_VERSION=v0.1.0 bash
```

## Packaging

GitHub Actions builds installers for macOS, Linux, and Windows.

- Pushes to `main` and manual workflow runs upload package artifacts.
- Tags matching `v*`, for example `v0.1.0`, also create a draft GitHub Release with the generated installers attached.

Create a release build:

```bash
git tag v0.1.0
git push origin v0.1.0
```

## CLI

The desktop app installs the `cronbox` command on startup when possible. The CLI shares the same local data as the desktop app, but it does not open or control the UI. Running `cronbox` without arguments prints usage help.

You can also install it manually from Settings or with:

```bash
bun run install-cli
```

Common commands:

```bash
cronbox add ~/scripts
cronbox add ~/scripts --include daily-report.sh
cronbox add ~/scripts/cleanup.sh --alias "Clean temporary files"
cronbox scripts list
cronbox schedules add ./daily-report.sh "0 9 * * *" --tz Asia/Shanghai
cronbox schedules add ./reminder.sh "* * * * *" --once
cronbox jobs list
cronbox run ~/scripts daily-report.sh --args '{"date":"today"}'
cronbox skills install
```

`cronbox add <directory>` previews only scripts with executable entrypoints. Use `--include <relative-script>` to add selected entries, repeat `--include` for several scripts, or use `--all` for script directories where every entrypoint should be registered. `cronbox schedules add ... --once` creates a one-shot schedule that disables itself after its first trigger.

## Agent Tasks

CronBox can generate local task scripts for Codex and Claude Code. By default those scripts run from:

```text
~/.cronbox
```

The workspace includes `AGENTS.md` and `CLAUDE.md` so you can keep durable rules for scheduled coding-agent tasks.

To install the bundled Claude Code skill for scheduling recurring local tasks through CronBox, run:

```bash
cronbox skills install
```

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md) before opening a pull request. Please report security issues through the private channel described in [SECURITY.md](SECURITY.md).

## License

MIT. See [LICENSE](LICENSE).

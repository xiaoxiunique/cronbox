---
name: cronbox
description: Schedule a script to run on a recurring local cron via CronBox. Write the script into the user's current project, then register it with the `cronbox` CLI. Use when the user wants something to run periodically on their machine — phrases like "schedule", "every day at", "every X minutes", "cron", "recurring", "定时", "每天", "每隔", "每周". Not for one-off runs.
---

# CronBox — schedule scripts on the user's local machine

CronBox is a local-first scheduler. The user's menu-bar app holds the SQLite DB and runs scheduled jobs; the `cronbox` CLI is how you create and manage them. This skill is how you, the AI, set up a scheduled task end-to-end without the user having to touch CronBox's UI.

## Step 0 — Verify CronBox is reachable

Before promising anything, check the CLI is on PATH and the DB opens:

```bash
cronbox dirs list
```

- Exit 0 → CronBox is installed. Proceed.
- `command not found: cronbox` → tell the user the CronBox menu-bar app isn't installed (or the CLI symlink isn't on PATH). Stop.
- DB errors → tell the user to launch the CronBox menu-bar app once (it creates the DB on first launch). Stop.

**Important — surface this to the user every time you finish setting up a schedule:** CronBox only fires scheduled jobs while its menu-bar app is open. If the user quits it, schedules pause.

## Step 1 — Write the script in the user's project

### Choose the script's form first

Two valid shapes — pick based on what the task actually needs each time:

- **Deterministic script** — same logic every tick (fetch a URL, run a query, copy files, send a webhook). Write bash/python/bun/SQL. Most "every X minutes/hours/days" tasks are this.
- **AI-at-runtime** — the task needs *ongoing judgment* (summarize the repo, code-review the latest diff, write a status note from changing context, decide whether to alert based on current state). Make the script body invoke an AI agent with the prompt embedded:

  ```bash
  #!/usr/bin/env bash
  set -euo pipefail
  cd /abs/path/to/project
  claude -p "<the prompt the task needs each time>" --output-format text
  # or: codex exec "<prompt>"
  ```

  CronBox treats this as a normal script — captures stdout as logs, marks failure on non-zero exit, respects the schedule. The judgment happens inside the agent at runtime.

Pick **AI-at-runtime** when "what to do" depends on current state that can't be enumerated up front (changing files, latest commits, recent logs, evolving conditions). Pick **deterministic** otherwise — it's cheaper, faster, and easier to reason about.

### Write the file

Drop the script **into the user's current project**, not into `~/.cronbox`. The user wants scripts version-controlled in their repo. Pick a sensible relative path like `scripts/<name>.{sh,py,ts}`.

Supported languages (auto-detected by extension):

- `.sh` — bash. Needs a shebang **or** the executable bit set.
- `.py` — python. Needs `if __name__ == "__main__":` **or** a shebang. If the project has `pyproject.toml` / `uv.lock`, CronBox auto-uses `uv run python`.
- `.ts` / `.js` / `.mts` / `.mjs` — bun. Needs `import.meta.main` / `require.main === module` / `Bun.main` / a shebang. `node_modules/.bin` is prepended to PATH.
- `.sql` — runs against PostgreSQL using connection args in `--args`.

Make the script self-contained:
- Print useful output to stdout — CronBox captures it as the job's logs.
- Exit non-zero on failure so CronBox marks the run as failed.
- Don't assume any cwd other than the script's own directory.

## Step 2 — Register the script and add a schedule

```bash
# Register the script with a short human description (~5-8 words).
# The description shows in CronBox's UI in place of the filename, so the user
# can scan their list and tell at a glance what each task does.
# This also adds the script's parent directory in MANUAL mode and registers
# just this one file, so the rest of the user's codebase is not pulled in.
cronbox add /abs/path/to/scripts/my-task.sh \
  --alias "Daily funding rate snapshot"

# Create a schedule. 5-field cron, explicit timezone.
cronbox schedules add \
  /abs/path/to/scripts/my-task.sh \
  "0 9 * * *" \
  --tz "Asia/Shanghai"
```

If the script takes arguments, pass them as a JSON object — CronBox makes them available three ways at once (`CRONBOX_ARGS` env var, parsed CLI flags, `ARG_KEY` env vars):

```bash
cronbox schedules add ./scripts/check.py "*/15 * * * *" --tz "Asia/Shanghai" \
  --args '{"threshold": 0.05, "pages": 3}'
```

## Step 3 — Verify and report back

Show the user the schedule and the next fire time:

```bash
cronbox schedules list
```

In your final reply: state (a) the script path you created, (b) the cron expression, (c) the timezone, (d) the next scheduled run time, and (e) remind them CronBox must stay open in the menu bar.

## CLI quick reference

| Operation | Command |
|---|---|
| Self-check (CLI + DB) | `cronbox dirs list` |
| Register a script file (manual mode) | `cronbox add <abs-script> --alias "<desc>"` |
| Register a directory and pick scripts | `cronbox add <dir> --include <rel-script>` (repeatable) or `--all` |
| List visible scripts | `cronbox scripts list` |
| Create schedule | `cronbox schedules add <abs-script> "<cron>" --tz <tz> [--args <json>]` |
| List schedules | `cronbox schedules list` |
| Enable / disable | `cronbox schedules enable <id-or-path>` / `disable` |
| Delete schedule | `cronbox schedules delete <id-or-path>` |
| Run once now (for testing) | `cronbox run <base-dir> <script-rel-path> [--args <json>]` |
| Recent job history | `cronbox jobs list` |

## Conventions

- **Always use absolute paths** for `cronbox add` and `cronbox schedules add`. AI cwd assumptions are flaky; absolute paths are unambiguous.
- **Always pass `--alias '<one-line description>'`** when registering a script. The user scans their CronBox list to know what each task is — `funding_check.sh` tells them nothing, `Daily funding rate snapshot` does.
- **Always pass `--tz`** explicitly. Don't rely on the user's default.
- **Project-local scripts**, not `~/.cronbox`. The user wants them in their repo.
- **Test once before scheduling**: `cronbox run <dir> <script-rel>` to confirm the script works in CronBox's environment before committing to a cron. CronBox resolves the user's login-shell PATH at startup so `bun`/`uv`/`jq` etc. should be available.

## Notifications

CronBox **automatically sends a macOS notification when a scheduled job completes** — success or failure, with the script's alias as the title and the duration or error as the body. The user does not need to wire this themselves.

The auto-notification only fires for **scheduled** runs (cron-triggered). Ad-hoc `cronbox run` invocations stay silent so testing doesn't spam.

If the user wants extra notifications beyond the built-in macOS one — push to Slack/Discord/Telegram, email, etc. — wire them inside the script and tell the user what you wired:

- Webhook: `curl -X POST <url> -d '{"text":"..."}'`
- Email: `mail` / `msmtp` / whatever the user already has.

## Failure modes to recognize

| Symptom | Likely cause | What to tell the user |
|---|---|---|
| `command not found: cronbox` | Menu-bar app not installed, or CLI symlink missing | Install the CronBox app, or open it once to auto-install the CLI |
| `Script not found` from `schedules add` | Wrong path | Re-check; use absolute paths |
| Job stays `skipped` | The previous run is still active (slow script) | Show `cronbox jobs list`; cron is too tight or the script hangs |
| Schedule never fires | Menu-bar app isn't running | Tell the user to open CronBox.app |
| Job log says `command not found` for a tool that works in their terminal | CronBox launched fresh and missed the user's `.zshrc` PATH | Have them restart the menu-bar app (PATH is resolved at startup) |

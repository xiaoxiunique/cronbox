use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Timelike, Utc};
use tauri::State;

use crate::cli;
use crate::db::Database;
use crate::executor;
use crate::models::*;
use crate::scheduler;
use crate::state::{scan_directory_entries, script_from_file, AppState};

type CmdResult<T> = Result<T, String>;

#[tauri::command]
pub fn dashboard_stats(state: State<AppState>) -> CmdResult<DashboardStats> {
    let script_total = state.scan_scripts().len() as u32;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let schedules = db.list_schedules().map_err(|e| e.to_string())?;
    let since = (Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
    let recent_runs = db
        .recent_run_stats_since(&since)
        .map_err(|e| e.to_string())?;

    Ok(DashboardStats {
        script_total,
        schedule_total: schedules.len() as u32,
        enabled_schedule_total: schedules.iter().filter(|schedule| schedule.enabled).count() as u32,
        recent_runs,
        schedule_distribution: schedule_distribution(&schedules),
    })
}

fn schedule_distribution(schedules: &[Schedule]) -> Vec<ScheduleDistributionBucket> {
    let mut buckets = [0u32; 24];

    for schedule in schedules.iter().filter(|schedule| schedule.enabled) {
        let next_run = schedule.next_run_at.clone().or_else(|| {
            scheduler::calculate_next_run(&schedule.cron_expr, &schedule.timezone).ok()
        });
        let Some(next_run) = next_run else {
            continue;
        };
        let Ok(next_run) = DateTime::parse_from_rfc3339(&next_run) else {
            continue;
        };

        let hour = schedule
            .timezone
            .parse::<chrono_tz::Tz>()
            .map(|tz| next_run.with_timezone(&tz).hour())
            .unwrap_or_else(|_| next_run.with_timezone(&Utc).hour());
        buckets[hour as usize] += 1;
    }

    buckets
        .iter()
        .enumerate()
        .map(|(hour, count)| ScheduleDistributionBucket {
            hour: format!("{hour:02}:00"),
            count: *count,
        })
        .collect()
}

// ── Work Dirs ──

#[tauri::command]
pub fn list_work_dirs(state: State<AppState>) -> CmdResult<Vec<WorkDir>> {
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .list_work_dirs()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_work_dir(state: State<AppState>, path: String) -> CmdResult<WorkDir> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .add_work_dir(&path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_work_dir_with_scan(state: State<AppState>, path: String) -> CmdResult<AddedWorkDir> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }
    let canonical_path = std::fs::canonicalize(&p)
        .map_err(|e| format!("Cannot resolve directory {}: {e}", p.display()))?;
    let canonical_path = canonical_path.to_string_lossy().to_string();
    let work_dir = state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .add_work_dir(&canonical_path)
        .map_err(|e| e.to_string())?;
    let entry_scripts = state
        .scan_scripts()
        .into_iter()
        .filter(|script| script.base_dir == work_dir.path)
        .collect();
    Ok(AddedWorkDir {
        work_dir,
        entry_scripts,
    })
}

#[tauri::command]
pub fn preview_script_entries(path: String) -> CmdResult<Vec<ScriptFile>> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }
    Ok(scan_directory_entries(&p))
}

#[tauri::command]
pub fn add_selected_scripts(
    state: State<AppState>,
    base_dir: String,
    script_paths: Vec<String>,
) -> CmdResult<AddedWorkDir> {
    let base = PathBuf::from(&base_dir);
    if !base.is_dir() {
        return Err(format!("Not a directory: {base_dir}"));
    }
    if script_paths.is_empty() {
        return Err("No scripts selected.".to_string());
    }

    let canonical_base = std::fs::canonicalize(&base)
        .map_err(|e| format!("Cannot resolve directory {}: {e}", base.display()))?;
    let canonical_base = canonical_base.to_string_lossy().to_string();
    let available = scan_directory_entries(Path::new(&canonical_base));

    let mut selected = Vec::new();
    for script_path in &script_paths {
        let Some(script) = available.iter().find(|script| script.path == *script_path) else {
            return Err(format!(
                "Script is not an executable entrypoint: {script_path}"
            ));
        };
        selected.push(script.clone());
    }

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let work_dir = db
        .add_work_dir_manual(&canonical_base)
        .map_err(|e| e.to_string())?;
    for script in &selected {
        db.add_script_entry(&work_dir.path, &script.path)
            .map_err(|e| e.to_string())?;
    }

    Ok(AddedWorkDir {
        work_dir,
        entry_scripts: selected,
    })
}

#[tauri::command]
pub fn add_script_file(state: State<AppState>, path: String) -> CmdResult<AddedWorkDir> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("Not a file: {path}"));
    }
    let script = script_from_file(&p)
        .ok_or_else(|| format!("File is not an executable entry script: {}", p.display()))?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let work_dir = db
        .add_work_dir_manual(&script.base_dir)
        .map_err(|e| e.to_string())?;
    db.add_script_entry(&work_dir.path, &script.path)
        .map_err(|e| e.to_string())?;

    Ok(AddedWorkDir {
        work_dir,
        entry_scripts: vec![script],
    })
}

#[tauri::command]
pub fn remove_work_dir(state: State<AppState>, id: String) -> CmdResult<bool> {
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .remove_work_dir(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn scan_scripts(state: State<AppState>) -> Vec<ScriptFile> {
    state.scan_scripts()
}

#[tauri::command]
pub fn set_script_alias(
    state: State<AppState>,
    base_dir: String,
    script_path: String,
    alias: Option<String>,
) -> CmdResult<()> {
    let full = PathBuf::from(&base_dir).join(&script_path);
    if !full.exists() {
        return Err(format!("Script not found: {script_path}"));
    }
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .set_script_alias(&base_dir, &script_path, alias.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ensure_agent_workspace(state: State<AppState>) -> CmdResult<WorkDir> {
    ensure_default_agent_workspace(&state)
}

#[tauri::command]
pub fn create_codex_task(
    state: State<AppState>,
    name: String,
    prompt: String,
) -> CmdResult<CreatedCodexTask> {
    create_agent_task(&state, AgentTaskKind::Codex, name, prompt)
}

#[tauri::command]
pub fn create_claude_task(
    state: State<AppState>,
    name: String,
    prompt: String,
) -> CmdResult<CreatedCodexTask> {
    create_agent_task(&state, AgentTaskKind::Claude, name, prompt)
}

enum AgentTaskKind {
    Codex,
    Claude,
}

impl AgentTaskKind {
    fn label(&self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
        }
    }

    fn folder(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    fn script(&self, base_dir: &str, name: &str, prompt: &str) -> String {
        match self {
            Self::Codex => codex_task_script(base_dir, name, prompt),
            Self::Claude => claude_task_script(base_dir, name, prompt),
        }
    }
}

fn create_agent_task(
    state: &State<AppState>,
    kind: AgentTaskKind,
    name: String,
    prompt: String,
) -> CmdResult<CreatedCodexTask> {
    let work_dir = ensure_default_agent_workspace(&state)?;
    let base_dir = work_dir.path.clone();
    let base = PathBuf::from(&base_dir);

    let slug = slugify_task_name(&name);
    if slug.is_empty() {
        return Err("Task name must contain letters or numbers.".to_string());
    }

    let task_dir = base.join("cronbox").join(kind.folder());
    std::fs::create_dir_all(&task_dir)
        .map_err(|e| format!("Cannot create {}: {e}", task_dir.display()))?;

    let script_path = task_dir.join(format!("{slug}.sh"));
    if script_path.exists() {
        return Err(format!(
            "{} task already exists: cronbox/{}/{slug}.sh",
            kind.label(),
            kind.folder()
        ));
    }

    let content = kind.script(&base_dir, &name, &prompt);
    std::fs::write(&script_path, content)
        .map_err(|e| format!("Cannot write {}: {e}", script_path.display()))?;
    make_executable(&script_path)?;

    let relative_path = format!("cronbox/{}/{slug}.sh", kind.folder());
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .set_script_alias(&base_dir, &relative_path, Some(name.trim()))
        .map_err(|e| e.to_string())?;
    Ok(CreatedCodexTask {
        script: ScriptFile {
            path: relative_path,
            name: format!("{slug}.sh"),
            alias: name.trim().to_string(),
            language: ScriptLanguage::Bash,
            base_dir,
        },
        full_path: script_path.to_string_lossy().to_string(),
    })
}

fn ensure_default_agent_workspace(state: &State<AppState>) -> CmdResult<WorkDir> {
    let path = cli::default_agent_workspace_path();
    std::fs::create_dir_all(&path).map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
    write_if_missing(&path.join("AGENTS.md"), default_agents_md())?;
    write_if_missing(&path.join("CLAUDE.md"), default_claude_md())?;
    migrate_codex_task_sandbox(&path)?;

    let canonical_path = std::fs::canonicalize(&path)
        .map_err(|e| format!("Cannot resolve {}: {e}", path.display()))?;
    let canonical_path = canonical_path.to_string_lossy().to_string();
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .add_work_dir(&canonical_path)
        .map_err(|e| e.to_string())
}

fn write_if_missing(path: &Path, content: &str) -> CmdResult<()> {
    if path.exists() {
        return Ok(());
    }
    std::fs::write(path, content).map_err(|e| format!("Cannot write {}: {e}", path.display()))
}

fn migrate_codex_task_sandbox(base: &Path) -> CmdResult<()> {
    let task_dir = base.join("cronbox").join("codex");
    let entries = match std::fs::read_dir(&task_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("sh") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if !content.contains("codex exec -C ") || content.contains("--sandbox ") {
            continue;
        }
        let migrated = content.replace(
            " --skip-git-repo-check -",
            " --sandbox workspace-write --skip-git-repo-check -",
        );
        if migrated != content {
            std::fs::write(&path, migrated)
                .map_err(|e| format!("Cannot update {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

fn default_agents_md() -> &'static str {
    r#"# CronBox Agent Workspace

This directory is the default workspace for scheduled coding-agent tasks created by CronBox.

## Operating Rules

- Keep generated scripts under `cronbox/codex/` or `cronbox/claude/`.
- Prefer small, reviewable changes and clear logs.
- Do not store secrets in prompts or generated scripts.
- When a task changes files, summarize what changed and how it was verified.
- If the requested task needs access outside this workspace, say so in the run log instead of guessing.

## CronBox Notes

CronBox runs agent tasks non-interactively from this directory. Use this file to add durable instructions that every scheduled agent task should follow.
"#
}

fn default_claude_md() -> &'static str {
    r#"# CronBox Agent Workspace

This file is loaded by Claude Code tasks created by CronBox.

Add project-specific conventions, allowed directories, verification commands, and safety rules here.
"#
}

fn slugify_task_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn codex_task_script(base_dir: &str, name: &str, prompt: &str) -> String {
    let heredoc = "CRONBOX_CODEX_PROMPT";
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

echo "[cronbox] Codex task: {quoted_name}"
echo "[cronbox] Working directory: {quoted_base_dir}"
echo "[cronbox] Started at: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo

if ! command -v codex >/dev/null 2>&1; then
  echo "codex CLI not found in PATH" >&2
  exit 127
fi

cat <<'{heredoc}' | codex exec -C {quoted_base_dir} --sandbox workspace-write --skip-git-repo-check -
{prompt}
{heredoc}
exit_code=$?

echo
echo "[cronbox] Finished at: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
exit "$exit_code"
"#,
        quoted_name = shell_single_quote(name),
        quoted_base_dir = shell_single_quote(base_dir),
        heredoc = heredoc,
        prompt = prompt.replace(heredoc, "CRONBOX_CODEX_PROMPT_BLOCK"),
    )
}

fn claude_task_script(base_dir: &str, name: &str, prompt: &str) -> String {
    let heredoc = "CRONBOX_CLAUDE_PROMPT";
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

echo "[cronbox] Claude task: {quoted_name}"
echo "[cronbox] Working directory: {quoted_base_dir}"
echo "[cronbox] Started at: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo

if ! command -v claude >/dev/null 2>&1; then
  echo "Claude Code CLI not found in PATH" >&2
  exit 127
fi

cd {quoted_base_dir}
cat <<'{heredoc}' | claude -p --permission-mode acceptEdits --output-format text
{prompt}
{heredoc}
exit_code=$?

echo
echo "[cronbox] Finished at: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
exit "$exit_code"
"#,
        quoted_name = shell_single_quote(name),
        quoted_base_dir = shell_single_quote(base_dir),
        heredoc = heredoc,
        prompt = prompt.replace(heredoc, "CRONBOX_CLAUDE_PROMPT_BLOCK"),
    )
}

fn make_executable(path: &Path) -> CmdResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path)
            .map_err(|e| format!("Cannot read {} permissions: {e}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions)
            .map_err(|e| format!("Cannot set {} executable: {e}", path.display()))?;
    }
    Ok(())
}

// ── Schedules ──

#[tauri::command]
pub fn create_schedule(
    state: State<AppState>,
    script_path: String,
    base_dir: String,
    cron_expr: String,
    timezone: String,
    args: String,
    env: String,
) -> CmdResult<Schedule> {
    scheduler::validate_cron(&cron_expr)?;
    let full = PathBuf::from(&base_dir).join(&script_path);
    if !full.exists() {
        return Err(format!("Script not found: {script_path}"));
    }

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let schedule = db
        .create_schedule(&script_path, &base_dir, &cron_expr, &timezone, &args, &env)
        .map_err(|e| e.to_string())?;
    if let Ok(next) = scheduler::calculate_next_run(&cron_expr, &timezone) {
        let _ = db.update_schedule_next_run(&schedule.id, &next);
    }
    db.get_schedule(&schedule.id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_schedules(state: State<AppState>) -> CmdResult<Vec<Schedule>> {
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .list_schedules()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_schedule(
    state: State<AppState>,
    id: String,
    cron_expr: Option<String>,
    timezone: Option<String>,
    args: Option<String>,
    env: Option<String>,
) -> CmdResult<Schedule> {
    if let Some(ref c) = cron_expr {
        scheduler::validate_cron(c)?;
    }
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let s = db
        .update_schedule(
            &id,
            cron_expr.as_deref(),
            timezone.as_deref(),
            args.as_deref(),
            env.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    if let Ok(next) = scheduler::calculate_next_run(&s.cron_expr, &s.timezone) {
        let _ = db.update_schedule_next_run(&s.id, &next);
    }
    db.get_schedule(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_schedule_enabled(state: State<AppState>, id: String, enabled: bool) -> CmdResult<()> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_schedule_enabled(&id, enabled)
        .map_err(|e| e.to_string())?;
    if enabled {
        let s = db.get_schedule(&id).map_err(|e| e.to_string())?;
        if let Ok(next) = scheduler::calculate_next_run(&s.cron_expr, &s.timezone) {
            let _ = db.update_schedule_next_run(&id, &next);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn delete_schedule(state: State<AppState>, id: String) -> CmdResult<bool> {
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .delete_schedule(&id)
        .map_err(|e| e.to_string())
}

// ── Jobs ──

#[tauri::command]
pub async fn run_now(
    state: State<'_, AppState>,
    script_path: String,
    base_dir: String,
    args: String,
) -> CmdResult<Job> {
    let full = PathBuf::from(&base_dir).join(&script_path);
    if !full.exists() {
        return Err(format!("Script not found: {script_path}"));
    }
    let language = ScriptLanguage::from_extension(&script_path)
        .ok_or_else(|| format!("Unknown script type: {script_path}"))?;

    let job = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.create_job(None, &script_path, &base_dir, &args, None)
            .map_err(|e| e.to_string())?
    };

    let job_id = job.id.clone();
    let db_path = state.db_path.clone();
    let bd = base_dir.clone();
    let sp = script_path.clone();
    let a = args.clone();

    tokio::spawn(async move {
        let db = match Database::open(&db_path) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("DB error: {e}");
                return;
            }
        };
        let _ = db.mark_job_running(&job_id);
        let full = PathBuf::from(&bd).join(&sp);
        let stream_db_path = db_path.clone();
        let stream_job_id = job_id.clone();
        let log_callback: executor::LogCallback = Arc::new(move |chunk| {
            if let Ok(db) = Database::open(&stream_db_path) {
                let _ = db.append_job_logs(&stream_job_id, chunk);
            }
        });
        let result =
            executor::execute_with_log_callback(&full, language, &a, "{}", Some(log_callback))
                .await;

        let success = result.exit_code == 0;
        let logs = db
            .get_job(&job_id)
            .map(|job| job.logs)
            .unwrap_or_else(|_| combined_logs(&result));
        let error = if success {
            None
        } else {
            Some(result.stderr.lines().last().unwrap_or("Error").to_string())
        };
        let _ = db.mark_job_completed(
            &job_id,
            success,
            result.result.as_deref(),
            &logs,
            error.as_deref(),
            result.duration_ms,
        );
    });

    Ok(job)
}

fn combined_logs(result: &ExecutionResult) -> String {
    if result.stderr.is_empty() {
        result.stdout.clone()
    } else {
        format!("{}\n--- stderr ---\n{}", result.stdout, result.stderr)
    }
}

#[tauri::command]
pub fn list_jobs(state: State<AppState>, limit: u32) -> CmdResult<Vec<Job>> {
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .list_jobs(limit)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_jobs_for_script(
    state: State<AppState>,
    script_path: String,
    base_dir: String,
    limit: u32,
) -> CmdResult<Vec<Job>> {
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .list_jobs_for_script(&script_path, &base_dir, limit)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_running_jobs(state: State<AppState>) -> CmdResult<Vec<Job>> {
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .list_running_jobs()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_job(state: State<AppState>, id: String) -> CmdResult<Job> {
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .get_job(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_job(state: State<AppState>, id: String) -> CmdResult<bool> {
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .cancel_job(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cleanup_old_jobs(state: State<AppState>, days: u32) -> CmdResult<u64> {
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .cleanup_old_jobs(days)
        .map(|n| n as u64)
        .map_err(|e| e.to_string())
}

// ── Utilities ──

#[tauri::command]
pub fn cli_status() -> String {
    cli::cli_status_text(None)
}

#[tauri::command]
pub fn install_cli(force: bool) -> CmdResult<String> {
    cli::install_cli_link(None, force).map(|path| path.display().to_string())
}

/// The PATH that scheduled scripts run with — resolved from the login shell at
/// startup. Surfaced read-only in Settings so users can see what is in effect.
#[tauri::command]
pub fn resolved_path() -> String {
    executor::env::effective_path()
}

#[tauri::command]
pub fn validate_cron(cron_expr: String) -> CmdResult<()> {
    scheduler::validate_cron(&cron_expr)
}

#[tauri::command]
pub fn upcoming_runs(cron_expr: String, timezone: String, count: u32) -> CmdResult<Vec<String>> {
    scheduler::upcoming_runs(&cron_expr, &timezone, count as usize)
}

/// Detect script parameters by running `script --help` and parsing the output
#[tauri::command]
pub async fn detect_args(script_path: String, base_dir: String) -> CmdResult<Vec<ScriptParam>> {
    let full = PathBuf::from(&base_dir).join(&script_path);
    if !full.exists() {
        return Err(format!("Script not found: {script_path}"));
    }

    let language = ScriptLanguage::from_extension(&script_path)
        .ok_or_else(|| format!("Unknown script type: {script_path}"))?;

    let help_output = get_help_output(&full, language, &base_dir).await?;
    Ok(parse_help_output(&help_output, language))
}

async fn get_help_output(
    file_path: &std::path::Path,
    language: ScriptLanguage,
    _base_dir: &str,
) -> Result<String, String> {
    use std::process::Stdio;

    let script_dir = file_path.parent().unwrap_or(std::path::Path::new("."));

    let output = match language {
        ScriptLanguage::Python => {
            let python_bin = crate::executor::python::resolve_python_bin(script_dir);
            let python_str = python_bin.to_str().unwrap_or("python3");
            // If resolver returned uv, use "uv run python script --help"
            if python_str.ends_with("/uv") || python_str == "uv" {
                tokio::process::Command::new(&python_bin)
                    .args(["run", "python", "-u"])
                    .arg(file_path)
                    .arg("--help")
                    .current_dir(script_dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
            } else {
                tokio::process::Command::new(&python_bin)
                    .arg(file_path)
                    .arg("--help")
                    .current_dir(script_dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
            }
        }
        ScriptLanguage::Bash => {
            tokio::process::Command::new("bash")
                .arg(file_path)
                .arg("--help")
                .current_dir(script_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
        }
        ScriptLanguage::Bun => {
            tokio::process::Command::new("bun")
                .arg("run")
                .arg(file_path)
                .arg("--help")
                .current_dir(script_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
        }
        ScriptLanguage::PostgreSql => {
            return Ok(String::new()); // SQL files don't have --help
        }
    };

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            // Some tools print help to stderr
            Ok(if stdout.len() > stderr.len() {
                stdout
            } else {
                stderr
            })
        }
        Err(e) => Err(format!("Failed to run --help: {e}")),
    }
}

fn parse_help_output(text: &str, _language: ScriptLanguage) -> Vec<ScriptParam> {
    let mut params = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut current_section = ""; // Track which section we're in

    for line in lines.iter() {
        let trimmed = line.trim();

        // Detect section headers like "positional arguments:", "options:", "optional arguments:"
        if !trimmed.is_empty() && trimmed.ends_with(':') && !line.starts_with(' ') {
            let header = trimmed.trim_end_matches(':').to_lowercase();
            if header.contains("positional") {
                current_section = "positional";
            } else if header.contains("option") || header == "options" {
                current_section = "options";
            } else {
                current_section = "other";
            }
            continue;
        }

        // Skip empty lines (but don't reset section)
        if trimmed.is_empty() {
            continue;
        }

        // A non-indented non-empty line that's not a section header resets tracking
        if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.starts_with('-') {
            // Could be usage line or something else
            current_section = "";
            continue;
        }

        // Parse option lines (start with -)
        if trimmed.starts_with('-') {
            if let Some(param) = parse_option_line(trimmed) {
                params.push(param);
            }
        } else if current_section == "positional" {
            // Positional argument: indented line under "positional arguments:"
            let parts: Vec<&str> = trimmed.splitn(2, "  ").collect();
            let name = parts[0].trim().to_string();
            let desc = parts
                .get(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !name.is_empty()
                && name != "{}"
                && name
                    .chars()
                    .next()
                    .map_or(false, |c| c.is_alphanumeric() || c == '_')
            {
                params.push(ScriptParam {
                    name,
                    param_type: "str".to_string(),
                    default: None,
                    required: true,
                    description: desc,
                    choices: Vec::new(),
                });
            }
        }
    }

    params
}

fn parse_option_line(line: &str) -> Option<ScriptParam> {
    // Match patterns like:
    //   -h, --help            show this help
    //   --catalog CATALOG     catalog id
    //   --pages PAGES         number of pages (default: 1)
    //   --type {user,keyword,all}  search type
    //   --detail              fetch detail (flag)
    //   -o OUTPUT, --output OUTPUT   output file

    let trimmed = line.trim();

    // Split into flags part and description part (separated by 2+ spaces)
    let (flags_part, desc_part) = match trimmed.find("  ") {
        Some(pos) => (&trimmed[..pos], trimmed[pos..].trim()),
        None => (trimmed, ""),
    };

    // Skip -h/--help
    if flags_part.contains("--help") || flags_part.contains("-h") && !flags_part.contains("--h") {
        return None;
    }

    // Extract the main flag name
    let flag_segments: Vec<&str> = flags_part.split(',').map(|s| s.trim()).collect();
    let main_flag = flag_segments
        .iter()
        .find(|s| s.starts_with("--"))
        .or(flag_segments.first())
        .map(|s| s.split_whitespace().next().unwrap_or(s))?;

    let name = main_flag.to_string();

    // Determine if it takes a value (has METAVAR after flag)
    let has_metavar = flag_segments.iter().any(|seg| {
        let parts: Vec<&str> = seg.split_whitespace().collect();
        parts.len() > 1
    });

    // Check for choices like {a,b,c}
    let choices: Vec<String> = if let Some(start) = flags_part.find('{') {
        if let Some(end) = flags_part.find('}') {
            flags_part[start + 1..end]
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            Vec::new()
        }
    } else if let Some(start) = desc_part.find('{') {
        if let Some(end) = desc_part.find('}') {
            desc_part[start + 1..end]
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Extract default value from description
    let default = extract_default(desc_part);

    // Determine type
    let param_type = if !choices.is_empty() {
        "choice".to_string()
    } else if !has_metavar && !flags_part.contains('{') {
        "bool".to_string()
    } else if desc_part.to_lowercase().contains("int")
        || desc_part.to_lowercase().contains("number")
    {
        "int".to_string()
    } else {
        "str".to_string()
    };

    let required = desc_part.to_lowercase().contains("required");

    Some(ScriptParam {
        name,
        param_type,
        default,
        required,
        description: desc_part.to_string(),
        choices,
    })
}

fn extract_default(desc: &str) -> Option<String> {
    // Look for (default: VALUE) or (default=VALUE)
    let lower = desc.to_lowercase();
    if let Some(pos) = lower.find("default:") {
        let rest = &desc[pos + 8..];
        let val = rest.trim_start().split(')').next().unwrap_or("").trim();
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }
    if let Some(pos) = lower.find("default=") {
        let rest = &desc[pos + 8..];
        let val = rest.split(')').next().unwrap_or("").trim();
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }
    None
}

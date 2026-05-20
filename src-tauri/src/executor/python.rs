use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::executor::{env, LogCallback};
use crate::models::ExecutionResult;

pub async fn execute(
    file_path: &Path,
    args: &str,
    schedule_env: &str,
    log_callback: Option<LogCallback>,
) -> ExecutionResult {
    let script_dir = file_path.parent().unwrap_or(Path::new("."));
    let base_path = env::effective_path();

    // Determine how to run: uv > .venv > system python3
    let (program, base_args) = resolve_python_runner(script_dir, file_path);
    let program = env::which_in(&program.to_string_lossy(), &base_path).unwrap_or(program);

    let mut cmd = Command::new(&program);
    cmd.args(&base_args)
        .env("CRONBOX_ARGS", args)
        .current_dir(script_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Base PATH is the resolved login-shell PATH; prepend the venv bin when
    // running a venv python directly.
    let mut run_path = base_path.clone();
    if program.to_str().map_or(false, |s| s.contains(".venv")) {
        if let Some(venv_dir) = program.parent().and_then(|bin| bin.parent()) {
            cmd.env("VIRTUAL_ENV", venv_dir);
            run_path = format!("{}:{}", venv_dir.join("bin").display(), base_path);
        }
    }
    cmd.env("PATH", &run_path);

    // Convert JSON args to CLI arguments AND env vars
    if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(args) {
        let cli_args = json_to_cli_args(&map);
        cmd.args(&cli_args);

        // Also set env vars for scripts that prefer env
        for (key, value) in &map {
            let val_str = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            cmd.env(&format!("ARG_{}", key.to_uppercase()), &val_str);
        }
    }

    // Per-schedule env applied last so it can override anything above.
    env::apply_schedule_env(&mut cmd, schedule_env);

    run_command(cmd, log_callback).await
}

/// Convert a JSON map to CLI arguments.
/// Rules:
///   - Keys starting with "--" or "-" are passed as-is (e.g. "--catalog" → "--catalog", "42")
///   - Keys without prefix: if looks like a flag name → prepend "--" (e.g. "pages" → "--pages", "1")
///   - Special key "_positional" or keys that are single words without dashes at the start
///     where value is a string → treated as positional (appended at end)
///   - Bool true → flag only (e.g. "--detail"), false → skip
///   - Null → skip
pub fn json_to_cli_args(map: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut flag_args = Vec::new();
    let mut positional_args = Vec::new();

    for (key, value) in map {
        // Skip null
        if value.is_null() {
            continue;
        }

        // Determine the flag name
        let flag = if key.starts_with('-') {
            key.clone()
        } else {
            format!("--{}", key)
        };

        match value {
            serde_json::Value::Bool(b) => {
                if *b {
                    flag_args.push(flag);
                }
                // false → omit the flag entirely
            }
            serde_json::Value::String(s) if s.is_empty() => {
                // Skip empty strings
            }
            serde_json::Value::String(s) => {
                // Check if this is a positional arg (key doesn't start with -)
                // and it's a simple word like "query"
                if !key.starts_with('-')
                    && !key.contains('_')
                    && key.len() < 20
                    && is_likely_positional(key)
                {
                    positional_args.push(s.clone());
                } else {
                    flag_args.push(flag);
                    flag_args.push(s.clone());
                }
            }
            serde_json::Value::Number(n) => {
                flag_args.push(flag);
                flag_args.push(n.to_string());
            }
            serde_json::Value::Array(arr) => {
                // Repeat flag for each value
                for item in arr {
                    flag_args.push(flag.clone());
                    flag_args.push(match item {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    });
                }
            }
            _ => {
                flag_args.push(flag);
                flag_args.push(value.to_string());
            }
        }
    }

    // Positional args go at the end
    flag_args.extend(positional_args);
    flag_args
}

/// Heuristic: a key is "likely positional" if it doesn't look like a flag name.
/// Positional arg names from argparse are usually simple nouns (query, file, path)
/// while option names often have underscores (page_size) or multiple words.
fn is_likely_positional(key: &str) -> bool {
    // Keys from our param detection have their original names
    // Positional args: "query", "file", "path", "command"
    // Flag args: "--catalog" → stored as "--catalog" or "catalog"
    // If the key was detected as positional, it won't have "--" prefix
    // This is a best-effort heuristic
    !key.contains('-') && !key.contains('_') && key.chars().all(|c| c.is_lowercase())
}

/// Decide the best way to run a Python script:
/// 1. If `uv` is available and (pyproject.toml OR uv.lock exists in ancestors) → `uv run python script.py`
/// 2. If .venv/bin/python3 found in ancestors → use that directly
/// 3. Fallback to system `python3`
fn resolve_python_runner(script_dir: &Path, file_path: &Path) -> (std::path::PathBuf, Vec<String>) {
    // Check for uv project (pyproject.toml or uv.lock in ancestors)
    if has_uv() && has_uv_project(script_dir) {
        return (
            which("uv").unwrap(),
            vec![
                "run".into(),
                "python".into(),
                "-u".into(),
                file_path.display().to_string(),
            ],
        );
    }

    // Check for .venv in script dir and ancestors
    if let Some(venv_python) = find_venv_python(script_dir) {
        return (
            venv_python,
            vec!["-u".into(), file_path.display().to_string()],
        );
    }

    // Fallback to system python3
    (
        "python3".into(),
        vec!["-u".into(), file_path.display().to_string()],
    )
}

fn has_uv() -> bool {
    which("uv").is_some()
}

fn has_uv_project(start_dir: &Path) -> bool {
    let mut dir = start_dir.to_path_buf();
    loop {
        if dir.join("pyproject.toml").exists() || dir.join("uv.lock").exists() {
            return true;
        }
        if !dir.pop() {
            break;
        }
    }
    false
}

fn which(name: &str) -> Option<std::path::PathBuf> {
    env::which_in(name, &env::effective_path())
}

/// Public helper: resolve the best python binary for a given script directory.
/// Used by detect_args command to run --help with the right interpreter.
pub fn resolve_python_bin(script_dir: &Path) -> std::path::PathBuf {
    if has_uv() && has_uv_project(script_dir) {
        which("uv").unwrap_or_else(|| "python3".into())
    } else {
        find_venv_python(script_dir).unwrap_or_else(|| "python3".into())
    }
}

/// Walk up from `start_dir` looking for .venv/bin/python3 (or .venv/bin/python)
fn find_venv_python(start_dir: &Path) -> Option<std::path::PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        for name in &["python3", "python"] {
            let p = dir.join(".venv/bin").join(name);
            if p.exists() {
                return Some(p);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

async fn run_command(mut cmd: Command, log_callback: Option<LogCallback>) -> ExecutionResult {
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return ExecutionResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Failed to spawn process: {e}"),
                result: None,
                duration_ms: 0,
            };
        }
    };
    collect_output(child, log_callback).await
}

pub(crate) async fn collect_output(
    mut child: tokio::process::Child,
    log_callback: Option<LogCallback>,
) -> ExecutionResult {
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();
    let stdout_log = log_callback.clone();
    let stderr_log = log_callback;

    let stdout_task = tokio::spawn(async move {
        let mut lines = Vec::new();
        if let Some(out) = stdout_handle {
            let mut reader = BufReader::new(out).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(callback) = &stdout_log {
                    callback(&format!("{line}\n"));
                }
                lines.push(line);
            }
        }
        lines
    });

    let stderr_task = tokio::spawn(async move {
        let mut lines = Vec::new();
        if let Some(err) = stderr_handle {
            let mut reader = BufReader::new(err).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(callback) = &stderr_log {
                    callback(&format!("{line}\n"));
                }
                lines.push(line);
            }
        }
        lines
    });

    let status = child.wait().await;
    let stdout_lines = stdout_task.await.unwrap_or_default();
    let stderr_lines = stderr_task.await.unwrap_or_default();

    let exit_code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
    let stdout = stdout_lines.join("\n");
    let stderr = stderr_lines.join("\n");

    let result = stdout_lines
        .last()
        .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .map(|v| v.to_string());

    ExecutionResult {
        exit_code,
        stdout,
        stderr,
        result,
        duration_ms: 0,
    }
}

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use tokio::process::Command;

static EFFECTIVE_PATH: OnceLock<String> = OnceLock::new();

/// Resolve the login shell's PATH once and cache it for all script executions.
///
/// Call this once at GUI startup. A GUI app launched from Finder is started by
/// `launchd`, which never sources the user's shell rc files — so its PATH lacks
/// Homebrew, `~/.bun/bin`, etc. and scripts fail to find `bun`/`uv`/`jq`.
pub fn init_effective_path_from_login_shell() {
    let resolved = resolve_login_shell_path().unwrap_or_else(fallback_path);
    let _ = EFFECTIVE_PATH.set(resolved);
}

/// The PATH that scripts should run with. Falls back to the inherited process
/// PATH when not initialized (CLI mode, tests) — the CLI already inherits the
/// terminal's full PATH.
pub fn effective_path() -> String {
    EFFECTIVE_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| std::env::var("PATH").unwrap_or_default())
}

/// Apply per-schedule environment variables (a JSON object of string→string) to
/// a command. Applied last by callers so it can override PATH and arg-derived
/// variables.
pub fn apply_schedule_env(cmd: &mut Command, env_json: &str) {
    let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(env_json)
    else {
        return;
    };
    for (key, value) in &map {
        if key.is_empty() {
            continue;
        }
        let val = match value {
            serde_json::Value::Null => continue,
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        cmd.env(key, val);
    }
}

/// Resolve a program name to an absolute path by scanning `path` (colon-separated).
/// A program that already contains a slash is returned unchanged.
pub fn which_in(program: &str, path: &str) -> Option<PathBuf> {
    if program.contains('/') {
        return Some(PathBuf::from(program));
    }
    path.split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(program))
        .find(|candidate| is_executable_file(candidate))
}

fn resolve_login_shell_path() -> Option<String> {
    const BEGIN: &str = "__CRONBOX_PATH_BEGIN__";
    const END: &str = "__CRONBOX_PATH_END__";

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let script = format!("printf '%s%s%s' '{BEGIN}' \"$PATH\" '{END}'");

    // Run on a worker thread with a timeout so a slow/broken rc file cannot
    // hang startup.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let output = std::process::Command::new(&shell)
            .args(["-lic", &script])
            .output();
        let _ = tx.send(output);
    });

    let output = rx.recv_timeout(Duration::from_secs(5)).ok()?.ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let start = stdout.find(BEGIN)? + BEGIN.len();
    let end = stdout[start..].find(END)? + start;
    let path = stdout[start..end].trim().to_string();
    (!path.is_empty()).then_some(path)
}

/// Build a reasonable PATH when login-shell resolution fails: common tool
/// directories merged with the inherited PATH, de-duplicated.
fn fallback_path() -> String {
    let mut dirs: Vec<String> = Vec::new();
    let mut push = |dir: String| {
        if !dir.is_empty() && !dirs.contains(&dir) {
            dirs.push(dir);
        }
    };

    if let Some(home) = dirs_next::home_dir() {
        for rel in [".cargo/bin", ".bun/bin", ".local/bin", ".deno/bin"] {
            push(home.join(rel).to_string_lossy().into_owned());
        }
    }
    for common in [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ] {
        push(common.to_string());
    }
    if let Ok(existing) = std::env::var("PATH") {
        for dir in existing.split(':') {
            push(dir.to_string());
        }
    }
    dirs.join(":")
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_path_includes_common_dirs() {
        let path = fallback_path();
        assert!(!path.is_empty());
        assert!(path.split(':').any(|dir| dir == "/usr/bin"));
    }

    #[test]
    fn which_in_returns_absolute_program_unchanged() {
        assert_eq!(
            which_in("/bin/sh", "/usr/bin"),
            Some(PathBuf::from("/bin/sh"))
        );
    }

    #[test]
    fn which_in_finds_executable_on_path() {
        let dir = std::env::temp_dir().join(format!("cronbox_which_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("cronbox_fake_tool");
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let found = which_in("cronbox_fake_tool", &dir.to_string_lossy());
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(found, Some(bin));
    }

    #[test]
    fn apply_schedule_env_ignores_invalid_json() {
        // Should not panic on malformed input.
        let mut cmd = Command::new("true");
        apply_schedule_env(&mut cmd, "not json");
        apply_schedule_env(&mut cmd, "{}");
    }
}

use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::executor::LogCallback;
use crate::models::ExecutionResult;

pub async fn execute(
    file_path: &Path,
    args: &str,
    log_callback: Option<LogCallback>,
) -> ExecutionResult {
    let script_dir = file_path.parent().unwrap_or(Path::new("."));

    // Prepend node_modules/.bin to PATH so locally installed bins are available
    let mut path_env = std::env::var("PATH").unwrap_or_default();
    let mut dir = script_dir.to_path_buf();
    loop {
        let local_bin = dir.join("node_modules/.bin");
        if local_bin.is_dir() {
            path_env = format!("{}:{}", local_bin.display(), path_env);
            break;
        }
        if !dir.pop() {
            break;
        }
    }

    let mut cmd = Command::new("bun");
    cmd.arg("run")
        .arg(file_path)
        .env("CRONBOX_ARGS", args)
        .env("PATH", &path_env)
        .current_dir(script_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(args) {
        for (key, value) in &map {
            let val_str = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            cmd.env(&format!("ARG_{}", key.to_uppercase()), &val_str);
        }
    }

    match cmd.spawn() {
        Ok(child) => super::python::collect_output(child, log_callback).await,
        Err(e) => ExecutionResult {
            exit_code: -1,
            stdout: String::new(),
            stderr: format!("Failed to spawn bun: {e}"),
            result: None,
            duration_ms: 0,
        },
    }
}

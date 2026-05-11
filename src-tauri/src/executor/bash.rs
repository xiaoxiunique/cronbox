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
    let mut cmd = Command::new("bash");
    cmd.arg(file_path)
        .env("CRONBOX_ARGS", args)
        .current_dir(file_path.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(args) {
        // Pass as CLI args
        let cli_args = super::python::json_to_cli_args(&map);
        cmd.args(&cli_args);
        // Also export as env vars for bash scripts that prefer $VAR
        for (key, value) in &map {
            let val_str = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            cmd.env(key, &val_str);
        }
    }

    match cmd.spawn() {
        Ok(child) => super::python::collect_output(child, log_callback).await,
        Err(e) => ExecutionResult {
            exit_code: -1,
            stdout: String::new(),
            stderr: format!("Failed to spawn bash: {e}"),
            result: None,
            duration_ms: 0,
        },
    }
}

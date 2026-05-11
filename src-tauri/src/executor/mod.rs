pub mod bash;
pub mod bun;
pub mod pgsql;
pub mod python;

use std::path::Path;
use std::sync::Arc;

use crate::models::{ExecutionResult, ScriptLanguage};

pub type LogCallback = Arc<dyn Fn(&str) + Send + Sync + 'static>;

/// Execute a script file by its absolute path.
/// Language is auto-detected from extension.
/// `args` is a JSON string passed to the script.
pub async fn execute(file_path: &Path, language: ScriptLanguage, args: &str) -> ExecutionResult {
    execute_with_log_callback(file_path, language, args, None).await
}

pub async fn execute_with_log_callback(
    file_path: &Path,
    language: ScriptLanguage,
    args: &str,
    log_callback: Option<LogCallback>,
) -> ExecutionResult {
    let start = std::time::Instant::now();

    let mut result = match language {
        ScriptLanguage::Python => python::execute(file_path, args, log_callback).await,
        ScriptLanguage::Bash => bash::execute(file_path, args, log_callback).await,
        ScriptLanguage::Bun => bun::execute(file_path, args, log_callback).await,
        ScriptLanguage::PostgreSql => pgsql::execute(file_path, args).await,
    };

    result.duration_ms = start.elapsed().as_millis() as i64;
    result
}

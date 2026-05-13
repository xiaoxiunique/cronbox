use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Language enum ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptLanguage {
    Python,
    Bash,
    Bun,
    #[serde(rename = "pgsql")]
    PostgreSql,
}

impl ScriptLanguage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Bash => "bash",
            Self::Bun => "bun",
            Self::PostgreSql => "pgsql",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "python" => Some(Self::Python),
            "bash" => Some(Self::Bash),
            "bun" => Some(Self::Bun),
            "pgsql" => Some(Self::PostgreSql),
            _ => None,
        }
    }

    /// Auto-detect language from file extension
    pub fn from_extension(path: &str) -> Option<Self> {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        match ext {
            "py" => Some(Self::Python),
            "sh" | "bash" => Some(Self::Bash),
            "ts" | "js" | "mts" | "mjs" => Some(Self::Bun),
            "sql" => Some(Self::PostgreSql),
            _ => None,
        }
    }
}

impl std::fmt::Display for ScriptLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Job status ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Success,
    Failure,
    Cancelled,
    Skipped,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "success" => Some(Self::Success),
            "failure" => Some(Self::Failure),
            "cancelled" => Some(Self::Cancelled),
            "skipped" => Some(Self::Skipped),
            _ => None,
        }
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Detected script parameter ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptParam {
    /// e.g. "--catalog", "--pages", "query" (positional)
    pub name: String,
    /// e.g. "int", "str", "bool", "choice"
    pub param_type: String,
    /// Default value if any
    pub default: Option<String>,
    /// Whether it's required
    pub required: bool,
    /// Description from help text
    pub description: String,
    /// Choices if type is "choice"
    pub choices: Vec<String>,
}

// ── Working directory (stored in DB) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkDir {
    pub id: String,
    pub path: String,
    pub scan_mode: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddedWorkDir {
    pub work_dir: WorkDir,
    pub entry_scripts: Vec<ScriptFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedCodexTask {
    pub script: ScriptFile,
    pub full_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptAlias {
    pub base_dir: String,
    pub script_path: String,
    pub alias: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptEntry {
    pub base_dir: String,
    pub script_path: String,
    pub created_at: String,
}

// ── Discovered script file (not stored in DB, scanned from filesystem) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptFile {
    /// Relative path from base directory (e.g. "backup/daily.sh")
    pub path: String,
    /// File name only (e.g. "daily.sh")
    pub name: String,
    /// Display alias. Defaults to the filename stem unless the user overrides it.
    pub alias: String,
    /// Auto-detected language
    pub language: ScriptLanguage,
    /// The base working directory this script belongs to
    pub base_dir: String,
}

// ── Schedule (stored in DB) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    /// Relative script path from base_dir
    pub script_path: String,
    /// Which working directory this schedule belongs to
    pub base_dir: String,
    pub cron_expr: String,
    pub timezone: String,
    pub args: String, // JSON
    pub enabled: bool,
    pub next_run_at: Option<String>,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ── Job (stored in DB) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub schedule_id: Option<String>,
    pub script_path: String,
    pub base_dir: String,
    pub status: JobStatus,
    pub args: String,
    pub result: Option<String>,
    pub logs: String,
    pub error: Option<String>,
    pub scheduled_for: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

// ── Dashboard statistics ──

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecentRunStats {
    pub total: u32,
    pub success: u32,
    pub failure: u32,
    pub running: u32,
    pub queued: u32,
    pub cancelled: u32,
    pub skipped: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleDistributionBucket {
    pub hour: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub script_total: u32,
    pub schedule_total: u32,
    pub enabled_schedule_total: u32,
    pub recent_runs: RecentRunStats,
    pub schedule_distribution: Vec<ScheduleDistributionBucket>,
}

// ── Execution result ──

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub result: Option<String>,
    pub duration_ms: i64,
}

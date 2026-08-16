use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::db::Database;
use crate::models::*;
use crate::scheduler;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub db_path: PathBuf,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Result<Self, String> {
        Self::open(db_path, false)
    }

    pub fn new_engine(db_path: PathBuf) -> Result<Self, String> {
        Self::open(db_path, true)
    }

    fn open(db_path: PathBuf, recover_interrupted: bool) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
        }
        let database = Database::open(&db_path).map_err(|e| e.to_string())?;
        if recover_interrupted {
            let interrupted = database
                .mark_interrupted_running_jobs()
                .map_err(|e| e.to_string())?;
            if interrupted > 0 {
                tracing::info!(
                    "Marked {} interrupted job(s) from a previous run",
                    interrupted
                );
            }
        }
        scheduler::initialize_schedule_times(&database)?;

        Ok(Self {
            db: Arc::new(Mutex::new(database)),
            db_path,
        })
    }

    pub fn recover_interrupted_jobs(&self) -> Result<usize, String> {
        let interrupted = self
            .db
            .lock()
            .map_err(|e| e.to_string())?
            .mark_interrupted_running_jobs()
            .map_err(|e| e.to_string())?;
        if interrupted > 0 {
            tracing::info!(
                "Marked {} interrupted job(s) from a previous scheduler",
                interrupted
            );
        }
        Ok(interrupted)
    }

    /// Scan all registered sources for scripts.
    pub fn scan_scripts(&self) -> Vec<ScriptFile> {
        let (dirs, entries, aliases) = {
            let db = self.db.lock().unwrap();
            let dirs = db.list_work_dirs().unwrap_or_default();
            let entries = db.list_script_entries().unwrap_or_default();
            let aliases = db
                .list_script_aliases()
                .unwrap_or_default()
                .into_iter()
                .map(|alias| ((alias.base_dir, alias.script_path), alias.alias))
                .collect::<HashMap<_, _>>();
            (dirs, entries, aliases)
        };

        let explicit_entries = entries
            .iter()
            .map(|entry| (entry.base_dir.clone(), entry.script_path.clone()))
            .collect::<HashSet<_>>();

        let mut all = Vec::new();
        for wd in &dirs {
            let base = Path::new(&wd.path);
            if !base.is_dir() {
                continue;
            }
            if wd.scan_mode == "manual" {
                continue;
            }
            if explicit_entries
                .iter()
                .any(|(base_dir, _)| base_dir == &wd.path)
            {
                continue;
            }
            if base.is_dir() {
                scan_recursive(base, base, &wd.path, &mut all);
            }
        }

        for entry in &entries {
            if let Some(script) = script_from_relative_path(&entry.base_dir, &entry.script_path) {
                all.push(script);
            }
        }

        for script in &mut all {
            script.alias = aliases
                .get(&(script.base_dir.clone(), script.path.clone()))
                .cloned()
                .unwrap_or_else(|| default_script_alias(&script.name));
        }
        all.sort_by(|a, b| (&a.base_dir, &a.path).cmp(&(&b.base_dir, &b.path)));
        all.dedup_by(|a, b| a.base_dir == b.base_dir && a.path == b.path);
        all
    }
}

pub fn scan_directory_entries(base_dir: &Path) -> Vec<ScriptFile> {
    let Ok(canonical_base) = std::fs::canonicalize(base_dir) else {
        return Vec::new();
    };
    let base_dir_str = canonical_base.to_string_lossy().to_string();
    let mut scripts = Vec::new();
    scan_recursive(
        &canonical_base,
        &canonical_base,
        &base_dir_str,
        &mut scripts,
    );
    for script in &mut scripts {
        script.alias = default_script_alias(&script.name);
    }
    scripts.sort_by(|a, b| a.path.cmp(&b.path));
    scripts
}

pub fn script_from_file(path: &Path) -> Option<ScriptFile> {
    let canonical_path = std::fs::canonicalize(path).ok()?;
    let base_dir = canonical_path.parent()?;
    let script_path = canonical_path.file_name()?.to_string_lossy().to_string();
    script_from_paths(base_dir, &canonical_path, &script_path)
}

fn script_from_relative_path(base_dir: &str, script_path: &str) -> Option<ScriptFile> {
    let full = Path::new(base_dir).join(script_path);
    script_from_paths(Path::new(base_dir), &full, script_path)
}

fn scan_recursive(base: &Path, dir: &Path, base_dir_str: &str, scripts: &mut Vec<ScriptFile>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map_or(false, |n| n.starts_with('.'))
        {
            continue;
        }
        // Skip node_modules and __pycache__
        if path.is_dir() {
            let dirname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dirname == "node_modules" || dirname == "__pycache__" {
                continue;
            }
            scan_recursive(base, &path, base_dir_str, scripts);
        } else if let Some(language) = ScriptLanguage::from_extension(path.to_str().unwrap_or("")) {
            if !is_entry_script(&path, language) {
                continue;
            }
            let relative = path.strip_prefix(base).unwrap_or(&path);
            if let Some(script) = script_from_paths(base, &path, &relative.to_string_lossy()) {
                scripts.push(ScriptFile {
                    base_dir: base_dir_str.to_string(),
                    ..script
                });
            }
        }
    }
}

fn script_from_paths(base_dir: &Path, full_path: &Path, script_path: &str) -> Option<ScriptFile> {
    let language = ScriptLanguage::from_extension(full_path.to_str()?)?;
    let entry_reason = entry_script_reason(full_path, language)?;
    let name = full_path.file_name()?.to_string_lossy().to_string();
    let alias = default_script_alias(&name);
    Some(ScriptFile {
        path: script_path.to_string(),
        name,
        alias,
        language,
        base_dir: base_dir.to_string_lossy().to_string(),
        entry_reason,
    })
}

pub fn default_script_alias(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(name)
        .to_string()
}

pub fn is_entry_script(path: &Path, language: ScriptLanguage) -> bool {
    entry_script_reason(path, language).is_some()
}

/// Why a file was detected as a runnable entry script — every signal that
/// matched, joined with " + ". `None` means it is not an entry script.
///
/// These signals are heuristics: a shebang is cosmetic and a Python `__main__`
/// block is common in library modules too, so the reason is surfaced to the
/// user rather than trusted blindly.
pub fn entry_script_reason(path: &Path, language: ScriptLanguage) -> Option<String> {
    let mut reasons: Vec<&str> = Vec::new();
    match language {
        ScriptLanguage::Python => {
            if has_shebang(path) {
                reasons.push("shebang");
            }
            if has_python_main_guard(path) {
                reasons.push("__main__ block");
            }
        }
        ScriptLanguage::Bash => {
            if has_shebang(path) {
                reasons.push("shebang");
            }
            if is_executable(path) {
                reasons.push("executable");
            }
        }
        ScriptLanguage::Bun => {
            if has_shebang(path) {
                reasons.push("shebang");
            }
            if has_js_entrypoint(path) {
                reasons.push("entrypoint marker");
            }
        }
        ScriptLanguage::PostgreSql => {
            if has_non_empty_content(path) {
                reasons.push("non-empty SQL");
            }
        }
    }
    if reasons.is_empty() {
        None
    } else {
        Some(reasons.join(" + "))
    }
}

fn has_shebang(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .is_some_and(|content| content.starts_with("#!"))
}

fn has_python_main_guard(path: &Path) -> bool {
    std::fs::read_to_string(path).ok().is_some_and(|content| {
        content.contains("if __name__ == \"__main__\"")
            || content.contains("if __name__ == '__main__'")
            || content.contains("if __name__==\"__main__\"")
            || content.contains("if __name__=='__main__'")
    })
}

fn has_js_entrypoint(path: &Path) -> bool {
    std::fs::read_to_string(path).ok().is_some_and(|content| {
        content.contains("import.meta.main")
            || content.contains("require.main === module")
            || content.contains("require.main == module")
            || content.contains("Bun.main")
    })
}

fn has_non_empty_content(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .is_some_and(|content| content.lines().any(|line| !line.trim().is_empty()))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    false
}

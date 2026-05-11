use std::collections::HashMap;
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
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
        }
        let database = Database::open(&db_path).map_err(|e| e.to_string())?;
        scheduler::initialize_schedule_times(&database)?;

        Ok(Self {
            db: Arc::new(Mutex::new(database)),
            db_path,
        })
    }

    /// Scan all registered work_dirs for scripts
    pub fn scan_scripts(&self) -> Vec<ScriptFile> {
        let (dirs, aliases) = {
            let db = self.db.lock().unwrap();
            let dirs = db.list_work_dirs().unwrap_or_default();
            let aliases = db
                .list_script_aliases()
                .unwrap_or_default()
                .into_iter()
                .map(|alias| ((alias.base_dir, alias.script_path), alias.alias))
                .collect::<HashMap<_, _>>();
            (dirs, aliases)
        };
        let mut all = Vec::new();
        for wd in &dirs {
            let base = Path::new(&wd.path);
            if base.is_dir() {
                scan_recursive(base, base, &wd.path, &mut all);
            }
        }
        for script in &mut all {
            script.alias = aliases
                .get(&(script.base_dir.clone(), script.path.clone()))
                .cloned()
                .unwrap_or_else(|| default_script_alias(&script.name));
        }
        all.sort_by(|a, b| (&a.base_dir, &a.path).cmp(&(&b.base_dir, &b.path)));
        all
    }
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
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            scripts.push(ScriptFile {
                path: relative.display().to_string(),
                name,
                alias: String::new(),
                language,
                base_dir: base_dir_str.to_string(),
            });
        }
    }
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
    match language {
        ScriptLanguage::Python => has_shebang(path) || has_python_main_guard(path),
        ScriptLanguage::Bash => has_shebang(path) || is_executable(path),
        ScriptLanguage::Bun => has_shebang(path) || has_js_entrypoint(path),
        ScriptLanguage::PostgreSql => has_non_empty_content(path),
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

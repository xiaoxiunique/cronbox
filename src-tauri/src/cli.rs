use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::Database;
use crate::executor;
use crate::models::{
    ExecutionResult, Job, JobStatus, Schedule, ScriptFile, ScriptLanguage, WorkDir,
};
use crate::scheduler;
use crate::state::{scan_directory_entries, script_from_file, AppState};
use tokio_util::sync::CancellationToken;

pub fn default_db_path() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.cronbox.app")
        .join("cronbox.db")
}

pub fn default_agent_workspace_path() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cronbox")
}

pub fn run_from_env() -> i32 {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}

fn run(args: Vec<String>) -> Result<i32, String> {
    if args.is_empty() {
        return run_serve(&[]);
    }

    match args[0].as_str() {
        "-h" | "--help" | "help" => {
            print_help();
            Ok(0)
        }
        "install-cli" => {
            let (target, force) = parse_install_cli_args(&args[1..])?;
            let path = install_cli_link(target, force)?;
            println!("installed: {}", path.display());
            Ok(0)
        }
        "cli-status" => {
            println!("{}", cli_status_text(None));
            Ok(0)
        }
        "add" => run_add(&args[1..]),
        "dirs" | "dir" => run_dirs(&args[1..]),
        "scripts" | "script" => run_scripts(&args[1..]),
        "schedules" | "schedule" => run_schedules(&args[1..]),
        "jobs" | "job" => run_jobs(&args[1..]),
        "skills" | "skill" => run_skills(&args[1..]),
        "run" => run_script(&args[1..]),
        "serve" => run_serve(&args[1..]),
        "daemon" => run_daemon(&args[1..]),
        "service" => crate::service::run(&args[1..]),
        "export" => run_export(&args[1..]),
        "import" => run_import(&args[1..]),
        other => Err(format!(
            "unknown command: {other}\n\nRun `cronbox help` for usage."
        )),
    }
}

fn run_serve(args: &[String]) -> Result<i32, String> {
    let mut host = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    let mut port = crate::web::DEFAULT_PORT;
    let mut open_browser = true;
    let mut auth_token: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--host" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("--host requires an IP address, e.g. 0.0.0.0 or 127.0.0.1")?;
                host = value
                    .parse::<std::net::IpAddr>()
                    .map_err(|_| format!("invalid IP address: {value}"))?;
            }
            "--port" => {
                i += 1;
                port = args
                    .get(i)
                    .ok_or("--port requires a number")?
                    .parse::<u16>()
                    .map_err(|_| "--port must be between 1 and 65535".to_string())?;
                if port == 0 {
                    return Err("--port must be between 1 and 65535".to_string());
                }
            }
            "--auth-token" => {
                i += 1;
                let value = args.get(i).ok_or("--auth-token requires a token value")?;
                auth_token = Some(value.clone());
            }
            "--no-open" => open_browser = false,
            other => {
                return Err(format!(
                    "unknown serve option: {other}\n\nUsage: cronbox serve [--host IP] [--port PORT] [--auth-token TOKEN] [--no-open]"
                ));
            }
        }
        i += 1;
    }

    let auth_token = match auth_token {
        Some(token) if token.is_empty() => None,
        Some(token) => Some(token),
        None => std::env::var("CRONBOX_AUTH_TOKEN")
            .ok()
            .filter(|token| !token.is_empty()),
    };

    crate::web::run(
        default_db_path(),
        crate::web::ServeOptions {
            host,
            port,
            auth_token,
            open_browser,
        },
    )
}

fn open_db() -> Result<Database, String> {
    let db_path = default_db_path();
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create data dir: {e}"))?;
    }
    Database::open(&db_path).map_err(|e| e.to_string())
}

fn run_dirs(args: &[String]) -> Result<i32, String> {
    match args.first().map(String::as_str) {
        Some("list") | None => {
            let db = open_db()?;
            print_work_dirs(&db.list_work_dirs().map_err(|e| e.to_string())?);
            Ok(0)
        }
        Some("add") => {
            let path = args.get(1).ok_or("usage: cronbox dirs add <path>")?;
            let path_buf = PathBuf::from(path);
            if !path_buf.is_dir() {
                return Err(format!("not a directory: {path}"));
            }
            let db = open_db()?;
            let dir = db.add_work_dir(path).map_err(|e| e.to_string())?;
            println!("added: {} {}", short_id(&dir.id), dir.path);
            Ok(0)
        }
        Some("remove") | Some("rm") => {
            let key = args
                .get(1)
                .ok_or("usage: cronbox dirs remove <id-or-path>")?;
            let db = open_db()?;
            let id = resolve_work_dir_id(&db, key)?;
            if db.remove_work_dir(&id).map_err(|e| e.to_string())? {
                println!("removed: {key}");
            } else {
                println!("not found: {key}");
            }
            Ok(0)
        }
        Some(other) => Err(format!("unknown dirs command: {other}")),
    }
}

fn run_scripts(args: &[String]) -> Result<i32, String> {
    match args.first().map(String::as_str) {
        Some("list") | None => {
            let state = AppState::new(default_db_path())?;
            print_scripts(&state.scan_scripts());
            Ok(0)
        }
        Some(other) => Err(format!("unknown scripts command: {other}")),
    }
}

fn run_add(args: &[String]) -> Result<i32, String> {
    let add = parse_cli_add_args(args)?;

    let path_buf = PathBuf::from(&add.path);
    if path_buf.is_file() {
        if add.all || !add.includes.is_empty() {
            return Err("usage: cronbox add <script-file> [--alias '<desc>']".to_string());
        }
        return add_script_file_from_cli(&path_buf, add.alias.as_deref());
    }

    if !path_buf.is_dir() {
        return Err(format!("not found: {}", add.path));
    }
    if add.alias.is_some() {
        return Err(
            "--alias only applies to the single-file form: cronbox add <script-file> --alias '<desc>'"
                .to_string(),
        );
    }

    let canonical_path = fs::canonicalize(&path_buf)
        .map_err(|e| format!("cannot resolve directory {}: {e}", path_buf.display()))?;
    let canonical_path = canonical_path.to_string_lossy().to_string();

    let candidates = scan_directory_entries(Path::new(&canonical_path));
    if candidates.is_empty() {
        println!("No entry scripts found.");
        return Ok(0);
    }

    if !add.all && add.includes.is_empty() {
        println!("Entry scripts found:");
        print_scripts(&candidates);
        println!();
        println!(
            "No scripts were added. Use `cronbox add <directory> --include <script>` or --all."
        );
        return Ok(0);
    }

    let selected = if add.all {
        candidates
    } else {
        let mut selected = Vec::new();
        for include in &add.includes {
            let Some(script) = candidates.iter().find(|script| script.path == *include) else {
                return Err(format!("script is not an executable entrypoint: {include}"));
            };
            selected.push(script.clone());
        }
        selected
    };

    let db = open_db()?;
    let dir = db
        .add_work_dir_manual(&canonical_path)
        .map_err(|e| e.to_string())?;
    println!("added directory: {} {}", short_id(&dir.id), dir.path);
    for script in &selected {
        db.add_script_entry(&dir.path, &script.path)
            .map_err(|e| e.to_string())?;
    }
    println!("Added scripts:");
    print_scripts(&selected);
    Ok(0)
}

struct CliAddArgs {
    path: String,
    all: bool,
    includes: Vec<String>,
    alias: Option<String>,
}

fn parse_cli_add_args(args: &[String]) -> Result<CliAddArgs, String> {
    let mut path = None;
    let mut all = false;
    let mut includes = Vec::new();
    let mut alias: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--all" => all = true,
            "--include" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("--include requires a relative script path")?;
                includes.push(value.clone());
            }
            "--alias" | "--name" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("--alias requires a short human description")?;
                alias = Some(value.clone());
            }
            value if value.starts_with("--") => {
                return Err(format!("unknown add option: {value}"));
            }
            value => {
                if path.is_some() {
                    return Err(
                        "usage: cronbox add <script-file> [--alias '<desc>']\n       cronbox add <directory> [--include SCRIPT] [--all]"
                            .to_string(),
                    );
                }
                path = Some(value.to_string());
            }
        }
        i += 1;
    }

    Ok(CliAddArgs {
        path: path.ok_or(
            "usage: cronbox add <script-file> [--alias '<desc>']\n       cronbox add <directory> [--include SCRIPT] [--all]",
        )?,
        all,
        includes,
        alias,
    })
}

fn add_script_file_from_cli(path: &Path, alias: Option<&str>) -> Result<i32, String> {
    let script = script_from_file(path)
        .ok_or_else(|| format!("file is not an executable entry script: {}", path.display()))?;
    let db = open_db()?;
    let dir = db
        .add_work_dir_manual(&script.base_dir)
        .map_err(|e| e.to_string())?;
    db.add_script_entry(&dir.path, &script.path)
        .map_err(|e| e.to_string())?;
    if let Some(alias) = alias.map(str::trim).filter(|a| !a.is_empty()) {
        db.set_script_alias(&dir.path, &script.path, Some(alias))
            .map_err(|e| e.to_string())?;
    }
    let resolved = db.get_script_alias(&dir.path, &script.path).ok().flatten();
    let mut final_script = script;
    if let Some(a) = resolved {
        final_script.alias = a;
    }
    println!("added script:");
    print_scripts(&[final_script]);
    Ok(0)
}

fn run_schedules(args: &[String]) -> Result<i32, String> {
    match args.first().map(String::as_str) {
        Some("list") | None => {
            let db = open_db()?;
            print_schedules(&db.list_schedules().map_err(|e| e.to_string())?);
            Ok(0)
        }
        Some("add") => {
            let db = open_db()?;
            let add = parse_schedule_add_args(&db, &args[1..])?;
            let schedule = create_schedule_from_cli(
                &db,
                &add.base_dir,
                &add.script_path,
                &add.cron_expr,
                &add.timezone,
                &add.args,
                add.one_shot,
            )?;
            print_schedules(&[schedule]);
            Ok(0)
        }
        Some("enable") => {
            set_schedule_enabled(args.get(1), true)?;
            Ok(0)
        }
        Some("disable") => {
            set_schedule_enabled(args.get(1), false)?;
            Ok(0)
        }
        Some("delete") | Some("rm") => {
            let key = args
                .get(1)
                .ok_or("usage: cronbox schedules delete <id-prefix>")?;
            let db = open_db()?;
            let id = resolve_schedule_id(&db, key)?;
            if db.delete_schedule(&id).map_err(|e| e.to_string())? {
                println!("deleted: {key}");
            }
            Ok(0)
        }
        Some(other) => Err(format!("unknown schedules command: {other}")),
    }
}

struct ScheduleAddArgs {
    base_dir: String,
    script_path: String,
    cron_expr: String,
    timezone: String,
    args: String,
    one_shot: bool,
}

fn parse_schedule_add_args(db: &Database, args: &[String]) -> Result<ScheduleAddArgs, String> {
    let parsed = parse_add_positionals_and_options(args)?;
    let timezone = parsed
        .timezone
        .unwrap_or_else(|| "Asia/Shanghai".to_string());

    match parsed.positionals.as_slice() {
        [script_file, cron_expr] => {
            let (base_dir, script_path) =
                resolve_script_location(db, script_file, parsed.base_dir.as_deref())?;
            Ok(ScheduleAddArgs {
                base_dir,
                script_path,
                cron_expr: cron_expr.clone(),
                timezone,
                args: parsed.args,
                one_shot: parsed.one_shot,
            })
        }
        [base_dir, script_path, cron_expr] if parsed.base_dir.is_none() => Ok(ScheduleAddArgs {
            base_dir: base_dir.clone(),
            script_path: script_path.clone(),
            cron_expr: cron_expr.clone(),
            timezone,
            args: parsed.args,
            one_shot: parsed.one_shot,
        }),
        _ => Err("usage: cronbox schedules add <script-file> <cron> [--dir DIR] [--tz TZ] [--args JSON] [--once]\n       cronbox schedules add <base-dir> <script-path> <cron> [--tz TZ] [--args JSON] [--once]".to_string()),
    }
}

struct ParsedAddInput {
    positionals: Vec<String>,
    base_dir: Option<String>,
    timezone: Option<String>,
    args: String,
    one_shot: bool,
}

fn parse_add_positionals_and_options(args: &[String]) -> Result<ParsedAddInput, String> {
    let mut input = ParsedAddInput {
        positionals: Vec::new(),
        base_dir: None,
        timezone: None,
        args: "{}".to_string(),
        one_shot: false,
    };

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--once" | "--one-shot" => {
                input.one_shot = true;
            }
            "--args" => {
                i += 1;
                let value = args.get(i).ok_or("--args requires a JSON value")?;
                input.args = read_args_value(value)?;
            }
            "--dir" | "--base-dir" => {
                i += 1;
                let value = args.get(i).ok_or("--dir requires a directory")?;
                input.base_dir = Some(value.clone());
            }
            "--tz" | "--timezone" => {
                i += 1;
                let value = args.get(i).ok_or("--tz requires a timezone")?;
                input.timezone = Some(value.clone());
            }
            other => input.positionals.push(other.to_string()),
        }
        i += 1;
    }

    Ok(input)
}

fn resolve_script_location(
    db: &Database,
    script_file: &str,
    explicit_base_dir: Option<&str>,
) -> Result<(String, String), String> {
    if let Some(base_dir) = explicit_base_dir {
        let full = PathBuf::from(base_dir).join(script_file);
        if !full.exists() {
            return Err(format!("script not found: {}", full.display()));
        }
        return Ok((base_dir.to_string(), script_file.to_string()));
    }

    let input_path = PathBuf::from(script_file);
    let full = if input_path.is_absolute() {
        input_path
    } else {
        env::current_dir()
            .map_err(|e| format!("cannot read current directory: {e}"))?
            .join(input_path)
    };

    if !full.exists() {
        return Err(format!("script not found: {}", full.display()));
    }

    let canonical_full =
        fs::canonicalize(&full).map_err(|e| format!("cannot resolve {}: {e}", full.display()))?;

    let mut best_match: Option<(usize, String, String)> = None;
    for work_dir in db.list_work_dirs().map_err(|e| e.to_string())? {
        let base = PathBuf::from(&work_dir.path);
        let Ok(canonical_base) = fs::canonicalize(&base) else {
            continue;
        };
        if !canonical_full.starts_with(&canonical_base) {
            continue;
        }
        let Ok(relative) = canonical_full.strip_prefix(&canonical_base) else {
            continue;
        };
        let depth = canonical_base.components().count();
        let script_path = relative.to_string_lossy().to_string();
        if best_match
            .as_ref()
            .is_none_or(|(best_depth, _, _)| depth > *best_depth)
        {
            best_match = Some((depth, work_dir.path, script_path));
        }
    }

    if let Some((_, base_dir, script_path)) = best_match {
        return Ok((base_dir, script_path));
    }

    let base_dir = canonical_full
        .parent()
        .ok_or_else(|| {
            format!(
                "script has no parent directory: {}",
                canonical_full.display()
            )
        })?
        .to_string_lossy()
        .to_string();
    let script_path = canonical_full
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid script filename: {}", canonical_full.display()))?
        .to_string();
    Ok((base_dir, script_path))
}

fn create_schedule_from_cli(
    db: &Database,
    base_dir: &str,
    script_path: &str,
    cron_expr: &str,
    timezone: &str,
    args: &str,
    one_shot: bool,
) -> Result<Schedule, String> {
    let script_full_path = PathBuf::from(base_dir).join(script_path);
    if !script_full_path.exists() {
        return Err(format!("script not found: {}", script_full_path.display()));
    }
    scheduler::validate_cron(cron_expr)?;
    validate_json(args)?;
    let next = scheduler::calculate_next_run(cron_expr, timezone)?;

    let schedule = db
        .create_schedule(
            script_path,
            base_dir,
            cron_expr,
            timezone,
            args,
            "{}",
            one_shot,
        )
        .map_err(|e| e.to_string())?;
    db.update_schedule_next_run(&schedule.id, &next)
        .map_err(|e| e.to_string())?;
    db.get_schedule(&schedule.id).map_err(|e| e.to_string())
}

fn set_schedule_enabled(key: Option<&String>, enabled: bool) -> Result<(), String> {
    let key = key.ok_or("usage: cronbox schedules <enable|disable> <id-prefix>")?;
    let db = open_db()?;
    let id = resolve_schedule_id(&db, key)?;
    db.set_schedule_enabled(&id, enabled)
        .map_err(|e| e.to_string())?;
    if enabled {
        let schedule = db.get_schedule(&id).map_err(|e| e.to_string())?;
        if let Ok(next) = scheduler::calculate_next_run(&schedule.cron_expr, &schedule.timezone) {
            let _ = db.update_schedule_next_run(&id, &next);
        }
    }
    println!("{}: {key}", if enabled { "enabled" } else { "disabled" });
    Ok(())
}

fn run_jobs(args: &[String]) -> Result<i32, String> {
    match args.first().map(String::as_str) {
        Some("list") | None => {
            let limit = args
                .get(1)
                .map(|s| s.parse::<u32>().map_err(|_| format!("invalid limit: {s}")))
                .transpose()?
                .unwrap_or(20);
            let db = open_db()?;
            print_jobs(&db.list_jobs(limit).map_err(|e| e.to_string())?);
            Ok(0)
        }
        Some("running") => {
            let db = open_db()?;
            print_jobs(&db.list_running_jobs().map_err(|e| e.to_string())?);
            Ok(0)
        }
        Some("show") => {
            let key = args.get(1).ok_or("usage: cronbox jobs show <id-prefix>")?;
            let db = open_db()?;
            let id = resolve_job_id(&db, key)?;
            print_job_detail(&db.get_job(&id).map_err(|e| e.to_string())?);
            Ok(0)
        }
        Some("cancel") => {
            let key = args
                .get(1)
                .ok_or("usage: cronbox jobs cancel <id-prefix>")?;
            let db = open_db()?;
            let id = resolve_job_id(&db, key)?;
            if db.cancel_job(&id).map_err(|e| e.to_string())? {
                println!("cancelled: {key}");
            } else {
                println!("not running: {key}");
            }
            Ok(0)
        }
        Some("cleanup") => {
            let days = args
                .get(1)
                .map(|s| s.parse::<u32>().map_err(|_| format!("invalid days: {s}")))
                .transpose()?
                .unwrap_or(30);
            let db = open_db()?;
            let count = db.cleanup_old_jobs(days).map_err(|e| e.to_string())?;
            println!("deleted {count} jobs older than {days} days");
            Ok(0)
        }
        Some(other) => Err(format!("unknown jobs command: {other}")),
    }
}

fn run_script(args: &[String]) -> Result<i32, String> {
    let base_dir = args
        .first()
        .ok_or("usage: cronbox run <base-dir> <script-path> [--args JSON]")?;
    let script_path = args
        .get(1)
        .ok_or("usage: cronbox run <base-dir> <script-path> [--args JSON]")?;
    let options = parse_run_options(&args[2..])?;
    validate_json(&options.args)?;

    let full_path = PathBuf::from(base_dir).join(script_path);
    if !full_path.exists() {
        return Err(format!("script not found: {}", full_path.display()));
    }
    let language = ScriptLanguage::from_extension(script_path)
        .ok_or_else(|| format!("unknown script type: {script_path}"))?;

    let db = open_db()?;
    let job = db
        .create_job(None, script_path, base_dir, &options.args, None)
        .map_err(|e| e.to_string())?;
    db.mark_job_running(&job.id).map_err(|e| e.to_string())?;

    let db_path = default_db_path();
    let job_id = job.id.clone();
    let log_callback: executor::LogCallback = Arc::new(move |chunk| {
        print!("{chunk}");
        if let Ok(db) = Database::open(&db_path) {
            let _ = db.append_job_logs(&job_id, chunk);
        }
    });
    let result = tokio::runtime::Runtime::new()
        .map_err(|e| e.to_string())?
        .block_on(executor::execute_with_log_callback(
            &full_path,
            language,
            &options.args,
            "{}",
            Some(log_callback),
        ));

    let success = result.exit_code == 0;
    let logs = db
        .get_job(&job.id)
        .map(|job| job.logs)
        .unwrap_or_else(|_| combined_logs(&result));
    let error = if success {
        None
    } else {
        Some(result.stderr.lines().last().unwrap_or("Error").to_string())
    };
    db.mark_job_completed(
        &job.id,
        success,
        result.result.as_deref(),
        &logs,
        error.as_deref(),
        result.duration_ms,
    )
    .map_err(|e| e.to_string())?;

    if !logs.ends_with('\n') && !logs.is_empty() {
        println!();
    }
    println!(
        "job {} {} in {}ms",
        short_id(&job.id),
        if success { "succeeded" } else { "failed" },
        result.duration_ms
    );
    Ok(if success { 0 } else { result.exit_code.max(1) })
}

/// Scheduler-only mode for an always-on host. Foreground; stops on Ctrl-C or SIGTERM.
fn run_daemon(args: &[String]) -> Result<i32, String> {
    if let Some(unexpected) = args.iter().find(|a| !a.is_empty()) {
        return Err(format!(
            "daemon takes no arguments, got: {unexpected}\n\nUsage: cronbox daemon"
        ));
    }

    // Resolve the login-shell PATH so scripts run with the same tools the user
    // has in their terminal.
    executor::env::init_effective_path_from_login_shell();

    let db_path = default_db_path();
    // Only one process may run the scheduler against the shared DB.
    let lock_path = db_path.with_file_name("scheduler.lock");
    let lease = match crate::scheduler_lock::try_acquire(&lock_path)? {
        Some(lease) => lease,
        None => {
            return Err(
                "another CronBox scheduler is already running (the Web console service or \
                        another daemon). Stop it first, then retry."
                    .to_string(),
            );
        }
    };
    let app_state = AppState::new_engine(db_path)?;
    let db = app_state.db.clone();
    let db_p = app_state.db_path.clone();

    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let _lease = lease; // held for the lifetime of the loop
        let cancel = CancellationToken::new();
        spawn_shutdown_signal(cancel.clone());

        println!("cronbox daemon started");
        println!("  db: {}", db_p.display());
        println!("  scheduler ticking every 1s · press Ctrl-C to stop");

        scheduler::run_scheduler(db, db_p, cancel).await;
        println!("cronbox daemon stopped");
    });

    Ok(0)
}

/// Cancel the scheduler on Ctrl-C (all platforms) or SIGTERM (unix).
fn spawn_shutdown_signal(cancel: CancellationToken) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            match signal(SignalKind::terminate()) {
                Ok(mut term) => {
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {}
                        _ = term.recv() => {}
                    }
                }
                Err(_) => {
                    let _ = tokio::signal::ctrl_c().await;
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        cancel.cancel();
    });
}

// ── export / import (portable, home-relative) ──

#[derive(serde::Serialize, serde::Deserialize)]
struct ExportBundle {
    cronbox_export_version: u32,
    exported_at: String,
    /// The $HOME the paths were relativized from, for human reference.
    home: String,
    work_dirs: Vec<ExportWorkDir>,
    script_entries: Vec<ExportEntry>,
    script_aliases: Vec<ExportAlias>,
    schedules: Vec<ExportSchedule>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ExportWorkDir {
    path: String,
    scan_mode: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ExportEntry {
    base_dir: String,
    script_path: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ExportAlias {
    base_dir: String,
    script_path: String,
    alias: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ExportSchedule {
    id: String,
    base_dir: String,
    script_path: String,
    cron_expr: String,
    timezone: String,
    args: String,
    env: String,
    enabled: bool,
    one_shot: bool,
}

fn home_dir_string() -> Result<String, String> {
    dirs_next::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "cannot resolve home directory".to_string())
}

/// Rewrite an absolute path under `$HOME` to a portable `~/...` form. Paths not
/// under `$HOME` are returned unchanged (and stay machine-specific).
fn portable_path(abs: &str, home: &str) -> String {
    if abs == home {
        "~".to_string()
    } else if let Some(rest) = abs.strip_prefix(&format!("{home}/")) {
        format!("~/{rest}")
    } else {
        abs.to_string()
    }
}

/// Expand a portable `~/...` path against the current platform's `$HOME`.
fn local_path(portable: &str, home: &str) -> String {
    if portable == "~" {
        home.to_string()
    } else if let Some(rest) = portable.strip_prefix("~/") {
        format!("{home}/{rest}")
    } else {
        portable.to_string()
    }
}

fn run_export(args: &[String]) -> Result<i32, String> {
    let mut out: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" | "-o" => {
                i += 1;
                out = Some(args.get(i).ok_or("--out requires a file path")?.clone());
            }
            other => return Err(format!("unknown export option: {other}")),
        }
        i += 1;
    }

    let home = home_dir_string()?;
    let db = open_db()?;
    let work_dirs = db.list_work_dirs().map_err(|e| e.to_string())?;
    let entries = db.list_script_entries().map_err(|e| e.to_string())?;
    let aliases = db.list_script_aliases().map_err(|e| e.to_string())?;
    let schedules = db.list_schedules().map_err(|e| e.to_string())?;

    let bundle = ExportBundle {
        cronbox_export_version: 1,
        exported_at: chrono::Utc::now().to_rfc3339(),
        home: home.clone(),
        work_dirs: work_dirs
            .iter()
            .map(|w| ExportWorkDir {
                path: portable_path(&w.path, &home),
                scan_mode: w.scan_mode.clone(),
            })
            .collect(),
        script_entries: entries
            .iter()
            .map(|e| ExportEntry {
                base_dir: portable_path(&e.base_dir, &home),
                script_path: e.script_path.clone(),
            })
            .collect(),
        script_aliases: aliases
            .iter()
            .map(|a| ExportAlias {
                base_dir: portable_path(&a.base_dir, &home),
                script_path: a.script_path.clone(),
                alias: a.alias.clone(),
            })
            .collect(),
        schedules: schedules
            .iter()
            .map(|s| ExportSchedule {
                id: s.id.clone(),
                base_dir: portable_path(&s.base_dir, &home),
                script_path: s.script_path.clone(),
                cron_expr: s.cron_expr.clone(),
                timezone: s.timezone.clone(),
                args: s.args.clone(),
                env: s.env.clone(),
                enabled: s.enabled,
                one_shot: s.one_shot,
            })
            .collect(),
    };

    let json = serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?;
    match out {
        Some(path) => {
            fs::write(&path, json).map_err(|e| format!("cannot write {path}: {e}"))?;
            eprintln!(
                "exported {} schedule(s), {} dir(s) to {path}",
                bundle.schedules.len(),
                bundle.work_dirs.len()
            );
        }
        None => println!("{json}"),
    }
    Ok(0)
}

fn run_import(args: &[String]) -> Result<i32, String> {
    let file = args
        .first()
        .ok_or("usage: cronbox import <file.json>  (use - for stdin)")?;

    let raw = if file == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("cannot read stdin: {e}"))?;
        buf
    } else {
        fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?
    };

    let bundle: ExportBundle =
        serde_json::from_str(&raw).map_err(|e| format!("invalid export file: {e}"))?;
    if bundle.cronbox_export_version != 1 {
        return Err(format!(
            "unsupported export version {} (this build understands version 1)",
            bundle.cronbox_export_version
        ));
    }

    let home = home_dir_string()?;
    let db = open_db()?;

    for w in &bundle.work_dirs {
        let path = local_path(&w.path, &home);
        if w.scan_mode == "manual" {
            db.add_work_dir_manual(&path)
        } else {
            db.add_work_dir(&path)
        }
        .map_err(|e| e.to_string())?;
    }
    for e in &bundle.script_entries {
        db.add_script_entry(&local_path(&e.base_dir, &home), &e.script_path)
            .map_err(|err| err.to_string())?;
    }
    for a in &bundle.script_aliases {
        db.set_script_alias(
            &local_path(&a.base_dir, &home),
            &a.script_path,
            Some(a.alias.as_str()),
        )
        .map_err(|e| e.to_string())?;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut missing: Vec<String> = Vec::new();
    for s in &bundle.schedules {
        let base_dir = local_path(&s.base_dir, &home);
        let full = PathBuf::from(&base_dir).join(&s.script_path);
        if !full.exists() {
            missing.push(full.display().to_string());
        }
        let next_run_at = if s.enabled {
            scheduler::calculate_next_run(&s.cron_expr, &s.timezone).ok()
        } else {
            None
        };
        let schedule = Schedule {
            id: s.id.clone(),
            script_path: s.script_path.clone(),
            base_dir,
            cron_expr: s.cron_expr.clone(),
            timezone: s.timezone.clone(),
            args: s.args.clone(),
            env: s.env.clone(),
            enabled: s.enabled,
            one_shot: s.one_shot,
            next_run_at,
            last_run_at: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        db.import_schedule(&schedule).map_err(|e| e.to_string())?;
    }

    eprintln!(
        "imported {} schedule(s), {} dir(s)",
        bundle.schedules.len(),
        bundle.work_dirs.len()
    );
    if !missing.is_empty() {
        eprintln!(
            "warning: {} script file(s) not found at the mapped path — sync the script files too:",
            missing.len()
        );
        for path in &missing {
            eprintln!("  {path}");
        }
    }
    Ok(0)
}

fn combined_logs(result: &ExecutionResult) -> String {
    if result.stderr.is_empty() {
        result.stdout.clone()
    } else {
        format!("{}\n--- stderr ---\n{}", result.stdout, result.stderr)
    }
}

pub fn install_cli_link(target: Option<PathBuf>, force: bool) -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("cannot resolve current executable: {e}"))?;
    let target = target.unwrap_or_else(default_cli_target);

    if paths_refer_to_same_file(&target, &exe) {
        return Ok(target);
    }

    if target.exists() {
        if !force {
            return Err(format!(
                "{} already exists. Re-run with --force or choose --target PATH.",
                target.display()
            ));
        }
        if target.is_dir() {
            return Err(format!("target is a directory: {}", target.display()));
        }
        fs::remove_file(&target)
            .map_err(|e| format!("cannot replace {}: {e}", target.display()))?;
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }

    create_link_or_copy(&exe, &target)?;
    Ok(target)
}

pub fn cli_status_text(target: Option<PathBuf>) -> String {
    let target = target.unwrap_or_else(default_cli_target);
    let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("<unknown>"));
    if paths_refer_to_same_file(&target, &exe) {
        format!("installed: {} -> {}", target.display(), exe.display())
    } else if target.exists() {
        format!("different file exists: {}", target.display())
    } else {
        format!("not installed: {}", target.display())
    }
}

fn default_cli_target() -> PathBuf {
    // Among common bin directories on the resolved login-shell PATH, prefer one
    // writable without sudo. On Apple Silicon `/usr/local/bin` is often root-owned.
    let path = crate::executor::env::effective_path();
    for dir in env::split_paths(&path) {
        let s = dir.to_string_lossy();
        let known = s == "/usr/local/bin" || s == "/opt/homebrew/bin" || s.ends_with("/.local/bin");
        if known && dir.is_dir() && dir_is_writable(&dir) {
            return dir.join("cronbox");
        }
    }
    // Fallback: ~/.local/bin — always creatable without sudo (the user may
    // still need to add it to PATH for the `cronbox` command to resolve).
    if let Some(home) = dirs_next::home_dir() {
        return home.join(".local/bin/cronbox");
    }
    PathBuf::from("/usr/local/bin/cronbox")
}

/// Whether a new file can be created in `dir` without sudo. Probes by creating
/// and removing a temp file — the only reliable cross-platform check.
fn dir_is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".cronbox-write-test-{}", std::process::id()));
    match fs::File::create(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn parse_install_cli_args(args: &[String]) -> Result<(Option<PathBuf>, bool), String> {
    let mut target = None;
    let mut force = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or("usage: cronbox install-cli [--target PATH] [--force]")?;
                target = Some(PathBuf::from(value));
            }
            "--force" | "-f" => force = true,
            other => return Err(format!("unknown install-cli option: {other}")),
        }
        i += 1;
    }
    Ok((target, force))
}

#[derive(Default)]
struct RunOptions {
    args: String,
    timezone: Option<String>,
}

fn parse_run_options(args: &[String]) -> Result<RunOptions, String> {
    let mut options = RunOptions {
        args: "{}".to_string(),
        timezone: None,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--args" => {
                i += 1;
                let value = args.get(i).ok_or("--args requires a JSON value")?;
                options.args = read_args_value(value)?;
            }
            "--tz" | "--timezone" => {
                i += 1;
                let value = args.get(i).ok_or("--tz requires a timezone")?;
                options.timezone = Some(value.clone());
            }
            other => return Err(format!("unknown option: {other}")),
        }
        i += 1;
    }
    Ok(options)
}

fn read_args_value(value: &str) -> Result<String, String> {
    if let Some(path) = value.strip_prefix('@') {
        fs::read_to_string(path).map_err(|e| format!("cannot read args file {path}: {e}"))
    } else {
        Ok(value.to_string())
    }
}

fn validate_json(value: &str) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(value)
        .map(|_| ())
        .map_err(|e| format!("invalid JSON args: {e}"))
}

fn resolve_work_dir_id(db: &Database, key: &str) -> Result<String, String> {
    let dirs = db.list_work_dirs().map_err(|e| e.to_string())?;
    resolve_id_or_path(key, dirs.iter().map(|d| (&d.id, Some(&d.path))))
}

fn resolve_schedule_id(db: &Database, key: &str) -> Result<String, String> {
    let schedules = db.list_schedules().map_err(|e| e.to_string())?;
    resolve_id_or_path(key, schedules.iter().map(|s| (&s.id, None)))
}

fn resolve_job_id(db: &Database, key: &str) -> Result<String, String> {
    let jobs = db.list_jobs(500).map_err(|e| e.to_string())?;
    resolve_id_or_path(key, jobs.iter().map(|j| (&j.id, None)))
}

fn resolve_id_or_path<'a, I>(key: &str, items: I) -> Result<String, String>
where
    I: Iterator<Item = (&'a String, Option<&'a String>)>,
{
    let mut matches = Vec::new();
    for (id, path) in items {
        if id == key || id.starts_with(key) || path.is_some_and(|p| p == key) {
            matches.push(id.clone());
        }
    }
    match matches.len() {
        0 => Err(format!("not found: {key}")),
        1 => Ok(matches.remove(0)),
        _ => Err(format!("ambiguous id prefix: {key}")),
    }
}

fn paths_refer_to_same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[cfg(unix)]
fn create_link_or_copy(exe: &Path, target: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(exe, target)
        .map_err(|e| format!("cannot link {} -> {}: {e}", target.display(), exe.display()))
}

#[cfg(not(unix))]
fn create_link_or_copy(exe: &Path, target: &Path) -> Result<(), String> {
    fs::copy(exe, target)
        .map(|_| ())
        .map_err(|e| format!("cannot copy {} -> {}: {e}", exe.display(), target.display()))
}

// ── Skill install (AI agent integration) ────────────────────────────────

/// SKILL.md content shipped with the binary. Lets an AI agent (Claude Code,
/// for now) understand the CronBox CLI end-to-end and create scheduled tasks
/// on the user's behalf without the user touching CronBox's UI.
const SKILL_CONTENT: &str = include_str!("../../skills/cronbox.md");

fn run_skills(args: &[String]) -> Result<i32, String> {
    match args.first().map(String::as_str) {
        Some("install") => skills_install(&args[1..]),
        Some("status") | None => {
            print!("{}", skills_status_text());
            Ok(0)
        }
        Some("uninstall") | Some("remove") | Some("rm") => skills_uninstall(),
        Some(other) => Err(format!(
            "unknown skills command: {other}\nusage: cronbox skills install [--force] | status | uninstall"
        )),
    }
}

fn claude_skill_path() -> Result<PathBuf, String> {
    let home = dirs_next::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".claude/skills/cronbox/SKILL.md"))
}

fn skills_install(args: &[String]) -> Result<i32, String> {
    let force = args.iter().any(|a| a == "--force" || a == "-f");
    let target = claude_skill_path()?;
    let skill_dir = target
        .parent()
        .ok_or_else(|| format!("invalid skill path: {}", target.display()))?;
    let skills_root = skill_dir
        .parent()
        .ok_or_else(|| format!("invalid skill path: {}", target.display()))?;

    if !skills_root.is_dir() {
        return Err(format!(
            "{} does not exist — install Claude Code (or run it once) before `cronbox skills install`",
            skills_root.display()
        ));
    }

    if target.exists() && !force {
        let existing = fs::read_to_string(&target).unwrap_or_default();
        if existing == SKILL_CONTENT {
            println!("up to date: {}", target.display());
            return Ok(0);
        }
        return Err(format!(
            "{} already exists with different content. Re-run with --force to overwrite.",
            target.display()
        ));
    }

    fs::create_dir_all(skill_dir)
        .map_err(|e| format!("cannot create {}: {e}", skill_dir.display()))?;
    fs::write(&target, SKILL_CONTENT)
        .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
    println!("installed: {}", target.display());
    Ok(0)
}

fn skills_status_text() -> String {
    let target = match claude_skill_path() {
        Ok(p) => p,
        Err(err) => return format!("error: {err}\n"),
    };
    if !target.exists() {
        return format!("not installed: {}\n", target.display());
    }
    let on_disk = fs::read_to_string(&target).unwrap_or_default();
    if on_disk == SKILL_CONTENT {
        format!("installed (current): {}\n", target.display())
    } else {
        format!(
            "installed (out of date — re-run `cronbox skills install --force`): {}\n",
            target.display()
        )
    }
}

fn skills_uninstall() -> Result<i32, String> {
    let target = claude_skill_path()?;
    if !target.exists() {
        println!("nothing to remove: {}", target.display());
        return Ok(0);
    }
    fs::remove_file(&target).map_err(|e| format!("cannot remove {}: {e}", target.display()))?;
    // Best-effort: clean up the now-empty parent dir.
    if let Some(parent) = target.parent() {
        let _ = fs::remove_dir(parent);
    }
    println!("removed: {}", target.display());
    Ok(0)
}

fn print_help() {
    println!(
        r#"CronBox CLI

Usage:
  cronbox
  cronbox serve [--host IP] [--port PORT] [--auth-token TOKEN] [--no-open]
  cronbox help
  cronbox add <script-file> [--alias DESC]
  cronbox add <directory> [--include SCRIPT] [--all]
  cronbox install-cli [--target PATH] [--force]
  cronbox cli-status

  cronbox dirs list
  cronbox dirs add <path>
  cronbox dirs remove <id-or-path>

  cronbox scripts list

  cronbox schedules list
  cronbox schedules add <script-file> <cron> [--dir DIR] [--tz TZ] [--args JSON] [--once]
  cronbox schedules add <base-dir> <script-path> <cron> [--tz TZ] [--args JSON] [--once]
  cronbox schedules enable <id-prefix>
  cronbox schedules disable <id-prefix>
  cronbox schedules delete <id-prefix>

  cronbox jobs list [limit]
  cronbox jobs running
  cronbox jobs show <id-prefix>
  cronbox jobs cancel <id-prefix>
  cronbox jobs cleanup [days]

  cronbox skills install [--force]
  cronbox skills status
  cronbox skills uninstall

  cronbox run <base-dir> <script-path> [--args JSON]

  cronbox daemon

  cronbox service install
  cronbox service status
  cronbox service start|stop|restart
  cronbox service logs [--follow]
  cronbox service uninstall

  cronbox export [--out FILE]
  cronbox import <file.json>

Notes:
  Running cronbox without arguments starts the scheduler and local web console
  at http://127.0.0.1:4317. The server only listens on this computer.
  To expose the console on a network (e.g. a Linux box), bind a non-loopback
  address with --host 0.0.0.0 and require a password:
    cronbox serve --host 0.0.0.0 --auth-token 'pick-a-long-random-token'
  The token can also be set via CRONBOX_AUTH_TOKEN. Serving on a non-loopback
  address without a token is refused. Browsers get a built-in login page; API
  calls need `Authorization: Bearer <token>`.
  Start with `cronbox add <script-file>` to register one script, or
  `cronbox add <directory>` to preview entry scripts in a directory.
  Directory adds are selective by default: use --include relative/path.py to
  add one candidate, repeat --include for several, or use --all for script
  library directories where every entrypoint should be registered.
  Then use `cronbox scripts list` to inspect discovered scripts, and
  `cronbox schedules add <script-file> <cron> [--dir DIR]` to schedule one.
  Quote cron expressions, for example: "0 * * * *"
  Use --args @file.json to load JSON args from a file.
  Scheduled jobs run while `cronbox` or `cronbox daemon` remains active.
"#
    );
}

fn print_work_dirs(dirs: &[WorkDir]) {
    if dirs.is_empty() {
        println!("No working directories.");
        return;
    }
    for dir in dirs {
        println!("{}\t{}", short_id(&dir.id), dir.path);
    }
}

fn print_scripts(scripts: &[ScriptFile]) {
    if scripts.is_empty() {
        println!("No scripts found.");
        return;
    }
    for script in scripts {
        println!(
            "{}\t{}\t{}\t{}",
            script.language.as_str(),
            script.alias,
            script.base_dir,
            script.path
        );
    }
}

fn print_schedules(schedules: &[Schedule]) {
    if schedules.is_empty() {
        println!("No schedules.");
        return;
    }
    for schedule in schedules {
        println!(
            "{}\t{}\t{}\t{}\t{}\tnext={}",
            short_id(&schedule.id),
            if schedule.enabled {
                "enabled"
            } else {
                "paused"
            },
            schedule.cron_expr,
            schedule.timezone,
            PathBuf::from(&schedule.base_dir)
                .join(&schedule.script_path)
                .display(),
            schedule.next_run_at.as_deref().unwrap_or("-")
        );
    }
}

fn print_jobs(jobs: &[Job]) {
    if jobs.is_empty() {
        println!("No jobs.");
        return;
    }
    for job in jobs {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            short_id(&job.id),
            status_label(job.status),
            job.created_at,
            job.duration_ms
                .map(|d| format!("{d}ms"))
                .unwrap_or_else(|| "-".to_string()),
            PathBuf::from(&job.base_dir)
                .join(&job.script_path)
                .display()
        );
    }
}

fn print_job_detail(job: &Job) {
    println!("id: {}", job.id);
    println!("status: {}", status_label(job.status));
    println!(
        "script: {}",
        PathBuf::from(&job.base_dir)
            .join(&job.script_path)
            .display()
    );
    println!("args: {}", job.args);
    println!("created_at: {}", job.created_at);
    if let Some(started_at) = &job.started_at {
        println!("started_at: {started_at}");
    }
    if let Some(completed_at) = &job.completed_at {
        println!("completed_at: {completed_at}");
    }
    if let Some(duration_ms) = job.duration_ms {
        println!("duration_ms: {duration_ms}");
    }
    if let Some(error) = &job.error {
        println!("error: {error}");
    }
    if let Some(result) = &job.result {
        println!("result: {result}");
    }
    if !job.logs.is_empty() {
        println!("\nlogs:\n{}", job.logs);
    }
}

fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn status_label(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Success => "success",
        JobStatus::Failure => "failure",
        JobStatus::Cancelled => "cancelled",
        JobStatus::Skipped => "skipped",
    }
}

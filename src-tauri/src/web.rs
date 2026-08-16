use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::commands;
use crate::scheduler;
use crate::scheduler_lock::SchedulerLease;
use crate::state::AppState;

pub const DEFAULT_PORT: u16 = 4317;

#[derive(Debug, Clone, Copy)]
pub struct ServeOptions {
    pub port: u16,
    pub open_browser: bool,
}

#[derive(Clone)]
struct WebState {
    app: Arc<AppState>,
    scheduler_active: Arc<AtomicBool>,
}

#[derive(RustEmbed)]
#[folder = "../dist"]
struct WebAssets;

pub fn run(db_path: PathBuf, options: ServeOptions) -> Result<i32, String> {
    crate::executor::env::init_effective_path_from_login_shell();

    let lock_path = db_path.with_file_name("scheduler.lock");
    let lease = crate::scheduler_lock::try_acquire(&lock_path)?;
    let app = if lease.is_some() {
        AppState::new_engine(db_path)?
    } else {
        AppState::new(db_path)?
    };

    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime.block_on(serve(Arc::new(app), lease, options))?;
    Ok(0)
}

async fn serve(
    app: Arc<AppState>,
    lease: Option<SchedulerLease>,
    options: ServeOptions,
) -> Result<(), String> {
    let starts_active = lease.is_some();
    let scheduler_active = Arc::new(AtomicBool::new(starts_active));
    let db_path = app.db_path.clone();
    let cancel = CancellationToken::new();
    let scheduler_task = {
        let app = app.clone();
        let scheduler_active = scheduler_active.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            scheduler_supervisor(app, lease, scheduler_active, cancel).await;
        })
    };

    let state = WebState {
        app,
        scheduler_active,
    };
    let router = Router::new()
        .route("/api/health", get(health))
        .route("/api/invoke/{command}", post(invoke))
        .fallback(get(asset))
        .with_state(state);

    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), options.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|e| format!("cannot listen on http://{address}: {e}"))?;
    let url = format!("http://{address}");

    println!("CronBox web console is running");
    println!("  URL: {url}");
    println!("  DB:  {}", db_path.display());
    if !starts_active {
        println!("  Scheduler: standby (waiting to acquire the scheduler lock)");
    }
    println!("  Press Ctrl-C to stop");

    if options.open_browser {
        open_browser(&url);
    }

    let shutdown = {
        let cancel = cancel.clone();
        async move {
            wait_for_shutdown().await;
            cancel.cancel();
        }
    };

    let result = axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(|e| e.to_string());
    cancel.cancel();
    let _ = scheduler_task.await;
    result
}

async fn scheduler_supervisor(
    app: Arc<AppState>,
    initial_lease: Option<SchedulerLease>,
    active: Arc<AtomicBool>,
    cancel: CancellationToken,
) {
    let lease = match initial_lease {
        Some(lease) => lease,
        None => {
            let lock_path = app.db_path.with_file_name("scheduler.lock");
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
                }
                match crate::scheduler_lock::try_acquire(&lock_path) {
                    Ok(Some(lease)) => break lease,
                    Ok(None) => continue,
                    Err(error) => {
                        tracing::error!("scheduler lock retry failed: {error}");
                    }
                }
            }
        }
    };

    if let Err(error) = app.recover_interrupted_jobs() {
        tracing::error!("scheduler recovery failed: {error}");
    }
    active.store(true, Ordering::Release);
    println!("CronBox scheduler is active");

    let _lease = lease;
    scheduler::run_scheduler(app.db.clone(), app.db_path.clone(), cancel).await;
    active.store(false, Ordering::Release);
}

async fn health(State(state): State<WebState>) -> Json<Value> {
    Json(serde_json::json!({
        "ok": true,
        "scheduler_active": state.scheduler_active.load(Ordering::Acquire),
    }))
}

async fn invoke(
    State(state): State<WebState>,
    Path(command): Path<String>,
    Json(args): Json<Value>,
) -> Response {
    match dispatch(&state, &command, args).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error })),
        )
            .into_response(),
    }
}

async fn dispatch(state: &WebState, command: &str, args: Value) -> Result<Value, String> {
    let app = state.app.as_ref();
    match command {
        "dashboard_stats" => encode(commands::dashboard_stats(app)),
        "list_work_dirs" => encode(commands::list_work_dirs(app)),
        "add_work_dir" => {
            let args: PathArgs = decode(args)?;
            encode(commands::add_work_dir(app, args.path))
        }
        "add_work_dir_with_scan" => {
            let args: PathArgs = decode(args)?;
            encode(commands::add_work_dir_with_scan(app, args.path))
        }
        "preview_script_entries" => {
            let args: PathArgs = decode(args)?;
            encode(commands::preview_script_entries(args.path))
        }
        "add_selected_scripts" => {
            let args: SelectedScriptsArgs = decode(args)?;
            encode(commands::add_selected_scripts(
                app,
                args.base_dir,
                args.script_paths,
            ))
        }
        "add_script_file" => {
            let args: PathArgs = decode(args)?;
            encode(commands::add_script_file(app, args.path))
        }
        "remove_work_dir" => {
            let args: IdArgs = decode(args)?;
            encode(commands::remove_work_dir(app, args.id))
        }
        "scan_scripts" => encode(Ok::<_, String>(commands::scan_scripts(app))),
        "set_script_alias" => {
            let args: ScriptAliasArgs = decode(args)?;
            encode(commands::set_script_alias(
                app,
                args.base_dir,
                args.script_path,
                args.alias,
            ))
        }
        "ensure_agent_workspace" => encode(commands::ensure_agent_workspace(app)),
        "create_codex_task" | "create_claude_task" => {
            let args: AgentTaskArgs = decode(args)?;
            if command == "create_codex_task" {
                encode(commands::create_codex_task(
                    app,
                    args.name,
                    args.prompt,
                    args.base_dir,
                ))
            } else {
                encode(commands::create_claude_task(
                    app,
                    args.name,
                    args.prompt,
                    args.base_dir,
                ))
            }
        }
        "create_schedule" => {
            let args: CreateScheduleArgs = decode(args)?;
            encode(commands::create_schedule(
                app,
                args.script_path,
                args.base_dir,
                args.cron_expr,
                args.timezone,
                args.args,
                args.env,
                args.one_shot,
            ))
        }
        "list_schedules" => encode(commands::list_schedules(app)),
        "update_schedule" => {
            let args: UpdateScheduleArgs = decode(args)?;
            encode(commands::update_schedule(
                app,
                args.id,
                args.cron_expr,
                args.timezone,
                args.args,
                args.env,
                args.one_shot,
            ))
        }
        "set_schedule_enabled" => {
            let args: ScheduleEnabledArgs = decode(args)?;
            encode(commands::set_schedule_enabled(app, args.id, args.enabled))
        }
        "delete_schedule" => {
            let args: IdArgs = decode(args)?;
            encode(commands::delete_schedule(app, args.id))
        }
        "run_now" => {
            let args: RunNowArgs = decode(args)?;
            encode(commands::run_now(app, args.script_path, args.base_dir, args.args).await)
        }
        "list_jobs" => {
            let args: LimitArgs = decode(args)?;
            encode(commands::list_jobs(app, args.limit))
        }
        "list_jobs_for_script" => {
            let args: ScriptJobsArgs = decode(args)?;
            encode(commands::list_jobs_for_script(
                app,
                args.script_path,
                args.base_dir,
                args.limit,
            ))
        }
        "list_running_jobs" => encode(commands::list_running_jobs(app)),
        "get_job" => {
            let args: IdArgs = decode(args)?;
            encode(commands::get_job(app, args.id))
        }
        "cancel_job" => {
            let args: IdArgs = decode(args)?;
            encode(commands::cancel_job(app, args.id))
        }
        "cleanup_old_jobs" => {
            let args: CleanupArgs = decode(args)?;
            encode(commands::cleanup_old_jobs(app, args.days))
        }
        "cli_status" => encode(Ok::<_, String>(commands::cli_status())),
        "install_cli" => {
            let args: InstallArgs = decode(args)?;
            encode(commands::install_cli(args.force))
        }
        "resolved_path" => encode(Ok::<_, String>(commands::resolved_path())),
        "scheduler_mode" => encode(Ok::<_, String>(
            state.scheduler_active.load(Ordering::Acquire),
        )),
        "validate_cron" => {
            let args: CronArgs = decode(args)?;
            encode(commands::validate_cron(args.cron_expr))
        }
        "upcoming_runs" => {
            let args: UpcomingArgs = decode(args)?;
            encode(commands::upcoming_runs(
                args.cron_expr,
                args.timezone,
                args.count,
            ))
        }
        "detect_args" => {
            let args: ScriptLocationArgs = decode(args)?;
            encode(commands::detect_args(args.script_path, args.base_dir).await)
        }
        _ => Err(format!("unknown API command: {command}")),
    }
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("invalid request: {e}"))
}

fn encode<T: Serialize>(result: Result<T, String>) -> Result<Value, String> {
    result.and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string()))
}

async fn asset(uri: Uri) -> Response {
    let requested = uri.path().trim_start_matches('/');
    let requested = if requested.is_empty() {
        "index.html"
    } else {
        requested
    };
    let (asset_path, asset) = match WebAssets::get(requested) {
        Some(asset) => (requested, asset),
        None => match WebAssets::get("index.html") {
            Some(asset) => ("index.html", asset),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };

    let mime = mime_guess::from_path(asset_path).first_or_octet_stream();
    let cache = if asset_path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };
    let mut response = asset.data.into_owned().into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

fn open_browser(url: &str) {
    let command = if cfg!(target_os = "macos") {
        Some(("open", vec![url]))
    } else if cfg!(target_os = "windows") {
        Some(("cmd", vec!["/C", "start", "", url]))
    } else {
        Some(("xdg-open", vec![url]))
    };

    if let Some((program, args)) = command {
        let _ = std::process::Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

async fn wait_for_shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SelectedScriptsArgs {
    base_dir: String,
    script_paths: Vec<String>,
}

#[derive(Deserialize)]
struct IdArgs {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptAliasArgs {
    base_dir: String,
    script_path: String,
    alias: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentTaskArgs {
    name: String,
    prompt: String,
    base_dir: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateScheduleArgs {
    script_path: String,
    base_dir: String,
    cron_expr: String,
    timezone: String,
    args: String,
    env: String,
    one_shot: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateScheduleArgs {
    id: String,
    cron_expr: Option<String>,
    timezone: Option<String>,
    args: Option<String>,
    env: Option<String>,
    one_shot: Option<bool>,
}

#[derive(Deserialize)]
struct ScheduleEnabledArgs {
    id: String,
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunNowArgs {
    script_path: String,
    base_dir: String,
    args: String,
}

#[derive(Deserialize)]
struct LimitArgs {
    limit: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptJobsArgs {
    script_path: String,
    base_dir: String,
    limit: u32,
}

#[derive(Deserialize)]
struct CleanupArgs {
    days: u32,
}

#[derive(Deserialize)]
struct InstallArgs {
    force: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CronArgs {
    cron_expr: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpcomingArgs {
    cron_expr: String,
    timezone: String,
    count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptLocationArgs {
    script_path: String,
    base_dir: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dashboard_command_is_available_over_dispatch() {
        let dir = std::env::temp_dir().join(format!("cronbox-web-test-{}", uuid::Uuid::new_v4()));
        let app = AppState::new(dir.join("cronbox.db")).expect("create app state");
        let state = WebState {
            app: Arc::new(app),
            scheduler_active: Arc::new(AtomicBool::new(true)),
        };

        let value = dispatch(&state, "dashboard_stats", serde_json::json!({}))
            .await
            .expect("dashboard response");
        assert_eq!(value["script_total"], 0);
        assert_eq!(value["schedule_total"], 0);

        drop(state);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn unknown_command_is_rejected() {
        let dir = std::env::temp_dir().join(format!("cronbox-web-test-{}", uuid::Uuid::new_v4()));
        let app = AppState::new(dir.join("cronbox.db")).expect("create app state");
        let state = WebState {
            app: Arc::new(app),
            scheduler_active: Arc::new(AtomicBool::new(false)),
        };

        let error = dispatch(&state, "missing", serde_json::json!({}))
            .await
            .expect_err("unknown commands must fail");
        assert!(error.contains("unknown API command"));

        drop(state);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standby_scheduler_takes_over_after_lock_is_released() {
        let dir = std::env::temp_dir().join(format!("cronbox-web-test-{}", uuid::Uuid::new_v4()));
        let lock_path = dir.join("scheduler.lock");
        let first = crate::scheduler_lock::try_acquire(&lock_path)
            .expect("acquire initial lock")
            .expect("initial lease");
        let app = Arc::new(AppState::new(dir.join("cronbox.db")).expect("create app state"));
        let active = Arc::new(AtomicBool::new(false));
        let cancel = CancellationToken::new();
        let task = tokio::spawn(scheduler_supervisor(
            app.clone(),
            None,
            active.clone(),
            cancel.clone(),
        ));

        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        assert!(!active.load(Ordering::Acquire));
        drop(first);

        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while !active.load(Ordering::Acquire) {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("standby scheduler should acquire the released lock");

        cancel.cancel();
        task.await.expect("supervisor exits");
        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }
}

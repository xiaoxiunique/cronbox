use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::middleware::Next;
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

/// LocalStorage key the web UI uses to remember the access token.
pub const AUTH_TOKEN_STORAGE_KEY: &str = "cronbox.authToken";

#[derive(Debug, Clone)]
pub struct ServeOptions {
    pub host: IpAddr,
    pub port: u16,
    /// When set, every request (except `/api/health` and `/api/auth/login`)
    /// must carry `Authorization: Bearer <token>`.
    pub auth_token: Option<String>,
    pub open_browser: bool,
}

#[derive(Clone)]
struct WebState {
    app: Arc<AppState>,
    scheduler_active: Arc<AtomicBool>,
    auth_token: Option<String>,
}

#[derive(RustEmbed)]
#[folder = "../dist"]
struct WebAssets;

pub fn run(db_path: PathBuf, options: ServeOptions) -> Result<i32, String> {
    if !options.host.is_loopback() && options.auth_token.as_deref().is_none_or(str::is_empty) {
        return Err(
            "refusing to listen on a non-loopback address without an access token\n\
             set one with --auth-token <token> or the CRONBOX_AUTH_TOKEN environment variable"
                .to_string(),
        );
    }

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
        auth_token: options.auth_token.clone(),
    };
    let router = build_router(state);

    let address = SocketAddr::new(options.host, options.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|e| format!("cannot listen on http://{address}: {e}"))?;
    let url = format!("http://{address}");

    println!("CronBox web console is running");
    println!("  URL: {url}");
    println!("  DB:  {}", db_path.display());
    match options.auth_token.as_deref() {
        Some(_) => println!("  Auth: enabled (Authorization: Bearer <token>)"),
        None => println!("  Auth: disabled (loopback only)"),
    }
    if !starts_active {
        println!("  Scheduler: standby (waiting to acquire the scheduler lock)");
    }
    println!("  Press Ctrl-C to stop");

    if options.open_browser && options.host.is_loopback() {
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

fn build_router(state: WebState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/login", post(login))
        .route("/api/invoke/{command}", post(invoke))
        .fallback(get(asset))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_auth,
        ))
        .with_state(state)
}

/// Gate every request behind the configured bearer token. `/api/health` and
/// `/api/auth/login` stay public: health returns no data (deployment probes)
/// and login exists to exchange the token for a session. Unauthenticated
/// page requests receive a minimal built-in login page instead of the SPA.
async fn require_auth(State(state): State<WebState>, request: Request, next: Next) -> Response {
    let Some(expected) = state.auth_token.as_deref() else {
        return next.run(request).await;
    };

    let path = request.uri().path();
    if path == "/api/health" || path == "/api/auth/login" {
        return next.run(request).await;
    }

    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|token| constant_time_eq(token, expected));
    if authorized {
        return next.run(request).await;
    }

    let wants_html = request
        .headers()
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/html"));
    if wants_html {
        let mut response = (StatusCode::UNAUTHORIZED, login_page()).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"cronbox\""),
        );
        response
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "unauthorized" })),
        )
            .into_response()
    }
}

/// Constant-time string comparison to avoid leaking the token via timing.
fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

async fn login(State(state): State<WebState>, Json(body): Json<LoginArgs>) -> Response {
    let Some(expected) = state.auth_token.as_deref() else {
        return Json(serde_json::json!({ "ok": true })).into_response();
    };
    if constant_time_eq(&body.token, expected) {
        Json(serde_json::json!({ "ok": true })).into_response()
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "invalid token" })),
        )
            .into_response()
    }
}

/// Self-contained login page served to unauthenticated browsers. It submits
/// the token to `/api/auth/login` and, on success, stores it under the same
/// key the SPA uses and reloads.
fn login_page() -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>CronBox</title>
<style>
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0; min-height: 100vh; display: flex; align-items: center; justify-content: center;
    background: #f5f5f7; color: #1d1d1f;
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
  }}
  .card {{ background: rgba(255,255,255,0.86); border-radius: 18px; padding: 40px 36px; width: 340px;
    box-shadow: 0 2px 6px rgba(0,0,0,0.05), 0 18px 44px rgba(0,0,0,0.08); }}
  h1 {{ margin: 0 0 4px; font-size: 22px; letter-spacing: -0.02em; }}
  .subtitle {{ margin: 0 0 24px; color: #6e6e73; font-size: 13px; }}
  input {{ width: 100%; padding: 10px 14px; margin-bottom: 14px; font-size: 14px;
    border: 1px solid rgba(0,0,0,0.13); border-radius: 12px; outline: none; }}
  input:focus {{ border-color: #0071e3; box-shadow: 0 0 0 3px rgba(0,113,227,0.25); }}
  button {{ width: 100%; padding: 10px 0; font-size: 14px; font-weight: 600; color: #fff;
    background: #0071e3; border: 0; border-radius: 12px; cursor: pointer; }}
  button:hover {{ background: #0066cc; }}
  .error {{ margin: 12px 0 0; color: #ff3b30; font-size: 13px; min-height: 1em; }}
</style>
</head>
<body>
  <form class="card" id="login">
    <h1>CronBox</h1>
    <p class="subtitle">Sign in to the control panel</p>
    <input id="token" type="password" placeholder="Access token" autocomplete="current-password" autofocus>
    <button type="submit" id="submit">Sign in</button>
    <p class="error" id="error"></p>
  </form>
  <script>
    const form = document.getElementById('login');
    const error = document.getElementById('error');
    const submit = document.getElementById('submit');
    form.addEventListener('submit', async (event) => {{
      event.preventDefault();
      const token = document.getElementById('token').value.trim();
      if (!token) return;
      error.textContent = '';
      submit.disabled = true;
      try {{
        const response = await fetch('/api/auth/login', {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify({{ token }}),
        }});
        if (!response.ok) {{
          error.textContent = 'Invalid access token';
          submit.disabled = false;
          return;
        }}
        localStorage.setItem('{storage_key}', token);
        location.reload();
      }} catch {{
        error.textContent = 'Network error';
        submit.disabled = false;
      }}
    }});
  </script>
</body>
</html>
"#,
        storage_key = AUTH_TOKEN_STORAGE_KEY
    )
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
struct LoginArgs {
    token: String,
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
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_state(auth_token: Option<&str>) -> WebState {
        let dir = std::env::temp_dir().join(format!("cronbox-web-test-{}", uuid::Uuid::new_v4()));
        let app = AppState::new(dir.join("cronbox.db")).expect("create app state");
        WebState {
            app: Arc::new(app),
            scheduler_active: Arc::new(AtomicBool::new(true)),
            auth_token: auth_token.map(|t| t.to_string()),
        }
    }

    fn cleanup(state: &WebState) {
        if let Some(parent) = state.app.db_path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    async fn api_request(router: Router, token: Option<&str>) -> Response {
        let mut builder = Request::builder()
            .uri("/api/invoke/dashboard_stats")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        router
            .oneshot(builder.body(Body::from("{}")).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn dashboard_command_is_available_over_dispatch() {
        let state = test_state(None);
        let value = dispatch(&state, "dashboard_stats", serde_json::json!({}))
            .await
            .expect("dashboard response");
        assert_eq!(value["script_total"], 0);
        assert_eq!(value["schedule_total"], 0);
        cleanup(&state);
    }

    #[tokio::test]
    async fn unknown_command_is_rejected() {
        let state = test_state(None);
        let error = dispatch(&state, "missing", serde_json::json!({}))
            .await
            .expect_err("unknown commands must fail");
        assert!(error.contains("unknown API command"));
        cleanup(&state);
    }

    #[tokio::test]
    async fn auth_disabled_accepts_everything() {
        let state = test_state(None);
        let response = api_request(build_router(state.clone()), None).await;
        assert_eq!(response.status(), StatusCode::OK);
        cleanup(&state);
    }

    #[tokio::test]
    async fn auth_requires_bearer_token() {
        let state = test_state(Some("secret-token"));
        let router = build_router(state.clone());

        let response = api_request(router.clone(), None).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = api_request(router.clone(), Some("wrong-token")).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = api_request(router.clone(), Some("secret-token")).await;
        assert_eq!(response.status(), StatusCode::OK);
        cleanup(&state);
    }

    #[tokio::test]
    async fn auth_returns_login_page_for_html_requests() {
        let state = test_state(Some("secret-token"));
        let router = build_router(state.clone());
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/")
                    .method("GET")
                    .header(header::ACCEPT, "text/html")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
        let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("Sign in to the control panel"));
        assert!(html.contains("/api/auth/login"));
        cleanup(&state);
    }

    #[tokio::test]
    async fn auth_login_endpoint_accepts_valid_token() {
        let state = test_state(Some("secret-token"));
        let router = build_router(state.clone());

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/auth/login")
                    .method("POST")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"token":"secret-token"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/auth/login")
                    .method("POST")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"token":"nope"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        cleanup(&state);
    }

    #[tokio::test]
    async fn health_stays_public_when_auth_enabled() {
        let state = test_state(Some("secret-token"));
        let router = build_router(state.clone());
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        cleanup(&state);
    }

    #[test]
    fn constant_time_eq_compares_secretly() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(!constant_time_eq("", "a"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn run_refuses_non_loopback_without_token() {
        let dir = std::env::temp_dir().join(format!("cronbox-web-test-{}", uuid::Uuid::new_v4()));
        let result = run(
            dir.join("cronbox.db"),
            ServeOptions {
                host: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                port: 0,
                auth_token: None,
                open_browser: false,
            },
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("non-loopback"));
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

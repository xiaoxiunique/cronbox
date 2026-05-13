pub mod cli;
pub mod commands;
pub mod db;
pub mod executor;
pub mod models;
pub mod scheduler;
pub mod state;
pub mod tray;

use state::AppState;
use tauri::Manager;

pub fn run() {
    let db_path = cli::default_db_path();

    let app_state = AppState::new(db_path.clone()).expect("Failed to initialize CronBox engine");

    let db = app_state.db.clone();
    let db_p = app_state.db_path.clone();

    // Clone for tray refresh loop
    let tray_db = app_state.db.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(move |app| {
            configure_menu_bar_runtime(app);

            // Set up system tray
            let _ = tray::setup_tray(app.handle());

            // Best-effort CLI install. Do not force-replace an existing command on startup.
            tauri::async_runtime::spawn_blocking(|| {
                if let Err(err) = cli::install_cli_link(None, false) {
                    eprintln!("cronbox cli auto-install skipped: {err}");
                }
            });

            // Spawn scheduler loop
            let cancel = tokio_util::sync::CancellationToken::new();
            tauri::async_runtime::spawn(async move {
                scheduler::run_scheduler(db, db_p, cancel).await;
            });

            // Spawn tray refresh loop (every 2 seconds)
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    tray::refresh_tray(&app_handle, &tray_db);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                    hide_menu_bar_app(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::dashboard_stats,
            commands::list_work_dirs,
            commands::add_work_dir,
            commands::add_work_dir_with_scan,
            commands::preview_script_entries,
            commands::add_selected_scripts,
            commands::add_script_file,
            commands::remove_work_dir,
            commands::scan_scripts,
            commands::set_script_alias,
            commands::ensure_agent_workspace,
            commands::create_codex_task,
            commands::create_claude_task,
            commands::create_schedule,
            commands::list_schedules,
            commands::update_schedule,
            commands::set_schedule_enabled,
            commands::delete_schedule,
            commands::run_now,
            commands::list_jobs,
            commands::list_jobs_for_script,
            commands::list_running_jobs,
            commands::get_job,
            commands::cancel_job,
            commands::cleanup_old_jobs,
            commands::cli_status,
            commands::install_cli,
            commands::validate_cron,
            commands::upcoming_runs,
            commands::detect_args,
        ])
        .build(tauri::generate_context!())
        .expect("error while building CronBox")
        .run(|_, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

fn configure_menu_bar_runtime(app: &mut tauri::App) {
    #[cfg(target_os = "macos")]
    {
        app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        app.set_dock_visibility(false);
    }

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_skip_taskbar(true);
    }
}

pub(crate) fn hide_menu_bar_app(_app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        if let Err(err) = _app.set_activation_policy(tauri::ActivationPolicy::Accessory) {
            eprintln!("cronbox hide: failed to set activation policy: {err}");
        }
        let _ = _app.set_dock_visibility(false);
    }
}

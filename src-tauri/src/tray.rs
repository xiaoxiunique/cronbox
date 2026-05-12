use std::sync::{Arc, Mutex};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

use crate::db::Database;
use crate::models::{Job, Schedule};

/// Set up the system tray icon with initial menu
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let tray_menu = build_tray_menu(app, &[], &[])?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&tray_menu)
        .tooltip("CronBox")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => {
                show_main_window(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        if let Err(err) = app.set_activation_policy(tauri::ActivationPolicy::Accessory) {
            eprintln!("cronbox tray show: failed to set activation policy: {err}");
        }
        if let Err(err) = app.set_dock_visibility(false) {
            eprintln!("cronbox tray show: failed to hide dock icon: {err}");
        }
    }

    let Some(window) = app.get_webview_window("main") else {
        eprintln!("cronbox tray show: main window not found");
        return;
    };

    if let Err(err) = window.unminimize() {
        eprintln!("cronbox tray show: failed to unminimize main window: {err}");
    }
    if let Err(err) = window.show() {
        eprintln!("cronbox tray show: failed to show main window: {err}");
    }
    if let Err(err) = window.set_always_on_top(true) {
        eprintln!("cronbox tray show: failed to raise main window: {err}");
    }
    if let Err(err) = window.set_always_on_top(false) {
        eprintln!("cronbox tray show: failed to restore main window z-order: {err}");
    }
    if let Err(err) = window.set_focus() {
        eprintln!("cronbox tray show: failed to focus main window: {err}");
    }
}

/// Rebuild the tray menu showing scheduled scripts and running jobs.
pub fn refresh_tray(app: &AppHandle, db: &Arc<Mutex<Database>>) {
    let (schedules, running) = db
        .lock()
        .ok()
        .map(|db| {
            (
                db.list_schedules().unwrap_or_default(),
                db.list_running_jobs().unwrap_or_default(),
            )
        })
        .unwrap_or_default();

    if let Ok(menu) = build_tray_menu(app, &schedules, &running) {
        // Update all tray icons with new menu
        for tray in app.tray_by_id("main").into_iter() {
            let _ = tray.set_menu(Some(menu.clone()));
        }
        // If there's a default tray (first one)
        if let Some(trays) = app.try_state::<TrayState>() {
            // Already handled above
            let _ = trays;
        }
    }
}

// Dummy type so the compiler doesn't complain — we use tray_by_id instead
struct TrayState;

fn build_tray_menu(
    app: &AppHandle,
    schedules: &[Schedule],
    running_jobs: &[Job],
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let mut builder = MenuBuilder::new(app);

    if schedules.is_empty() {
        let idle = MenuItemBuilder::with_id("no_schedules", "No scheduled scripts")
            .enabled(false)
            .build(app)?;
        builder = builder.item(&idle);
    } else {
        let header = MenuItemBuilder::with_id(
            "schedule_header",
            format!("Scheduled ({}):", schedules.len()),
        )
        .enabled(false)
        .build(app)?;
        builder = builder.item(&header);

        for (i, schedule) in schedules.iter().enumerate() {
            let status = if schedule.enabled { "✓" } else { "⏸" };
            let item = MenuItemBuilder::with_id(
                format!("schedule_{i}"),
                format!(
                    "  {status} {} · {}",
                    schedule.script_path, schedule.cron_expr
                ),
            )
            .enabled(false)
            .build(app)?;
            builder = builder.item(&item);
        }
    }

    builder = builder.separator();

    if running_jobs.is_empty() {
        let idle = MenuItemBuilder::with_id("no_running_jobs", "No running jobs")
            .enabled(false)
            .build(app)?;
        builder = builder.item(&idle);
    } else {
        let header = MenuItemBuilder::with_id(
            "running_header",
            format!("Running ({}):", running_jobs.len()),
        )
        .enabled(false)
        .build(app)?;
        builder = builder.item(&header);

        for (i, job) in running_jobs.iter().enumerate() {
            let item =
                MenuItemBuilder::with_id(format!("job_{i}"), format!("  🔄 {}", job.script_path))
                    .enabled(false)
                    .build(app)?;
            builder = builder.item(&item);
        }
    }

    builder = builder.separator();

    let show = MenuItemBuilder::with_id("show", "Show CronBox").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    builder = builder.items(&[&show, &quit]);

    Ok(builder.build()?)
}

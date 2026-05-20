use chrono::Utc;
use chrono_tz::Tz;
use croner::Cron;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing;

use crate::db::Database;
use crate::executor;
use crate::models::{Schedule, ScriptLanguage};

pub fn calculate_next_run(cron_expr: &str, timezone: &str) -> Result<String, String> {
    let tz: Tz = timezone
        .parse()
        .map_err(|_| format!("Invalid timezone: {timezone}"))?;
    let cron = Cron::new(cron_expr)
        .parse()
        .map_err(|e| format!("Invalid cron: {e}"))?;
    let now = Utc::now().with_timezone(&tz);
    let next = cron
        .find_next_occurrence(&now, false)
        .map_err(|e| format!("No next: {e}"))?;
    Ok(next.with_timezone(&Utc).to_rfc3339())
}

pub fn validate_cron(cron_expr: &str) -> Result<(), String> {
    Cron::new(cron_expr)
        .parse()
        .map_err(|e| format!("Invalid cron: {e}"))?;
    Ok(())
}

pub fn upcoming_runs(cron_expr: &str, timezone: &str, count: usize) -> Result<Vec<String>, String> {
    let tz: Tz = timezone
        .parse()
        .map_err(|_| format!("Invalid timezone: {timezone}"))?;
    let cron = Cron::new(cron_expr)
        .parse()
        .map_err(|e| format!("Invalid cron: {e}"))?;
    let now = Utc::now().with_timezone(&tz);
    let times: Vec<String> = cron
        .iter_from(now)
        .take(count)
        .map(|t| t.with_timezone(&Utc).to_rfc3339())
        .collect();
    if times.len() < count {
        return Err("Not enough upcoming occurrences".to_string());
    }
    Ok(times)
}

pub async fn run_scheduler(db: Arc<Mutex<Database>>, db_path: PathBuf, cancel: CancellationToken) {
    tracing::info!("Scheduler started");
    loop {
        tokio::select! {
            _ = cancel.cancelled() => { tracing::info!("Scheduler shutting down"); break; }
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                if let Err(e) = tick(&db, &db_path).await {
                    tracing::error!("Scheduler tick: {e}");
                }
            }
        }
    }
}

async fn tick(db: &Arc<Mutex<Database>>, db_path: &Path) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let due: Vec<Schedule> = {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.get_due_schedules(&now).map_err(|e| e.to_string())?
    };

    for schedule in due {
        let sid = schedule.id.clone();
        let scheduled_for = schedule.next_run_at.clone();
        let skipped = {
            let db = db.lock().map_err(|e| e.to_string())?;
            if db
                .has_active_job_for_schedule(&sid)
                .map_err(|e| e.to_string())?
            {
                let reason =
                    "Skipped because the previous scheduled run is still queued or running";
                db.create_skipped_job(
                    &sid,
                    &schedule.script_path,
                    &schedule.base_dir,
                    &schedule.args,
                    scheduled_for.as_deref(),
                    reason,
                )
                .map_err(|e| e.to_string())?;
                true
            } else {
                false
            }
        };

        let job_id = if skipped {
            tracing::info!(
                "Skipping schedule {} for {} because a previous run is still active",
                schedule.script_path,
                sid
            );
            None
        } else {
            let db = db.lock().map_err(|e| e.to_string())?;
            let job = db
                .create_job(
                    Some(&sid),
                    &schedule.script_path,
                    &schedule.base_dir,
                    &schedule.args,
                    scheduled_for.as_deref(),
                )
                .map_err(|e| e.to_string())?;
            Some(job.id)
        };

        match calculate_next_run(&schedule.cron_expr, &schedule.timezone) {
            Ok(next) => {
                let db = db.lock().map_err(|e| e.to_string())?;
                db.update_schedule_next_run(&sid, &next)
                    .map_err(|e| e.to_string())?;
                db.update_schedule_last_run(&sid, &now)
                    .map_err(|e| e.to_string())?;
            }
            Err(e) => tracing::error!("Next run calc failed for {sid}: {e}"),
        }

        // One-shot schedules disable themselves after their first trigger,
        // even when the run was a skip (the trigger still fired once). The
        // next tick won't pick this schedule up because get_due_schedules
        // filters enabled = 1.
        if schedule.one_shot {
            let db = db.lock().map_err(|e| e.to_string())?;
            if let Err(err) = db.set_schedule_enabled(&sid, false) {
                tracing::error!("disable one_shot {sid}: {err}");
            }
        }

        let db_path = db_path.to_path_buf();
        let bd = schedule.base_dir.clone();
        let sp = schedule.script_path.clone();
        let a = schedule.args.clone();
        let e = schedule.env.clone();
        if let Some(job_id) = job_id {
            tokio::spawn(async move {
                execute_job(&db_path, &bd, &job_id, &sp, &a, &e).await;
            });
        }
    }
    Ok(())
}

async fn execute_job(
    db_path: &Path,
    base_dir: &str,
    job_id: &str,
    script_path: &str,
    args: &str,
    env: &str,
) {
    let db = match Database::open(db_path) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("DB error: {e}");
            return;
        }
    };
    let _ = db.mark_job_running(job_id);

    let full_path = PathBuf::from(base_dir).join(script_path);
    let language = match ScriptLanguage::from_extension(script_path) {
        Some(l) => l,
        None => {
            let _ = db.mark_job_completed(
                job_id,
                false,
                None,
                "",
                Some(&format!("Unknown type: {script_path}")),
                0,
            );
            return;
        }
    };

    if !full_path.exists() {
        let _ = db.mark_job_completed(
            job_id,
            false,
            None,
            "",
            Some(&format!("Not found: {}", full_path.display())),
            0,
        );
        return;
    }

    tracing::info!("Job {job_id}: {script_path} ({language})");
    let stream_db_path = db_path.to_path_buf();
    let stream_job_id = job_id.to_string();
    let log_callback: executor::LogCallback = Arc::new(move |chunk| {
        if let Ok(db) = Database::open(&stream_db_path) {
            let _ = db.append_job_logs(&stream_job_id, chunk);
        }
    });
    let result =
        executor::execute_with_log_callback(&full_path, language, args, env, Some(log_callback))
            .await;

    let success = result.exit_code == 0;
    let logs = db
        .get_job(job_id)
        .map(|job| job.logs)
        .unwrap_or_else(|_| combined_logs(&result));
    let error = if success {
        None
    } else {
        Some(result.stderr.lines().last().unwrap_or("Error").to_string())
    };

    let _ = db.mark_job_completed(
        job_id,
        success,
        result.result.as_deref(),
        &logs,
        error.as_deref(),
        result.duration_ms,
    );

    // Surface scheduled runs through a system notification — the user is not
    // watching the app, and silent runs leave them guessing whether the job
    // fired at all. Manual `run_now` / CLI `cronbox run` paths don't go
    // through here, so ad-hoc tests aren't notified.
    let alias = db
        .get_script_alias(base_dir, script_path)
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            std::path::Path::new(script_path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| script_path.to_string())
        });
    let body = if success {
        format!("Completed in {}", fmt_duration_ms(result.duration_ms))
    } else {
        let msg = error.as_deref().unwrap_or("Failed").replace('\n', " ");
        format!("Failed: {msg}")
    };
    notify_job_completion(&alias, &body);
}

fn fmt_duration_ms(ms: i64) -> String {
    if ms < 1000 {
        return format!("{ms}ms");
    }
    let secs = ms / 1000;
    if secs < 60 {
        return format!("{secs}s");
    }
    let m = secs / 60;
    let r = secs % 60;
    if r == 0 {
        format!("{m}m")
    } else {
        format!("{m}m{r}s")
    }
}

#[cfg(target_os = "macos")]
fn notify_job_completion(alias: &str, body: &str) {
    let title = format!("CronBox · {alias}");
    let script = format!(
        r#"display notification "{}" with title "{}" sound name "default""#,
        escape_applescript(body),
        escape_applescript(&title),
    );
    let _ = std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn();
}

#[cfg(not(target_os = "macos"))]
fn notify_job_completion(_alias: &str, _body: &str) {}

#[cfg(target_os = "macos")]
fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn combined_logs(result: &crate::models::ExecutionResult) -> String {
    if result.stderr.is_empty() {
        result.stdout.clone()
    } else {
        format!("{}\n--- stderr ---\n{}", result.stdout, result.stderr)
    }
}

pub fn initialize_schedule_times(db: &Database) -> Result<(), String> {
    let schedules = db.list_schedules().map_err(|e| e.to_string())?;
    for s in schedules {
        if s.enabled && s.next_run_at.is_none() {
            if let Ok(next) = calculate_next_run(&s.cron_expr, &s.timezone) {
                db.update_schedule_next_run(&s.id, &next)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cron_basics() {
        assert!(validate_cron("* * * * *").is_ok());
        assert!(validate_cron("invalid").is_err());
        let next = calculate_next_run("0 * * * *", "UTC").unwrap();
        assert!(!next.is_empty());
        let runs = upcoming_runs("0 * * * *", "UTC", 3).unwrap();
        assert_eq!(runs.len(), 3);
    }
}

use rusqlite::{params, Connection, Result as SqlResult};
use std::path::Path;

use crate::models::*;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS work_dirs (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS script_aliases (
    base_dir TEXT NOT NULL,
    script_path TEXT NOT NULL,
    alias TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (base_dir, script_path)
);

CREATE TABLE IF NOT EXISTS schedules (
    id TEXT PRIMARY KEY,
    script_path TEXT NOT NULL,
    base_dir TEXT NOT NULL,
    cron_expr TEXT NOT NULL,
    timezone TEXT NOT NULL DEFAULT 'Asia/Shanghai',
    args TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1,
    next_run_at TEXT,
    last_run_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    schedule_id TEXT REFERENCES schedules(id) ON DELETE SET NULL,
    script_path TEXT NOT NULL,
    base_dir TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'queued',
    args TEXT NOT NULL DEFAULT '{}',
    result TEXT,
    logs TEXT NOT NULL DEFAULT '',
    error TEXT,
    scheduled_for TEXT,
    started_at TEXT,
    completed_at TEXT,
    duration_ms INTEGER,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
CREATE INDEX IF NOT EXISTS idx_jobs_schedule ON jobs(schedule_id);
CREATE INDEX IF NOT EXISTS idx_jobs_created ON jobs(created_at);
CREATE INDEX IF NOT EXISTS idx_jobs_script ON jobs(base_dir, script_path, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_schedules_next ON schedules(enabled, next_run_at);
"#;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> SqlResult<()> {
        // Drop old tables without base_dir / work_dirs
        let has_work_dirs: bool = self
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='work_dirs'",
                [],
                |r| r.get::<_, i32>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_work_dirs {
            let _ = self.conn.execute_batch("DROP TABLE IF EXISTS jobs; DROP TABLE IF EXISTS schedules; DROP TABLE IF EXISTS scripts;");
        }
        self.conn.execute_batch(SCHEMA)
    }

    // ── Work Dirs ──

    pub fn add_work_dir(&self, path: &str) -> SqlResult<WorkDir> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR IGNORE INTO work_dirs (id, path, created_at) VALUES (?1, ?2, ?3)",
            params![id, path, now],
        )?;
        self.conn.query_row(
            "SELECT id, path, created_at FROM work_dirs WHERE path = ?1",
            params![path],
            |row| {
                Ok(WorkDir {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    created_at: row.get(2)?,
                })
            },
        )
    }

    pub fn list_work_dirs(&self) -> SqlResult<Vec<WorkDir>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, created_at FROM work_dirs ORDER BY created_at")?;
        let rows = stmt.query_map([], |row| {
            Ok(WorkDir {
                id: row.get(0)?,
                path: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    pub fn remove_work_dir(&self, id: &str) -> SqlResult<bool> {
        let count = self
            .conn
            .execute("DELETE FROM work_dirs WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    // ── Script Aliases ──

    pub fn set_script_alias(
        &self,
        base_dir: &str,
        script_path: &str,
        alias: Option<&str>,
    ) -> SqlResult<()> {
        let cleaned = alias.map(str::trim).filter(|value| !value.is_empty());
        if let Some(alias) = cleaned {
            let now = chrono::Utc::now().to_rfc3339();
            self.conn.execute(
                "INSERT INTO script_aliases (base_dir, script_path, alias, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(base_dir, script_path)
                 DO UPDATE SET alias = excluded.alias, updated_at = excluded.updated_at",
                params![base_dir, script_path, alias, now],
            )?;
        } else {
            self.conn.execute(
                "DELETE FROM script_aliases WHERE base_dir = ?1 AND script_path = ?2",
                params![base_dir, script_path],
            )?;
        }
        Ok(())
    }

    pub fn get_script_alias(&self, base_dir: &str, script_path: &str) -> SqlResult<Option<String>> {
        let result = self.conn.query_row(
            "SELECT alias FROM script_aliases WHERE base_dir = ?1 AND script_path = ?2",
            params![base_dir, script_path],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(alias) => Ok(Some(alias)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn list_script_aliases(&self) -> SqlResult<Vec<ScriptAlias>> {
        let mut stmt = self.conn.prepare(
            "SELECT base_dir, script_path, alias, updated_at FROM script_aliases ORDER BY base_dir, script_path",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ScriptAlias {
                base_dir: row.get(0)?,
                script_path: row.get(1)?,
                alias: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    // ── Schedule CRUD ──

    pub fn create_schedule(
        &self,
        script_path: &str,
        base_dir: &str,
        cron_expr: &str,
        timezone: &str,
        args: &str,
    ) -> SqlResult<Schedule> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO schedules (id, script_path, base_dir, cron_expr, timezone, args, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)",
            params![id, script_path, base_dir, cron_expr, timezone, args, now, now],
        )?;
        self.get_schedule(&id)
    }

    pub fn get_schedule(&self, id: &str) -> SqlResult<Schedule> {
        self.conn.query_row(
            "SELECT id, script_path, base_dir, cron_expr, timezone, args, enabled, next_run_at, last_run_at, created_at, updated_at FROM schedules WHERE id = ?1",
            params![id],
            |row| row_to_schedule(row),
        )
    }

    pub fn list_schedules(&self) -> SqlResult<Vec<Schedule>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, script_path, base_dir, cron_expr, timezone, args, enabled, next_run_at, last_run_at, created_at, updated_at FROM schedules ORDER BY script_path",
        )?;
        let rows = stmt.query_map([], |row| row_to_schedule(row))?;
        rows.collect()
    }

    pub fn update_schedule(
        &self,
        id: &str,
        cron_expr: Option<&str>,
        timezone: Option<&str>,
        args: Option<&str>,
    ) -> SqlResult<Schedule> {
        let now = chrono::Utc::now().to_rfc3339();
        let existing = self.get_schedule(id)?;
        let cron_expr = cron_expr.unwrap_or(&existing.cron_expr);
        let timezone = timezone.unwrap_or(&existing.timezone);
        let args = args.unwrap_or(&existing.args);
        self.conn.execute(
            "UPDATE schedules SET cron_expr = ?1, timezone = ?2, args = ?3, updated_at = ?4 WHERE id = ?5",
            params![cron_expr, timezone, args, now, id],
        )?;
        self.get_schedule(id)
    }

    pub fn set_schedule_enabled(&self, id: &str, enabled: bool) -> SqlResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE schedules SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![enabled as i32, now, id],
        )?;
        Ok(())
    }

    pub fn update_schedule_next_run(&self, id: &str, next_run_at: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE schedules SET next_run_at = ?1 WHERE id = ?2",
            params![next_run_at, id],
        )?;
        Ok(())
    }

    pub fn update_schedule_last_run(&self, id: &str, last_run_at: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE schedules SET last_run_at = ?1 WHERE id = ?2",
            params![last_run_at, id],
        )?;
        Ok(())
    }

    pub fn delete_schedule(&self, id: &str) -> SqlResult<bool> {
        let count = self
            .conn
            .execute("DELETE FROM schedules WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    pub fn get_due_schedules(&self, now: &str) -> SqlResult<Vec<Schedule>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, script_path, base_dir, cron_expr, timezone, args, enabled, next_run_at, last_run_at, created_at, updated_at
             FROM schedules WHERE enabled = 1 AND next_run_at IS NOT NULL AND next_run_at <= ?1",
        )?;
        let rows = stmt.query_map(params![now], |row| row_to_schedule(row))?;
        rows.collect()
    }

    pub fn has_active_job_for_schedule(&self, schedule_id: &str) -> SqlResult<bool> {
        self.conn.query_row(
            "SELECT EXISTS (
                SELECT 1 FROM jobs
                WHERE schedule_id = ?1 AND status IN ('queued', 'running')
            )",
            params![schedule_id],
            |row| row.get::<_, bool>(0),
        )
    }

    // ── Job CRUD ──

    pub fn create_job(
        &self,
        schedule_id: Option<&str>,
        script_path: &str,
        base_dir: &str,
        args: &str,
        scheduled_for: Option<&str>,
    ) -> SqlResult<Job> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO jobs (id, schedule_id, script_path, base_dir, status, args, logs, scheduled_for, created_at)
             VALUES (?1, ?2, ?3, ?4, 'queued', ?5, '', ?6, ?7)",
            params![id, schedule_id, script_path, base_dir, args, scheduled_for, now],
        )?;
        self.get_job(&id)
    }

    pub fn create_skipped_job(
        &self,
        schedule_id: &str,
        script_path: &str,
        base_dir: &str,
        args: &str,
        scheduled_for: Option<&str>,
        reason: &str,
    ) -> SqlResult<Job> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO jobs (
                id, schedule_id, script_path, base_dir, status, args, logs, error,
                scheduled_for, completed_at, duration_ms, created_at
             )
             VALUES (?1, ?2, ?3, ?4, 'skipped', ?5, ?6, ?7, ?8, ?9, 0, ?10)",
            params![
                id,
                schedule_id,
                script_path,
                base_dir,
                args,
                reason,
                reason,
                scheduled_for,
                now,
                now
            ],
        )?;
        self.get_job(&id)
    }

    pub fn get_job(&self, id: &str) -> SqlResult<Job> {
        self.conn.query_row(
            "SELECT id, schedule_id, script_path, base_dir, status, args, result, logs, error, scheduled_for, started_at, completed_at, duration_ms, created_at FROM jobs WHERE id = ?1",
            params![id],
            |row| row_to_job(row),
        )
    }

    pub fn list_jobs(&self, limit: u32) -> SqlResult<Vec<Job>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, schedule_id, script_path, base_dir, status, args, result, logs, error, scheduled_for, started_at, completed_at, duration_ms, created_at
             FROM jobs ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| row_to_job(row))?;
        rows.collect()
    }

    pub fn list_jobs_for_script(
        &self,
        script_path: &str,
        base_dir: &str,
        limit: u32,
    ) -> SqlResult<Vec<Job>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, schedule_id, script_path, base_dir, status, args, result, logs, error, scheduled_for, started_at, completed_at, duration_ms, created_at
             FROM jobs WHERE script_path = ?1 AND base_dir = ?2 ORDER BY created_at DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![script_path, base_dir, limit], |row| row_to_job(row))?;
        rows.collect()
    }

    pub fn list_running_jobs(&self) -> SqlResult<Vec<Job>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, schedule_id, script_path, base_dir, status, args, result, logs, error, scheduled_for, started_at, completed_at, duration_ms, created_at
             FROM jobs WHERE status IN ('queued', 'running') ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| row_to_job(row))?;
        rows.collect()
    }

    pub fn mark_job_running(&self, id: &str) -> SqlResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE jobs SET status = 'running', started_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn append_job_logs(&self, id: &str, chunk: &str) -> SqlResult<()> {
        if chunk.is_empty() {
            return Ok(());
        }
        self.conn.execute(
            "UPDATE jobs SET logs = logs || ?1 WHERE id = ?2",
            params![chunk, id],
        )?;
        Ok(())
    }

    pub fn mark_job_completed(
        &self,
        id: &str,
        success: bool,
        result: Option<&str>,
        logs: &str,
        error: Option<&str>,
        duration_ms: i64,
    ) -> SqlResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let status = if success { "success" } else { "failure" };
        self.conn.execute(
            "UPDATE jobs SET status = ?1, result = ?2, logs = ?3, error = ?4, completed_at = ?5, duration_ms = ?6 WHERE id = ?7",
            params![status, result, logs, error, now, duration_ms, id],
        )?;
        Ok(())
    }

    pub fn cancel_job(&self, id: &str) -> SqlResult<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let count = self.conn.execute(
            "UPDATE jobs SET status = 'cancelled', completed_at = ?1 WHERE id = ?2 AND status IN ('queued', 'running')",
            params![now, id],
        )?;
        Ok(count > 0)
    }

    pub fn cleanup_old_jobs(&self, days: u32) -> SqlResult<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let count = self.conn.execute(
            "DELETE FROM jobs WHERE created_at < ?1 AND status IN ('success', 'failure', 'cancelled', 'skipped')",
            params![cutoff.to_rfc3339()],
        )?;
        Ok(count)
    }
}

fn row_to_schedule(row: &rusqlite::Row) -> SqlResult<Schedule> {
    Ok(Schedule {
        id: row.get(0)?,
        script_path: row.get(1)?,
        base_dir: row.get(2)?,
        cron_expr: row.get(3)?,
        timezone: row.get(4)?,
        args: row.get(5)?,
        enabled: row.get::<_, i32>(6)? != 0,
        next_run_at: row.get(7)?,
        last_run_at: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn row_to_job(row: &rusqlite::Row) -> SqlResult<Job> {
    let status_str: String = row.get(4)?;
    Ok(Job {
        id: row.get(0)?,
        schedule_id: row.get(1)?,
        script_path: row.get(2)?,
        base_dir: row.get(3)?,
        status: JobStatus::from_str(&status_str).unwrap_or(JobStatus::Queued),
        args: row.get(5)?,
        result: row.get(6)?,
        logs: row.get(7)?,
        error: row.get(8)?,
        scheduled_for: row.get(9)?,
        started_at: row.get(10)?,
        completed_at: row.get(11)?,
        duration_ms: row.get(12)?,
        created_at: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_dirs() {
        let db = Database::open_in_memory().unwrap();
        let wd = db.add_work_dir("/tmp/scripts").unwrap();
        assert_eq!(wd.path, "/tmp/scripts");
        let dirs = db.list_work_dirs().unwrap();
        assert_eq!(dirs.len(), 1);
        // Adding same path again should not duplicate
        db.add_work_dir("/tmp/scripts").unwrap();
        assert_eq!(db.list_work_dirs().unwrap().len(), 1);
        db.remove_work_dir(&wd.id).unwrap();
        assert!(db.list_work_dirs().unwrap().is_empty());
    }

    #[test]
    fn test_script_aliases() {
        let db = Database::open_in_memory().unwrap();

        assert_eq!(
            db.get_script_alias("/tmp/scripts", "jobs/sync.sh").unwrap(),
            None
        );

        db.set_script_alias("/tmp/scripts", "jobs/sync.sh", Some("Sync chain data"))
            .unwrap();
        assert_eq!(
            db.get_script_alias("/tmp/scripts", "jobs/sync.sh")
                .unwrap()
                .as_deref(),
            Some("Sync chain data")
        );

        db.set_script_alias("/tmp/scripts", "jobs/sync.sh", Some("  Sync telemetry  "))
            .unwrap();
        let aliases = db.list_script_aliases().unwrap();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].alias, "Sync telemetry");

        db.set_script_alias("/tmp/scripts", "jobs/sync.sh", Some("  "))
            .unwrap();
        assert_eq!(
            db.get_script_alias("/tmp/scripts", "jobs/sync.sh").unwrap(),
            None
        );
        assert!(db.list_script_aliases().unwrap().is_empty());
    }

    #[test]
    fn test_schedule_with_base_dir() {
        let db = Database::open_in_memory().unwrap();
        let s = db
            .create_schedule("test.sh", "/home/user/scripts", "* * * * *", "UTC", "{}")
            .unwrap();
        assert_eq!(s.base_dir, "/home/user/scripts");
        assert_eq!(s.script_path, "test.sh");
    }

    #[test]
    fn test_job_with_base_dir() {
        let db = Database::open_in_memory().unwrap();
        let j = db
            .create_job(None, "test.sh", "/home/user/scripts", "{}", None)
            .unwrap();
        assert_eq!(j.base_dir, "/home/user/scripts");
        assert_eq!(j.status, JobStatus::Queued);
    }

    #[test]
    fn test_running_jobs() {
        let db = Database::open_in_memory().unwrap();
        let j = db.create_job(None, "test.sh", "/tmp", "{}", None).unwrap();
        db.mark_job_running(&j.id).unwrap();
        let running = db.list_running_jobs().unwrap();
        assert_eq!(running.len(), 1);
        db.mark_job_completed(&j.id, true, None, "", None, 10)
            .unwrap();
        assert!(db.list_running_jobs().unwrap().is_empty());
    }

    #[test]
    fn test_append_job_logs() {
        let db = Database::open_in_memory().unwrap();
        let job = db.create_job(None, "test.sh", "/tmp", "{}", None).unwrap();

        db.append_job_logs(&job.id, "first\n").unwrap();
        db.append_job_logs(&job.id, "second\n").unwrap();

        assert_eq!(db.get_job(&job.id).unwrap().logs, "first\nsecond\n");
    }

    #[test]
    fn test_schedule_active_job_and_skipped_job() {
        let db = Database::open_in_memory().unwrap();
        let schedule = db
            .create_schedule("test.sh", "/tmp", "* * * * *", "UTC", "{}")
            .unwrap();

        assert!(!db.has_active_job_for_schedule(&schedule.id).unwrap());

        let job = db
            .create_job(Some(&schedule.id), "test.sh", "/tmp", "{}", None)
            .unwrap();
        assert!(db.has_active_job_for_schedule(&schedule.id).unwrap());

        db.mark_job_completed(&job.id, true, None, "", None, 1)
            .unwrap();
        assert!(!db.has_active_job_for_schedule(&schedule.id).unwrap());

        let skipped = db
            .create_skipped_job(
                &schedule.id,
                "test.sh",
                "/tmp",
                "{}",
                Some("2026-05-12T00:00:00Z"),
                "Previous scheduled run still active",
            )
            .unwrap();
        assert_eq!(skipped.status, JobStatus::Skipped);
        assert!(!db.has_active_job_for_schedule(&schedule.id).unwrap());
    }

    #[test]
    fn test_list_jobs_for_script() {
        let db = Database::open_in_memory().unwrap();
        db.create_job(None, "a.sh", "/tmp/one", "{}", None).unwrap();
        db.create_job(None, "a.sh", "/tmp/two", "{}", None).unwrap();
        db.create_job(None, "b.sh", "/tmp/one", "{}", None).unwrap();

        let jobs = db.list_jobs_for_script("a.sh", "/tmp/one", 50).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].script_path, "a.sh");
        assert_eq!(jobs[0].base_dir, "/tmp/one");
    }
}

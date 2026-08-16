//! Single-active-scheduler lease.
//!
//! Both the Web console service and `cronbox daemon` can run the scheduler loop, but
//! they share one SQLite database — two loops ticking the same DB can
//! double-fire a schedule. This module grants exactly one process the right to
//! run the scheduler at a time, via an advisory lock on a sibling lock file.
//!
//! The lock is held by an open file descriptor for the lifetime of the
//! `SchedulerLease`. When the process exits (cleanly or by crash) the OS
//! releases it, so a successor can take over with no stale-state cleanup.

use std::fs::OpenOptions;
use std::path::Path;

/// Holds the active-scheduler lease until dropped (or the process exits).
pub struct SchedulerLease {
    _file: std::fs::File,
}

/// Try to become the single active scheduler.
///
/// - `Ok(Some(lease))` — acquired; the caller should run the scheduler and keep
///   the lease alive for as long as it does.
/// - `Ok(None)` — another live CronBox process already holds it (standby mode).
/// - `Err(_)` — the lock file could not be opened.
pub fn try_acquire(lock_path: &Path) -> Result<Option<SchedulerLease>, String> {
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|e| format!("cannot open scheduler lock {}: {e}", lock_path.display()))?;

    #[cfg(unix)]
    {
        use rustix::fs::{flock, FlockOperation};
        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(Some(SchedulerLease { _file: file })),
            Err(rustix::io::Errno::WOULDBLOCK) => Ok(None),
            Err(e) => Err(format!("scheduler lock failed: {e}")),
        }
    }

    // No advisory locking wired up off-unix; preserve the prior single-process
    // behaviour by always granting the lease.
    #[cfg(not(unix))]
    {
        Ok(Some(SchedulerLease { _file: file }))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn temp_lock_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cronbox-test-{}.lock", uuid::Uuid::new_v4()))
    }

    #[test]
    fn second_acquire_is_blocked_until_first_is_dropped() {
        let path = temp_lock_path();

        let first = try_acquire(&path).expect("first acquire ok");
        assert!(first.is_some(), "first caller should win the lease");

        let second = try_acquire(&path).expect("second acquire ok");
        assert!(second.is_none(), "second caller should see it held");

        drop(first);

        let third = try_acquire(&path).expect("third acquire ok");
        assert!(third.is_some(), "lease should be free again after drop");

        drop(third);
        let _ = std::fs::remove_file(&path);
    }
}

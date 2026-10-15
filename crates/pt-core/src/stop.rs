//! SIGSTOP/SIGCONT orchestration with restore on every path.
//!
//! [`StopGuard`] stops the target once and resumes it exactly once:
//! explicitly through [`StopGuard::resume`], or implicitly through
//! `Drop` on any error path, early return, or panic. A stopped process
//! can never leak out of this module.
//!
//! # Signal safety
//!
//! `SIGSTOP` cannot be caught or ignored by the target, so the stop is
//! guaranteed once `kill` returns. Delivery is asynchronous, so
//! [`StopGuard::stop`] waits (bounded) for `/proc/<pid>/status` to
//! report the stopped state before returning. `SIGCONT` resumes the
//! process; if the target died in between, `SIGCONT` is a no-op.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::error::CoreError;

/// How long [`StopGuard::stop`] waits for the target to reach the
/// stopped state.
const STOP_TIMEOUT: Duration = Duration::from_millis(2000);

/// Poll interval while waiting for the stopped state.
const POLL_INTERVAL: Duration = Duration::from_millis(2);

/// A guard that stops a process on creation and resumes it on drop.
///
/// The guard is only responsible for a process it stopped itself: if
/// the target is already stopped when [`StopGuard::stop`] is called,
/// the call fails with [`CoreError::AlreadyStopped`] instead of
/// taking over someone else's stop.
#[derive(Debug)]
pub struct StopGuard {
    pid: i32,
    stopped: bool,
}

impl StopGuard {
    /// Stops `pid` and waits for the stopped state.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::AlreadyStopped`] when the target is already
    /// stopped, [`CoreError::ProcessGone`] when it died, and
    /// [`CoreError::StopTimeout`] when it does not reach the stopped
    /// state within 2 seconds.
    pub fn stop(pid: i32) -> Result<Self, CoreError> {
        if process_state(pid)? == 'T' {
            return Err(CoreError::AlreadyStopped { pid });
        }
        send_signal(pid, libc::SIGSTOP)?;
        let deadline = Instant::now() + STOP_TIMEOUT;
        loop {
            match process_state(pid)? {
                'T' => break,
                _ if Instant::now() >= deadline => {
                    return Err(CoreError::StopTimeout {
                        pid,
                        timeout_ms: STOP_TIMEOUT.as_millis() as u64,
                    })
                }
                _ => std::thread::sleep(POLL_INTERVAL),
            }
        }
        tracing::debug!(pid, "target stopped");
        Ok(Self { pid, stopped: true })
    }

    /// Resumes the target if this guard still owns its stop state.
    ///
    /// Idempotent: calling it after a successful resume does nothing.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::ProcessGone`] when the target died.
    pub fn resume(&mut self) -> Result<(), CoreError> {
        if !self.stopped {
            return Ok(());
        }
        send_signal(self.pid, libc::SIGCONT)?;
        self.stopped = false;
        tracing::debug!(pid = self.pid, "target resumed");
        Ok(())
    }

    /// Whether the target is still stopped under this guard.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }
}

impl Drop for StopGuard {
    fn drop(&mut self) {
        // The restore path of last resort: runs on every error path,
        // early return, and panic (via unwinding). A failed SIGCONT is
        // logged but not propagated — the surrounding operation's
        // error is more useful, and the process is gone anyway if the
        // signal failed with ESRCH.
        if self.stopped {
            if send_signal(self.pid, libc::SIGCONT).is_err() {
                tracing::warn!(
                    pid = self.pid,
                    "failed to SIGCONT the target during cleanup"
                );
            } else {
                self.stopped = false;
            }
        }
    }
}

/// Sends `signal` to `pid`, mapping kernel errors to [`CoreError`].
fn send_signal(pid: i32, signal: libc::c_int) -> Result<(), CoreError> {
    // SAFETY: `kill` only sends a signal to the given pid; it has no
    // memory-safety invariants beyond the integer arguments.
    let result = unsafe { libc::kill(pid, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Err(CoreError::ProcessGone { pid }),
        _ => Err(CoreError::Io {
            path: PathBuf::from(format!("/proc/{pid}")),
            source: error,
        }),
    }
}

/// Reads the process state letter from `/proc/<pid>/status`
/// (`R`unning, `S`leeping, `T` stopped, `Z`ombie, ...).
///
/// # Errors
///
/// Returns [`CoreError::ProcessGone`] when the status file cannot be
/// read (the process exited) and [`CoreError::Io`] on any other
/// unexpected failure.
pub fn process_state(pid: i32) -> Result<char, CoreError> {
    let path = Path::new("/proc").join(pid.to_string()).join("status");
    let text = std::fs::read_to_string(&path).map_err(|source| CoreError::Io { path, source })?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("State:") {
            let state = rest.trim().chars().next();
            if let Some(state) = state {
                return Ok(state);
            }
        }
    }
    Err(CoreError::Io {
        path: PathBuf::from(format!("/proc/{pid}/status")),
        source: std::io::Error::other("no State line"),
    })
}

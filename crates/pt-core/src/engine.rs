//! The write engine: dry-run plans and verified applies.
//!
//! The engine's contract, enforced by [`apply`]'s implementation and
//! the integration tests:
//!
//! 1. Nothing is ever written without a [`WritePlan`], and a plan is
//!    only built for ranges inside one mapped writable region.
//! 2. [`plan_write`] is the *dry-run*: it reads the current bytes and
//!    performs zero writes.
//! 3. [`apply`] stops the target (SIGSTOP), re-validates the region
//!    against a fresh maps read, writes, reads back, verifies, and
//!    resumes the target (SIGCONT) — on success and on every error
//!    path, including panics, via the [`StopGuard`] drop path.

use crate::error::CoreError;
use crate::maps::{read_maps, validate_writable_range, Region};
use crate::mem::{read_bytes, write_bytes};
use crate::stop::StopGuard;

/// A prepared, validated write: everything that will happen, without
/// doing it. `proton-trainer --dry-run` prints this verbatim;
/// `--apply` executes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritePlan {
    /// Target process id.
    pub pid: i32,
    /// Address where `new_bytes` will be written.
    pub address: u64,
    /// Bytes at `address` at plan time (the pre-state).
    pub old_bytes: Vec<u8>,
    /// Bytes that would be written.
    pub new_bytes: Vec<u8>,
    /// The mapped writable region containing the whole range,
    /// re-validated at apply time.
    pub region: Region,
}

/// Builds the [`WritePlan`] for writing `new_bytes` at `address` in
/// process `pid`. Performs no writes and never stops the target.
///
/// This is the dry-run: printing the returned plan is exactly what
/// `proton-trainer --dry-run` shows.
///
/// # Errors
///
/// Returns [`CoreError::EmptyWrite`] for an empty payload,
/// [`CoreError::RegionNotWritable`] or [`CoreError::RegionSpans`] when
/// the range is not inside one mapped writable region, and the read
/// errors from [`crate::mem::read_bytes`] (e.g.
/// [`CoreError::PtraceDenied`], [`CoreError::ProcessGone`]).
pub fn plan_write(pid: i32, address: u64, new_bytes: &[u8]) -> Result<WritePlan, CoreError> {
    if new_bytes.is_empty() {
        return Err(CoreError::EmptyWrite);
    }
    let regions = read_maps(pid)?;
    let region =
        validate_writable_range(&regions, pid, address, address + new_bytes.len() as u64)?.clone();

    let mut old_bytes = vec![0u8; new_bytes.len()];
    read_bytes(pid, address, &mut old_bytes)?;

    tracing::debug!(
        pid,
        address = format!("{address:#x}"),
        len = new_bytes.len(),
        "write plan built"
    );
    Ok(WritePlan {
        pid,
        address,
        old_bytes,
        new_bytes: new_bytes.to_vec(),
        region,
    })
}

/// Applies a [`WritePlan`]: stop, re-validate, write, read back,
/// verify, resume.
///
/// # Signal safety
///
/// The target is stopped with SIGSTOP for the duration of the write
/// and read-back — a window of a few milliseconds — and always
/// resumed with SIGCONT, on success and on every error path, by
/// [`StopGuard`]'s `Drop`. We only resume a process we stopped
/// ourselves.
///
/// # Error modes
///
/// Any failure — target gone (`ESRCH`), ptrace denied (`EPERM`), region
/// re-mapped between plan and apply, or read-back mismatch — aborts
/// the apply and leaves the target running (never stopped, never
/// partially written: a single `process_vm_writev` of the whole
/// payload precedes verification, and a mismatch is reported, not
/// rolled back, because the target may legitimately react to a
/// partial write — the write itself is all-or-nothing).
///
/// # Errors
///
/// See [`StopGuard::stop`], [`crate::maps::validate_writable_range`],
/// and [`crate::mem`] for the full set.
pub fn apply(plan: &WritePlan) -> Result<(), CoreError> {
    let mut guard = StopGuard::stop(plan.pid)?;
    let result = apply_stopped(plan);
    guard.resume()?;
    result
}

/// The write itself, run while the target is stopped.
fn apply_stopped(plan: &WritePlan) -> Result<(), CoreError> {
    // Re-validate with a fresh maps read: the target may have re-mapped
    // between plan time and now.
    let regions = read_maps(plan.pid)?;
    validate_writable_range(
        &regions,
        plan.pid,
        plan.address,
        plan.address + plan.new_bytes.len() as u64,
    )?;

    write_bytes(plan.pid, plan.address, &plan.new_bytes)?;

    let mut verify = vec![0u8; plan.new_bytes.len()];
    read_bytes(plan.pid, plan.address, &mut verify)?;
    if verify != plan.new_bytes {
        return Err(CoreError::VerifyFailed {
            pid: plan.pid,
            addr: plan.address,
            expected: plan.new_bytes.clone(),
            actual: verify,
        });
    }
    tracing::debug!(
        pid = plan.pid,
        address = format!("{:#x}", plan.address),
        "write applied and verified"
    );
    Ok(())
}

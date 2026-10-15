//! Cross-process memory access via `process_vm_readv`/`process_vm_writev`.
//!
//! # Safety and error modes
//!
//! Both wrappers below contain `unsafe` blocks calling libc. The safety
//! invariants are:
//!
//! - The local buffer always points to a valid, correctly-sized Rust
//!   slice for the duration of the call.
//! - The remote address range is validated against the target's
//!   mapped regions by the caller (see [`crate::engine`]) before any
//!   write; reads of unmapped remote memory fail with `EFAULT`/`ENOMEM`
//!   in the kernel, never corrupting the target or this process.
//! - Signals: neither call sends signals to the target. They may be
//!   called while the target is stopped (SIGSTOP) — that is the
//!   intended state for writes — or running.
//!
//! Error mapping: `EPERM` becomes [`CoreError::PtraceDenied`] with an
//! actionable message, `ESRCH` becomes [`CoreError::ProcessGone`],
//! and everything else surfaces as [`CoreError::Io`] with the kernel
//! errno preserved.

use std::path::PathBuf;

use crate::error::CoreError;
use crate::process::{ptrace_advice, ptrace_scope};

/// Reads `buf.len()` bytes from `addr` in process `pid`.
///
/// Partial transfers are continued automatically: the kernel may
/// transfer fewer bytes than requested when the range crosses internal
/// boundaries; the remainder is retried until the buffer is full or the
/// process is gone.
///
/// # Errors
///
/// See the module documentation for the error mapping.
pub fn read_bytes(pid: i32, addr: u64, buf: &mut [u8]) -> Result<(), CoreError> {
    let mut done = 0usize;
    while done < buf.len() {
        let transferred = process_vm(pid, addr + done as u64, &mut buf[done..])?;
        if transferred == 0 {
            // The kernel transferred nothing but reported success,
            // which in practice means the remote page is unmapped.
            return Err(CoreError::RegionNotWritable {
                pid,
                addr: addr + done as u64,
            });
        }
        done += transferred;
    }
    Ok(())
}

/// Writes `bytes` to `addr` in process `pid`.
///
/// Callers must have validated that `[addr, addr + bytes.len())` lies
/// inside one mapped writable region of the target (see
/// [`crate::maps::validate_writable_range`]); this function does not
/// re-check by itself so that the validation happens while the target
/// is stopped.
///
/// # Errors
///
/// See the module documentation for the error mapping.
pub fn write_bytes(pid: i32, addr: u64, bytes: &[u8]) -> Result<(), CoreError> {
    let mut done = 0usize;
    while done < bytes.len() {
        let transferred = process_vm_write(pid, addr + done as u64, &bytes[done..])?;
        if transferred == 0 {
            return Err(CoreError::RegionNotWritable {
                pid,
                addr: addr + done as u64,
            });
        }
        done += transferred;
    }
    Ok(())
}

/// One `process_vm_readv` call; returns the number of bytes
/// transferred.
fn process_vm(pid: i32, addr: u64, buf: &mut [u8]) -> Result<usize, CoreError> {
    if buf.is_empty() {
        return Ok(0);
    }
    let local = libc::iovec {
        iov_base: buf.as_mut_ptr() as *mut libc::c_void,
        iov_len: buf.len(),
    };
    let remote = libc::iovec {
        iov_base: addr as usize as *mut libc::c_void,
        iov_len: buf.len(),
    };
    // SAFETY: `local` points to the valid `buf` slice for the duration
    // of the call; `remote` describes the target's address range, which
    // the kernel checks. Neither pointer outlives the call.
    let result = unsafe { libc::process_vm_readv(pid, &local, 1, &remote, 1, 0) };
    check_result(pid, result)
}

/// One `process_vm_writev` call; returns the number of bytes
/// transferred.
fn process_vm_write(pid: i32, addr: u64, bytes: &[u8]) -> Result<usize, CoreError> {
    if bytes.is_empty() {
        return Ok(0);
    }
    let local = libc::iovec {
        iov_base: bytes.as_ptr() as *mut libc::c_void,
        iov_len: bytes.len(),
    };
    let remote = libc::iovec {
        iov_base: addr as usize as *mut libc::c_void,
        iov_len: bytes.len(),
    };
    // SAFETY: `local` points to the valid `bytes` slice; the kernel
    // treats it read-only during the call. `remote` describes the
    // target's range; the caller validated it against the target's
    // mapped writable regions.
    let result = unsafe { libc::process_vm_writev(pid, &local, 1, &remote, 1, 0) };
    check_result(pid, result)
}

/// Maps a libc result to bytes transferred or a [`CoreError`].
fn check_result(pid: i32, result: isize) -> Result<usize, CoreError> {
    if result >= 0 {
        return Ok(result as usize);
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::EPERM) => {
            let scope = ptrace_scope().unwrap_or_else(|error| {
                tracing::warn!(%error, "failed to read ptrace_scope");
                None
            });
            Err(CoreError::PtraceDenied {
                pid,
                advice: ptrace_advice(scope),
            })
        }
        Some(libc::ESRCH) => Err(CoreError::ProcessGone { pid }),
        _ => Err(CoreError::Io {
            path: PathBuf::from(format!("/proc/{pid}/mem")),
            source: error,
        }),
    }
}

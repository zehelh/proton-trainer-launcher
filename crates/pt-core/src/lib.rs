//! Process discovery and memory read/write engine for proton-trainer.
//!
//! This crate owns every cross-process memory access in the workspace.
//! The safety contract:
//!
//! - Writes only go through [`engine::plan_write`] +
//!   [`engine::apply`], which validate the target range against one
//!   mapped writable region, stop the target, write, read back,
//!   verify, and resume the target on every path.
//! - Nothing is written without an explicit apply of a printed plan
//!   (dry-run semantics).
//! - The target is never left stopped: [`stop::StopGuard`] resumes it
//!   on drop, including on panics.
//!
//! `unsafe` exists only in [`mem`] and [`stop`], with documented
//! invariants; every other workspace crate forbids `unsafe`.
//!
//! Typical use (compiled but not executed by rustdoc; it needs a live
//! target):
//!
//! ```no_run
//! use pt_core::engine::{apply, plan_write};
//! use pt_core::process::{find_processes, TargetProcess};
//!
//! # fn main() -> Result<(), pt_core::CoreError> {
//! let target: TargetProcess = find_processes("game.exe")?
//!     .into_iter()
//!     .next()
//!     .ok_or(pt_core::CoreError::ProcessNotFound {
//!         name: "game.exe".to_string(),
//!     })?;
//! let plan = plan_write(target.pid, 0x1234_5678, &42u32.to_ne_bytes())?;
//! apply(&plan)?;
//! # Ok(())
//! # }
//! ```

pub mod engine;
pub mod error;
pub mod maps;
pub mod mem;
pub mod pattern;
pub mod process;
pub mod stop;

pub use engine::{apply, plan_write, WritePlan};
pub use error::CoreError;
pub use maps::{read_maps, validate_writable_range, Region};
pub use mem::{read_bytes, write_bytes};
pub use pattern::{scan_process, Pattern};
pub use process::{find_processes, ptrace_advice, ptrace_scope, TargetProcess};
pub use stop::StopGuard;

/// Returns the crate version, for diagnostics and `--version` output.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver() {
        assert_eq!(super::version().split('.').count(), 3);
    }
}

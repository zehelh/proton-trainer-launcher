//! Process discovery and memory read/write engine for proton-trainer.
//!
//! This crate owns every cross-process memory access in the workspace.
//! Milestone P2 will add: `/proc/<pid>` scanning, `/proc/<pid>/maps`
//! parsing, `process_vm_readv`/`process_vm_writev` with mapped-region
//! validation, read-back verification, and SIGSTOP/SIGCONT orchestration
//! that restores the target on every error path.
//!
//! `unsafe` is allowed in this crate only; all other workspace crates
//! forbid it.

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

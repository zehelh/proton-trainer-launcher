//! Terminal user interface for proton-trainer.
//!
//! Milestone P4 will add the ratatui screens: game list, running-process
//! check, trainer entry list, and apply/dry-run views. The TUI is a pure
//! library; `pt-cli` owns the terminal lifecycle.

#![forbid(unsafe_code)]

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

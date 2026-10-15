//! Terminal user interface for proton-trainer.
//!
//! The TUI is a pure library; the `proton-trainer` binary owns the
//! process and calls [`run_terminal`] with an [`App`].
//!
//! Architecture:
//!
//! - [`app`]: the state machine (screens, keys, transitions). Talks to
//!   a [`backend::TuiBackend`] only — no terminal, no engine.
//! - [`render`]: ratatui drawing of the [`App`] state; works at any
//!   terminal size.
//! - [`terminal`]: the crossterm event loop with restore-on-exit and
//!   restore-on-panic guarantees.
//! - [`live`]: the production backend wiring pt-discover (games),
//!   pt-trainers (definitions + denylist), and pt-core (attach, scan,
//!   plan, apply, freeze). The headless CLI commands reuse the same
//!   workflow functions.
//!
//! ```no_run
//! use pt_tui::app::App;
//! use pt_tui::live::LiveBackend;
//! use pt_tui::terminal::run_terminal;
//! use std::path::PathBuf;
//!
//! let backend = LiveBackend::new(PathBuf::from("trainers"))?;
//! let app = App::new(Box::new(backend));
//! run_terminal(app)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]

pub mod app;
pub mod backend;
pub mod live;
pub mod render;
pub mod terminal;

pub use app::{App, Screen};
pub use backend::{
    format_plan_row, ApplyResult, AttachOutcome, EntryBrief, GameRow, PlanRow, TuiBackend,
};
pub use live::{AttachReport, AttachedSession, LiveBackend};

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

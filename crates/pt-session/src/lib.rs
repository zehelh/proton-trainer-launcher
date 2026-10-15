//! The session workflow shared by the GUI and the headless CLI:
//! resolving a trainer's target, checking the anti-cheat denylist,
//! building dry-run plans, applying them, and freezing values.
//!
//! This crate contains no UI: the GUI (Tauri, `pt-gui`) and the CLI
//! subcommands both drive [`LiveSession`] / the free functions here.
//!
//! ```no_run
//! use pt_session::session::{AttachReport, attach_trainer, load_trainers};
//! use pt_trainers::Denylist;
//! use std::path::Path;
//!
//! let trainers = load_trainers(Path::new("trainers"))?;
//! let trainer = trainers.first().ok_or("no trainers")?;
//! let denylist = Denylist::embedded().map_err(|error| error.to_string())?;
//! match attach_trainer(trainer, &denylist)? {
//!     AttachReport::Running(_) => println!("attached"),
//!     _ => println!("not running or refused"),
//! }
//! # Ok::<(), String>(())
//! ```

#![forbid(unsafe_code)]

pub mod config;
pub mod freeze;
pub mod info;
pub mod launch;
pub mod logging;
pub mod session;
pub mod types;

pub use config::{AppConfig, GameTrainer, GlobalTrainer};
pub use freeze::{freeze_tick, freeze_write, spawn_freeze, FreezeHandle};
pub use launch::{launch_game, launch_with_trainer};
pub use session::{
    all_games, apply_plans, attach_outcome, attach_trainer, check_denylist, load_trainers,
    plan_entries, AttachReport, AttachedSession, LiveSession,
};
pub use types::{
    format_plan_row, hex, ApplyResult, AttachOutcome, EntryBrief, GameRow, PlanRow, PlanRowView,
};

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

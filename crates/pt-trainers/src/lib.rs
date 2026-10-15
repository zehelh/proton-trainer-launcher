//! Trainer definitions, validation, and the anti-cheat denylist.
//!
//! Trainer definitions are TOML files describing how to patch one
//! game's memory: byte patterns to scan for, offsets to the values,
//! typed values to write, and whether a value should be frozen.
//! Definitions are validated strictly (unknown keys rejected, ids
//! unique, patterns and values well-formed) with clear English errors.
//!
//! The anti-cheat denylist ([`Denylist`]) is shipped with the binary
//! and consulted before any attach: a target matching a known
//! anti-cheat component is refused, never bypassed.
//!
//! Typical use (compiled but not executed by rustdoc; the directory
//! must exist at runtime):
//!
//! ```no_run
//! use pt_trainers::loader::load_dir;
//! use std::path::Path;
//!
//! let trainers = load_dir(Path::new("trainers"))?;
//! for trainer in &trainers {
//!     println!("{}: {} entries", trainer.id, trainer.entries.len());
//! }
//! # Ok::<(), pt_trainers::TrainerError>(())
//! ```

#![forbid(unsafe_code)]

pub mod denylist;
pub mod error;
pub mod loader;
pub mod pattern;
pub mod trainer;
pub mod value;

pub use denylist::{Component, Denylist, Kind, Refusal};
pub use error::TrainerError;
pub use loader::{load_dir, load_trainer};
pub use trainer::{Entry, SearchSpec, Trainer, SUPPORTED_SCHEMA_VERSION};
pub use value::ValueSpec;

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

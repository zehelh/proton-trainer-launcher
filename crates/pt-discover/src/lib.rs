//! Steam game and compatdata discovery for proton-trainer.
//!
//! This crate answers "which Steam games are installed on this machine
//! and where are their Proton prefixes?" It scans native Steam roots
//! (`~/.local/share/Steam`, `~/.steam/steam`, `~/.steam/root`) and
//! Flatpak Steam (`~/.var/app/com.valvesoftware.Steam`), reads each
//! root's `steamapps/libraryfolders.vdf` and `appmanifest_*.acf` files,
//! and detects each game's `compatdata/<appid>` prefix.
//!
//! Typical use:
//!
//! ```
//! use pt_discover::{ScanOptions, scan};
//!
//! let options = ScanOptions::from_default_roots()?;
//! let result = scan(&options)?;
//! for game in &result.games {
//!     println!("{} {}", game.app_id, game.name);
//! }
//! # Ok::<(), pt_discover::DiscoverError>(())
//! ```
//!
//! For testing without Steam installed, set the
//! [`STEAM_ROOTS_ENV`](library::STEAM_ROOTS_ENV) environment variable
//! to a colon-separated list of directories containing `steamapps/`.

#![forbid(unsafe_code)]

pub mod cache;
pub mod error;
pub mod heroic;
pub mod library;
pub mod manifest;
pub mod scan;
pub mod vdf;

pub use error::DiscoverError;
pub use heroic::{scan_heroic, HeroicGame, HeroicStore};
pub use library::{SteamFlavor, SteamRoot};
pub use manifest::AppManifest;
pub use scan::{scan, CompatData, Game, ScanOptions, ScanResult};

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

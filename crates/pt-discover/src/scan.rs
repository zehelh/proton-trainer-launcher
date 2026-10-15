//! Full library scan: Steam roots -> libraries -> app manifests -> games.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::DiscoverError;
use crate::library::{default_steam_roots, parse_library_folders, SteamFlavor, SteamRoot};
use crate::manifest::{parse_app_manifest, AppManifest};

/// A Proton compatdata prefix for one game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatData {
    /// Path to `compatdata/<appid>/`, which contains the Wine prefix
    /// in its `pfx/` subdirectory.
    pub prefix: PathBuf,
}

/// An installed Steam game discovered on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    /// Steam application id.
    pub app_id: u32,
    /// Human-readable game name as Steam reports it.
    pub name: String,
    /// Install directory: `<steamapps>/common/<installdir>`.
    pub install_dir: PathBuf,
    /// The `steamapps/` directory holding the game's manifest.
    pub steamapps_dir: PathBuf,
    /// Whether this game was found through a native or Flatpak Steam.
    pub flavor: SteamFlavor,
    /// The game's Proton compatdata prefix, if it has one.
    ///
    /// `None` for native Linux games and for Proton games that have
    /// not been launched yet.
    pub compatdata: Option<CompatData>,
}

/// The result of one library scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanResult {
    /// Unix timestamp (seconds) at which the scan finished.
    pub scanned_at_unix: u64,
    /// Installed games, sorted by app id.
    pub games: Vec<Game>,
}

/// Parameters of a library scan.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Steam roots to scan, in priority order.
    pub roots: Vec<SteamRoot>,
    /// When `true`, also include games that are not fully installed.
    pub include_uninstalled: bool,
}

impl ScanOptions {
    /// Options using the default Steam root discovery.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoverError::NoHome`] when the home directory cannot
    /// be determined.
    pub fn from_default_roots() -> Result<Self, DiscoverError> {
        Ok(Self {
            roots: default_steam_roots()?,
            include_uninstalled: false,
        })
    }
}

/// Scans all configured Steam roots and returns the installed games.
///
/// For each root, libraries are read from its
/// `steamapps/libraryfolders.vdf` (falling back to the root's own
/// `steamapps/`), every `appmanifest_*.acf` is parsed, and games are
/// deduplicated by app id with the first root winning. Unreadable or
/// invalid manifests are logged as warnings and skipped, so a single
/// corrupt file never fails the whole scan.
///
/// # Errors
///
/// Returns [`DiscoverError::Io`] when a directory cannot be listed or a
/// readable file fails to load, and parse errors from
/// [`parse_library_folders`](crate::library::parse_library_folders).
pub fn scan(options: &ScanOptions) -> Result<ScanResult, DiscoverError> {
    let mut seen_app_ids: HashSet<u32> = HashSet::new();
    let mut games = Vec::new();

    for root in &options.roots {
        let steamapps = root.path.join("steamapps");
        if !steamapps.is_dir() {
            tracing::debug!(root = %root.path.display(), "skipping root: no steamapps");
            continue;
        }
        for library in libraries_for_root(&steamapps)? {
            if !library.is_dir() {
                tracing::debug!(library = %library.display(), "skipping library: not a directory");
                continue;
            }
            games.extend(games_for_library(
                &library,
                root.flavor,
                options.include_uninstalled,
                &mut seen_app_ids,
            )?);
        }
    }

    games.sort_by_key(|game| game.app_id);
    let scanned_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    tracing::debug!(games = games.len(), "scan finished");
    Ok(ScanResult {
        scanned_at_unix,
        games,
    })
}

/// Returns the `steamapps/` directory of every library declared by a
/// root, or just the root's own `steamapps/` when it declares none.
fn libraries_for_root(steamapps: &Path) -> Result<Vec<PathBuf>, DiscoverError> {
    let vdf_path = steamapps.join("libraryfolders.vdf");
    if !vdf_path.is_file() {
        return Ok(vec![steamapps.to_path_buf()]);
    }
    let text = std::fs::read_to_string(&vdf_path).map_err(|source| DiscoverError::Io {
        path: vdf_path.clone(),
        source,
    })?;
    let libraries = parse_library_folders(&text)?;
    if libraries.is_empty() {
        return Ok(vec![steamapps.to_path_buf()]);
    }
    Ok(libraries
        .into_iter()
        .map(|library| library.join("steamapps"))
        .collect())
}

/// Collects the games of one `steamapps/` directory.
fn games_for_library(
    steamapps: &Path,
    flavor: SteamFlavor,
    include_uninstalled: bool,
    seen_app_ids: &mut HashSet<u32>,
) -> Result<Vec<Game>, DiscoverError> {
    let mut games = Vec::new();
    let entries = std::fs::read_dir(steamapps).map_err(|source| DiscoverError::Io {
        path: steamapps.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| DiscoverError::Io {
            path: steamapps.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !(file_name.starts_with("appmanifest_") && file_name.ends_with(".acf")) {
            continue;
        }

        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) => {
                tracing::warn!(manifest = %path.display(), %source, "skipping unreadable app manifest");
                continue;
            }
        };
        let manifest = match parse_app_manifest(&text) {
            Ok(manifest) => manifest,
            Err(error) => {
                tracing::warn!(manifest = %path.display(), %error, "skipping invalid app manifest");
                continue;
            }
        };

        if !include_uninstalled && !manifest.is_fully_installed() {
            tracing::debug!(app_id = manifest.app_id, "skipping: not fully installed");
            continue;
        }
        if !seen_app_ids.insert(manifest.app_id) {
            tracing::debug!(
                app_id = manifest.app_id,
                "skipping: already seen in an earlier root"
            );
            continue;
        }

        games.push(game_from_manifest(&manifest, steamapps, flavor));
    }
    Ok(games)
}

/// Builds a [`Game`] from a manifest, detecting its compatdata prefix.
fn game_from_manifest(manifest: &AppManifest, steamapps: &Path, flavor: SteamFlavor) -> Game {
    let compatdata_dir = steamapps
        .join("compatdata")
        .join(manifest.app_id.to_string());
    let compatdata = if compatdata_dir.join("pfx").is_dir() {
        Some(CompatData {
            prefix: compatdata_dir,
        })
    } else {
        None
    };
    Game {
        app_id: manifest.app_id,
        name: manifest.name.clone(),
        install_dir: steamapps.join("common").join(&manifest.installdir),
        steamapps_dir: steamapps.to_path_buf(),
        flavor,
        compatdata,
    }
}

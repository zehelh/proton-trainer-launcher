//! Discovery of Steam installation roots and library folders.

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::DiscoverError;
use crate::vdf::{self, VdfValue};

/// Environment variable that overrides Steam root discovery.
///
/// Set to a colon-separated list of directories, each containing a
/// `steamapps/` subdirectory. Intended for testing on machines without
/// Steam (also used by the test suite and CI).
pub const STEAM_ROOTS_ENV: &str = "PROTON_TRAINER_STEAM_ROOTS";

/// Where a Steam installation lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SteamFlavor {
    /// Steam installed on the host, e.g. `~/.local/share/Steam`.
    Native,
    /// Steam installed as a Flatpak, e.g. under
    /// `~/.var/app/com.valvesoftware.Steam`.
    Flatpak,
}

/// A Steam installation root: the directory containing `steamapps/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteamRoot {
    /// Path to the Steam root directory.
    pub path: PathBuf,
    /// Whether this root is a native or Flatpak installation.
    pub flavor: SteamFlavor,
}

/// Returns the Steam roots to scan, honoring [`STEAM_ROOTS_ENV`].
///
/// Candidates (first match wins, duplicates through symlinks removed):
///
/// - Native: `$HOME/.local/share/Steam`, `$HOME/.steam/steam`,
///   `$HOME/.steam/root`
/// - Flatpak: `$HOME/.var/app/com.valvesoftware.Steam/data/Steam`,
///   `$HOME/.var/app/com.valvesoftware.Steam/.local/share/Steam`
///
/// A candidate only counts when its `steamapps/` directory exists, so
/// the returned list is directly scannable.
pub fn default_steam_roots() -> Result<Vec<SteamRoot>, DiscoverError> {
    if let Ok(value) = std::env::var(STEAM_ROOTS_ENV) {
        if !value.trim().is_empty() {
            let roots = value
                .split(':')
                .filter(|part| !part.is_empty())
                .map(PathBuf::from)
                .map(|path| SteamRoot {
                    path,
                    flavor: SteamFlavor::Native,
                })
                .collect::<Vec<_>>();
            tracing::debug!(count = roots.len(), "using steam roots from environment");
            return Ok(roots);
        }
    }

    let home = dirs::home_dir().ok_or(DiscoverError::NoHome)?;

    let candidates = [
        (home.join(".local/share/Steam"), SteamFlavor::Native),
        (home.join(".steam/steam"), SteamFlavor::Native),
        (home.join(".steam/root"), SteamFlavor::Native),
        (
            home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
            SteamFlavor::Flatpak,
        ),
        (
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            SteamFlavor::Flatpak,
        ),
    ];

    let mut seen = BTreeSet::new();
    let mut roots = Vec::new();
    for (path, flavor) in candidates {
        if !path.join("steamapps").is_dir() {
            tracing::debug!(root = %path.display(), "skipping: no steamapps directory");
            continue;
        }
        // `~/.steam/steam` and friends are symlinks to the same install.
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !seen.insert(canonical) {
            tracing::debug!(root = %path.display(), "skipping: duplicate of an earlier root");
            continue;
        }
        roots.push(SteamRoot { path, flavor });
    }
    Ok(roots)
}

/// Parses the text of a `libraryfolders.vdf` file into library paths.
///
/// Returns the `"path"` value of each numbered entry, ordered by entry
/// number (Steam writes `"0"`, `"1"`, ...). Duplicated paths are kept
/// only once.
pub fn parse_library_folders(text: &str) -> Result<Vec<PathBuf>, DiscoverError> {
    let root = vdf::parse(text)?;
    let folders = root
        .get("libraryfolders")
        .and_then(VdfValue::as_map)
        .ok_or_else(|| {
            DiscoverError::LibraryFolders("missing \"libraryfolders\" map".to_string())
        })?;

    let mut numbered: Vec<(u64, PathBuf)> = Vec::new();
    for (key, value) in folders {
        let index: u64 = key.parse().map_err(|_| {
            DiscoverError::LibraryFolders(format!("non-numeric library key: {key:?}"))
        })?;
        let path = value
            .get("path")
            .and_then(VdfValue::as_str)
            .ok_or_else(|| {
                DiscoverError::LibraryFolders(format!("library {index} is missing its \"path\""))
            })?;
        if path.is_empty() {
            return Err(DiscoverError::LibraryFolders(format!(
                "library {index} has an empty \"path\""
            )));
        }
        numbered.push((index, PathBuf::from(path)));
    }
    numbered.sort_by_key(|(index, _)| *index);

    let mut seen = HashSet::new();
    Ok(numbered
        .into_iter()
        .map(|(_, path)| path)
        .filter(|path| seen.insert(path.clone()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_folders_are_ordered_numerically() {
        // Lexicographic order would put "10" before "2".
        let text = "\"libraryfolders\"\n{\n\t\"2\"\t{ \"path\" \"/lib2\" }\n\t\"10\"\t{ \"path\" \"/lib10\" }\n\t\"1\"\t{ \"path\" \"/lib1\" }\n}\n";
        let paths = parse_library_folders(text).unwrap();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/lib1"),
                PathBuf::from("/lib2"),
                PathBuf::from("/lib10")
            ]
        );
    }

    #[test]
    fn duplicate_library_paths_are_deduplicated() {
        let text = "\"libraryfolders\"\n{\n\t\"0\"\t{ \"path\" \"/lib\" }\n\t\"1\"\t{ \"path\" \"/lib\" }\n}\n";
        let paths = parse_library_folders(text).unwrap();
        assert_eq!(paths, vec![PathBuf::from("/lib")]);
    }

    #[test]
    fn rejects_missing_libraryfolders_section() {
        let error = parse_library_folders("\"something\" { }").unwrap_err();
        assert!(error.to_string().contains("libraryfolders"));
    }

    #[test]
    fn rejects_missing_path() {
        let error = parse_library_folders("\"libraryfolders\" { \"0\" { } }").unwrap_err();
        assert!(error.to_string().contains("path"));
    }
}

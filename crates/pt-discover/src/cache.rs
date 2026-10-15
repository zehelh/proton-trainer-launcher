//! Scan-result cache in the XDG config directory (TOML).

use std::path::{Path, PathBuf};

use crate::error::DiscoverError;
use crate::scan::ScanResult;

/// Returns the default cache file path:
/// `$XDG_CONFIG_HOME/proton-trainer/scan-cache.toml`.
///
/// # Errors
///
/// Returns [`DiscoverError::NoHome`] when the XDG config directory
/// cannot be determined.
pub fn cache_file() -> Result<PathBuf, DiscoverError> {
    let config_dir = dirs::config_dir().ok_or(DiscoverError::NoHome)?;
    Ok(config_dir.join("proton-trainer").join("scan-cache.toml"))
}

/// Saves a scan result to the default cache file.
///
/// # Errors
///
/// Returns an error when the config directory cannot be determined,
/// the file cannot be written, or serialization fails.
pub fn save(result: &ScanResult) -> Result<(), DiscoverError> {
    save_to(&cache_file()?, result)
}

/// Loads the cached scan result from the default cache file.
///
/// Returns `Ok(None)` when no cache exists yet.
///
/// # Errors
///
/// Returns an error when the config directory cannot be determined,
/// the file cannot be read, or deserialization fails.
pub fn load() -> Result<Option<ScanResult>, DiscoverError> {
    load_from(&cache_file()?)
}

/// Writes a scan result as TOML to `path`, creating parent directories.
pub fn save_to(path: &Path, result: &ScanResult) -> Result<(), DiscoverError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| DiscoverError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let text = toml::to_string(result)?;
    std::fs::write(path, text).map_err(|source| DiscoverError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    tracing::debug!(cache = %path.display(), "scan cache saved");
    Ok(())
}

/// Reads a scan result from a TOML cache file; `Ok(None)` when missing.
pub fn load_from(path: &Path) -> Result<Option<ScanResult>, DiscoverError> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path).map_err(|source| DiscoverError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let result = toml::from_str(&text)?;
    tracing::debug!(cache = %path.display(), "scan cache loaded");
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::Game;

    fn sample_result() -> ScanResult {
        ScanResult {
            scanned_at_unix: 1_700_000_000,
            games: vec![Game {
                app_id: 367520,
                name: "Hollow Knight".to_string(),
                install_dir: PathBuf::from("/steamapps/common/Hollow Knight"),
                steamapps_dir: PathBuf::from("/steamapps"),
                flavor: crate::library::SteamFlavor::Native,
                compatdata: Some(crate::scan::CompatData {
                    prefix: PathBuf::from("/steamapps/compatdata/367520"),
                }),
            }],
        }
    }

    #[test]
    fn cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("scan-cache.toml");

        assert_eq!(load_from(&path).unwrap(), None);

        save_to(&path, &sample_result()).unwrap();
        assert_eq!(load_from(&path).unwrap().unwrap(), sample_result());
    }

    #[test]
    fn corrupted_cache_reports_deserialize_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan-cache.toml");
        std::fs::write(&path, "not = [valid").unwrap();
        assert!(load_from(&path).is_err());
    }
}

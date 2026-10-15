//! Parsing of Steam `appmanifest_*.acf` files.

use std::collections::BTreeMap;

use crate::error::DiscoverError;
use crate::vdf::{self, VdfValue};

/// `StateFlags` bit indicating the app is fully installed.
pub const STATE_FLAG_FULLY_INSTALLED: u32 = 0x0004;

/// The subset of an `appmanifest_*.acf` file proton-trainer cares about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManifest {
    /// Steam application id, e.g. `367520`.
    pub app_id: u32,
    /// Human-readable game name as Steam reports it.
    pub name: String,
    /// Directory name under `steamapps/common/`.
    pub installdir: String,
    /// Raw `StateFlags` bitmask; bit `0x4` means fully installed.
    pub state_flags: u32,
}

impl AppManifest {
    /// Returns `true` when the app is fully installed per `StateFlags`.
    pub fn is_fully_installed(&self) -> bool {
        self.state_flags & STATE_FLAG_FULLY_INSTALLED != 0
    }
}

/// Parses the text of an `appmanifest_*.acf` file.
///
/// Required fields are `appid`, `name`, `installdir`, and `StateFlags`.
/// Missing or invalid fields produce a [`DiscoverError::Manifest`] with
/// the offending key named in the message.
pub fn parse_app_manifest(text: &str) -> Result<AppManifest, DiscoverError> {
    let root = vdf::parse(text)?;
    let state = root
        .get("AppState")
        .and_then(VdfValue::as_map)
        .ok_or_else(|| DiscoverError::Manifest("missing \"AppState\" section".to_string()))?;

    let app_id = required_str(state, "appid")?;
    let app_id = parse_number(&app_id, "appid")?;

    let name = required_str(state, "name")?;
    if name.trim().is_empty() {
        return Err(DiscoverError::Manifest("\"name\" is empty".to_string()));
    }

    let installdir = required_str(state, "installdir")?;
    if installdir.trim().is_empty() {
        return Err(DiscoverError::Manifest(
            "\"installdir\" is empty".to_string(),
        ));
    }

    let state_flags = required_str(state, "StateFlags")?;
    let state_flags = parse_number(&state_flags, "StateFlags")?;

    Ok(AppManifest {
        app_id,
        name,
        installdir,
        state_flags,
    })
}

fn required_str(map: &BTreeMap<String, VdfValue>, key: &str) -> Result<String, DiscoverError> {
    map.get(key)
        .and_then(VdfValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| DiscoverError::Manifest(format!("missing or non-string \"{key}\"")))
}

fn parse_number(value: &str, key: &str) -> Result<u32, DiscoverError> {
    value
        .parse()
        .map_err(|_| DiscoverError::Manifest(format!("\"{key}\" is not a number: {value:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "\"AppState\"\n{\n\t\"appid\"\t\t\"367520\"\n\t\"name\"\t\t\"Hollow Knight\"\n\t\"installdir\"\t\t\"Hollow Knight\"\n\t\"StateFlags\"\t\t\"4\"\n}\n";

    #[test]
    fn parses_a_valid_manifest() {
        let manifest = parse_app_manifest(MANIFEST).unwrap();
        assert_eq!(manifest.app_id, 367520);
        assert_eq!(manifest.name, "Hollow Knight");
        assert!(manifest.is_fully_installed());
    }

    #[test]
    fn not_fully_installed_flag() {
        let manifest = parse_app_manifest(MANIFEST.replace("\"4\"", "\"2\"").as_str()).unwrap();
        assert!(!manifest.is_fully_installed());
    }

    #[test]
    fn rejects_missing_fields() {
        let error = parse_app_manifest("\"AppState\" { \"appid\" \"1\" }").unwrap_err();
        assert!(error.to_string().contains("name"));
    }

    #[test]
    fn rejects_non_numeric_appid() {
        let error =
            parse_app_manifest("\"AppState\" { \"appid\" \"abc\" \"name\" \"X\" \"installdir\" \"x\" \"StateFlags\" \"4\" }")
                .unwrap_err();
        assert!(error.to_string().contains("appid"));
    }

    #[test]
    fn rejects_empty_name() {
        let error = parse_app_manifest("\"AppState\" { \"appid\" \"1\" \"name\" \"\" \"installdir\" \"x\" \"StateFlags\" \"4\" }")
            .unwrap_err();
        assert!(error.to_string().contains("empty"));
    }
}

//! Trainer definition structs, parsing, and validation.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::TrainerError;
use crate::pattern::{parse_pattern, to_core_pattern};
use crate::value::ValueSpec;
use pt_core::Pattern;

/// The only trainer schema version this release understands.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// A complete trainer definition loaded from a TOML file.
///
/// # Example
///
/// ```toml
/// schema_version = 1
/// id = "hollow-knight"
/// name = "Hollow Knight"
/// steam_app_id = 367520
/// exe_names = ["hollow_knight.exe"]
/// version = "0.1.0"
/// description = "Single-player trainer example"
///
/// [[entry]]
/// id = "soul"
/// label = "Infinite SOUL"
/// search = { pattern = "89 45 ?? 8B 06" }
/// offset = 12
/// value = { type = "i32", data = 99 }
/// freeze = false
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trainer {
    /// Schema version; must be [`SUPPORTED_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Unique trainer id, lowercase kebab-case (e.g. `hollow-knight`).
    pub id: String,
    /// Human-readable game name.
    pub name: String,
    /// Steam application id of the game.
    pub steam_app_id: u32,
    /// Executable names (lowercase) the trainer can attach to.
    pub exe_names: Vec<String>,
    /// Trainer version string.
    pub version: String,
    /// Optional human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// The trainer entries (at least one); written as `[[entry]]`.
    #[serde(rename = "entry")]
    pub entries: Vec<Entry>,
}

/// One trainer entry: scan for a pattern, then write a value at a fixed
/// offset from the match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Unique entry id within the trainer, lowercase kebab-case.
    pub id: String,
    /// Human-readable label shown in the TUI.
    pub label: String,
    /// The byte pattern to scan for.
    pub search: SearchSpec,
    /// Bytes between the match start and the value location.
    pub offset: u32,
    /// The value to write.
    pub value: ValueSpec,
    /// When `true`, the value is re-applied periodically while the
    /// session is open.
    #[serde(default)]
    pub freeze: bool,
}

/// The pattern specification of an entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchSpec {
    /// Hex bytes with `??` wildcards, whitespace-separated.
    pub pattern: String,
}

impl SearchSpec {
    /// Parses the pattern into a [`pt_core::Pattern`].
    ///
    /// # Errors
    ///
    /// Returns an error naming the offending token.
    pub fn to_pattern(&self) -> Result<Pattern, String> {
        to_core_pattern(&self.pattern)
    }
}

impl Entry {
    /// Returns the address where this entry's value lives, given the
    /// address of the pattern match.
    pub fn value_address(&self, match_address: u64) -> u64 {
        match_address + self.offset as u64
    }
}

impl Trainer {
    /// Finds an entry by id.
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Returns `true` when this trainer can target a process with the
    /// given (lowercase) executable name.
    pub fn matches_process(&self, exe_name: &str) -> bool {
        let exe_name = exe_name.to_ascii_lowercase();
        self.exe_names.contains(&exe_name)
    }
}

/// Parses a trainer definition from TOML text and validates it.
///
/// Unknown keys, an unsupported schema version, invalid or duplicate
/// ids, empty executable lists, and malformed patterns are all
/// rejected with clear English errors naming the file and the
/// offending item.
///
/// # Errors
///
/// Returns a [`TrainerError`] naming `path` on any failure.
pub fn parse_trainer(path: &Path, text: &str) -> Result<Trainer, TrainerError> {
    let trainer: Trainer = toml::from_str(text).map_err(|source| TrainerError::Deserialize {
        path: path.to_path_buf(),
        source,
    })?;
    validate_trainer(path, trainer)
}

/// Validates and normalizes a parsed trainer definition.
fn validate_trainer(path: &Path, mut trainer: Trainer) -> Result<Trainer, TrainerError> {
    let schema = || {
        TrainerError::schema(
            path,
            format!(
                "unsupported schema_version {} (this build supports {})",
                trainer.schema_version, SUPPORTED_SCHEMA_VERSION
            ),
        )
    };
    if trainer.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(schema());
    }

    if !valid_id(&trainer.id) {
        return Err(TrainerError::schema(
            path,
            format!(
                "trainer id {:?} is invalid: use lowercase letters, digits, and dashes",
                trainer.id
            ),
        ));
    }
    if trainer.name.trim().is_empty() {
        return Err(TrainerError::schema(path, "\"name\" is empty".to_string()));
    }

    if trainer.exe_names.is_empty() {
        return Err(TrainerError::schema(
            path,
            "\"exe_names\" is empty: a trainer must declare at least one executable name"
                .to_string(),
        ));
    }
    for exe_name in &trainer.exe_names {
        if exe_name.trim().is_empty() {
            return Err(TrainerError::schema(
                path,
                "\"exe_names\" contains an empty name".to_string(),
            ));
        }
    }
    trainer.exe_names = trainer
        .exe_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();

    if trainer.entries.is_empty() {
        return Err(TrainerError::schema(
            path,
            "\"entries\" is empty: a trainer must declare at least one entry".to_string(),
        ));
    }
    let mut seen = HashSet::new();
    for entry in &trainer.entries {
        if !valid_id(&entry.id) {
            return Err(TrainerError::schema(
                path,
                format!(
                    "entry id {:?} is invalid: use lowercase letters, digits, and dashes",
                    entry.id
                ),
            ));
        }
        if !seen.insert(entry.id.clone()) {
            return Err(TrainerError::schema(
                path,
                format!("duplicate entry id {:?}", entry.id),
            ));
        }
        if entry.label.trim().is_empty() {
            return Err(TrainerError::schema(
                path,
                format!("entry {:?} has an empty \"label\"", entry.id),
            ));
        }
        if let Err(message) = parse_pattern(&entry.search.pattern) {
            return Err(TrainerError::schema(
                path,
                format!(
                    "entry {:?} has an invalid search pattern: {message}",
                    entry.id
                ),
            ));
        }
        if let Err(message) = entry.value.to_bytes() {
            return Err(TrainerError::schema(
                path,
                format!("entry {:?} has an invalid value: {message}", entry.id),
            ));
        }
    }
    Ok(trainer)
}

/// Returns `true` when `id` is a non-empty lowercase kebab-case id.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_toml() -> String {
        "\
schema_version = 1
id = \"test-game\"
name = \"Test Game\"
steam_app_id = 1234
exe_names = [\"Game.exe\"]
version = \"0.1.0\"

[[entry]]
id = \"gold\"
label = \"Infinite gold\"
search = { pattern = \"89 45 ?? 8B\" }
offset = 12
value = { type = \"i32\", data = 99 }
"
        .to_string()
    }

    fn parse(text: &str) -> Result<Trainer, TrainerError> {
        parse_trainer(Path::new("test.toml"), text)
    }

    #[test]
    fn parses_and_normalizes_a_valid_trainer() {
        let trainer = parse(&valid_toml()).unwrap();
        assert_eq!(trainer.id, "test-game");
        assert_eq!(trainer.exe_names, vec!["game.exe"]);
        assert_eq!(trainer.entries.len(), 1);
        assert_eq!(trainer.entries[0].value_address(0x1000), 0x1000 + 12);
        assert!(trainer.matches_process("GAME.EXE"));
        assert!(!trainer.matches_process("other.exe"));
    }

    #[test]
    fn rejects_unknown_keys() {
        let text = valid_toml().replace("version = \"0.1.0\"", "version = \"0.1.0\"\nbogus = 1");
        let error = parse(&text).unwrap_err();
        assert!(error.to_string().contains("bogus"), "got: {error}");
    }

    #[test]
    fn rejects_unsupported_schema_versions() {
        let text = valid_toml().replace("schema_version = 1", "schema_version = 2");
        let error = parse(&text).unwrap_err();
        assert!(error.to_string().contains("schema_version"));
    }

    #[test]
    fn rejects_invalid_trainer_ids() {
        let text = valid_toml().replace("id = \"test-game\"", "id = \"Test Game\"");
        assert!(parse(&text).unwrap_err().to_string().contains("trainer id"));
    }

    #[test]
    fn rejects_empty_exe_names() {
        let text = valid_toml().replace("exe_names = [\"Game.exe\"]", "exe_names = []");
        let error = parse(&text).unwrap_err();
        assert!(error.to_string().contains("exe_names"));
    }

    #[test]
    fn rejects_duplicate_entry_ids() {
        // Append a second entry table with the same entry id "gold".
        let second_entry = "\n\
[[entry]]\n\
id = \"gold\"\n\
label = \"Also gold\"\n\
search = { pattern = \"8B 06\" }\n\
offset = 2\n\
value = { type = \"i32\", data = 1 }\n";
        let text = valid_toml() + second_entry;
        let error = parse(&text).unwrap_err();
        assert!(error.to_string().contains("duplicate entry id"));
    }

    #[test]
    fn rejects_empty_entries() {
        // Header only: no [[entry]] table at all.
        let text = valid_toml().split("[[entry]]").next().unwrap().to_string();
        let error = parse(&text).unwrap_err();
        assert!(error.to_string().contains("entry"), "got: {error}");
    }

    #[test]
    fn rejects_invalid_patterns_with_entry_context() {
        let text = valid_toml().replace("89 45 ?? 8B", "89 ZZ");
        let error = parse(&text).unwrap_err();
        assert!(error.to_string().contains("entry \"gold\""));
        assert!(error.to_string().contains("ZZ"));
    }

    #[test]
    fn rejects_invalid_values_with_entry_context() {
        // type = "bytes" with an invalid hex payload reaches the
        // value validation, which names the offending entry.
        let text = valid_toml().replace(
            "value = { type = \"i32\", data = 99 }",
            "value = { type = \"bytes\", data = \"XX 99\" }",
        );
        let error = parse(&text).unwrap_err();
        assert!(error.to_string().contains("entry \"gold\""), "got: {error}");
        assert!(error.to_string().contains("XX"));
    }
}

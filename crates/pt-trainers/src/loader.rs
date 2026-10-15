//! Loading trainer definitions from a directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::TrainerError;
use crate::trainer::{parse_trainer, Trainer};

/// Loads one trainer definition file.
///
/// # Errors
///
/// Returns a [`TrainerError`] naming `path` on any read, parse, or
/// validation failure.
pub fn load_trainer(path: &Path) -> Result<Trainer, TrainerError> {
    let text = std::fs::read_to_string(path).map_err(|source| TrainerError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_trainer(path, &text)
}

/// Loads every `*.toml` trainer definition in `dir`, sorted by trainer
/// id.
///
/// Every file must be valid: a single broken trainer file fails the
/// load with the file named, because silently ignoring a broken
/// definition would surprise the user more than an explicit error.
/// Duplicate trainer ids across files are also refused, with both file
/// paths named.
///
/// # Errors
///
/// Returns a [`TrainerError`] on any read, parse, validation, or
/// duplicate-id failure.
pub fn load_dir(dir: &Path) -> Result<Vec<Trainer>, TrainerError> {
    let entries = std::fs::read_dir(dir).map_err(|source| TrainerError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| TrainerError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut trainers = Vec::new();
    let mut ids: BTreeMap<String, PathBuf> = BTreeMap::new();
    for path in paths {
        let trainer = load_trainer(&path)?;
        if let Some(first) = ids.get(&trainer.id) {
            return Err(TrainerError::DuplicateTrainerId {
                id: trainer.id.clone(),
                first: first.clone(),
                second: path,
            });
        }
        ids.insert(trainer.id.clone(), path);
        trainers.push(trainer);
    }
    trainers.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(trainers)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "\
schema_version = 1
id = \"test-game\"
name = \"Test Game\"
steam_app_id = 1234
exe_names = [\"game.exe\"]
version = \"0.1.0\"

[[entry]]
id = \"gold\"
label = \"Infinite gold\"
search = { pattern = \"89 45\" }
offset = 4
value = { type = \"i32\", data = 99 }
";

    #[test]
    fn loads_all_valid_files_sorted_by_id() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("b.toml"),
            VALID.replace("test-game", "b-game"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("a.toml"),
            VALID.replace("test-game", "a-game"),
        )
        .unwrap();
        std::fs::write(dir.path().join("notes.txt"), "not a trainer").unwrap();

        let trainers = load_dir(dir.path()).unwrap();
        let ids: Vec<&str> = trainers.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["a-game", "b-game"]);
    }

    #[test]
    fn refuses_duplicate_trainer_ids_across_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("one.toml"), VALID).unwrap();
        std::fs::write(dir.path().join("two.toml"), VALID).unwrap();

        let error = load_dir(dir.path()).unwrap_err();
        assert!(error.to_string().contains("duplicate trainer id"));
        assert!(error.to_string().contains("one.toml"));
        assert!(error.to_string().contains("two.toml"));
    }

    #[test]
    fn refuses_invalid_files_with_the_path_named() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("broken.toml"), "not a trainer").unwrap();

        let error = load_dir(dir.path()).unwrap_err();
        assert!(error.to_string().contains("broken.toml"));
    }

    #[test]
    fn errors_on_missing_directories() {
        let error = load_dir(Path::new("/nonexistent-trainer-dir-xyz")).unwrap_err();
        assert!(error.to_string().contains("/nonexistent-trainer-dir-xyz"));
    }
}

//! User configuration: external global trainers and per-game linked
//! trainers, stored in `$XDG_CONFIG_HOME/proton-trainer/config.toml`.
//!
//! proton-trainer is a neutral launcher: the trainer files themselves
//! are never bundled, distributed, or validated beyond existing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The only config schema version this release understands.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Default seconds to wait for a game's protonhax environment
/// before falling back to a direct prefix run.
pub const DEFAULT_PROTONHAX_WAIT_SECS: u64 = 10;

/// Default seconds to wait for a launched game to actually start
/// (Steam's launch reaper, Heroic's game executable) before the
/// trainer connects.
pub const DEFAULT_GAME_WAIT_SECS: u64 = 90;

/// Serde default for [`AppConfig::protonhax_wait_secs`].
fn default_protonhax_wait_secs() -> u64 {
    DEFAULT_PROTONHAX_WAIT_SECS
}

/// Serde default for [`AppConfig::game_wait_secs`].
fn default_game_wait_secs() -> u64 {
    DEFAULT_GAME_WAIT_SECS
}

/// A global trainer: one Windows executable usable with any game
/// (Aurora, WeMod, xModHub, ...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalTrainer {
    /// Unique trainer id, lowercase kebab-case.
    pub id: String,
    /// Human-readable name shown in the UI.
    pub name: String,
    /// Absolute path to the trainer executable.
    pub path: String,
}

/// A trainer linked to one specific game (e.g. a FLiNG trainer for a
/// single game).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameTrainer {
    /// The store the game comes from (`steam` or `heroic`).
    pub store: String,
    /// Steam application id, or Heroic app name.
    pub game_id: String,
    /// Human-readable name shown in the UI.
    pub name: String,
    /// Absolute path to the trainer executable.
    pub path: String,
}

/// The application configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    /// Schema version; must be [`SUPPORTED_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Global trainers usable with any game.
    #[serde(default)]
    pub trainers: Vec<GlobalTrainer>,
    /// Trainers linked to specific games.
    #[serde(rename = "game_trainer", default)]
    pub game_trainers: Vec<GameTrainer>,
    /// Seconds to wait for a game's protonhax environment directory
    /// (games launched with `protonhax init %command%`) before
    /// falling back to a direct prefix run. `0` checks once without
    /// waiting.
    #[serde(default = "default_protonhax_wait_secs")]
    pub protonhax_wait_secs: u64,
    /// Seconds to wait for a launched game to actually start before
    /// the trainer connects (Steam: the launch reaper; Heroic: the
    /// game executable). When the wait runs out, Steam trainers
    /// launch anyway with a warning and Heroic trainers abort with
    /// an error — a trainer connecting before the game owns the
    /// prefix would deadlock it.
    #[serde(default = "default_game_wait_secs")]
    pub game_wait_secs: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            trainers: Vec::new(),
            game_trainers: Vec::new(),
            protonhax_wait_secs: DEFAULT_PROTONHAX_WAIT_SECS,
            game_wait_secs: DEFAULT_GAME_WAIT_SECS,
        }
    }
}

impl AppConfig {
    /// Returns the config file path:
    /// `$XDG_CONFIG_HOME/proton-trainer/config.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error when the XDG config directory cannot be
    /// determined.
    pub fn path() -> Result<PathBuf, String> {
        let config_dir = dirs::config_dir().ok_or("could not find the config directory")?;
        Ok(config_dir.join("proton-trainer").join("config.toml"))
    }

    /// Loads the configuration; a missing file yields the default
    /// (empty) config.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message on read or parse failures.
    pub fn load() -> Result<Self, String> {
        Self::load_from(&Self::path()?)
    }

    /// Loads the configuration from an explicit path.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message on read or parse failures.
    pub fn load_from(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let config: AppConfig = toml::from_str(&text)
                    .map_err(|error| format!("invalid config {}: {error}", path.display()))?;
                config.into_validated()
            }
            Err(source) if matches!(source.kind(), std::io::ErrorKind::NotFound) => {
                Ok(Self::default())
            }
            Err(source) => Err(format!("failed to read {}: {source}", path.display())),
        }
    }

    /// Saves the configuration.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message on write failures.
    pub fn save(&self) -> Result<(), String> {
        Self::save_to(&Self::path()?, self)
    }

    /// Saves the configuration to an explicit path.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message on write failures.
    pub fn save_to(path: &Path, config: &Self) -> Result<(), String> {
        config.validate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|source| format!("failed to create {}: {source}", parent.display()))?;
        }
        let text = toml::to_string(config)
            .map_err(|error| format!("failed to serialize the config: {error}"))?;
        std::fs::write(path, text)
            .map_err(|source| format!("failed to write {}: {source}", path.display()))
    }

    /// Validates structure: unique trainer ids, non-empty names and
    /// paths, known stores.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message naming the offending entry.
    pub fn validate(&self) -> Result<(), String> {
        let validated = self.clone().into_validated()?;
        let _ = validated;
        Ok(())
    }

    /// Validates and returns the config, for internal use.
    fn into_validated(self) -> Result<Self, String> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(format!(
                "unsupported config schema_version {} (this build supports {})",
                self.schema_version, SUPPORTED_SCHEMA_VERSION
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for trainer in &self.trainers {
            if !valid_id(&trainer.id) {
                return Err(format!(
                    "trainer id {:?} is invalid: use lowercase letters, digits, and dashes",
                    trainer.id
                ));
            }
            if !seen.insert(trainer.id.clone()) {
                return Err(format!("duplicate trainer id {:?}", trainer.id));
            }
            if trainer.name.trim().is_empty() {
                return Err(format!("trainer {:?} has an empty name", trainer.id));
            }
            if trainer.path.trim().is_empty() {
                return Err(format!("trainer {:?} has an empty path", trainer.id));
            }
        }
        for link in &self.game_trainers {
            if link.store != "steam" && link.store != "heroic" {
                return Err(format!(
                    "game trainer {:?} has an unknown store {:?}",
                    link.name, link.store
                ));
            }
            if link.game_id.trim().is_empty() {
                return Err(format!("game trainer {:?} has an empty game id", link.name));
            }
            if link.path.trim().is_empty() {
                return Err(format!("game trainer {:?} has an empty path", link.name));
            }
        }
        Ok(self)
    }

    /// The trainers linked to one game.
    pub fn trainers_for_game(&self, store: &str, game_id: &str) -> Vec<&GameTrainer> {
        self.game_trainers
            .iter()
            .filter(|link| link.store == store && link.game_id == game_id)
            .collect()
    }
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

    #[test]
    fn missing_config_file_yields_defaults() {
        let config =
            AppConfig::load_from(Path::new("/nonexistent-config-xyz/config.toml")).unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn protonhax_wait_defaults_when_the_key_is_absent() {
        let config: AppConfig = toml::from_str("schema_version = 1\n").unwrap();
        assert_eq!(config.protonhax_wait_secs, DEFAULT_PROTONHAX_WAIT_SECS);
    }

    #[test]
    fn game_wait_defaults_when_the_key_is_absent() {
        let config: AppConfig = toml::from_str("schema_version = 1\n").unwrap();
        assert_eq!(config.game_wait_secs, DEFAULT_GAME_WAIT_SECS);
    }

    #[test]
    fn game_wait_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let config = AppConfig {
            game_wait_secs: 45,
            ..AppConfig::default()
        };
        AppConfig::save_to(&path, &config).unwrap();
        assert_eq!(AppConfig::load_from(&path).unwrap(), config);
    }

    #[test]
    fn protonhax_wait_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let config = AppConfig {
            protonhax_wait_secs: 25,
            ..AppConfig::default()
        };
        AppConfig::save_to(&path, &config).unwrap();
        assert_eq!(AppConfig::load_from(&path).unwrap(), config);
    }

    #[test]
    fn config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let config = AppConfig {
            trainers: vec![GlobalTrainer {
                id: "aurora".to_string(),
                name: "Aurora".to_string(),
                path: "/opt/Aurora.exe".to_string(),
            }],
            game_trainers: vec![GameTrainer {
                store: "steam".to_string(),
                game_id: "367520".to_string(),
                name: "FLiNG Hollow Knight".to_string(),
                path: "/opt/fling/hk.exe".to_string(),
            }],
            ..AppConfig::default()
        };
        AppConfig::save_to(&path, &config).unwrap();
        assert_eq!(AppConfig::load_from(&path).unwrap(), config);
    }

    #[test]
    fn rejects_duplicate_trainer_ids() {
        let trainer = GlobalTrainer {
            id: "aurora".to_string(),
            name: "Aurora".to_string(),
            path: "/x.exe".to_string(),
        };
        let config = AppConfig {
            trainers: vec![trainer.clone(), trainer],
            ..AppConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_invalid_entries() {
        assert!(AppConfig {
            trainers: vec![GlobalTrainer {
                id: "Bad ID".to_string(),
                name: "x".to_string(),
                path: "/x".to_string(),
            }],
            ..AppConfig::default()
        }
        .validate()
        .is_err());
        assert!(AppConfig {
            game_trainers: vec![GameTrainer {
                store: "epic".to_string(),
                game_id: "x".to_string(),
                name: "x".to_string(),
                path: "/x".to_string(),
            }],
            ..AppConfig::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn game_trainer_lookup() {
        let config = AppConfig {
            game_trainers: vec![GameTrainer {
                store: "steam".to_string(),
                game_id: "367520".to_string(),
                name: "FLiNG".to_string(),
                path: "/x.exe".to_string(),
            }],
            ..AppConfig::default()
        };
        assert_eq!(config.trainers_for_game("steam", "367520").len(), 1);
        assert!(config.trainers_for_game("heroic", "367520").is_empty());
    }
}

//! Heroic Games Launcher discovery: installed games (Epic via
//! legendary, GOG) and each game's Wine/Proton configuration.
//!
//! Heroic's on-disk layout varies between versions and setups, so every
//! parse is tolerant: a missing file simply contributes no games, and
//! per-game Wine settings are looked up from `GamesConfig` when
//! present. Native and Flatpak installations are both scanned.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::DiscoverError;

/// Where a Heroic game comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeroicStore {
    /// An Epic game managed by legendary.
    Epic,
    /// A GOG game.
    Gog,
    /// A game known only through its GamesConfig entry (configured in
    /// Heroic, but not listed in an installed-games file we know of).
    Configured,
}

/// One Heroic game with everything the launcher needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeroicGame {
    /// Heroic's internal app name (the `heroic launch` argument).
    pub app_name: String,
    /// Human-readable title.
    pub title: String,
    /// Install path, when known.
    pub install_path: Option<PathBuf>,
    /// The game's Wine prefix, when configured.
    pub wine_prefix: Option<PathBuf>,
    /// The Wine or Proton binary the game runs with, when configured.
    pub wine_bin: Option<PathBuf>,
    /// The store the game comes from.
    pub store: HeroicStore,
}

/// Returns the Heroic configuration roots to scan: the native
/// `~/.config/heroic` and, when present, the Flatpak
/// `~/.var/app/com.heroicgameslauncher.hgl/config/heroic`.
///
/// # Errors
///
/// Returns an error when the home directory cannot be determined.
pub fn heroic_roots() -> Result<Vec<PathBuf>, DiscoverError> {
    let home = dirs::home_dir().ok_or(DiscoverError::NoHome)?;
    let mut roots = Vec::new();
    let native = home.join(".config/heroic");
    if native.is_dir() {
        roots.push(native);
    }
    let flatpak = home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic");
    if flatpak.is_dir() {
        roots.push(flatpak);
    }
    Ok(roots)
}

/// Scans every Heroic root and returns the installed games, deduped by
/// app name (native root wins over Flatpak).
///
/// # Errors
///
/// Returns an error only for unexpected filesystem failures; missing
/// Heroic data contributes no games.
pub fn scan_heroic() -> Result<Vec<HeroicGame>, DiscoverError> {
    let mut games: Vec<HeroicGame> = Vec::new();
    for root in heroic_roots()? {
        games.extend(scan_root(&root)?);
    }
    games.sort_by_key(|game| game.title.to_lowercase());
    Ok(dedup_by_app_name(games))
}

/// Scans one Heroic root.
fn scan_root(root: &Path) -> Result<Vec<HeroicGame>, DiscoverError> {
    // Wine settings per app, from GamesConfig/<app_name>.json.
    let game_configs = read_games_configs(root);

    let mut games = Vec::new();

    // Epic games (legendary): installed.json maps app names to info.
    let legendary = root.join("legendaryConfig/legendary/installed.json");
    if let Some(installed) = read_json(&legendary)? {
        let parsed: Option<std::collections::BTreeMap<String, LegendaryInstall>> =
            serde_json::from_value(installed).ok();
        if let Some(installed) = parsed {
            for (app_name, info) in installed {
                let config = game_configs.get(&app_name);
                games.push(HeroicGame {
                    title: info.title,
                    app_name,
                    install_path: Some(PathBuf::from(info.install_path)),
                    wine_prefix: config.and_then(|c| c.wine_prefix.clone()),
                    wine_bin: config.and_then(|c| c.wine_bin.clone()),
                    store: HeroicStore::Epic,
                });
            }
        }
    }

    // GOG games: installed.json is an array of entries, or an object
    // wrapping the array under "installed", depending on the Heroic
    // version.
    let gog = root.join("gog_store/installed.json");
    if let Some(installed) = read_json(&gog)? {
        let entries = match &installed {
            serde_json::Value::Array(_) => Some(installed),
            serde_json::Value::Object(_) => installed.get("installed").cloned(),
            _ => None,
        };
        let parsed: Option<Vec<GogInstall>> =
            entries.and_then(|entries| serde_json::from_value(entries).ok());
        if let Some(installed) = parsed {
            for info in installed {
                let config = game_configs.get(&info.app_name);
                games.push(HeroicGame {
                    title: info.title,
                    app_name: info.app_name,
                    install_path: info.install_path.map(PathBuf::from),
                    wine_prefix: config.and_then(|c| c.wine_prefix.clone()),
                    wine_bin: config.and_then(|c| c.wine_bin.clone()),
                    store: HeroicStore::Gog,
                });
            }
        }
    }

    // Games known only through their configuration: listed by the
    // installed-games files above are skipped here.
    for (app_name, config) in &game_configs {
        if games.iter().any(|game| game.app_name == *app_name) {
            continue;
        }
        // Best-effort title: the prefix folder name, else the app name.
        let title = config
            .wine_prefix
            .as_ref()
            .and_then(|prefix| prefix.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or(app_name)
            .to_string();
        // Titles that mark the entry as a shared Wine-prefix holder
        // rather than a launchable game.
        if PREFIX_HOLDER_TITLES
            .iter()
            .any(|holder| title.eq_ignore_ascii_case(holder))
        {
            continue;
        }
        games.push(HeroicGame {
            app_name: app_name.clone(),
            title,
            install_path: None,
            wine_prefix: config.wine_prefix.clone(),
            wine_bin: config.wine_bin.clone(),
            store: HeroicStore::Configured,
        });
    }

    Ok(games)
}

/// Titles of configuration-only entries that hold a shared Wine
/// prefix instead of being a launchable game. `"shared"` is the
/// conventional name for such a prefix (for example
/// `~/.Games/Heroic/Prefixes/shared`).
const PREFIX_HOLDER_TITLES: &[&str] = &["shared"];

/// The subset of a `GamesConfig/<app>.json` file we care about.
#[derive(Debug, Clone)]
struct GameConfig {
    wine_prefix: Option<PathBuf>,
    wine_bin: Option<PathBuf>,
}

/// Reads every `GamesConfig/*.json` entry that has Wine settings.
fn read_games_configs(root: &Path) -> std::collections::BTreeMap<String, GameConfig> {
    let mut configs = std::collections::BTreeMap::new();
    let dir = root.join("GamesConfig");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return configs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let Some(map) = value.as_object() else {
            continue;
        };
        let app_name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        // Heroic adds "version" and "explicit" keys next to the app
        // name; prefer the key matching the file name, else the first
        // value that actually has Wine settings.
        let settings = map.get(&app_name).or_else(|| {
            map.values().find(|value| {
                value.get("winePrefix").is_some() || value.get("wineVersion").is_some()
            })
        });
        let Some(settings) = settings else {
            continue;
        };
        let wine_prefix = settings
            .get("winePrefix")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);
        let wine_bin = settings
            .get("wineVersion")
            .and_then(|v| v.get("bin"))
            .and_then(|v| v.as_str())
            .map(PathBuf::from);
        if wine_prefix.is_none() && wine_bin.is_none() {
            continue;
        }
        if app_name.is_empty() {
            continue;
        }
        configs.insert(
            app_name,
            GameConfig {
                wine_prefix,
                wine_bin,
            },
        );
    }
    configs
}

/// One legendary installed-game entry.
#[derive(Debug, Deserialize)]
struct LegendaryInstall {
    title: String,
    install_path: String,
}

/// One GOG installed-game entry.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GogInstall {
    app_name: String,
    title: String,
    install_path: Option<String>,
}

/// Reads a JSON file; `Ok(None)` when it does not exist.
fn read_json(path: &Path) -> Result<Option<serde_json::Value>, DiscoverError> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let value = serde_json::from_str(&text).map_err(|source| DiscoverError::Io {
                path: path.to_path_buf(),
                source: std::io::Error::other(source),
            })?;
            Ok(Some(value))
        }
        Err(source) if matches!(source.kind(), std::io::ErrorKind::NotFound) => Ok(None),
        Err(source) => Err(DiscoverError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Keeps the first occurrence of each app name.
fn dedup_by_app_name(games: Vec<HeroicGame>) -> Vec<HeroicGame> {
    let mut seen = std::collections::BTreeSet::new();
    games
        .into_iter()
        .filter(|game| seen.insert(game.app_name.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a synthetic Heroic root mirroring the real layout:
    /// GamesConfig files (with the "version"/"explicit" sibling keys),
    /// legendary and GOG installed lists.
    fn build_root(base: &Path) -> PathBuf {
        let root = base.join("heroic");

        // GamesConfig with the extra top-level keys Heroic writes.
        std::fs::create_dir_all(root.join("GamesConfig")).unwrap();
        std::fs::write(
            root.join("GamesConfig/hLiGQ9ZZAyM9dYYiNcbbVF.json"),
            r#"{
  "hLiGQ9ZZAyM9dYYiNcbbVF": {
    "autoInstallDxvk": true,
    "winePrefix": "/home/tester/Games/Heroic/Prefixes/Guild Wars",
    "wineVersion": {"bin": "/opt/Proton-GE/proton", "name": "Proton-GE Latest", "type": "proton"}
  },
  "version": "v2",
  "explicit": true
}"#,
        )
        .unwrap();

        // A shared-prefix holder: a GamesConfig entry that is not a
        // game and must stay out of the list.
        std::fs::write(
            root.join("GamesConfig/cdr7B2wXzrsqhsKv1wLdwW.json"),
            r#"{
  "cdr7B2wXzrsqhsKv1wLdwW": {
    "winePrefix": "/home/tester/Games/Heroic/Prefixes/shared",
    "wineVersion": {"bin": "/usr/bin/wine", "name": "wine-11.16", "type": "wine"}
  },
  "version": "v2"
}"#,
        )
        .unwrap();

        // An Epic game listed by legendary, with its GamesConfig.
        std::fs::create_dir_all(root.join("legendaryConfig/legendary")).unwrap();
        std::fs::write(
            root.join("legendaryConfig/legendary/installed.json"),
            r#"{
  "Citron": {
    "title": "Citron",
    "install_path": "/mnt/other-disk/Epic/Citron",
    "platform": "Windows"
  }
}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("GamesConfig/Citron.json"),
            r#"{
  "Citron": {
    "winePrefix": "/home/tester/Games/Heroic/Prefixes/Citron",
    "wineVersion": {"bin": "/usr/bin/wine", "name": "Wine-GE", "type": "wine"}
  },
  "version": "v2"
}"#,
        )
        .unwrap();

        // A GOG game without any GamesConfig.
        std::fs::create_dir_all(root.join("gog_store")).unwrap();
        std::fs::write(
            root.join("gog_store/installed.json"),
            r#"[{
  "appName": "1207658924",
  "title": "GOG Game",
  "install_path": "/mnt/other-disk/GOG Game"
}]"#,
        )
        .unwrap();

        root
    }

    #[test]
    fn scans_epic_gog_and_configured_games() {
        let base = tempfile::tempdir().unwrap();
        let root = build_root(base.path());

        let games = scan_root(&root).unwrap();
        let by_app: std::collections::BTreeMap<&str, &HeroicGame> =
            games.iter().map(|g| (g.app_name.as_str(), g)).collect();

        // Epic (legendary) with its Wine settings from GamesConfig.
        let citron = by_app.get("Citron").expect("epic game");
        assert_eq!(citron.title, "Citron");
        assert_eq!(citron.store, HeroicStore::Epic);
        assert_eq!(
            citron.wine_prefix,
            Some(PathBuf::from("/home/tester/Games/Heroic/Prefixes/Citron"))
        );
        assert_eq!(citron.wine_bin, Some(PathBuf::from("/usr/bin/wine")));

        // GOG without configuration.
        let gog = by_app.get("1207658924").expect("gog game");
        assert_eq!(gog.title, "GOG Game");
        assert_eq!(gog.store, HeroicStore::Gog);
        assert!(gog.wine_prefix.is_none());

        // Configured-only game: the "version"/"explicit" sibling keys
        // must not hide the settings (real-world Heroic quirk).
        let guild_wars = by_app
            .get("hLiGQ9ZZAyM9dYYiNcbbVF")
            .expect("configured game");
        assert_eq!(guild_wars.title, "Guild Wars");
        assert_eq!(guild_wars.store, HeroicStore::Configured);
        assert_eq!(
            guild_wars.wine_bin,
            Some(PathBuf::from("/opt/Proton-GE/proton"))
        );

        // The shared-prefix holder must be excluded.
        assert!(!by_app.contains_key("cdr7B2wXzrsqhsKv1wLdwW"));

        assert_eq!(games.len(), 3);
    }

    #[test]
    fn missing_files_contribute_nothing() {
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("heroic");
        assert!(scan_root(&root).unwrap().is_empty());
    }
}

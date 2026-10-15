//! Per-game information for the GUI detail view: the game's real
//! install path and the compatibility tool it runs with.

use std::path::Path;

use serde::Serialize;

use crate::launch::{compat_tool_name, find_proton, primary_steam_root};
use crate::types::{GameRow, GameStore};

/// Information about one game, for the detail view.
#[derive(Debug, Clone, Serialize)]
pub struct GameInfo {
    /// The game's install directory, when known.
    pub install_path: Option<String>,
    /// The compatibility tool the game runs with: the Proton tool
    /// name (Steam's `CompatToolMapping`), or the tool directory /
    /// wine binary name (Heroic).
    pub compat_tool: Option<String>,
    /// The resolved Proton/Wine binary, when found.
    pub runner_bin: Option<String>,
    /// A user-facing recommendation shown when the tool is not a
    /// GE-Proton build: GE-Proton ships more permissive defaults,
    /// which external trainers need.
    pub hint: Option<String>,
}

/// Collects the information for one game.
///
/// All lookups are best effort: a missing Steam root, an unknown
/// tool, or an unparsable app id simply yield `None` fields.
pub fn game_info(game: &GameRow) -> GameInfo {
    match game.store {
        GameStore::Steam => steam_info(game),
        GameStore::Heroic => heroic_info(game),
    }
}

/// The Steam side: the tool mapped to the game in `config.vdf`
/// (per-game override, else Steam's default), resolved through the
/// same Proton lookup the trainer launch uses.
fn steam_info(game: &GameRow) -> GameInfo {
    let mut info = GameInfo {
        install_path: game.install_path.clone(),
        compat_tool: None,
        runner_bin: None,
        hint: None,
    };
    let app_id = game.id.parse::<u32>().unwrap_or(0);
    let Ok(steam_root) = primary_steam_root() else {
        return info;
    };
    info.compat_tool = compat_tool_name(&steam_root.join("config").join("config.vdf"), app_id);
    if let Some(steamapps) = game.steamapps.as_deref() {
        if let Some(proton) = find_proton(&steam_root, Path::new(steamapps), app_id) {
            info.runner_bin = Some(proton.to_string_lossy().to_string());
        }
    }
    if let Some(tool) = &info.compat_tool {
        if !is_ge_proton(tool) {
            info.hint = Some(format!(
                "This game runs with {tool}. GE-Proton is recommended for trainers: it \
                 ships more permissive defaults and better trainer compatibility. \
                 Install it in ~/.local/share/Steam/compatibilitytools.d and select it \
                 for this game (Steam \u{2192} Properties \u{2192} Compatibility)."
            ));
        }
    }
    info
}

/// The Heroic side: the configured Wine/Proton of the game.
fn heroic_info(game: &GameRow) -> GameInfo {
    let mut info = GameInfo {
        install_path: game.install_path.clone(),
        compat_tool: None,
        runner_bin: game.wine_bin.clone(),
        hint: None,
    };
    let Some(wine_bin) = game.wine_bin.as_deref().map(Path::new) else {
        return info;
    };
    info.compat_tool = Some(heroic_tool_name(wine_bin));
    // The GE recommendation only applies to Proton builds — a plain
    // Wine game has nothing to switch to.
    let is_proton = wine_bin
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "proton");
    if is_proton && !is_ge_proton(&info.compat_tool.clone().unwrap_or_default()) {
        info.hint = Some(format!(
            "This game runs with {}. GE-Proton is recommended for trainers: it ships \
             more permissive defaults and better trainer compatibility. Install it in \
             ~/.local/share/Steam/compatibilitytools.d and select it in the game's \
             settings (Heroic \u{2192} game settings \u{2192} Wine).",
            info.compat_tool.clone().unwrap_or_default()
        ));
    }
    info
}

/// The display name of a Heroic Wine/Proton: the tool directory for
/// Proton builds (e.g. `GE-Proton11-7`), else the binary name.
fn heroic_tool_name(wine_bin: &Path) -> String {
    let is_proton = wine_bin
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "proton");
    if is_proton {
        wine_bin
            .parent()
            .and_then(|tool| tool.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| wine_bin.to_string_lossy().to_string())
    } else {
        wine_bin
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| wine_bin.to_string_lossy().to_string())
    }
}

/// Returns `true` when the tool name looks like a GE-Proton build
/// (GE-Proton / Proton-GE naming).
fn is_ge_proton(tool: &str) -> bool {
    let tool = tool.to_ascii_lowercase();
    ["ge-proton", "ge_proton", "proton-ge", "proton_ge"]
        .iter()
        .any(|pattern| tool.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ge_proton_detection() {
        assert!(is_ge_proton("GE-Proton11-7"));
        assert!(is_ge_proton("Proton-GE Latest"));
        assert!(is_ge_proton("ge_proton10-8"));
        assert!(!is_ge_proton("Proton 9.0-4"));
        assert!(!is_ge_proton("cachyos-11.0-20261005-slr-x86_64_v3"));
        assert!(!is_ge_proton("wine-11.16"));
    }

    #[test]
    fn heroic_tool_names() {
        assert_eq!(
            heroic_tool_name(Path::new(
                "/opt/Steam/compatibilitytools.d/GE-Proton11-7/proton"
            )),
            "GE-Proton11-7"
        );
        assert_eq!(heroic_tool_name(Path::new("/usr/bin/wine")), "wine");
    }

    #[test]
    fn heroic_hint_only_for_non_ge_proton() {
        let ge = GameRow {
            store: GameStore::Heroic,
            id: "app".to_string(),
            name: "X".to_string(),
            native_trainer: None,
            wine_prefix: None,
            wine_bin: Some("/opt/tools/GE-Proton11-7/proton".to_string()),
            install_path: Some("/games/x".to_string()),
            steamapps: None,
        };
        assert!(heroic_info(&ge).hint.is_none());

        let official = GameRow {
            store: GameStore::Heroic,
            id: "app".to_string(),
            name: "X".to_string(),
            native_trainer: None,
            wine_prefix: None,
            wine_bin: Some("/opt/tools/cachyos-11.0/proton".to_string()),
            install_path: None,
            steamapps: None,
        };
        let info = heroic_info(&official);
        let hint = info.hint.expect("cachyos proton needs the GE hint");
        assert!(hint.contains("cachyos-11.0"), "hint was: {hint}");

        // Plain wine: no GE recommendation.
        let wine = GameRow {
            store: GameStore::Heroic,
            id: "app".to_string(),
            name: "X".to_string(),
            native_trainer: None,
            wine_prefix: None,
            wine_bin: Some("/usr/bin/wine".to_string()),
            install_path: None,
            steamapps: None,
        };
        assert!(heroic_info(&wine).hint.is_none());
    }
}

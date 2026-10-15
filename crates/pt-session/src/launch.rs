//! Launching games and running external trainers inside the game's
//! Wine/Proton prefix — the launcher half of proton-trainer.
//!
//! proton-trainer is a neutral gateway: it launches the trainer the
//! user points it at, in the game's own prefix, with the game's own
//! Wine/Proton. It bundles and distributes no trainers and adds no
//! anti-cheat policy on top of the user's own choices.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::types::{GameRow, GameStore};

/// How long to wait for a first-run Steam game to create its
/// compatdata prefix.
const PREFIX_WAIT: Duration = Duration::from_secs(120);

/// Poll interval while waiting for a prefix to appear.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Arguments passed to the `steam` client to run one game.
pub fn steam_launch_args(app_id: u32) -> Vec<String> {
    vec![format!("steam://rungameid/{app_id}")]
}

/// Arguments passed to the `heroic` client to run one game.
pub fn heroic_launch_args(app_name: &str) -> Vec<String> {
    vec!["launch".to_string(), app_name.to_string()]
}

/// The command that runs a trainer inside a prefix: Proton takes
/// `run <exe>`, plain wine takes the exe directly.
pub fn trainer_run_args(wine_bin: &Path, trainer: &Path) -> (PathBuf, Vec<PathBuf>) {
    let is_proton = wine_bin
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name == "proton")
        .unwrap_or(false);
    if is_proton {
        (
            wine_bin.to_path_buf(),
            vec!["run".into(), trainer.to_path_buf()],
        )
    } else {
        (wine_bin.to_path_buf(), vec![trainer.to_path_buf()])
    }
}

/// Finds the Proton binary for a Steam game: the tool mapped to the
/// game in `config.vdf` (`CompatToolMapping`), else the newest custom
/// tool in `compatibilitytools.d`, else any official Proton in
/// `steamapps/common`.
///
/// `steamapps` is the game's `steamapps/` directory; the Steam root is
/// its parent.
pub fn find_proton(steamapps: &Path, app_id: u32) -> Option<PathBuf> {
    let steam_root = steamapps.parent()?;

    // 1. The tool mapped to this game in config.vdf.
    let mapped = compat_tool_name(&steam_root.join("config").join("config.vdf"), app_id);
    if let Some(tool) = mapped {
        if let Some(proton) = proton_for_tool(steam_root, &tool) {
            return Some(proton);
        }
    }

    // 2. The newest custom tool (GE-Proton and friends).
    let custom = steam_root.join("compatibilitytools.d");
    if let Ok(entries) = std::fs::read_dir(&custom) {
        let mut tools: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| proton_candidate(path).is_some())
            .collect();
        tools.sort();
        if let Some(latest) = tools.pop() {
            return proton_candidate(&latest);
        }
    }

    // 3. Any official Proton in steamapps/common.
    let common = steamapps.join("common");
    if let Ok(entries) = std::fs::read_dir(&common) {
        let mut tools: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_ascii_lowercase().starts_with("proton"))
                    .unwrap_or(false)
            })
            .collect();
        tools.sort();
        if let Some(latest) = tools.pop() {
            return proton_candidate(&latest);
        }
    }
    None
}

/// The `proton` binary of one compatibility tool directory, checking
/// both the classic root layout and the newer `files/` layout.
fn proton_candidate(tool_dir: &Path) -> Option<PathBuf> {
    [
        tool_dir.join("proton"),
        tool_dir.join("files").join("proton"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

/// Resolves a compatibility tool name to its proton binary.
fn proton_for_tool(steam_root: &Path, tool: &str) -> Option<PathBuf> {
    proton_candidate(&steam_root.join("compatibilitytools.d").join(tool))
        .or_else(|| proton_candidate(&steam_root.join("steamapps").join("common").join(tool)))
}

/// Reads the compat tool name mapped to one app from `config.vdf`.
pub fn compat_tool_name(config_vdf: &Path, app_id: u32) -> Option<String> {
    let text = std::fs::read_to_string(config_vdf).ok()?;
    let root = pt_discover::vdf::parse(&text).ok()?;
    root.get("InstallConfigStore")?
        .get("Software")?
        .get("Valve")?
        .get("Steam")?
        .get("CompatToolMapping")?
        .get(&app_id.to_string())?
        .get("name")?
        .as_str()
        .map(str::to_string)
}

/// Launches a game without any trainer.
///
/// # Errors
///
/// Returns a human-readable message when the store client cannot be
/// started.
pub fn launch_game(game: &GameRow) -> Result<(), String> {
    match game.store {
        GameStore::Steam => {
            let app_id: u32 = game
                .id
                .parse()
                .map_err(|_| format!("invalid Steam app id {:?}", game.id))?;
            let url = steam_launch_args(app_id);
            spawn_detached(&find_client("steam", "com.valvesoftware.Steam"), &url)
        }
        GameStore::Heroic => {
            let args = heroic_launch_args(&game.id);
            spawn_detached(&find_client("heroic", "com.heroicgameslauncher.hgl"), &args)
        }
    }
}

/// Launches a game and then runs the trainer executable inside the
/// game's Wine/Proton prefix.
///
/// For Steam games, the compatdata prefix is reused when it exists
/// (the game ran before); otherwise we wait up to two minutes for
/// Steam to create it after launch. The Proton binary comes from
/// [`find_proton`]. For Heroic games, the per-game `wine_bin` and
/// `wine_prefix` recorded by Heroic are used.
///
/// # Errors
///
/// Returns a human-readable message when the game cannot be launched,
/// the prefix never appears, or no Wine/Proton binary can be found.
pub fn launch_with_trainer(game: &GameRow, trainer: &Path) -> Result<(), String> {
    if !trainer.is_file() {
        return Err(format!("trainer not found: {}", trainer.display()));
    }
    launch_game(game)?;

    match game.store {
        GameStore::Steam => {
            let steamapps = game
                .steamapps
                .as_ref()
                .map(PathBuf::from)
                .ok_or("the game has no known Steam library directory")?;
            let app_id: u32 = game
                .id
                .parse()
                .map_err(|_| format!("invalid Steam app id {:?}", game.id))?;
            let prefix = wait_for_steam_prefix(&steamapps, app_id)?;
            let proton = find_proton(&steamapps, app_id).ok_or_else(|| {
                "no Proton found: install GE-Proton in compatibilitytools.d \
                 or a Proton in steamapps/common"
                    .to_string()
            })?;
            let (program, args) = trainer_run_args(&proton, trainer);
            run_trainer(&program, &args, &prefix)
        }
        GameStore::Heroic => {
            let prefix = game
                .wine_prefix
                .as_ref()
                .map(PathBuf::from)
                .ok_or("this Heroic game has no configured Wine prefix")?;
            let wine_bin = game
                .wine_bin
                .as_ref()
                .map(PathBuf::from)
                .ok_or("this Heroic game has no configured Wine/Proton binary")?;
            let (program, args) = trainer_run_args(&wine_bin, trainer);
            run_trainer(&program, &args, &prefix)
        }
    }
}

/// Waits for `<steamapps>/compatdata/<appid>/pfx` to exist (first
/// launch of a game creates it a few seconds after start).
fn wait_for_steam_prefix(steamapps: &Path, app_id: u32) -> Result<PathBuf, String> {
    let prefix = steamapps
        .join("compatdata")
        .join(app_id.to_string())
        .join("pfx");
    let deadline = Instant::now() + PREFIX_WAIT;
    loop {
        if prefix.is_dir() {
            return Ok(prefix);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "the game's Proton prefix never appeared under {} \
                 (waited {} seconds); is the game running?",
                prefix.display(),
                PREFIX_WAIT.as_secs()
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Runs the trainer inside a prefix, detached from this process.
fn run_trainer(program: &Path, args: &[PathBuf], prefix: &Path) -> Result<(), String> {
    std::process::Command::new(program)
        .env("WINEPREFIX", prefix)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|source| {
            format!(
                "failed to run the trainer with {}: {source}",
                program.display()
            )
        })
}

/// Finds a store client: the native binary, else the Flatpak.
fn find_client(binary: &str, flatpak_id: &str) -> (PathBuf, Vec<String>) {
    if which(binary) {
        (PathBuf::from(binary), Vec::new())
    } else {
        (
            PathBuf::from("flatpak"),
            vec!["run".to_string(), flatpak_id.to_string()],
        )
    }
}

/// Launches a command detached; `prefix_args` precede the URL/args.
fn spawn_detached(command: &(PathBuf, Vec<String>), args: &[String]) -> Result<(), String> {
    std::process::Command::new(&command.0)
        .args(&command.1)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|source| format!("failed to launch with {}: {source}", command.0.display()))
}

/// Returns `true` when the binary is found on PATH.
fn which(binary: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(binary);
        candidate.is_file()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_url_is_wellformed() {
        assert_eq!(steam_launch_args(367520), vec!["steam://rungameid/367520"]);
    }

    #[test]
    fn heroic_args_are_wellformed() {
        assert_eq!(heroic_launch_args("GuildWars"), vec!["launch", "GuildWars"]);
    }

    #[test]
    fn proton_takes_run_wine_takes_exe() {
        let (program, args) =
            trainer_run_args(Path::new("/opt/GE-Proton/proton"), Path::new("/t.exe"));
        assert_eq!(program, Path::new("/opt/GE-Proton/proton"));
        assert_eq!(args, vec![PathBuf::from("run"), PathBuf::from("/t.exe")]);

        let (program, args) = trainer_run_args(Path::new("/usr/bin/wine"), Path::new("/t.exe"));
        assert_eq!(program, Path::new("/usr/bin/wine"));
        assert_eq!(args, vec![PathBuf::from("/t.exe")]);
    }

    #[test]
    fn compat_tool_mapping_reads_from_config_vdf() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.vdf");
        std::fs::write(
            &config,
            "\"InstallConfigStore\"\n{\n\t\"Software\"\n\t{\n\t\t\"Valve\"\n\t\t{\n\t\t\t\"Steam\"\n\t\t\t{\n\t\t\t\t\"CompatToolMapping\"\n\t\t\t\t{\n\t\t\t\t\t\"367520\"\n\t\t\t\t{\n\t\t\t\t\t\t\"name\"\t\t\"GE-Proton10-34\"\n\t\t\t\t\t}\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}\n",
        )
        .unwrap();
        assert_eq!(
            compat_tool_name(&config, 367520),
            Some("GE-Proton10-34".to_string())
        );
        assert_eq!(compat_tool_name(&config, 1), None);
    }

    #[test]
    fn find_proton_prefers_the_mapped_tool() {
        let dir = tempfile::tempdir().unwrap();
        let steam_root = dir.path();
        let steamapps = steam_root.join("steamapps");
        let tool = steam_root.join("compatibilitytools.d/GE-Proton10-34");
        std::fs::create_dir_all(&tool).unwrap();
        std::fs::write(tool.join("proton"), "#!/bin/sh\n").unwrap();
        let other = steamapps.join("common/Proton - Experimental");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("proton"), "#!/bin/sh\n").unwrap();
        std::fs::create_dir_all(steam_root.join("config")).unwrap();
        std::fs::write(
            steam_root.join("config/config.vdf"),
            "\"InstallConfigStore\" { \"Software\" { \"Valve\" { \"Steam\" { \"CompatToolMapping\" { \"42\" { \"name\" \"GE-Proton10-34\" } } } } } }",
        )
        .unwrap();

        assert_eq!(find_proton(&steamapps, 42), Some(tool.join("proton")));
        // Unmapped game falls back to the custom tool.
        assert_eq!(find_proton(&steamapps, 43), Some(tool.join("proton")));
    }

    #[test]
    fn find_proton_handles_files_layout() {
        let dir = tempfile::tempdir().unwrap();
        let steam_root = dir.path();
        let steamapps = steam_root.join("steamapps");
        let tool = steam_root.join("compatibilitytools.d/UMU-Proton/files");
        std::fs::create_dir_all(&tool).unwrap();
        std::fs::write(tool.join("proton"), "#!/bin/sh\n").unwrap();
        assert_eq!(find_proton(&steamapps, 7), Some(tool.join("proton")));
    }

    #[test]
    fn find_proton_returns_none_without_tools() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(find_proton(&dir.path().join("steamapps"), 7), None);
    }

    #[test]
    fn launch_with_trainer_rejects_missing_trainers() {
        let game = GameRow {
            store: GameStore::Steam,
            id: "367520".to_string(),
            name: "X".to_string(),
            native_trainer: None,
            wine_prefix: None,
            wine_bin: None,
            steamapps: None,
        };
        let error = launch_with_trainer(&game, Path::new("/nope/trainer.exe")).unwrap_err();
        assert!(error.contains("trainer not found"));
    }
}

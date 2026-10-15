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

/// Finds the Proton binary for a Steam game. `steam_root` is the
/// primary Steam installation root (custom tools and `config.vdf`
/// live there, never in secondary libraries); `game_steamapps` is the
/// `steamapps/` directory of the library the game is installed in
/// (official Protons can also be installed there).
///
/// Order: the tool mapped to the game in `config.vdf`
/// (`CompatToolMapping`), else the newest custom tool in the primary
/// root's `compatibilitytools.d`, else any official Proton in either
/// `steamapps/common`.
pub fn find_proton(steam_root: &Path, game_steamapps: &Path, app_id: u32) -> Option<PathBuf> {
    // 1. The tool mapped to this game in config.vdf.
    let mapped = compat_tool_name(&steam_root.join("config").join("config.vdf"), app_id);
    if let Some(tool) = mapped {
        if let Some(proton) = proton_for_tool(steam_root, game_steamapps, &tool) {
            return Some(proton);
        }
    }

    // 2. The newest custom tool (GE-Proton and friends) in the
    //    primary root.
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

    // 3. Any official Proton, in the primary root's or the game
    //    library's steamapps/common.
    for common in [
        steam_root.join("steamapps").join("common"),
        game_steamapps.join("common"),
    ] {
        if let Some(proton) = newest_official_proton(&common) {
            return Some(proton);
        }
    }
    None
}

/// The newest `Proton*` tool directory in one `steamapps/common`.
fn newest_official_proton(common: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(common).ok()?;
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
    tools.pop().and_then(|latest| proton_candidate(&latest))
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

/// Resolves a compatibility tool name to its proton binary. Steam
/// stores names that do not always match the directory (e.g.
/// `GE-Proton11-7-x86_64` vs the directory `GE-Proton11-7`), so after
/// the exact matches, a prefix-tolerant lookup runs.
fn proton_for_tool(steam_root: &Path, game_steamapps: &Path, tool: &str) -> Option<PathBuf> {
    if let Some(proton) = proton_candidate(&steam_root.join("compatibilitytools.d").join(tool)) {
        return Some(proton);
    }
    if let Some(proton) = proton_candidate(&steam_root.join("steamapps").join("common").join(tool))
    {
        return Some(proton);
    }
    if let Some(proton) = proton_candidate(&game_steamapps.join("common").join(tool)) {
        return Some(proton);
    }
    // Prefix-tolerant: a directory whose name is a prefix of the tool
    // name, or the reverse.
    let custom = steam_root.join("compatibilitytools.d");
    if let Ok(entries) = std::fs::read_dir(&custom) {
        for entry in entries.flatten() {
            let dir_name = entry.file_name();
            let Some(dir_name) = dir_name.to_str() else {
                continue;
            };
            if tool.starts_with(dir_name) || dir_name.starts_with(tool) {
                if let Some(proton) = proton_candidate(&entry.path()) {
                    return Some(proton);
                }
            }
        }
    }
    None
}

/// Reads the compat tool name mapped to one app from `config.vdf`.
///
/// Games without an explicit override run with Steam's **default**
/// tool, which is stored under the `"0"` key of `CompatToolMapping` —
/// so that key is the fallback. Using anything else than the game's
/// actual tool makes Proton try to upgrade the game's prefix.
pub fn compat_tool_name(config_vdf: &Path, app_id: u32) -> Option<String> {
    let text = std::fs::read_to_string(config_vdf).ok()?;
    let root = pt_discover::vdf::parse(&text).ok()?;
    let mapping = root
        .get("InstallConfigStore")?
        .get("Software")?
        .get("Valve")?
        .get("Steam")?
        .get("CompatToolMapping")?;
    for key in [app_id.to_string(), "0".to_string()] {
        if let Some(name) = mapping
            .get(&key)
            .and_then(|entry| entry.get("name"))
            .and_then(|name| name.as_str())
        {
            return Some(name.to_string());
        }
    }
    None
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
            tracing::info!(app_id, game = %game.name, "launching via steam");
            let url = steam_launch_args(app_id);
            spawn_detached(&find_client("steam", "com.valvesoftware.Steam"), &url)
        }
        GameStore::Heroic => {
            tracing::info!(app = %game.id, game = %game.name, "launching via heroic");
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
            let game_steamapps = game
                .steamapps
                .as_ref()
                .map(PathBuf::from)
                .ok_or("the game has no known Steam library directory")?;
            let app_id: u32 = game
                .id
                .parse()
                .map_err(|_| format!("invalid Steam app id {:?}", game.id))?;
            // The primary Steam installation: custom compatibility
            // tools and config.vdf live there even when the game is
            // installed in a secondary library on another disk.
            let steam_root = primary_steam_root()?;
            let prefix = wait_for_steam_prefix(&game_steamapps, app_id)?;
            tracing::info!(prefix = %prefix.display(), "game prefix located");
            let proton = find_proton(&steam_root, &game_steamapps, app_id).ok_or_else(|| {
                format!(
                    "no Proton found: install GE-Proton in \
                         {}/compatibilitytools.d or a Proton in steamapps/common",
                    steam_root.display()
                )
            })?;
            tracing::info!(proton = %proton.display(), "proton located");
            let compat_data = prefix
                .parent()
                .map(|parent| parent.to_path_buf())
                .unwrap_or_else(|| prefix.clone());
            let steam_env = SteamTrainerEnv {
                compat_data,
                steam_root: steam_root.clone(),
                app_id,
            };
            let (program, args) = trainer_run_args(&proton, trainer);
            run_trainer(&program, &args, &prefix, Some(&steam_env))
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
            tracing::info!(
                wine = %wine_bin.display(),
                prefix = %prefix.display(),
                "heroic wine configuration located"
            );
            let (program, args) = trainer_run_args(&wine_bin, trainer);
            run_trainer(&program, &args, &prefix, None)
        }
    }
}

/// The primary Steam installation root (where compatibility tools and
/// `config.vdf` live).
fn primary_steam_root() -> Result<PathBuf, String> {
    let roots = pt_discover::library::default_steam_roots().map_err(|error| error.to_string())?;
    roots
        .first()
        .map(|root| root.path.clone())
        .ok_or_else(|| "no Steam installation found".to_string())
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

/// The Steam environment a trainer needs in the game's prefix.
#[derive(Debug, Clone)]
pub struct SteamTrainerEnv {
    /// `STEAM_COMPAT_DATA_PATH`: the game's compatdata directory.
    pub compat_data: PathBuf,
    /// `STEAM_COMPAT_CLIENT_INSTALL_PATH`: the primary Steam root.
    pub steam_root: PathBuf,
    /// `STEAM_COMPAT_APP_ID`: the game's app id.
    pub app_id: u32,
}

/// Runs the trainer inside a prefix, detached from this process. The
/// trainer's stdout and stderr are appended to the application log so
/// launch failures are diagnosable after the fact.
fn run_trainer(
    program: &Path,
    args: &[PathBuf],
    prefix: &Path,
    steam_env: Option<&SteamTrainerEnv>,
) -> Result<(), String> {
    let mut command = std::process::Command::new(program);
    command
        .env("WINEPREFIX", prefix)
        .args(args)
        .stdin(std::process::Stdio::null());
    if let Some(env) = steam_env {
        command
            .env("STEAM_COMPAT_DATA_PATH", &env.compat_data)
            .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &env.steam_root)
            .env("STEAM_COMPAT_APP_ID", env.app_id.to_string());
    }
    let mut logged = false;
    if let Ok(log) = crate::logging::log_file() {
        let stdout = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&log);
        let stderr = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&log);
        if let (Ok(stdout), Ok(stderr)) = (stdout, stderr) {
            command.stdout(std::process::Stdio::from(stdout));
            command.stderr(std::process::Stdio::from(stderr));
            logged = true;
        }
    }
    if !logged {
        command.stdout(std::process::Stdio::null());
        command.stderr(std::process::Stdio::null());
    }
    tracing::info!(
        program = %program.display(),
        args = ?args,
        "spawning the trainer"
    );
    command
        .spawn()
        .map(|child| tracing::info!(pid = child.id(), "trainer started"))
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

        assert_eq!(
            find_proton(steam_root, &steamapps, 42),
            Some(tool.join("proton"))
        );
        // Unmapped game falls back to the custom tool.
        assert_eq!(
            find_proton(steam_root, &steamapps, 43),
            Some(tool.join("proton"))
        );
    }

    #[test]
    fn find_proton_handles_files_layout() {
        let dir = tempfile::tempdir().unwrap();
        let steam_root = dir.path();
        let steamapps = steam_root.join("steamapps");
        let tool = steam_root.join("compatibilitytools.d/UMU-Proton/files");
        std::fs::create_dir_all(&tool).unwrap();
        std::fs::write(tool.join("proton"), "#!/bin/sh\n").unwrap();
        assert_eq!(
            find_proton(steam_root, &steamapps, 7),
            Some(tool.join("proton"))
        );
    }

    #[test]
    fn find_proton_returns_none_without_tools() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            find_proton(dir.path(), &dir.path().join("steamapps"), 7),
            None
        );
    }

    #[test]
    fn find_proton_uses_primary_root_for_secondary_library_games() {
        // The game lives in a secondary library with no compatibility
        // tools; the primary root has one.
        let dir = tempfile::tempdir().unwrap();
        let steam_root = dir.path().join("steam-root");
        let game_steamapps = dir.path().join("MP600/SteamLibrary/steamapps");
        let tool = steam_root.join("compatibilitytools.d/GE-Proton10-34");
        std::fs::create_dir_all(&tool).unwrap();
        std::fs::write(tool.join("proton"), "#!/bin/sh\n").unwrap();

        assert_eq!(
            find_proton(&steam_root, &game_steamapps, 99),
            Some(tool.join("proton"))
        );
    }

    #[test]
    fn find_proton_finds_official_proton_in_the_game_library() {
        // No custom tools anywhere; the game's library ships a Proton.
        let dir = tempfile::tempdir().unwrap();
        let steam_root = dir.path().join("steam-root");
        let game_steamapps = dir.path().join("MP600/SteamLibrary/steamapps");
        let proton = game_steamapps.join("common/Proton - Experimental");
        std::fs::create_dir_all(&proton).unwrap();
        std::fs::write(proton.join("proton"), "#!/bin/sh\n").unwrap();

        assert_eq!(
            find_proton(&steam_root, &game_steamapps, 7),
            Some(proton.join("proton"))
        );
    }

    #[test]
    fn compat_tool_name_falls_back_to_the_default_tool() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.vdf");
        std::fs::write(
            &config,
            "\"InstallConfigStore\"\n{\n\t\"Software\"\n\t{\n\t\t\"Valve\"\n\t\t{\n\t\t\t\"Steam\"\n\t\t\t{\n\t\t\t\t\"CompatToolMapping\"\n\t\t\t\t{\n\t\t\t\t\t\"42\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"name\"\t	\"GE-Proton10-34\"\n\t\t\t\t\t}\n\t\t\t\t\t\"0\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"name\"\t	\"GE-Proton11-7-x86_64\"\n\t\t\t\t\t}\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}\n",
        )
        .unwrap();
        // Explicit override wins.
        assert_eq!(
            compat_tool_name(&config, 42),
            Some("GE-Proton10-34".to_string())
        );
        // Unmapped games use the default tool stored under "0".
        assert_eq!(
            compat_tool_name(&config, 2246340),
            Some("GE-Proton11-7-x86_64".to_string())
        );
    }

    #[test]
    fn tool_names_resolve_prefix_tolerantly() {
        // Steam stores "GE-Proton11-7-x86_64"; the directory is
        // "GE-Proton11-7".
        let dir = tempfile::tempdir().unwrap();
        let steam_root = dir.path();
        let steamapps = steam_root.join("steamapps");
        let tool = steam_root.join("compatibilitytools.d/GE-Proton11-7");
        std::fs::create_dir_all(&tool).unwrap();
        std::fs::write(tool.join("proton"), "#!/bin/sh\n").unwrap();

        let proton = proton_for_tool(steam_root, &steamapps, "GE-Proton11-7-x86_64");
        assert_eq!(proton, Some(tool.join("proton")));
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

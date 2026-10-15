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

/// The argument that makes Heroic run a game. Heroic 2.22+ does not
/// parse a `launch <app>` CLI command: launches are requested through
/// the `heroic://` protocol URL scanned from the process arguments.
/// `gui=false` keeps the Heroic window hidden — this is a launcher.
pub fn heroic_launch_args(app_name: &str) -> Vec<String> {
    vec![format!("heroic://launch?appName={app_name}&gui=false")]
}

/// The command that runs a trainer inside a prefix: Proton takes
/// `run <exe>`, plain wine takes the exe directly.
pub fn trainer_run_args(wine_bin: &Path, trainer: &Path) -> (PathBuf, Vec<PathBuf>) {
    if is_proton(wine_bin) {
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
            spawn_logged(&find_client("heroic", "com.heroicgameslauncher.hgl"), &args)
        }
    }
}

/// Validates a Heroic game's Wine/Proton configuration: the binary
/// must be known AND exist — Heroic keeps absolute paths in
/// `GamesConfig`, and a deleted Proton build (e.g. an updated
/// `Proton-GE Latest`) silently breaks every launch until the user
/// re-selects a version in Heroic's game settings.
///
/// # Errors
///
/// Returns a human-readable, actionable message when the binary is
/// missing or does not exist.
fn validate_heroic_wine(game: &GameRow) -> Result<(), String> {
    let wine_bin = game
        .wine_bin
        .as_deref()
        .ok_or("this Heroic game has no configured Wine/Proton binary")?;
    let wine_bin = Path::new(wine_bin);
    if !wine_bin.is_file() {
        return Err(format!(
            "this Heroic game is configured with a Wine/Proton that no \
             longer exists: {}. Re-select a working version in Heroic \
             (game settings \u{2192} Wine) and try again",
            wine_bin.display()
        ));
    }
    Ok(())
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
///
/// On success the `Ok` value is an optional user-facing warning: an
/// empty string means everything went the intended way, a non-empty
/// string explains a fallback the user should know about.
pub fn launch_with_trainer(game: &GameRow, trainer: &Path) -> Result<String, String> {
    if !trainer.is_file() {
        return Err(format!("trainer not found: {}", trainer.display()));
    }
    tracing::info!(
        game = %game.name,
        trainer = %trainer.display(),
        "launching game with trainer"
    );

    // Steam games must be RUNNING before the trainer enters the
    // prefix: the trainer's own Wine session races the game's launch
    // otherwise. While the game has not appeared, the launch URL is
    // re-sent every 20 seconds (Steam ignores duplicates) to force
    // the launch — everything stays in the game's own prefix.
    if game.store == GameStore::Steam {
        let app_id: u32 = game
            .id
            .parse()
            .map_err(|_| format!("invalid Steam app id {:?}", game.id))?;
        if steam_game_running(app_id) {
            tracing::info!(app_id, "game already running");
        } else {
            launch_game(game)?;
            let launched = wait_for_steam_game(app_id, || {
                let _ = spawn_detached(
                    &find_client("steam", "com.valvesoftware.Steam"),
                    &steam_launch_args(app_id),
                );
            });
            if !launched {
                tracing::warn!(
                    app_id,
                    "game did not appear to start; launching the trainer anyway"
                );
            }
        }
    } else {
        // Fail fast: a missing Wine/Proton binary cannot launch the
        // trainer either, so do not even start the game.
        validate_heroic_wine(game)?;
        launch_game(game)?;
    }

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
            // Steam Proton games run inside a pressure-vessel/bwrap
            // container (Steam Linux Runtime). A trainer spawned
            // outside it cannot join the game's wineserver and dies
            // within seconds, even in the same prefix. protonhax
            // (started by the game itself via `protonhax init
            // %command%` in its Steam launch options) lets us run the
            // trainer INSIDE the game's container. When protonhax is
            // not usable we fall back to a direct prefix run — which
            // works only outside containers — and tell the user.
            if which("protonhax") {
                let wait_secs = crate::config::AppConfig::load()
                    .map(|config| config.protonhax_wait_secs)
                    .unwrap_or_else(|error| {
                        tracing::warn!(
                            %error,
                            "could not read the config; using the default protonhax wait"
                        );
                        crate::config::DEFAULT_PROTONHAX_WAIT_SECS
                    });
                tracing::info!(
                    app_id,
                    seconds = wait_secs,
                    "waiting for the game's protonhax environment"
                );
                if wait_for_protonhax_env(app_id, Duration::from_secs(wait_secs)) {
                    tracing::info!(
                        app_id,
                        "protonhax environment ready; joining the game's container"
                    );
                    let (program, args) = protonhax_run_args(app_id, trainer);
                    // protonhax sources WINEPREFIX and the whole
                    // Proton environment from the env dir the game
                    // exported; setting our own would fight it.
                    run_trainer(&program, &args, &TrainerEnv::Managed)?;
                    return Ok(String::new());
                }
                tracing::warn!(
                    app_id,
                    launch_options = ?steam_launch_options(&steam_root, app_id),
                    "protonhax is installed but the game exported no environment; \
                     is `protonhax init %command%` in its Steam launch options? \
                     Falling back to a direct prefix run"
                );
            } else {
                tracing::warn!(
                    "protonhax not found on PATH; the trainer will run in the \
                     prefix directly and may not survive Steam's container"
                );
            }
            let compat_data = prefix
                .parent()
                .map(|parent| parent.to_path_buf())
                .unwrap_or_else(|| prefix.clone());
            let (program, args) = trainer_run_args(&proton, trainer);
            run_trainer(
                &program,
                &args,
                &TrainerEnv::Proton {
                    compat_data,
                    steam_root: steam_root.clone(),
                    app_id,
                },
            )?;
            Ok(protonhax_hint(&steam_root, app_id))
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
            // The game must be RUNNING before the trainer starts:
            // Heroic launches games through umu inside a
            // pressure-vessel container, and the game's wineserver
            // socket is the reliable "it is up" signal. A trainer
            // started earlier still worked on paper, but raced the
            // game's launch and blurred every diagnosis so far.
            // For Proton, the Wine prefix is `<compatdata>/pfx`.
            let wine_prefix_dir = if is_proton(&wine_bin) {
                prefix.join("pfx")
            } else {
                prefix.clone()
            };
            // The trainer must CONNECT to the game's wineserver, never
            // create or wait for a competing one. Heroic launches
            // games through umu: its chain reaches
            // `proton waitforexitandrun <game exe>` exactly when the
            // game starts — after the prefix setup — so that is the
            // moment we wait for. The trainer then runs through
            // `<proton> run`, whose wine connects to the LIVE server
            // of the game as a plain client (verified live: game and
            // trainer coexist on ONE shared server). Anything spawned
            // through umu-run instead runs `waitforexitandrun` too,
            // which blocks in `wineserver -w` — waiting for the
            // game's server to die — so the trainer would only start
            // after the game closes.
            let is_proton = is_proton(&wine_bin);
            let wait = Duration::from_secs(configured_game_wait_secs());
            if is_proton {
                let running =
                    wait_for_heroic_game_exe(game.wine_prefix.as_deref(), wait, POLL_INTERVAL);
                if !running {
                    return Err(format!(
                        "the game executable did not start within {wait_secs} seconds. Raise \
                         the game start wait in the Configuration tab, or launch the game \
                         separately and retry — starting the trainer first would block the \
                         game until the trainer closes",
                        wait_secs = wait.as_secs()
                    ));
                }
            } else if !wait_for_wineserver(&wine_prefix_dir, wait, POLL_INTERVAL) {
                return Err(format!(
                    "the game's wineserver never appeared within {wait_secs} seconds. \
                     Raise the game start wait in the Configuration tab, or launch the \
                     game separately and retry",
                    wait_secs = wait.as_secs()
                ));
            }
            // Heroic's `winePrefix` for a Proton game is a compatdata
            // directory — the Wine prefix is `<dir>/pfx`, created by
            // Proton — so the trainer needs the full Steam compat
            // environment, exactly like a Steam game. Plain-Wine
            // Heroic games keep the WINEPREFIX flow.
            let env = if is_proton {
                let steam_root = steam_client_root(&wine_bin)
                    .or_else(|| primary_steam_root().ok())
                    .ok_or("could not determine the Steam client path for this Proton")?;
                TrainerEnv::Proton {
                    compat_data: prefix,
                    steam_root,
                    app_id: 0,
                }
            } else {
                TrainerEnv::Wine { prefix }
            };
            let (program, args) = trainer_run_args(&wine_bin, trainer);
            run_trainer(&program, &args, &env)?;
            Ok(String::new())
        }
    }
}

/// The user-facing hint shown when the trainer could not be run
/// inside the game's container and a direct prefix run was used
/// instead (likely to die on Steam Linux Runtime games). Names the
/// game's actual Steam launch options when they are known, so the
/// exact fix is obvious.
fn protonhax_hint(steam_root: &Path, app_id: u32) -> String {
    let options = steam_launch_options(steam_root, app_id);
    let suggestion = match options.as_deref() {
        Some(options) if options.contains("%command%") => {
            options.replace("%command%", "protonhax init %command%")
        }
        Some(options) if !options.is_empty() => {
            format!("{options} protonhax init %command%")
        }
        _ => "protonhax init %command%".to_string(),
    };
    format!(
        "The trainer was started in the game's prefix directly and may not \
         survive the game's Steam Linux Runtime container. Set this game's \
         Steam launch options (Steam \u{2192} Properties \u{2192} Launch Options) to \
         `{suggestion}` and relaunch, so the trainer can join the game's \
         container."
    )
}

/// The game's launch options as configured in Steam, read from
/// `userdata/<uid>/config/localconfig.vdf` under the Steam root.
/// Returns `None` when unknown (missing file or entry, or the game
/// has no launch options).
pub fn steam_launch_options(steam_root: &Path, app_id: u32) -> Option<String> {
    let userdata = steam_root.join("userdata");
    let users = std::fs::read_dir(userdata).ok()?;
    for user in users.flatten() {
        let vdf = user.path().join("config").join("localconfig.vdf");
        let Ok(text) = std::fs::read_to_string(vdf) else {
            continue;
        };
        let Ok(root) = pt_discover::vdf::parse(&text) else {
            continue;
        };
        let options = root
            .get("UserLocalConfigStore")
            .and_then(|v| v.get("Software"))
            .and_then(|v| v.get("Valve"))
            .and_then(|v| v.get("Steam"))
            .and_then(|v| v.get("apps"))
            .and_then(|v| v.get(&app_id.to_string()))
            .and_then(|v| v.get("LaunchOptions"))
            .and_then(|v| v.as_str());
        if let Some(options) = options {
            return Some(options.to_string());
        }
    }
    None
}

/// Poll interval while waiting for the protonhax env dir.
const PROTONHAX_POLL: Duration = Duration::from_secs(1);

/// The command that runs a trainer inside the game's container:
/// `protonhax run <appid> <exe>`.
pub fn protonhax_run_args(app_id: u32, trainer: &Path) -> (PathBuf, Vec<PathBuf>) {
    (
        PathBuf::from("protonhax"),
        vec![
            "run".into(),
            app_id.to_string().into(),
            trainer.to_path_buf(),
        ],
    )
}

/// The env directory protonhax writes when a game starts with
/// `protonhax init %command%`: `<XDG_RUNTIME_DIR>/protonhax/<appid>`.
/// `runtime_dir` is the value of `XDG_RUNTIME_DIR`.
pub fn protonhax_env_dir(runtime_dir: &Path, app_id: u32) -> PathBuf {
    runtime_dir.join("protonhax").join(app_id.to_string())
}

/// Waits until the game's protonhax env dir exists (it is written by
/// `protonhax init` while the game launches). Returns whether it did.
/// `wait` comes from the user configuration
/// ([`crate::config::AppConfig::protonhax_wait_secs`]).
fn wait_for_protonhax_env(app_id: u32, wait: Duration) -> bool {
    let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return false;
    };
    wait_for_dir_with(
        &protonhax_env_dir(Path::new(&runtime_dir), app_id),
        wait,
        PROTONHAX_POLL,
    )
}

/// The testable core of [`wait_for_protonhax_env`]: polls until `dir`
/// exists or the deadline passes.
fn wait_for_dir_with(dir: &Path, wait: Duration, poll: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        if dir.is_dir() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(poll);
    }
}

/// The primary Steam installation root (where compatibility tools and
/// `config.vdf` live).
pub(crate) fn primary_steam_root() -> Result<PathBuf, String> {
    let roots = pt_discover::library::default_steam_roots().map_err(|error| error.to_string())?;
    roots
        .first()
        .map(|root| root.path.clone())
        .ok_or_else(|| "no Steam installation found".to_string())
}

/// Interval between checks for the game's process.
const GAME_POLL: Duration = Duration::from_secs(2);

/// How often the launch URL is re-sent while the game has not
/// appeared (Steam ignores duplicates; this forces the launch).
const GAME_RETRY: Duration = Duration::from_secs(20);

/// The configured max wait for a launched game to actually start
/// ([`crate::config::AppConfig::game_wait_secs`]); falls back to
/// the default when the config cannot be read.
fn configured_game_wait_secs() -> u64 {
    crate::config::AppConfig::load()
        .map(|config| config.game_wait_secs)
        .unwrap_or_else(|error| {
            tracing::warn!(
                %error,
                "could not read the config; using the default game wait"
            );
            crate::config::DEFAULT_GAME_WAIT_SECS
        })
}

/// Returns `true` when one argv slice matches Steam's game-launch
/// reaper: `[.., "SteamLaunch", "AppId=<id>", ..]`.
fn is_reaper_argv(argv: &[&str], app_id: u32) -> bool {
    let wanted = format!("AppId={app_id}");
    argv.windows(2)
        .any(|pair| pair[0] == "SteamLaunch" && pair[1] == wanted)
}

/// Returns `true` when Steam's reaper for `app_id` is running — the
/// reliable signal that the game has actually started (used by the
/// classic trainer launchers).
pub fn steam_game_running(app_id: u32) -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<i32>().ok())
        else {
            continue;
        };
        let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{pid}/cmdline")) else {
            continue;
        };
        let argv: Vec<&str> = cmdline
            .split('\0')
            .filter(|part| !part.is_empty())
            .collect();
        if is_reaper_argv(&argv, app_id) {
            return true;
        }
    }
    false
}

/// Returns `true` when a wineserver is running for the Wine prefix
/// directory: wine names its socket
/// `<tmp>/.wine-<uid>/server-<dev>-<inode>/socket`, where `dev` and
/// `inode` are those of the prefix directory (verified against a
/// live GE-Proton session). The `socket` file exists exactly while
/// the server is alive — the definitive signal that the game's wine
/// owns the prefix, and that a trainer connecting now will be a
/// plain client rather than a competing server.
pub fn wineserver_running(prefix: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(meta) = std::fs::metadata(prefix) else {
        return false;
    };
    let name = format!("server-{:x}-{:x}", meta.dev(), meta.ino());
    for root in wine_socket_roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let is_wine_root = entry
                .file_name()
                .to_str()
                .map(|name| name.starts_with(".wine-"))
                .unwrap_or(false);
            if is_wine_root && entry.path().join(&name).join("socket").exists() {
                return true;
            }
        }
    }
    false
}

/// The directories wine keeps its `.wine-<uid>` socket roots in:
/// `$TMPDIR` (when set) and `/tmp`.
fn wine_socket_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        if !tmpdir.is_empty() {
            roots.push(PathBuf::from(tmpdir));
        }
    }
    roots.push(PathBuf::from("/tmp"));
    roots
}

/// Returns `true` when a command line is Heroic's launch chain
/// reaching the game executable: the umu/proton chain runs
/// `proton waitforexitandrun <game exe>` exactly when the game
/// starts — after the prefix setup. For sideloaded games the exe
/// lives under the Wine prefix, which makes the match specific;
/// without a prefix any `waitforexitandrun` matches (store games
/// keep their exe outside the prefix).
fn cmdline_is_game_exe(cmdline: &str, wine_prefix: Option<&str>) -> bool {
    if !cmdline.contains("waitforexitandrun") {
        return false;
    }
    match wine_prefix.filter(|prefix| !prefix.is_empty()) {
        Some(prefix) => cmdline.contains(prefix),
        None => true,
    }
}

/// Returns `true` when Heroic's launch chain has reached the game
/// executable: the umu/proton chain runs
/// `proton waitforexitandrun <game exe>` exactly when the game
/// starts — after the prefix setup. The `wine_prefix` makes the
/// match specific when the game executable lives under it.
pub fn heroic_game_exe_running(wine_prefix: Option<&str>) -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<i32>().ok())
        else {
            continue;
        };
        let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{pid}/cmdline")) else {
            continue;
        };
        if cmdline_is_game_exe(&cmdline, wine_prefix) {
            return true;
        }
    }
    false
}

/// Waits until the Heroic game executable starts. Returns whether
/// it did.
fn wait_for_heroic_game_exe(wine_prefix: Option<&str>, wait: Duration, poll: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        if heroic_game_exe_running(wine_prefix) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(poll);
    }
}

/// Waits until the game's wineserver owns the prefix. Returns
/// whether it did.
fn wait_for_wineserver(prefix: &Path, wait: Duration, poll: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        if wineserver_running(prefix) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(poll);
    }
}

/// Waits until the game's reaper appears, re-sending the launch URL
/// every [`GAME_RETRY`] to force the launch. Returns whether the game
/// is running.
fn wait_for_steam_game(app_id: u32, resend: impl Fn()) -> bool {
    let wait = Duration::from_secs(configured_game_wait_secs());
    wait_for_steam_game_with(app_id, resend, wait, GAME_POLL, GAME_RETRY)
}

/// The testable core of [`wait_for_steam_game`]: polls
/// [`steam_game_running`] until the deadline, re-sending the launch
/// URL every `retry` while the game has not appeared.
fn wait_for_steam_game_with(
    app_id: u32,
    resend: impl Fn(),
    wait: Duration,
    poll: Duration,
    retry: Duration,
) -> bool {
    let deadline = Instant::now() + wait;
    let mut next_retry = Instant::now() + retry;
    loop {
        if steam_game_running(app_id) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        if Instant::now() >= next_retry {
            tracing::info!(app_id, "game not started yet; re-sending the launch URL");
            resend();
            next_retry = Instant::now() + retry;
        }
        std::thread::sleep(poll);
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

/// The Wine/Proton environment a trainer is spawned into.
#[derive(Debug, Clone)]
pub enum TrainerEnv {
    /// Plain Wine: `WINEPREFIX` points at the prefix directory.
    Wine {
        /// The Wine prefix directory.
        prefix: PathBuf,
    },
    /// Proton (Steam or Heroic): Proton ignores `WINEPREFIX` and
    /// EXITS with "No compat data path?" unless
    /// `STEAM_COMPAT_DATA_PATH` is set — it derives the Wine prefix
    /// as `<compat_data>/pfx`. The client path and app id complete
    /// the environment Steam normally provides (Proton reads the
    /// client path when writing prefix configuration).
    Proton {
        /// `STEAM_COMPAT_DATA_PATH`: the compatdata directory.
        compat_data: PathBuf,
        /// `STEAM_COMPAT_CLIENT_INSTALL_PATH`: the Steam root.
        steam_root: PathBuf,
        /// `STEAM_COMPAT_APP_ID`: the game's app id (0 when none).
        app_id: u32,
    },
    /// protonhax: the runner sources its own environment from the
    /// env dir the game exported; nothing must be overridden.
    Managed,
}

/// Returns `true` when `wine_bin` is a Proton launcher (it takes
/// `run <exe>` and requires the Steam compat environment).
fn is_proton(wine_bin: &Path) -> bool {
    wine_bin
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name == "proton")
        .unwrap_or(false)
}

/// The Steam client root a Proton tool belongs to:
/// `<root>/compatibilitytools.d/<tool>/proton` → `<root>`.
/// Used for `STEAM_COMPAT_CLIENT_INSTALL_PATH`.
fn steam_client_root(proton: &Path) -> Option<PathBuf> {
    let compat_dir = proton.parent()?.parent()?;
    if compat_dir.file_name().and_then(|name| name.to_str()) == Some("compatibilitytools.d") {
        compat_dir.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

/// Runs the trainer detached from this process. The trainer's stdout
/// and stderr are appended to the application log so launch failures
/// are diagnosable after the fact.
fn run_trainer(program: &Path, args: &[PathBuf], env: &TrainerEnv) -> Result<(), String> {
    let mut command = std::process::Command::new(program);
    command.args(args).stdin(std::process::Stdio::null());
    match env {
        TrainerEnv::Wine { prefix } => {
            command.env("WINEPREFIX", prefix);
        }
        TrainerEnv::Proton {
            compat_data,
            steam_root,
            app_id,
        } => {
            command
                .env("STEAM_COMPAT_DATA_PATH", compat_data)
                .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", steam_root)
                .env("STEAM_COMPAT_APP_ID", app_id.to_string());
        }
        TrainerEnv::Managed => {}
    }
    attach_log_stdio(&mut command);
    tracing::info!(
        program = %program.display(),
        args = ?args,
        env = ?env,
        "spawning the trainer"
    );
    command
        .spawn()
        .map(|child| tracing::info!(pid = child.id(), "trainer started"))
        .inspect_err(|error| {
            tracing::error!(%error, program = %program.display(), "failed to spawn the trainer")
        })
        .map_err(|source| {
            format!(
                "failed to run the trainer with {}: {source}",
                program.display()
            )
        })
}

/// Appends the child's stdout and stderr to the application log;
/// falls back to null stdio when the log file is unavailable.
fn attach_log_stdio(command: &mut std::process::Command) {
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
            return;
        }
    }
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());
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
///
/// `ELECTRON_RUN_AS_NODE` is stripped: it leaks from IDE terminals
/// (VS Code exports it) and turns Electron-based clients such as
/// Heroic into plain Node, where they crash with
/// "Cannot find module 'electron'" instead of launching the game.
fn spawn_detached(command: &(PathBuf, Vec<String>), args: &[String]) -> Result<(), String> {
    std::process::Command::new(&command.0)
        .args(&command.1)
        .args(args)
        .env_remove("ELECTRON_RUN_AS_NODE")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|source| format!("failed to launch with {}: {source}", command.0.display()))
}

/// Launches a store client detached with its output appended to the
/// application log, so a failed launch (a Heroic CLI error, a missing
/// Wine binary in Heroic's own settings, ...) is diagnosable after
/// the fact. [`spawn_detached`]'s Electron note applies here too.
fn spawn_logged(client: &(PathBuf, Vec<String>), args: &[String]) -> Result<(), String> {
    let mut command = std::process::Command::new(&client.0);
    command
        .args(&client.1)
        .args(args)
        .env_remove("ELECTRON_RUN_AS_NODE")
        .stdin(std::process::Stdio::null());
    attach_log_stdio(&mut command);
    tracing::info!(
        program = %client.0.display(),
        args = ?args,
        "spawning the store client"
    );
    command
        .spawn()
        .map(|child| tracing::info!(pid = child.id(), "store client spawned"))
        .inspect_err(|error| {
            tracing::error!(%error, program = %client.0.display(), "failed to spawn the client")
        })
        .map_err(|source| {
            format!(
                "failed to launch with {}: {source}",
                client.0.display()
            )
        })
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
        assert_eq!(
            heroic_launch_args("GuildWars"),
            vec!["heroic://launch?appName=GuildWars&gui=false"]
        );
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
    fn reaper_argv_matching() {
        let argv = [
            "/home/reaper",
            "SteamLaunch",
            "AppId=2246340",
            "--",
            "bwrap",
        ];
        assert!(is_reaper_argv(&argv, 2246340));
        // App id prefix must not match other ids.
        assert!(!is_reaper_argv(&argv, 224634));
        assert!(!is_reaper_argv(&argv, 2246341));
        // Unrelated processes.
        assert!(!is_reaper_argv(&["steam", "webhelper"], 2246340));
    }

    #[test]
    fn wait_for_game_times_out_and_resends() {
        // An app id with no reaper running: the wait must time out
        // and re-send the launch URL on every retry interval.
        let resends = std::sync::atomic::AtomicUsize::new(0);
        let started = Instant::now();
        let running = wait_for_steam_game_with(
            999_999_123,
            || {
                resends.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            },
            Duration::from_millis(40),
            Duration::from_millis(5),
            Duration::from_millis(10),
        );
        assert!(!running, "no such game can be running");
        let elapsed = started.elapsed();
        assert!(
            elapsed >= Duration::from_millis(40),
            "waited out the full window"
        );
        assert!(
            resends.load(std::sync::atomic::Ordering::SeqCst) >= 2,
            "the launch URL was re-sent"
        );
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
            install_path: None,
            steamapps: None,
        };
        let error = launch_with_trainer(&game, Path::new("/nope/trainer.exe")).unwrap_err();
        assert!(error.contains("trainer not found"));
    }

    #[test]
    fn protonhax_args_are_wellformed() {
        let (program, args) = protonhax_run_args(2246340, Path::new("/tmp/Aurora.exe"));
        assert_eq!(program, PathBuf::from("protonhax"));
        assert_eq!(
            args,
            vec![
                PathBuf::from("run"),
                PathBuf::from("2246340"),
                PathBuf::from("/tmp/Aurora.exe"),
            ]
        );
    }

    #[test]
    fn protonhax_env_dir_is_under_the_runtime_dir() {
        let dir = protonhax_env_dir(Path::new("/run/user/1000"), 42);
        assert_eq!(dir, PathBuf::from("/run/user/1000/protonhax/42"));
    }

    #[test]
    fn wait_for_dir_times_out_on_a_missing_dir() {
        let missing = std::env::temp_dir().join("pt-session-missing-env-dir");
        let started = Instant::now();
        let found = wait_for_dir_with(
            &missing,
            Duration::from_millis(30),
            Duration::from_millis(5),
        );
        assert!(!found);
        assert!(started.elapsed() >= Duration::from_millis(30));
    }

    #[test]
    fn wait_for_dir_returns_immediately_for_an_existing_dir() {
        let existing = std::env::temp_dir();
        assert!(wait_for_dir_with(
            &existing,
            Duration::from_millis(30),
            Duration::from_millis(5)
        ));
    }

    /// Writes a `localconfig.vdf` with the given launch options for
    /// app 2246340 under a fake Steam root.
    fn steam_root_with_launch_options(options: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("userdata/1/config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("localconfig.vdf"),
            format!(
                "\"UserLocalConfigStore\"\n{{\n\t\"Software\"\n\t{{\n\t\t\"Valve\"\n\t\t{{\n\t\t\t\
                 \"Steam\"\n\t\t\t{{\n\t\t\t\t\"apps\"\n\t\t\t\t{{\n\t\t\t\t\t\"2246340\"\n\t\t\t\t\t\
                 {{\n\t\t\t\t\t\t\"LaunchOptions\"\t\t\"{options}\"\n\t\t\t\t\t}}\n\t\t\t\t}}\n\t\t\t\
                 }}\n\t\t}}\n\t}}\n}}\n"
            ),
        )
        .unwrap();
        dir
    }

    #[test]
    fn steam_launch_options_reads_localconfig() {
        let dir = steam_root_with_launch_options("PROTON_FSR4_UPGRADE=1 %command%");
        assert_eq!(
            steam_launch_options(dir.path(), 2246340).as_deref(),
            Some("PROTON_FSR4_UPGRADE=1 %command%")
        );
        assert_eq!(steam_launch_options(dir.path(), 367520), None);
    }

    #[test]
    fn steam_launch_options_is_none_without_steam() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(steam_launch_options(dir.path(), 2246340), None);
    }

    #[test]
    fn protonhax_hint_suggests_the_exact_new_options() {
        let dir = steam_root_with_launch_options("PROTON_FSR4_UPGRADE=1 %command%");
        let hint = protonhax_hint(dir.path(), 2246340);
        assert!(
            hint.contains("`PROTON_FSR4_UPGRADE=1 protonhax init %command%`"),
            "hint was: {hint}"
        );
    }

    #[test]
    fn protonhax_hint_falls_back_to_the_plain_command() {
        let dir = tempfile::tempdir().unwrap();
        let hint = protonhax_hint(dir.path(), 2246340);
        assert!(
            hint.contains("`protonhax init %command%`"),
            "hint was: {hint}"
        );
    }

    #[test]
    fn heroic_wine_validation_rejects_deleted_binaries() {
        let game = GameRow {
            store: GameStore::Heroic,
            id: "app".to_string(),
            name: "X".to_string(),
            native_trainer: None,
            wine_prefix: Some("/prefix".to_string()),
            wine_bin: Some("/nonexistent/proton".to_string()),
            install_path: None,
            steamapps: None,
        };
        let error = validate_heroic_wine(&game).unwrap_err();
        assert!(error.contains("no longer exists"), "error was: {error}");
        assert!(error.contains("/nonexistent/proton"), "error was: {error}");
    }

    #[test]
    fn heroic_wine_validation_accepts_existing_binaries() {
        let dir = tempfile::tempdir().unwrap();
        let wine_bin = dir.path().join("proton");
        std::fs::write(&wine_bin, b"#!/bin/sh\n").unwrap();
        let game = GameRow {
            store: GameStore::Heroic,
            id: "app".to_string(),
            name: "X".to_string(),
            native_trainer: None,
            wine_prefix: Some("/prefix".to_string()),
            wine_bin: Some(wine_bin.to_string_lossy().into_owned()),
            install_path: None,
            steamapps: None,
        };
        assert!(validate_heroic_wine(&game).is_ok());
    }

    #[test]
    fn heroic_wine_validation_requires_a_binary() {
        let game = GameRow {
            store: GameStore::Heroic,
            id: "app".to_string(),
            name: "X".to_string(),
            native_trainer: None,
            wine_prefix: Some("/prefix".to_string()),
            wine_bin: None,
            install_path: None,
            steamapps: None,
        };
        assert!(validate_heroic_wine(&game)
            .unwrap_err()
            .contains("no configured"));
    }

    #[test]
    fn steam_client_root_is_derived_from_the_tool_layout() {
        let proton =
            Path::new("/home/x/.local/share/Steam/compatibilitytools.d/GE-Proton11-7/proton");
        assert_eq!(
            steam_client_root(proton).as_deref(),
            Some(Path::new("/home/x/.local/share/Steam"))
        );
        assert_eq!(steam_client_root(Path::new("/usr/bin/wine")), None);
    }

    #[test]
    fn wineserver_detection_requires_an_existing_prefix() {
        assert!(!wineserver_running(Path::new(
            "/nonexistent-prefix-xyz/pfx"
        )));
    }

    #[test]
    fn heroic_game_exe_detection() {
        let prefix = "/prefixes/Guild Wars";
        // the game executable starting: umu's proton chain
        assert!(cmdline_is_game_exe(
            "/usr/bin/python3\0/opt/GE-Proton/proton\0waitforexitandrun\0/prefixes/Guild Wars/pfx/drive_c/Gw.exe\0",
            Some(prefix)
        ));
        // the prefix setup phase: NOT the game yet
        assert!(!cmdline_is_game_exe(
            "/usr/bin/python3\0/opt/GE-Proton/proton\0run\0wineboot\0",
            Some(prefix)
        ));
        // another game's executable (different prefix)
        assert!(!cmdline_is_game_exe(
            "/usr/bin/python3\0/opt/GE-Proton/proton\0waitforexitandrun\0/other/game.exe\0",
            Some(prefix)
        ));
        // no prefix configured: any game start matches
        assert!(cmdline_is_game_exe(
            "/usr/bin/python3\0/opt/GE-Proton/proton\0waitforexitandrun\0/other/game.exe\0",
            None
        ));
        assert!(!cmdline_is_game_exe("something\0else\0", None));
    }

    #[test]
    fn wineserver_detection_finds_a_live_socket() {
        // A fake wine socket tree for a temporary prefix directory:
        // <tmp>/.wine-pttest/server-<dev>-<inode>/socket.
        let prefix = tempfile::tempdir().unwrap();
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(prefix.path()).unwrap();
        let name = format!("server-{:x}-{:x}", meta.dev(), meta.ino());
        let wine_root = std::env::temp_dir().join(".wine-pttest");
        let server_dir = wine_root.join(&name);
        std::fs::create_dir_all(&server_dir).unwrap();
        std::fs::write(server_dir.join("socket"), b"").unwrap();
        assert!(wineserver_running(prefix.path()));
        std::fs::remove_dir_all(&wine_root).unwrap();
        assert!(!wineserver_running(prefix.path()));
    }
}

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Configurable game start wait: `game_wait_secs` in `config.toml`
  (default 90 s) sets how long a trainer launch waits for the game
  to actually start before the trainer connects — Steam's launch
  reaper or Heroic's game executable. On Heroic, a timeout now
  aborts with a clear error instead of risking the old deadlock;
  editable in the Configuration tab.
- Games list: a search field and a source filter (built from the
  stores actually present — Steam, Heroic, ...).
- Heroic configuration-only entries whose prefix marks them as a
  shared-prefix holder (conventional name "shared") are excluded
  from the game list.
- Configurable protonhax wait: `protonhax_wait_secs` in `config.toml`
  (default 10, `0` checks once without waiting) sets how long a
  trainer launch waits for the game's protonhax environment before
  falling back to a direct prefix run; editable in the Configuration
  tab.

### Changed

- The game detail view no longer shows the Wine prefix path, only
  the source.
- The source filter is styled like the other inputs (the WebKit
  native look was left in place before).

### Fixed

- Heroic trainers wait for the game's WINESERVER SOCKET
  (`/tmp/.wine-<uid>/server-<dev>-<inode>/socket`) instead of just a
  related process: the process check matched umu-run seconds before
  the game's wine owned the prefix, so the trainer still created a
  competing host-side wineserver and the game blocked until the
  trainer closed.

### Fixed

- Heroic Proton trainers now connect to the game's wineserver as a
  client: the app waits until the game executable actually starts
  (umu's `proton waitforexitandrun <exe>` chain) and then runs the
  trainer with the game's Proton in `run` mode. The umu-run attempt
  was wrong: umu also launches through `waitforexitandrun`, whose
  `wineserver -w` waits for the game's server to die, so the trainer
  only opened after the game closed. Verified live: game and trainer
  coexist on the one shared wineserver.
- Heroic Proton trainers now wait for the game EXECUTABLE to run
  (detecting umu's `proton waitforexitandrun <exe>` chain) instead
  of the wineserver socket: the socket appears during the prefix
  setup, and a trainer launched then raced the game's proton for
  the prefix and blocked the game. Full flow verified live: game
  via the heroic:// protocol, trainer via umu-run once the game
  executable is running — both coexist indefinitely.
- Heroic Proton trainers now run through `umu-run`, in their own
  umu/pressure-vessel container with the game's environment
  (`UMU_ID`/`WINEPREFIX`/`PROTONPATH`) — the same topology as the
  game itself. Verified live: the game and a umu-run trainer
  coexist, while a trainer spawned next to the game (host-side
  Proton, even after waiting for the game's wineserver socket)
  deadlocked the game until the trainer was closed. Falls back to
  the direct prefix run with a warning when umu-run is missing;
  plain-Wine Heroic games keep the WINEPREFIX flow.
- Heroic trainers no longer block the game: the game is now started
  first and waited for (up to 90 s, detecting its umu/wine process
  by prefix or install path) before the trainer connects. Heroic
  runs Proton games through umu inside a pressure-vessel container;
  a trainer started first created a host-side wineserver for the
  same prefix and the in-container game then waited on it until the
  trainer closed.
- Heroic games now actually launch: Heroic 2.22+ ignores a
  `launch <app>` CLI command — launches are requested through the
  `heroic://launch?appName=<id>&gui=false` protocol URL, which also
  keeps the Heroic window hidden when launched from proton-trainer.
- Heroic games configured with Proton now get the full Steam compat
  environment for the trainer (`STEAM_COMPAT_DATA_PATH` — Heroic's
  `winePrefix` is a compatdata directory, Proton derives the Wine
  prefix as `<dir>/pfx` — plus client path and app id). Proton
  ignores `WINEPREFIX` and exited with "No compat data path?" before,
  so the trainer never started; plain-Wine Heroic games keep
  `WINEPREFIX`.
- Heroic launches: the store client is now spawned with
  `ELECTRON_RUN_AS_NODE` stripped (leaking from IDE terminals, it
  made the Heroic CLI crash with "Cannot find module 'electron'")
  and with its output appended to the log file, so launch failures
  are diagnosable. Launching a game with a trainer also fails fast
  with a clear error when the game's configured Wine/Proton binary
  no longer exists (e.g. a deleted `Proton-GE Latest` build) —
  re-select a version in Heroic's game settings.
- Steam trainers now run INSIDE the game's container via protonhax
  (`protonhax run <appid> <exe>`). Games under the Steam Linux Runtime
  run in a pressure-vessel/bwrap container; a trainer spawned outside
  it died within seconds (`Some threads failed to suspend`) even in
  the same prefix. Requires `protonhax init %command%` in the game's
  Steam launch options; without it the app falls back to a direct
  prefix run and surfaces a warning in the GUI and the log.
- The warning shown on the direct-prefix fallback now reads the
  game's actual Steam launch options (`localconfig.vdf`) and suggests
  the exact corrected options (e.g. `PROTON_FSR4_UPGRADE=1
  protonhax init %command%`); the log also traces the start of the
  protonhax environment wait (up to 30 s) instead of going silent.

- Launch with trainer now WAITS for the game to actually start before
  running the trainer (detecting Steam's `SteamLaunch AppId=<id>`
  reaper process): the trainer used to enter the prefix 47 ms after
  the URL was sent, racing the game's launch. While the game has not
  appeared, the `steam://rungameid` URL is re-sent every 20 seconds
  (Steam ignores duplicates) to force the launch; an already-running
  game is detected instantly; after 90 seconds the trainer starts
  anyway with a warning in the log.

- Launch with trainer: games without an explicit compat-tool override
  now resolve to Steam's **default tool** (`CompatToolMapping` key
  `"0"`) instead of the newest custom tool — running a trainer with
  the wrong Proton made it try to upgrade the game's prefix.
- Tool names that do not match their directory (e.g. Steam's
  `GE-Proton11-7-x86_64` vs the directory `GE-Proton11-7`) now
  resolve prefix-tolerantly.
- The trainer's Proton environment now includes
  `STEAM_COMPAT_CLIENT_INSTALL_PATH` and `STEAM_COMPAT_APP_ID`
  (modern Proton crashes without the former).

- GUI: adding a global trainer or linking a trainer to a game now
  refreshes the list immediately (void commands resolved to `null`,
  which the frontend mistook for failure).
- Launch with trainer: Proton is now looked up in the **primary**
  Steam installation (compatibility tools and `config.vdf` never live
  in secondary libraries), fixing trainer launches for games installed
  on other disks; official Protons in the game's own library are also
  found; `STEAM_COMPAT_DATA_PATH` is set alongside `WINEPREFIX`.

### Added

- File logging: app events and trainer output are appended to
  `~/.local/state/proton-trainer/proton-trainer.log` (stderr output is
  kept for terminal launches); launch steps (game, prefix, Proton,
  trainer command line and pid) are logged so failures are
  diagnosable after the fact.

- Trainer launcher mode: a Configuration tab manages any number of
  **global trainers** (Aurora, WeMod, xModHub, ...) and per-game
  trainers (e.g. FLiNG); each game can be launched plain, with a global
  trainer, or with a per-game trainer, which runs inside the game's
  own Wine/Proton prefix (no Steam launch-options editing needed).
  Configuration lives in `$XDG_CONFIG_HOME/proton-trainer/config.toml`.
- Heroic game discovery (Epic via legendary, GOG, plus configured
  games), native and Flatpak, with per-game Wine prefix and binary
  detection; Heroic games launch through the Heroic CLI.
- The game list now shows every installed game from every store
  (Steam runtimes and redistributables filtered out), with the store
  as a badge.
- Proton detection for Steam games: per-game mapping from
  `config.vdf` (`CompatToolMapping`), newest custom tool in
  `compatibilitytools.d`, or official Protons in `steamapps/common`;
  first-run prefixes are awaited for up to two minutes.

### Changed

- proton-trainer is now a **neutral launcher gateway**: it bundles
  and distributes no trainers and adds no anti-cheat policy when
  launching external trainers (the denylist still applies to the
  native memory engine's own writes).
- The application is now a Tauri 2 **graphical interface** (window)
  instead of a terminal UI: game list, trainer entries, dry-run plan,
  and apply results as screens; heavy operations run on a worker
  thread so the window never freezes. `pt-tui` and `pt-cli` are
  removed; the new `pt-session` crate holds the workflow shared by
  the GUI and the headless CLI subcommands (`list`, `attach`,
  `apply`), which remain unchanged.
- Packaging: the AppImage is now produced by the Tauri bundler in the
  release workflow (the linuxdeploy packaging is gone); AUR packages
  gained the `gtk3`/`webkit2gtk-4.1` runtime dependencies.

## [0.1.0] - 2026-10-15

### Added

- Release packaging: GitHub Actions release workflow (tag-triggered)
  building the x86_64 binary tarball and the AppImage (linuxdeploy),
  with SHA256SUMS; every release is built by CI, never locally.
- CI now also builds the AppImage on every push and smoke-runs it
  without FUSE (`APPIMAGE_EXTRACT_AND_RUN`), then runs it inside the
  Arch container in a clean `$HOME` sandbox.
- AUR packaging: `aur/proton-trainer` (source build) and
  `aur/proton-trainer-bin` (prebuilt) PKGBUILDs plus publishing
  instructions; both install the example trainers to
  `/usr/share/proton-trainer/trainers/`.
- Trainer definitions now also resolve from install layouts
  (`<binary>/../share/proton-trainer/trainers`), which makes the
  tarball, AUR, and AppImage installs work out of the box.
- `deny.toml` for `cargo deny check` (license and advisory gates).
- Installation documentation for CachyOS (AUR) and Bazzite (AppImage),
  including the full ptrace requirements section (setcap for plain
  binaries, sysctl for the AppImage, Flatpak sandbox notes).

### Changed

- Freeze ticks now honor the project write rules in the background
  loop too: every tick re-validates the target range against a fresh
  maps read, writes, and verifies the read-back.
- `apply` reports the more informative error when both the write and
  the resume fail (the target died in between).
- `proton-trainer attach <id> --dry-run` is accepted for explicitness;
  `attach` never writes either way.

### Hardening (milestone P6)

- Safety pass completed: zero `unwrap`/`expect` outside tests; all
  `unsafe` blocks carry documented invariants; SIGSTOP/SIGCONT
  restore verified on all error paths including panics.
- Fuzz-style property tests for every parser (VDF, app manifests,
  maps, trainer TOML, denylist): arbitrary input never panics.
- Freeze re-application proven end to end against the dummy target.
- `cargo deny check` clean (licenses, advisories, bans, sources).

### Fixed

- The `ptrace_scope = 1` advice now explains that the AppImage cannot
  use `setcap` (its runtime forks) and that `sysctl
  kernel.yama.ptrace_scope = 0` is required there.

- `pt-tui`: ratatui interface — game list, running-process check,
  trainer entry list, and apply/dry-run views, keyboard-driven and
  readable at any terminal size. The state machine is pure and
  backend-driven (effects behind a `TuiBackend` trait); rendering is
  covered by TestBackend tests at normal and tiny sizes; the terminal
  loop restores the terminal on exit, error, and panic.
- Freeze entries: after a successful apply, frozen values are
  re-applied (~30 Hz) in the background while the TUI session is open,
  and stopped when it ends.
- CLI: `attach <trainer-id>` (read-only dry-run plan), `apply
  <trainer-id> --dry-run|--apply` (dry-run by default, writes only
  with `--apply`), and `tui` (interactive). Headless commands share
  the same workflow functions as the TUI.
- `--trainers-dir` option; default resolution: explicit flag, then
  `$XDG_CONFIG_HOME/proton-trainer/trainers`, then `./trainers`.

### Fixed

- The pattern scan no longer swallows `EPERM` and dead-target errors:
  a blocked scan now reports the actionable ptrace situation (exact
  `setcap`/`sysctl` fix) instead of a misleading "pattern not found".

- `pt-trainers`: TOML trainer definitions (game metadata, executable
  names, entries with a hex search pattern supporting `??` wildcards,
  a value offset, typed values `i32`/`i64`/`f32`/`f64`/`bytes`, and a
  freeze option) with strict validation: unknown keys rejected, clear
  English errors naming the file and the offending entry, unique ids
  enforced within and across files.
- Anti-cheat denylist (`pt-trainers/denylist.toml`, compiled into the
  binary): attach is refused when the target's exe name or any mapped
  file path matches a known component (EasyAntiCheat, BattlEye,
  Vanguard, EQU8, XIGNCODE3, nProtect GameGuard). Refusal, never
  bypass, with a plain-English explanation.
- Example trainers shipped in `trainers/`: a fully working one for the
  integration-test dummy target and two schema examples with
  placeholder patterns.
- Property tests (proptest): pattern parse roundtrips, value byte
  encodings, entry offset math against the engine's region validation,
  and no-panic parsing of arbitrary input.
- `pt-core` memory engine: process discovery by exe name (`/proc`
  scan), `/proc/<pid>/maps` parsing with fixture-based tests,
  `process_vm_readv`/`process_vm_writev` with partial-transfer
  handling, mapped-region validation before every write, read-back
  verification, and SIGSTOP/SIGCONT orchestration that resumes the
  target on every error path (RAII guard, including panics).
- Byte-pattern scanning (`Pattern` with per-byte wildcards) over the
  target's private writable regions.
- Dry-run semantics: `plan_write` builds a validated `WritePlan`
  (pid, address, old/new bytes, region) without writing; `apply`
  executes it after re-validating the region with a fresh maps read.
- Actionable ptrace errors: `EPERM` is reported in plain English with
  the exact fix (`setcap cap_sys_ptrace=ep` or
  `sysctl kernel.yama.ptrace_scope=0`), tailored to the detected
  Yama ptrace scope; verified end-to-end on a `ptrace_scope=1` system.
- Dummy target process (`pt-dummy-target` test binary) and
  integration tests covering discovery, scan, read, dry-run, verified
  apply, stop/restore guarantees, and refusals (unmapped, read-only,
  region-crossing, empty writes, dead processes).
- `pt-discover`: VDF parser, Steam root discovery (native
  `~/.local/share/Steam`, `~/.steam/steam`, `~/.steam/root`, and Flatpak
  `~/.var/app/com.valvesoftware.Steam`), `libraryfolders.vdf` and
  `appmanifest_*.acf` parsing with checked-in realistic fixtures,
  compatdata prefix detection, and a scan-result cache in
  `$XDG_CONFIG_HOME/proton-trainer/scan-cache.toml`.
- `proton-trainer list` subcommand printing installed Steam games, with
  `--cached` to print the previous scan without rescanning.
- `PROTON_TRAINER_STEAM_ROOTS` environment variable to override Steam
  root discovery (testing on machines without Steam).
- Cargo workspace with five crates: `pt-core`, `pt-discover`, `pt-trainers`,
  `pt-tui`, and the `proton-trainer` binary (`pt-cli`).
- Strict workspace lints (warnings denied, clippy `all` denied).
- Tracing wiring in the binary (`RUST_LOG` / `--log-filter`).
- `justfile` with `build`, `test`, `lint`, `fmt`, and `release` recipes.
- GitHub Actions CI: format check, clippy, and tests on `ubuntu-latest`
  and in an Arch Linux container.
- Project documentation: `README.md`, `CONTRIBUTING.md`, this changelog.
- Archived v1 shell/fish trainer-launcher scripts under `archive/v1/`
  for reference.

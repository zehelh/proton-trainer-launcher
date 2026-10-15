# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-10-15

### Added

- Trainer launcher mode: any number of **global trainers** (Aurora,
  WeMod, xModHub, ...) and per-game trainers (e.g. a FLiNG trainer)
  managed in the Configuration tab; every game can be launched plain
  or with a trainer, which runs inside the game's own Wine/Proton
  prefix. Stored in `$XDG_CONFIG_HOME/proton-trainer/config.toml`.
- Heroic game discovery: Epic (legendary), GOG, and configured
  games, native and Flatpak, with per-game Wine prefix and binary
  detection; sideloaded games show their real title and install
  path, read from `sideload_apps/library.json`. Shared-prefix
  holder entries are excluded from the game list.
- The game list shows every installed game from every store (Steam
  runtimes and redistributables filtered out), with a search field
  and a source filter.
- Game detail view: an Information card with the game's install
  path and the compatibility tool it runs with, plus a GE-Proton
  recommendation when the tool is not a GE build (GE-Proton ships
  the more permissive defaults external trainers need).
- Two configurable timeouts in the Configuration tab: the **game
  start wait** (`game_wait_secs`, default 90 s — how long to wait
  for the launched game to actually start before connecting the
  trainer) and the **protonhax wait** (`protonhax_wait_secs`,
  default 10 s, `0` = a single check — how long a Steam launch
  waits for the game's protonhax environment before falling back
  to a direct prefix run).
- File logging: app events, launch steps, and trainer output are
  appended to `~/.local/state/proton-trainer/proton-trainer.log`;
  store-client output (Heroic) is captured there too.

### Changed

- proton-trainer is now a **neutral launcher gateway**: it bundles
  and distributes no trainers, monetizes nothing, and adds no
  anti-cheat policy on top of the user's choices when launching
  external trainers. The denylist still applies to the native
  memory engine's own writes.
- The application is now a Tauri 2 **graphical interface** instead
  of a terminal UI: game list, trainer entries, dry-run plan, and
  apply results as screens, heavy operations on a worker thread.
  `pt-tui` and `pt-cli` are removed; the new `pt-session` crate
  holds the workflow shared by the GUI and the headless CLI
  subcommands (`list`, `attach`, `apply`).
- Packaging: the AppImage is produced by the Tauri bundler in the
  release workflow; AUR packages gained the `gtk3`/`webkit2gtk-4.1`
  runtime dependencies and no longer install an examples directory.
- CI: `cargo doc` (warnings denied) and `cargo-deny` jobs; the
  release gate runs the full tooling set (fmt, clippy, tests, doc,
  deny) since tag pushes do not trigger the main CI, and fails
  loudly when the tag, `Cargo.toml`, and `tauri.conf.json` versions
  disagree.

### Removed

- The bundled example trainer definitions (`trainers/`): two were
  placeholder patterns and the third only targeted the integration
  test binary, which never ships. The README's TOML block remains
  the schema reference.

### Fixed

- Steam trainers run INSIDE the game's container via protonhax
  (`protonhax run <appid> <exe>`, requires `protonhax init
  %command%` in the game's launch options): a trainer spawned
  outside the Steam Linux Runtime container died within seconds
  even in the same prefix. Without protonhax the app falls back to
  a direct prefix run, warns in the GUI and the log, and suggests
  the exact corrected launch options read from `localconfig.vdf`.
- Steam launch-with-trainer waits for the game to actually start
  (Steam's `SteamLaunch AppId=<id>` reaper), re-sending the
  `steam://rungameid` URL every 20 seconds to force the launch, and
  resolves the game's actual compatibility tool (per-game
  `CompatToolMapping` entry, else Steam's default tool) with
  prefix-tolerant directory matching and the full Steam compat
  environment — the wrong Proton used to try upgrading the game's
  prefix.
- Heroic games launch through the `heroic://launch?appName=<id>`
  protocol URL (Heroic 2.22+ ignores a `launch <app>` CLI command);
  `gui=false` keeps the Heroic window hidden.
- Heroic trainers get the full Steam compat environment
  (`STEAM_COMPAT_DATA_PATH` — Proton ignores `WINEPREFIX` and
  exited with "No compat data path?" before) and fail fast with a
  clear error when the configured Wine/Proton binary no longer
  exists.
- Heroic store clients are spawned with `ELECTRON_RUN_AS_NODE`
  stripped — the variable leaks from IDE terminals and crashed the
  Heroic CLI with "Cannot find module 'electron'".
- Heroic trainers connect to the game's wineserver as a plain
  client: the app waits until the game executable actually starts
  (umu's `proton waitforexitandrun <exe>` chain), then runs the
  trainer with the game's Proton in `run` mode. Every earlier
  topology deadlocked the pair — umu launches serialize on a
  prefix (`wineserver -w`), so a trainer started before or
  alongside the game blocked it until the trainer closed. Verified
  live: game and trainer coexist on the one shared server.
- GUI: adding a global trainer or linking a trainer to a game now
  refreshes the list immediately (void commands resolved to
  `null`, which the frontend mistook for failure).

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

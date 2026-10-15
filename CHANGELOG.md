# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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

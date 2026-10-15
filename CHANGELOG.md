# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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

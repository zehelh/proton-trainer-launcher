# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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

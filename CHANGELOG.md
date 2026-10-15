# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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

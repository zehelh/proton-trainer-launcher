# AGENTS.md — proton-trainer

## Language
- Write ALL code, comments, docstrings, commit messages, docs, and README in English.
- The project is open source: optimize for external contributors from day one.

## Secrets & hygiene
- Never commit secrets, .env files, keys, or tokens.
- Conventional commits (feat:, fix:, docs:, test:, refactor:, chore:). Small atomic commits.

## Quality bar
- Every feature ships with tests. Run the full test suite before every commit.
- Prefer boring, well-documented solutions over clever ones. No TODO left without a linked issue.

## Workflow
- Plan before coding: when asked to implement something non-trivial, propose the plan first and wait for approval.
- After each milestone: update docs and CHANGELOG.md (Keep a Changelog format).


Open-source Linux game trainer for SINGLE-PLAYER games running under Steam
Proton/Wine. Written in Rust. No anti-cheat bypass. Online/competitive games
are out of scope, forever.

## Compatibility targets (priority order)
1. CachyOS (Arch-based, x86_64) — primary dev/test machine.
2. Bazzite (immutable, rpm-ostree, Steam often Flatpak) — must work without
   requiring the user to layer packages: binary must run from ~/.local/bin or
   as AppImage; must find Flatpak Steam at
   ~/.var/app/com.valvesoftware.Steam and native Steam at ~/.steam/steam AND
   ~/.local/share/Steam.
3. Generic x86_64 Linux, best effort.

## Stack (fixed — do not change without asking)
- Rust stable, edition 2021+. MSRV = current stable minus 2.
- Cargo workspace:
  - `pt-core` — process discovery + memory read/write (pure lib, fully tested)
  - `pt-discover` — Steam library scan: libraryfolders.vdf parsing, appmanifest
    parsing, compatdata prefix detection, Flatpak + native paths
  - `pt-trainers` — trainer definitions (TOML), parsing, validation
  - `pt-session` — the session workflow shared by the GUI and the CLI
    (attach, denylist check, plan, apply, freeze)
  - `pt-gui` — the `proton-trainer` application: a Tauri 2 GUI (window)
    plus the headless CLI subcommands (clap)
- Errors: thiserror in libraries, anyhow in the binary. NO unwrap/expect
  outside tests and main.
- Logging: tracing + tracing-subscriber (RUST_LOG documented in README).
- Config in XDG dirs (dirs crate), TOML via serde.

## Memory-safety rules (non-negotiable)
- Every write: validate the target address range is a mapped, writable region
  before writing; read back and verify after writing.
- Stop the target process (SIGSTOP) around write sequences, restore state
  (SIGCONT) even on error paths — no leaked stopped processes, ever.
- Explicit `--dry-run` mode everywhere: shows the exact bytes/addresses without
  writing. Writes require `--apply`.
- Never write outside the target process's mapped regions. Never touch kernel
  structures, other processes, or /proc besides the target.
- Anti-cheat deny-list: refuse to attach to processes whose exe name matches
  known anti-cheat components (EAC, BattlEye, vanguard, etc.). REFUSE, never
  bypass. Maintained in pt-trainers/denylist.toml.
- Document for every write function: signal safety, error modes, restore path.

## Tests (this project lives or dies by them)
- Unit tests for every parser (vdf, appmanifest, trainer TOML) with fixture files.
- Integration tests in pt-core that spawn a DUMMY target process (a tiny test
  binary that allocates known patterns) and exercise read/write/verify/restore.
- No test ever attaches to a real game or Steam itself.
- Property tests (proptest) for address-range and offset math.

## Tooling
- `cargo fmt --check`, `cargo clippy --all-features -- -D warnings`, 
  `cargo test --all-features`, `cargo doc --no-deps`, `cargo deny check`.
- CI on GitHub Actions: build + test on ubuntu-latest AND an Arch container
  (to catch glibc/clang noise early), x86_64 only for v0.x.

## Distribution
- Static-ish binary release (GitHub Releases), AppImage, and an AUR PKGBUILD
  (`proton-trainer` + `-bin`). Bazzite users use the AppImage or the binary.
- Every release ships SHA256SUMS and is built by CI, not locally.

## Definition of done (every feature)
1. Plan agreed → 2. Tests written first → 3. Implementation →
4. fmt + clippy + tests green → 5. Public items have doc comments (rustdoc) →
6. Docs + CHANGELOG updated → 7. Conventional commit.

## ptrace notes (documented in README)
- kernel.yama.ptrace_scope: 0 = works for all, 1 = parent-only (default on
  many distros). The app must DETECT the situation and print a clear,
  actionable fix (exact sysctl command or setcap cap_sys_ptrace=ep on the
  binary) instead of failing with a cryptic EPERM.
- Flatpak Steam games on Bazzite: processes may live inside a different
  namespace; if ptrace is blocked, detect it and say so in plain English.
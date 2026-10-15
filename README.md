# proton-trainer

An open-source Linux game trainer for **single-player** games running under
Steam Proton/Wine. Written in Rust. No anti-cheat bypass — online and
competitive games are out of scope, forever.

> **Status: pre-alpha (milestone P3).** The workspace structure, CI,
> Steam discovery, the memory engine, and trainer definitions are in
> place. The TUI and the attach/apply commands land with milestone P4.
> See the roadmap below.

## Principles

- Single-player games only. Processes matching known anti-cheat components
  (EAC, BattlEye, Vanguard, ...) are **refused**, never bypassed.
- Every write is validated against the target's mapped regions, applied
  while the target is briefly stopped, read back and verified, and the
  target is always resumed — even on error.
- `--dry-run` everywhere: plans are shown before anything is written.
  Writes require an explicit `--apply`.
- Boring, well-documented code, tested end to end with a dummy target
  process (no test ever touches a real game).

## Compatibility targets (priority order)

1. **CachyOS** (Arch-based, x86_64) — primary dev/test machine.
2. **Bazzite** (immutable, Steam often Flatpak) — works from `~/.local/bin`
   or as an AppImage, without layering packages.
3. Generic x86_64 Linux — best effort.

## Building from source

Requires Rust stable (see `rust-version` in `Cargo.toml` for the MSRV).

```bash
cargo build --release
./target/release/proton-trainer --version
```

Or with [just](https://github.com/casey/just) (optional; every recipe maps
to a raw cargo command inside the `justfile`):

```bash
just build
just test
just lint
```

## Usage

### List installed Steam games

```bash
proton-trainer list          # scan Steam libraries, print games, update the cache
proton-trainer list --cached # print the previous scan without rescanning
```

Output columns: app id, name, prefix (`proton` when a compatdata prefix
exists, `none` for native Linux games), and the `steamapps/` directory
holding the game. The scan result is cached in
`$XDG_CONFIG_HOME/proton-trainer/scan-cache.toml`.

Steam roots are auto-detected (native `~/.local/share/Steam`,
`~/.steam/steam`, `~/.steam/root`, and Flatpak
`~/.var/app/com.valvesoftware.Steam`). To scan other directories —
e.g. on a machine without Steam — set `PROTON_TRAINER_STEAM_ROOTS` to a
colon-separated list:

```bash
PROTON_TRAINER_STEAM_ROOTS=/tmp/fake-steam proton-trainer list
```

### Trainer definitions (milestone P3)

Trainers are TOML files in a `trainers/` directory. Each entry scans
for a byte pattern (hex, `??` = wildcard) and writes a typed value at
a fixed offset from the match:

```toml
schema_version = 1
id = "hollow-knight"
name = "Hollow Knight"
steam_app_id = 367520
exe_names = ["hollow_knight.exe"]
version = "0.1.0"

[[entry]]
id = "soul"
label = "Infinite SOUL"
search = { pattern = "89 45 ?? 8B 06" }
offset = 12
value = { type = "i32", data = 99 }
freeze = true
```

Values support `i32`, `i64`, `f32`, `f64`, and raw `bytes` (hex).
Definitions are validated strictly: unknown keys, duplicate ids, and
malformed patterns are rejected with clear errors. The examples in
[`trainers/`](trainers/) include a fully working one for the test
dummy target; the others use placeholder patterns by design.

### Anti-cheat policy

The binary ships with an anti-cheat denylist (EasyAntiCheat, BattlEye,
Vanguard, EQU8, XIGNCODE3, nProtect GameGuard). If the target's
executable or any of its mapped files match, attach is **refused**
with a plain-English explanation. There is no bypass and there never
will be: proton-trainer targets single-player games only.

### Planned (milestone P4)

```text
proton-trainer attach <game> --dry-run               # show the write plan
proton-trainer apply <game> --apply                  # apply trainer values
```

The memory engine backing those commands is complete (`pt-core`):
every write is validated against one mapped writable region, applied
while the target is briefly stopped, read back and verified, and the
target is always resumed — even on errors. See the crate docs
(`cargo doc --open -p pt-core`).

## Logging

The binary logs with `tracing`. Set `RUST_LOG` or pass `--log-filter`:

```bash
RUST_LOG=proton_trainer=debug proton-trainer list
proton-trainer --log-filter=debug list
```

## ptrace notes

Memory access uses `process_vm_readv`/`process_vm_writev`, which are subject
to Yama ptrace restrictions:

- `kernel.yama.ptrace_scope = 0`: works for all same-user processes.
- `kernel.yama.ptrace_scope = 1` (default on many distros): parent-only —
  the trainer must be granted access, e.g. with
  `sudo setcap cap_sys_ptrace=ep proton-trainer` or by setting
  `sudo sysctl kernel.yama.ptrace_scope=0`.

The tool **detects** the situation and prints the exact fix instead of
failing with a cryptic `EPERM` (implemented in milestone P2).

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md). The v1 shell/fish launcher scripts
that inspired this project are archived under [archive/v1/](archive/v1/).

## License

MIT — see [LICENSE](LICENSE).

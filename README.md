# proton-trainer

An open-source Linux game trainer for **single-player** games running under
Steam Proton/Wine. Written in Rust. No anti-cheat bypass — online and
competitive games are out of scope, forever.

> **Status: pre-alpha (milestone P5).** All core features are in place
> (Steam discovery, memory engine, trainer definitions, TUI, CLI).
> v0.1.0 ships after hardening (milestone P6). See the roadmap below.

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

## Installation

### CachyOS / Arch-based (AUR)

```bash
yay -S proton-trainer      # builds from source
yay -S proton-trainer-bin  # prebuilt binary from the GitHub release
```

Both packages install the binary to `/usr/bin/proton-trainer` and the
example trainer definitions to `/usr/share/proton-trainer/trainers/`.
The `-bin` package tracks the GitHub releases; the sha256 is checked
against the release's `SHA256SUMS`.

### Bazzite (immutable, no layering)

Use the AppImage or the release binary from the [releases
page](https://github.com/zehelh/proton-trainer-launcher/releases) —
nothing needs to be layered onto the system:

```bash
# AppImage: verify, make executable, run
sha256sum -c SHA256SUMS
chmod +x proton-trainer-*-x86_64.AppImage
./proton-trainer-*-x86_64.AppImage tui
```

Put the AppImage in `~/.local/bin` (or anywhere on `PATH`). The
binary variant also runs from `~/.local/bin`:

```bash
tar -xzf proton-trainer-*-x86_64.tar.gz
cp proton-trainer-*-x86_64/proton-trainer ~/.local/bin/
```

The AppImage bundles everything except glibc; the release binary
requires a 2024+ glibc (built on Ubuntu 24.04). On older systems,
build from source.

### ptrace requirements (read this)

The trainer needs `process_vm_readv`/`process_vm_writev` access to
the game process, which Yama restricts on most distros:

```bash
cat /proc/sys/kernel/yama/ptrace_scope   # 1 = parent-only (common default)
```

- **ptrace_scope = 0**: everything works as-is.
- **ptrace_scope = 1** (common): the trainer must be privileged.
  - Plain binary (AUR, tarball): grant it the capability once:
    ```bash
    sudo setcap cap_sys_ptrace=ep /usr/bin/proton-trainer
    ```
  - **AppImage**: `setcap` cannot apply to the binary inside the
    image — set, in `/etc/sysctl.d/99-ptrace.conf` (persistent) or per
    session:
    ```bash
    echo 'kernel.yama.ptrace_scope = 0' | sudo tee /etc/sysctl.d/99-ptrace.conf
    sudo sysctl --system
    ```

proton-trainer detects the situation and prints these exact fixes on
`EPERM` — including the Flatpak/pressure-vessel explanation when Yama
is not the blocker (Steam on Bazzite may run games inside a sandbox;
if tracing is blocked with `ptrace_scope = 0`, run the trainer from
the same environment).

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

### Attach, apply, and the TUI (milestone P4)

Trainer definitions are resolved from `--trainers-dir`, then
`$XDG_CONFIG_HOME/proton-trainer/trainers`, then `./trainers`.

```bash
proton-trainer attach <trainer-id>            # read-only: print the write plan
proton-trainer apply <trainer-id>             # dry-run: same plan, nothing written
proton-trainer apply <trainer-id> --apply     # write, verified, target resumed
proton-trainer tui                            # interactive interface
```

The TUI flow: game list → running-process check (denylist applied) →
trainer entry list (Space toggles, `a` plan+apply) → dry-run plan →
apply results. Entries marked `[freeze]` are re-applied in the
background while the session stays open. Attach is always refused with
a plain-English explanation when the target matches the anti-cheat
denylist, and blocked memory access reports the exact ptrace fix.

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

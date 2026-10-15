# proton-trainer

An open-source Linux **trainer launcher**: it detects your installed
games (Steam — every library disk, native and Flatpak — and Heroic) and
launches them with the external trainer of your choice (Aurora, WeMod,
xModHub, FLiNG, ...) running inside the game's own Proton/Wine prefix.
It also ships a native TOML trainer engine (pattern scan, verified
writes, freeze) for trainers authored for this app. Written in Rust.

proton-trainer is a neutral gateway: it bundles and distributes no
trainers and monetizes nothing. What you launch is your choice and
your responsibility.

> **Status: v1.0.1 — trainer launcher.** The application now has a
> graphical interface (Tauri 2). Steam discovery, the memory engine,
> trainer definitions, and packaging are unchanged; the headless CLI
> subcommands remain. Trainer definitions for real games are community
> content — the shipped ones are examples.

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

Both packages install the binary to `/usr/bin/proton-trainer`.
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
malformed patterns are rejected with clear errors. No example
definitions ship with the app — the TOML above is the schema
reference.

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
proton-trainer                                # graphical interface
proton-trainer attach <trainer-id>            # read-only: print the write plan
proton-trainer apply <trainer-id>             # dry-run: same plan, nothing written
proton-trainer apply <trainer-id> --apply     # write, verified, target resumed
```

The GUI has two tabs:

- **Games** — every installed game from every store (Steam libraries
  on all disks, native and Flatpak Steam, plus Heroic). The detail
  view shows the game's install path and the compatibility tool it
  runs with, with a recommendation to switch to GE-Proton when the
  tool is not a GE build (GE-Proton ships the more permissive
  defaults trainers need). Click a game
  to: launch it plain, launch it with one of your **global trainers**
  (Aurora, WeMod, xModHub, ...), launch it with a **per-game trainer**
  you linked to it (e.g. a FLiNG trainer), or — when a native TOML
  definition covers it — run the attach / dry-run plan / apply flow
  (the denylist applies there, and blocked memory access reports the
  exact ptrace fix).
- **Configuration** — manage the global trainers (name + path, as
  many as you like), review the per-game links, and set two
  timeouts: the **game start wait** (seconds to wait for a launched
  game to actually start before the trainer connects; default 90)
  and the **protonhax wait** (seconds a Steam trainer launch waits
  for the game's protonhax environment before falling back to a
  direct prefix run; default 10, `0` checks once without waiting).
  Everything is stored in `$XDG_CONFIG_HOME/proton-trainer/config.toml`.

External trainers run inside the game's own Wine/Proton prefix with
the game's own compatibility tool. Steam games launch through
`steam://rungameid/<appid>`; Heroic games through the Heroic CLI; on a
first run the app waits for Steam to create the game's prefix before
starting the trainer (up to two minutes).

**Steam Linux Runtime (container) note.** Steam runs Proton games
inside a pressure-vessel/bwrap container; a trainer spawned outside
it cannot join the game's wineserver and dies within seconds, even in
the same prefix. proton-trainer therefore runs Steam trainers through
[protonhax](https://github.com/illnyang/protonhax), which enters the
game's container. This needs two things:

1. `protonhax` installed (AUR: `protonhax`),
2. `protonhax init %command%` in the game's launch options
   (Steam → Properties → Launch Options).

When protonhax cannot be used, the app falls back to running the
trainer directly in the prefix — that only works for games NOT running
in a container — and shows a warning.

**Heroic (umu) note.** Heroic runs Proton games through
[umu](https://github.com/Open-Wine-Components/umu-launcher), inside a
pressure-vessel container that SHARES the host's `/tmp` — so the game
and anything else on the same prefix share ONE wineserver. umu always
launches through `proton waitforexitandrun`, which first waits
(`wineserver -w`) for any existing server of the prefix to die: two
umu launches therefore serialize, never run in parallel. proton-trainer
waits until the game executable actually starts (umu's
`proton waitforexitandrun <game exe>` chain), then runs the trainer
with the game's Proton in `run` mode — its Wine connects to the live
game server as a plain client (verified live: game and trainer
coexist on the one shared server).

The GUI is a Tauri 2 application; the system must provide
`webkit2gtk-4.1` (present on CachyOS/Arch and in the Bazzite base
image).

### Troubleshooting

Everything — app events, launch steps, and the output of the trainers
themselves — is appended to
`~/.local/state/proton-trainer/proton-trainer.log`. When a game or a
trainer fails to start, that file says why (selected Proton, prefix,
command line, and the trainer's own stderr). Store-client output
(Heroic's CLI) is captured there too.

Two common environment pitfalls:

- IDE terminals (VS Code) export `ELECTRON_RUN_AS_NODE`, which turns
  Electron-based clients like Heroic into plain Node where they crash
  with "Cannot find module 'electron'". proton-trainer strips the
  variable when spawning store clients — launch it from your
  application menu or a clean terminal if you ever see that error
  elsewhere.
- A Heroic game whose configured Wine/Proton was deleted (e.g. an
  updated `Proton-GE Latest`) cannot launch a trainer; proton-trainer
  reports the missing binary — re-select a version in Heroic's game
  settings (Wine tab).
- AppImages leak their runtime environment (`PYTHONHOME`,
  `PYTHONPATH`, `LD_LIBRARY_PATH`) into child processes; since the
  Proton launcher is a Python script, that killed every trainer
  spawned from the 1.0.0 AppImage. proton-trainer sanitizes the
  environment of everything it spawns — fixed from 1.0.1.

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

# proton-trainer

An open-source Linux game trainer for **single-player** games running under
Steam Proton/Wine. Written in Rust. No anti-cheat bypass — online and
competitive games are out of scope, forever.

> **Status: pre-alpha scaffold (milestone P0).** The workspace structure, CI,
> and CLI skeleton are in place. Steam discovery, the memory engine, trainer
> definitions, and the TUI land with milestones P1-P4. See the roadmap below.

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

Not yet functional beyond `--version` (pre-alpha). The planned CLI:

```text
proton-trainer list                                  # installed Steam games
proton-trainer attach <game> --dry-run               # show the write plan
proton-trainer apply <game> --apply                 # apply trainer values
```

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

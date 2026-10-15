# Contributing to proton-trainer

Thanks for helping. This project is open source from day one; these rules
keep it safe and maintainable.

## Scope (fixed)

- **Single-player games only.** Never add support for, or trainer
  definitions for, online or competitive games.
- **No anti-cheat bypass.** The denylist in `pt-trainers` causes a refusal,
  never a workaround. PRs attempting a bypass will be rejected.

## Getting started

Requires Rust stable (MSRV: see `rust-version` in `Cargo.toml`).

```bash
git clone https://github.com/zehelh/proton-trainer-launcher
cd proton-trainer-launcher
cargo build --workspace
cargo test --all-features --workspace
```

[`just`](https://github.com/casey/just) is optional; every recipe maps to a
raw cargo command:

```bash
just build    # cargo build --workspace
just test     # cargo test --all-features --workspace
just lint     # cargo fmt --all --check && cargo clippy ... -D warnings
```

## Quality bar

- **Every feature ships with tests.** Run the full suite before every commit.
- `cargo fmt --check`, `cargo clippy --all-features -- -D warnings`, and
  `cargo test --all-features` must all pass. CI enforces this on
  `ubuntu-latest` and in an Arch Linux container.
- Workspace lints deny warnings; do not silence lints with `allow`
  attributes without a comment explaining why.
- **No `unwrap`/`expect` outside tests and `main`.** Use `thiserror` types
  in library crates, `anyhow` in the binary.
- Prefer boring, well-documented solutions. No TODOs without a linked issue.
- All code, comments, docstrings, commit messages, and docs are in English.
- Public items need rustdoc comments.
- Conventional commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`,
  `chore:`), small and atomic.

## Memory-safety rules (non-negotiable)

Any change touching cross-process memory must:

1. Validate the target address range is a mapped, writable region before
   writing.
2. Read back and verify after writing.
3. Stop the target (SIGSTOP) around write sequences and restore (SIGCONT)
   on every error path — no leaked stopped processes, ever.
4. Never write outside the target's mapped regions, and never touch other
   processes or kernel structures.
5. Document signal safety, error modes, and the restore path on every
   write function.

Integration tests use the dummy target process; **no test ever attaches to
a real game or to Steam.**

## Definition of done

1. Plan agreed → 2. Tests written first → 3. Implementation → 4. fmt +
   clippy + tests green → 5. Public items documented → 6. Docs and
   `CHANGELOG.md` updated → 7. Conventional commit.

## Pull requests

Keep PRs small and focused on one milestone item. Describe what changed and
why, and list the tests you added. If your PR touches memory access, state
how the rules above are satisfied.

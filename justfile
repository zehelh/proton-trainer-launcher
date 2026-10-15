# proton-trainer development recipes.
#
# `just` is optional: every recipe maps 1:1 to the raw cargo command shown
# inside. If just is not installed, run those commands directly.

default: lint test

# Build all crates (debug).
build:
    cargo build --workspace

# Build all crates (release).
release:
    cargo build --release --workspace

# Run the full test suite.
test:
    cargo test --all-features --workspace

# Run all lint checks (formatting + clippy).
lint: fmt-check clippy

# Format the codebase.
fmt:
    cargo fmt --all

# Check formatting without modifying files.
fmt-check:
    cargo fmt --all --check

# Run clippy with warnings denied.
clippy:
    cargo clippy --all-targets --all-features --workspace -- -D warnings

# Build the rustdoc API documentation.
doc:
    cargo doc --no-deps --workspace

# Check dependency licenses, advisories, bans, and sources.
# Requires cargo-deny (cargo install cargo-deny).
deny:
    cargo deny check

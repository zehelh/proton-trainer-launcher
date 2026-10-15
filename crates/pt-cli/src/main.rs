//! The `proton-trainer` command-line binary.
//!
//! P0 scaffold: CLI skeleton with `--version` and tracing wiring.
//! Subcommands (`list`, `attach`, `apply`) arrive with milestones
//! P1, P2, and P4.

use clap::Parser;

/// Memory trainer for single-player Steam games running under Proton/Wine.
///
/// Single-player only. Anti-cheat protected games are refused, never bypassed.
#[derive(Debug, Parser)]
#[command(
    name = "proton-trainer",
    version = env!("CARGO_PKG_VERSION"),
    about,
    arg_required_else_help = true
)]
struct Cli {
    /// Log filter override (defaults to RUST_LOG, then "info").
    #[arg(long, global = true)]
    log_filter: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let filter = cli
        .log_filter
        .as_deref()
        .map(tracing_subscriber::EnvFilter::try_new)
        .transpose()?
        .unwrap_or_else(|| {
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        });

    tracing_subscriber::fmt().with_env_filter(filter).init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "proton-trainer starting"
    );

    // No subcommands exist yet (P0); clap already printed help and exited
    // when invoked without arguments. Nothing else to do.
    Ok(())
}

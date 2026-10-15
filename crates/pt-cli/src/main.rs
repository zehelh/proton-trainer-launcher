//! The `proton-trainer` command-line binary.
//!
//! Subcommands: `list` (milestone P1). `attach` and `apply` arrive with
//! milestones P2 and P4.

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
use pt_discover::cache;
use pt_discover::scan::{scan, ScanOptions, ScanResult};

/// Memory trainer for single-player Steam games running under Proton/Wine.
///
/// Single-player only. Anti-cheat protected games are refused, never bypassed.
#[derive(Debug, Parser)]
#[command(
    name = "proton-trainer",
    version = env!("CARGO_PKG_VERSION"),
    about,
    arg_required_else_help = true,
    subcommand_required = true
)]
struct Cli {
    /// Log filter override (defaults to RUST_LOG, then "info").
    #[arg(long, global = true)]
    log_filter: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List installed Steam games and their Proton compatdata prefixes.
    List {
        /// Print the cached scan result instead of scanning again.
        #[arg(long)]
        cached: bool,
    },
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

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "proton-trainer starting"
    );

    match cli.command {
        Commands::List { cached } => cmd_list(cached),
    }
}

/// Implementation of `proton-trainer list`.
fn cmd_list(cached: bool) -> anyhow::Result<()> {
    let result = if cached {
        cache::load()
            .context("failed to read the scan cache")?
            .ok_or_else(|| {
                anyhow!("no scan cache found; run `proton-trainer list` without --cached first")
            })?
    } else {
        let options =
            ScanOptions::from_default_roots().context("failed to discover Steam installations")?;
        let result = scan(&options).context("the Steam library scan failed")?;
        // A cache failure must not prevent listing; it is only a cache.
        if let Err(error) = cache::save(&result) {
            tracing::warn!(%error, "failed to update the scan cache");
        }
        result
    };

    print_games(&result);
    Ok(())
}

/// Prints the scan result as a fixed-column table.
fn print_games(result: &ScanResult) {
    if result.games.is_empty() {
        println!("No installed Steam games found.");
        return;
    }

    let name_width = result
        .games
        .iter()
        .map(|game| game.name.chars().count())
        .max()
        .unwrap_or(0)
        .clamp(12, 32);

    println!(
        "{:<8} {:<width$} {:<7} STEAMAPPS",
        "APP ID",
        "NAME",
        "PREFIX",
        width = name_width
    );
    for game in &result.games {
        let prefix = if game.compatdata.is_some() {
            "proton"
        } else {
            "none"
        };
        println!(
            "{:<8} {:<width$} {:<7} {}",
            game.app_id,
            fit(&game.name, name_width),
            prefix,
            game.steamapps_dir.display(),
            width = name_width
        );
    }
}

/// Truncates `text` to `width` characters, appending an ellipsis when
/// characters were dropped.
fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(width.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_short_names() {
        assert_eq!(fit("Terraria", 10), "Terraria");
    }

    #[test]
    fn fit_truncates_long_names() {
        let fitted = fit("Some Extremely Long Video Game Name Here", 10);
        assert_eq!(fitted.chars().count(), 10);
        assert!(fitted.ends_with('…'));
    }
}

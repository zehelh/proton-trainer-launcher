//! The `proton-trainer` application: a Tauri GUI by default, with the
//! headless subcommands (`list`, `attach`, `apply`) kept for terminal
//! and scripting use.

mod cli;
mod commands;

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pt_session::LiveSession;

/// Memory trainer for single-player Steam games running under Proton/Wine.
///
/// Single-player only. Anti-cheat protected games are refused, never bypassed.
#[derive(Debug, Parser)]
#[command(name = "proton-trainer", version = env!("CARGO_PKG_VERSION"), about)]
struct Cli {
    /// Log filter override (defaults to RUST_LOG, then "info").
    #[arg(long, global = true)]
    log_filter: Option<String>,

    /// Trainer definitions directory.
    ///
    /// Defaults to $XDG_CONFIG_HOME/proton-trainer/trainers when it
    /// exists, then ./trainers.
    #[arg(long, global = true)]
    trainers_dir: Option<PathBuf>,

    /// Without a subcommand, the GUI launches.
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List installed Steam games and their Proton compatdata prefixes.
    List {
        /// Print the cached scan result instead of scanning again.
        #[arg(long)]
        cached: bool,
    },
    /// Attach to a trainer's game and print the write plan. Read-only:
    /// never writes, never stops the target.
    Attach {
        /// Trainer definition id, e.g. `hollow-knight`.
        trainer_id: String,
        /// Accepted for explicitness; `attach` is always a dry run and
        /// never writes, with or without this flag.
        #[arg(long)]
        dry_run: bool,
    },
    /// Apply a trainer's entries. Dry-run by default; writes only with
    /// --apply.
    Apply {
        /// Trainer definition id, e.g. `hollow-knight`.
        trainer_id: String,
        /// Perform the writes (without this flag nothing is written).
        #[arg(long)]
        apply: bool,
    },
    /// Launch the graphical interface.
    Gui,
}

/// Shared GUI state: one attach session at a time, behind a mutex the
/// commands lock for the duration of each operation.
pub type GuiState = Arc<Mutex<LiveSession>>;

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let filter = args
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

    match args.command {
        Some(Commands::List { cached }) => cli::cmd_list(cached),
        Some(Commands::Attach { trainer_id, .. }) => {
            cli::cmd_attach(&trainer_id, args.trainers_dir)
        }
        Some(Commands::Apply { trainer_id, apply }) => {
            cli::cmd_apply(&trainer_id, args.trainers_dir, apply)
        }
        Some(Commands::Gui) | None => run_gui(args.trainers_dir),
    }
}

/// Launches the Tauri GUI window.
fn run_gui(trainers_dir: Option<PathBuf>) -> anyhow::Result<()> {
    // Fall back to the XDG path when no directory is found: the GUI
    // shows a helpful banner listing the candidates instead of
    // refusing to start.
    let dir = cli::resolve_trainers_dir(trainers_dir.as_ref())
        .ok()
        .or_else(|| dirs::config_dir().map(|c| c.join("proton-trainer").join("trainers")))
        .context("could not determine a trainers directory")?;
    let session = LiveSession::new(dir)
        .map_err(|error| anyhow::anyhow!("failed to initialize the session: {error}"))?;
    let state: GuiState = Arc::new(Mutex::new(session));

    tauri::Builder::default()
        .manage(Arc::clone(&state))
        .invoke_handler(tauri::generate_handler![
            commands::list_games,
            commands::attach,
            commands::plan,
            commands::apply,
            commands::stop_session,
        ])
        .run(tauri::generate_context!())
        .context("the GUI event loop failed")?;
    Ok(())
}

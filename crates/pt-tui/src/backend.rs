//! Effects and data types the TUI needs from the rest of the system.
//!
//! Everything with side effects (process discovery, scanning, writing)
//! sits behind the [`TuiBackend`] trait, so the [`crate::app::App`]
//! state machine is fully testable without a terminal, without a game,
//! and without touching any real process.

use pt_core::WritePlan;

/// One row of the TUI game list: an installed game paired with the
/// trainer definition that covers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRow {
    /// Steam application id.
    pub app_id: u32,
    /// Game name as Steam reports it.
    pub name: String,
    /// Id of the trainer definition covering this game.
    pub trainer_id: String,
}

/// A trainer entry as shown in the TUI entry list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryBrief {
    /// Entry id.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Whether the entry freezes its value while the session is open.
    pub freeze: bool,
}

/// The outcome of attempting to attach to a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachOutcome {
    /// A running target was found and passed the denylist check.
    Running {
        /// Target process id.
        pid: i32,
        /// Executable name of the target.
        exe_name: String,
        /// The trainer's entries.
        entries: Vec<EntryBrief>,
    },
    /// No process with any of the trainer's executable names is
    /// running.
    NotRunning {
        /// The executable names that were searched.
        exe_names: Vec<String>,
    },
    /// The target matched the anti-cheat denylist; attach is refused.
    Refused {
        /// Plain-English explanation of the refusal.
        explanation: String,
    },
}

/// One prepared write: the TUI plan view and the CLI dry-run print the
/// same rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRow {
    /// Entry id this plan belongs to.
    pub entry_id: String,
    /// Human-readable entry label.
    pub label: String,
    /// Whether this entry should be frozen after applying.
    pub freeze: bool,
    /// The engine write plan (pid, address, bytes, region).
    pub plan: WritePlan,
}

/// The outcome of applying one plan row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyResult {
    /// Entry id this result belongs to.
    pub entry_id: String,
    /// Human-readable entry label.
    pub label: String,
    /// `Ok` when the write was applied and verified, an error message
    /// otherwise.
    pub outcome: Result<(), String>,
}

/// Formats one plan row for display (TUI plan screen and CLI
/// dry-run share this).
pub fn format_plan_row(row: &PlanRow) -> String {
    let freeze = if row.freeze { " [freeze]" } else { "" };
    format!(
        "{} ({}):{}\n  pid      : {}\n  address  : {:#x}\n  region   : {:#x}-{:#x}\n  old      : {}\n  new      : {}\n",
        row.entry_id,
        row.label,
        freeze,
        row.plan.pid,
        row.plan.address,
        row.plan.region.start,
        row.plan.region.end,
        hex(&row.plan.old_bytes),
        hex(&row.plan.new_bytes),
    )
}

/// Formats a byte slice as lowercase hex pairs.
pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Effects the TUI needs: listing games, attaching to a game,
/// planning writes, applying them, and running freeze in the
/// background.
///
/// Implementations must be self-contained: the [`App`](crate::app::App)
/// only calls these methods and never touches pt-core directly.
pub trait TuiBackend {
    /// Lists the installed games that have trainer definitions.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message on failure (e.g. Steam
    /// discovery or trainer loading failed).
    fn games(&mut self) -> Result<Vec<GameRow>, String>;

    /// Attaches to the game covered by `trainer_id`: resolves the
    /// running process, checks the anti-cheat denylist, and returns
    /// the outcome. Never writes.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message on unexpected failures.
    fn attach(&mut self, trainer_id: &str) -> Result<AttachOutcome, String>;

    /// Builds dry-run plans for the given entry ids of the attached
    /// trainer. Never writes.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message naming the entry on failure
    /// (pattern not found, ambiguous match, ...).
    fn plan(&mut self, entry_ids: &[String]) -> Result<Vec<PlanRow>, String>;

    /// Applies the plans built by the last [`TuiBackend::plan`] call.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message on unexpected failure.
    fn apply(&mut self) -> Result<Vec<ApplyResult>, String>;

    /// Starts re-applying the given entries periodically (freeze)
    /// until [`TuiBackend::stop_freeze`] is called.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message on failure.
    fn start_freeze(&mut self, entry_ids: &[String]) -> Result<(), String>;

    /// Stops the freeze loop started by [`TuiBackend::start_freeze`].
    fn stop_freeze(&mut self);
}

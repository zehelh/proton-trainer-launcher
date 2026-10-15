//! Serializable data types shared by the CLI and the GUI.

use serde::Serialize;

use pt_core::WritePlan;

/// Where a listed game comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameStore {
    /// A Steam game (native or Flatpak Steam).
    Steam,
    /// A Heroic Games Launcher game (Epic, GOG, or configured).
    Heroic,
}

/// One row of the game list, from any store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct GameRow {
    /// The store the game comes from.
    pub store: GameStore,
    /// Steam application id, or Heroic app name.
    pub id: String,
    /// Game name.
    pub name: String,
    /// Id of the native TOML trainer definition covering this game,
    /// when one exists (Steam only).
    pub native_trainer: Option<String>,
    /// Wine prefix: the Steam compatdata prefix, or the Heroic
    /// per-game prefix.
    pub wine_prefix: Option<String>,
    /// Wine or Proton binary (Heroic per-game configuration).
    pub wine_bin: Option<String>,
    /// The game's install directory, when known.
    pub install_path: Option<String>,
    /// The `steamapps/` directory holding the game (Steam only), used
    /// for Proton detection and prefix waiting.
    pub steamapps: Option<String>,
}

/// A trainer entry as shown in the GUI entry list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntryBrief {
    /// Entry id.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Whether the entry freezes its value while the session is open.
    pub freeze: bool,
}

/// The outcome of attempting to attach to a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

/// One prepared write: the dry-run plan shown to the user before any
/// `apply`.
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

/// A serializable view of a [`PlanRow`] for the GUI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanRowView {
    /// Entry id this plan belongs to.
    pub entry_id: String,
    /// Human-readable entry label.
    pub label: String,
    /// Whether this entry should be frozen after applying.
    pub freeze: bool,
    /// Target process id.
    pub pid: i32,
    /// Address where the value will be written.
    pub address: u64,
    /// Start of the containing mapped writable region.
    pub region_start: u64,
    /// End of the containing mapped writable region.
    pub region_end: u64,
    /// Current bytes at the address, hex-encoded.
    pub old_hex: String,
    /// Bytes that would be written, hex-encoded.
    pub new_hex: String,
}

impl From<&PlanRow> for PlanRowView {
    fn from(row: &PlanRow) -> Self {
        Self {
            entry_id: row.entry_id.clone(),
            label: row.label.clone(),
            freeze: row.freeze,
            pid: row.plan.pid,
            address: row.plan.address,
            region_start: row.plan.region.start,
            region_end: row.plan.region.end,
            old_hex: hex(&row.plan.old_bytes),
            new_hex: hex(&row.plan.new_bytes),
        }
    }
}

/// The outcome of applying one plan row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplyResult {
    /// Entry id this result belongs to.
    pub entry_id: String,
    /// Human-readable entry label.
    pub label: String,
    /// `Ok` when the write was applied and verified, an error message
    /// otherwise.
    pub outcome: Result<(), String>,
}

/// Formats one plan row for display (CLI dry-run and logs).
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

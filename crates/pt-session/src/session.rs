//! The attach session: resolving a trainer's target, checking the
//! denylist, building dry-run plans, and applying them.

use std::path::Path;

use crate::types::EntryBrief;
use pt_core::engine::{apply, plan_write};
use pt_core::maps::read_maps;
use pt_core::pattern::scan_process;
use pt_core::process::{find_processes, TargetProcess};
use pt_discover::{scan, ScanOptions};
use pt_trainers::loader::load_dir;
use pt_trainers::{Denylist, Entry, Refusal, Trainer};

use crate::types::{ApplyResult, AttachOutcome, GameRow, GameStore, PlanRow};

/// A successful attach: the trainer and its located target.
#[derive(Debug, Clone)]
pub struct AttachedSession {
    /// The trainer definition in use.
    pub trainer: Trainer,
    /// The located target process.
    pub target: TargetProcess,
}

/// The full result of attaching to a trainer's game.
#[derive(Debug, Clone)]
pub enum AttachReport {
    /// A target was found and passed the denylist check.
    Running(AttachedSession),
    /// No matching process is running.
    NotRunning(Vec<String>),
    /// The target matched the anti-cheat denylist; attach is refused.
    Refused(String),
}

/// Loads trainer definitions from `dir`.
///
/// # Errors
///
/// Returns a human-readable message on any loading or validation
/// failure (the message already names the offending file).
pub fn load_trainers(dir: &Path) -> Result<Vec<Trainer>, String> {
    load_dir(dir).map_err(|error| error.to_string())
}

/// Resolves the running process for a trainer and checks the
/// anti-cheat denylist. Never writes.
///
/// # Errors
///
/// Returns a human-readable message on unexpected failures (process
/// listing, maps reading).
pub fn attach_trainer(trainer: &Trainer, denylist: &Denylist) -> Result<AttachReport, String> {
    for exe_name in &trainer.exe_names {
        let found = find_processes(exe_name).map_err(|error| error.to_string())?;
        let Some(target) = found.into_iter().next() else {
            continue;
        };
        if let Some(refusal) = check_denylist(&target, denylist)? {
            tracing::warn!(
                pid = target.pid,
                component = %refusal.component,
                "attach refused by denylist"
            );
            return Ok(AttachReport::Refused(refusal.explanation()));
        }
        tracing::info!(pid = target.pid, exe = %target.exe_name, "attached to target");
        return Ok(AttachReport::Running(AttachedSession {
            trainer: trainer.clone(),
            target,
        }));
    }
    Ok(AttachReport::NotRunning(trainer.exe_names.clone()))
}

/// Checks the target against the denylist using its exe name and its
/// mapped file paths.
///
/// # Errors
///
/// Returns a human-readable message when the target's maps cannot be
/// read or the denylist check fails.
pub fn check_denylist(
    target: &TargetProcess,
    denylist: &Denylist,
) -> Result<Option<Refusal>, String> {
    let maps = read_maps(target.pid).map_err(|error| error.to_string())?;
    let paths: Vec<String> = maps
        .iter()
        .filter_map(|region| region.pathname.clone())
        .collect();
    denylist
        .check(&target.exe_name, &paths)
        .map_err(|error| error.to_string())
}

/// Converts an [`AttachReport`] into the serializable
/// [`AttachOutcome`] used by the GUI.
pub fn attach_outcome(report: &AttachReport) -> AttachOutcome {
    match report {
        AttachReport::Running(session) => AttachOutcome::Running {
            pid: session.target.pid,
            exe_name: session.target.exe_name.clone(),
            entries: session
                .trainer
                .entries
                .iter()
                .map(|entry| EntryBrief {
                    id: entry.id.clone(),
                    label: entry.label.clone(),
                    freeze: entry.freeze,
                })
                .collect(),
        },
        AttachReport::NotRunning(exe_names) => AttachOutcome::NotRunning {
            exe_names: exe_names.clone(),
        },
        AttachReport::Refused(explanation) => AttachOutcome::Refused {
            explanation: explanation.clone(),
        },
    }
}

/// Steam app ids that are runtimes and redistributables, not games.
const IGNORED_STEAM_APP_IDS: &[u32] = &[
    228980,  // Steamworks Common Redistributables
    1070560, // Steam Linux Runtime 1.0 (scout)
    1391110, // Steam Linux Runtime 2.0 (soldier)
    1628350, // Proton EasyAntiCheat Runtime
    1161040, // Proton BattlEye Runtime
    1826350, // Steam Linux Runtime 3.0 (sniper)
    1826330, // Proton EasyAntiCheat Runtime (older id)
    961879,  // Proton 6.3 runtime
];

/// Name prefixes that mark runtimes and tools rather than games.
const IGNORED_NAME_PREFIXES: &[&str] = &[
    "Proton ",             // "Proton 10.0", "Proton Hotfix", ...
    "Steam Linux Runtime", // sniper/soldier/scout variants
    "Steamworks Common Redist",
];

/// Returns `true` when a Steam entry is a runtime, not a game.
fn is_steam_runtime(app_id: u32, name: &str) -> bool {
    if IGNORED_STEAM_APP_IDS.contains(&app_id) {
        return true;
    }
    IGNORED_NAME_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Lists every installed game from every store (Steam libraries —
/// including secondary disks — and Heroic), with per-game launch data.
///
/// # Errors
///
/// Returns a human-readable message on Steam or Heroic discovery
/// failures.
pub fn all_games(trainers_dir: &Path) -> Result<Vec<GameRow>, String> {
    let trainers = load_trainers(trainers_dir)?;
    let options = ScanOptions::from_default_roots().map_err(|error| error.to_string())?;
    let result = scan(&options).map_err(|error| error.to_string())?;

    let mut rows: Vec<GameRow> = result
        .games
        .iter()
        .filter(|game| !is_steam_runtime(game.app_id, &game.name))
        .map(|game| GameRow {
            store: GameStore::Steam,
            id: game.app_id.to_string(),
            name: game.name.clone(),
            native_trainer: trainers
                .iter()
                .find(|trainer| trainer.steam_app_id == game.app_id)
                .map(|trainer| trainer.id.clone()),
            wine_prefix: game
                .compatdata
                .as_ref()
                .map(|c| c.prefix.join("pfx").to_string_lossy().to_string()),
            wine_bin: None,
            steamapps: Some(game.steamapps_dir.to_string_lossy().to_string()),
        })
        .collect();

    for heroic in pt_discover::scan_heroic().map_err(|error| error.to_string())? {
        rows.push(GameRow {
            store: GameStore::Heroic,
            id: heroic.app_name,
            name: heroic.title,
            native_trainer: None,
            wine_prefix: heroic
                .wine_prefix
                .map(|prefix| prefix.to_string_lossy().to_string()),
            wine_bin: heroic.wine_bin.map(|bin| bin.to_string_lossy().to_string()),
            steamapps: None,
        });
    }

    rows.sort_by_key(|row| row.name.to_lowercase());
    Ok(rows)
}

/// Builds the dry-run plans for the given entries. Never writes and
/// never stops the target.
///
/// Ambiguous patterns (several matches) and missing patterns are
/// refused with the entry named.
///
/// # Errors
///
/// Returns a human-readable message naming the entry on failure.
pub fn plan_entries(
    session: &AttachedSession,
    entry_ids: &[String],
) -> Result<Vec<PlanRow>, String> {
    let mut rows = Vec::new();
    for id in entry_ids {
        let entry: &Entry = session
            .trainer
            .entry(id)
            .ok_or_else(|| format!("entry {id:?} does not exist in this trainer"))?;
        let pattern = entry
            .search
            .to_pattern()
            .map_err(|error| format!("entry {id:?} has an invalid pattern: {error}"))?;
        let matches =
            scan_process(session.target.pid, &pattern).map_err(|error| error.to_string())?;
        let address = match matches.as_slice() {
            [] => {
                return Err(format!(
                    "entry {id:?}: pattern not found in the running process"
                ))
            }
            [address] => *address,
            many => {
                return Err(format!(
                    "entry {id:?}: pattern is ambiguous, {} matches at {:#x?}; \
                     refusing to guess",
                    many.len(),
                    many
                ))
            }
        };
        let value_bytes = entry
            .value
            .to_bytes()
            .map_err(|error| format!("entry {id:?}: invalid value: {error}"))?;
        let plan = plan_write(
            session.target.pid,
            entry.value_address(address),
            &value_bytes,
        )
        .map_err(|error| format!("entry {id:?}: {error}"))?;
        tracing::info!(entry = %id, address = format!("{:#x}", plan.address), "planned");
        rows.push(PlanRow {
            entry_id: entry.id.clone(),
            label: entry.label.clone(),
            freeze: entry.freeze,
            plan,
        });
    }
    Ok(rows)
}

/// Applies plans, verifying every write. Never leaves the target
/// stopped.
///
/// # Errors
///
/// Returns a human-readable message when applying cannot even start
/// (no plans). Per-entry failures are reported in the results.
pub fn apply_plans(plans: &[PlanRow]) -> Result<Vec<ApplyResult>, String> {
    if plans.is_empty() {
        return Err("nothing to apply: no plans were built".to_string());
    }
    let mut results = Vec::new();
    for row in plans {
        let outcome = match apply(&row.plan) {
            Ok(()) => {
                tracing::info!(entry = %row.entry_id, "applied and verified");
                Ok(())
            }
            Err(error) => {
                tracing::warn!(entry = %row.entry_id, %error, "apply failed");
                Err(error.to_string())
            }
        };
        results.push(ApplyResult {
            entry_id: row.entry_id.clone(),
            label: row.label.clone(),
            outcome,
        });
    }
    Ok(results)
}

/// The stateful session used by the GUI: one attach at a time, with
/// the last plans and the freeze loop.
pub struct LiveSession {
    trainers_dir: std::path::PathBuf,
    denylist: Denylist,
    state: Option<AttachedSession>,
    plans: Vec<PlanRow>,
    freeze: Option<crate::freeze::FreezeHandle>,
}

impl LiveSession {
    /// Creates the session with the denylist compiled into the binary.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message when the embedded denylist is
    /// invalid (a build-time authoring mistake).
    pub fn new(trainers_dir: std::path::PathBuf) -> Result<Self, String> {
        let denylist = Denylist::embedded().map_err(|error| error.to_string())?;
        Ok(Self {
            trainers_dir,
            denylist,
            state: None,
            plans: Vec::new(),
            freeze: None,
        })
    }

    /// Lists every installed game from every store.
    ///
    /// # Errors
    ///
    /// See [`all_games`].
    pub fn games(&self) -> Result<Vec<GameRow>, String> {
        all_games(&self.trainers_dir)
    }

    /// Attaches to the game covered by `trainer_id`. Never writes.
    /// Any previous attach (and its freeze loop) is released first.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message when the trainer cannot be
    /// found.
    pub fn attach(&mut self, trainer_id: &str) -> Result<AttachOutcome, String> {
        self.detach();
        let trainers = load_trainers(&self.trainers_dir)?;
        let trainer = trainers
            .iter()
            .find(|trainer| trainer.id == trainer_id)
            .ok_or_else(|| {
                let ids: Vec<&str> = trainers.iter().map(|t| t.id.as_str()).collect();
                format!("trainer {trainer_id:?} not found; available: {ids:?}")
            })?
            .clone();
        let report = attach_trainer(&trainer, &self.denylist)?;
        if let AttachReport::Running(session) = &report {
            self.state = Some(session.clone());
        }
        Ok(attach_outcome(&report))
    }

    /// The attached session, if any.
    pub fn attached(&self) -> Option<&AttachedSession> {
        self.state.as_ref()
    }

    /// Builds the dry-run plans for the given entries of the attached
    /// trainer. Never writes.
    ///
    /// # Errors
    ///
    /// See [`plan_entries`].
    pub fn plan(&mut self, entry_ids: &[String]) -> Result<Vec<PlanRow>, String> {
        let session = self
            .state
            .as_ref()
            .ok_or_else(|| "not attached: select a game first".to_string())?;
        let plans = plan_entries(session, entry_ids)?;
        self.plans = plans.clone();
        Ok(plans)
    }

    /// Applies the plans built by the last [`LiveSession::plan`].
    ///
    /// # Errors
    ///
    /// See [`apply_plans`].
    pub fn apply(&mut self) -> Result<Vec<ApplyResult>, String> {
        apply_plans(&self.plans)
    }

    /// Starts re-applying the given entries periodically (freeze)
    /// until [`LiveSession::stop_freeze`] or [`LiveSession::detach`].
    ///
    /// # Errors
    ///
    /// Returns a human-readable message when the loop cannot start.
    pub fn start_freeze(&mut self, entry_ids: &[String]) -> Result<(), String> {
        let plans: Vec<PlanRow> = self
            .plans
            .iter()
            .filter(|row| entry_ids.contains(&row.entry_id) && row.freeze)
            .cloned()
            .collect();
        if plans.is_empty() {
            return Ok(());
        }
        let handle = crate::freeze::spawn_freeze(plans)?;
        self.freeze = Some(handle);
        Ok(())
    }

    /// Stops the freeze loop.
    pub fn stop_freeze(&mut self) {
        self.freeze = None;
    }

    /// Releases the attach: stops the freeze loop and clears the
    /// session and plans.
    pub fn detach(&mut self) {
        self.stop_freeze();
        self.state = None;
        self.plans.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_entries_are_filtered_by_id_and_name() {
        assert!(is_steam_runtime(
            228980,
            "Steamworks Common Redistributables"
        ));
        assert!(is_steam_runtime(123, "Proton 10.0"));
        assert!(is_steam_runtime(123, "Proton Hotfix"));
        assert!(is_steam_runtime(123, "Steam Linux Runtime 3.0 (sniper)"));
        assert!(!is_steam_runtime(367520, "Hollow Knight"));
        assert!(!is_steam_runtime(2246340, "Monster Hunter Wilds"));
    }
}

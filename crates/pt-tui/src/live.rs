//! The production [`TuiBackend`]: wires pt-discover (games),
//! pt-trainers (definitions + denylist), and pt-core (attach, scan,
//! plan, apply, freeze).
//!
//! The non-UI workflow functions ([`attach_trainer`],
//! [`plan_entries`], [`apply_plans`]) are public so the CLI's headless
//! `attach`/`apply` commands share the exact same logic as the TUI.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use pt_core::engine::{apply, plan_write};
use pt_core::maps::read_maps;
use pt_core::pattern::scan_process;
use pt_core::process::{find_processes, TargetProcess};
use pt_core::write_bytes;
use pt_discover::{scan, ScanOptions};
use pt_trainers::loader::load_dir;
use pt_trainers::{Denylist, Entry, Refusal, Trainer};

use crate::backend::{ApplyResult, AttachOutcome, EntryBrief, GameRow, PlanRow, TuiBackend};

/// How often freeze entries are re-applied while the session is open.
const FREEZE_INTERVAL: Duration = Duration::from_millis(33);

/// The result of a successful attach: the trainer and its target.
#[derive(Debug, Clone)]
pub struct AttachedSession {
    /// The trainer definition in use.
    pub trainer: Trainer,
    /// The located target process.
    pub target: TargetProcess,
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
            tracing::warn!(pid = target.pid, component = %refusal.component, "attach refused by denylist");
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

/// Converts an [`AttachReport`] into the TUI's [`AttachOutcome`].
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

/// Checks the target against the denylist using its exe name and its
/// mapped file paths.
fn check_denylist(target: &TargetProcess, denylist: &Denylist) -> Result<Option<Refusal>, String> {
    let maps = read_maps(target.pid).map_err(|error| error.to_string())?;
    let paths: Vec<String> = maps
        .iter()
        .filter_map(|region| region.pathname.clone())
        .collect();
    denylist
        .check(&target.exe_name, &paths)
        .map_err(|error| error.to_string())
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

/// One freeze tick: re-writes the frozen plans through the injected
/// writer. Returns how many writes succeeded.
///
/// # Errors
///
/// Errors are counted as failures; the function itself never fails.
pub fn freeze_tick_with(
    plans: &[PlanRow],
    writer: impl Fn(&PlanRow) -> Result<(), String>,
) -> usize {
    plans
        .iter()
        .filter(|row| row.freeze)
        .filter(|row| writer(row).is_ok())
        .count()
}

/// The production backend used by both the TUI and the headless CLI.
pub struct LiveBackend {
    trainers_dir: PathBuf,
    denylist: Denylist,
    session: Option<AttachedSession>,
    plans: Vec<PlanRow>,
    freeze: Option<FreezeHandle>,
}

impl LiveBackend {
    /// Creates the backend, loading the anti-cheat denylist compiled
    /// into the binary.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message when the embedded denylist is
    /// invalid (a build-time authoring mistake).
    pub fn new(trainers_dir: PathBuf) -> Result<Self, String> {
        let denylist = Denylist::embedded().map_err(|error| error.to_string())?;
        Ok(Self {
            trainers_dir,
            denylist,
            session: None,
            plans: Vec::new(),
            freeze: None,
        })
    }

    /// The attached session, if any.
    pub fn session(&self) -> Option<&AttachedSession> {
        self.session.as_ref()
    }
}

impl TuiBackend for LiveBackend {
    fn games(&mut self) -> Result<Vec<GameRow>, String> {
        let trainers = load_trainers(&self.trainers_dir)?;
        let options = ScanOptions::from_default_roots().map_err(|error| error.to_string())?;
        let result = scan(&options).map_err(|error| error.to_string())?;
        let rows = result
            .games
            .iter()
            .filter_map(|game| {
                let trainer = trainers
                    .iter()
                    .find(|trainer| trainer.steam_app_id == game.app_id)?;
                Some(GameRow {
                    app_id: game.app_id,
                    name: game.name.clone(),
                    trainer_id: trainer.id.clone(),
                })
            })
            .collect::<Vec<_>>();
        Ok(rows)
    }

    fn attach(&mut self, trainer_id: &str) -> Result<AttachOutcome, String> {
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
            self.session = Some(session.clone());
        }
        Ok(attach_outcome(&report))
    }

    fn plan(&mut self, entry_ids: &[String]) -> Result<Vec<PlanRow>, String> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| "not attached: select a game first".to_string())?;
        let plans = plan_entries(session, entry_ids)?;
        self.plans = plans.clone();
        Ok(plans)
    }

    fn apply(&mut self) -> Result<Vec<ApplyResult>, String> {
        apply_plans(&self.plans)
    }

    fn start_freeze(&mut self, entry_ids: &[String]) -> Result<(), String> {
        let plans: Vec<PlanRow> = self
            .plans
            .iter()
            .filter(|row| entry_ids.contains(&row.entry_id) && row.freeze)
            .cloned()
            .collect();
        if plans.is_empty() {
            return Ok(());
        }
        let handle = spawn_freeze(plans).map_err(|error| error.to_string())?;
        self.freeze = Some(handle);
        Ok(())
    }

    fn stop_freeze(&mut self) {
        self.freeze = None;
    }
}

/// A joinable handle to the background freeze loop.
pub struct FreezeHandle {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for FreezeHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Spawns the freeze loop: re-writes every freeze plan every 33 ms
/// until the handle is dropped.
///
/// Each tick is a single write without SIGSTOP and without read-back
/// verification (the value was verified when first applied); the plan
/// was validated at plan time, and writes to unmapped memory fail
/// harmlessly in the kernel.
///
/// # Errors
///
/// Returns an error string when the thread cannot be spawned.
pub fn spawn_freeze(plans: Vec<PlanRow>) -> Result<FreezeHandle, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let handle = std::thread::Builder::new()
        .name("pt-freeze".to_string())
        .spawn(move || {
            let freeze_plans: Vec<PlanRow> = plans.into_iter().filter(|row| row.freeze).collect();
            while !thread_stop.load(Ordering::SeqCst) {
                for row in &freeze_plans {
                    if let Err(error) =
                        write_bytes(row.plan.pid, row.plan.address, &row.plan.new_bytes)
                    {
                        tracing::warn!(entry = %row.entry_id, %error, "freeze tick failed");
                    }
                }
                std::thread::sleep(FREEZE_INTERVAL);
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(FreezeHandle {
        stop,
        handle: Some(handle),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{format_plan_row, hex};
    use pt_core::maps::Region;

    fn sample_row(freeze: bool) -> PlanRow {
        PlanRow {
            entry_id: "soul".to_string(),
            label: "Infinite SOUL".to_string(),
            freeze,
            plan: pt_core::WritePlan {
                pid: 42,
                address: 0x1000,
                old_bytes: vec![0x39, 0x05],
                new_bytes: vec![0x63, 0x00],
                region: Region {
                    start: 0x1000,
                    end: 0x2000,
                    readable: true,
                    writable: true,
                    executable: false,
                    private: true,
                    pathname: None,
                },
            },
        }
    }

    #[test]
    fn format_plan_row_shows_entry_address_and_bytes() {
        let text = format_plan_row(&sample_row(true));
        assert!(text.contains("soul"));
        assert!(text.contains("Infinite SOUL"));
        assert!(text.contains("[freeze]"));
        assert!(text.contains("0x1000"));
        assert!(text.contains("39 05"));
        assert!(text.contains("63 00"));
    }

    #[test]
    fn hex_formats_bytes() {
        assert_eq!(hex(&[0xde, 0xad]), "de ad");
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn freeze_tick_only_touches_freeze_rows() {
        let rows = vec![sample_row(true), sample_row(false)];
        let written: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
        let count = freeze_tick_with(&rows, |row| {
            written.lock().expect("lock").push(row.entry_id.clone());
            Ok(())
        });
        assert_eq!(count, 1);
        assert_eq!(written.into_inner().expect("into_inner"), vec!["soul"]);
    }

    #[test]
    fn freeze_tick_counts_failures() {
        let rows = vec![sample_row(true)];
        let count = freeze_tick_with(&rows, |_| Err("gone".to_string()));
        assert_eq!(count, 0);
    }

    #[test]
    fn spawn_and_stop_freeze_thread() {
        let handle = spawn_freeze(vec![sample_row(true)]).expect("spawn");
        std::thread::sleep(Duration::from_millis(80));
        // Writing to pid 42 fails harmlessly; the loop must survive it.
        drop(handle);
        // Joining succeeded (no hang): reaching here is the assertion.
    }
}

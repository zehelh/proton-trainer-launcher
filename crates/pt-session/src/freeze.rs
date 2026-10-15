//! The freeze loop: re-applying frozen values in the background.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use pt_core::maps::{read_maps, validate_writable_range};
use pt_core::{read_bytes, write_bytes};

use crate::types::PlanRow;

/// How often freeze entries are re-applied while the session is open.
const FREEZE_INTERVAL: Duration = Duration::from_millis(33);

/// One freeze tick: re-applies the frozen plans through the injected
/// writer. Returns how many entries were held.
///
/// # Errors
///
/// Failures are only counted; the function itself never fails. The
/// live writer ([`freeze_write`]) re-validates and verifies each write.
pub fn freeze_tick(plans: &[PlanRow], write_one: impl Fn(&PlanRow) -> Result<(), String>) -> usize {
    plans
        .iter()
        .filter(|row| row.freeze)
        .filter(|row| write_one(row).is_ok())
        .count()
}

/// One frozen re-write, honoring the project's write rules even in the
/// background loop: re-validate the target range against a fresh maps
/// read, write, read back, and verify.
///
/// The plan's region data from apply time is never trusted across
/// ticks: if the game re-mapped the address away, the tick is skipped
/// instead of writing into whatever lives there now.
///
/// # Errors
///
/// Returns a human-readable message when validation, the write, or the
/// verification fails.
pub fn freeze_write(row: &PlanRow) -> Result<(), String> {
    let plan = &row.plan;
    let regions = read_maps(plan.pid).map_err(|error| error.to_string())?;
    validate_writable_range(
        &regions,
        plan.pid,
        plan.address,
        plan.address + plan.new_bytes.len() as u64,
    )
    .map_err(|error| error.to_string())?;
    write_bytes(plan.pid, plan.address, &plan.new_bytes).map_err(|error| error.to_string())?;
    let mut verify = vec![0u8; plan.new_bytes.len()];
    read_bytes(plan.pid, plan.address, &mut verify).map_err(|error| error.to_string())?;
    if verify != plan.new_bytes {
        return Err("read-back verification failed".to_string());
    }
    Ok(())
}

/// [`freeze_write`] with per-entry warning logging, used by the
/// background loop.
fn freeze_row(row: &PlanRow) -> Result<(), String> {
    freeze_write(row).map_err(|error| {
        tracing::warn!(entry = %row.entry_id, %error, "freeze tick skipped");
        error
    })
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
/// Each tick re-validates the target range against a fresh maps read,
/// writes, and verifies the read-back (see [`freeze_write`]); no
/// SIGSTOP is used because a tick is a single small write, never a
/// multi-write sequence.
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
                freeze_tick(&freeze_plans, freeze_row);
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
    use crate::types::{format_plan_row, hex};
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
        let count = freeze_tick(&rows, |row| {
            written.lock().expect("lock").push(row.entry_id.clone());
            Ok(())
        });
        assert_eq!(count, 1);
        assert_eq!(written.into_inner().expect("into_inner"), vec!["soul"]);
    }

    #[test]
    fn freeze_tick_counts_failures() {
        let rows = vec![sample_row(true)];
        let count = freeze_tick(&rows, |_| Err("gone".to_string()));
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

//! Headless CLI subcommands, sharing the same session workflow as the
//! GUI.

use anyhow::{anyhow, Context};
use std::path::PathBuf;

use pt_discover::cache;
use pt_discover::scan::{scan, ScanOptions, ScanResult};
use pt_session::session::{attach_trainer, plan_entries, AttachReport};
use pt_session::types::{format_plan_row, PlanRow};
use pt_trainers::Denylist;

/// Resolves the trainers directory: explicit flag, then the XDG config
/// directory, then install layouts, then `./trainers`.
pub fn resolve_trainers_dir(flag: Option<&PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(dir) = flag {
        if dir.is_dir() {
            return Ok(dir.clone());
        }
        return Err(anyhow!("trainers directory not found: {}", dir.display()));
    }

    if let Some(config) = dirs::config_dir() {
        let dir = config.join("proton-trainer").join("trainers");
        if dir.is_dir() {
            return Ok(dir);
        }
    }
    // Install layouts (tarball, /usr/local): the trainers directory
    // ships next to the binary under ../share/.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(share) = exe.parent().and_then(|parent| parent.parent()) {
            let dir = share.join("share").join("proton-trainer").join("trainers");
            if dir.is_dir() {
                return Ok(dir);
            }
        }
    }
    let local = PathBuf::from("trainers");
    if local.is_dir() {
        return Ok(local);
    }

    Err(anyhow!(
        "no trainers directory found; pass --trainers-dir, create \
         $XDG_CONFIG_HOME/proton-trainer/trainers, or run from a directory \
         containing ./trainers"
    ))
}

/// Implementation of `proton-trainer list`.
pub fn cmd_list(cached: bool) -> anyhow::Result<()> {
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

/// Loads a trainer by id from the resolved trainers directory.
fn find_trainer(
    trainers_dir: Option<&PathBuf>,
    trainer_id: &str,
) -> anyhow::Result<pt_trainers::Trainer> {
    let dir = resolve_trainers_dir(trainers_dir)?;
    let trainers = pt_trainers::load_dir(&dir)
        .with_context(|| format!("failed to load trainers from {}", dir.display()))?;
    let trainer = trainers
        .into_iter()
        .find(|trainer| trainer.id == trainer_id)
        .ok_or_else(|| anyhow!("trainer {trainer_id:?} not found in {}", dir.display()))?;
    Ok(trainer)
}

/// Prints the plan rows in the CLI dry-run format.
fn print_plans(plans: &[PlanRow]) {
    println!();
    for row in plans {
        println!("{}", format_plan_row(row));
    }
}

/// Implementation of `proton-trainer attach` (read-only dry-run plan).
pub fn cmd_attach(trainer_id: &str, trainers_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let trainer = find_trainer(trainers_dir.as_ref(), trainer_id)?;
    let denylist = Denylist::embedded().context("the embedded anti-cheat denylist is invalid")?;
    match attach_trainer(&trainer, &denylist).map_err(|e| anyhow!("attach failed: {e}"))? {
        AttachReport::Running(session) => {
            println!(
                "Attached to {} (pid {})",
                session.target.exe_name, session.target.pid
            );
            println!("Trainer: {} v{}", trainer.id, trainer.version);
            let ids: Vec<String> = trainer.entries.iter().map(|e| e.id.clone()).collect();
            let plans = plan_entries(&session, &ids)
                .map_err(|error| anyhow!("building the write plan failed: {error}"))?;
            println!("Dry-run plan (nothing has been written):");
            print_plans(&plans);
            Ok(())
        }
        AttachReport::NotRunning(exe_names) => Err(anyhow!(
            "no running process found for {exe_names:?}; launch the game first"
        )),
        AttachReport::Refused(explanation) => {
            eprintln!("REFUSED: {explanation}");
            Err(anyhow!("attach refused by the anti-cheat denylist"))
        }
    }
}

/// Implementation of `proton-trainer apply`: dry-run by default,
/// writes only with `--apply`.
pub fn cmd_apply(
    trainer_id: &str,
    trainers_dir: Option<PathBuf>,
    write: bool,
) -> anyhow::Result<()> {
    let trainer = find_trainer(trainers_dir.as_ref(), trainer_id)?;
    let denylist = Denylist::embedded().context("the embedded anti-cheat denylist is invalid")?;
    let report = attach_trainer(&trainer, &denylist).map_err(|e| anyhow!("attach failed: {e}"))?;
    let session = match report {
        AttachReport::Running(session) => session,
        AttachReport::NotRunning(exe_names) => {
            return Err(anyhow!(
                "no running process found for {exe_names:?}; launch the game first"
            ))
        }
        AttachReport::Refused(explanation) => {
            eprintln!("REFUSED: {explanation}");
            return Err(anyhow!("attach refused by the anti-cheat denylist"));
        }
    };
    println!(
        "Attached to {} (pid {})",
        session.target.exe_name, session.target.pid
    );
    println!("Trainer: {} v{}", trainer.id, trainer.version);

    let ids: Vec<String> = trainer.entries.iter().map(|e| e.id.clone()).collect();
    let plans = plan_entries(&session, &ids)
        .map_err(|error| anyhow!("building the write plan failed: {error}"))?;

    if !write {
        println!("Dry-run plan (nothing has been written; pass --apply to write):");
        print_plans(&plans);
        return Ok(());
    }

    println!("Applying {} plan(s):", plans.len());
    print_plans(&plans);
    let results = pt_session::session::apply_plans(&plans)
        .map_err(|error| anyhow!("apply failed: {error}"))?;
    let mut failures = 0;
    for result in &results {
        match &result.outcome {
            Ok(()) => println!(
                "{} ({}): applied and verified",
                result.entry_id, result.label
            ),
            Err(error) => {
                failures += 1;
                println!("{} ({}): FAILED - {error}", result.entry_id, result.label);
            }
        }
    }
    if failures > 0 {
        return Err(anyhow!("{failures} of {} entries failed", results.len()));
    }
    Ok(())
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

    #[test]
    fn resolve_trainers_dir_fails_for_missing_explicit_dir() {
        assert!(resolve_trainers_dir(Some(&PathBuf::from("/nonexistent-xyz"))).is_err());
    }

    #[test]
    fn print_games_handles_empty_results() {
        let result = ScanResult {
            scanned_at_unix: 0,
            games: Vec::new(),
        };
        // Must not panic on an empty list.
        print_games(&result);
    }
}

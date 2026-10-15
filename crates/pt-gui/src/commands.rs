//! Tauri commands: thin, thread-safe wrappers around the
//! [`pt_session`] workflow. The frontend calls these via `invoke`.
//!
//! All commands run their blocking work (scans can take seconds on
//! Wine games) on a blocking thread, so the window never freezes; the
//! frontend gets a promise and can show a spinner meanwhile.

use std::path::Path;
use std::sync::{Arc, Mutex};

use pt_session::info::{game_info as game_info_of, GameInfo};
use pt_session::launch::{launch_game, launch_with_trainer};
use pt_session::types::{ApplyResult, AttachOutcome, GameRow, PlanRowView};
use pt_session::{AppConfig, LiveSession};

use crate::GuiState;

/// Lists every installed game from every store.
#[tauri::command]
pub async fn list_games(state: tauri::State<'_, GuiState>) -> Result<Vec<GameRow>, String> {
    run_blocking(&state, |session| session.games()).await
}

/// Attaches to the game covered by `trainer_id`: resolves the running
/// process and checks the anti-cheat denylist. Never writes.
#[tauri::command]
pub async fn attach(
    state: tauri::State<'_, GuiState>,
    trainer_id: String,
) -> Result<AttachOutcome, String> {
    run_blocking(&state, move |session| session.attach(&trainer_id)).await
}

/// Builds the dry-run plans for the given entries of the attached
/// trainer. Never writes.
#[tauri::command]
pub async fn plan(
    state: tauri::State<'_, GuiState>,
    entry_ids: Vec<String>,
) -> Result<Vec<PlanRowView>, String> {
    run_blocking(&state, move |session| session.plan(&entry_ids))
        .await
        .map(|plans| plans.iter().map(PlanRowView::from).collect())
}

/// Applies the plans built by the last [`plan`] command; when the
/// applied entries are frozen, the freeze loop is started.
#[tauri::command]
pub async fn apply(state: tauri::State<'_, GuiState>) -> Result<Vec<ApplyResult>, String> {
    let results = run_blocking(&state, |session| session.apply()).await?;
    let applied: Vec<String> = results
        .iter()
        .filter(|result| result.outcome.is_ok())
        .map(|result| result.entry_id.clone())
        .collect();
    tracing::debug!(entries = ?applied, "freeze loop started where applicable");
    run_blocking(&state, move |session| {
        session.start_freeze(&applied)?;
        Ok(())
    })
    .await?;
    Ok(results)
}

/// Releases the attach: stops the freeze loop and clears the session.
#[tauri::command]
pub async fn stop_session(state: tauri::State<'_, GuiState>) -> Result<(), String> {
    run_blocking(&state, |session| {
        session.detach();
        Ok(())
    })
    .await
}

/// Loads the user configuration (global trainers, per-game trainers).
#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    AppConfig::load()
}

/// Validates and saves the whole user configuration.
#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<(), String> {
    AppConfig::save_to(&AppConfig::path()?, &config)
}

/// Collects the detail-view information for one game (install
/// path, compatibility tool, GE-Proton recommendation).
#[tauri::command]
pub async fn game_info(row: GameRow) -> Result<GameInfo, String> {
    tauri::async_runtime::spawn_blocking(move || game_info_of(&row))
        .await
        .map_err(|error| error.to_string())
}

/// Launches a game without any trainer.
#[tauri::command]
pub async fn launch(row: GameRow) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || launch_game(&row))
        .await
        .map_err(|error| error.to_string())?
}

/// Launches a game and runs the trainer in its Wine/Proton prefix.
/// Blocking (waits for the prefix on first runs), hence the worker
/// thread. Returns an optional user-facing warning when the trainer
/// had to be started a suboptimal way (e.g. outside the game's
/// container).
#[tauri::command]
pub async fn launch_trainer(row: GameRow, trainer_path: String) -> Result<Option<String>, String> {
    let warning = tauri::async_runtime::spawn_blocking(move || {
        launch_with_trainer(&row, Path::new(&trainer_path))
    })
    .await
    .map_err(|error| error.to_string())??;
    Ok(if warning.is_empty() {
        None
    } else {
        Some(warning)
    })
}

/// Runs `work` with the session lock on a blocking thread.
async fn run_blocking<T, F>(state: &tauri::State<'_, GuiState>, work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut LiveSession) -> Result<T, String> + Send + 'static,
{
    let state: Arc<Mutex<LiveSession>> = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let mut session = state.lock().map_err(|_| "session poisoned")?;
        work(&mut session)
    })
    .await
    .map_err(|error| error.to_string())?
}

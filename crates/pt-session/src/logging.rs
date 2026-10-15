//! The application log file location: everything the app and the
//! launched trainers print lands here, so problems on a user's
//! machine are diagnosable after the fact.

use std::path::PathBuf;

/// Returns the log file path:
/// `$XDG_STATE_HOME/proton-trainer/proton-trainer.log` (falling back
/// to `~/.local/state/proton-trainer/proton-trainer.log`), creating
/// the directory.
///
/// # Errors
///
/// Returns a human-readable message when the directory cannot be
/// determined or created.
pub fn log_file() -> Result<PathBuf, String> {
    let state = dirs::state_dir().ok_or("could not find the state directory")?;
    let dir = state.join("proton-trainer");
    std::fs::create_dir_all(&dir)
        .map_err(|source| format!("failed to create {}: {source}", dir.display()))?;
    Ok(dir.join("proton-trainer.log"))
}

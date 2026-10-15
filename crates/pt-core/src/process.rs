//! Process discovery via `/proc` and ptrace restriction detection.

use std::path::{Path, PathBuf};

use crate::error::CoreError;

/// A discovered target process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetProcess {
    /// Process id.
    pub pid: i32,
    /// Basename of the executable (e.g. `game.exe`), lowercased.
    pub exe_name: String,
}

/// Finds all processes whose executable basename matches `name`
/// (case-insensitive).
///
/// Wine/Proton game processes expose their `.exe` through
/// `/proc/<pid>/exe`, so this works for both native Linux and Proton
/// games. Kernel threads and processes of other users are skipped
/// silently (their `exe` link is not readable).
///
/// # Errors
///
/// Returns [`CoreError::Io`] when `/proc` cannot be listed.
pub fn find_processes(name: &str) -> Result<Vec<TargetProcess>, CoreError> {
    let wanted = name.to_ascii_lowercase();
    let mut found = Vec::new();

    let entries = std::fs::read_dir("/proc").map_err(|source| CoreError::Io {
        path: PathBuf::from("/proc"),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| CoreError::Io {
            path: PathBuf::from("/proc"),
            source,
        })?;
        let Some(pid) = pid_from_name(&entry.file_name()) else {
            continue;
        };
        let Some(exe_name) = exe_basename(pid) else {
            continue;
        };
        if exe_name == wanted {
            found.push(TargetProcess { pid, exe_name });
        }
    }
    found.sort_by_key(|process| process.pid);
    Ok(found)
}

/// Parses a `/proc` entry name into a pid; `None` for non-numeric names.
fn pid_from_name(name: &std::ffi::OsStr) -> Option<i32> {
    name.to_str()?.parse().ok()
}

/// Returns the lowercased basename of `/proc/<pid>/exe`, or `None` when
/// the link cannot be read (kernel thread, other user, gone process).
fn exe_basename(pid: i32) -> Option<String> {
    let link = std::fs::read_link(Path::new("/proc").join(pid.to_string()).join("exe")).ok()?;
    let name = link.file_name()?.to_str()?.to_ascii_lowercase();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Reads `/proc/sys/kernel/yama/ptrace_scope`.
///
/// Returns `Ok(None)` when the file does not exist (the kernel was
/// built without Yama, so there is no restriction).
///
/// # Errors
///
/// Returns [`CoreError::Io`] when the file exists but cannot be read.
pub fn ptrace_scope() -> Result<Option<u32>, CoreError> {
    let path = Path::new("/proc/sys/kernel/yama/ptrace_scope");
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(CoreError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    let scope = text.trim().parse::<u32>().map_err(|source| CoreError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::other(source),
    })?;
    Ok(Some(scope))
}

/// Builds the plain-English fix shown when memory access is denied
/// with `EPERM`, tailored to the current ptrace restriction.
pub fn ptrace_advice(scope: Option<u32>) -> String {
    match scope {
        None | Some(0) => "Yama does not restrict tracing on this system, so the target is \
             likely isolated in a sandbox (Flatpak Steam or pressure-vessel). \
             Run the trainer from the same environment, or as the same user \
             outside the sandbox, and try again."
            .to_string(),
        Some(1) => "Yama ptrace_scope is 1 (parent-only tracing). Either relax it for \
             this session: sudo sysctl kernel.yama.ptrace_scope=0, or grant the \
             binary the capability once: sudo setcap cap_sys_ptrace=ep \
             <path-to>/proton-trainer"
            .to_string(),
        Some(_) => "Yama ptrace_scope restricts tracing to admin-approved processes \
             on this system. An administrator must relax it: sudo sysctl \
             kernel.yama.ptrace_scope=0"
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_processes_rejects_unknown_names() {
        assert!(find_processes("definitely-not-a-real-process-xyz")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn find_processes_finds_this_test_runner() {
        // The integration test binary runs as a process we can find by
        // its own name.
        let exe = std::env::current_exe().unwrap();
        let name = exe
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap()
            .to_ascii_lowercase();
        let found = find_processes(&name).unwrap();
        assert!(found.iter().any(|p| p.pid == std::process::id() as i32));
    }

    #[test]
    fn ptrace_scope_reads_or_reports_absent() {
        // On any mainstream distro this is 0-3 or absent; both are fine.
        assert!(matches!(ptrace_scope(), Ok(None) | Ok(Some(0..=3))));
    }

    #[test]
    fn advice_for_scope_1_is_actionable() {
        let advice = ptrace_advice(Some(1));
        assert!(advice.contains("setcap cap_sys_ptrace=ep"));
        assert!(advice.contains("ptrace_scope=0"));
    }

    #[test]
    fn advice_for_scope_0_mentions_sandboxes() {
        assert!(ptrace_advice(Some(0)).contains("Flatpak"));
    }
}

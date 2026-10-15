//! Error type for all pt-core operations.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced while discovering, stopping, reading, or writing the
/// memory of a target process.
#[derive(Debug, Error)]
pub enum CoreError {
    /// An input file could not be read.
    #[error("failed to read {path}: {source}")]
    Io {
        /// Path of the file that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// `/proc/<pid>/maps` could not be parsed.
    #[error("failed to parse maps of process {pid} at line {line}: {message}")]
    MapsParse {
        /// Target process id.
        pid: i32,
        /// Line number (1-based) of the failure.
        line: usize,
        /// Human-readable description of the failure.
        message: String,
    },

    /// No process matches the requested executable name.
    #[error("no process found matching {name:?}")]
    ProcessNotFound {
        /// The executable name that was searched for.
        name: String,
    },

    /// The target process died or was reaped before or during the
    /// operation.
    #[error("process {pid} no longer exists")]
    ProcessGone {
        /// Target process id.
        pid: i32,
    },

    /// The target was already stopped by someone else; proton-trainer
    /// refuses to take over responsibility for resuming it.
    #[error("process {pid} is already stopped; refusing to manage its stop state")]
    AlreadyStopped {
        /// Target process id.
        pid: i32,
    },

    /// The target did not reach the stopped state in time.
    #[error("process {pid} did not stop within {timeout_ms} ms")]
    StopTimeout {
        /// Target process id.
        pid: i32,
        /// How long the stop was awaited.
        timeout_ms: u64,
    },

    /// The address does not lie inside a mapped writable region.
    #[error("address {addr:#x} is not inside a mapped writable region of process {pid}")]
    RegionNotWritable {
        /// Target process id.
        pid: i32,
        /// The offending address.
        addr: u64,
    },

    /// The address range does not fit inside a single mapped writable
    /// region.
    #[error("address range {start:#x}-{end:#x} does not fit inside a single mapped writable region of process {pid}")]
    RegionSpans {
        /// Target process id.
        pid: i32,
        /// First byte of the range.
        start: u64,
        /// One past the last byte of the range.
        end: u64,
    },

    /// A write was requested with an empty payload.
    #[error("refusing to write zero bytes")]
    EmptyWrite,

    /// A byte pattern is structurally invalid (empty, or its mask does
    /// not match its byte count).
    #[error("invalid pattern: {0}")]
    InvalidPattern(String),

    /// The bytes read back after a write did not match what was
    /// written; the target's memory at the address is not what the
    /// trainer expects.
    #[error("write verification failed at {addr:#x} in process {pid}: expected {expected:02x?}, read back {actual:02x?}")]
    VerifyFailed {
        /// Target process id.
        pid: i32,
        /// Address of the failed write.
        addr: u64,
        /// Bytes that were written.
        expected: Vec<u8>,
        /// Bytes that were read back.
        actual: Vec<u8>,
    },

    /// The kernel denied memory access to the target (`EPERM`); the
    /// message contains an actionable fix for the current situation.
    #[error("memory access to process {pid} was denied: {advice}")]
    PtraceDenied {
        /// Target process id.
        pid: i32,
        /// Plain-English explanation and fix for the denial.
        advice: String,
    },
}

//! Error type for all discovery operations.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced while discovering Steam installations and games.
#[derive(Debug, Error)]
pub enum DiscoverError {
    /// An input file could not be read.
    #[error("failed to read {path}: {source}")]
    Io {
        /// Path of the file that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A VDF document is malformed.
    #[error("VDF parse error at {line}:{column}: {message}")]
    Vdf {
        /// Line number (1-based) of the failure.
        line: usize,
        /// Column number (1-based) of the failure.
        column: usize,
        /// Human-readable description of the failure.
        message: String,
    },

    /// An `appmanifest_*.acf` document is missing or has invalid fields.
    #[error("invalid app manifest: {0}")]
    Manifest(String),

    /// A `libraryfolders.vdf` document is missing or has invalid fields.
    #[error("invalid libraryfolders.vdf: {0}")]
    LibraryFolders(String),

    /// The user home directory could not be determined.
    #[error("could not determine the user home directory")]
    NoHome,

    /// The scan cache could not be serialized.
    #[error("failed to serialize the scan cache: {0}")]
    CacheSerialize(#[from] toml::ser::Error),

    /// The scan cache could not be deserialized.
    #[error("failed to deserialize the scan cache: {0}")]
    CacheDeserialize(#[from] toml::de::Error),
}

//! Error type for trainer-definition parsing and validation.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced while loading trainer definitions or the denylist.
#[derive(Debug, Error)]
pub enum TrainerError {
    /// A file could not be read.
    #[error("failed to read {path}: {source}")]
    Io {
        /// Path of the file that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A TOML file could not be parsed.
    #[error("failed to parse {path}: {source}")]
    Deserialize {
        /// Path of the file that failed to parse.
        path: PathBuf,
        /// Underlying TOML error (includes line and column).
        #[source]
        source: toml::de::Error,
    },

    /// A trainer definition is structurally invalid.
    #[error("invalid trainer {path}: {message}")]
    Schema {
        /// Path of the trainer file.
        path: PathBuf,
        /// Human-readable description of the failure.
        message: String,
    },

    /// Two trainer files declare the same trainer id.
    #[error("duplicate trainer id {id:?}: defined in {first} and {second}")]
    DuplicateTrainerId {
        /// The colliding trainer id.
        id: String,
        /// Path of the first definition.
        first: PathBuf,
        /// Path of the second definition.
        second: PathBuf,
    },

    /// The denylist could not be parsed or is invalid.
    #[error("invalid denylist: {0}")]
    Denylist(String),
}

impl TrainerError {
    /// Builds a [`Schema`](Self::Schema) error with the given file and
    /// message.
    pub fn schema(path: &std::path::Path, message: String) -> Self {
        Self::Schema {
            path: path.to_path_buf(),
            message,
        }
    }
}

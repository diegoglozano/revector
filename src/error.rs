//! Error types for revector.
//!
//! The library surfaces a single [`Error`] enum so callers (the CLI, tests, or
//! downstream embedders) can match on failure categories without depending on
//! the underlying qdrant-client error type.

use std::path::PathBuf;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// All the ways a revector operation can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failure talking to Qdrant (network, gRPC status, etc.).
    ///
    /// Boxed because `QdrantError` is large and would otherwise bloat every
    /// `Result` in the crate.
    #[error("qdrant error: {0}")]
    Qdrant(Box<qdrant_client::QdrantError>),

    /// A migration file could not be read from disk.
    #[error("failed to read migration file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    /// A migration file could not be parsed as YAML.
    #[error("failed to parse migration file {path}: {source}")]
    ParseFile {
        path: PathBuf,
        source: serde_yaml::Error,
    },

    /// The configuration file could not be loaded or merged.
    #[error("configuration error: {0}")]
    Config(String),

    /// The revision chain is malformed (cycles, duplicates, missing parents,
    /// multiple heads, …).
    #[error("invalid migration chain: {0}")]
    Chain(String),

    /// A requested revision id does not exist among the discovered migrations.
    #[error("unknown revision: {0}")]
    UnknownRevision(String),

    /// The recorded checksum for an applied revision no longer matches the file
    /// on disk — the migration was edited after being applied.
    #[error("checksum mismatch for revision {revision}: migration file changed after it was applied (recorded {recorded}, found {found})")]
    ChecksumMismatch {
        revision: String,
        recorded: String,
        found: String,
    },

    /// A downgrade was requested across an irreversible step.
    #[error("revision {revision} is irreversible: {reason}")]
    Irreversible { revision: String, reason: String },

    /// An exec-hook command exited non-zero.
    #[error("exec hook failed (exit {code}): {command}")]
    ExecHook { command: String, code: i32 },

    /// A migration operation references something that is missing or invalid.
    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    /// Catch-all for I/O outside of file parsing.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Wraps an [`anyhow::Error`] for contexts that don't warrant a variant.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<qdrant_client::QdrantError> for Error {
    fn from(e: qdrant_client::QdrantError) -> Self {
        Error::Qdrant(Box::new(e))
    }
}

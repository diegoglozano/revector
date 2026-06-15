//! Migration files: parsing, discovery, and checksums.
//!
//! A migration is a single YAML file carrying a `revision` id, a
//! `down_revision` link to its parent (forming an Alembic-style chain), and
//! `up` / optional `down` operation lists.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::ops::{Operation, Reversibility};

/// The serde-facing shape of a migration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationFile {
    /// Unique revision identifier (e.g. `0001_create_products`).
    pub revision: String,
    /// Parent revision; `None` marks the base of the chain.
    #[serde(default)]
    pub down_revision: Option<String>,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Operations applied on `up`.
    pub up: Vec<Operation>,
    /// Operations applied on `down`. When omitted, revector tries to derive an
    /// inverse automatically (see [`Migration::downgrade_ops`]).
    #[serde(default)]
    pub down: Option<Vec<Operation>>,
}

/// A parsed migration plus the metadata revector tracks about it.
#[derive(Debug, Clone)]
pub struct Migration {
    /// The parsed file contents.
    pub file: MigrationFile,
    /// Path the migration was loaded from.
    pub path: PathBuf,
    /// SHA-256 of the raw file bytes, recorded when applied to detect drift.
    pub checksum: String,
}

impl Migration {
    /// Revision id.
    pub fn revision(&self) -> &str {
        &self.file.revision
    }

    /// Parent revision id, if any.
    pub fn down_revision(&self) -> Option<&str> {
        self.file.down_revision.as_deref()
    }

    /// Parse a single migration file from disk, computing its checksum.
    pub fn from_path(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|source| Error::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        let file: MigrationFile =
            serde_yaml::from_slice(&bytes).map_err(|source| Error::ParseFile {
                path: path.to_path_buf(),
                source,
            })?;
        let checksum = checksum_bytes(&bytes);
        Ok(Migration {
            file,
            path: path.to_path_buf(),
            checksum,
        })
    }

    /// Resolve the operations to run when downgrading this revision.
    ///
    /// Uses the explicit `down` block when present. Otherwise auto-inverts the
    /// `up` ops in reverse order, failing if any step is irreversible.
    pub fn downgrade_ops(&self) -> Result<Vec<Operation>> {
        if let Some(down) = &self.file.down {
            return Ok(down.clone());
        }
        let mut inverted = Vec::with_capacity(self.file.up.len());
        for op in self.file.up.iter().rev() {
            match op.auto_inverse() {
                Reversibility::Auto(inverse) => inverted.push(*inverse),
                Reversibility::Irreversible(reason) => {
                    return Err(Error::Irreversible {
                        revision: self.file.revision.clone(),
                        reason,
                    })
                }
            }
        }
        Ok(inverted)
    }

    /// Whether this revision can be downgraded (explicit `down` or fully
    /// auto-invertible `up`).
    pub fn is_reversible(&self) -> bool {
        self.downgrade_ops().is_ok()
    }
}

/// Compute the SHA-256 checksum of arbitrary bytes, hex-encoded.
pub fn checksum_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Discover and parse every `*.yaml` / `*.yml` migration in a directory.
///
/// Files are returned sorted by path so discovery is deterministic; chain
/// ordering is resolved separately by [`crate::chain`].
pub fn discover(dir: &Path) -> Result<Vec<Migration>> {
    if !dir.exists() {
        return Err(Error::Config(format!(
            "migrations directory not found: {}",
            dir.display()
        )));
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .max_depth(1)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("yaml") | Some("yml") => paths.push(path.to_path_buf()),
            _ => {}
        }
    }
    paths.sort();

    let mut migrations = Vec::with_capacity(paths.len());
    for path in paths {
        migrations.push(Migration::from_path(&path)?);
    }
    Ok(migrations)
}

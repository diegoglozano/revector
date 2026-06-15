//! The re-embedding escape hatch: shell out to a user-provided command.
//!
//! The one operation a schema-migration binary genuinely can't own is
//! re-embedding points with the user's model. Rather than build embedding in,
//! revector lets a migration step run an arbitrary command (e.g. a Python
//! script that reads, re-embeds, and upserts). The command runs via `sh -c`,
//! inherits stdio, and a non-zero exit aborts the migration.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;
use tracing::info;

use crate::error::{Error, Result};
use crate::ops::ExecOp;

/// Run an exec-hook command, failing the migration on a non-zero exit.
pub async fn run(op: &ExecOp, project_root: &Path) -> Result<()> {
    let label = op.name.clone().unwrap_or_else(|| op.command.clone());
    let workdir = match &op.workdir {
        Some(w) => project_root.join(w),
        None => project_root.to_path_buf(),
    };

    info!(target: "revector", "running exec hook `{label}` in {}", workdir.display());

    let status = Command::new("sh")
        .arg("-c")
        .arg(&op.command)
        .current_dir(&workdir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        return Err(Error::ExecHook {
            command: op.command.clone(),
            code: status.code().unwrap_or(-1),
        });
    }
    Ok(())
}

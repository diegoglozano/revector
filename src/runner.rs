//! Orchestration: apply (`up`), roll back (`down`), jump (`to`), and report
//! (`status`) over a resolved revision [`Chain`].
//!
//! The runner is the safety layer. Before mutating anything it verifies that
//! already-applied migrations still match their recorded checksums, so an
//! edited-after-apply migration is caught rather than silently diverging.

use std::path::PathBuf;

use qdrant_client::Qdrant;
use tracing::{info, warn};

use crate::chain::Chain;
use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::tracking::Tracker;

/// Per-revision status line.
#[derive(Debug, Clone)]
pub struct RevisionStatus {
    pub revision: String,
    pub description: Option<String>,
    pub applied: bool,
    pub applied_at: Option<String>,
    /// `None` if not applied; `Some(true/false)` for checksum match.
    pub checksum_ok: Option<bool>,
    pub reversible: bool,
}

/// Full status report for `revector status`.
#[derive(Debug, Clone)]
pub struct StatusReport {
    pub revisions: Vec<RevisionStatus>,
    pub head: Option<String>,
    pub current: Option<String>,
    /// Applied revisions recorded in Qdrant but absent from the migrations dir.
    pub orphaned: Vec<String>,
}

/// Direction/outcome of a plan, for callers that want to print a summary.
#[derive(Debug, Clone)]
pub struct Applied {
    pub revisions: Vec<String>,
    pub dry_run: bool,
}

/// Outcome of a `stamp`: which revisions were newly marked applied and which
/// were removed from the applied set, without running any operations.
#[derive(Debug, Clone)]
pub struct Stamped {
    pub marked: Vec<String>,
    pub removed: Vec<String>,
    pub dry_run: bool,
}

pub struct Runner<'a> {
    client: &'a Qdrant,
    chain: &'a Chain,
    tracker: Tracker<'a>,
    project_root: PathBuf,
    dry_run: bool,
    force: bool,
    holder: String,
}

/// Identify this run for the advisory lock: `user@host pid:N`.
fn default_holder() -> String {
    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string());
    format!("{user}@{host} pid:{}", std::process::id())
}

impl<'a> Runner<'a> {
    pub fn new(
        client: &'a Qdrant,
        chain: &'a Chain,
        tracking_collection: &str,
        project_root: impl Into<PathBuf>,
    ) -> Self {
        Runner {
            client,
            chain,
            tracker: Tracker::new(client, tracking_collection),
            project_root: project_root.into(),
            dry_run: false,
            force: false,
            holder: default_holder(),
        }
    }

    pub fn dry_run(mut self, on: bool) -> Self {
        self.dry_run = on;
        self
    }

    /// Override a held/stale advisory lock instead of failing.
    pub fn force(mut self, on: bool) -> Self {
        self.force = on;
        self
    }

    /// Acquire the advisory lock, run `op`, then release the lock regardless of
    /// outcome. Dry runs don't lock (they mutate nothing).
    async fn with_lock<T, F, Fut>(&self, op: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        if self.dry_run {
            return op().await;
        }
        self.tracker.ensure().await?;
        self.tracker.acquire_lock(&self.holder, self.force).await?;
        let result = op().await;
        if let Err(e) = self.tracker.release_lock().await {
            // Don't mask the real error if the operation itself failed.
            if result.is_ok() {
                return Err(e);
            }
            warn!("failed to release migration lock: {e}");
        }
        result
    }

    fn executor(&self) -> Executor<'a> {
        Executor::new(self.client, self.project_root.clone()).dry_run(self.dry_run)
    }

    /// Current position in the chain: index of the highest applied revision, or
    /// `None` if nothing is applied.
    async fn current_position(&self) -> Result<Option<usize>> {
        let applied = self.tracker.applied().await?;
        let mut current = None;
        for m in self.chain.migrations() {
            if applied.contains_key(m.revision()) {
                let pos = self.chain.position(m.revision()).unwrap();
                current = Some(current.map_or(pos, |c: usize| c.max(pos)));
            }
        }
        Ok(current)
    }

    /// Build a full status report.
    pub async fn status(&self) -> Result<StatusReport> {
        let applied = self.tracker.applied().await?;
        let mut revisions = Vec::with_capacity(self.chain.len());
        let mut current = None;

        for m in self.chain.migrations() {
            let record = applied.get(m.revision());
            let checksum_ok = record.map(|r| r.checksum == m.checksum);
            if record.is_some() {
                current = Some(m.revision().to_string());
            }
            revisions.push(RevisionStatus {
                revision: m.revision().to_string(),
                description: m.file.description.clone(),
                applied: record.is_some(),
                applied_at: record.map(|r| r.applied_at.clone()),
                checksum_ok,
                reversible: m.is_reversible(),
            });
        }

        // Anything applied but not present on disk is orphaned.
        let known: std::collections::HashSet<&str> = self
            .chain
            .migrations()
            .iter()
            .map(|m| m.revision())
            .collect();
        let mut orphaned: Vec<String> = applied
            .keys()
            .filter(|r| !known.contains(r.as_str()))
            .cloned()
            .collect();
        orphaned.sort();

        Ok(StatusReport {
            revisions,
            head: self.chain.head().map(str::to_string),
            current,
            orphaned,
        })
    }

    /// Verify recorded checksums for every applied revision present on disk.
    async fn verify_checksums(&self) -> Result<()> {
        let applied = self.tracker.applied().await?;
        for m in self.chain.migrations() {
            if let Some(record) = applied.get(m.revision()) {
                if record.checksum != m.checksum {
                    return Err(Error::ChecksumMismatch {
                        revision: m.revision().to_string(),
                        recorded: record.checksum.clone(),
                        found: m.checksum.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Apply pending migrations up to `target` (default: chain head), holding
    /// the advisory lock for the duration.
    pub async fn up(&self, target: Option<&str>) -> Result<Applied> {
        self.with_lock(|| self.up_locked(target)).await
    }

    async fn up_locked(&self, target: Option<&str>) -> Result<Applied> {
        self.tracker.ensure().await?;
        self.verify_checksums().await?;

        if self.chain.is_empty() {
            info!("no migrations found");
            return Ok(Applied {
                revisions: vec![],
                dry_run: self.dry_run,
            });
        }

        let target_pos = match target {
            Some(rev) => self
                .chain
                .position(rev)
                .ok_or_else(|| Error::UnknownRevision(rev.to_string()))?,
            None => self.chain.len() - 1,
        };

        let start = match self.current_position().await? {
            Some(c) if c >= target_pos => {
                info!("already at or beyond target; nothing to apply");
                return Ok(Applied {
                    revisions: vec![],
                    dry_run: self.dry_run,
                });
            }
            Some(c) => c + 1,
            None => 0,
        };

        let executor = self.executor();
        let mut done = Vec::new();
        for m in &self.chain.migrations()[start..=target_pos] {
            info!(
                "applying {} — {}",
                m.revision(),
                m.file.description.as_deref().unwrap_or("(no description)")
            );
            for op in &m.file.up {
                executor.execute(op).await?;
            }
            if !self.dry_run {
                self.tracker.record(m).await?;
            }
            done.push(m.revision().to_string());
        }
        Ok(Applied {
            revisions: done,
            dry_run: self.dry_run,
        })
    }

    /// Roll back applied migrations down to `target` (exclusive). When `target`
    /// is `None`, roll back `steps` revisions from the current head. Holds the
    /// advisory lock for the duration.
    pub async fn down(&self, target: Option<&str>, steps: usize) -> Result<Applied> {
        self.with_lock(|| self.down_locked(target, steps)).await
    }

    async fn down_locked(&self, target: Option<&str>, steps: usize) -> Result<Applied> {
        self.tracker.ensure().await?;
        self.verify_checksums().await?;

        let current = match self.current_position().await? {
            Some(c) => c,
            None => {
                info!("nothing applied; nothing to roll back");
                return Ok(Applied {
                    revisions: vec![],
                    dry_run: self.dry_run,
                });
            }
        };

        // Determine the exclusive lower bound (positions strictly above it are
        // rolled back, highest first).
        let floor: isize = match target {
            Some(rev) => self
                .chain
                .position(rev)
                .ok_or_else(|| Error::UnknownRevision(rev.to_string()))?
                as isize,
            None => current as isize - steps as isize,
        };

        let executor = self.executor();
        let mut done = Vec::new();
        let mut pos = current as isize;
        while pos > floor {
            let m = &self.chain.migrations()[pos as usize];
            info!("rolling back {}", m.revision());
            // Resolve downgrade ops first so an irreversible step fails before
            // any mutation happens.
            let ops = m.downgrade_ops()?;
            for op in &ops {
                executor.execute(op).await?;
            }
            if !self.dry_run {
                self.tracker.remove(m.revision()).await?;
            }
            done.push(m.revision().to_string());
            pos -= 1;
        }

        if done.is_empty() {
            info!("nothing to roll back for the requested range");
        }
        Ok(Applied {
            revisions: done,
            dry_run: self.dry_run,
        })
    }

    /// Compute, without executing or locking, which revisions a `down` would
    /// roll back (highest first). Used to preview a confirmation prompt.
    pub async fn plan_down(&self, target: Option<&str>, steps: usize) -> Result<Vec<String>> {
        let current = match self.current_position().await? {
            Some(c) => c,
            None => return Ok(vec![]),
        };
        let floor: isize = match target {
            Some(rev) => self
                .chain
                .position(rev)
                .ok_or_else(|| Error::UnknownRevision(rev.to_string()))?
                as isize,
            None => current as isize - steps as isize,
        };
        let mut planned = Vec::new();
        let mut pos = current as isize;
        while pos > floor {
            planned.push(self.chain.migrations()[pos as usize].revision().to_string());
            pos -= 1;
        }
        Ok(planned)
    }

    /// Migrate to an exact revision, choosing up or down automatically.
    pub async fn to(&self, target: &str) -> Result<Applied> {
        let target_pos = self
            .chain
            .position(target)
            .ok_or_else(|| Error::UnknownRevision(target.to_string()))?;
        match self.current_position().await? {
            Some(c) if c > target_pos => self.down(Some(target), 0).await,
            Some(c) if c == target_pos => {
                info!("already at {target}");
                Ok(Applied {
                    revisions: vec![],
                    dry_run: self.dry_run,
                })
            }
            _ => self.up(Some(target)).await,
        }
    }

    /// Mark the database as being at `target` **without running** any
    /// operations (Alembic's `stamp`). Records every revision up to and
    /// including the target as applied and removes any recorded above it.
    ///
    /// `target` accepts a revision id, or the special tokens `head` (the chain
    /// tip) and `base` (mark nothing applied). This is how an existing,
    /// hand-built collection is adopted without re-creating it.
    pub async fn stamp(&self, target: &str) -> Result<Stamped> {
        self.with_lock(|| self.stamp_locked(target)).await
    }

    async fn stamp_locked(&self, target: &str) -> Result<Stamped> {
        self.tracker.ensure().await?;

        // Resolve target to a chain position; `base` is -1 (nothing applied).
        let target_pos: isize = match target {
            "base" => -1,
            "head" => self.chain.len() as isize - 1,
            rev => self
                .chain
                .position(rev)
                .ok_or_else(|| Error::UnknownRevision(rev.to_string()))?
                as isize,
        };

        let applied = self.tracker.applied().await?;
        let mut marked = Vec::new();
        let mut removed = Vec::new();

        for (i, m) in self.chain.migrations().iter().enumerate() {
            let is_applied = applied.contains_key(m.revision());
            if (i as isize) <= target_pos {
                if !is_applied {
                    info!("stamping {} as applied", m.revision());
                    if !self.dry_run {
                        self.tracker.record(m).await?;
                    }
                    marked.push(m.revision().to_string());
                }
            } else if is_applied {
                info!("clearing applied mark for {}", m.revision());
                if !self.dry_run {
                    self.tracker.remove(m.revision()).await?;
                }
                removed.push(m.revision().to_string());
            }
        }

        Ok(Stamped {
            marked,
            removed,
            dry_run: self.dry_run,
        })
    }

    /// Warn about orphaned applied revisions (helpful before any operation).
    pub async fn warn_orphans(&self) -> Result<()> {
        let report = self.status().await?;
        for o in &report.orphaned {
            warn!("revision `{o}` is applied in Qdrant but missing from the migrations directory");
        }
        Ok(())
    }
}

//! Resolve discovered migrations into an ordered revision chain.
//!
//! Migrations form an Alembic-style linked list via `revision` / `down_revision`.
//! v1 enforces a **linear** chain (single base, single head, no branches) — the
//! common case, and unambiguous to apply/rollback. The resolver validates the
//! structure up front so the runner can trust the ordering.

use std::collections::{HashMap, HashSet};

use crate::error::{Error, Result};
use crate::migration::Migration;

/// An ordered, validated linear chain of migrations (base → head).
#[derive(Debug)]
pub struct Chain {
    /// Migrations in apply order.
    ordered: Vec<Migration>,
    /// revision id → index in `ordered`.
    index: HashMap<String, usize>,
}

impl Chain {
    /// Validate and order a set of discovered migrations.
    pub fn resolve(migrations: Vec<Migration>) -> Result<Self> {
        if migrations.is_empty() {
            return Ok(Chain {
                ordered: Vec::new(),
                index: HashMap::new(),
            });
        }

        // Detect duplicate revision ids.
        let mut by_rev: HashMap<String, Migration> = HashMap::new();
        for m in migrations {
            let rev = m.revision().to_string();
            if by_rev.contains_key(&rev) {
                return Err(Error::Chain(format!("duplicate revision id `{rev}`")));
            }
            by_rev.insert(rev, m);
        }

        // Every down_revision must reference an existing revision.
        for m in by_rev.values() {
            if let Some(parent) = m.down_revision() {
                if !by_rev.contains_key(parent) {
                    return Err(Error::Chain(format!(
                        "revision `{}` references unknown down_revision `{parent}`",
                        m.revision()
                    )));
                }
            }
        }

        // Exactly one base (down_revision == None).
        let bases: Vec<&str> = by_rev
            .values()
            .filter(|m| m.down_revision().is_none())
            .map(|m| m.revision())
            .collect();
        match bases.len() {
            1 => {}
            0 => {
                return Err(Error::Chain(
                    "no base migration found (every migration has a down_revision — cycle?)"
                        .to_string(),
                ))
            }
            _ => {
                return Err(Error::Chain(format!(
                    "multiple base migrations found: {bases:?}; v1 supports a single linear chain"
                )))
            }
        }

        // Build parent → child map and detect branches (a parent with >1 child).
        let mut children: HashMap<Option<&str>, Vec<&str>> = HashMap::new();
        for m in by_rev.values() {
            children
                .entry(m.down_revision())
                .or_default()
                .push(m.revision());
        }
        for (parent, kids) in &children {
            if kids.len() > 1 {
                return Err(Error::Chain(format!(
                    "revision `{}` has multiple children {kids:?}; v1 supports only linear chains",
                    parent.unwrap_or("<base>")
                )));
            }
        }

        // Walk from base to head, guarding against cycles.
        let mut order: Vec<String> = Vec::with_capacity(by_rev.len());
        let mut seen: HashSet<String> = HashSet::new();
        let mut cursor: Option<&str> = None; // base's down_revision is None
        loop {
            let next = children.get(&cursor).and_then(|v| v.first()).copied();
            match next {
                Some(rev) => {
                    if !seen.insert(rev.to_string()) {
                        return Err(Error::Chain(format!("cycle detected at `{rev}`")));
                    }
                    order.push(rev.to_string());
                    cursor = Some(rev);
                }
                None => break,
            }
        }

        if order.len() != by_rev.len() {
            return Err(Error::Chain(format!(
                "migration graph is disconnected: {} of {} revisions are unreachable from the base",
                by_rev.len() - order.len(),
                by_rev.len()
            )));
        }

        let ordered: Vec<Migration> = order
            .into_iter()
            .map(|rev| by_rev.remove(&rev).expect("revision present"))
            .collect();
        let index = ordered
            .iter()
            .enumerate()
            .map(|(i, m)| (m.revision().to_string(), i))
            .collect();

        Ok(Chain { ordered, index })
    }

    /// Migrations in apply order.
    pub fn migrations(&self) -> &[Migration] {
        &self.ordered
    }

    /// Number of migrations in the chain.
    pub fn len(&self) -> usize {
        self.ordered.len()
    }

    /// Whether the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.ordered.is_empty()
    }

    /// The head (latest) revision id, if any.
    pub fn head(&self) -> Option<&str> {
        self.ordered.last().map(|m| m.revision())
    }

    /// Look up a migration by revision id.
    pub fn get(&self, revision: &str) -> Option<&Migration> {
        self.index.get(revision).map(|&i| &self.ordered[i])
    }

    /// Position (0-based, apply order) of a revision.
    pub fn position(&self, revision: &str) -> Option<usize> {
        self.index.get(revision).copied()
    }
}

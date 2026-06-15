//! # revector
//!
//! Declarative, versioned schema & config migrations for [Qdrant] — think
//! "Alembic for vector collections".
//!
//! revector manages the *automatable, in-place* surface of a Qdrant
//! deployment: collections, payload indexes, named-vector add/drop, aliases,
//! and all tunable configs. Migrations are plain YAML files discovered from a
//! `migrations/` directory, chained Alembic-style via `revision` /
//! `down_revision`, and applied/rolled back through a CLI. Applied state is
//! tracked inside Qdrant itself, so no external database is required.
//!
//! The one thing a binary can't own — re-embedding points with the user's
//! model — is handled by an [exec-hook](crate::ops::ExecOp) escape hatch.
//!
//! [Qdrant]: https://qdrant.tech
//!
//! ## Module map
//!
//! - [`config`] — layered configuration loading.
//! - [`spec`] — declarative schema/config vocabulary authored in YAML.
//! - [`ops`] — migration operations and their reversibility.
//! - [`migration`] / [`chain`] — file parsing, discovery, and DAG resolution.
//! - [`convert`] — spec → `qdrant-client` type translation.
//! - [`executor`] — idempotent execution of operations.
//! - [`tracking`] — applied-revision bookkeeping inside Qdrant.
//! - [`runner`] — up/down/to/status orchestration with checksum safety.
//! - [`diff`] — declaration-driven drift detection.

pub mod chain;
pub mod cli;
pub mod client;
pub mod config;
pub mod convert;
pub mod diff;
pub mod error;
pub mod exec_hook;
pub mod executor;
pub mod migration;
pub mod ops;
pub mod runner;
pub mod scaffold;
pub mod spec;
pub mod tracking;

pub use config::Config;
pub use error::{Error, Result};

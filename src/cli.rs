//! Command-line interface definition (clap derive).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Declarative, versioned schema & config migrations for Qdrant.
#[derive(Debug, Parser)]
#[command(name = "revector", version, about, long_about = None)]
pub struct Cli {
    /// Path to a `revector.toml` config file (defaults to ./revector.toml).
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Qdrant gRPC URL (overrides config / env).
    #[arg(long, global = true, env = "REVECTOR_URL")]
    pub url: Option<String>,

    /// Qdrant API key (overrides config / env).
    #[arg(long, global = true, env = "REVECTOR_API_KEY")]
    pub api_key: Option<String>,

    /// Migrations directory (overrides config / env).
    #[arg(long, global = true, value_name = "DIR")]
    pub migrations_dir: Option<PathBuf>,

    /// Increase log verbosity (-v debug, -vv trace).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create the migrations directory and a starter config file.
    Init,

    /// Scaffold a new migration chained onto the current head.
    New {
        /// Human-readable migration name (used for the slug & description).
        name: String,
    },

    /// Show applied vs pending migrations and any drift in tracking.
    Status,

    /// Apply pending migrations (optionally up to a specific revision).
    Up {
        /// Stop after applying this revision.
        #[arg(long, value_name = "REV")]
        to: Option<String>,
        /// Print the plan without mutating Qdrant.
        #[arg(long)]
        dry_run: bool,
    },

    /// Roll back applied migrations.
    Down {
        /// Roll back down to (but not including) this revision.
        #[arg(long, value_name = "REV", conflicts_with = "steps")]
        to: Option<String>,
        /// Number of revisions to roll back (default 1).
        #[arg(long, default_value_t = 1)]
        steps: usize,
        /// Print the plan without mutating Qdrant.
        #[arg(long)]
        dry_run: bool,
    },

    /// Migrate to an exact revision, choosing up or down automatically.
    To {
        /// Target revision id.
        revision: String,
        /// Print the plan without mutating Qdrant.
        #[arg(long)]
        dry_run: bool,
    },

    /// Validate migrations offline: parse every file and resolve the revision
    /// chain without connecting to Qdrant. Useful as a CI / pre-commit check.
    Validate,

    /// Mark the database as being at a revision **without running** any
    /// operations — for adopting an existing collection (Alembic's `stamp`).
    ///
    /// Records every revision up to and including the target as applied, and
    /// removes any recorded revisions above it. Accepts a revision id, or the
    /// special values `head` and `base`.
    Stamp {
        /// Target revision id (or `head` / `base`).
        revision: String,
        /// Print what would change without writing to Qdrant.
        #[arg(long)]
        dry_run: bool,
    },

    /// Compare a declared collection spec against the live collection.
    Diff {
        /// Collection name to inspect.
        collection: String,
        /// YAML file containing the declared `CollectionSpec`.
        #[arg(long, value_name = "FILE")]
        spec: PathBuf,
    },
}

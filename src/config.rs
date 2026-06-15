//! Layered configuration loading.
//!
//! Settings are resolved with the following precedence (highest wins):
//!
//! 1. Explicit CLI flags (applied by the caller after [`Config::load`]).
//! 2. Environment variables prefixed with `REVECTOR_` (e.g. `REVECTOR_URL`).
//! 3. A `revector.toml` file in the project root (or one passed with
//!    `--config`).
//! 4. Built-in defaults.
//!
//! Layering is handled by [`figment`].

use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Default directory (relative to the project root) holding migration files.
pub const DEFAULT_MIGRATIONS_DIR: &str = "migrations";
/// Default Qdrant gRPC endpoint.
pub const DEFAULT_URL: &str = "http://localhost:6334";
/// Name of the collection revector uses to track applied revisions.
pub const DEFAULT_TRACKING_COLLECTION: &str = "_revector_migrations";

/// Fully resolved configuration for a revector invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Qdrant gRPC URL, e.g. `http://localhost:6334`.
    #[serde(default = "default_url")]
    pub url: String,

    /// Optional API key for Qdrant Cloud / secured deployments.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Directory containing migration `*.yaml` files.
    #[serde(default = "default_migrations_dir")]
    pub migrations_dir: PathBuf,

    /// Collection used to persist applied-revision bookkeeping.
    #[serde(default = "default_tracking_collection")]
    pub tracking_collection: String,

    /// Request timeout in seconds for Qdrant calls.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_url() -> String {
    DEFAULT_URL.to_string()
}
fn default_migrations_dir() -> PathBuf {
    PathBuf::from(DEFAULT_MIGRATIONS_DIR)
}
fn default_tracking_collection() -> String {
    DEFAULT_TRACKING_COLLECTION.to_string()
}
fn default_timeout_secs() -> u64 {
    30
}

impl Default for Config {
    fn default() -> Self {
        Config {
            url: default_url(),
            api_key: None,
            migrations_dir: default_migrations_dir(),
            tracking_collection: default_tracking_collection(),
            timeout_secs: default_timeout_secs(),
        }
    }
}

impl Config {
    /// Load configuration by layering defaults, an optional TOML file, and
    /// `REVECTOR_*` environment variables.
    ///
    /// If `config_path` is `None`, `revector.toml` in the current directory is
    /// used when present. A missing default file is not an error; a missing
    /// explicitly-requested file is.
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let mut figment = Figment::from(Serialized::defaults(Config::default()));

        match config_path {
            Some(path) => {
                if !path.exists() {
                    return Err(Error::Config(format!(
                        "config file not found: {}",
                        path.display()
                    )));
                }
                figment = figment.merge(Toml::file(path));
            }
            None => {
                // Only merge the conventional file when it exists.
                let default = Path::new("revector.toml");
                if default.exists() {
                    figment = figment.merge(Toml::file(default));
                }
            }
        }

        figment = figment.merge(Env::prefixed("REVECTOR_"));

        figment.extract().map_err(|e| Error::Config(e.to_string()))
    }
}

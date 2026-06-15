//! Thin wrapper around `qdrant_client::Qdrant` that applies revector's config.

use std::time::Duration;

use qdrant_client::Qdrant;

use crate::config::Config;
use crate::error::Result;

/// Connect to Qdrant using the resolved [`Config`].
pub fn connect(config: &Config) -> Result<Qdrant> {
    let mut builder =
        Qdrant::from_url(&config.url).timeout(Duration::from_secs(config.timeout_secs));
    if let Some(key) = &config.api_key {
        builder = builder.api_key(key.clone());
    }
    // Skip the client/server version compatibility check: revector targets the
    // documented v1.18 API surface and should run against newer servers too.
    builder = builder.skip_compatibility_check();
    Ok(builder.build()?)
}

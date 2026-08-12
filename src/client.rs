//! Thin wrapper around `qdrant_client::Qdrant` that applies revector's config.

use std::time::Duration;

use qdrant_client::Qdrant;

use crate::config::Config;
use crate::error::Result;
use crate::version::ServerVersion;

/// Connect to Qdrant using the resolved [`Config`].
pub fn connect(config: &Config) -> Result<Qdrant> {
    let mut builder =
        Qdrant::from_url(&config.url).timeout(Duration::from_secs(config.timeout_secs));
    if let Some(key) = &config.api_key {
        builder = builder.api_key(key.clone());
    }
    // Skip the client/server version compatibility check: revector targets the
    // documented v1.19 API surface and should run against newer servers too.
    // Older servers work as well — they ignore the 1.19-only fields a migration
    // may declare (see the compatibility notes in the README).
    builder = builder.skip_compatibility_check();
    Ok(builder.build()?)
}

/// Ask the server what version it runs.
///
/// Returns `None` when the reported string doesn't parse — callers treat that
/// as "can't tell" and carry on rather than blocking a migration over an
/// unrecognised build string.
pub async fn server_version(client: &Qdrant) -> Result<Option<ServerVersion>> {
    let reply = client.health_check().await?;
    Ok(ServerVersion::parse(&reply.version))
}

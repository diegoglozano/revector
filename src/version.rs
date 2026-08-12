//! Server version parsing, and the minimum server version a migration needs.
//!
//! Qdrant's gRPC API is proto3: a field a server predates is **dropped during
//! decode, not rejected**. Applying a migration that declares, say, `memory:`
//! to a 1.18 server therefore succeeds — the setting silently evaporates, and
//! the runner goes on to record the revision as applied. Upgrading the server
//! later doesn't retroactively apply it, `up` reports nothing to do, and the
//! checksum guard blocks editing the file to try again.
//!
//! So revector states the requirement up front instead: each operation reports
//! the oldest server that understands the fields it will actually send, and
//! [`crate::runner`] refuses to run when the live server is older. Only fields
//! that reach the wire count — [`crate::executor`] drops some specs by design
//! (per-vector tuning at `create_vector` time, index params on a delete), and
//! those must not raise the requirement.

use std::fmt;

/// Qdrant 1.19: unified `memory` placement, the collection `payload` block,
/// and keyword `prefix` index params.
pub const MEMORY_PLACEMENT: ServerVersion = ServerVersion::new(1, 19, 0);

/// Qdrant 1.18.2 is where the server learned the TurboQuant 4-bit datatype,
/// even though the client crate only exposed it in 1.19.
pub const TURBO4_DATATYPE: ServerVersion = ServerVersion::new(1, 18, 2);

/// A Qdrant server version, ordered by `(major, minor, patch)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServerVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ServerVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        ServerVersion {
            major,
            minor,
            patch,
        }
    }

    /// Parse what `health_check` reports: `1.19.0`, `1.19`, or a pre-release /
    /// build-suffixed string like `1.19.0-rc.1`. Returns `None` for anything
    /// else — callers treat an unparseable version as "can't tell", never as
    /// "too old".
    pub fn parse(s: &str) -> Option<Self> {
        let core = s
            .trim()
            .trim_start_matches('v')
            .split(['-', '+'])
            .next()?
            .trim();
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        // A missing patch means `.0`; a non-numeric one makes the whole string
        // suspect, so it fails rather than silently reading as `.0`.
        let patch = match parts.next() {
            Some(p) => p.parse().ok()?,
            None => 0,
        };
        Some(ServerVersion::new(major, minor, patch))
    }
}

impl fmt::Display for ServerVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// "This field needs a server at least this new", with the field named so the
/// error can point at the line to change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRequirement {
    pub version: ServerVersion,
    /// Where the requirement came from, e.g. ``memory` on vectors.image``.
    pub feature: String,
}

impl VersionRequirement {
    pub fn new(version: ServerVersion, feature: impl Into<String>) -> Self {
        VersionRequirement {
            version,
            feature: feature.into(),
        }
    }
}

/// Accumulator threaded through the spec types while they report requirements.
pub(crate) fn require(
    out: &mut Vec<VersionRequirement>,
    version: ServerVersion,
    feature: impl Into<String>,
) {
    out.push(VersionRequirement::new(version, feature));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_shapes_qdrant_reports() {
        assert_eq!(
            ServerVersion::parse("1.19.0"),
            Some(ServerVersion::new(1, 19, 0))
        );
        assert_eq!(
            ServerVersion::parse("v1.18.2"),
            Some(ServerVersion::new(1, 18, 2))
        );
        assert_eq!(
            ServerVersion::parse("1.19"),
            Some(ServerVersion::new(1, 19, 0))
        );
        assert_eq!(
            ServerVersion::parse("1.19.0-rc.1"),
            Some(ServerVersion::new(1, 19, 0))
        );
        assert_eq!(ServerVersion::parse("nightly"), None);
        assert_eq!(ServerVersion::parse("1.19.x"), None);
    }

    #[test]
    fn orders_by_component() {
        assert!(ServerVersion::new(1, 18, 2) < ServerVersion::new(1, 19, 0));
        assert!(ServerVersion::new(1, 18, 10) > ServerVersion::new(1, 18, 2));
        assert!(ServerVersion::new(2, 0, 0) > ServerVersion::new(1, 99, 99));
    }
}

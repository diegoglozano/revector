//! Drift detection: compare a declared [`CollectionSpec`] against the live
//! collection returned by `get_collection`.
//!
//! The classic autogenerate pitfall (same as Alembic) is that Qdrant fills in
//! and normalizes defaults on read, so a naive structural compare reports a
//! diff for every field the user never set. revector sidesteps this by being
//! **declaration-driven**: only fields the user explicitly wrote in the spec
//! are compared. A `None` in the spec means "don't care", never "must be
//! unset". This keeps `diff` quiet unless something the user actually declared
//! has drifted.

use qdrant_client::qdrant::{vectors_config::Config as VConfig, VectorParams};
use qdrant_client::Qdrant;

use crate::error::{Error, Result};
use crate::spec::{CollectionSpec, HnswConfigSpec, VectorSpec};

/// A single detected difference between declared and live state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub path: String,
    pub declared: String,
    pub live: String,
}

/// The result of diffing one collection.
#[derive(Debug, Clone)]
pub struct DiffReport {
    pub collection: String,
    pub exists: bool,
    pub differences: Vec<Difference>,
}

impl DiffReport {
    /// Whether the live collection matches the declared spec.
    pub fn in_sync(&self) -> bool {
        self.exists && self.differences.is_empty()
    }
}

fn distance_name(i: i32) -> String {
    match qdrant_client::qdrant::Distance::try_from(i) {
        Ok(d) => format!("{d:?}"),
        Err(_) => format!("{i}"),
    }
}

fn cmp<T: PartialEq + std::fmt::Debug>(
    diffs: &mut Vec<Difference>,
    path: &str,
    declared: Option<T>,
    live: T,
) {
    if let Some(d) = declared {
        if d != live {
            diffs.push(Difference {
                path: path.to_string(),
                declared: format!("{d:?}"),
                live: format!("{live:?}"),
            });
        }
    }
}

fn diff_hnsw(
    diffs: &mut Vec<Difference>,
    prefix: &str,
    declared: &HnswConfigSpec,
    live: Option<&qdrant_client::qdrant::HnswConfigDiff>,
) {
    let live = match live {
        Some(l) => l,
        None => return,
    };
    cmp(
        diffs,
        &format!("{prefix}.m"),
        declared.m,
        live.m.unwrap_or_default(),
    );
    cmp(
        diffs,
        &format!("{prefix}.ef_construct"),
        declared.ef_construct,
        live.ef_construct.unwrap_or_default(),
    );
    cmp(
        diffs,
        &format!("{prefix}.full_scan_threshold"),
        declared.full_scan_threshold,
        live.full_scan_threshold.unwrap_or_default(),
    );
    cmp(
        diffs,
        &format!("{prefix}.on_disk"),
        declared.on_disk,
        live.on_disk.unwrap_or_default(),
    );
}

fn diff_vector(
    diffs: &mut Vec<Difference>,
    name: &str,
    declared: &VectorSpec,
    live: &VectorParams,
) {
    let display = if name.is_empty() { "<default>" } else { name };
    let prefix = format!("vectors.{display}");
    // size and distance are immutable; a mismatch is a hard structural drift.
    cmp(
        diffs,
        &format!("{prefix}.size"),
        Some(declared.size),
        live.size,
    );

    let declared_distance = qdrant_client::qdrant::Distance::from(declared.distance) as i32;
    if declared_distance != live.distance {
        diffs.push(Difference {
            path: format!("{prefix}.distance"),
            declared: format!("{:?}", declared.distance),
            live: distance_name(live.distance),
        });
    }

    cmp(
        diffs,
        &format!("{prefix}.on_disk"),
        declared.on_disk,
        live.on_disk.unwrap_or_default(),
    );

    if let Some(h) = &declared.hnsw_config {
        diff_hnsw(
            diffs,
            &format!("{prefix}.hnsw_config"),
            h,
            live.hnsw_config.as_ref(),
        );
    }
}

/// Diff a declared collection spec against the live collection.
pub async fn diff_collection(
    client: &Qdrant,
    name: &str,
    spec: &CollectionSpec,
) -> Result<DiffReport> {
    if !client.collection_exists(name).await? {
        return Ok(DiffReport {
            collection: name.to_string(),
            exists: false,
            differences: vec![],
        });
    }

    let info = client.collection_info(name).await?;
    let config = info.result.and_then(|r| r.config).ok_or_else(|| {
        Error::InvalidOperation(format!("collection `{name}` returned no config"))
    })?;
    let params = config.params.ok_or_else(|| {
        Error::InvalidOperation(format!("collection `{name}` returned no params"))
    })?;

    let mut diffs = Vec::new();

    // --- vectors -----------------------------------------------------------
    let mut live_vectors: std::collections::HashMap<String, VectorParams> =
        std::collections::HashMap::new();
    if let Some(vc) = params.vectors_config {
        match vc.config {
            Some(VConfig::Params(p)) => {
                live_vectors.insert(String::new(), p);
            }
            Some(VConfig::ParamsMap(m)) => {
                live_vectors.extend(m.map);
            }
            None => {}
        }
    }

    for (vname, vspec) in &spec.vectors {
        match live_vectors.get(vname) {
            Some(live) => diff_vector(&mut diffs, vname, vspec, live),
            None => diffs.push(Difference {
                path: format!("vectors.{vname}"),
                declared: "present".to_string(),
                live: "missing".to_string(),
            }),
        }
    }
    for vname in live_vectors.keys() {
        if !spec.vectors.contains_key(vname) {
            diffs.push(Difference {
                path: format!("vectors.{vname}"),
                declared: "absent".to_string(),
                live: "present (undeclared)".to_string(),
            });
        }
    }

    // --- collection-level config ------------------------------------------
    if let Some(h) = &spec.hnsw_config {
        diff_hnsw(&mut diffs, "hnsw_config", h, config.hnsw_config.as_ref());
    }
    cmp(
        &mut diffs,
        "on_disk_payload",
        spec.on_disk_payload,
        params.on_disk_payload,
    );
    cmp(
        &mut diffs,
        "replication_factor",
        spec.replication_factor,
        params.replication_factor.unwrap_or(1),
    );
    cmp(
        &mut diffs,
        "shard_number",
        spec.shard_number,
        params.shard_number,
    );

    Ok(DiffReport {
        collection: name.to_string(),
        exists: true,
        differences: diffs,
    })
}

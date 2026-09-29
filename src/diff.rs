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

use qdrant_client::qdrant::{
    vectors_config::Config as VConfig, Memory as QMemory, SparseVectorParams, VectorParams,
};
use qdrant_client::Qdrant;

use crate::error::{Error, Result};
use crate::spec::{CollectionSpec, HnswConfigSpec, Memory, SparseVectorSpec, VectorSpec};

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

/// Render a live memory-placement discriminant. Qdrant leaves the field unset
/// on components whose placement was never declared (including everything
/// created before 1.19), so `unset` is a normal value here, not an error.
fn memory_name(i: Option<i32>) -> String {
    match i.map(QMemory::try_from) {
        None | Some(Ok(QMemory::Unknown)) => "unset".to_string(),
        Some(Ok(m)) => format!("{m:?}").to_lowercase(),
        Some(Err(_)) => format!("{}", i.unwrap_or_default()),
    }
}

/// Compare a declared memory placement against the live one.
fn cmp_memory(
    diffs: &mut Vec<Difference>,
    path: &str,
    declared: Option<Memory>,
    live: Option<i32>,
) {
    let Some(d) = declared else { return };
    if QMemory::from(d) as i32 != live.unwrap_or(QMemory::Unknown as i32) {
        diffs.push(Difference {
            path: path.to_string(),
            declared: d.as_str().to_string(),
            live: memory_name(live),
        });
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

// The `on_disk` reads below hit fields Qdrant deprecated in 1.19 in favour of
// `memory`. revector still compares them, because a migration that declared
// `on_disk` is still declaring that field and deserves an honest answer about
// it. Applies to every `allow(deprecated)` in this file.
#[allow(deprecated)]
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
    cmp_memory(
        diffs,
        &format!("{prefix}.memory"),
        declared.memory,
        live.memory,
    );
}

#[allow(deprecated)]
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
    cmp_memory(
        diffs,
        &format!("{prefix}.memory"),
        declared.memory,
        live.memory,
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

#[allow(deprecated)]
fn diff_sparse_vector(
    diffs: &mut Vec<Difference>,
    name: &str,
    declared: &SparseVectorSpec,
    live: &SparseVectorParams,
) {
    let prefix = format!("sparse_vectors.{name}");
    let index = live.index.as_ref();
    cmp(
        diffs,
        &format!("{prefix}.on_disk"),
        declared.on_disk,
        index.and_then(|i| i.on_disk).unwrap_or_default(),
    );
    cmp(
        diffs,
        &format!("{prefix}.full_scan_threshold"),
        declared.full_scan_threshold,
        index
            .and_then(|i| i.full_scan_threshold)
            .unwrap_or_default(),
    );
    cmp_memory(
        diffs,
        &format!("{prefix}.memory"),
        declared.memory,
        index.and_then(|i| i.memory),
    );
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

    // --- sparse vectors ----------------------------------------------------
    let live_sparse = params
        .sparse_vectors_config
        .map(|sc| sc.map)
        .unwrap_or_default();

    for (sname, sspec) in &spec.sparse_vectors {
        match live_sparse.get(sname) {
            Some(live) => diff_sparse_vector(&mut diffs, sname, sspec, live),
            None => diffs.push(Difference {
                path: format!("sparse_vectors.{sname}"),
                declared: "present".to_string(),
                live: "missing".to_string(),
            }),
        }
    }
    for sname in live_sparse.keys() {
        if !spec.sparse_vectors.contains_key(sname) {
            diffs.push(Difference {
                path: format!("sparse_vectors.{sname}"),
                declared: "absent".to_string(),
                live: "present (undeclared)".to_string(),
            });
        }
    }

    // --- collection-level config ------------------------------------------
    if let Some(h) = &spec.hnsw_config {
        diff_hnsw(&mut diffs, "hnsw_config", h, config.hnsw_config.as_ref());
    }
    #[allow(deprecated)]
    cmp(
        &mut diffs,
        "on_disk_payload",
        spec.on_disk_payload,
        params.on_disk_payload,
    );
    if let Some(p) = &spec.payload {
        cmp_memory(
            &mut diffs,
            "payload.memory",
            p.memory,
            params.payload.as_ref().and_then(|l| l.memory),
        );
    }
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

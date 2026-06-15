//! Declarative spec types — the on-disk vocabulary for describing Qdrant
//! schema and config.
//!
//! These types are deliberately decoupled from `qdrant-client`: they are the
//! stable, serde-friendly surface that users author in YAML. The executor
//! ([`crate::ops`]) translates them into client calls. Keeping the file format
//! independent of the client crate means a `qdrant-client` upgrade can't
//! silently change the meaning of a committed migration.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Distance metric for a vector field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Distance {
    Cosine,
    Euclid,
    Dot,
    Manhattan,
}

/// Storage datatype for vector elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Datatype {
    Float32,
    Uint8,
    Float16,
}

/// Configuration for a single (dense) named vector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorSpec {
    /// Dimensionality. Immutable once created — changing it requires the
    /// add-vector → re-embed → drop-old dance.
    pub size: u64,
    /// Distance metric. Also immutable in place.
    pub distance: Distance,
    /// Store vectors on disk rather than in RAM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk: Option<bool>,
    /// Per-vector HNSW overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hnsw_config: Option<HnswConfigSpec>,
    /// Per-vector quantization overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization_config: Option<QuantizationSpec>,
    /// Element storage type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datatype: Option<Datatype>,
}

/// How sparse vector indexes are stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SparseVectorSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_scan_threshold: Option<u64>,
}

/// HNSW index parameters. All fields optional — only the ones set are sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HnswConfigSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ef_construct: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_scan_threshold: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_indexing_threads: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_m: Option<u64>,
}

/// Quantization configuration. Exactly one variant should be set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationSpec {
    Scalar(ScalarQuantizationSpec),
    Product(ProductQuantizationSpec),
    Binary(BinaryQuantizationSpec),
    /// Explicitly disable quantization (used by `update_collection`).
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScalarQuantizationSpec {
    /// Quantization type. Only `int8` exists today.
    #[serde(default = "default_int8")]
    pub r#type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantile: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_ram: Option<bool>,
}

fn default_int8() -> String {
    "int8".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductQuantizationSpec {
    /// Compression ratio: `x4`, `x8`, `x16`, `x32`, `x64`.
    pub compression: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_ram: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinaryQuantizationSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_ram: Option<bool>,
}

/// Optimizer thresholds and behaviour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptimizersConfigSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vacuum_min_vector_number: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_segment_number: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_segment_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memmap_threshold: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexing_threshold: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flush_interval_sec: Option<u64>,
}

/// Field types for payload indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadSchemaType {
    Keyword,
    Integer,
    Float,
    Geo,
    Text,
    Bool,
    Datetime,
    Uuid,
}

/// A full collection specification, used both for `create_collection` ops and
/// as the declared desired state for `diff`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionSpec {
    /// Named dense vectors. A single-vector collection uses one entry, by
    /// convention keyed `""` or any chosen name.
    #[serde(default)]
    pub vectors: IndexMap<String, VectorSpec>,
    /// Named sparse vectors.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub sparse_vectors: IndexMap<String, SparseVectorSpec>,
    /// Collection-level HNSW defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hnsw_config: Option<HnswConfigSpec>,
    /// Collection-level quantization defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization_config: Option<QuantizationSpec>,
    /// Optimizer configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimizers_config: Option<OptimizersConfigSpec>,
    /// Number of shards (immutable after create on single-node).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shard_number: Option<u32>,
    /// Replication factor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replication_factor: Option<u32>,
    /// Write consistency factor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_consistency_factor: Option<u32>,
    /// Store the whole collection on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk_payload: Option<bool>,
}

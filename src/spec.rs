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

use crate::error::{Error, Result};

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
    /// TurboQuant 4-bit storage (Qdrant 1.19+).
    Turbo4,
}

/// Where a component's data is held in RAM (Qdrant 1.19+).
///
/// Data is always persisted on disk regardless of this setting; `memory` only
/// controls caching behaviour. It supersedes the older `on_disk` /
/// `always_ram` booleans, which Qdrant still accepts but now marks deprecated —
/// when both are set, `memory` wins. revector keeps emitting whichever of the
/// two a migration declared, so a file committed against 1.18 keeps its exact
/// meaning.
///
/// Qdrant rejects `pinned` for dense vector storage and for the payload store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Memory {
    /// Not pre-loaded; cached lazily as it is used.
    Cold,
    /// Pre-loaded into the disk cache on start, evictable under pressure.
    Cached,
    /// Held in RAM and never evicted.
    Pinned,
}

impl Memory {
    /// The name used in YAML, for diff and error output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Memory::Cold => "cold",
            Memory::Cached => "cached",
            Memory::Pinned => "pinned",
        }
    }
}

/// Configuration for a single (dense) named vector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorSpec {
    /// Dimensionality. Immutable once created — changing it requires the
    /// add-vector → re-embed → drop-old dance.
    pub size: u64,
    /// Distance metric. Also immutable in place.
    pub distance: Distance,
    /// Store vectors on disk rather than in RAM. Superseded by `memory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk: Option<bool>,
    /// Memory placement of the vector storage (Qdrant 1.19+). `pinned` is not
    /// supported for dense vectors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
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
    /// Superseded by `memory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_scan_threshold: Option<u64>,
    /// Memory placement of the inverted index (Qdrant 1.19+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
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
    /// Superseded by `memory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_m: Option<u64>,
    /// Memory placement of the HNSW graph (Qdrant 1.19+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
}

/// Quantization configuration. Exactly one variant should be set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationSpec {
    Scalar(ScalarQuantizationSpec),
    Product(ProductQuantizationSpec),
    Binary(BinaryQuantizationSpec),
    Turboquant(TurboquantQuantizationSpec),
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
    /// Superseded by `memory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_ram: Option<bool>,
    /// Memory placement of the quantized vectors (Qdrant 1.19+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
}

fn default_int8() -> String {
    "int8".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductQuantizationSpec {
    /// Compression ratio: `x4`, `x8`, `x16`, `x32`, `x64`.
    pub compression: String,
    /// Superseded by `memory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_ram: Option<bool>,
    /// Memory placement of the quantized vectors (Qdrant 1.19+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinaryQuantizationSpec {
    /// Superseded by `memory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_ram: Option<bool>,
    /// Memory placement of the quantized vectors (Qdrant 1.19+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurboquantQuantizationSpec {
    /// Bits per component: `1`, `1.5`, `2`, or `4`. Omitted → server default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bits: Option<String>,
    /// Superseded by `memory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_ram: Option<bool>,
    /// Memory placement of the quantized vectors (Qdrant 1.19+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
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

impl PayloadSchemaType {
    /// The name used in YAML, for error messages that quote the file.
    pub fn as_str(&self) -> &'static str {
        match self {
            PayloadSchemaType::Keyword => "keyword",
            PayloadSchemaType::Integer => "integer",
            PayloadSchemaType::Float => "float",
            PayloadSchemaType::Geo => "geo",
            PayloadSchemaType::Text => "text",
            PayloadSchemaType::Bool => "bool",
            PayloadSchemaType::Datetime => "datetime",
            PayloadSchemaType::Uuid => "uuid",
        }
    }
}

/// Payload storage configuration (Qdrant 1.19+) — the successor to the
/// collection-level `on_disk_payload` flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadStorageSpec {
    /// Memory placement of the payload store. `pinned` is not supported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
}

/// Tokenizer used by a `text` payload index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tokenizer {
    Prefix,
    Whitespace,
    Word,
    Multilingual,
}

/// Stopword configuration for a `text` payload index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopwordsSpec {
    /// Built-in stopword lists to apply, by language name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    /// Extra stopwords to apply on top of the built-in lists.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<String>,
}

/// Stemming algorithm for a `text` payload index.
///
/// ```yaml
/// stemmer: { snowball: english }   # Snowball stemmer for a language
/// stemmer: disabled                # opt out of the language default (1.19+)
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StemmerSpec {
    /// Snowball stemmer for the named language.
    Snowball(String),
    /// Explicitly disable stemming, overriding the language default (Qdrant 1.19+).
    Disabled,
}

/// Tuning knobs for a payload index.
///
/// Qdrant models these as a per-field-type union, but the field type is already
/// named by the operation's `schema:`, so revector keeps the YAML flat and
/// checks applicability instead: setting a field that the chosen type doesn't
/// accept is a validation error rather than a silently dropped setting.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadIndexParamsSpec {
    /// Store the index on disk. Superseded by `memory`. All types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk: Option<bool>,
    /// Memory placement of the index (Qdrant 1.19+). All types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
    /// Build payload-aware HNSW links for this field (needs `payload_m > 0`).
    /// All types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_hnsw: Option<bool>,
    /// Optimize storage for multitenancy on this field. `keyword`, `uuid`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_tenant: Option<bool>,
    /// Organize collection storage by this field. `integer`, `float`, `datetime`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_principal: Option<bool>,
    /// Enable `match: { prefix: … }` filtering (Qdrant 1.19+). `keyword` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<bool>,
    /// Support direct lookups. `integer` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lookup: Option<bool>,
    /// Support range filters. `integer` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<bool>,
    /// Tokenizer. `text` only — required when a `text` index declares params.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<Tokenizer>,
    /// Lowercase all tokens. `text` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lowercase: Option<bool>,
    /// Minimum token length. `text` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_token_len: Option<u64>,
    /// Maximum token length. `text` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_token_len: Option<u64>,
    /// Support phrase matching. `text` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phrase_matching: Option<bool>,
    /// Fold accented characters to ASCII. `text` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ascii_folding: Option<bool>,
    /// Stopwords to drop when tokenizing. `text` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopwords: Option<StopwordsSpec>,
    /// Stemming algorithm. `text` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stemmer: Option<StemmerSpec>,
}

/// Params every payload index type accepts.
const COMMON_INDEX_PARAMS: &[&str] = &["on_disk", "memory", "enable_hnsw"];

impl PayloadIndexParamsSpec {
    /// The params this spec actually sets, by YAML field name.
    fn set_fields(&self) -> Vec<&'static str> {
        let mut set = Vec::new();
        let mut push = |name, is_set| {
            if is_set {
                set.push(name);
            }
        };
        push("on_disk", self.on_disk.is_some());
        push("memory", self.memory.is_some());
        push("enable_hnsw", self.enable_hnsw.is_some());
        push("is_tenant", self.is_tenant.is_some());
        push("is_principal", self.is_principal.is_some());
        push("prefix", self.prefix.is_some());
        push("lookup", self.lookup.is_some());
        push("range", self.range.is_some());
        push("tokenizer", self.tokenizer.is_some());
        push("lowercase", self.lowercase.is_some());
        push("min_token_len", self.min_token_len.is_some());
        push("max_token_len", self.max_token_len.is_some());
        push("phrase_matching", self.phrase_matching.is_some());
        push("ascii_folding", self.ascii_folding.is_some());
        push("stopwords", self.stopwords.is_some());
        push("stemmer", self.stemmer.is_some());
        set
    }

    /// The params accepted on top of [`COMMON_INDEX_PARAMS`] for a field type.
    fn extra_fields(schema: PayloadSchemaType) -> &'static [&'static str] {
        match schema {
            PayloadSchemaType::Keyword => &["is_tenant", "prefix"],
            PayloadSchemaType::Uuid => &["is_tenant"],
            PayloadSchemaType::Integer => &["is_principal", "lookup", "range"],
            PayloadSchemaType::Float | PayloadSchemaType::Datetime => &["is_principal"],
            PayloadSchemaType::Geo | PayloadSchemaType::Bool => &[],
            PayloadSchemaType::Text => &[
                "tokenizer",
                "lowercase",
                "min_token_len",
                "max_token_len",
                "phrase_matching",
                "ascii_folding",
                "stopwords",
                "stemmer",
            ],
        }
    }

    /// Reject params the chosen field type doesn't accept.
    ///
    /// Run at parse time so a typo fails `revector validate` offline instead of
    /// halfway through an `up` against a live server.
    pub fn validate_for(&self, schema: PayloadSchemaType) -> Result<()> {
        let extra = Self::extra_fields(schema);
        let rejected: Vec<&str> = self
            .set_fields()
            .into_iter()
            .filter(|f| !COMMON_INDEX_PARAMS.contains(f) && !extra.contains(f))
            .collect();
        if !rejected.is_empty() {
            let mut accepted = COMMON_INDEX_PARAMS.to_vec();
            accepted.extend_from_slice(extra);
            return Err(Error::InvalidOperation(format!(
                "payload index params {rejected:?} do not apply to a `{}` field \
                 (accepted: {accepted:?})",
                schema.as_str()
            )));
        }
        // Qdrant's text index params carry the tokenizer as a required field, so
        // there is no "leave it to the server" value we could send.
        if schema == PayloadSchemaType::Text && self.tokenizer.is_none() {
            return Err(Error::InvalidOperation(
                "a `text` payload index with `params` must set `tokenizer` \
                 (prefix | whitespace | word | multilingual)"
                    .to_string(),
            ));
        }
        Ok(())
    }
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
    /// Store the whole collection payload on disk. Superseded by
    /// `payload.memory`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk_payload: Option<bool>,
    /// Payload storage configuration (Qdrant 1.19+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<PayloadStorageSpec>,
}

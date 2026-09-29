//! Translate declarative [`crate::spec`] types into `qdrant-client` proto/builder
//! types.
//!
//! Isolating every `qdrant_client::qdrant::*` construction here keeps the rest
//! of the crate readable and means a client upgrade only ripples through this
//! one file.

use qdrant_client::qdrant::{
    self, quantization_config, quantization_config_diff, stemming_algorithm, vectors_config,
    BinaryQuantization, BoolIndexParams, CollectionParamsDiff, CreateCollectionBuilder,
    Datatype as QDatatype, DatetimeIndexParams, DisabledStemmer, Distance as QDistance, FieldType,
    FloatIndexParams, GeoIndexParams, HnswConfigDiff, IntegerIndexParams, KeywordIndexParams,
    KeywordPrefixParams, Memory as QMemory, OptimizersConfigDiff, PayloadStorageParams,
    ProductQuantization, QuantizationConfig, ScalarQuantization, SnowballParams, SparseIndexConfig,
    SparseVectorConfig, SparseVectorParams, StemmingAlgorithm, StopwordsSet, TextIndexParams,
    TokenizerType, TurboQuantization, UuidIndexParams, VectorParams, VectorParamsDiff,
    VectorsConfig,
};

use qdrant_client::qdrant::payload_index_params::IndexParams as QIndexParams;

use crate::spec::{
    CollectionSpec, Datatype, Distance, HnswConfigSpec, Memory, OptimizersConfigSpec,
    PayloadIndexParamsSpec, PayloadSchemaType, PayloadStorageSpec, QuantizationSpec,
    SparseVectorSpec, StemmerSpec, StopwordsSpec, Tokenizer, VectorSpec,
};

impl From<Distance> for QDistance {
    fn from(d: Distance) -> Self {
        match d {
            Distance::Cosine => QDistance::Cosine,
            Distance::Euclid => QDistance::Euclid,
            Distance::Dot => QDistance::Dot,
            Distance::Manhattan => QDistance::Manhattan,
        }
    }
}

impl From<Datatype> for QDatatype {
    fn from(d: Datatype) -> Self {
        match d {
            Datatype::Float32 => QDatatype::Float32,
            Datatype::Uint8 => QDatatype::Uint8,
            Datatype::Float16 => QDatatype::Float16,
            Datatype::Turbo4 => QDatatype::Turbo4,
        }
    }
}

impl From<Memory> for QMemory {
    fn from(m: Memory) -> Self {
        match m {
            Memory::Cold => QMemory::Cold,
            Memory::Cached => QMemory::Cached,
            Memory::Pinned => QMemory::Pinned,
        }
    }
}

/// Memory placement as the proto enum discriminant, or `None` when undeclared
/// (leaving the server to apply its own default).
fn memory(m: Option<Memory>) -> Option<i32> {
    m.map(|m| QMemory::from(m) as i32)
}

impl From<Tokenizer> for TokenizerType {
    fn from(t: Tokenizer) -> Self {
        match t {
            Tokenizer::Prefix => TokenizerType::Prefix,
            Tokenizer::Whitespace => TokenizerType::Whitespace,
            Tokenizer::Word => TokenizerType::Word,
            Tokenizer::Multilingual => TokenizerType::Multilingual,
        }
    }
}

impl From<&StopwordsSpec> for StopwordsSet {
    fn from(s: &StopwordsSpec) -> Self {
        StopwordsSet {
            languages: s.languages.clone(),
            custom: s.custom.clone(),
        }
    }
}

impl From<&StemmerSpec> for StemmingAlgorithm {
    fn from(s: &StemmerSpec) -> Self {
        let params = match s {
            StemmerSpec::Snowball(language) => {
                stemming_algorithm::StemmingParams::Snowball(SnowballParams {
                    language: language.clone(),
                })
            }
            StemmerSpec::Disabled => {
                stemming_algorithm::StemmingParams::Disabled(DisabledStemmer {})
            }
        };
        StemmingAlgorithm {
            stemming_params: Some(params),
        }
    }
}

impl From<PayloadSchemaType> for FieldType {
    fn from(t: PayloadSchemaType) -> Self {
        match t {
            PayloadSchemaType::Keyword => FieldType::Keyword,
            PayloadSchemaType::Integer => FieldType::Integer,
            PayloadSchemaType::Float => FieldType::Float,
            PayloadSchemaType::Geo => FieldType::Geo,
            PayloadSchemaType::Text => FieldType::Text,
            PayloadSchemaType::Bool => FieldType::Bool,
            PayloadSchemaType::Datetime => FieldType::Datetime,
            PayloadSchemaType::Uuid => FieldType::Uuid,
        }
    }
}

impl From<&HnswConfigSpec> for HnswConfigDiff {
    // `on_disk` is deprecated in favour of `memory` since Qdrant 1.19, but we
    // keep forwarding it verbatim: a migration committed against an older
    // release must keep meaning exactly what it meant when it was written, and
    // 1.18 servers only understand the boolean. Same reasoning applies to every
    // other `allow(deprecated)` in this file.
    #[allow(deprecated)]
    fn from(h: &HnswConfigSpec) -> Self {
        HnswConfigDiff {
            m: h.m,
            ef_construct: h.ef_construct,
            full_scan_threshold: h.full_scan_threshold,
            max_indexing_threads: h.max_indexing_threads,
            on_disk: h.on_disk,
            payload_m: h.payload_m,
            memory: memory(h.memory),
            ..Default::default()
        }
    }
}

impl From<&OptimizersConfigSpec> for OptimizersConfigDiff {
    fn from(o: &OptimizersConfigSpec) -> Self {
        OptimizersConfigDiff {
            deleted_threshold: o.deleted_threshold,
            vacuum_min_vector_number: o.vacuum_min_vector_number,
            default_segment_number: o.default_segment_number,
            max_segment_size: o.max_segment_size,
            memmap_threshold: o.memmap_threshold,
            indexing_threshold: o.indexing_threshold,
            flush_interval_sec: o.flush_interval_sec,
            ..Default::default()
        }
    }
}

/// Map a compression keyword (`x4`..`x64`) to the proto enum discriminant.
fn compression_ratio(s: &str) -> i32 {
    match s.to_ascii_lowercase().as_str() {
        "x4" => qdrant::CompressionRatio::X4 as i32,
        "x8" => qdrant::CompressionRatio::X8 as i32,
        "x16" => qdrant::CompressionRatio::X16 as i32,
        "x32" => qdrant::CompressionRatio::X32 as i32,
        "x64" => qdrant::CompressionRatio::X64 as i32,
        _ => qdrant::CompressionRatio::X4 as i32,
    }
}

/// Map a TurboQuant bit keyword (`1`, `1.5`, `2`, `4`) to the proto enum
/// discriminant. Unknown values yield `None` so Qdrant applies its own default.
fn turbo_quant_bits(s: &str) -> Option<i32> {
    match s.trim() {
        "1" => Some(qdrant::TurboQuantBitSize::Bits1 as i32),
        "1.5" | "1_5" => Some(qdrant::TurboQuantBitSize::Bits15 as i32),
        "2" => Some(qdrant::TurboQuantBitSize::Bits2 as i32),
        "4" => Some(qdrant::TurboQuantBitSize::Bits4 as i32),
        _ => None,
    }
}

/// Build the `quantization_config::Quantization` oneof used on create.
/// Returns `None` for [`QuantizationSpec::Disabled`] — on create, disabled
/// simply means "don't send any quantization".
#[allow(deprecated)] // `always_ram`: see the note on `HnswConfigDiff::from`.
pub fn quantization_oneof(q: &QuantizationSpec) -> Option<quantization_config::Quantization> {
    match q {
        QuantizationSpec::Scalar(s) => Some(quantization_config::Quantization::Scalar(
            ScalarQuantization {
                r#type: qdrant::QuantizationType::Int8 as i32,
                quantile: s.quantile,
                always_ram: s.always_ram,
                memory: memory(s.memory),
            },
        )),
        QuantizationSpec::Product(p) => Some(quantization_config::Quantization::Product(
            ProductQuantization {
                compression: compression_ratio(&p.compression),
                always_ram: p.always_ram,
                memory: memory(p.memory),
            },
        )),
        QuantizationSpec::Binary(b) => Some(quantization_config::Quantization::Binary(
            BinaryQuantization {
                always_ram: b.always_ram,
                memory: memory(b.memory),
                ..Default::default()
            },
        )),
        QuantizationSpec::Turboquant(t) => Some(quantization_config::Quantization::Turboquant(
            TurboQuantization {
                always_ram: t.always_ram,
                bits: t.bits.as_deref().and_then(turbo_quant_bits),
                memory: memory(t.memory),
            },
        )),
        QuantizationSpec::Disabled => None,
    }
}

/// Build the `quantization_config_diff::Quantization` oneof used on update,
/// where `Disabled` is an explicit variant that clears quantization.
pub fn quantization_diff_oneof(q: &QuantizationSpec) -> quantization_config_diff::Quantization {
    match q {
        QuantizationSpec::Disabled => {
            quantization_config_diff::Quantization::Disabled(qdrant::Disabled {})
        }
        other => match quantization_oneof(other).expect("non-disabled variant yields Some") {
            quantization_config::Quantization::Scalar(s) => {
                quantization_config_diff::Quantization::Scalar(s)
            }
            quantization_config::Quantization::Product(p) => {
                quantization_config_diff::Quantization::Product(p)
            }
            quantization_config::Quantization::Binary(b) => {
                quantization_config_diff::Quantization::Binary(b)
            }
            quantization_config::Quantization::Turboquant(t) => {
                quantization_config_diff::Quantization::Turboquant(t)
            }
        },
    }
}

/// Convert a [`VectorSpec`] into proto [`VectorParams`].
#[allow(deprecated)] // `on_disk`: see the note on `HnswConfigDiff::from`.
pub fn vector_params(v: &VectorSpec) -> VectorParams {
    VectorParams {
        size: v.size,
        distance: QDistance::from(v.distance) as i32,
        hnsw_config: v.hnsw_config.as_ref().map(HnswConfigDiff::from),
        quantization_config: v.quantization_config.as_ref().and_then(|q| {
            quantization_oneof(q).map(|quantization| QuantizationConfig {
                quantization: Some(quantization),
            })
        }),
        on_disk: v.on_disk,
        datatype: v.datatype.map(|d| QDatatype::from(d) as i32),
        multivector_config: None,
        memory: memory(v.memory),
    }
}

/// Build the [`VectorsConfig`] (single or named map) for a collection spec.
pub fn vectors_config(spec: &CollectionSpec) -> VectorsConfig {
    // A single unnamed vector uses the `Params` form; multiple (or any named)
    // vectors use `ParamsMap`. We treat a lone entry keyed "" as unnamed.
    if spec.vectors.len() == 1 && spec.vectors.keys().next().map(String::as_str) == Some("") {
        let params = vector_params(spec.vectors.values().next().unwrap());
        VectorsConfig {
            config: Some(vectors_config::Config::Params(params)),
        }
    } else {
        let map = spec
            .vectors
            .iter()
            .map(|(name, v)| (name.clone(), vector_params(v)))
            .collect();
        VectorsConfig {
            config: Some(vectors_config::Config::ParamsMap(qdrant::VectorParamsMap {
                map,
            })),
        }
    }
}

/// Convert a [`SparseVectorSpec`] into proto [`SparseVectorParams`].
///
/// The spec's `on_disk` / `full_scan_threshold` / `memory` map onto the sparse
/// [`SparseIndexConfig`]; we only attach an index when at least one is set so a
/// bare `{}` sparse entry creates the vector with server defaults.
#[allow(deprecated)] // `on_disk`: see the note on `HnswConfigDiff::from`.
pub fn sparse_vector_params(s: &SparseVectorSpec) -> SparseVectorParams {
    let index = if s.on_disk.is_some() || s.full_scan_threshold.is_some() || s.memory.is_some() {
        Some(SparseIndexConfig {
            full_scan_threshold: s.full_scan_threshold,
            on_disk: s.on_disk,
            datatype: None,
            memory: memory(s.memory),
        })
    } else {
        None
    };
    SparseVectorParams {
        index,
        modifier: None,
    }
}

/// Build the [`SparseVectorConfig`] for a collection spec, or `None` when no
/// sparse vectors are declared (so we don't send an empty map on create).
pub fn sparse_vectors_config(spec: &CollectionSpec) -> Option<SparseVectorConfig> {
    if spec.sparse_vectors.is_empty() {
        return None;
    }
    let map = spec
        .sparse_vectors
        .iter()
        .map(|(name, s)| (name.clone(), sparse_vector_params(s)))
        .collect();
    Some(SparseVectorConfig { map })
}

/// Build a [`VectorParamsDiff`] for in-place vector param updates.
#[allow(deprecated)] // `on_disk`: see the note on `HnswConfigDiff::from`.
pub fn vector_params_diff(d: &crate::ops::VectorParamsDiff) -> VectorParamsDiff {
    VectorParamsDiff {
        hnsw_config: d.hnsw_config.as_ref().map(HnswConfigDiff::from),
        quantization_config: d.quantization_config.as_ref().map(|q| {
            qdrant::QuantizationConfigDiff {
                quantization: Some(quantization_diff_oneof(q)),
            }
        }),
        on_disk: d.on_disk,
        memory: memory(d.memory),
    }
}

/// Apply collection-level config from a [`CollectionSpec`] onto a
/// [`CreateCollectionBuilder`].
pub fn apply_collection_spec(
    mut builder: CreateCollectionBuilder,
    spec: &CollectionSpec,
) -> CreateCollectionBuilder {
    // Dense vectors are optional: a sparse-only collection declares none, and
    // sending an empty vectors map would be rejected by Qdrant.
    if !spec.vectors.is_empty() {
        builder = builder.vectors_config(vectors_config(spec));
    }
    if let Some(sparse) = sparse_vectors_config(spec) {
        builder = builder.sparse_vectors_config(sparse);
    }

    if let Some(h) = &spec.hnsw_config {
        builder = builder.hnsw_config(HnswConfigDiff::from(h));
    }
    if let Some(q) = &spec.quantization_config {
        if let Some(quantization) = quantization_oneof(q) {
            builder = builder.quantization_config(quantization);
        }
    }
    if let Some(o) = &spec.optimizers_config {
        builder = builder.optimizers_config(OptimizersConfigDiff::from(o));
    }
    if let Some(n) = spec.shard_number {
        builder = builder.shard_number(n);
    }
    if let Some(r) = spec.replication_factor {
        builder = builder.replication_factor(r);
    }
    if let Some(w) = spec.write_consistency_factor {
        builder = builder.write_consistency_factor(w);
    }
    if let Some(on_disk) = spec.on_disk_payload {
        builder = builder.on_disk_payload(on_disk);
    }
    if let Some(p) = &spec.payload {
        builder = builder.payload(payload_storage_params(p));
    }
    builder
}

/// Convert a [`PayloadStorageSpec`] into proto [`PayloadStorageParams`]
/// (Qdrant 1.19+).
pub fn payload_storage_params(p: &PayloadStorageSpec) -> PayloadStorageParams {
    PayloadStorageParams {
        memory: memory(p.memory),
    }
}

/// Build a [`CollectionParamsDiff`] from the patchable fields of a spec
/// (replication / write-consistency / payload storage).
#[allow(deprecated)] // `on_disk_payload`: see the note on `HnswConfigDiff::from`.
pub fn collection_params_diff(
    replication_factor: Option<u32>,
    write_consistency_factor: Option<u32>,
    on_disk_payload: Option<bool>,
    payload: Option<&PayloadStorageSpec>,
) -> CollectionParamsDiff {
    CollectionParamsDiff {
        replication_factor,
        write_consistency_factor,
        on_disk_payload,
        payload: payload.map(payload_storage_params),
        ..Default::default()
    }
}

/// Build the per-field-type payload index params union from the flat spec.
///
/// The caller is expected to have validated applicability
/// ([`PayloadIndexParamsSpec::validate_for`]) at parse time; anything not
/// accepted by `schema` is simply not read here.
#[allow(deprecated)] // `on_disk`: see the note on `HnswConfigDiff::from`.
pub fn payload_index_params(schema: PayloadSchemaType, p: &PayloadIndexParamsSpec) -> QIndexParams {
    let on_disk = p.on_disk;
    let memory = memory(p.memory);
    let enable_hnsw = p.enable_hnsw;

    match schema {
        PayloadSchemaType::Keyword => {
            QIndexParams::KeywordIndexParams(KeywordIndexParams {
                is_tenant: p.is_tenant,
                on_disk,
                enable_hnsw,
                // Presence of the message is what enables prefix matching, so
                // `prefix: false` means "leave it off", not "send an empty one".
                prefix: p.prefix.unwrap_or(false).then_some(KeywordPrefixParams {}),
                memory,
            })
        }
        PayloadSchemaType::Integer => QIndexParams::IntegerIndexParams(IntegerIndexParams {
            lookup: p.lookup,
            range: p.range,
            is_principal: p.is_principal,
            on_disk,
            enable_hnsw,
            memory,
        }),
        PayloadSchemaType::Float => QIndexParams::FloatIndexParams(FloatIndexParams {
            on_disk,
            is_principal: p.is_principal,
            enable_hnsw,
            memory,
        }),
        PayloadSchemaType::Geo => QIndexParams::GeoIndexParams(GeoIndexParams {
            on_disk,
            enable_hnsw,
            memory,
        }),
        PayloadSchemaType::Text => {
            QIndexParams::TextIndexParams(TextIndexParams {
                // Validation guarantees a tokenizer is set for text indexes.
                tokenizer: p
                    .tokenizer
                    .map(|t| TokenizerType::from(t) as i32)
                    .unwrap_or(TokenizerType::Unknown as i32),
                lowercase: p.lowercase,
                min_token_len: p.min_token_len,
                max_token_len: p.max_token_len,
                on_disk,
                stopwords: p.stopwords.as_ref().map(StopwordsSet::from),
                phrase_matching: p.phrase_matching,
                stemmer: p.stemmer.as_ref().map(StemmingAlgorithm::from),
                ascii_folding: p.ascii_folding,
                enable_hnsw,
                memory,
            })
        }
        PayloadSchemaType::Bool => QIndexParams::BoolIndexParams(BoolIndexParams {
            on_disk,
            enable_hnsw,
            memory,
        }),
        PayloadSchemaType::Datetime => QIndexParams::DatetimeIndexParams(DatetimeIndexParams {
            on_disk,
            is_principal: p.is_principal,
            enable_hnsw,
            memory,
        }),
        PayloadSchemaType::Uuid => QIndexParams::UuidIndexParams(UuidIndexParams {
            is_tenant: p.is_tenant,
            on_disk,
            enable_hnsw,
            memory,
        }),
    }
}

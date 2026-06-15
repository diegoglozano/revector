//! Translate declarative [`crate::spec`] types into `qdrant-client` proto/builder
//! types.
//!
//! Isolating every `qdrant_client::qdrant::*` construction here keeps the rest
//! of the crate readable and means a client upgrade only ripples through this
//! one file.

use qdrant_client::qdrant::{
    self, quantization_config, quantization_config_diff, vectors_config, BinaryQuantization,
    CollectionParamsDiff, CreateCollectionBuilder, Datatype as QDatatype, Distance as QDistance,
    FieldType, HnswConfigDiff, OptimizersConfigDiff, ProductQuantization, QuantizationConfig,
    ScalarQuantization, VectorParams, VectorParamsDiff, VectorsConfig,
};

use crate::spec::{
    CollectionSpec, Datatype, Distance, HnswConfigSpec, OptimizersConfigSpec, PayloadSchemaType,
    QuantizationSpec, VectorSpec,
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
    fn from(h: &HnswConfigSpec) -> Self {
        HnswConfigDiff {
            m: h.m,
            ef_construct: h.ef_construct,
            full_scan_threshold: h.full_scan_threshold,
            max_indexing_threads: h.max_indexing_threads,
            on_disk: h.on_disk,
            payload_m: h.payload_m,
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

/// Build the `quantization_config::Quantization` oneof used on create.
/// Returns `None` for [`QuantizationSpec::Disabled`] — on create, disabled
/// simply means "don't send any quantization".
pub fn quantization_oneof(q: &QuantizationSpec) -> Option<quantization_config::Quantization> {
    match q {
        QuantizationSpec::Scalar(s) => Some(quantization_config::Quantization::Scalar(
            ScalarQuantization {
                r#type: qdrant::QuantizationType::Int8 as i32,
                quantile: s.quantile,
                always_ram: s.always_ram,
            },
        )),
        QuantizationSpec::Product(p) => Some(quantization_config::Quantization::Product(
            ProductQuantization {
                compression: compression_ratio(&p.compression),
                always_ram: p.always_ram,
            },
        )),
        QuantizationSpec::Binary(b) => Some(quantization_config::Quantization::Binary(
            BinaryQuantization {
                always_ram: b.always_ram,
                ..Default::default()
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

/// Build a [`VectorParamsDiff`] for in-place vector param updates.
pub fn vector_params_diff(d: &crate::ops::VectorParamsDiff) -> VectorParamsDiff {
    VectorParamsDiff {
        hnsw_config: d.hnsw_config.as_ref().map(HnswConfigDiff::from),
        quantization_config: d.quantization_config.as_ref().map(|q| {
            qdrant::QuantizationConfigDiff {
                quantization: Some(quantization_diff_oneof(q)),
            }
        }),
        on_disk: d.on_disk,
    }
}

/// Apply collection-level config from a [`CollectionSpec`] onto a
/// [`CreateCollectionBuilder`].
pub fn apply_collection_spec(
    mut builder: CreateCollectionBuilder,
    spec: &CollectionSpec,
) -> CreateCollectionBuilder {
    builder = builder.vectors_config(vectors_config(spec));

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
    builder
}

/// Build a [`CollectionParamsDiff`] from the patchable fields of a spec
/// (replication / write-consistency / on-disk payload).
pub fn collection_params_diff(
    replication_factor: Option<u32>,
    write_consistency_factor: Option<u32>,
    on_disk_payload: Option<bool>,
) -> CollectionParamsDiff {
    CollectionParamsDiff {
        replication_factor,
        write_consistency_factor,
        on_disk_payload,
        ..Default::default()
    }
}

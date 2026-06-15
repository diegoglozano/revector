//! Migration operations — the verbs a migration can perform.
//!
//! Each [`Operation`] is an internally-tagged enum: a YAML list of steps where
//! each step names its operation via an `op:` field. This reads naturally and,
//! unlike serde's externally-tagged form, doesn't require YAML `!tag` syntax:
//!
//! ```yaml
//! up:
//!   - op: create_collection
//!     name: products
//!     spec:
//!       vectors:
//!         "":
//!           size: 768
//!           distance: Cosine
//!   - op: create_payload_index
//!     collection: products
//!     field_name: category
//!     schema: keyword
//! ```
//!
//! This module holds the *data* and the *reversibility* logic. Execution
//! against a live Qdrant lives in [`crate::executor`] so the file format stays
//! independent of the client crate.

use serde::{Deserialize, Serialize};

use crate::spec::{
    CollectionSpec, HnswConfigSpec, OptimizersConfigSpec, PayloadSchemaType, QuantizationSpec,
    SparseVectorSpec, VectorSpec,
};

/// A single migration step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    /// Create a new collection from a full spec.
    CreateCollection { name: String, spec: CollectionSpec },
    /// Delete a collection entirely. **Irreversible** (data loss).
    DeleteCollection { name: String },
    /// Patch tunable collection-level config in place.
    UpdateCollection(UpdateCollectionOp),
    /// Add a new named dense vector to an existing collection (v1.18+).
    CreateVector {
        collection: String,
        name: String,
        spec: VectorSpec,
    },
    /// Add a new named sparse vector to an existing collection.
    CreateSparseVector {
        collection: String,
        name: String,
        spec: SparseVectorSpec,
    },
    /// Drop a named vector. **Irreversible** (vector data loss).
    DeleteVector { collection: String, name: String },
    /// Create a payload field index.
    CreatePayloadIndex {
        collection: String,
        field_name: String,
        schema: PayloadSchemaType,
    },
    /// Delete a payload field index. Reversible only when `schema` is supplied
    /// so the index can be recreated.
    DeletePayloadIndex {
        collection: String,
        field_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema: Option<PayloadSchemaType>,
    },
    /// Create an alias pointing at a collection.
    CreateAlias { collection: String, alias: String },
    /// Delete an alias. **Irreversible** (target not recorded).
    DeleteAlias { alias: String },
    /// Atomically repoint an alias to a new collection (zero-downtime swap).
    /// Auto-inversion is unavailable because the previous target is not known
    /// without querying; supply an explicit `down` to reverse.
    SwitchAlias {
        alias: String,
        to_collection: String,
    },
    /// Shell out to a user-provided command — the escape hatch for
    /// re-embedding and other data steps the binary can't own.
    Exec(ExecOp),
}

/// Patchable collection-level configuration. Only the set fields are sent to
/// `update_collection`; `None` fields are left untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateCollectionOp {
    pub collection: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hnsw_config: Option<HnswConfigSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization_config: Option<QuantizationSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimizers_config: Option<OptimizersConfigSpec>,
    /// Patch parameters of existing named vectors (on_disk, hnsw, quantization).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vectors: Option<indexmap::IndexMap<String, VectorParamsDiff>>,
}

/// In-place tunables for an existing named vector (size/distance excluded —
/// those are immutable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorParamsDiff {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hnsw_config: Option<HnswConfigSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization_config: Option<QuantizationSpec>,
}

/// A shell command run as a migration step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecOp {
    /// Command line, executed via `sh -c`.
    pub command: String,
    /// Optional human-readable label for status output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Working directory; defaults to the project root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
}

/// The outcome of asking an operation for its automatic inverse.
#[derive(Debug, Clone)]
pub enum Reversibility {
    /// The inverse operation could be derived automatically. Boxed because
    /// `Operation` is large relative to the `Irreversible` variant.
    Auto(Box<Operation>),
    /// The operation cannot be safely auto-reversed; the string explains why.
    Irreversible(String),
}

impl Operation {
    /// A short, human-readable description for status/log output.
    pub fn describe(&self) -> String {
        match self {
            Operation::CreateCollection { name, .. } => format!("create collection `{name}`"),
            Operation::DeleteCollection { name } => format!("delete collection `{name}`"),
            Operation::UpdateCollection(op) => {
                format!("update config of collection `{}`", op.collection)
            }
            Operation::CreateVector {
                collection, name, ..
            } => format!("add vector `{name}` to `{collection}`"),
            Operation::CreateSparseVector {
                collection, name, ..
            } => format!("add sparse vector `{name}` to `{collection}`"),
            Operation::DeleteVector { collection, name } => {
                format!("drop vector `{name}` from `{collection}`")
            }
            Operation::CreatePayloadIndex {
                collection,
                field_name,
                ..
            } => format!("create index on `{collection}.{field_name}`"),
            Operation::DeletePayloadIndex {
                collection,
                field_name,
                ..
            } => format!("drop index on `{collection}.{field_name}`"),
            Operation::CreateAlias { collection, alias } => {
                format!("create alias `{alias}` -> `{collection}`")
            }
            Operation::DeleteAlias { alias } => format!("delete alias `{alias}`"),
            Operation::SwitchAlias {
                alias,
                to_collection,
            } => format!("switch alias `{alias}` -> `{to_collection}`"),
            Operation::Exec(op) => {
                format!(
                    "exec `{}`",
                    op.name.clone().unwrap_or_else(|| op.command.clone())
                )
            }
        }
    }

    /// Derive the operation that undoes this one, when possible.
    ///
    /// Operations that destroy data, or that would need previous state we never
    /// captured, return [`Reversibility::Irreversible`]. A migration author can
    /// always override by supplying an explicit `down` block.
    pub fn auto_inverse(&self) -> Reversibility {
        match self {
            Operation::CreateCollection { name, .. } => {
                Reversibility::Auto(Box::new(Operation::DeleteCollection { name: name.clone() }))
            }
            Operation::DeleteCollection { name } => Reversibility::Irreversible(format!(
                "deleting collection `{name}` destroys all its data; provide an explicit `down` to recreate it"
            )),
            Operation::UpdateCollection(op) => Reversibility::Irreversible(format!(
                "config updates to `{}` are not auto-reversible (previous values unknown); provide an explicit `down`",
                op.collection
            )),
            Operation::CreateVector {
                collection, name, ..
            } => Reversibility::Auto(Box::new(Operation::DeleteVector {
                collection: collection.clone(),
                name: name.clone(),
            })),
            Operation::CreateSparseVector {
                collection, name, ..
            } => Reversibility::Auto(Box::new(Operation::DeleteVector {
                collection: collection.clone(),
                name: name.clone(),
            })),
            Operation::DeleteVector { collection, name } => Reversibility::Irreversible(format!(
                "dropping vector `{name}` from `{collection}` destroys its vector data; provide an explicit `down`"
            )),
            Operation::CreatePayloadIndex {
                collection,
                field_name,
                schema,
            } => Reversibility::Auto(Box::new(Operation::DeletePayloadIndex {
                collection: collection.clone(),
                field_name: field_name.clone(),
                schema: Some(*schema),
            })),
            Operation::DeletePayloadIndex {
                collection,
                field_name,
                schema,
            } => match schema {
                Some(schema) => Reversibility::Auto(Box::new(Operation::CreatePayloadIndex {
                    collection: collection.clone(),
                    field_name: field_name.clone(),
                    schema: *schema,
                })),
                None => Reversibility::Irreversible(format!(
                    "cannot recreate index on `{collection}.{field_name}` without its `schema`; add `schema:` to the op or provide an explicit `down`"
                )),
            },
            Operation::CreateAlias { collection: _, alias } => {
                Reversibility::Auto(Box::new(Operation::DeleteAlias {
                    alias: alias.clone(),
                }))
            }
            Operation::DeleteAlias { alias } => Reversibility::Irreversible(format!(
                "alias `{alias}`'s previous target is unknown; provide an explicit `down`"
            )),
            Operation::SwitchAlias { alias, .. } => Reversibility::Irreversible(format!(
                "alias `{alias}`'s previous target is unknown; provide an explicit `down`"
            )),
            Operation::Exec(op) => Reversibility::Irreversible(format!(
                "exec step `{}` has no automatic inverse; provide an explicit `down`",
                op.name.clone().unwrap_or_else(|| op.command.clone())
            )),
        }
    }
}

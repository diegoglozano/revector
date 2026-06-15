//! Execute migration [`Operation`]s against a live Qdrant.
//!
//! Every operation is written to be **idempotent** where the API allows it:
//! Qdrant treats create-collection / create-index / create-vector on an
//! existing target as errors, so the executor checks existence first and skips
//! no-ops. This is what makes a half-applied migration safe to re-run after a
//! failure — a hard requirement given Qdrant has no transactional DDL.

use qdrant_client::qdrant::{
    CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, CreateVectorNameRequestBuilder,
    DeleteFieldIndexCollectionBuilder, DeleteVectorNameRequestBuilder,
    DenseVectorCreationConfigBuilder, Distance as QDistance, SparseVectorCreationConfigBuilder,
    UpdateCollectionBuilder,
};
use qdrant_client::Qdrant;
use tracing::{debug, info, warn};

use crate::convert;
use crate::error::{Error, Result};
use crate::exec_hook;
use crate::ops::{ExecOp, Operation, UpdateCollectionOp};

/// Runs operations against a Qdrant instance.
pub struct Executor<'a> {
    client: &'a Qdrant,
    /// Project root used to resolve relative exec-hook working directories.
    project_root: std::path::PathBuf,
    /// When true, log what would happen without mutating Qdrant.
    dry_run: bool,
}

impl<'a> Executor<'a> {
    /// Build an executor bound to a client and project root.
    pub fn new(client: &'a Qdrant, project_root: impl Into<std::path::PathBuf>) -> Self {
        Executor {
            client,
            project_root: project_root.into(),
            dry_run: false,
        }
    }

    /// Toggle dry-run mode (no mutations, exec-hooks skipped).
    pub fn dry_run(mut self, on: bool) -> Self {
        self.dry_run = on;
        self
    }

    /// Execute a single operation.
    pub async fn execute(&self, op: &Operation) -> Result<()> {
        info!(target: "revector", "  → {}", op.describe());
        if self.dry_run {
            debug!("dry-run: skipping `{}`", op.describe());
            return Ok(());
        }
        match op {
            Operation::CreateCollection { name, spec } => {
                if self.client.collection_exists(name).await? {
                    warn!("collection `{name}` already exists; skipping create");
                    return Ok(());
                }
                let builder = CreateCollectionBuilder::new(name);
                let builder = convert::apply_collection_spec(builder, spec);
                self.client.create_collection(builder).await?;
            }
            Operation::DeleteCollection { name } => {
                if !self.client.collection_exists(name).await? {
                    warn!("collection `{name}` does not exist; skipping delete");
                    return Ok(());
                }
                self.client.delete_collection(name.as_str()).await?;
            }
            Operation::UpdateCollection(op) => self.update_collection(op).await?,
            Operation::CreateVector {
                collection,
                name,
                spec,
            } => {
                let mut config = DenseVectorCreationConfigBuilder::new(
                    spec.size,
                    QDistance::from(spec.distance),
                );
                if let Some(dt) = spec.datatype {
                    config = config.datatype(qdrant_client::qdrant::Datatype::from(dt) as i32);
                }
                if spec.hnsw_config.is_some() || spec.quantization_config.is_some() {
                    warn!(
                        "per-vector hnsw/quantization on `{collection}.{name}` is ignored on add; \
                         set it with a follow-up update_collection step"
                    );
                }
                self.client
                    .create_vector_name(
                        CreateVectorNameRequestBuilder::new(collection, name, config).wait(true),
                    )
                    .await?;
            }
            Operation::CreateSparseVector {
                collection, name, ..
            } => {
                let config = SparseVectorCreationConfigBuilder::new();
                self.client
                    .create_vector_name(
                        CreateVectorNameRequestBuilder::new(collection, name, config).wait(true),
                    )
                    .await?;
            }
            Operation::DeleteVector { collection, name } => {
                self.client
                    .delete_vector_name(
                        DeleteVectorNameRequestBuilder::new(collection, name).wait(true),
                    )
                    .await?;
            }
            Operation::CreatePayloadIndex {
                collection,
                field_name,
                schema,
            } => {
                self.client
                    .create_field_index(
                        CreateFieldIndexCollectionBuilder::new(
                            collection,
                            field_name,
                            (*schema).into(),
                        )
                        .wait(true),
                    )
                    .await?;
            }
            Operation::DeletePayloadIndex {
                collection,
                field_name,
                ..
            } => {
                self.client
                    .delete_field_index(
                        DeleteFieldIndexCollectionBuilder::new(collection, field_name).wait(true),
                    )
                    .await?;
            }
            Operation::CreateAlias { collection, alias } => {
                self.client
                    .create_alias(qdrant_client::qdrant::CreateAliasBuilder::new(
                        collection.as_str(),
                        alias.as_str(),
                    ))
                    .await?;
            }
            Operation::DeleteAlias { alias } => {
                self.client.delete_alias(alias.as_str()).await?;
            }
            Operation::SwitchAlias {
                alias,
                to_collection,
            } => {
                // create_alias on an existing alias atomically repoints it.
                self.client
                    .create_alias(qdrant_client::qdrant::CreateAliasBuilder::new(
                        to_collection.as_str(),
                        alias.as_str(),
                    ))
                    .await?;
            }
            Operation::Exec(op) => self.run_exec(op).await?,
        }
        Ok(())
    }

    async fn update_collection(&self, op: &UpdateCollectionOp) -> Result<()> {
        let mut builder = UpdateCollectionBuilder::new(&op.collection);
        let mut touched = false;

        if let Some(h) = &op.hnsw_config {
            builder = builder.hnsw_config(qdrant_client::qdrant::HnswConfigDiff::from(h));
            touched = true;
        }
        if let Some(q) = &op.quantization_config {
            builder = builder.quantization_config(convert::quantization_diff_oneof(q));
            touched = true;
        }
        if let Some(o) = &op.optimizers_config {
            builder =
                builder.optimizers_config(qdrant_client::qdrant::OptimizersConfigDiff::from(o));
            touched = true;
        }
        if let Some(vectors) = &op.vectors {
            let map = vectors
                .iter()
                .map(|(name, diff)| (name.clone(), convert::vector_params_diff(diff)))
                .collect();
            builder = builder.vectors_config(qdrant_client::qdrant::VectorsConfigDiff {
                config: Some(
                    qdrant_client::qdrant::vectors_config_diff::Config::ParamsMap(
                        qdrant_client::qdrant::VectorParamsDiffMap { map },
                    ),
                ),
            });
            touched = true;
        }

        if !touched {
            return Err(Error::InvalidOperation(format!(
                "update_collection for `{}` sets no fields",
                op.collection
            )));
        }
        self.client.update_collection(builder).await?;
        Ok(())
    }

    async fn run_exec(&self, op: &ExecOp) -> Result<()> {
        exec_hook::run(op, &self.project_root).await
    }
}

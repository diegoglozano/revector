//! Applied-revision bookkeeping, stored inside Qdrant itself.
//!
//! revector tracks state in a dedicated collection (default
//! `_revector_migrations`) so it needs no external database — the same
//! self-contained approach Qdrant's own data tool takes. Each applied revision
//! is one point: a dummy 1-d vector plus a payload recording the revision id,
//! its parent, checksum, description, and the time it was applied.

use std::collections::HashMap;

use qdrant_client::qdrant::{
    point_id::PointIdOptions, CreateCollectionBuilder, Distance, PointId, PointStruct,
    PointsIdsList, ScrollPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::{Payload, Qdrant};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::migration::Migration;

/// One row of migration history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedRecord {
    pub revision: String,
    #[serde(default)]
    pub down_revision: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub checksum: String,
    /// RFC 3339 timestamp of when the revision was applied.
    pub applied_at: String,
}

/// Handle to the tracking collection.
pub struct Tracker<'a> {
    client: &'a Qdrant,
    collection: String,
}

impl<'a> Tracker<'a> {
    /// Bind a tracker to a client and collection name.
    pub fn new(client: &'a Qdrant, collection: impl Into<String>) -> Self {
        Tracker {
            client,
            collection: collection.into(),
        }
    }

    /// Ensure the tracking collection exists, creating it if necessary.
    ///
    /// Idempotent: a no-op when the collection is already present. Uses a
    /// 1-dimensional Dot-distance vector (zero vectors are invalid under
    /// Cosine) since the vector is never actually queried.
    pub async fn ensure(&self) -> Result<()> {
        if self.client.collection_exists(&self.collection).await? {
            return Ok(());
        }
        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection)
                    .vectors_config(VectorParamsBuilder::new(1, Distance::Dot)),
            )
            .await?;
        Ok(())
    }

    /// Stable point id for a revision (UUIDv5 over the revision string).
    fn point_id(revision: &str) -> PointId {
        let uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, revision.as_bytes());
        PointId {
            point_id_options: Some(PointIdOptions::Uuid(uuid.to_string())),
        }
    }

    /// Record a revision as applied.
    pub async fn record(&self, migration: &Migration) -> Result<()> {
        let now = OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "unknown".to_string());
        let record = AppliedRecord {
            revision: migration.revision().to_string(),
            down_revision: migration.down_revision().map(str::to_string),
            description: migration.file.description.clone(),
            checksum: migration.checksum.clone(),
            applied_at: now,
        };
        let json = serde_json::to_value(&record).map_err(|e| Error::Other(e.into()))?;
        let payload: Payload = Payload::try_from(json)?;

        let point = PointStruct::new(Self::point_id(migration.revision()), vec![0.0f32], payload);
        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection, vec![point]).wait(true))
            .await?;
        Ok(())
    }

    /// Remove a revision from the applied set (used on downgrade).
    pub async fn remove(&self, revision: &str) -> Result<()> {
        self.client
            .delete_points(
                qdrant_client::qdrant::DeletePointsBuilder::new(&self.collection)
                    .points(PointsIdsList {
                        ids: vec![Self::point_id(revision)],
                    })
                    .wait(true),
            )
            .await?;
        Ok(())
    }

    /// Fetch all applied revisions, keyed by revision id.
    pub async fn applied(&self) -> Result<HashMap<String, AppliedRecord>> {
        self.ensure().await?;
        let mut out = HashMap::new();
        let mut offset: Option<PointId> = None;
        loop {
            let mut builder = ScrollPointsBuilder::new(&self.collection)
                .with_payload(true)
                .limit(256u32);
            if let Some(off) = offset.take() {
                builder = builder.offset(off);
            }
            let resp = self.client.scroll(builder).await?;
            for point in resp.result {
                let mut map = serde_json::Map::new();
                for (k, v) in point.payload {
                    map.insert(k, v.into_json());
                }
                match serde_json::from_value::<AppliedRecord>(serde_json::Value::Object(map)) {
                    Ok(record) => {
                        out.insert(record.revision.clone(), record);
                    }
                    Err(e) => {
                        tracing::warn!("skipping unparseable tracking record: {e}");
                    }
                }
            }
            match resp.next_page_offset {
                Some(next) => offset = Some(next),
                None => break,
            }
        }
        Ok(out)
    }
}

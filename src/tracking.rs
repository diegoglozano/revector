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

/// The advisory lock record, stored as a single control point in the tracking
/// collection so concurrent `revector` runs can detect each other.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockRecord {
    pub lock_holder: String,
    /// RFC 3339 timestamp of when the lock was acquired.
    pub acquired_at: String,
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

    /// The control-point id used for the advisory lock. Distinct from any
    /// revision id (revisions are namespaced by their own string).
    fn lock_uuid() -> String {
        Uuid::new_v5(&Uuid::NAMESPACE_OID, b"__revector_lock__").to_string()
    }

    fn lock_id() -> PointId {
        PointId {
            point_id_options: Some(PointIdOptions::Uuid(Self::lock_uuid())),
        }
    }

    fn now_rfc3339() -> String {
        OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "unknown".to_string())
    }

    /// Read the current lock record, if any. The tracking collection is tiny,
    /// so a scroll is cheap and avoids depending on a points-retrieve API.
    pub async fn read_lock(&self) -> Result<Option<LockRecord>> {
        let lock_uuid = Self::lock_uuid();
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
                let is_lock = matches!(
                    &point.id,
                    Some(PointId { point_id_options: Some(PointIdOptions::Uuid(u)) }) if *u == lock_uuid
                );
                if !is_lock {
                    continue;
                }
                let mut map = serde_json::Map::new();
                for (k, v) in point.payload {
                    map.insert(k, v.into_json());
                }
                if let Ok(record) =
                    serde_json::from_value::<LockRecord>(serde_json::Value::Object(map))
                {
                    return Ok(Some(record));
                }
            }
            match resp.next_page_offset {
                Some(next) => offset = Some(next),
                None => break,
            }
        }
        Ok(None)
    }

    /// Acquire the advisory lock. Fails with [`Error::Locked`] if another holder
    /// already has it, unless `force` is set (to break a stale lock).
    ///
    /// Best-effort: Qdrant has no compare-and-set, so this is advisory — it
    /// reliably catches the common "two runs at once" case but is not a hard
    /// mutex against a precise simultaneous race.
    pub async fn acquire_lock(&self, holder: &str, force: bool) -> Result<()> {
        if let Some(existing) = self.read_lock().await? {
            if !force {
                return Err(Error::Locked {
                    holder: existing.lock_holder,
                    since: existing.acquired_at,
                });
            }
            tracing::warn!(
                "overriding lock held by {} since {}",
                existing.lock_holder,
                existing.acquired_at
            );
        }
        let record = LockRecord {
            lock_holder: holder.to_string(),
            acquired_at: Self::now_rfc3339(),
        };
        let json = serde_json::to_value(&record).map_err(|e| Error::Other(e.into()))?;
        let payload: Payload = Payload::try_from(json)?;
        let point = PointStruct::new(Self::lock_id(), vec![0.0f32], payload);
        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection, vec![point]).wait(true))
            .await?;
        Ok(())
    }

    /// Release the advisory lock (best-effort; safe to call when not held).
    pub async fn release_lock(&self) -> Result<()> {
        self.client
            .delete_points(
                qdrant_client::qdrant::DeletePointsBuilder::new(&self.collection)
                    .points(PointsIdsList {
                        ids: vec![Self::lock_id()],
                    })
                    .wait(true),
            )
            .await?;
        Ok(())
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
                // Control points (e.g. the advisory lock) carry no `revision`
                // field — skip them quietly rather than warning.
                if !map.contains_key("revision") {
                    continue;
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

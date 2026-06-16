---
name: revector
description: >-
  Manage Qdrant collection schema and config with versioned, reversible
  migrations using revector (the "Alembic for Qdrant"). Use when: adding or
  dropping a payload index; tuning hnsw_config, quantization, or optimizers;
  adding or removing a named or sparse vector; changing a vector's size or
  distance; promoting collection changes across dev/staging/prod; rolling back
  a collection change; adopting an existing collection into version control; or
  wiring Qdrant schema migrations into CI.
---

# revector — versioned Qdrant schema migrations

First decide which *kind* of change this is — they use different tools.
**Schema/config** changes (payload indexes, hnsw/quantization/optimizers,
named-vector add/drop, aliases) are mutable in place and belong in versioned,
ordered migration files — that is what revector manages. **Moving points**
between instances is data movement: use `qdrant/migration`, not revector.
**Re-embedding** with a new model is a data+model change: drive the embedding
side with the `qdrant-model-migration` skill — revector only orchestrates the
schema and shells out for the embed step.

## Changing tunable config (hnsw / quantization / optimizers)
Use when: tuning search quality or memory after a collection already has data.
- These are patchable in place via [update_collection](https://qdrant.tech/documentation/concepts/collections/#update-collection-parameters); author an `update_collection` migration op.
- `size` and `distance` are NOT patchable — see "Changing a vector's size or distance".
- Config updates aren't auto-reversible (prior values aren't recorded), so write an explicit `down` block.

## Adding or dropping a named vector (Qdrant v1.18+)
Use when: introducing a second [named vector](https://qdrant.tech/documentation/concepts/vectors/#named-vectors) (e.g. a new modality) on a live collection.
- Add with a `create_vector` op; it auto-reverses to `delete_vector`.
- Dropping a vector destroys its data — mark it deliberate, in its own migration, with an explicit `down`.

## Payload indexes
Use when: you start filtering on a field, or need tenant isolation.
- Use `create_payload_index` / `delete_payload_index` ops; see [payload indexing](https://qdrant.tech/documentation/concepts/indexing/#payload-index).
- To make a delete reversible, keep the field `schema` on the op so revector can recreate it.

## Changing a vector's size or distance (model change)
Use when: switching embedding model or dimensionality.
- These are immutable in place — Qdrant will not alter them. Do not recreate the whole collection.
- Pattern: add a new named vector → re-embed via a revector `exec` hook → drop the old vector once cut over. Coordinate the re-embedding with the `qdrant-model-migration` skill.

## Adopting an existing collection
Use when: the collection already exists and was not created by revector.
- Run `revector stamp <revision>` to record state without executing any ops, then `revector up` for everything after it. Do not re-run the create migration against live data.

## Running in CI / promoting across environments
Use when: applying the same schema to staging/prod or inside a pipeline.
- Gate PRs with `revector validate` (offline: parses + resolves the chain, no DB).
- `revector up` is idempotent and resumable (Qdrant has no transactional DDL), and takes an advisory lock so parallel jobs don't race.
- Rollbacks require `--yes` on non-interactive shells; pass `--force` only to clear a stale lock from a crashed run.

## Detecting drift
Use when: someone may have changed a collection by hand.
- `revector diff <collection> --spec <file>` compares declared vs live, comparing only fields you actually declared (avoids false positives from Qdrant's normalized defaults).

## What NOT to Do
- Don't use revector to copy points between clusters — that is data movement (`qdrant/migration`).
- Don't try to change `size`/`distance` in place — add a new vector and re-embed instead.
- Don't edit a migration after it has been applied — revector's checksum guard refuses it; add a new revision.
- Don't assume a downgrade is lossless — dropping a vector or collection destroys data; treat irreversible steps as one-way.
- Don't make ad-hoc schema tweaks in prod outside migrations — that causes drift; catch it with `revector diff`.

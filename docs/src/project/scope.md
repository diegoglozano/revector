# Scope & limitations

- **Schema, not data.** revector manages collection schema and config —
  collections, payload indexes, named vectors, aliases, and tunable knobs.
  Moving points between instances is out of scope; use
  [`qdrant/migration`](https://github.com/qdrant/migration) for that. The one
  data operation revector helps with — re-embedding — goes through the
  [exec-hook](../guides/re-embedding.md).
- **Linear chains only** (single base, single head) in v1 — branching/merging
  is rejected with a clear error.
- Per-vector `hnsw_config` / `quantization_config` can't be set at
  `create_vector` time (Qdrant's add-vector API doesn't accept them); apply
  them with a follow-up `update_collection` step.
- `diff` reads a standalone spec file; folding the full migration chain into a
  desired-state spec is future work.

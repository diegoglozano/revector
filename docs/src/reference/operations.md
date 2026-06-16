# Operations

Each step inside `up:` / `down:` is identified by an `op:` key. The table
summarizes effect and auto-reversibility; click through for a detailed
description, runnable example, and the full list of spec fields.

| `op:` | Effect | Auto-reversible? |
|-------|--------|------------------|
| [`create_collection`](./operations/create_collection.md) | Create a collection from a full spec | ✔ → `delete_collection` |
| [`delete_collection`](./operations/delete_collection.md) | Drop a collection | ✘ (data loss) |
| [`update_collection`](./operations/update_collection.md) | Patch `hnsw_config`, `quantization_config`, `optimizers_config`, or per-vector params in place | ✘ (prior state unknown) |
| [`create_vector`](./operations/create_vector.md) | Add a named dense vector (Qdrant v1.18+) | ✔ → `delete_vector` |
| [`create_sparse_vector`](./operations/create_sparse_vector.md) | Add a named sparse vector | ✔ → `delete_vector` |
| [`delete_vector`](./operations/delete_vector.md) | Drop a named vector | ✘ (data loss) |
| [`create_payload_index`](./operations/create_payload_index.md) | Index a payload field | ✔ → `delete_payload_index` |
| [`delete_payload_index`](./operations/delete_payload_index.md) | Remove a payload index | ✔ iff `schema:` is given |
| [`create_alias`](./operations/create_alias.md) | Point an alias at a collection | ✔ → `delete_alias` |
| [`delete_alias`](./operations/delete_alias.md) | Remove an alias | ✘ (target unknown) |
| [`switch_alias`](./operations/switch_alias.md) | Atomically repoint an alias (zero-downtime swap) | ✘ (prior target unknown) |
| [`exec`](./operations/exec.md) | Run a shell command (the re-embedding escape hatch) | ✘ unless `down` provided |

When an operation isn't auto-reversible, supply an explicit `down:` block — then
`revector down` uses it verbatim.

The shapes referenced by these operations (`CollectionSpec`, `VectorSpec`,
`HnswConfigSpec`, …) are documented on the [Specs](./specs.md) page.

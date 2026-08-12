# Changelog

All notable changes to revector are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and the project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Qdrant 1.19 support.** The `qdrant-client` dependency moves to 1.19 and the
  integration suite is pinned to server `v1.19.0`. revector 0.4.x does not build
  against the 1.19 client, so this is the upgrade path.
- Memory placement (`memory: cold | cached | pinned`) — Qdrant 1.19's unified
  replacement for the `on_disk` / `always_ram` / `on_disk_payload` booleans.
  Accepted on `VectorSpec`, `HnswConfigSpec`, `SparseVectorSpec`, every
  quantization variant, `update_collection.vectors.*`, payload index `params`,
  and the new collection-level `payload` block. Declaring the legacy booleans
  keeps working and keeps sending exactly what it always sent — nothing is
  rewritten under a committed migration.
- `payload: { memory: … }` on `create_collection` specs and `update_collection`,
  Qdrant 1.19's successor to `on_disk_payload`.
- `turbo4` datatype — TurboQuant 4-bit primary vector storage (Qdrant 1.19).
- `create_payload_index.params` / `delete_payload_index.params` — payload index
  tuning knobs, including Qdrant 1.19's keyword `prefix` matching and `stemmer:
  disabled`, alongside the pre-existing ones (`is_tenant`, `is_principal`,
  `lookup`, `range`, `enable_hnsw`, and the text analysis settings). Params ride
  along into the auto-generated inverse, so a rollback recreates the same index.
  Params that don't apply to the declared `schema:` are rejected when the file is
  parsed, so `revector validate` catches them offline.
- `diff` compares declared memory placement for vectors, HNSW graphs, sparse
  indexes, and the payload store.

### Changed
- `create_vector` now warns when a spec declares `on_disk` or `memory`, which
  Qdrant's add-vector API cannot accept — previously `on_disk` was dropped
  silently. Apply it with a follow-up `update_collection`, as with
  `hnsw_config` / `quantization_config`.

## [0.4.0] - 2026-07-12

### Added
- TurboQuant quantization (`quantization_config.turboquant`) — Qdrant's fast,
  data-oblivious quantization (1.18+). Configurable `bits` (`1`, `1.5`, `2`,
  `4`) and `always_ram`, usable anywhere a `QuantizationSpec` is accepted
  (`create_collection`, `update_collection`, per-vector).

## [0.3.0] - 2026-06-25

### Added
- Docs: a CI/CD integration guide covering offline `validate` on pull requests,
  `up` on deploy, drift checks via `diff`, and non-interactive usage, with
  GitHub Actions and GitLab CI examples.

### Fixed
- `create_collection` now provisions the `sparse_vectors` declared in a spec.
  They were silently dropped during spec→client conversion, so collections came
  up dense-only (and sparse-only specs sent an empty dense map). `diff` now also
  compares declared sparse vectors against the live collection.

## [0.2.0]

### Added
- `revector validate` — parse all migrations and resolve the revision chain
  offline (no Qdrant connection); a fast CI / pre-commit check.
- `revector stamp <rev|head|base>` — mark the database as being at a revision
  **without running** any operations (Alembic's `stamp`), for adopting an
  existing collection. Supports `--dry-run`.
- Advisory locking — `up`/`down`/`to`/`stamp` take a lock record in the tracking
  collection so concurrent runs don't race; `--force` overrides a stale lock.
- Rollback confirmations — `down` (and a backwards `to`) prompt before running;
  `-y`/`--yes` skips, and a non-interactive shell refuses without it.
- A Qdrant-style Agent Skill (`skills/revector/SKILL.md`) for schema migrations.
- Supply-chain CI: `cargo-deny` (advisories/licenses/bans/sources, run weekly),
  Dependabot, and SLSA build-provenance attestations on release artifacts.

### Notes
- Linear revision chains only; per-vector hnsw/quantization can't be set at
  `create_vector` time (apply via a follow-up `update_collection`).

## [0.1.0]

### Added
- Initial release: declarative, versioned schema & config migrations for Qdrant.
- Operations: create/delete collection, in-place config updates
  (hnsw/quantization/optimizers/per-vector), named dense & sparse vector
  add/drop, payload index create/delete, alias create/delete/switch, and an
  `exec` hook for re-embedding.
- Alembic-style revision chain with checksum tracking inside Qdrant.
- Commands: `init`, `new`, `status`, `up`, `down`, `to`, `diff`.
- Distribution via cargo-dist (binaries + installers), crates.io, and Homebrew.

[0.3.0]: https://github.com/diegoglozano/revector/releases/tag/v0.3.0
[0.2.0]: https://github.com/diegoglozano/revector/releases/tag/v0.2.0
[0.1.0]: https://github.com/diegoglozano/revector/releases/tag/v0.1.0

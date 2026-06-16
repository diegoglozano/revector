# Roadmap

Tracking for follow-up work. Items are roughly in priority order within each
section.

## Distribution

- [x] **cargo-dist** — prebuilt binaries + shell/PowerShell installers + a
  Homebrew formula, released on tag push (`.github/workflows/release.yml`).
- [x] **crates.io** — `cargo install revector` (published `v0.1.0`; auto-publishes
  on version tags via `.github/workflows/publish-crate.yml`).
- [x] **Homebrew** — `brew install diegoglozano/tap/revector` (formula auto-pushed
  to the tap on release).
- [ ] **PyPI via maturin (for `uvx` / `pipx` / `pip`)** — package the binary
  crate into platform wheels with `maturin` (no PyO3 bindings needed; same
  pattern Astral uses for `ruff`). Add a `maturin publish` CI job gated on tags.
  Enables `uvx revector …` for the embeddings/uv crowd — high ROI for our
  audience. See discussion: maturin can build wheels for a `[[bin]]`-only crate
  that install the executable as a console script.
- [ ] **Docker image on ghcr.io** — multi-stage build → `FROM scratch` with the
  musl static binary, for CI/CD pipelines that run migrations as a step.
- Go (`go install`): not possible for a Rust binary — intentionally skipped.

## Correctness / features

- [ ] **`diff` from the migration chain** — fold `create_collection` /
  `update_collection` / `create_vector` ops into a desired-state spec so
  `revector diff <collection>` needs no separate `--spec` file.
- [ ] **Auto-reversible `update_collection`** — snapshot live config into the
  tracking payload before patching, so config changes can be rolled back
  without a hand-written `down`.
- [ ] **Optimizer-status polling** — after index/config builds, poll
  `collection_info` until optimizers settle so "done" isn't fuzzy.
- [ ] More live integration coverage: irreversible-downgrade refusal and the
  checksum-mismatch guard (both have unit coverage today).

## Qdrant coverage gaps

The automatable surface we haven't exposed as ops yet:

- [ ] **`update_collection` params patch** — replication_factor,
  write_consistency_factor, on_disk_payload (the `collection_params_diff` helper
  exists but isn't wired into `UpdateCollectionOp`).
- [ ] **`strict_mode_config`** and collection **`metadata`** — both listed in the
  capability matrix; `update_collection` supports them.
- [ ] **Payload index params** — `create_payload_index` only takes a schema type;
  expose `is_tenant`, `on_disk`, `is_principal`, text tokenizer, etc. (key for
  the multitenancy / tenant-isolation skill).
- [ ] **Sharding-key** create/delete operations.

## Operational / UX (table-stakes for a migration tool)

- [x] **`stamp` / `baseline`** — mark the DB as already at revision X without
  running it (Alembic `stamp`), so revector can adopt an existing collection
  without re-creating it. Supports a revision id plus `head` / `base`.
- [x] **`validate` / lint** — parse all migrations + resolve the chain with no DB
  connection (great as a CI/pre-commit check).
- [x] **Advisory locking** — `up`/`down`/`to`/`stamp` take a lock record in the
  tracking collection; concurrent runs fail with a clear error, `--force`
  overrides a stale lock. (Best-effort; no compare-and-set in Qdrant.)
- [x] **Confirmations + `--yes`** — rollbacks prompt before proceeding; `--yes`
  skips, and a non-interactive shell refuses without it.
- [ ] **`--json` output** on `status` / `up` / `down` / `diff` for CI consumption.

## Ecosystem & docs

- [ ] **mdBook documentation site.**
- [ ] **Align with / publish a Qdrant "skill"** — Qdrant Skills
  (https://skills.qdrant.tech, https://github.com/qdrant/skills) are agent-facing
  "when/why" decision trees; their topics include **model migration** and
  **version upgrades**, which is exactly revector's lane. Worth: (a) authoring a
  `revector` skill so agents reach for it during schema/model migrations, and
  (b) making the re-embedding/add-vector→drop-vector flow a first-class,
  documented "model migration" recipe.

## Multi-backend support (v2/v3)

Generalize beyond Qdrant to other open-source vector DBs. The architecture is
already half-prepared: the declarative `spec`/`ops` layer is decoupled from the
client (all Qdrant calls live in `convert.rs` + `executor.rs`).

- [ ] Introduce a `Backend` trait (create/delete collection, create/delete index,
  add/drop vector, config patch, alias ops, tracking-store read/write) and move
  the Qdrant implementation behind it.
- [ ] Per-backend capability flags — each engine supports a different in-place
  surface; `diff`/`up` must degrade gracefully and reject unsupported ops with a
  clear error.
- [ ] Candidate backends (rough order of fit): **pgvector** (DDL maps cleanly to
  SQL, huge audience), **Milvus**, **Weaviate**, **LanceDB**, **Chroma**.
  Pinecone is managed/closed, so lower priority.
- [ ] Decide tracking-store strategy per backend (in-DB collection vs. a sidecar
  table) and keep the revision-chain/checksum logic backend-agnostic.
- Open question: does multi-backend dilute the "Alembic for Qdrant" focus? Lean
  toward making Qdrant feature-complete first, then abstract.

## v2 (engine-level)

- [ ] Branching/merging revision graphs (v1 enforces a single linear chain).
- [ ] PyO3 / uniffi bindings (deferred — low payoff for a network-I/O workload).

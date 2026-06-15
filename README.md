# revector

**Declarative, versioned schema & config migrations for [Qdrant](https://qdrant.tech) — Alembic for vector collections.**

[![CI](https://github.com/diegoglozano/revector/actions/workflows/ci.yml/badge.svg)](https://github.com/diegoglozano/revector/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

revector brings ordered, reversible, database-tracked migrations to Qdrant —
the piece that, unlike for relational databases, didn't exist yet. You write
declarative YAML migrations, commit them next to your code, and apply or roll
them back with a single static binary. No Python venv, no external state store.

> **Schema, not data.** revector manages collection **schema and config** —
> collections, payload indexes, named vectors, aliases, and all tunable knobs.
> Moving *points* between instances is a solved problem (see
> [`qdrant/migration`](https://github.com/qdrant/migration)); that's explicitly
> out of scope. The one data operation revector *does* help with — re-embedding
> — is handled through an [exec-hook](#re-embedding-the-exec-hook).

---

## Why

Qdrant collections drift. You tune `hnsw_config`, add a payload index, introduce
a second named vector for a new model, flip quantization on. Today those changes
live in ad-hoc scripts or a teammate's shell history. revector makes them:

- **Versioned** — each change is a file with a `revision` and `down_revision`,
  forming an ordered chain (Alembic's model).
- **Tracked** — applied revisions are recorded *inside Qdrant itself*, in a
  dedicated `_revector_migrations` collection. No external database.
- **Reversible (honestly)** — downgrades are auto-derived where safe and
  **refused loudly** where they'd lose data, instead of pretending.
- **Idempotent & resumable** — Qdrant has no transactional DDL and builds
  indexes asynchronously, so every step is safe to re-run after a failure.

## Install

```sh
# From source (requires Rust 1.82+)
cargo install --path .

# Or build a static binary
cargo build --release   # ./target/release/revector
```

## Quick start

```sh
# 1. Scaffold a config + migrations/ directory
revector init

# 2. Create your first migration
revector new "create products collection"
#  → migrations/1718480000_create_products_collection.yaml

# 3. Edit the file (see the format below), then apply
export REVECTOR_URL=http://localhost:6334
revector up

# 4. Inspect state
revector status

# 5. Roll back the last migration
revector down
```

## Migration files

A migration is a YAML file with a revision id, a link to its parent, and `up` /
optional `down` operation lists. Each operation names itself with an `op:` key.

```yaml
revision: "0001_products"
down_revision: null            # null marks the base of the chain
description: create products collection

up:
  - op: create_collection
    name: products
    spec:
      vectors:
        "":                    # "" is the unnamed/default vector
          size: 768
          distance: Cosine
      hnsw_config:
        m: 16
        ef_construct: 128

  - op: create_payload_index
    collection: products
    field_name: category
    schema: keyword

# Optional. If omitted, revector auto-inverts the `up` ops in reverse order
# and refuses the downgrade if any step is irreversible.
down:
  - op: delete_collection
    name: products
```

### Supported operations

| `op:` | Effect | Auto-reversible? |
|-------|--------|------------------|
| `create_collection` | Create a collection from a full spec | ✔ → `delete_collection` |
| `delete_collection` | Drop a collection | ✘ (data loss) |
| `update_collection` | Patch `hnsw_config`, `quantization_config`, `optimizers_config`, or per-vector params in place | ✘ (prior state unknown) |
| `create_vector` | Add a named dense vector (v1.18+) | ✔ → `delete_vector` |
| `create_sparse_vector` | Add a named sparse vector | ✔ → `delete_vector` |
| `delete_vector` | Drop a named vector | ✘ (data loss) |
| `create_payload_index` | Index a payload field | ✔ → `delete_payload_index` |
| `delete_payload_index` | Remove a payload index | ✔ iff `schema:` is given |
| `create_alias` | Point an alias at a collection | ✔ → `delete_alias` |
| `delete_alias` | Remove an alias | ✘ (target unknown) |
| `switch_alias` | Atomically repoint an alias (zero-downtime swap) | ✘ (prior target unknown) |
| `exec` | Run a shell command (the re-embedding escape hatch) | ✘ unless `down` provided |

When an operation isn't auto-reversible, supply an explicit `down:` block — then
`revector down` uses it verbatim.

## Commands

| Command | Description |
|---------|-------------|
| `revector init` | Create `migrations/` and a starter `revector.toml`. |
| `revector new <name>` | Scaffold a new migration chained onto the current head. |
| `revector status` | Show applied vs pending revisions, checksums, and reversibility. |
| `revector up [--to <rev>] [--dry-run]` | Apply pending migrations. |
| `revector down [--to <rev>] [--steps N] [--dry-run]` | Roll back migrations. |
| `revector to <rev> [--dry-run]` | Migrate to an exact revision (up or down). |
| `revector diff <collection> --spec <file.yaml>` | Compare a declared collection spec against the live collection. |

`--dry-run` prints the plan without touching Qdrant.

## Configuration

Settings are layered (highest precedence first): **CLI flags → `REVECTOR_*`
environment variables → `revector.toml` → defaults.**

```toml
# revector.toml
url = "http://localhost:6334"
migrations_dir = "migrations"
# api_key = "..."                      # or REVECTOR_API_KEY
# tracking_collection = "_revector_migrations"
```

| Setting | Env | Default |
|---------|-----|---------|
| `url` | `REVECTOR_URL` | `http://localhost:6334` |
| `api_key` | `REVECTOR_API_KEY` | _none_ |
| `migrations_dir` | `REVECTOR_MIGRATIONS_DIR` | `migrations` |
| `tracking_collection` | `REVECTOR_TRACKING_COLLECTION` | `_revector_migrations` |

Set `REVECTOR_LOG=revector=debug` for verbose logging (or pass `-v` / `-vv`).

## Drift detection (`diff`)

`revector diff` compares a declared collection spec against the live collection.
It is **declaration-driven**: only fields you actually wrote in the spec are
compared. A field you leave unset means "don't care", never "must be unset" —
this avoids the classic Alembic-autogenerate false positives caused by Qdrant
normalizing and defaulting config on read.

```sh
revector diff products --spec products.spec.yaml
# collection `products` has 1 difference(s):
#   vectors.<default>.size : declared 1024 | live 768
```

## Re-embedding (the exec-hook)

Changing a vector's `size` or `distance` is structural — Qdrant can't mutate it
in place. The path is: add a new named vector → re-embed points with your model
→ drop the old vector. The re-embedding step is the one thing a generic binary
can't own, so revector shells out to *your* command:

```yaml
up:
  - op: create_vector
    collection: products
    name: text_v2
    spec: { size: 1024, distance: Cosine }

  - op: exec
    name: re-embed with the new model
    command: "python scripts/reembed.py --collection products --target text_v2"

  - op: delete_vector          # irreversible — make this a separate, deliberate migration
    collection: products
    name: text_v1
```

The command runs via `sh -c`, inherits the environment and stdio, and a non-zero
exit aborts the migration.

## How state is tracked

Applied revisions are stored as points in the `_revector_migrations` collection
(a dummy 1-d vector plus a payload of revision id, parent, checksum, and
timestamp). Because the checksum of each migration file is recorded, revector
refuses to proceed if a migration was **edited after being applied** — catching
silent divergence between your files and the database.

## Scope & limitations

- **Linear chains only** (single base, single head) in v1 — branching/merging is
  rejected with a clear error.
- Per-vector `hnsw_config` / `quantization_config` can't be set at
  `create_vector` time (Qdrant's add-vector API doesn't accept them); apply them
  with a follow-up `update_collection` step.
- `diff` reads a standalone spec file; folding the full migration chain into a
  desired-state spec is future work.

## Development

```sh
cargo test                       # unit + logic tests
cargo clippy --all-targets       # lints
cargo fmt                        # format
```

Integration tests spin up a real Qdrant via [testcontainers] (Docker required);
they skip automatically when Docker is unavailable. To run them against an
already-running Qdrant instead:

```sh
REVECTOR_TEST_URL=http://localhost:6334 cargo test --test integration
```

[testcontainers]: https://github.com/testcontainers/testcontainers-rs

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at
your option.

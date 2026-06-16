# revector

**Declarative, versioned schema & config migrations for [Qdrant](https://qdrant.tech) — Alembic for vector collections.**

[![CI](https://github.com/diegoglozano/revector/actions/workflows/ci.yml/badge.svg)](https://github.com/diegoglozano/revector/actions/workflows/ci.yml)
[![Docs](https://github.com/diegoglozano/revector/actions/workflows/docs.yml/badge.svg)](https://diegoglozano.github.io/revector/)
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
> — is handled through an
> [exec-hook](https://diegoglozano.github.io/revector/guides/re-embedding.html).

📖 **Full docs:** <https://diegoglozano.github.io/revector/>

---

## Install

Once a release is cut, prebuilt binaries for Linux, macOS, and Windows are
attached to each [GitHub Release](https://github.com/diegoglozano/revector/releases)
by [cargo-dist], with installers:

```sh
# Shell (Linux/macOS) — downloads the right prebuilt binary
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.sh | sh

# Windows (PowerShell)
powershell -c "irm https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.ps1 | iex"
```

Or build from source (requires Rust 1.82+):

```sh
cargo install --path .          # from a checkout
cargo build --release           # ./target/release/revector
```

Homebrew and crates.io are planned — see [ROADMAP.md](ROADMAP.md).

[cargo-dist]: https://opensource.axo.dev/cargo-dist/

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
| [`create_collection`](https://diegoglozano.github.io/revector/reference/operations/create_collection.html) | Create a collection from a full spec | ✔ → `delete_collection` |
| [`delete_collection`](https://diegoglozano.github.io/revector/reference/operations/delete_collection.html) | Drop a collection | ✘ (data loss) |
| [`update_collection`](https://diegoglozano.github.io/revector/reference/operations/update_collection.html) | Patch `hnsw_config`, `quantization_config`, `optimizers_config`, or per-vector params in place | ✘ (prior state unknown) |
| [`create_vector`](https://diegoglozano.github.io/revector/reference/operations/create_vector.html) | Add a named dense vector (Qdrant v1.18+) | ✔ → `delete_vector` |
| [`create_sparse_vector`](https://diegoglozano.github.io/revector/reference/operations/create_sparse_vector.html) | Add a named sparse vector | ✔ → `delete_vector` |
| [`delete_vector`](https://diegoglozano.github.io/revector/reference/operations/delete_vector.html) | Drop a named vector | ✘ (data loss) |
| [`create_payload_index`](https://diegoglozano.github.io/revector/reference/operations/create_payload_index.html) | Index a payload field | ✔ → `delete_payload_index` |
| [`delete_payload_index`](https://diegoglozano.github.io/revector/reference/operations/delete_payload_index.html) | Remove a payload index | ✔ iff `schema:` is given |
| [`create_alias`](https://diegoglozano.github.io/revector/reference/operations/create_alias.html) | Point an alias at a collection | ✔ → `delete_alias` |
| [`delete_alias`](https://diegoglozano.github.io/revector/reference/operations/delete_alias.html) | Remove an alias | ✘ (target unknown) |
| [`switch_alias`](https://diegoglozano.github.io/revector/reference/operations/switch_alias.html) | Atomically repoint an alias (zero-downtime swap) | ✘ (prior target unknown) |
| [`exec`](https://diegoglozano.github.io/revector/reference/operations/exec.html) | Run a shell command (the re-embedding escape hatch) | ✘ unless `down` provided |

Each operation page links to a runnable example and the full list of spec
fields. The shapes referenced above (`CollectionSpec`, `VectorSpec`,
`HnswConfigSpec`, `QuantizationSpec`, …) are documented on the
[Specs](https://diegoglozano.github.io/revector/reference/specs.html) page.

## Commands

| Command | Description |
|---------|-------------|
| `revector init` | Create `migrations/` and a starter `revector.toml`. |
| `revector new <name>` | Scaffold a new migration chained onto the current head. |
| `revector status` | Show applied vs pending revisions, checksums, and reversibility. |
| `revector up [--to <rev>] [--dry-run]` | Apply pending migrations. |
| `revector down [--to <rev>] [--steps N] [--dry-run]` | Roll back migrations. |
| `revector to <rev> [--dry-run]` | Migrate to an exact revision (up or down). |
| `revector validate` | Parse all migrations and resolve the chain offline — no Qdrant connection. |
| `revector stamp <rev\|head\|base> [--dry-run]` | Mark the DB as being at a revision **without running** any ops. |
| `revector diff <collection> --spec <file.yaml>` | Compare a declared collection spec against the live collection. |

Full command, configuration, and flag reference:
<https://diegoglozano.github.io/revector/reference/commands.html>.

## More

- [Quick start](https://diegoglozano.github.io/revector/quick-start.html)
- [Migration files](https://diegoglozano.github.io/revector/migration-files.html)
- [Operations reference](https://diegoglozano.github.io/revector/reference/operations.html)
- [Specs](https://diegoglozano.github.io/revector/reference/specs.html)
- [Drift detection (`diff`)](https://diegoglozano.github.io/revector/guides/diff.html)
- [Re-embedding (the exec-hook)](https://diegoglozano.github.io/revector/guides/re-embedding.html)
- [Adopting an existing collection](https://diegoglozano.github.io/revector/guides/adopting.html)
- [How state is tracked](https://diegoglozano.github.io/revector/guides/state-tracking.html)
- [Security & supply chain](https://diegoglozano.github.io/revector/project/security.html)
- [Development](https://diegoglozano.github.io/revector/project/development.html)

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at
your option.

# revector

**Declarative, versioned schema & config migrations for [Qdrant](https://qdrant.tech) — Alembic for vector collections.**

revector brings ordered, reversible, database-tracked migrations to Qdrant —
the piece that, unlike for relational databases, didn't exist yet. You write
declarative YAML migrations, commit them next to your code, and apply or roll
them back with a single static binary. No Python venv, no external state store.

> **Schema, not data.** revector manages collection **schema and config** —
> collections, payload indexes, named vectors, aliases, and all tunable knobs.
> Moving *points* between instances is a solved problem (see
> [`qdrant/migration`](https://github.com/qdrant/migration)); that's explicitly
> out of scope. The one data operation revector *does* help with — re-embedding
> — is handled through an [exec-hook](./guides/re-embedding.md).

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

## Where to next

- New here? → [Install](./install.md) → [Quick start](./quick-start.md).
- Writing a migration? → [Migration files](./migration-files.md) and the
  [Operations reference](./reference/operations.md).
- Want to know every knob? → [Specs](./reference/specs.md).
- Existing collection? → [Adopting an existing collection](./guides/adopting.md).

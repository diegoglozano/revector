# CLAUDE.md

Guidance for AI assistants working in this repository.

## What this is

**revector** is a single static Rust binary that brings declarative, versioned,
reversible schema & config migrations to [Qdrant](https://qdrant.tech) — "Alembic
for vector collections." Users author YAML migrations, commit them next to their
code, and apply/roll them back with the CLI. Applied state is tracked **inside
Qdrant itself** (a dedicated collection), so there is no external state store.

**Scope is schema/config, not data.** revector manages collections, payload
indexes, named/sparse vectors, aliases, and tunable configs (hnsw, quantization,
optimizers). Moving *points* between instances is explicitly out of scope (that's
`qdrant/migration`). The one data operation it helps with — re-embedding — is
handled by shelling out via the `exec` op (the escape hatch).

## Repository layout

```
src/                  Library + thin CLI binary (all logic is in the lib)
  lib.rs              Crate root + module map (read this first)
  main.rs             CLI entry point — thin shell: parse flags → config → connect → call lib
  cli.rs              clap derive command/flag definitions
  config.rs           Layered config loading (figment): defaults < revector.toml < REVECTOR_* env < CLI flags
  spec.rs             Declarative YAML vocabulary (CollectionSpec, VectorSpec, HnswConfigSpec, …) — decoupled from qdrant-client
  ops.rs              Operation enum (the migration "verbs") + reversibility logic. Holds DATA, not execution
  migration.rs        MigrationFile parsing, discovery, SHA-256 checksums, downgrade-op resolution
  chain.rs            Resolve discovered migrations into an ordered LINEAR chain (Alembic-style revision/down_revision)
  convert.rs          spec → qdrant-client type translation
  executor.rs         Idempotent execution of a single Operation against live Qdrant
  tracking.rs         Applied-revision bookkeeping + advisory lock, stored as points in the tracking collection
  runner.rs           Orchestration: up/down/to/status/stamp, checksum safety, lock acquisition
  diff.rs             Declaration-driven drift detection (diff command)
  client.rs           Thin Qdrant client wrapper applying config
  exec_hook.rs        Runs `exec` op shell commands
  error.rs            Single Error enum (thiserror) so callers match on failure categories
  scaffold.rs         `init` and `new` file scaffolding
tests/
  logic.rs            Pure unit tests (chain resolution, reversibility, checksums, spec parsing) — no Qdrant
  integration.rs      Live end-to-end tests via testcontainers (real Qdrant; skipped if Docker unavailable)
examples/migrations/  Reference migration YAML files
docs/                 mdBook source (docs/src/), published to GitHub Pages
skills/revector/      Agent Skill (SKILL.md) describing when/how to use revector
.github/workflows/    ci.yml, security.yml, release.yml, publish-crate.yml, docs.yml
```

## Architecture & data flow

The binary is intentionally a thin shell; **all real logic lives in `revector::*`**
so it's testable without a process boundary. A typical `up` flow:

`main.rs` parses CLI → `Config::load` resolves config → `client::connect` →
`migration::discover` reads `*.yaml` from `migrations_dir` → `Chain::resolve`
validates/orders them → `Runner` acquires the advisory lock, verifies checksums of
already-applied revisions, then drives `Executor` per operation and records each
applied revision via `Tracker`.

Key design boundaries to respect when editing:

- **`spec.rs` is decoupled from `qdrant-client` on purpose.** Users author specs in
  YAML; `convert.rs` is the only place that translates them to client types. Keeping
  the file format independent means a `qdrant-client` upgrade can't silently change
  the meaning of a committed migration. Don't leak client types into `spec.rs`/`ops.rs`.
- **`ops.rs` holds data + reversibility; `executor.rs` holds execution.** Adding a new
  operation means: add the variant to `Operation` (internally tagged via `#[serde(tag = "op")]`),
  add its `describe()` + `auto_inverse()` arms, then handle it in `Executor::execute`.
- **Operations must be idempotent.** Qdrant has no transactional DDL, so a half-applied
  migration must be safe to re-run. The executor checks existence before create/delete
  and skips no-ops. Preserve this when adding ops.
- **Reversibility is explicit.** `auto_inverse` returns `Reversibility::Auto(_)` or
  `Irreversible(reason)`. Anything destructive or needing unrecorded prior state is
  irreversible; the user must supply an explicit `down` block. `is_reversible()` drives
  status output and pre-flight failure.

## Migration file format

YAML with a `revision` id, a `down_revision` parent link (`null` = base), and `up` /
optional `down` operation lists. Each op names itself with an `op:` key (snake_case).
Chains are **linear only** (single base, single head, no branches). See
`examples/migrations/` and `docs/src/migration-files.md`.

```yaml
revision: "0002_index"
down_revision: "0001_create"   # null marks the base
description: index category
up:
  - op: create_payload_index
    collection: products
    field_name: category
    schema: keyword
# Omit `down` to auto-invert the up ops in reverse; revector refuses the
# downgrade if any step is irreversible (e.g. update_collection, delete_*).
```

Operations live in `ops.rs`; specs in `spec.rs`. State is tracked as points in the
`_revector_migrations` collection (configurable). Editing an applied migration is
caught by a SHA-256 checksum guard — add a new revision instead.

## CLI commands

`init`, `new <name>`, `status`, `up [--to/--dry-run]`, `down [--to/--steps/--dry-run]`,
`to <rev>`, `validate` (offline, no DB), `stamp <rev|head|base>` (adopt existing state),
`diff <collection> --spec <file>` (drift detection). Global flags: `--config`, `--url`,
`--api-key`, `--migrations-dir`, `-v/-vv`, `-y/--yes`, `--force`.

## Development workflow

```sh
cargo test                                  # unit + integration (integration needs Docker, else auto-skips)
cargo test --test logic                     # pure logic tests only, no Qdrant
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all                             # format (CI runs --check)
cargo build --release                       # ./target/release/revector
```

Run integration tests against an already-running Qdrant instead of testcontainers:

```sh
REVECTOR_TEST_URL=http://localhost:6334 cargo test --test integration
```

CI (`.github/workflows/ci.yml`) gates every PR on `cargo fmt --check`, `clippy -D warnings`,
and the full test suite (Docker is available on the runner, so integration tests run for real).
Treat clippy warnings as errors locally — CI will. There is also `security.yml` (cargo-deny:
advisories/licenses/bans/sources, see `deny.toml`).

### Docs

The site is mdBook (`docs/src/`), published to GitHub Pages on push to `main`:

```sh
mdbook serve docs --open   # live preview; docs/book/ is gitignored
```

When you add/change an operation, config key, or command, update both the relevant
`docs/src/` page and `README.md`. Keep `CHANGELOG.md` current (Keep a Changelog format,
SemVer). Releases go through `cargo-dist` (`dist-workspace.toml`) — binaries + installers.

## Conventions

- **Edition 2021, MSRV 1.82.** Don't use features newer than that without bumping `rust-version`.
- **Errors:** one `Error` enum in `error.rs` (thiserror). Add a variant rather than stringly-typed
  errors when callers might want to match. `Result<T>` alias is crate-wide.
- **Module docs matter.** Every `src/*.rs` opens with a `//!` doc comment explaining its role and
  invariants — match that style and keep them accurate when behavior changes.
- **Logging** via `tracing` (`info!`/`warn!`/`debug!`), controlled by `-v`/`REVECTOR_LOG`.
- **Async** via tokio; the binary is `#[tokio::main]`.
- Match the surrounding code's comment density and naming. Comments explain *why* (invariants,
  trade-offs), not *what*.

## Git / PR workflow for this environment

- Develop on the designated feature branch; create it locally if missing. Do **not** push to `main`.
- Push with `git push -u origin <branch>`; retry network failures with exponential backoff.
- Commit only when asked; write clear, descriptive messages.
- **Do not open a PR unless explicitly asked.**
- Do not reference model identifiers in commits, code, or any pushed artifact.

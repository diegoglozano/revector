# Changelog

All notable changes to revector are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and the project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Docs: a CI/CD integration guide covering offline `validate` on pull requests,
  `up` on deploy, drift checks via `diff`, and non-interactive usage, with
  GitHub Actions and GitLab CI examples.

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

[0.2.0]: https://github.com/diegoglozano/revector/releases/tag/v0.2.0
[0.1.0]: https://github.com/diegoglozano/revector/releases/tag/v0.1.0

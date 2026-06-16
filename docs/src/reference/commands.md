# Commands

| Command | Description |
|---------|-------------|
| `revector init` | Create `migrations/` and a starter `revector.toml`. |
| `revector new <name>` | Scaffold a new migration chained onto the current head. |
| `revector status` | Show applied vs pending revisions, checksums, and reversibility. |
| `revector up [--to <rev>] [--dry-run]` | Apply pending migrations. |
| `revector down [--to <rev>] [--steps N] [--dry-run]` | Roll back migrations (default: 1 step). |
| `revector to <rev> [--dry-run]` | Migrate to an exact revision (up or down). |
| `revector validate` | Parse all migrations and resolve the chain offline — no Qdrant connection. Good as a CI / pre-commit check. |
| `revector stamp <rev\|head\|base> [--dry-run]` | Mark the DB as being at a revision **without running** any ops — for adopting an existing collection (Alembic's `stamp`). |
| `revector diff <collection> --spec <file.yaml>` | Compare a declared collection spec against the live collection. |

`--dry-run` prints the plan without touching Qdrant.

## Global flags

These flags work on every subcommand:

| Flag | Env | Description |
|------|-----|-------------|
| `--config <FILE>` |   | Path to a `revector.toml` (default: `./revector.toml`). |
| `--url <URL>` | `REVECTOR_URL` | Qdrant gRPC URL. |
| `--api-key <KEY>` | `REVECTOR_API_KEY` | Qdrant API key. |
| `--migrations-dir <DIR>` |   | Migrations directory. |
| `-v`, `-vv` |   | Increase log verbosity (debug, trace). |
| `-y`, `--yes` |   | Skip confirmation prompts (required in non-interactive shells for rollbacks). |
| `--force` |   | Override a held or stale migration lock. |

Set `REVECTOR_LOG=revector=debug` for verbose logging (equivalent to `-v`).

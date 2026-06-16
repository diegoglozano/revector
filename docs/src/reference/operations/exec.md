# `exec`

Run a shell command as a migration step — the escape hatch for steps a generic
binary can't own (most often re-embedding). See the
[Re-embedding guide](../../guides/re-embedding.md) for the canonical pattern.

## Example

```yaml
up:
  - op: exec
    name: re-embed with the new model
    command: "python scripts/reembed.py --collection products --target text_v2"
    workdir: ./scripts

# `exec` has no automatic inverse. Spell out a compensating command if one
# exists, or omit `down` and revector will refuse the downgrade.
down:
  - op: exec
    name: restore embeddings from snapshot
    command: "python scripts/restore_text_v1.py"
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `command` | string | yes | Command line, executed via `sh -c`. Inherits environment and stdio. |
| `name` | string | no | Human-readable label for status / log output. |
| `workdir` | string | no | Working directory. Defaults to the project root. |

A non-zero exit aborts the migration.

## Reversibility

**Not auto-reversible.** Provide an explicit `down:` block if a compensating
command exists; otherwise revector refuses the downgrade.

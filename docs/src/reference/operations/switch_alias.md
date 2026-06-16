# `switch_alias`

Atomically repoint an alias to a different collection — the zero-downtime swap
used to roll out a rebuilt collection.

## Example

```yaml
up:
  - op: switch_alias
    alias: products
    to_collection: products_v2

# Spell out the inverse — revector does not record the previous target.
down:
  - op: switch_alias
    alias: products
    to_collection: products_v1
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `alias` | string | yes | Alias to repoint. |
| `to_collection` | string | yes | New target collection. |

## Reversibility

**Not auto-reversible** — the previous target isn't stored. Supply an explicit
`down:` that switches the alias back to the prior collection.

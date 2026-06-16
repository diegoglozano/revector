# `delete_alias`

Remove an alias. The target collection stays in place.

## Example

```yaml
up:
  - op: delete_alias
    alias: products_v1_legacy

# Spell out the inverse — revector does not record the previous target.
down:
  - op: create_alias
    alias: products_v1_legacy
    collection: products
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `alias` | string | yes | Alias name to remove. |

## Reversibility

**Not auto-reversible** — the previous target isn't recorded anywhere, so
revector can't know which collection to re-point at on downgrade. Supply an
explicit `down:` that calls [`create_alias`](./create_alias.md).

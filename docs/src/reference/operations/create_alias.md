# `create_alias`

Point an alias at a collection. Aliases are how you keep callers stable while
rebuilding collections underneath.

## Example

```yaml
up:
  - op: create_alias
    collection: products_v2
    alias: products
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `collection` | string | yes | Collection the alias should resolve to. |
| `alias` | string | yes | Alias name. |

## Reversibility

Auto-reversible → [`delete_alias`](./delete_alias.md).

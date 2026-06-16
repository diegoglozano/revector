# `delete_vector`

Drop a named vector (dense or sparse) from a collection.

## Example

```yaml
up:
  - op: delete_vector
    collection: products
    name: text_v1

# delete_vector is irreversible — declare an explicit `down` if rollback matters.
down:
  - op: create_vector
    collection: products
    name: text_v1
    spec:
      size: 768
      distance: Cosine
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `collection` | string | yes | Collection that holds the vector. |
| `name` | string | yes | Vector name to drop. |

## Reversibility

**Irreversible.** Dropping a vector destroys its embeddings; revector cannot
restore them. Supply an explicit `down:` to recreate the vector definition
(though the embeddings themselves will need to be regenerated — typically via an
[`exec`](./exec.md) step).

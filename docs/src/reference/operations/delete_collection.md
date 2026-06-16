# `delete_collection`

Drop a collection entirely.

## Example

```yaml
up:
  - op: delete_collection
    name: legacy_products

# delete_collection is irreversible — declare an explicit `down`
# (typically a full create_collection) if you need to be able to roll back.
down:
  - op: create_collection
    name: legacy_products
    spec:
      vectors:
        "":
          size: 768
          distance: Cosine
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Collection to drop. |

## Reversibility

**Irreversible.** Dropping a collection destroys all its points; revector
cannot reconstruct them. `revector down` refuses unless you supply an explicit
`down:` block — typically a [`create_collection`](./create_collection.md) op
matching the original spec.

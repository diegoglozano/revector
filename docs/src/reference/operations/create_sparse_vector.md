# `create_sparse_vector`

Add a named sparse vector to an existing collection.

## Example

```yaml
up:
  - op: create_sparse_vector
    collection: products
    name: keywords
    spec:
      on_disk: true
      full_scan_threshold: 5000
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `collection` | string | yes | Collection to add the vector to. |
| `name` | string | yes | Sparse vector name (must be unique within the collection). |
| `spec` | [`SparseVectorSpec`](../specs.md#sparsevectorspec) | yes | Sparse-vector configuration. |

### `spec` fields

| Field | Type | Description |
|-------|------|-------------|
| `on_disk` | bool | Store the sparse index on disk. |
| `full_scan_threshold` | uint | Postings-list size below which Qdrant performs a full scan instead of using the index. |

## Reversibility

Auto-reversible → [`delete_vector`](./delete_vector.md). The downgrade drops
any sparse vectors stored under this name.

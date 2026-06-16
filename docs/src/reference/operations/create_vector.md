# `create_vector`

Add a new named dense vector to an existing collection. Requires Qdrant v1.18+.

## Example

```yaml
up:
  - op: create_vector
    collection: products
    name: image
    spec:
      size: 512
      distance: Dot
      on_disk: true
      datatype: float16
```

To tune the new vector's `hnsw_config` or `quantization_config`, follow it with
an [`update_collection`](./update_collection.md) step — Qdrant's add-vector API
doesn't accept those at create time.

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `collection` | string | yes | Collection to add the vector to. |
| `name` | string | yes | Vector name (must be unique within the collection). |
| `spec` | [`VectorSpec`](../specs.md#vectorspec) | yes | Vector configuration. |

### `spec` fields

| Field | Type | Description |
|-------|------|-------------|
| `size` | uint | Dimensionality. Immutable once created. |
| `distance` | `Cosine` \| `Euclid` \| `Dot` \| `Manhattan` | Distance metric. Immutable. |
| `on_disk` | bool | Store vectors on disk rather than in RAM. |
| `datatype` | `float32` \| `uint8` \| `float16` | Element storage type. |
| `hnsw_config` | [`HnswConfigSpec`](../specs.md#hnswconfigspec) | Ignored at create time — apply via `update_collection`. |
| `quantization_config` | [`QuantizationSpec`](../specs.md#quantizationspec) | Ignored at create time — apply via `update_collection`. |

## Reversibility

Auto-reversible → [`delete_vector`](./delete_vector.md). Note the downgrade
discards any embeddings written to the vector in the meantime — a deliberate,
declared choice.

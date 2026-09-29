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
      datatype: float16
```

To tune the new vector's `hnsw_config`, `quantization_config` or `memory`
placement, follow it with an [`update_collection`](./update_collection.md)
step — Qdrant's add-vector API doesn't accept those at create time. revector
warns when a `create_vector` spec declares one of them.

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
| `on_disk` | bool | Ignored at create time — apply via `update_collection`. |
| `memory` | [`Memory`](../specs.md#memory) | Ignored at create time — apply via `update_collection`. |
| `datatype` | `float32` \| `uint8` \| `float16` \| `turbo4` | Element storage type (`turbo4` needs Qdrant 1.19+). |
| `hnsw_config` | [`HnswConfigSpec`](../specs.md#hnswconfigspec) | Ignored at create time — apply via `update_collection`. |
| `quantization_config` | [`QuantizationSpec`](../specs.md#quantizationspec) | Ignored at create time — apply via `update_collection`. |

## Reversibility

Auto-reversible → [`delete_vector`](./delete_vector.md). Note the downgrade
discards any embeddings written to the vector in the meantime — a deliberate,
declared choice.

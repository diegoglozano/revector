# `create_collection`

Create a new collection from a full [`CollectionSpec`](../specs.md#collectionspec).

## Example

```yaml
up:
  - op: create_collection
    name: products
    spec:
      vectors:
        "":                          # "" is the unnamed/default vector
          size: 768
          distance: Cosine
        image:                       # multiple named vectors
          size: 512
          distance: Dot
          on_disk: true
      sparse_vectors:
        keywords:
          on_disk: false
      hnsw_config:
        m: 16
        ef_construct: 128
      optimizers_config:
        default_segment_number: 2
      shard_number: 2
      replication_factor: 2
      on_disk_payload: true
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Collection name. |
| `spec` | [`CollectionSpec`](../specs.md#collectionspec) | yes | Full collection specification. |

### `spec` fields

| Field | Type | Description |
|-------|------|-------------|
| `vectors` | map<name, [`VectorSpec`](../specs.md#vectorspec)> | Named dense vectors. Use `""` as the key for the unnamed/default vector. |
| `sparse_vectors` | map<name, [`SparseVectorSpec`](../specs.md#sparsevectorspec)> | Named sparse vectors. |
| `hnsw_config` | [`HnswConfigSpec`](../specs.md#hnswconfigspec) | Collection-level HNSW defaults. |
| `quantization_config` | [`QuantizationSpec`](../specs.md#quantizationspec) | Collection-level quantization defaults. |
| `optimizers_config` | [`OptimizersConfigSpec`](../specs.md#optimizersconfigspec) | Optimizer thresholds. |
| `shard_number` | uint | Number of shards (immutable on single-node). |
| `replication_factor` | uint | Replication factor. |
| `write_consistency_factor` | uint | Write consistency factor. |
| `on_disk_payload` | bool | Store the whole collection payload on disk. |

## Reversibility

Auto-reversible → [`delete_collection`](./delete_collection.md). Dropping the
new collection destroys whatever was written into it in the interim, so the
downgrade is destructive but unambiguous.

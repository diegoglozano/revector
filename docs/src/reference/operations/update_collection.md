# `update_collection`

Patch tunable collection-level configuration in place. Only the fields you set
are sent to Qdrant — unset fields are left untouched.

## Example

```yaml
up:
  - op: update_collection
    collection: products
    hnsw_config:
      ef_construct: 256
    quantization_config:
      scalar:
        type: int8
        quantile: 0.99
        always_ram: true
    optimizers_config:
      indexing_threshold: 20000
    vectors:                       # patch existing named-vector params
      image:
        on_disk: true

# update_collection is not auto-reversible (previous values aren't recorded).
# Spell out the inverse explicitly:
down:
  - op: update_collection
    collection: products
    quantization_config: disabled
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `collection` | string | yes | Collection to update. |
| `hnsw_config` | [`HnswConfigSpec`](../specs.md#hnswconfigspec) | no | Override collection-level HNSW params. |
| `quantization_config` | [`QuantizationSpec`](../specs.md#quantizationspec) | no | Set or replace quantization (`scalar` / `product` / `binary` / `disabled`). |
| `optimizers_config` | [`OptimizersConfigSpec`](../specs.md#optimizersconfigspec) | no | Tune optimizer thresholds. |
| `vectors` | map<name, [`VectorParamsDiff`](../specs.md#vectorparamsdiff)> | no | Patch params of existing named vectors (`on_disk`, `hnsw_config`, `quantization_config`). |

At least one of those four fields must be set, otherwise revector refuses the
op as a no-op.

> **Note.** `size` and `distance` of a named vector are immutable. Per-vector
> `hnsw_config` / `quantization_config` cannot be set at `create_vector` time
> either (Qdrant's add-vector API doesn't accept them); apply them with a
> follow-up `update_collection` step as shown above.

## Reversibility

**Not auto-reversible.** The previous values aren't stored, so revector cannot
synthesise an inverse. Provide an explicit `down:` block:

- To restore prior numeric values, repeat `update_collection` with the original
  numbers.
- To turn quantization back off, use `quantization_config: disabled`.

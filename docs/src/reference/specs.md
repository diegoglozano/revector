# Specs

The shapes referenced by operations. These types are the on-disk vocabulary —
they are deliberately decoupled from `qdrant-client`, so a client-crate upgrade
can't silently change the meaning of a committed migration.

## `CollectionSpec`

The full specification of a collection. Used by
[`create_collection`](./operations/create_collection.md) and as the desired
state passed to `revector diff`.

| Field | Type | Description |
|-------|------|-------------|
| `vectors` | map<name, [`VectorSpec`](#vectorspec)> | Named dense vectors. Use `""` as the key for the unnamed/default vector. |
| `sparse_vectors` | map<name, [`SparseVectorSpec`](#sparsevectorspec)> | Named sparse vectors. |
| `hnsw_config` | [`HnswConfigSpec`](#hnswconfigspec) | Collection-level HNSW defaults. |
| `quantization_config` | [`QuantizationSpec`](#quantizationspec) | Collection-level quantization defaults. |
| `optimizers_config` | [`OptimizersConfigSpec`](#optimizersconfigspec) | Optimizer thresholds. |
| `shard_number` | uint | Number of shards (immutable after create on single-node). |
| `replication_factor` | uint | Replication factor. |
| `write_consistency_factor` | uint | Write consistency factor. |
| `on_disk_payload` | bool | Store the whole collection payload on disk. |

## `VectorSpec`

Configuration of a single (dense) named vector.

| Field | Type | Description |
|-------|------|-------------|
| `size` | uint | Dimensionality. Immutable once created. |
| `distance` | [`Distance`](#distance) | Distance metric. Immutable in place. |
| `on_disk` | bool | Store vectors on disk rather than in RAM. |
| `hnsw_config` | [`HnswConfigSpec`](#hnswconfigspec) | Per-vector HNSW overrides. Ignored at `create_vector` time — apply via `update_collection`. |
| `quantization_config` | [`QuantizationSpec`](#quantizationspec) | Per-vector quantization overrides. Ignored at `create_vector` time — apply via `update_collection`. |
| `datatype` | [`Datatype`](#datatype) | Element storage type. |

## `SparseVectorSpec`

Configuration of a single named sparse vector.

| Field | Type | Description |
|-------|------|-------------|
| `on_disk` | bool | Store the sparse index on disk. |
| `full_scan_threshold` | uint | Postings-list size below which Qdrant performs a full scan instead of using the index. |

## `VectorParamsDiff`

Used inside [`update_collection.vectors`](./operations/update_collection.md) to
patch the in-place tunables of an *existing* named vector. `size` and
`distance` are deliberately excluded — they are immutable.

| Field | Type | Description |
|-------|------|-------------|
| `on_disk` | bool | Move the vector on / off disk. |
| `hnsw_config` | [`HnswConfigSpec`](#hnswconfigspec) | Per-vector HNSW params. |
| `quantization_config` | [`QuantizationSpec`](#quantizationspec) | Per-vector quantization. |

## `HnswConfigSpec`

HNSW index parameters. Only fields you set are sent — unset means "leave alone".

| Field | Type | Description |
|-------|------|-------------|
| `m` | uint | Number of edges per node in the index graph. |
| `ef_construct` | uint | Size of the dynamic candidate list during construction. |
| `full_scan_threshold` | uint | Vector count below which Qdrant uses a full scan instead of the index. |
| `max_indexing_threads` | uint | Maximum threads to use when building the index. |
| `on_disk` | bool | Store the HNSW graph on disk. |
| `payload_m` | uint | `m` value for the dedicated payload-filtered graph. |

## `QuantizationSpec`

Tagged union — set exactly one variant. Use `disabled` inside
`update_collection` to turn quantization off.

```yaml
# scalar
quantization_config:
  scalar:
    type: int8
    quantile: 0.99
    always_ram: true

# product
quantization_config:
  product:
    compression: x8
    always_ram: true

# binary
quantization_config:
  binary:
    always_ram: true

# disable (update_collection only)
quantization_config: disabled
```

### `ScalarQuantizationSpec`

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Quantization type. Only `int8` exists today. Default: `int8`. |
| `quantile` | float | Quantile used to clip outliers when computing the scale. |
| `always_ram` | bool | Keep quantized vectors in RAM. |

### `ProductQuantizationSpec`

| Field | Type | Description |
|-------|------|-------------|
| `compression` | string | Compression ratio — one of `x4`, `x8`, `x16`, `x32`, `x64`. |
| `always_ram` | bool | Keep quantized vectors in RAM. |

### `BinaryQuantizationSpec`

| Field | Type | Description |
|-------|------|-------------|
| `always_ram` | bool | Keep quantized vectors in RAM. |

## `OptimizersConfigSpec`

Optimizer thresholds and behavior. All fields optional.

| Field | Type | Description |
|-------|------|-------------|
| `deleted_threshold` | float | Fraction of deleted points that triggers segment vacuum. |
| `vacuum_min_vector_number` | uint | Minimum vectors per segment before vacuum is considered. |
| `default_segment_number` | uint | Target number of segments. |
| `max_segment_size` | uint | Maximum segment size in KB. |
| `memmap_threshold` | uint | Segment size in KB above which Qdrant memory-maps it. |
| `indexing_threshold` | uint | Vector count above which a segment becomes indexed. |
| `flush_interval_sec` | uint | Interval (seconds) between automatic flushes. |

## `Distance`

`Cosine` · `Euclid` · `Dot` · `Manhattan`

## `Datatype`

`float32` · `uint8` · `float16`

## `PayloadSchemaType`

`keyword` · `integer` · `float` · `geo` · `text` · `bool` · `datetime` · `uuid`

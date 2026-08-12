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
| `on_disk_payload` | bool | Store the whole collection payload on disk. Superseded by `payload.memory`. |
| `payload` | [`PayloadStorageSpec`](#payloadstoragespec) | Payload storage configuration (Qdrant 1.19+). |

## `VectorSpec`

Configuration of a single (dense) named vector.

| Field | Type | Description |
|-------|------|-------------|
| `size` | uint | Dimensionality. Immutable once created. |
| `distance` | [`Distance`](#distance) | Distance metric. Immutable in place. |
| `on_disk` | bool | Store vectors on disk rather than in RAM. Superseded by `memory`. |
| `memory` | [`Memory`](#memory) | Memory placement of the vector storage (Qdrant 1.19+). `pinned` is not supported for dense vectors. Ignored at `create_vector` time — apply via `update_collection`. |
| `hnsw_config` | [`HnswConfigSpec`](#hnswconfigspec) | Per-vector HNSW overrides. Ignored at `create_vector` time — apply via `update_collection`. |
| `quantization_config` | [`QuantizationSpec`](#quantizationspec) | Per-vector quantization overrides. Ignored at `create_vector` time — apply via `update_collection`. |
| `datatype` | [`Datatype`](#datatype) | Element storage type. |

## `SparseVectorSpec`

Configuration of a single named sparse vector.

| Field | Type | Description |
|-------|------|-------------|
| `on_disk` | bool | Store the sparse index on disk. Superseded by `memory`. |
| `full_scan_threshold` | uint | Postings-list size below which Qdrant performs a full scan instead of using the index. |
| `memory` | [`Memory`](#memory) | Memory placement of the inverted index (Qdrant 1.19+). |

## `VectorParamsDiff`

Used inside [`update_collection.vectors`](./operations/update_collection.md) to
patch the in-place tunables of an *existing* named vector. `size` and
`distance` are deliberately excluded — they are immutable.

| Field | Type | Description |
|-------|------|-------------|
| `on_disk` | bool | Move the vector on / off disk. Superseded by `memory`. |
| `memory` | [`Memory`](#memory) | Move the vector storage between memory tiers (Qdrant 1.19+). |
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
| `on_disk` | bool | Store the HNSW graph on disk. Superseded by `memory`. |
| `payload_m` | uint | `m` value for the dedicated payload-filtered graph. |
| `memory` | [`Memory`](#memory) | Memory placement of the HNSW graph (Qdrant 1.19+). |

## `QuantizationSpec`

Tagged union — set exactly one variant. Use `disabled` inside
`update_collection` to turn quantization off.

```yaml
# scalar
quantization_config:
  scalar:
    type: int8
    quantile: 0.99
    memory: pinned      # or the older `always_ram: true`

# product
quantization_config:
  product:
    compression: x8
    always_ram: true

# binary
quantization_config:
  binary:
    always_ram: true

# turboquant
quantization_config:
  turboquant:
    bits: "1.5"
    always_ram: true

# disable (update_collection only)
quantization_config: disabled
```

### `ScalarQuantizationSpec`

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Quantization type. Only `int8` exists today. Default: `int8`. |
| `quantile` | float | Quantile used to clip outliers when computing the scale. |
| `always_ram` | bool | Keep quantized vectors in RAM. Superseded by `memory`. |
| `memory` | [`Memory`](#memory) | Memory placement of the quantized vectors (Qdrant 1.19+). |

### `ProductQuantizationSpec`

| Field | Type | Description |
|-------|------|-------------|
| `compression` | string | Compression ratio — one of `x4`, `x8`, `x16`, `x32`, `x64`. |
| `always_ram` | bool | Keep quantized vectors in RAM. Superseded by `memory`. |
| `memory` | [`Memory`](#memory) | Memory placement of the quantized vectors (Qdrant 1.19+). |

### `BinaryQuantizationSpec`

| Field | Type | Description |
|-------|------|-------------|
| `always_ram` | bool | Keep quantized vectors in RAM. Superseded by `memory`. |
| `memory` | [`Memory`](#memory) | Memory placement of the quantized vectors (Qdrant 1.19+). |

### `TurboquantQuantizationSpec`

TurboQuant is Qdrant's fast, data-oblivious quantization (Qdrant 1.18+). It
encodes each component down to a handful of bits without a training pass.

| Field | Type | Description |
|-------|------|-------------|
| `bits` | string | Bits per component — one of `1`, `1.5`, `2`, `4`. Omitted → server default. |
| `always_ram` | bool | Keep quantized vectors in RAM. Superseded by `memory`. |
| `memory` | [`Memory`](#memory) | Memory placement of the quantized vectors (Qdrant 1.19+). |

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

`float32` · `uint8` · `float16` · `turbo4`

`turbo4` stores the primary vectors with TurboQuant 4-bit encoding and requires
Qdrant 1.19+.

## `Memory`

`cold` · `cached` · `pinned` — where a component's data is held in RAM
(Qdrant 1.19+).

| Value | Meaning |
|-------|---------|
| `cold` | Not pre-loaded; cached lazily as it is used. |
| `cached` | Pre-loaded into the disk cache on start, evictable under memory pressure. |
| `pinned` | Held in RAM and never evicted. |

Data is always persisted on disk regardless of this setting — `memory` only
controls caching. It supersedes the older `on_disk` / `always_ram` /
`on_disk_payload` booleans, which Qdrant still accepts; when both are set on a
component, `memory` wins. revector sends whichever one a migration declared, so
a file written against Qdrant 1.18 keeps its exact meaning.

Qdrant rejects `pinned` for dense vector storage and for the payload store.

`memory` needs a Qdrant 1.19 server. Declaring it against an older one would be
a *silent* no-op — gRPC drops fields a server predates rather than rejecting
them — so `up` and `down` check the live server version first and refuse the
run, naming the field and revision. The same guard covers the collection
`payload:` block, `params.prefix`, `params.stemmer: disabled`, and
`datatype: turbo4` (which needs 1.18.2).

## `PayloadStorageSpec`

Payload storage configuration (Qdrant 1.19+) — the successor to
`on_disk_payload`.

| Field | Type | Description |
|-------|------|-------------|
| `memory` | [`Memory`](#memory) | Memory placement of the payload store. `pinned` is not supported. |

```yaml
payload:
  memory: cold
```

## `PayloadSchemaType`

`keyword` · `integer` · `float` · `geo` · `text` · `bool` · `datetime` · `uuid`

## `PayloadIndexParamsSpec`

Tuning knobs for a payload index, set via
[`create_payload_index.params`](./operations/create_payload_index.md). Qdrant
models these as a per-field-type union; since the field type is already named by
`schema:`, revector keeps the YAML flat and validates applicability instead —
setting a field the chosen type doesn't accept fails `revector validate`
offline rather than being silently dropped.

| Field | Type | Applies to | Description |
|-------|------|-----------|-------------|
| `on_disk` | bool | all | Store the index on disk. Superseded by `memory`. |
| `memory` | [`Memory`](#memory) | all | Memory placement of the index (Qdrant 1.19+). |
| `enable_hnsw` | bool | all | Build payload-aware HNSW links for this field (needs `payload_m > 0`). |
| `is_tenant` | bool | `keyword`, `uuid` | Optimize storage for multitenancy on this field. |
| `is_principal` | bool | `integer`, `float`, `datetime` | Organize collection storage by this field. |
| `prefix` | bool | `keyword` | Enable `match: { prefix: … }` filtering (Qdrant 1.19+). |
| `lookup` | bool | `integer` | Support direct lookups. |
| `range` | bool | `integer` | Support range filters. |
| `tokenizer` | `prefix`·`whitespace`·`word`·`multilingual` | `text` | Tokenizer. Required when a `text` index declares `params`. |
| `lowercase` | bool | `text` | Lowercase all tokens. |
| `min_token_len` / `max_token_len` | uint | `text` | Token length bounds. |
| `phrase_matching` | bool | `text` | Support phrase matching. |
| `ascii_folding` | bool | `text` | Fold accented characters to ASCII. |
| `stopwords` | [`StopwordsSpec`](#stopwordsspec) | `text` | Stopwords to drop when tokenizing. |
| `stemmer` | [`StemmerSpec`](#stemmerspec) | `text` | Stemming algorithm. |

### `StopwordsSpec`

| Field | Type | Description |
|-------|------|-------------|
| `languages` | list<string> | Built-in stopword lists to apply, by language name. |
| `custom` | list<string> | Extra stopwords on top of the built-in lists. |

### `StemmerSpec`

```yaml
stemmer: { snowball: english }   # Snowball stemmer for a language
stemmer: disabled                # opt out of the language default (Qdrant 1.19+)
```

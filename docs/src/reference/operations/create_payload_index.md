# `create_payload_index`

Create an index on a payload field for efficient filtering or BM25 text ranking.

## Example

```yaml
up:
  - op: create_payload_index
    collection: products
    field_name: category
    schema: keyword

  - op: create_payload_index
    collection: products
    field_name: price
    schema: float

  - op: create_payload_index
    collection: products
    field_name: location
    schema: geo

  # Tuning knobs are optional; omitted → server defaults.
  - op: create_payload_index
    collection: products
    field_name: sku
    schema: keyword
    params:
      prefix: true            # enables `match: { prefix: … }` (Qdrant 1.19+)
      is_tenant: true
      memory: cached          # index placement (Qdrant 1.19+)

  - op: create_payload_index
    collection: products
    field_name: description
    schema: text
    params:
      tokenizer: word         # required when a text index declares params
      lowercase: true
      stemmer: { snowball: english }

  # Qdrant 1.20+: BM25 ranking without sparse embeddings.
  - op: create_payload_index
    collection: products
    field_name: source_code
    schema: text
    params:
      tokenizer: code
      scoring: { type: bm25 }

  # Qdrant 1.20+: index each array element for nested filters.
  - op: create_payload_index
    collection: products
    field_name: offers[].price
    schema: integer
    params:
      nested: offers
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `collection` | string | yes | Collection that holds the field. |
| `field_name` | string | yes | Payload field to index. |
| `schema` | [`PayloadSchemaType`](../specs.md#payloadschematype) | yes | Field type: `keyword`, `integer`, `float`, `geo`, `text`, `bool`, `datetime`, `uuid`. |
| `params` | [`PayloadIndexParamsSpec`](../specs.md#payloadindexparamsspec) | no | Index tuning knobs (tenancy, prefix matching, memory placement, text analysis, …). |

Params are checked against `schema:` when the migration file is parsed: setting
one the field type doesn't accept — `prefix` on an `integer` field, say — fails
`revector validate` offline, rather than being quietly dropped on the way to
the server.

Creating an index is idempotent: an existing field index is skipped. To enable
scoring or change the tokenizer on an existing index, add a new revision that
first deletes the index and then recreates it with the new params. Supply the
old schema and params on the delete so automatic rollback restores them.

Applications can then query a BM25 index with Qdrant's Query API:

```json
{ "query": { "text": "user name" }, "using": "source_code" }
```

## Reversibility

Auto-reversible → [`delete_payload_index`](./delete_payload_index.md) with the
same `schema:` **and** `params:` carried over, so the inverse can recreate the
index exactly as it was.

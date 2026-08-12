# `create_payload_index`

Create an index on a payload field so it can be used in filters efficiently.

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

## Reversibility

Auto-reversible → [`delete_payload_index`](./delete_payload_index.md) with the
same `schema:` **and** `params:` carried over, so the inverse can recreate the
index exactly as it was.

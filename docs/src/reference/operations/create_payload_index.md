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
```

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `collection` | string | yes | Collection that holds the field. |
| `field_name` | string | yes | Payload field to index. |
| `schema` | [`PayloadSchemaType`](../specs.md#payloadschematype) | yes | Field type: `keyword`, `integer`, `float`, `geo`, `text`, `bool`, `datetime`, `uuid`. |

## Reversibility

Auto-reversible → [`delete_payload_index`](./delete_payload_index.md) with the
same `schema:` carried over, so the inverse can recreate the index if needed.

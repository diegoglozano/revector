# `delete_payload_index`

Drop an index from a payload field. The field stays in the payload — only the
index goes away.

## Example

```yaml
up:
  - op: delete_payload_index
    collection: products
    field_name: category
    schema: keyword          # optional, but needed for auto-rollback
```

Pass `schema:` when you want the downgrade to be auto-reversible — revector
uses it to reconstruct the matching `create_payload_index`.

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `collection` | string | yes | Collection that holds the index. |
| `field_name` | string | yes | Payload field whose index to drop. |
| `schema` | [`PayloadSchemaType`](../specs.md#payloadschematype) | no | Original field schema. Required for auto-reversibility. |

## Reversibility

- Auto-reversible **iff** `schema:` is supplied → recreates the index via
  [`create_payload_index`](./create_payload_index.md).
- Without `schema:`, revector has no way to know the original field type and
  refuses the auto-downgrade; supply an explicit `down:`.

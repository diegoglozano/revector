# Drift detection (`diff`)

`revector diff` compares a declared collection spec against the live collection.
It is **declaration-driven**: only fields you actually wrote in the spec are
compared. A field you leave unset means "don't care", never "must be unset" —
this avoids the classic Alembic-autogenerate false positives caused by Qdrant
normalizing and defaulting config on read.

```sh
revector diff products --spec products.spec.yaml
# collection `products` has 1 difference(s):
#   vectors.<default>.size : declared 1024 | live 768
```

The spec file uses the same [`CollectionSpec`](../reference/specs.md#collectionspec)
shape as a `create_collection` op's `spec:` block — without the surrounding
`op:` / `name:` keys, since `diff` already knows the collection from its CLI
argument.

```yaml
# products.spec.yaml
vectors:
  "":
    size: 1024
    distance: Cosine
hnsw_config:
  m: 16
  ef_construct: 128
```

`diff` exits non-zero when differences are found, so it slots into CI as a
drift guard. Folding the full migration chain into a desired-state spec is
future work — today you write the spec file by hand.

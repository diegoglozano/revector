# Re-embedding (the exec-hook)

Changing a vector's `size` or `distance` is structural — Qdrant can't mutate it
in place. The path is: add a new named vector → re-embed points with your model
→ drop the old vector. The re-embedding step is the one thing a generic binary
can't own, so revector shells out to *your* command via the
[`exec`](../reference/operations/exec.md) op:

```yaml
up:
  - op: create_vector
    collection: products
    name: text_v2
    spec: { size: 1024, distance: Cosine }

  - op: exec
    name: re-embed with the new model
    command: "python scripts/reembed.py --collection products --target text_v2"

  - op: delete_vector          # irreversible — make this a separate, deliberate migration
    collection: products
    name: text_v1
```

The command runs via `sh -c`, inherits the environment and stdio, and a
non-zero exit aborts the migration.

## Recommended split

Put the destructive step (`delete_vector`) in a **separate** migration applied
after you've verified the new vector is healthy. Two migrations instead of one
gives you:

- A safe rollback point — you can roll back to "both vectors exist" without
  losing data.
- An explicit checkpoint where someone has to look at the new state before the
  old one is destroyed.

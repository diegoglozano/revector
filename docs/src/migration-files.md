# Migration files

A migration is a YAML file with a revision id, a link to its parent, and `up` /
optional `down` operation lists. Each operation names itself with an `op:` key.

```yaml
revision: "0001_products"
down_revision: null            # null marks the base of the chain
description: create products collection

up:
  - op: create_collection
    name: products
    spec:
      vectors:
        "":                    # "" is the unnamed/default vector
          size: 768
          distance: Cosine
      hnsw_config:
        m: 16
        ef_construct: 128

  - op: create_payload_index
    collection: products
    field_name: category
    schema: keyword

# Optional. If omitted, revector auto-inverts the `up` ops in reverse order
# and refuses the downgrade if any step is irreversible.
down:
  - op: delete_collection
    name: products
```

## Required fields

| Field | Type | Notes |
|-------|------|-------|
| `revision` | string | Unique id. Auto-scaffolded by `revector new` as `<timestamp>_<slug>`. |
| `down_revision` | string \| `null` | Parent revision. `null` marks the base of the chain. |
| `description` | string | Free-text label for `revector status` output. |
| `up` | list of operations | Steps applied on `revector up`. |
| `down` | list of operations | Optional. If omitted, revector auto-inverts `up` in reverse order. |

## What each `op:` does

See the [Operations reference](./reference/operations.md) for a full description,
example, spec fields, and reversibility behavior of every operation.

## Safety

- **Confirmation.** Rollbacks (`down`, and a `to` that moves backwards) prompt
  before proceeding. Pass `-y` / `--yes` to skip the prompt; in a non-interactive
  shell (CI) revector refuses rather than guessing, so `--yes` is required there.
- **Advisory lock.** `up` / `down` / `to` / `stamp` take a lock record in the
  tracking collection for the duration of the run, so two concurrent runs (e.g.
  parallel CI jobs) don't stomp on each other. If a previous run died and left a
  stale lock, `--force` overrides it. (Best-effort — Qdrant has no
  compare-and-set — but it reliably catches the common case.)
- **Checksums.** revector records the SHA-256 of each migration file when
  applied; editing an already-applied file fails loudly on the next run. See
  [How state is tracked](./guides/state-tracking.md).

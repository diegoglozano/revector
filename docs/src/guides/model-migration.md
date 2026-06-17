# Model migration (end-to-end recipe)

You're switching embedding models — a new model, a new dimensionality, or a new
distance metric. In Qdrant a vector's `size` and `distance` are **immutable**:
the engine will not alter them in place, and you must not recreate the whole
collection under a live service. The supported path is a three-beat dance:

> **add a new vector → re-embed your points into it → drop the old vector**

revector owns the first and third beats as ordinary, reversible schema
operations. The middle beat — running *your* model over *your* points — is the
one thing a generic binary can't own, so revector shells out to your command via
the [`exec`](../reference/operations/exec.md) op. This page walks the whole
thing end-to-end, including the checkpoints where you stop and verify before
anything destructive happens.

If you only want the mechanics of the exec-hook itself, see
[Re-embedding (the exec-hook)](./re-embedding.md). This guide is the full recipe
built on top of it.

## Pick a strategy

There are two shapes, and the right one depends on **which** vector you're
changing:

| Strategy | Use when | Cutover | Rollback |
|----------|----------|---------|----------|
| **Named-vector swap** (in place) | The collection uses **named** vectors and you can add a second one alongside the old. | Switch your query code from `text_v1` to `text_v2`. | Roll back to "both vectors exist". |
| **Collection rebuild + alias** | You're changing the **default/unnamed** vector, or you want a clean blue/green collection. | `switch_alias` flips reads atomically. | `switch_alias` back to the old collection. |

The named-vector swap is the default — it's cheaper (no full copy of the
collection) and keeps point ids stable. Reach for the alias rebuild when the
vector you need to change is the unnamed default (which can't sit alongside a
second default) or when you want a fully isolated new collection you can smoke-
test before any traffic touches it.

---

## Strategy 1 — Named-vector swap

Scenario: `products` has a named `text_v1` vector (768-dim, an old model) and
you're moving to a 1024-dim model under the name `text_v2`.

Split the work across **three migrations** so each destructive boundary is its
own deliberate, reversible step:

```text
0005_add_text_v2_vector      add the new (empty) vector       reversible
0006_reembed_text_v2         exec-hook: fill it with the new model
0007_drop_text_v1_vector     remove the old vector            one-way
```

### Migration 1 — add the new vector

```yaml
revision: "0005_add_text_v2_vector"
down_revision: "0004_..."
description: add text_v2 vector for the new embedding model

up:
  - op: create_vector
    collection: products
    name: text_v2
    spec:
      size: 1024
      distance: Cosine
# down: auto-inverts to delete_vector (the vector is still empty here).
```

`create_vector` [auto-reverses](../reference/operations/create_vector.md) to
`delete_vector`, so this migration is freely reversible — at this point the new
vector holds no embeddings worth keeping.

### Migration 2 — re-embed via the exec-hook

```yaml
revision: "0006_reembed_text_v2"
down_revision: "0005_add_text_v2_vector"
description: re-embed products into text_v2 with the new model

up:
  - op: exec
    name: re-embed products → text_v2
    command: "python scripts/reembed.py --collection products --target text_v2"

# `exec` has no automatic inverse. Re-running migration 1's down would already
# drop text_v2 and its data; only spell out a `down` here if you need a
# distinct compensating action.
down:
  - op: exec
    name: clear text_v2 (no-op placeholder)
    command: "true"
```

The command runs via `sh -c`, inherits the environment and stdio, and a
**non-zero exit aborts the migration** — so make your script fail loudly. A
minimal, resumable re-embed script:

```python
# scripts/reembed.py
import argparse
from qdrant_client import QdrantClient, models
from my_embeddings import embed  # your new model

def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--collection", required=True)
    ap.add_argument("--target", required=True)       # the new vector name
    ap.add_argument("--source-field", default="text")  # payload field to embed
    ap.add_argument("--batch", type=int, default=256)
    args = ap.parse_args()

    client = QdrantClient(url="http://localhost:6334")
    offset = None
    while True:
        points, offset = client.scroll(
            collection_name=args.collection,
            with_payload=True,
            with_vectors=False,
            limit=args.batch,
            offset=offset,
        )
        if not points:
            break

        texts = [p.payload[args.source_field] for p in points]
        vectors = embed(texts)  # batch-encode with the NEW model

        client.update_vectors(
            collection_name=args.collection,
            points=[
                models.PointVectors(id=p.id, vector={args.target: v})
                for p, v in zip(points, vectors)
            ],
        )
        if offset is None:        # last page
            break

if __name__ == "__main__":
    main()
```

Notes that make this safe in production:

- **Idempotent.** `update_vectors` only writes the named `target` vector, so
  re-running after a crash just overwrites the same points — exactly what you
  want, since Qdrant has no transactional DDL and revector's `exec` is resumable.
- **Only the target vector is touched.** The old `text_v1` and all payloads are
  left intact, which is what keeps migration 1's rollback lossless.
- **Fail loudly.** Let exceptions propagate (non-zero exit) so a partial run
  aborts the migration instead of silently leaving half-embedded points.

### Apply and verify

```sh
export REVECTOR_URL=http://localhost:6334
revector up            # applies 0005 then 0006
revector status        # confirm both are recorded as applied
```

Now **verify before you drop anything**. Point a few real queries at `text_v2`,
compare recall against `text_v1`, and confirm every point actually has the new
vector (a point missing from your `--source-field` would be skipped). This is
the checkpoint the three-migration split exists to give you.

If something's wrong, you can still roll all the way back — nothing destructive
has happened yet:

```sh
revector down          # undo 0006 (exec down)
revector down          # undo 0005 → delete_vector text_v2
```

### Migration 3 — drop the old vector (one-way)

Once you've cut your query code over to `text_v2` and you're happy, retire the
old vector in its **own** migration:

```yaml
revision: "0007_drop_text_v1_vector"
down_revision: "0006_reembed_text_v2"
description: drop the retired text_v1 vector

up:
  - op: delete_vector
    collection: products
    name: text_v1
# No down: dropping a vector destroys its data. revector refuses the downgrade
# rather than pretend it's reversible.
```

```sh
revector up            # applies 0007 — deletes text_v1
```

`delete_vector` is irreversible: revector will **refuse** a downgrade past this
point instead of pretending the old embeddings can come back. Keeping it in a
separate revision means everything *before* it stays rollback-able, and the
destructive step is an explicit, reviewable boundary.

---

## Strategy 2 — Collection rebuild + alias

Use this when you're changing the **default/unnamed** vector, or you want a
fresh collection you can smoke-test in isolation before any read touches it.
Callers talk to a stable **alias** the whole time; you build `products_v2`
alongside `products_v1` and flip the alias atomically.

Assume your app already reads through the `products` alias (if it reads the raw
collection name, add a `create_alias` first and switch your client to the
alias).

### Migration 1 — build the new collection

```yaml
revision: "0005_build_products_v2"
down_revision: "0004_..."
description: build products_v2 with the new model dimensions

up:
  - op: create_collection
    name: products_v2
    spec:
      vectors:
        "":
          size: 1024          # new model's dimensionality
          distance: Cosine
      hnsw_config: { m: 16, ef_construct: 128 }
# down: auto-inverts to delete_collection products_v2 (still empty).
```

### Migration 2 — re-embed into the new collection

```yaml
revision: "0006_reembed_products_v2"
down_revision: "0005_build_products_v2"
description: embed all points into products_v2 with the new model

up:
  - op: exec
    name: re-embed products_v1 → products_v2
    command: "python scripts/reembed_into.py --source products_v1 --target products_v2"
```

Here the script reads points (and payloads) from `products_v1`, embeds with the
new model, and **upserts** full points into `products_v2`. Same idempotency and
fail-loud rules as Strategy 1; the difference is you're writing whole points
into a separate collection rather than one named vector in place.

### Migration 3 — flip the alias (the zero-downtime cutover)

```yaml
revision: "0007_switch_products_alias"
down_revision: "0006_reembed_products_v2"
description: point the products alias at products_v2

up:
  - op: switch_alias
    alias: products
    to_collection: products_v2

# switch_alias does NOT record the previous target — spell out the inverse.
down:
  - op: switch_alias
    alias: products
    to_collection: products_v1
```

[`switch_alias`](../reference/operations/switch_alias.md) is atomic, so reads
move from `v1` to `v2` with no gap. Because you supplied a `down`, rolling this
migration back instantly repoints the alias to the old collection — your
blue/green safety net. Keep `products_v1` around until you're confident, then
retire it in a final, one-way migration:

```yaml
revision: "0008_drop_products_v1"
down_revision: "0007_switch_products_alias"
description: drop the retired products_v1 collection

up:
  - op: delete_collection
    name: products_v1
# No down — delete_collection destroys data; revector refuses the downgrade.
```

---

## Checklist & gotchas

- **Split destructive steps into their own migration.** `delete_vector` /
  `delete_collection` are one-way; isolating them keeps every earlier step
  reversible and gives reviewers an explicit checkpoint.
- **Verify between re-embed and drop.** Check recall and coverage on the new
  vector/collection *before* the destructive migration runs — that gap is the
  whole point of the split.
- **Make the embed script resumable.** `exec` re-runs cleanly after a failure;
  use `update_vectors` (named-vector swap) or idempotent upserts (rebuild) so a
  retry overwrites rather than duplicates.
- **Let the script fail loudly.** A non-zero exit aborts the migration; swallow
  errors and you'll record a "successful" half-embedded state.
- **Coordinate the model side with the [`qdrant-model-migration`](https://github.com/qdrant/skills)
  skill** — revector orchestrates the schema and the cutover; that skill drives
  the embedding work itself.
- **Don't move points between clusters with revector.** That's data movement —
  use [`qdrant/migration`](https://github.com/qdrant/migration). revector only
  re-embeds in place via the exec-hook.

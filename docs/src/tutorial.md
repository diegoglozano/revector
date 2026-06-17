# Tutorial: from zero to a versioned collection

This is the hands-on walkthrough: start a local Qdrant in Docker, create a
collection from scratch with revector, evolve it across a few migrations, roll a
change back, and tear it all down. By the end you'll have a `migrations/`
directory you could commit next to your code.

Everything here runs locally and disposably — no cloud account, no external
state store.

## Prerequisites

- **Docker** (to run Qdrant locally).
- **revector** on your `PATH` — see [Install](./install.md). The quickest route
  from a checkout is `cargo install --path .`; verify with `revector --help`.

## 1. Start Qdrant locally (latest Docker image)

Qdrant publishes an official image. Pull the latest and run it, exposing both
the REST (`6333`) and gRPC (`6334`) ports — revector talks gRPC on `6334`:

```sh
docker run -p 6333:6333 -p 6334:6334 \
  --name qdrant-revector-tutorial \
  qdrant/qdrant:latest
```

Leave that running in its own terminal. To confirm it's up, the REST API and a
web dashboard are on `6333`:

```sh
curl http://localhost:6333/healthz       # → healthz check passed
# or open the dashboard:                 http://localhost:6333/dashboard
```

> **Tip — persist data across restarts.** The container above is ephemeral; add
> `-v "$(pwd)/qdrant_storage:/qdrant/storage"` to keep the data on disk. For a
> throwaway tutorial you don't need it.

## 2. Initialize a revector project

In a fresh working directory:

```sh
revector init
```

This creates two things:

- `migrations/` — where your migration files live.
- `revector.toml` — project config, pre-pointed at `http://localhost:6334`,
  which is exactly where our Docker container is listening:

```toml
# revector.toml
url = "http://localhost:6334"
migrations_dir = "migrations"
# api_key = "..."            # or set REVECTOR_API_KEY
# tracking_collection = "_revector_migrations"
```

Because the default already matches local Docker, you don't have to set anything
else. (If you'd run Qdrant elsewhere, you'd set `url` here or export
`REVECTOR_URL`. See [Configuration](./reference/configuration.md).)

## 3. Write your first migration — create a collection

Scaffold a migration. `revector new` creates a timestamped file chained onto the
current head (here, nothing yet, so it becomes the base of the chain):

```sh
revector new "create products collection"
#  → migrations/1718480000_create_products_collection.yaml
```

Open that file. It's a commented template; replace the `up:` section so it
creates a `products` collection with a 768-dim cosine text vector:

```yaml
revision: "1718480000_create_products_collection"
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

# No explicit `down`: revector auto-inverts create_collection to
# delete_collection on a rollback.
```

(Your `revision` and filename will use a real timestamp — leave them as
scaffolded; the `0001_…` ids elsewhere in the docs are just hand-authored
examples.)

## 4. Validate, then apply

First check the chain parses and resolves — **offline**, no Qdrant needed. This
is the same check you'd run in CI:

```sh
revector validate
```

Now apply it. revector connects to Qdrant, creates the collection, and records
the revision as applied inside Qdrant itself (in a `_revector_migrations`
collection):

```sh
revector up
```

Preview first without touching Qdrant by adding `--dry-run` to print the plan.

## 5. Inspect what happened

Check revector's own view of the world:

```sh
revector status
```

You'll see `1718480000_create_products_collection` marked **applied**. And the
collection really exists — ask Qdrant directly:

```sh
curl http://localhost:6333/collections/products
```

or browse it in the dashboard at <http://localhost:6333/dashboard>. Notice
there's also a `_revector_migrations` collection — that's revector's tracking
store, living inside the same Qdrant instance (no external database). See
[How state is tracked](./guides/state-tracking.md).

## 6. Evolve the schema — a second migration

Real projects change. Let's index a payload field and turn on scalar
quantization. Scaffold another migration — it chains automatically onto the
previous head:

```sh
revector new "index category and quantize"
```

Edit it so `up` adds an index and patches the config, with an explicit `down`
(config patches aren't auto-reversible because Qdrant doesn't hand back the
prior values):

```yaml
revision: "1718480100_index_category_and_quantize"
down_revision: "1718480000_create_products_collection"
description: index the category field and enable scalar quantization

up:
  - op: create_payload_index
    collection: products
    field_name: category
    schema: keyword

  - op: update_collection
    collection: products
    quantization_config:
      scalar:
        type: int8
        quantile: 0.99
        always_ram: true

down:
  - op: update_collection
    collection: products
    quantization_config: disabled

  - op: delete_payload_index
    collection: products
    field_name: category
    schema: keyword
```

Apply just the new one:

```sh
revector up
revector status      # both revisions now applied
```

`revector up` is idempotent and resumable — if a step fails partway (Qdrant has
no transactional DDL), just run it again and it picks up where it left off.

## 7. Roll a change back

Made a mistake, or just want to see reversibility work? Roll back the last
migration:

```sh
revector down        # undoes the quantize/index migration
revector status      # the second migration is pending again; the first still applied
```

`down` rolls back one step by default; pass `--steps N` or `--to <rev>` to go
further, and `--yes` to skip the confirmation prompt (required in
non-interactive shells like CI). revector **refuses** rollbacks that would
silently lose data instead of pretending — see the
[operations reference](./reference/operations.md) for each op's reversibility.

Re-apply when you're ready:

```sh
revector up
```

## 8. (Optional) Check for drift

If someone changes the collection by hand outside revector, you can catch it.
Declare the expected shape in a spec file and diff it against the live
collection:

```sh
revector diff products --spec expected.yaml
```

See [Drift detection (`diff`)](./guides/diff.md) for the spec format and how it
avoids false positives from Qdrant's normalized defaults.

## 9. Tear down

When you're done experimenting:

```sh
docker rm -f qdrant-revector-tutorial
```

Your `migrations/` directory and `revector.toml` remain — that's the artifact
you'd commit to version control. Pointed at a staging or prod Qdrant (via
`REVECTOR_URL`), the exact same files reproduce this schema there. `revector up`
is safe to run in CI: it takes an advisory lock so parallel jobs don't race.

## Where to next

- Changing an embedding model later? → [Model migration (end-to-end
  recipe)](./guides/model-migration.md).
- Adopting a collection that already exists? →
  [Adopting an existing collection](./guides/adopting.md).
- Every subcommand and flag → [Commands](./reference/commands.md); every op →
  [Operations](./reference/operations.md).

# How state is tracked

Applied revisions are stored as points in the `_revector_migrations` collection
(a dummy 1-d vector plus a payload of revision id, parent, checksum, and
timestamp). Because the checksum of each migration file is recorded, revector
refuses to proceed if a migration was **edited after being applied** — catching
silent divergence between your files and the database.

You can rename the tracking collection via the
[`tracking_collection`](../reference/configuration.md) config option (or the
`REVECTOR_TRACKING_COLLECTION` env var), if `_revector_migrations` clashes with
naming conventions in your cluster.

## Advisory lock

While `up` / `down` / `to` / `stamp` is running, revector writes a lock record
into the tracking collection so a second concurrent process refuses to start.
If a previous run died mid-flight and left a stale lock, pass `--force` to
override it.

This is best-effort — Qdrant has no compare-and-set primitive, so two processes
that both check the lock at the exact same moment can both proceed. In
practice the window is small and the common case (parallel CI jobs racing for
the same DB) is caught reliably.

## Inspecting the tracking collection

The tracking collection is a normal Qdrant collection — you can query it with
the Qdrant API or dashboard if you need to debug. Each point's payload looks
roughly like:

```json
{
  "revision":      "0002_index_and_quantize",
  "down_revision": "0001_create_products",
  "checksum":      "sha256:…",
  "applied_at":    "2025-01-15T12:34:56Z"
}
```

Never edit these payloads by hand — use `revector stamp` instead.

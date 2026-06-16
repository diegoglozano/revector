# Adopting an existing collection

If you already have a collection that matches an early migration, `stamp` lets
revector take over without re-creating it:

```sh
# Tell revector the DB is already at 0002 (no operations run)
revector stamp 0002_index_and_quantize
# Now apply everything after it normally
revector up
```

`stamp` accepts a revision id, the literal `head` (latest revision in the
chain), or `base` (clear all tracking, mark nothing applied). It records every
revision up to and including the target as applied, and removes any recorded
revisions above it.

Use `--dry-run` to preview the change to the tracking collection without
writing.

## Recommended bootstrap

1. Write the migrations that would have produced your current schema, in
   order. Don't apply them.
2. Run `revector validate` to make sure the chain parses and resolves.
3. Run `revector stamp head --dry-run` and confirm what would be written.
4. Run `revector stamp head` for real.
5. From now on, new schema changes go through `revector new` + `revector up`.

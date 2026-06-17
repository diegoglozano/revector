# Quick start

```sh
# 1. Scaffold a config + migrations/ directory
revector init

# 2. Create your first migration
revector new "create products collection"
#  → migrations/1718480000_create_products_collection.yaml

# 3. Edit the file (see Migration files), then apply
export REVECTOR_URL=http://localhost:6334
revector up

# 4. Inspect state
revector status

# 5. Roll back the last migration
revector down
```

Want the full, hands-on version — starting a local Qdrant in Docker and building
a collection from scratch? See the
[Tutorial](./tutorial.md).

See the [Commands reference](./reference/commands.md) for every subcommand and
flag, and [Migration files](./migration-files.md) for the YAML format.

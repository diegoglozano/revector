# Configuration

Settings are layered (highest precedence first): **CLI flags → `REVECTOR_*`
environment variables → `revector.toml` → defaults.**

```toml
# revector.toml
url = "http://localhost:6334"
migrations_dir = "migrations"
# api_key = "..."                      # or REVECTOR_API_KEY
# tracking_collection = "_revector_migrations"
```

| Setting | Env | Default |
|---------|-----|---------|
| `url` | `REVECTOR_URL` | `http://localhost:6334` |
| `api_key` | `REVECTOR_API_KEY` | _none_ |
| `migrations_dir` | `REVECTOR_MIGRATIONS_DIR` | `migrations` |
| `tracking_collection` | `REVECTOR_TRACKING_COLLECTION` | `_revector_migrations` |

Set `REVECTOR_LOG=revector=debug` for verbose logging (or pass `-v` / `-vv`).

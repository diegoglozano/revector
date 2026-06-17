# CI/CD integration

revector is a single static binary that reads its config from flags or
`REVECTOR_*` environment variables and runs fully non-interactively — which is
exactly what a pipeline needs. The natural split is:

- **On every pull request** — run `revector validate` (offline, no Qdrant) so a
  broken or unorderable chain never merges. Optionally add `revector diff` as a
  drift guard.
- **On deploy / merge to `main`** — run `revector up` against the target Qdrant
  so the schema moves forward in lockstep with the code that depends on it.

The two safety nets that make this trustworthy in automation are built in: the
**SHA-256 checksum guard** refuses to proceed if an already-applied migration
file was edited, and the **advisory lock** (held as a point in the tracking
collection) stops two concurrent pipeline runs from applying at once. Neither
needs configuration.

## Non-interactive usage

Three things matter when running outside a TTY:

1. **Pass connection details as env vars**, not flags, so secrets stay out of
   process listings and logs:

   ```sh
   export REVECTOR_URL="$QDRANT_URL"
   export REVECTOR_API_KEY="$QDRANT_API_KEY"   # from your CI secret store
   ```

2. **Use `-y` / `--yes` for anything that prompts.** `up` is non-destructive and
   runs unattended, but `down` and `to` (when rolling back) require confirmation
   in an interactive shell — `-y` is mandatory for them in CI.

3. **Rely on exit codes.** Every command exits non-zero on failure, so no extra
   `grep`-the-output logic is needed. `revector diff` exits non-zero when it
   finds drift; `validate` exits non-zero on a malformed or unresolvable chain.

## Installing the binary in a runner

Use the prebuilt installer — no Rust toolchain required:

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.sh | sh
```

Pin to a specific release in CI rather than tracking `latest`, so a new release
can't change behavior under you — swap `latest/download` for
`download/vX.Y.Z`. (See [Security & supply chain](../project/security.md) for
verifying release artifacts.)

## GitHub Actions

### Validate on every pull request

This job needs no Qdrant and no secrets — it just parses the migration files and
resolves the chain offline. Keep it fast and required.

```yaml
# .github/workflows/migrations.yml
name: migrations
on:
  pull_request:
    paths:
      - "migrations/**"
      - "revector.toml"

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install revector
        run: |
          curl --proto '=https' --tlsv1.2 -LsSf \
            https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.sh | sh
      - name: Validate migration chain
        run: revector validate
```

### Apply on deploy

Gate this on your deploy event (a push to `main`, a tag, or an environment
deployment) and pull the Qdrant URL and API key from repository or environment
secrets. The GitHub `environment:` key lets you require a manual approval before
the migration runs.

```yaml
  apply:
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    environment: production       # optional: gate behind a required reviewer
    env:
      REVECTOR_URL: ${{ secrets.QDRANT_URL }}
      REVECTOR_API_KEY: ${{ secrets.QDRANT_API_KEY }}
    steps:
      - uses: actions/checkout@v4
      - name: Install revector
        run: |
          curl --proto '=https' --tlsv1.2 -LsSf \
            https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.sh | sh
      - name: Show plan
        run: revector status
      - name: Apply pending migrations
        run: revector up
```

`revector up` is idempotent: every operation checks existence before
create/delete, so a re-run after a partial failure (or a duplicate pipeline
trigger) is safe. A run with nothing pending is a no-op that exits zero.

### Preview the plan on the PR

To see *what* a merge would change before it merges, run `up --dry-run` against
a staging instance — it prints the ordered plan without touching anything:

```yaml
      - name: Plan against staging
        env:
          REVECTOR_URL: ${{ secrets.STAGING_QDRANT_URL }}
          REVECTOR_API_KEY: ${{ secrets.STAGING_QDRANT_API_KEY }}
        run: revector up --dry-run
```

## Drift detection as a guard

If you want CI to catch a live collection that has drifted from its declared
spec (someone hand-edited config in the Qdrant dashboard, say), add a
[`diff`](./diff.md) step. It exits non-zero on any difference, so a failing job
flags the drift:

```yaml
      - name: Check for drift
        run: revector diff products --spec specs/products.spec.yaml
```

This is **declaration-driven** — only fields you actually wrote in the spec are
compared — so it won't false-positive on Qdrant's read-time defaulting. See the
[drift detection guide](./diff.md) for the spec-file shape.

## GitLab CI

The same shape maps onto any runner. Store `QDRANT_URL` and `QDRANT_API_KEY` as
masked/protected CI/CD variables.

```yaml
stages: [validate, deploy]

validate-migrations:
  stage: validate
  image: debian:stable-slim
  before_script:
    - apt-get update && apt-get install -y curl
    - curl --proto '=https' --tlsv1.2 -LsSf
        https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.sh | sh
    - export PATH="$HOME/.cargo/bin:$PATH"
  script:
    - revector validate

apply-migrations:
  stage: deploy
  image: debian:stable-slim
  rules:
    - if: $CI_COMMIT_BRANCH == "main"
  variables:
    REVECTOR_URL: $QDRANT_URL
    REVECTOR_API_KEY: $QDRANT_API_KEY
  before_script:
    - apt-get update && apt-get install -y curl
    - curl --proto '=https' --tlsv1.2 -LsSf
        https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.sh | sh
    - export PATH="$HOME/.cargo/bin:$PATH"
  script:
    - revector status
    - revector up
```

## Rollbacks in automation

Rollbacks are deliberately less automatic, because a `down` can be destructive
or irreversible (revector refuses to auto-invert a step that needs unrecorded
prior state — see [Migration files](../migration-files.md)). If you do wire a
rollback step — for example a manually-triggered job — it must pass `-y` and
should target an explicit revision rather than relying on step counts:

```yaml
      - name: Roll back to a known-good revision
        run: revector to 0002_index_and_quantize -y
```

Prefer `revector to <rev> -y` over `down --steps N` in pipelines: it's explicit
about the end state and idempotent if re-run.

## Checklist

- [ ] `revector validate` runs on every PR that touches `migrations/`.
- [ ] Secrets come from the CI secret store as `REVECTOR_API_KEY` /
      `REVECTOR_URL`, never committed to `revector.toml`.
- [ ] The apply job is gated on a deploy event and, ideally, a required
      reviewer / protected environment.
- [ ] `revector status` (or `up --dry-run`) runs before `up` so the plan is in
      the logs.
- [ ] The installer is pinned to a release tag, not `latest`.
- [ ] Any rollback job passes `-y` and targets an explicit revision.

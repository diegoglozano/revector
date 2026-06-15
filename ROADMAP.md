# Roadmap

Tracking for follow-up work. Items are roughly in priority order within each
section.

## Distribution

- [x] **cargo-dist** — prebuilt binaries + shell/PowerShell installers + a
  Homebrew formula, released on tag push (`.github/workflows/release.yml`).
- [ ] **Homebrew auto-publish** — create a public `diegoglozano/homebrew-tap`
  repo and a `HOMEBREW_TAP_TOKEN` secret (PAT with `contents:write` on the tap),
  then uncomment `tap` + `publish-jobs` in `dist-workspace.toml`. Enables
  `brew install diegoglozano/tap/revector`.
- [ ] **crates.io** — reserve the `revector` name and publish (`cargo publish`),
  ideally automated with `cargo-release`. Enables `cargo install revector`.
- [ ] **PyPI via maturin (for `uvx` / `pipx` / `pip`)** — package the binary
  crate into platform wheels with `maturin` (no PyO3 bindings needed; same
  pattern Astral uses for `ruff`). Add a `maturin publish` CI job gated on tags.
  Enables `uvx revector …` for the embeddings/uv crowd — high ROI for our
  audience. See discussion: maturin can build wheels for a `[[bin]]`-only crate
  that install the executable as a console script.
- [ ] **Docker image on ghcr.io** — multi-stage build → `FROM scratch` with the
  musl static binary, for CI/CD pipelines that run migrations as a step.
- Go (`go install`): not possible for a Rust binary — intentionally skipped.

## Correctness / features

- [ ] **`diff` from the migration chain** — fold `create_collection` /
  `update_collection` / `create_vector` ops into a desired-state spec so
  `revector diff <collection>` needs no separate `--spec` file.
- [ ] **Auto-reversible `update_collection`** — snapshot live config into the
  tracking payload before patching, so config changes can be rolled back
  without a hand-written `down`.
- [ ] **Optimizer-status polling** — after index/config builds, poll
  `collection_info` until optimizers settle so "done" isn't fuzzy.
- [ ] More live integration coverage: irreversible-downgrade refusal and the
  checksum-mismatch guard (both have unit coverage today).

## v2

- [ ] Branching/merging revision graphs (v1 enforces a single linear chain).
- [ ] PyO3 / uniffi bindings (deferred — low payoff for a network-I/O workload).
- [ ] mdBook documentation site.

# Development

```sh
cargo test                       # unit + logic tests
cargo clippy --all-targets       # lints
cargo fmt                        # format
```

Integration tests spin up a real Qdrant via
[testcontainers](https://github.com/testcontainers/testcontainers-rs) (Docker
required); they skip automatically when Docker is unavailable. To run them
against an already-running Qdrant instead:

```sh
REVECTOR_TEST_URL=http://localhost:6334 cargo test --test integration
```

## Building the docs site

The site you're reading is built with [mdBook](https://rust-lang.github.io/mdBook/):

```sh
cargo install mdbook       # one-time
mdbook serve docs --open   # live preview on http://localhost:3000
mdbook build docs          # static site → docs/book/
```

`docs/book/` is gitignored — CI builds and publishes the site to GitHub Pages
on every push to `main`. The source lives under `docs/src/`.

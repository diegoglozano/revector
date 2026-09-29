# Install

```sh
# Homebrew (macOS/Linux)
brew install diegoglozano/tap/revector

# crates.io (requires Rust 1.82+)
cargo install revector
```

Prebuilt binaries for Linux, macOS, and Windows are attached to each
[GitHub Release](https://github.com/diegoglozano/revector/releases) by
[cargo-dist], with shell/PowerShell installers:

```sh
# Shell (Linux/macOS) — downloads the right prebuilt binary
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.sh | sh

# Windows (PowerShell)
powershell -c "irm https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.ps1 | iex"
```

Or build from source:

```sh
cargo install --path .          # from a checkout
cargo build --release           # ./target/release/revector
```

Further distribution channels (PyPI/`uvx`, a Docker image) are tracked in
[ROADMAP.md](https://github.com/diegoglozano/revector/blob/main/ROADMAP.md).

[cargo-dist]: https://opensource.axo.dev/cargo-dist/

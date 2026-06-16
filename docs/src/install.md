# Install

Once a release is cut, prebuilt binaries for Linux, macOS, and Windows are
attached to each [GitHub Release](https://github.com/diegoglozano/revector/releases)
by [cargo-dist], with installers:

```sh
# Shell (Linux/macOS) — downloads the right prebuilt binary
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.sh | sh

# Windows (PowerShell)
powershell -c "irm https://github.com/diegoglozano/revector/releases/latest/download/revector-installer.ps1 | iex"
```

Or build from source (requires Rust 1.82+):

```sh
cargo install --path .          # from a checkout
cargo build --release           # ./target/release/revector
```

Homebrew and crates.io are planned — see
[ROADMAP.md](https://github.com/diegoglozano/revector/blob/main/ROADMAP.md).

[cargo-dist]: https://opensource.axo.dev/cargo-dist/

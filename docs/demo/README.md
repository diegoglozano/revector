# Demo recording

`revector.gif` (used at the top of the project README) is generated from
`revector.tape` with [VHS](https://github.com/charmbracelet/vhs). The recording
runs the real CLI against a live Qdrant — nothing is faked.

## Regenerate

Prerequisites:

- `vhs` on your PATH (it pulls a headless Chromium on first run; it will not run
  as root — use a normal user).
- `revector` on your PATH: `cargo build --release` then add `target/release/`.
- A Qdrant reachable at `$REVECTOR_URL` (defaults to `http://localhost:6334`),
  e.g. `docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant`.

Then, from this directory:

```sh
vhs revector.tape      # writes revector.gif
```

`migration.yaml` is the pre-written migration body the tape drops in after
`revector new` (so the demo doesn't show a live text-editor session).

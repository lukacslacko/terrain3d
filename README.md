# terrain3d

A simple 3D app with a small planet with generated terrain, where you can place cities and connect them with train lines.

The terrain is generated using Perlin noise. The connections are calculated by the bidirectional version of Dijkstra's algorithm, and elevation change is taken into account in the edge weights.

## Run locally

Run with `cargo run -r`. Startup takes a bit of time, see the progress report in the console.

## Usage

- Drag with left mouse button to rotate the planet.
- Scroll with the mouse to zoom in and out.
- Right click to create a city.
- Left click one city then another to connect them.
- Left click a train to ride it.
- Press Esc to get off a train.

## Development

### Profiling

To profile, do `cargo install flamegraph`, then run with `cargo flamegraph`
as admin, eg in windows by searching for `cmd`, right clicking on
`Command Prompt` and selecting `Run as administrator`.
Finally, open the result flamegraph.svg in a browser.

### Clippy

CI runs `cargo clippy --all-targets -- -D warnings` with the latest stable Rust, so any
clippy warning fails the build. To fix them:

1. Run `rustup update`, since newer clippy versions add lints, then run the same command
   locally to see the warnings.
2. Run `cargo clippy --fix --all-targets` to apply clippy's automatic suggestions. Add
   `--allow-dirty` if you have uncommitted changes.
3. Clippy's edits aren't formatted, so run `rustfmt --edition 2024 <file>` on the files it
   changed. Avoid `cargo fmt` for this: not all files are rustfmt-formatted yet, and it
   would reformat them too.
4. Fix whatever is left by hand; not every lint can be fixed automatically.

### WebAssembly (WASM) Build & Deployment

#### Local WASM Build

To build the Bevy app for WebAssembly and output to the `dist/` directory (for local testing):

- On Linux/macOS:
  ```sh
  bash build-wasm.sh
  ```
- On Windows:
  ```bat
  build-wasm.bat
  ```

The output will be in the `dist/` directory, which is not checked in. To test the WASM build
locally, run
```
trunk serve --release --filehash=false
```

#### Deployment (GitHub Actions)

The `WASM build and deploy` workflow runs on every push to `main` that changes the app's
code or build setup, and can also be started manually from the Actions tab. It:
- Builds the WASM app using Trunk and optimizes it with `wasm-opt`
- Uploads the `dist/` directory as a Pages artifact and deploys it

On pull requests it only builds the WASM app, so a broken web build shows up before merging.

The build output is never committed. For this to work, GitHub Pages must be set to deploy
from GitHub Actions: Settings → Pages → Build and deployment → Source: "GitHub Actions".
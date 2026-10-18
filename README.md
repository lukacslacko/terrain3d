# terrain3d

- Run with `cargo run -r`. Startup takes a bit of time, see the progress report in the console.
- Drag with left mouse button to rotate the Earth.
- Scroll with the mouse to zoom in and out.
- Right click to create a city.
- Left click one city then another to connect them.
- Left click a train to ride it.
- Press Esc to get off a train.

## Profiling

To profile, do `cargo install flamegraph`, then run with `cargo flamegraph`
as admin, eg in windows by searching for `cmd`, right clicking on
`Command Prompt` and selecting `Run as administrator`.
Finally, open the result flamegraph.svg in a browser.

## WebAssembly (WASM) Build & Deployment

### Local WASM Build

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

### Deployment (GitHub Actions)

The `Deploy to GitHub Pages` workflow is started manually from the Actions tab. It:
- Builds the WASM app using Trunk and optimizes it with `wasm-opt`
- Uploads the `dist/` directory as a Pages artifact and deploys it

The build output is never committed. For this to work, GitHub Pages must be set to deploy
from GitHub Actions: Settings → Pages → Build and deployment → Source: "GitHub Actions".
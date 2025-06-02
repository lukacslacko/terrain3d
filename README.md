# terrain3d

## WebAssembly (WASM) Build & Deployment

### Local WASM Build

To build the Bevy app for WebAssembly and output to the `dist/` directory (for local testing or manual deployment):

- On Linux/macOS:
  ```sh
  bash build-wasm.sh
  ```
- On Windows:
  ```bat
  build-wasm.bat
  ```

The output will be in the `dist/` directory. You can serve this directory locally with 
```
trunk server --release --filehash=false
```
to test the WASM build.

### Automatic Deployment (GitHub Actions)

On every push to the `main` branch, a GitHub Actions workflow automatically:
- Builds the WASM app using Trunk
- Outputs the result to the `dist/` directory
- Commits and pushes any changes in `dist/` back to the repository

You can configure GitHub Pages to serve from the `/dist` folder on the `main` branch for automatic deployment.
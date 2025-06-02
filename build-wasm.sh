#!/usr/bin/env bash
# build-wasm.sh: Build the Bevy app for WebAssembly and output to docs/
set -e

# Ensure trunk is installed
if ! command -v trunk &> /dev/null; then
    echo "Trunk not found. Installing..."
    cargo install trunk
fi

# Build the app to docs/
trunk build --release --public-url terrain3d --dist docs

echo "WASM build complete. Output is in ./docs."

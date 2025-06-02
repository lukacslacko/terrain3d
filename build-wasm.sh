#!/usr/bin/env bash
# build-wasm.sh: Build the Bevy app for WebAssembly and output to docs/
set -e

# Ensure trunk is installed
if ! command -v trunk &> /dev/null; then
    echo "Trunk not found. Installing..."
    cargo install trunk
fi

cargo install wasm-opt

rustup target add wasm32-unknown-unknown

trunk build --release --public-url terrain3d --filehash false --dist docs
wasm-opt -Oz -o docs/terrain3d_bg.wasm docs/terrain3d_bg.wasm

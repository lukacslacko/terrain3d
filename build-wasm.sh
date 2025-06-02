#!/usr/bin/env bash
# build-wasm.sh: Build the Bevy app for WebAssembly and output to dist/
set -e

# Ensure trunk is installed
if ! command -v trunk &> /dev/null; then
    echo "Trunk not found. Installing..."
    cargo install trunk
fi

cargo install wasm-opt

rustup target add wasm32-unknown-unknown

trunk build --release --public-url terrain3d --filehash false
wasm-opt -Oz -o dist/terrain3d_bg.wasm dist/terrain3d_bg.wasm

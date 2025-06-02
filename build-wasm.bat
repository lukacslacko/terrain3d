@echo off
REM build-wasm.bat: Build the Bevy app for WebAssembly and output to dist/

where trunk >nul 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo Trunk not found. Installing...
    cargo install trunk
)

cargo install wasm-opt

rustup target add wasm32-unknown-unknown
trunk build --release --public-url terrain3d --filehash false
wasm-opt -Oz -o dist/terrain3d_bg.wasm dist/terrain3d_bg.wasm

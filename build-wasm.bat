@echo off
REM build-wasm.bat: Build the Bevy app for WebAssembly and output to docs/

where trunk >nul 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo Trunk not found. Installing...
    cargo install trunk
)

trunk build --release --public-url terrain3d --dist docs

echo WASM build complete. Output is in .\docs.

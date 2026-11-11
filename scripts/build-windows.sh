#!/bin/sh
# Cross-compiles a Windows build from macOS.
#   scripts/build-windows.sh  -> target/x86_64-pc-windows-msvc/release/rusty-text.exe
# One-time setup:
#   rustup target add x86_64-pc-windows-msvc
#   cargo install --locked cargo-xwin
set -e
cd "$(dirname "$0")/.."

TARGET=x86_64-pc-windows-msvc

# Static C runtime: the .exe runs without the Visual C++ redistributable installed.
# /ignore:4099: the downloaded SDK libraries ship without their debug-symbol files.
RUSTFLAGS="-C target-feature=+crt-static -C link-arg=/ignore:4099" \
    cargo xwin build --release --target "$TARGET"
echo "Built target/$TARGET/release/rusty-text.exe"

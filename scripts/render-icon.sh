#!/bin/sh
# Renders assets/icon.svg into assets/icon.png (1024px, for the macOS bundle)
# and assets/icon-256.png (baked into the binary for the window and Dock icon).
# Needs Google Chrome, which draws the SVG filters that make the rust.
set -e
cd "$(dirname "$0")/.."

CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
"$CHROME" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --window-size=1024,1024 --default-background-color=00000000 \
    --screenshot="$PWD/assets/icon.png" "file://$PWD/assets/icon.svg" 2>/dev/null
sips -z 256 256 assets/icon.png --out assets/icon-256.png >/dev/null
echo "Rendered assets/icon.png and assets/icon-256.png"

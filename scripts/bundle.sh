#!/bin/sh
# Builds "RustyText.app" and optionally installs it.
#   scripts/bundle.sh            -> target/RustyText.app
#   scripts/bundle.sh --install  -> also copies it to /Applications
set -e
cd "$(dirname "$0")/.."

NAME="RustyText"
APP="target/$NAME.app"

cargo build --release
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# The program inside the bundle is named after the app, not the crate: macOS builds the
# application menu from the process name ("About RustyText", "Quit RustyText").
cp target/release/rusty-text "$APP/Contents/MacOS/$NAME"

# assets/icon.png (1024px) -> AppIcon.icns with every size macOS asks for.
ICONSET=target/AppIcon.iconset
rm -rf "$ICONSET" && mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
    sips -z $size $size assets/icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    sips -z $((size * 2)) $((size * 2)) assets/icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$NAME</string>
    <key>CFBundleDisplayName</key><string>$NAME</string>
    <key>CFBundleIdentifier</key><string>local.rustytext</string>
    <key>CFBundleExecutable</key><string>$NAME</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>0.1</string>
    <key>CFBundleVersion</key><string>1</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>Copyright © 2026 Olivier Lafont. GNU General Public License version 3.</string>
</dict>
</plist>
PLIST

codesign --force --sign - "$APP"
echo "Built $APP"

if [ "$1" = "--install" ]; then
    rm -rf "/Applications/$NAME.app"
    cp -R "$APP" /Applications/
    echo "Installed /Applications/$NAME.app"
fi

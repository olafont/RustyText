#!/bin/sh
# Builds "RustyText.app" and optionally installs it.
#   scripts/bundle.sh                    -> target/RustyText.app, for this Mac
#   scripts/bundle.sh --install          -> also copies it to /Applications
#   scripts/bundle.sh --target <triple>  -> for another kind of Mac:
#                                           aarch64-apple-darwin (Apple silicon) or x86_64-apple-darwin (Intel)
#   scripts/bundle.sh --notarize         -> also has Apple notarize the app, so that other Macs
#                                           open it without a warning (needs the Developer ID below)
#
# Signing. With a "Developer ID Application" certificate in your keychain, the app is signed
# with it (hardened runtime, timestamp); without one it is signed ad hoc, which only this Mac
# trusts. To choose among several certificates, or to force ad hoc:
#   CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" scripts/bundle.sh
#   CODESIGN_IDENTITY=- scripts/bundle.sh
# Notarizing needs your Apple ID stored once in the keychain, under the profile name "rustytext"
# (or the name in NOTARY_PROFILE):
#   xcrun notarytool store-credentials rustytext --apple-id you@example.com --team-id TEAMID
set -e
cd "$(dirname "$0")/.."

NAME="RustyText"
APP="target/$NAME.app"
INSTALL=0
NOTARIZE=0
TARGET=""
while [ $# -gt 0 ]; do
    case "$1" in
        --install) INSTALL=1 ;;
        --notarize) NOTARIZE=1 ;;
        --target) TARGET="$2"; shift ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

if [ -n "$TARGET" ]; then
    rustup target add "$TARGET" >/dev/null
    cargo build --release --target "$TARGET"
    BINARY="target/$TARGET/release/rusty-text"
else
    cargo build --release
    BINARY="target/release/rusty-text"
fi
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# The program inside the bundle is named after the app, not the crate: macOS builds the
# application menu from the process name ("About RustyText", "Quit RustyText").
cp "$BINARY" "$APP/Contents/MacOS/$NAME"
# The license and the notices travel with the app (GPL-3.0 section 4).
cp LICENSE THIRD_PARTY_NOTICES.md "$APP/Contents/Resources/"

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
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key><string>Markdown document</string>
            <key>CFBundleTypeRole</key><string>Editor</string>
            <key>LSHandlerRank</key><string>Default</string>
            <key>LSItemContentTypes</key>
            <array><string>net.daringfireball.markdown</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>Text document</string>
            <key>CFBundleTypeRole</key><string>Editor</string>
            <key>LSHandlerRank</key><string>Default</string>
            <key>LSItemContentTypes</key>
            <array><string>public.plain-text</string></array>
        </dict>
    </array>
    <key>NSHumanReadableCopyright</key><string>Copyright © 2026 Olivier Lafont. GNU General Public License version 3.</string>
</dict>
</plist>
PLIST

# Sign: with the Developer ID certificate if there is one, ad hoc otherwise.
if [ -z "${CODESIGN_IDENTITY+set}" ]; then
    CODESIGN_IDENTITY=$(security find-identity -v -p codesigning 2>/dev/null \
        | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)
fi
if [ -n "$CODESIGN_IDENTITY" ] && [ "$CODESIGN_IDENTITY" != "-" ]; then
    # The hardened runtime and a secure timestamp are what notarization asks for.
    codesign --force --options runtime --timestamp --sign "$CODESIGN_IDENTITY" "$APP"
    echo "Signed as $CODESIGN_IDENTITY"
else
    codesign --force --sign - "$APP"
    echo "Signed ad hoc (no Developer ID Application certificate in the keychain)"
    if [ "$NOTARIZE" = 1 ]; then
        echo "--notarize needs a Developer ID Application certificate" >&2
        exit 1
    fi
fi
codesign --verify --strict "$APP"

if [ "$NOTARIZE" = 1 ]; then
    # Apple checks a zip of the app; the ticket it issues is then attached to the app itself,
    # so that it opens without a warning even on a Mac that is offline.
    ZIP="target/$NAME-notarize.zip"
    ditto -c -k --keepParent "$APP" "$ZIP"
    xcrun notarytool submit "$ZIP" --keychain-profile "${NOTARY_PROFILE:-rustytext}" --wait
    rm -f "$ZIP"
    xcrun stapler staple "$APP"
    echo "Notarized $APP"
fi
echo "Built $APP"

if [ "$INSTALL" = 1 ]; then
    rm -rf "/Applications/$NAME.app"
    cp -R "$APP" /Applications/
    echo "Installed /Applications/$NAME.app"
fi

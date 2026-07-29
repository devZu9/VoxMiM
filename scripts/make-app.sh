#!/bin/bash
set -e

cd "$(dirname "$0")/.."

APP_NAME="VoxMiM"
APP_DIR="$PWD/$APP_NAME.app"

echo "=== Building $APP_NAME ==="
cargo build --release

echo "=== Creating .app bundle ==="
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Info.plist — background-only app (no dock icon)
cat > "$APP_DIR/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
 "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>com.devzu.$APP_NAME</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleVersion</key>
    <string>$(cargo metadata --format-version=1 --no-deps | python3 -c "import sys,json; print(json.load(sys.stdin)['packages'][0]['version'])")</string>
    <key>CFBundleShortVersionString</key>
    <string>$(cargo metadata --format-version=1 --no-deps | python3 -c "import sys,json; print(json.load(sys.stdin)['packages'][0]['version'])")</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSUIElement</key>
    <true/>
</dict>
</plist>
EOF

# Binary
cp target/release/voxmim "$APP_DIR/Contents/MacOS/$APP_NAME"

# Icon — generate .icns from the largest PNG asset
ICON_SRC="assets/blue-voice.png"
ICON_DIR="$APP_DIR/Contents/Resources"

if [ -f "$ICON_SRC" ]; then
    echo "=== Generating icon ==="
    ICONSET=$(mktemp -d)/icon.iconset
    mkdir -p "$ICONSET"

    for s in 16 32 128 256 512; do
        sips -z $s $s "$ICON_SRC" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null 2>&1 || true
        s2=$((s * 2))
        sips -z $s2 $s2 "$ICON_SRC" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null 2>&1 || true
    done

    iconutil -c icns "$ICONSET" -o "$ICON_DIR/$APP_NAME.icns" 2>/dev/null || true
    rm -rf "$(dirname "$ICONSET")"
fi

# Copy fix-permissions script
cp scripts/fix-permissions.sh "$APP_DIR/Contents/Resources/"

echo "=== Done ==="
echo "$APP_DIR"
echo ""
echo "Run: open '$APP_DIR'"

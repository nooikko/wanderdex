#!/bin/bash
# Builds Wobbuffet.app in this folder.
set -euo pipefail
cd "$(dirname "$0")"

APP=Wobbuffet.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/sprites"

# Base-form sprite sheets only (Offsets/Shadow sheets and alternate forms are not used).
cp assets/0202/AnimData.xml assets/0202/*-Anim.png "$APP/Contents/Resources/sprites/"
cp CREDITS.md "$APP/Contents/Resources/"

# Vendored pixel-art scalers: xBRZ (GPL-3) and MMPX (MIT).
OBJ=$(mktemp -d)
clang++ -O2 -std=c++17 -c ThirdParty/xbrz/xbrz.cpp -o "$OBJ/xbrz.o"
clang++ -O2 -std=c++17 -IThirdParty -c ThirdParty/xbrz_bridge.cpp -o "$OBJ/xbrz_bridge.o"
clang -O2 -std=c99 -c ThirdParty/mmpx/mmpx.c -o "$OBJ/mmpx.o"

swiftc -O -import-objc-header ThirdParty/scalers.h -o "$APP/Contents/MacOS/Wobbuffet" \
  Sources/main.swift "$OBJ"/*.o -lc++
rm -rf "$OBJ"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Wobbuffet</string>
  <key>CFBundleIdentifier</key><string>local.wobbuffet.pet</string>
  <key>CFBundleExecutable</key><string>Wobbuffet</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>LSUIElement</key><true/>
</dict>
</plist>
PLIST

codesign --force --sign - "$APP" >/dev/null 2>&1 || true
echo "Built $APP — run: open $APP"

#!/usr/bin/env bash
# Builds a universal (Apple Silicon + Intel) NawsMapper.app and packs it into a disk image
# that opens as the usual "drag NawsMapper onto Applications" window.
# Runs on macOS only (CI or a Mac). Output: dist/NawsMapper-macOS.dmg
set -euo pipefail

cd "$(dirname "$0")/.."
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
BUILD="${NAWSMAPPER_BUILD:-${GITHUB_RUN_NUMBER:-0}}"

rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --locked -p om-app --target aarch64-apple-darwin
cargo build --release --locked -p om-app --target x86_64-apple-darwin

# The disk image's contents: the app and a link to /Applications to drag it onto.
STAGE="dist/dmg"
APP="$STAGE/NawsMapper.app"
rm -rf dist
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create \
    target/aarch64-apple-darwin/release/nawsmapper \
    target/x86_64-apple-darwin/release/nawsmapper \
    -output "$APP/Contents/MacOS/nawsmapper"
sed -e "s/__VERSION__/$VERSION/" -e "s/__BUILD__/$BUILD/" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
ln -s /Applications "$STAGE/Applications"

# Ad-hoc signature: required for Apple Silicon to launch the binary at all.
# (Not notarized, so the first launch needs "Open Anyway", see docs/platforms.md.)
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

# hdiutil now and then fails with "Resource busy" on CI machines; trying again fixes it.
DMG="dist/NawsMapper-macOS.dmg"
for attempt in 1 2 3 4 5; do
    if hdiutil create -volname NawsMapper -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG"; then
        break
    fi
    if [[ $attempt == 5 ]]; then
        echo "hdiutil failed $attempt times, giving up" >&2
        exit 1
    fi
    echo "hdiutil failed (attempt $attempt), retrying in 10 s" >&2
    sleep 10
done
echo "Built $DMG (version $VERSION, build $BUILD)"

#!/usr/bin/env bash
# Builds a universal (Apple Silicon + Intel) NawsMapper.app and zips it.
# Runs on macOS only (CI or a Mac). Output: dist/NawsMapper-macos-universal.zip
set -euo pipefail

cd "$(dirname "$0")/.."
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
BUILD="${GITHUB_RUN_NUMBER:-0}"

rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --locked -p om-app --target aarch64-apple-darwin
cargo build --release --locked -p om-app --target x86_64-apple-darwin

APP="dist/NawsMapper.app"
rm -rf dist
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create \
    target/aarch64-apple-darwin/release/nawsmapper \
    target/x86_64-apple-darwin/release/nawsmapper \
    -output "$APP/Contents/MacOS/nawsmapper"
sed -e "s/__VERSION__/$VERSION/" -e "s/__BUILD__/$BUILD/" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

# Ad-hoc signature: required for Apple Silicon to launch the binary at all.
# (Not notarized — first launch needs right-click > Open, see docs/platforms.md.)
codesign --force --deep --sign - "$APP"

(cd dist && ditto -c -k --keepParent NawsMapper.app NawsMapper-macos-universal.zip)
echo "Built dist/NawsMapper-macos-universal.zip (version $VERSION, build $BUILD)"

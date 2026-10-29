# Platforms and builds

## Linux (development)

```sh
cargo run              # debug build, deps optimized
cargo run --release
```

Needs a Vulkan driver (Mesa is fine), X11 or Wayland, GTK 3 (for file dialogs; the dev
headers to build) and a C++ compiler (OpenH264 is compiled from source by `openh264-sys2`).
The GPU backend is chosen by wgpu automatically. Force one with `WGPU_BACKEND=vulkan` or `WGPU_BACKEND=gl`.
Logging: `RUST_LOG=om_app=debug,om_render=debug cargo run`.

## macOS (primary target)

Built by GitHub Actions on `macos-latest` (see `.github/workflows/ci.yml`):

- `scripts/bundle-macos.sh` builds `aarch64` and `x86_64`, merges them with `lipo`
  into a universal binary, wraps it in `NawsMapper.app` (Info.plist from
  `packaging/macos/`), ad-hoc signs it and zips it.
- Every push uploads the zip as a workflow artifact
  (Actions → run → *NawsMapper-macos-universal*).
- Pushing a `v*` tag also attaches the zip to a GitHub release.

### First launch of an unsigned build

The app is ad-hoc signed but **not notarized**, so Gatekeeper blocks a
double-click on first launch. Either:

- right-click `NawsMapper.app` → **Open** → **Open**, or
- run this once in Terminal:

```sh
xattr -dr com.apple.quarantine /Applications/NawsMapper.app
```

Proper signing and notarization need an Apple Developer ID ($99/yr) and can be added to
the workflow later (`codesign --sign "Developer ID Application: …"` + `notarytool`).

### Notes

- Rendering uses Metal (through wgpu). Retina is handled by egui.
- `Cmd` shortcuts come from `egui::Modifiers::COMMAND`, which is Cmd on macOS and Ctrl elsewhere.
- Settings and last project: `~/Library/Application Support/net.nawsmapper.NawsMapper/`.
- File dialogs open as sheets on the main window (rfd async API, see architecture.md).
- If something misbehaves, start the app from Terminal to see its log:
  `/Applications/NawsMapper.app/Contents/MacOS/nawsmapper`

## Video codec and licensing

Video uses OpenH264 compiled from source (`openh264` crate, default `source` feature), so
there are no system libraries to bundle on any platform. Note that Cisco's H.264 patent
license only covers Cisco's own prebuilt binaries, not builds from source. Look into
that before distributing builds publicly.
The crate can also load Cisco's binary at runtime (`libloading` feature) if that matters later.

Only H.264 is decoded today. HEVC (iPhone default), ProRes and HAP files are recognized and
rejected with a message that says how to re-export them. A native AVFoundation backend on
macOS would add HEVC/ProRes with hardware decoding (see roadmap).

## Windows

CI builds `nawsmapper.exe` on `windows-latest` (artifact *NawsMapper-windows-x64*).

To cross-compile from Linux, one option is the GNU target with MinGW:

```sh
rustup target add x86_64-pc-windows-gnu
# plus a MinGW-w64 toolchain from your distro, e.g. on Void: cross-x86_64-w64-mingw32
cargo build --release -p om-app --target x86_64-pc-windows-gnu
```

Or the MSVC target with `cargo-xwin` (`cargo install cargo-xwin`, then
`cargo xwin build --release -p om-app --target x86_64-pc-windows-msvc`).
Rendering uses DX12 or Vulkan.

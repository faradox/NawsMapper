# 0001: Foundation (v0.1)

- **Status:** done (pending first hands-on session with the user)
- **Started / finished:** 2026-10-28 / 2026-10-28
- **Driven by:** [feedback-log.md](../feedback-log.md) background entry

## Goal

A first build that compiles, runs on Linux and macOS, and lets a newcomer map a few
shapes onto real objects and animate them. It should already show the two things
competitors lack: easy texture animation and comfortable multi-shape editing.

## Scope

In:
- Cargo workspace `om-core` / `om-render` / `om-app`; egui 0.37 + wgpu 30.
- Four-corner perspective surfaces; quad / ellipse / triangle masks; feather; blend modes.
- Canvas editor: select, Shift/Cmd toggle, marquee, move, corner pin, snapping,
  welded corners, arrow nudging, pan/zoom (Space, middle mouse, scroll, pinch).
- Layers panel (visibility, lock, restack, range select), Sources panel, Properties panel
  with multi-edit through `diff::propagate_changes`.
- Sources: solid color, 7 GPU generators, images (dialog + drag & drop).
- Texture transform (Fit/Span, offset, scale, rotation, flip) and animation (scroll,
  spin, hue cycle, zoom LFO, opacity LFO, speed, time offset, presets, stagger).
- Projector output window (immediate viewport, `F` full screen, outlines option).
- Undo/redo (snapshot, commit on pointer release), save/open `.omap`, reopen last project,
  unsaved-changes prompt.
- Docs, CI (Linux checks, universal macOS `.app`, Windows `.exe`).

Out (deferred): video, timeline/keyframes, monitor picker, multiple outputs, mesh warp,
groups, native macOS menu, signing/notarization.

## Design notes

- One shared output texture for preview and projector, so what you see is what's projected.
- Fragment-shader inverse homography for perspective-correct UVs on any quad.
- Generators evaluated in texture-UV space, so all motion controls apply to them too.
- Undo by snapshots: the project is small; robustness beats cleverness.
- Multi-edit via JSON-value diff: zero per-field code, new fields join automatically.
- Corner welding by coincidence (≤ 0.5 px) instead of explicit links: no extra data
  model, behaves the way snapping already implies.

## Tasks

- [x] Workspace, dependencies, rustfmt config
- [x] `om-core`: project model, homography, animation, diff, history (+ tests)
- [x] `om-render`: pipeline per blend mode, dynamic uniforms, image cache, WGSL (+ naga validation test)
- [x] `om-app`: shell, actions, shortcuts, theme
- [x] Canvas editor interaction + overlays (+ tests for snap/weld/hit-test)
- [x] Layers, Sources, Properties panels
- [x] Projector output window
- [x] File I/O, persistence, drag & drop, close prompt
- [x] README, AGENTS, docs, CI, macOS bundling script
- [ ] First hands-on session with the user → feedback-log entry

## Verification

- `cargo test`: 25 tests (core math/animation/diff/history/serialization, shader
  validation, uniform layout, document ops, editor snap/weld/hit-test).
- `cargo clippy --all-targets`: clean. `cargo fmt`: clean.
- Smoke-tested on Linux (X11, Intel Vulkan): demo renders at 60 fps, animation plays,
  output window opens and mirrors the canvas, list panels and glyphs render.
- Not yet verified by hand: corner dragging/snapping on a real projector, and the macOS build
  (first CI run will tell).

## Follow-ups

- Narrow windows squeeze the canvas (side panels have fixed min widths). Consider
  collapsible panels (`Tab` to hide all, like Photoshop).
- egui menus live inside the window. A native macOS menu bar would feel more at home.
- The fps readout shows the UI repaint rate, which drops when paused (no repaint needed).

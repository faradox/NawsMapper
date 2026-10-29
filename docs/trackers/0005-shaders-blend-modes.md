# 0005: More shaders, Photoshop blend modes

- **Status:** done (pending her test on macOS)
- **Started / finished:** 2026-10-29 / 2026-10-29
- **Driven by:** [feedback-log.md](../feedback-log.md), v0.3 feedback

## Goal

She liked the Shader Library and wants more pretty, trippy shaders, and more blend
modes than Normal / Add / Screen / Multiply.

## Scope

In:
- 14 new shaders (`Pattern` Melt … Halftone, shader indices 27–40): Melt, Fractal Lace
  (Kali fractal), Quasicrystal, Caustics, Retro Grid, Galaxy, Electric, Bubbles, Mandala,
  Ink Blot, Silk, Glitch, Infinite Zoom, Halftone. Each has a `detail` control.
- 3 palettes: Vaporwave, Gold, Cosmic. 16 new library presets (42 total), including
  Pastel Kaleidoscope and Vapor Tunnel on existing shaders.
- All 24 Photoshop blend modes, grouped and ordered like Photoshop's menu
  (`BlendMode::GROUPS`), separators in the Properties combo.
- `Shift +` / `Shift -` cycles the blend mode of the selection (`Action::CycleBlend`).

Out: layer groups / clipping masks, blend modes for cutouts (they always paint the
background), linear-light (gamma-correct) blending.

## Design notes

- Six modes are exact with fixed-function blending on premultiplied output (Normal,
  Add, Screen, Multiply, Subtract via reverse-subtract, Exclusion via
  `OneMinusDst, OneMinusSrc`). They keep a pipeline each and cost nothing extra.
- The other 18 need the backdrop color. Before such a surface, the renderer ends the
  pass, copies only the surface's pixel bounding box from the target into a backdrop
  texture, and resumes with `LoadOp::Load`. The shader `textureLoad`s the backdrop at
  its pixel, applies the W3C compositing formula (separable modes plus Hue / Saturation
  / Color / Luminosity with `set_lum` / `set_sat`), mixes by backdrop alpha, and
  composites with normal premultiplied blending. Copying only the bounds keeps the cost
  proportional to the surface's size.
- The backdrop texture only grows (max of all targets), so the thumbnail atlas and the
  output don't fight over its size. Group 2 is bound in every pass.
- The blend mode reaches the shader in `shape.y`, so the uniform size is unchanged.
  `BlendMode` is serialized by name; new variants are appended.

## Tasks

- [x] `BlendMode`: 20 new variants, `GROUPS`, `all()`, `cycle()` (+ test)
- [x] Renderer: `fixed_blend`, `backdrop_index`, `pixel_bounds`, backdrop texture and
      pipeline, pass splitting in `draw` (+ 3 tests)
- [x] WGSL: backdrop binding, blend functions, 14 shader cases
- [x] `Palette` ×3, catalog presets, shader index test
- [x] Properties blend combo with groups; `Action::CycleBlend` on `Shift +/-`
- [x] Docs: README, user guide §4, architecture, roadmap, feedback log

## Verification

- `cargo test`: 64 tests pass (4 new). `cargo clippy --all-targets -D warnings` and
  `cargo fmt --check` clean. WGSL validates through naga.
- Xvfb (GL backend): a test scene with a rainbow backdrop and 24 tiles of a gray ramp,
  one per mode. Each tile matches its formula (e.g. Color Burn keeps blue where the
  backdrop's blue channel is 1, Hue/Saturation/Color give gray for a gray source,
  Luminosity keeps the backdrop's hue). `Shift +` twice went Lighten → Screen → Color
  Dodge, and the tile updated.
- Shader Library: all 42 previews checked by eye; tuned Quasicrystal (density),
  Bubbles (clipping and square outlines at cell borders), Galaxy (size), and swapped a
  too-dark Cosmic Kaleidoscope preset for Pastel Kaleidoscope.
- **Not yet verified:** macOS, and the cost on her GPU with many backdrop-mode
  surfaces at projector resolution (llvmpipe ran the 24-tile scene at ~25 fps).

## Follow-ups

- If she uses many Overlay/Soft Light surfaces on a slow GPU: batch non-overlapping
  surfaces into one copy.
- Shader favorites, ISF import (see roadmap).

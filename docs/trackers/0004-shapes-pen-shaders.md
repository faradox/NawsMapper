# 0004: More shapes, Pen and Edit Points, Shader Library

- **Status:** done (pending her test on macOS)
- **Started / finished:** 2026-10-29 / 2026-10-29
- **Driven by:** [feedback-log.md](../feedback-log.md), v0.2 feedback

## Goal

Her two wishes after v0.2: more shapes than circle/square/triangle, including
free-form shapes built from anchor points she can drag; and a catalogue of cool,
trippy shaders like the ones she uses in HeavyM.

## Scope

In:
- 11 new built-in shapes: rounded rectangle, diamond, pentagon, hexagon, octagon,
  star, burst, heart, cross, arrow, bolt. Toolbar **◆ Shapes** picker with icons,
  Surface → Add Shape, Shape combo with icon in Properties.
- `Mask::Freeform` + `Surface::path` (anchor points in the unit square, corner or smooth).
- **Pen** (`P`, toolbar ✒ Pen): click = corner point, press-drag = curve point; click
  the first point / double-click / Enter to finish; Backspace, Esc. Snaps to corners.
- **Edit Points** (double-click a free-form shape, or Properties → ✎ Edit Points, which
  converts any built-in shape): drag, click an edge to insert, Alt-click / Delete to
  remove, double-click to toggle curve/corner, arrows to nudge, Enter/Esc to leave.
  Properties: point count, All curves / All corners.
- Hit testing and outlines (editor and projector) follow the actual shape.
- 18 new shaders (`Pattern` Tunnel … Rain), `Generator::palette` (11 palettes) and
  `Generator::detail` (one per-shader control with its own label).
- `om_core::catalog`: 26 presets in 5 categories. **Shader Library** window (`Cmd+L`,
  toolbar ✨ Shaders, Sources ✨ Shaders…) with live animated previews; Apply to the
  selection, Add to Sources, double-click.
- Sources panel: palette combo with gradient strips, Background vs. two colors, the
  detail slider, palette swatches in the list; new sources scroll into view.

Out: Bézier handles, shapes with holes, boolean operations, ISF/WGSL import, user
presets/favorites, audio reactivity.

## Design notes

- **Shapes live in the unit square of the four corners.** The shader already maps every
  pixel into that square, so a shape is just a coverage function there. Built-in and
  free-form shapes therefore follow corner pinning, perspective and Free Transform with
  no extra work, and the four corners stay the handle for all of that.
- Quad/ellipse/triangle keep their exact SDFs. Everything else is a flattened polygon
  (≤ 128 points) in the uniforms, filled with an even-odd polygon distance (iq), so
  antialiasing and feather work the same for all shapes. One source of truth for
  outlines: `shape::outline_uv` feeds the shader, hit testing, overlays and icons.
- **Smooth points without handles:** Catmull-Rom tangents turned into cubic Béziers,
  so the curve passes through each anchor. Corner points have zero tangents. Simple to
  use; real handles can come later if she misses them.
- **Frame refit:** after every point edit the four corners move to the outline's
  bounds through the current homography (exact, also in perspective), and the points
  are re-expressed. The shape never leaves its quad (which is what gets drawn), and the
  texture keeps fitting the shape.
- **Palettes:** each shader returns (position, intensity). The palette is up to 6 stops
  in the uniforms: mirrored (first → last → first, seamless for any position) or
  cyclic (Rainbow). Intensity fades over color A, which acts as the background.
  Duotone reproduces the old two-color look exactly.
- Angle-based shaders use an explicit filter width (`aa_square_w`) and even multiples
  of the angle for palette positions, so the atan2 seam doesn't show.
- Thumbnails: the renderer draws a throwaway project of 192×120 tiles into an atlas
  texture, with the normal pipeline, only while the window is open.
- Uniforms grew from 224 to 1376 bytes per surface (palette stops + outline points).

## Tasks

- [x] `om-core::shape`: `Mask` (moved, 11 new), `PathPoint`, anchors, `flatten`, outlines,
      `Surface::freeform`/`outline`/`make_freeform`/point edits/`fit_frame_to_path` (+ 8 tests)
- [x] `Pattern` ×18, `Palette`, `Generator::{palette, detail}` (+ test), `catalog` (+ test)
- [x] Renderer: uniforms, polygon SDF, palette, 18 shader cases, thumbnail atlas (+ 2 tests)
- [x] Editor: outline hit testing/overlays, Pen, Edit Points (+ 3 tests)
- [x] App: actions, shortcuts (P, Cmd+L, Enter/Esc/Delete per mode), shape picker,
      Properties shape UI, Shader Library window, Sources generator UI
- [x] `Document::add_freeform` (+ test), `add_generator`
- [x] Docs: README, user guide §4/§8, architecture, AGENTS, roadmap, feedback log

## Verification

- `cargo test`: 60 tests pass (16 new). `cargo clippy --all-targets -D warnings` and
  `cargo fmt --check` clean. The WGSL (incl. uniformity analysis) validates through naga.
- Xvfb (GL backend), demo project:
  - Shape picker shows all 14 icons; Star and Heart added and render correctly.
  - Pen: four points with one press-dragged (curve) point; preview shows the curve,
    the closing edge and the highlighted first point; clicking it creates "Shape 1"
    (Free-form, 4 points) with the test grid fitted to it.
  - Edit Points: double-click entered the mode; dragging an anchor outside the frame
    refit the frame (dashed) without moving the other points; clicking an edge inserted
    a point that could be dragged right away.
  - Shader Library: all 26 previews animate; tuned Julia, spiral, Truchet, lava,
    starfield after the first look. Apply put Acid Tunnel / Julia Bloom on Box Left;
    the new source is selected and scrolled into view; Palette, Background and the
    detail slider ("Twist") show in Sources.
- **Not yet verified:** macOS, and shader cost on her GPU at projector resolution
  (Julia and Liquid are the heaviest; llvmpipe in Xvfb ran the library at ~20–30 fps).

## Follow-ups

- Bézier handles (Alt-drag a point), shapes with holes (ring), boolean ops.
- ISF import and a WGSL editor; favorites / save-as-preset in the library.
- Whatever shaders she reaches for most: more in that direction.

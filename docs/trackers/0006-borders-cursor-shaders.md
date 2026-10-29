# 0006: Borders, mouse on projector, more shaders

- **Status:** done (pending her test on macOS)
- **Started / finished:** 2026-10-29 / 2026-10-29
- **Driven by:** [feedback-log.md](../feedback-log.md), 2026-10-29 feature requests

## Goal

She can give any shape a border (color, thickness, solid/dashed/dotted/…), see her
mouse pointer on the projection while tracing a real object, and pick from many more
psychedelic shaders.

## Scope

In:
- `Surface::border` (`om_core::border::Border`): enabled, color, width (px), style
  (Solid, Dashed, Dotted, Double, Dash-dot), dash and gap lengths (px), flow (px/s).
  Properties → Border section. Multi-edit and undo come for free.
- Output → **Show Mouse on Projector** (plus a toolbar checkbox while the output is
  open): the projector draws an arrow where the pointer is on the editor canvas.
- 14 shaders (indices 41–54): Tie-Dye, Groovy, Twisted Checker, Mandelbrot Dive,
  Crystal, Chladni, Flower of Life, Rosettes, Tumbling Blocks, Lasers, Bokeh, Marble,
  Gyroid, Equalizer. 18 presets (60 total), including Pastel Tie-Dye, Flower Power,
  Oil Slick and Fairy Lights on the new shaders.

Out (deferred):
- Outside / centered borders (would need geometry beyond the surface quad).
- Border glow, gradient or animated-color borders; round dash caps.
- Remembering the projector toggles between sessions (Outlines isn't remembered either).

## Design notes

- **Inside borders.** Projection mapping puts shapes on real objects; a border that
  grows outward would spill onto the wall. Inside also needs no extra geometry: the
  fragment shader already covers the shape.
- **Width in pixels.** `d / |∇d|` (coverage distance over its screen-space gradient) is
  the distance from the edge in pixels to first order, also under perspective. Checked
  numerically on a strong keystone: 1 / 5 / 10 px come out as 1.00 / 5.02 / 10.06.
- **Dashes need arc length.** `border_path` returns the outline (any shape, quad and
  ellipse included) with the pixel distance at each point. Edges are split into pieces
  of equal *canvas* length (lerp on the canvas, `unmap` back: a homography keeps lines
  straight), so the shader's linear interpolation within a piece stays accurate. The
  distances are stretched so a whole number of periods fits: the pattern closes without
  a seam. The subdivided outline is the same polygon, so polygon shapes use it for
  coverage too (one list in the uniforms). Uniforms grow by 35 vec4s to 1936 bytes.
- The nearest segment is found in unit-square space, so dash ends follow the surface's
  perspective (on a keystoned quad they lean with it). That suits "painted on the object".
- **Border over fill, then blend.** The border is composited over the fill with straight
  alpha before the blend mode and opacity apply, so a border behaves like part of the
  layer (cutouts can have one too). Feather only softens the fill.
- **Projector cursor** is an egui overlay (design rule 1: never in the output texture).
  `Editor::pointer` holds the canvas position while the pointer is over the canvas view.
  In full screen the projector hides the OS cursor unless the toggle is on.
- **Shaders.** Mandelbrot Dive ping-pongs its zoom (pure function of time) and stops at
  ~1e-4 where f32 runs out. Gyroid flies along `x = π/2, y = 0`, a line that never
  touches the surface. Angle terms are seam-free (`a * 2`, integer multiples inside
  `sin`/`cos`); the beam angle in Lasers has its seam below the sources, off-surface.

## Tasks

- [x] `om_core::border`: `Border`, `BorderStyle`, `border_path` (+ 6 tests)
- [x] Renderer: border uniforms, outline + arc lengths for broken styles (+ 1 test)
- [x] WGSL: `border_coverage`, `outline_position`, pixel distance, compositing
- [x] Properties → Border section
- [x] `Editor::pointer`, `OutputWindow::show_cursor`, arrow drawing, Output menu + toolbar
- [x] 14 `Pattern` variants, labels, detail labels, WGSL cases, 18 presets, index test
- [x] Docs: README, user guide, architecture, roadmap, feedback log

## Verification

- `cargo test` (all crates), `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --check`: clean. WGSL validates through naga.
- Xvfb (GL backend, llvmpipe):
  - Border scene: solid quad, dashed ellipse (flowing), dotted star, dash-dot on a strong
    keystone, double heart, dashed outline-only hexagon. At 100% zoom: widths match on
    slanted edges, dots round, corners clean.
  - All 14 new shaders tiled, plus Gyroid / Mandelbrot / Crystal large at several times.
    Gyroid was reworked after the first look (too much glow, no depth cue).
  - Output menu shows the toggle; with it on, the projector window draws the arrow on
    the shape under the editor pointer.
  - Shader Library shows the new presets with live thumbnails.
- Not tested: macOS, real projector, OS cursor hiding in real full screen (Xvfb has no WM).

## Follow-ups

- Outside/centered border position if she asks for it (needs an expanded draw quad).
- Shader favorites / user presets now that the library has 60 entries.

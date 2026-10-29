# 0007: Animated borders, resizable Properties panel

- **Status:** done (pending her test on macOS)
- **Started / finished:** 2026-10-29 / 2026-10-29
- **Driven by:** [feedback-log.md](../feedback-log.md), 2026-10-29 "v0.5, borders and sidebar"

## Goal

Borders she can animate: neon gradients, stars flowing around the shape, a twirling
helix, fireworks. And a Properties panel that stays as narrow as she makes it, whatever
names are in its drop-downs.

## Scope

In:
- `Border` gains `effect` (Color, Gradient, Stars, Comets, Helix, Fireworks), `palette`
  and `color_b` (the shader palettes, two-color included), `glow`, `position` (Inside,
  Center, Outside) and `length` (gradient length / spacing / twist length). `flow` now
  moves every effect. All `#[serde(default)]`, so older files load unchanged.
- Seven presets in Properties → Border: Neon, Rainbow, Marching ants, Stars, Comets,
  Helix, Fireworks (`Border::presets`).
- Sidebar bug: long text in a drop-down no longer widens the Properties panel, and the
  panel can be dragged down to its 260 px minimum.

Out (deferred):
- Per-effect extra controls (star shape, burst size, helix strand count).
- Effects reacting to audio or tempo.

## Design notes

- **Two coordinates.** Everything is computed in `s` (px along the outline, from
  `border_path`) and `y` (px across the band from its center). Line styles became signed
  distances in (s, y), which lets glow follow dashes and dots.
- **Particles** (stars, comets, fireworks): one per repeat of `length` (fitted to a whole
  number around the outline), jittered by a hash of the repeat index modulo the count, so
  the seam where the outline starts is invisible. Each pixel checks its repeat and both
  neighbors. Colors come from the bright 60% of the palette, because several palettes
  (Fire, Gold, Neon) start near black.
- **Drawing outside the shape.** Center/Outside, glow and sparks paint beyond the edge, but
  a surface used to draw only inside its corners. Now `Border::outer_extent` says how far
  out it paints; the renderer and `vs_main` draw the corners' bounding box grown by that
  much (`draw_corners`), and the backdrop copy uses the same rectangle. The fragment shader
  already maps any pixel back through the inverse homography; pixels beyond the quad's
  horizon (`hp.z <= 0`) are skipped. Plain inside borders keep the old, tight quad.
- Far from the border (`|y|` beyond band + reach) the shader returns before the outline
  search, so effects cost little on the inside of big shapes.
- Uniforms: +8 vec4s (effect/glow/band, palette info, 6 palette stops) → 2064 bytes.
- **Sidebar.** egui grows a side panel to fit its content, and a combo box sizes itself to
  its selected text, so one long file name pinned the panel. Three changes:
  - The Properties content is laid out in a child `Ui` that doesn't report its size
    (`new_child` + `advance_cursor_after_rect`): the panel is as wide as the user drags it,
    anything wider is clipped.
  - `widgets::combo` truncates its text with "…" (full name on hover for Source).
  - `widgets::fit_slider` sizes the slider rail from the space left after last frame's
    value box; rows of choices wrap. So nothing needs clipping at normal widths.
  A first attempt that only reserved a fixed width for the value box made the panel creep
  wider (a row a few px too wide grows the panel, which grows the row). That is why the
  panel itself no longer listens to its content.

## Tasks

- [x] `BorderEffect`, `BorderPosition`, new `Border` fields, `band`, `glow_radius`,
      `reach`, `outer_extent`, `needs_path`, `presets` (+ 4 tests)
- [x] Renderer: effect uniforms, palette stops, `draw_corners`, grown backdrop copy (+ 1 test,
      1 extended)
- [x] WGSL: grown quad in `vs_main`, `border_paint` with 6 effects, horizon guard
- [x] Properties → Border: presets row, effect, palette, position, glow, length
- [x] `widgets::combo`, `widgets::fit_slider`, non-reporting Properties content
- [x] Docs: README, user guide, architecture, roadmap, feedback log

## Verification

- `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`: clean.
- Xvfb (GL, llvmpipe), a scene with one shape per preset look (quad, ellipse, star,
  heart, hexagon, rounded rectangle, burst): checked at 47–52% and at 100% zoom. Particle
  effects were too small and dark at first (palette ends near black); fixed and rechecked.
- Sidebar, with an image source named
  `projection_texture_for_the_living_room_wall_final_version_2_really_final.png`: the
  panel opens at its 300 px default for every surface, drags down to 260 px and out to
  459 px, and the Source combo shows "projection_texture_for_the_livi…".
- Not tested: macOS, real projector, performance of the effects on her Mac.

## Follow-ups

- "Video first, shapes as its mask" workflow: being discussed with the maintainer
  (see feedback log).

# NawsMapper

![NawsMapper: three faces of a box mapped with animated shaders, layers on the left, surface properties on the right](docs/images/screenshot.png)

Projection mapping software for designers. You draw surfaces over the real
objects you project onto, fill them with media or generated patterns, and
animate them. The interface feels familiar to anyone who knows Photoshop.

Written in Rust (egui + wgpu). macOS is the primary target; Linux is the
development platform; Windows builds come for free.

> Status: **v0.4.** Enough to map a box, a wall or a few shapes, fill them with
> shaders, images or video, and make them move. See [docs/roadmap.md](docs/roadmap.md)
> for what comes next.

## Why another mapper?

- **Animation first.** Every surface can scroll, spin, zoom-pulse, strobe and
  hue-cycle its texture, with no keyframing needed. One click staggers the
  animation across several surfaces for chase effects. (MadMapper users miss this.)
- **Many shapes at once.** Multi-select (Shift/Cmd-click or marquee) and every
  property edit applies to all selected surfaces. Only the field you changed
  is touched, so per-surface differences survive. (HeavyM users miss this.)
- **Familiar to designers.** Layers panel, blend modes, opacity, Cmd+Z,
  Cmd+D, Cmd+[ / ], Space to pan. Shortcuts follow Photoshop where it makes sense.

## What it can do

- Perspective-correct **four-corner surfaces** in 14 shapes (quad, ellipse, triangle,
  rounded rectangle, diamond, pentagon, hexagon, octagon, star, burst, heart, cross,
  arrow, bolt), all following corner pinning.
- **Free-form shapes**: draw your own with the **Pen** (`P`), or turn any shape into
  points with **Edit Points**: drag points, click an edge to add one, curve or corner points.
- **Free Transform** (`Cmd+T`): scale, rotate and move one or many surfaces with a
  bounding box, just like Photoshop (Shift = proportional / 15° steps, Alt = from center).
- **Cutouts**: a shape that cuts a hole in every surface below it (windows, doors,
  anything in front of the wall). Works with ellipse/triangle shapes, feather and opacity.
- **Corner snapping** and **welded corners**: faces that share a corner move together,
  which makes mapping a cube easy. Hold Alt/Option to bypass either.
- **Shader Library** (`Cmd+L`): 60 animated, live-previewed presets of 53 shaders (tunnel,
  kaleidoscope, tie-dye, groovy 70s bands, Mandelbrot dive, gyroid flight, melt, fractal
  lace, crystal, quasicrystal, mandala, flower of life, rosettes, Chladni figures, tumbling
  blocks, lasers, bokeh, marble, equalizer, infinite zoom, Julia fractal, lava lamp,
  caustics, silk, ink blot, galaxy, retro grid, electric, digital rain…), each
  with 14 color palettes and an extra control, all editable after adding.
- **Sources**: solid colors, the GPU shaders above (including a calibration test grid),
  **images** (PNG/JPEG/GIF/WebP/BMP/TGA)
  and **videos** (H.264 in MP4/MOV, looping, synced to the play clock). Import from the
  menu or by drag & drop.
- **Fit** (whole texture per surface) or **Span** (one texture across the whole canvas)
  mapping, plus texture offset, scale, rotation and flip.
- **Animate**: scroll, spin, hue cycle, zoom pulse and opacity pulse/strobe (LFOs with
  sine/triangle/saw/square), per-surface speed and time offset, presets, **Stagger**.
- **Borders** on any shape: color, width, solid / dashed / dotted / double / dash-dot,
  inside / centered / outside the edge, with **neon glow** and **animated effects**:
  flowing gradients, twinkling stars, comets, a twirling helix and fireworks, all with
  the shader palettes. One-click presets (Neon, Rainbow, Marching ants…). With no
  source, a surface is an outline only.
- Opacity, tint, feathered edges and all 24 **Photoshop blend modes** (Multiply, Screen,
  Overlay, Soft Light, Color Dodge, Difference, Hue, Color, Luminosity…), applied
  against everything below the surface. `Shift +` / `Shift -` cycles them.
- **Projector output window**: drag it to the projector and press `F` for full screen.
  Optional outlines on the projector help with aligning corners on the real object, and
  **Show Mouse on Projector** draws the pointer on the projection while you point at the
  canvas, so you can trace the real object while looking at it.
- Undo/redo, save/open (`.omap`, JSON), reopens the last project on launch
  (or the one given on the command line: `nawsmapper show.omap`).

## Quick start

```sh
cargo run --release          # build & launch
cargo test                   # run the test suite
```

On launch you get a demo scene: three faces of a box, animated. Put a box
in front of the projector, open **Output** (Cmd/Ctrl+Enter), move that window
to the projector, press `F`, then drag the corners in the editor until they
match the box. [docs/user-guide.md](docs/user-guide.md) walks through it.

Build requirements: Rust (stable, 1.95+). On Linux you also need the usual X11/Wayland
dev libraries and Vulkan drivers. See [docs/platforms.md](docs/platforms.md) for
macOS bundles (built by GitHub Actions) and Windows.

## Keyboard

| Key | Action |
| --- | --- |
| `Space` | Play / pause (hold and drag to pan) |
| `Q` | Add quad |
| `P` | Pen: draw a free-form shape (`Enter` finish, `Esc` cancel) |
| Double-click a free-form shape | Edit its points (`Delete` removes the selected point) |
| `Cmd+L` | Shader Library |
| `Shift +` / `Shift -` | Next / previous blend mode on the selection |
| `Cmd+T` | Free Transform (then `Enter` apply, `Esc` cancel) |
| Click / Shift-click / Cmd-click | Select / add to selection |
| Drag on empty canvas | Marquee select |
| Arrows (`Shift` ×10) | Nudge selection, or the last clicked corner |
| `Alt`/`Option` while dragging | No snapping, move a welded corner alone |
| `Cmd+S` / `Cmd+Shift+S` | Save / save as |
| `Cmd+Z` / `Cmd+Shift+Z` | Undo / redo |
| `Cmd+D`, `Delete` | Duplicate, delete |
| `Cmd+]` / `Cmd+[` (+`Shift`) | Bring forward / send backward (to front / back) |
| `Cmd+0`, `Cmd+1`, `Cmd+=`/`-` | Fit, 100 %, zoom in/out (or pinch / Cmd+scroll) |
| `Cmd+Enter` | Toggle projector output window |
| `F`, double-click, `Esc` | (in output window) full screen on / toggle / off |

`Cmd` is `Ctrl` on Linux and Windows.

## Repository layout

```
crates/
  om-core/    document model, perspective math, animation, undo, multi-edit diff (no GPU)
  om-media/   video decoding (MP4 + H.264 via OpenH264) on background threads
  om-render/  wgpu compositor: renders a project into the output texture (WGSL shader)
  om-app/     the desktop app: egui panels, canvas editor, projector window
docs/         architecture, user guide, roadmap, platform notes, implementation trackers
packaging/    macOS Info.plist
scripts/      bundling scripts
```

## Documentation

- [AGENTS.md](AGENTS.md): how to work on this codebase (humans and coding agents)
- [docs/architecture.md](docs/architecture.md): how the pieces fit
- [docs/user-guide.md](docs/user-guide.md): first projection, step by step
- [docs/roadmap.md](docs/roadmap.md): where we're going
- [docs/platforms.md](docs/platforms.md): macOS / Windows / Linux builds
- [docs/feedback-log.md](docs/feedback-log.md): user feedback and the decisions it drove
- [docs/trackers/](docs/trackers/): one implementation tracker per feature

## License

MIT OR Apache-2.0

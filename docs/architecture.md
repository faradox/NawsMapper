# Architecture

## Big picture

```
 ┌──────────── om-app (eframe / egui) ─────────────────────────────────────┐
 │  menus · toolbar · Layers · Sources · Properties · status   ──► Action  │
 │                                                                  │       │
 │  Editor (canvas): select / marquee / move / corner-pin / snap    ▼       │
 │        │ mutates                                   NawsMapperApp::run_action
 │        ▼                                                                 │
 │  Document { project, history, path }  ── commit_if_changed() ─► undo    │
 │        │ &Project, time                                                  │
 └────────┼─────────────────────────────────────────────────────────────────┘
          ▼
 ┌──── om-render ───────────────┐        ┌──── om-core ─────────────────────┐
 │ Renderer::render(project, t) │ uses   │ Project / Surface / Source        │
 │  per surface: uniforms ──────┼───────►│ anim::evaluate(surface, t)        │
 │  draw 2 tris, WGSL fragment  │        │ geom::Homography (perspective)    │
 │  → output texture (Rgba8)    │        │ diff::propagate_changes           │
 └──────────────┬───────────────┘        │ History (snapshots)               │
                │ registered as egui texture                                 │
                ▼                        └───────────────────────────────────┘
   editor preview (zoom/pan)   +   projector window (full screen, stretched)
```

## Frame loop (`NawsMapperApp::ui`)

1. Advance the transport clock (if playing).
2. Read shortcuts and dropped files → queue `Action`s.
3. `Renderer::render(project, time)`: one render pass into the output texture.
4. Lay out the panels and canvas. UI code edits `doc.project` directly.
5. Show the projector viewport (an immediate egui viewport, i.e. a second OS window).
6. Run queued actions.
7. If no pointer button is down, `commit_if_changed()` records an undo snapshot.
8. Update window title, handle close (asks to save), request repaint while playing.

Rendering happens before the UI is drawn, so the preview is at most one frame
behind an edit. That's invisible in practice and keeps the loop simple.

## Rendering

Each visible surface is one draw call of 6 vertices (no vertex buffer). Corners
come from a uniform; the vertex shader converts canvas pixels to clip space.

The fragment shader:

1. Maps the pixel back into the surface's unit square with the **inverse homography**
   (`geom::Homography`, Heckbert's square-to-quad). This gives perspective-correct
   texturing for any four-corner shape. Interpolating UVs across the two triangles
   would not.
2. Computes coverage from the **shape** (`Mask`) as a signed distance in the unit
   square, with `fwidth`-based antialiasing plus optional **feather**. Quad, ellipse
   and triangle have exact formulas. Every other shape, including free-form ones, is
   a polygon of up to 128 points sent in the uniforms (`om_core::shape::outline_uv`)
   and filled with an even-odd polygon distance. Because shapes live in the unit
   square, they follow corner pinning, perspective and Free Transform for free.
3. Chooses the base UV (surface UV for *Fit*, canvas UV for *Span*) and applies the
   animated **UV matrix** (offset/scale/rotation/flip + scroll/spin/zoom), with
   aspect correction so rotation stays rigid on non-square surfaces.
4. Samples the source: image texture (repeat addressing), solid color, or a
   procedural **generator** evaluated in the same UV space, so all motion
   controls apply to patterns too. Each generator (`field()` in the WGSL) returns a
   palette position and an intensity; the **palette** (up to 6 sRGB stops in the
   uniforms, mirrored or cyclic) turns the position into a color, and the intensity
   fades it over color A (the background). The test grid paints its own colors.
5. Hue rotation, tint, opacity (with opacity LFO), then premultiplied output.

**Blend modes.** Modes that fixed-function blending computes exactly (Normal, Add,
Screen, Multiply, Subtract, Exclusion) each have a pipeline with their own blend state
(`fixed_blend`). The other 18 need the color underneath: before drawing such a
surface, `Renderer::draw` ends the render pass, copies the pixels under the surface's
bounding box (`pixel_bounds`) into a **backdrop** texture, and starts a new pass. The
shader reads the backdrop at its own pixel (`textureLoad`), applies the W3C/Photoshop
blend function (`blend()` in the WGSL, selected by `backdrop_index`), and composites
normally. Cost: one small copy and one pass break per such surface. Blending happens
on sRGB-encoded values, like Photoshop's default.

Uniforms for all surfaces go into one buffer with dynamic offsets (1376 bytes per
surface, padded to the device's offset alignment).

**Shader Library thumbnails** use the same pipeline: `Renderer::render_thumbnails`
builds a throwaway project with one 192×120 tile per preset and draws it into a
separate atlas texture, only while the library window is open.

Image sources are decoded with the `image` crate on the UI thread when first seen
or when their path changes.

**Cutouts** (`Surface::cutout`) are drawn like a solid surface whose color is the
canvas background, with normal blending. The mask, feather and opacity give the hole
its shape, so "cut out of everything below" needs no extra pass or stencil. Layer
order decides what a cutout cuts.

## Video

`om-media::VideoPlayer` owns one decoder thread per video source:

- On open, the `moov` box is parsed (`mp4` crate) and a `Timeline` is built from the
  `stts`/`ctts`/`stss` tables: presentation order, frame start times, keyframes.
  B-frames are handled by mapping decode order to display order.
- Each frame, `TextureCache::sync` calls `seek(time)` with the transport time (the
  video loops) and uploads the newest finished frame with `queue.write_texture`.
- The thread decodes forward towards the requested frame (H.264 via OpenH264, YUV to
  RGBA on the CPU). It restarts from the nearest keyframe when the target is behind it
  or past the next keyframe. Only the requested frame is converted to RGBA.
- When a frame is ready it calls the renderer's waker (`ctx.request_repaint`), so
  scrubbing while paused still updates.

Video time is the global transport time, so a source shared by several surfaces
shows the same frame on all of them (per-surface time offset doesn't apply to video).

## File dialogs

All file dialogs are rfd's *async* dialogs (`dialogs.rs`): created on the UI thread,
awaited on a helper thread, result polled once per frame. Blocking dialogs crash or
misbehave on macOS inside winit's event handler. Questions ("Save changes?") and errors
are egui modals (`Prompt`). Actions that would discard changes go through
`NawsMapperApp::guard(Then)`, which asks first and resumes the action after saving.

## Editor interaction

`editor.rs` holds selection (ordered; last = primary), view (zoom/pan, auto-fit),
and the drag state machine: `Pan | Move | Corner | Marquee | Tf* | PenPoint | Anchor`.

- **Hit testing** is front-to-back on unlocked, visible surfaces (point-in-polygon
  on the shape's outline, so clicking beside a star's tip misses it).
- **Pen** (`Editor::pen`): canvas-space anchor points; the finished path becomes a
  surface via `Surface::freeform`, whose corners are the path's bounding box.
- **Edit Points** (`Editor::points`): anchors are stored in the surface's unit square
  (`Surface::path`). Every edit goes through `Surface::move_path_point` /
  `insert_path_point` / `remove_path_point`, which call `fit_frame_to_path`: the four
  corners move onto the outline's bounds *through the current homography* and the
  points are re-expressed in that frame. The shape stays exactly where it was (also
  under perspective), and it never sticks out of the quad that gets drawn.
- **Smooth points** use Catmull-Rom tangents as cubic Béziers (`shape::flatten`), so
  the outline passes through every anchor and needs no handles.
- **Corner handles** exist only on selected surfaces and take priority.
- **Snapping**: candidates are corners of non-moving surfaces plus the canvas corners
  and center, within 10 screen px.
- **Welding**: corners within 0.5 px of the dragged corner (on any unlocked surface)
  move with it. Arrow-key nudging respects welding too. Alt/Option bypasses both.

## Multi-selection editing

`panels/properties.rs` clones the primary surface, shows its controls, and if
anything changed calls `diff::propagate_changes(before, after, other)` for every
other selected surface. The diff runs on `serde_json::Value` trees: objects
recurse, leaves (numbers, arrays such as colors or vectors, enum variants) are replaced.
Result: changing *Opacity* on 12 surfaces leaves their different *Time offsets* alone.

Operations that need different values per surface (e.g. **Stagger**) are explicit
`Document` methods.

## Persistence

- Project files: pretty JSON, extension `.omap`, `format_version` field. Relative
  image paths are resolved against the project file's folder on load.
- App state (window size, last opened project): eframe persistence
  (folder named after the app ID, `~/Library/Application Support/net.nawsmapper.NawsMapper`
  on macOS).

## Known limitations (v0.3)

- Output window placement is manual (drag to projector, then `F`). No monitor picker yet.
- Video is H.264 only (no HEVC/ProRes/HAP yet), no audio, CPU decoding.
  No live camera, Syphon/Spout/NDI yet.
- No mesh warp. Free-form shapes have at most 64 points and no Bézier handles
  (smooth points are automatic). Shapes have no holes (a ring needs a cutout).
- No timeline or keyframes. Animation is parametric (LFO/rates).
- No mipmaps on image textures, so heavily minified images may shimmer.

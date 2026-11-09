# 0010: Mesh warp

- **Status:** done (pending her test on a real curved object)
- **Started / finished:** 2026-11-09 / 2026-11-09
- **Driven by:** [pro-features.md](../pro-features.md) P1-3

## Goal

Surfaces fit curved and uneven objects (columns, arches, round sculptures, fabric):
she drags points of a grid on the surface until the picture lies on the object, and
the content, the shape and its border bend with it.

## Scope

In:
- `Surface::warp: Option<Warp>`: a grid of 2–12 × 2–12 control points, smooth
  (Catmull-Rom through the points) or straight (bilinear).
- Mesh Warp mode (`W`, toolbar **⌗ Warp**, Surface menu, Properties, double-click a
  warped surface): drag points, arrows nudge, Enter/Esc leave; the grid shows on the
  projector with Outlines on projector.
- Properties → Warp: Edit/Done, Reset, Remove, Grid size (keeps the shape), Smooth.
- GPU: tessellated drawing with perspective-correct texturing, cached per surface.
- Everything that uses the outline follows the warp: hit testing, overlays, borders
  (dash spacing, effects), mapping template.

Out (explicitly deferred):
- Bézier handles on mesh points.
- Output-level warp (the whole projector image), needed for multi-projector (P2-1).
- Edit Points on warped surfaces (refused with a message: shape first, then bend).
- Snapping mesh points.

## Design notes

- **Where the mesh lives.** In the surface's unit square, before the corners' homography:
  `canvas = H(warp(uv))`. Shapes, borders and textures are all defined in `uv`, so
  they bend without any code of their own, and corners, Free Transform, snapping and
  welding keep working on the four corners with the mesh riding along (the same trick
  as free-form paths). The untouched grid is a lattice, which maps every point to
  itself; `Surface::active_warp` ignores untouched or invalid grids, so those surfaces
  keep the fast single-quad path.
- **Interpolation.** Uniform Catmull-Rom, with phantom points continuing the grid in a
  straight line past its edges, so the lattice reproduces itself exactly and the
  surface passes through every point (no handles, like smooth path points). Past the
  unit square (border margins) the mesh continues along the edge's slope.
- **Grid corners** are the surface's corners. They are not draggable as mesh points;
  the corner handles keep priority, so pinning works exactly as before.
- **Rendering.** The fragment shader only needs the surface `uv` per pixel. Quads get it
  from the inverse homography; a warped surface can't be inverted cheaply, so it is
  tessellated on the CPU (48×48 cells, `Warp::tessellate`) into vertices with
  homogeneous canvas positions `(x·w, y·w, w)` and `uv`. The GPU then interpolates
  `uv` with the corners' perspective exactly, and the warp's own curvature is
  approximated by the fine grid. `vs_mesh` plus a "mesh" variant of every pipeline;
  `shape.z = 1` tells `fs_main` to take `uv` from the vertex stage. One shared index
  buffer. Grids are cached per surface and re-uploaded only when corners, mesh or margin
  change (a cue crossfade draws some surfaces twice; they share one grid).
- **Outside borders and glow** need pixels beyond the shape: the grid is grown by a
  margin in uv (border extent ÷ shortest corner edge).
- **Multi-edit.** Changing the mesh from Properties goes through actions (Reset, Remove,
  Grid, Smooth) that act on each selected surface's own mesh. The generic multi-edit
  copy would paste the primary's bent grid onto the others.
- **File format.** `warp` is `#[serde(default)]` (missing means none); no version bump.

## Tasks

- [x] `om_core::warp`: model, Catmull-Rom/bilinear map, resize keeping the shape,
      tessellation, grid indices
- [x] `Surface::to_canvas`, warped `outline`/`path_on_canvas`, `border_path` with warp
- [x] Renderer: `vs_mesh`, pipeline pairs, vertex/index buffers, cache, backdrop bounds
- [x] Editor: Mesh Warp mode, dragging, nudging, drawing, projector overlay
- [x] Actions, `W` shortcut, Surface menu, toolbar, Properties rows
- [x] Edit Points refused on warped surfaces
- [x] Tests, smoke test in Xvfb, docs

## Verification

Unit tests (12 new): lattice maps to itself (smooth and straight, inside and past the
edges, all grid sizes); the surface passes through every control point; smooth bends
softer than straight; resizing keeps the shape; invalid grids change nothing and an
empty `warp` deserializes to a valid identity; grid corners recognized; tessellation
covers the surface with positive w under both corner orders and grows by the margin;
index bounds; warped outline and hit area bend and corners stay; untouched grids are
ignored; a bent edge's border length grows; mesh points move on the canvas while
corners stay; mesh hit testing skips corners.

Xvfb (GL backend): a test grid on a 3×3 smooth bulge with an outside glowing border, and
a straight-line arch with an ellipse mask and the Overlay blend mode (backdrop copy)
render correctly; outlines follow both. Mesh mode shows the hint, the bent grid,
draggable points and the corner handles; dragging the center point bends the content
smoothly; undo restores it; the projector shows the grid while bending. After adding the
cache, a drag still changed 15% of the surface's pixels (re-tessellated and uploaded).

## Follow-ups

- Output-level warp for multi-projector (P2-1) can reuse `Warp` and the mesh pipeline.
- Bézier handles for sharper control; snapping mesh points to other surfaces' edges.
- A 12×12 grid gets 4 tessellation cells per grid cell; enough for smooth meshes, could
  be adaptive.

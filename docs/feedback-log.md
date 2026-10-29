# Feedback log

User feedback, newest first. Each entry: date, build/version, what was tried,
what was said (quote where possible), and the decision it led to (link the tracker).

## Template

```
## YYYY-MM-DD: vX.Y (commit abc1234)

Setup: (projector, object, what she tried to make)

What worked:
-

Pain points / wishes (in her words where possible):
-

Decisions:
- → docs/trackers/NNNN-....md
```

## 2026-10-29: rename

Decisions:
- The app is now called **NawsMapper** (binary `nawsmapper`, bundle ID
  `net.nawsmapper.NawsMapper`). The `.omap` extension and the `om-*` crates stay.
- The settings folder follows the name, so the first launch after the rename
  doesn't reopen the last project (old builds used `net.openmapper.OpenMapper`).

## 2026-10-29: v0.3

Pain points / wishes (in her words where possible):
- Liked the Shader Library: "add some more shaders, make them pretty and trippy".
- "Add some more blend modes."

Decisions:
- 14 more shaders (psychedelic, organic, cosmic) and 3 palettes, tuned by eye in the library.
- All of Photoshop's blend modes in Photoshop's order, since that's her reference;
  modes that need the backdrop read it from a copy of the canvas.
  → [trackers/0005-shaders-blend-modes.md](trackers/0005-shaders-blend-modes.md)

## 2026-10-29: v0.2

Pain points / wishes (in her words where possible):
- More shapes than circle, square and triangle; "the ability to create free-form
  shapes from adding anchor points and dragging them around".
- She likes the shaders in HeavyM: wants a shader catalogue with cool, trippy shaders.

Decisions:
- Shapes live in the surface's unit square, so all of them (built-in and free-form)
  keep working with corner pinning and Free Transform. Pen tool + Edit Points with
  automatic curve points (no Bézier handles yet; see how far that gets her).
- Built-in shaders with palettes and a live-preview Shader Library first; ISF import
  later if she wants shaders from elsewhere.
  → [trackers/0004-shapes-pen-shaders.md](trackers/0004-shapes-pen-shaders.md)

## 2026-10-28: v0.1 + Free Transform, first hands-on session

Pain points / wishes (in her words where possible):
- "Mask -> Cutout from other shapes"
- "Save + Save as + Import image are not working right now"
- "Needs import video (mp4)"

Findings:
- Save/Import: all file dialogs were blocking rfd dialogs called inside winit's event
  handler. On macOS that nested modal loop can re-enter winit, which treats it as fatal.
  On the Linux dev box, rfd's default backend needs xdg-desktop-portal or zenity
  (neither installed), so dialogs failed silently.

Decisions:
- Cutouts as a surface flag ("cut out of layers below"), ordered by the layer stack.
- Async file dialogs + in-app prompts on every platform; GTK3 dialogs on Linux.
- H.264 video via OpenH264 (cross-platform, testable on Linux); AVFoundation later.
  → [trackers/0003-cutouts-video-dialogs.md](trackers/0003-cutouts-video-dialogs.md)

## 2026-10-28: v0.1, first question

- "How do I change the size of a shape?" Only corner dragging existed; no way to scale a
  shape (or several) as a whole.

Decisions:
- Build Photoshop-style Free Transform next.
  → [trackers/0002-free-transform.md](trackers/0002-free-transform.md)

## 2026-10-28: background (before v0.1)

- Tried **MadMapper**: animation features not rich enough; specifically missed the
  ability to animate textures.
- Tried **HeavyM**: liked it overall; handling of multiple shapes at once felt cumbersome.
- UX design background, long-time Photoshop user, new to projection mapping, on macOS.

Decisions:
- v0.1 focuses on parametric texture animation on every surface plus field-level
  multi-selection editing, with Photoshop-like conventions.
  → [trackers/0001-foundation.md](trackers/0001-foundation.md)

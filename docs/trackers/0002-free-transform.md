# 0002: Free Transform

- **Status:** done
- **Started / finished:** 2026-10-28 / 2026-10-28
- **Driven by:** [feedback-log.md](../feedback-log.md), "How do I change the size of a shape?"

## Goal

Resize, rotate and move one or many surfaces as a group with the gesture a Photoshop user
already knows: `Cmd+T`, drag handles, `Enter`.

## Scope

In:
- `Cmd+T` / Surface menu / toolbar **Transform** button on the current selection.
- Bounding box with 8 handles. Corner = both axes, side = one axis. Shift = proportional,
  Alt/Option = from center.
- Drag outside to rotate around the box center; Shift snaps the absolute angle to 15°.
- Drag inside to move (with corner snapping, Alt bypasses); arrow keys nudge.
- Toolbar readout (W %, H %, angle) with Apply / Cancel.
- `Enter`, double-click inside, `Cmd+T` again or Apply → keep. `Esc` / Cancel → restore.
- Selection changes, undo/redo, delete, duplicate, new/open → apply first.
- Locked surfaces are excluded.

Out: numeric entry in the readout, Skew/Distort/Perspective sub-modes, a movable pivot,
snapping handles to corners while scaling.

## Design notes

- `FreeTransform` stores the original corners plus one `glam::Affine2`. Each gesture
  rebuilds the transform from its value at drag start, so nothing drifts or accumulates.
- Scaling multiplies on the right (local frame), rotation on the left (canvas frame).
  The transform therefore stays rotation × scale (no shear), and the handles stay
  attached to the rotated box.
- Scale is clamped to ±1 % so the transform stays invertible (negative = flip is allowed).
- Every drag is one undo step via the existing commit-on-release. Cancel restores the
  originals, which is itself undoable.
- In transform mode, clicks never change the selection, and per-corner handles are hidden.

## Tasks

- [x] `transform.rs`: state + `scaled` / `rotated` / `moved` (+ 8 unit tests)
- [x] Editor: drag variants, hit testing, direction-aware resize cursors, box drawing, hint line
- [x] Actions + shortcuts (`Cmd+T`, Enter, Esc), menu item, toolbar readout
- [x] Docs: README, user guide, roadmap, feedback log, AGENTS

## Verification

- `cargo test`: 33 tests pass (8 new in `transform.rs`), clippy `-D warnings` clean.
- Checked by hand in an isolated Xvfb session (GL backend): select all → Cmd+T shows the
  box and readout; Shift-dragging the bottom-right handle gives 124 % × 124 %; dragging
  outside rotates the group 22° with the box following; Esc restores the original geometry.
- Not yet checked: the macOS trackpad, and on a real projector.

## Follow-ups

- Type exact W/H/angle values in the toolbar readout.
- Perspective/Distort modes: dragging a box corner warps the whole group's corners.
- Optional "always show transform controls" (Photoshop's Move-tool checkbox).

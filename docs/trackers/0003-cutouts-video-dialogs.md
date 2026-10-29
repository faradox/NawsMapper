# 0003: Cutouts, video, working file dialogs

- **Status:** done (pending her test on macOS)
- **Started / finished:** 2026-10-28 / 2026-10-28
- **Driven by:** [feedback-log.md](../feedback-log.md), first hands-on session

## Goal

Her three requests after the first session: cut shapes out of other shapes, make
Save / Save As / Import work, and play MP4 videos.

## Scope

In:
- `Surface::cutout`: a surface that paints the canvas background over everything below
  it. Shape, feather, opacity and opacity animation apply. Toolbar **✂ Cutout**,
  Surface → Add Cutout, Properties checkbox (multi-edit works), dashed pink outline,
  "✂ Cutout" in Layers.
- Async file dialogs (`dialogs.rs`) for Open, Save As, Import, Replace; egui modals
  for "Save changes?" and errors; `guard(Then)` resumes New/Open/Quit after saving.
- rfd's GTK3 backend on Linux.
- `SourceKind::Video` + new crate `om-media`: MP4/MOV demux, H.264 decode on a thread,
  B-frames, looping, seeking; frames follow the transport clock. Import via dialog and
  drag & drop. Source panel shows size, length and errors; broken sources get a ⚠.
- Clear error for HEVC / ProRes / HAP / AV1 / VP9 files (codec sniffed from the file).
- `nawsmapper path.omap` opens a project from the command line.

Out: audio, HEVC/ProRes (AVFoundation backend), per-source playback speed / in-out,
GPU YUV conversion, inverted cutouts, cutting only specific layers.

## Design notes

- **Cutout = solid surface in the background color**, normal blending. No stencil or
  extra pass, and anything that shapes coverage (mask SDF, feather, opacity LFO) shapes
  the hole. Layer order decides what gets cut, which matches Photoshop thinking.
  Limitation: with a non-opaque canvas background the hole shows that color, not
  transparency (the output is always opaque today, so this doesn't show).
- **Why Save/Import failed:** blocking `rfd::FileDialog`/`MessageDialog` inside winit's
  event handler. On macOS the modal's nested run loop can re-enter winit's handler,
  which winit treats as fatal re-entrancy. On Linux without a portal or zenity the
  dialogs returned `None` silently. Now dialogs are created on the UI thread
  (`AsyncFileDialog::pick_file()` etc., a sheet on macOS), awaited with `pollster` on a
  helper thread and polled once per frame.
- **Video decoding:** `openh264` (built from source, so nothing to bundle) + `mp4`.
  `Timeline` maps decode order to display order (B-frames) using `stts`/`ctts`/`stss`.
  The decoder thread feeds one sample per loop iteration so a new target is picked up
  quickly, seeks to a keyframe when the target is behind or past the next keyframe, and
  converts only the requested frame to RGBA.
- Shared video sources show the same frame on every surface. Per-surface time offset
  doesn't apply to video (it would need one decoder per surface).

## Tasks

- [x] `om-core`: `Surface::cutout`, `SourceKind::Video`, `media_path()`, relative video paths on load (+ test)
- [x] `om-render`: cutout uniforms (+ test), video textures with streaming upload, frame waker
- [x] `om-media`: `Timeline` (+ 3 tests), `VideoPlayer`, AVCC→Annex B (+ test), codec sniffing
- [x] `om-media` integration tests on a 30-frame B-frame fixture (forward, backward, loop, errors)
- [x] `dialogs.rs`: async file dialogs, prompts; rewire Open/Save/Save As/Import/Replace/Quit/drop
- [x] UI: toolbar/menu cutout, Properties cutout toggle (hides tint/blend/texture), Layers hint,
      dashed outlines, "+ Media…", video info, ⚠ for broken sources, black for failed media
- [x] Docs: README, user guide §4/§8/§9, architecture, platforms, AGENTS, roadmap, feedback log

## Verification

- `cargo test`: 44 tests pass (11 new). `cargo clippy --all-targets -D warnings` and `cargo fmt --check` clean.
- Xvfb (GL backend) with a test project: 720p H.264 MP4 and MOV play and loop; a
  feathered ellipse cutout and a square cutout cut through video and pattern layers; a
  new cutout from the toolbar cuts the three layers under it; the HEVC file shows its
  error in Sources with ⚠ and renders black (not white) on the canvas.
- Unsaved-changes modal on Cmd+N; Enter saves to the existing file and then continues.
- GTK Save As dialog: saved to a path without extension, `.omap` was appended, title updated.
- GTK Import dialog: imported a `.mov` as a video source.
- **Not yet verified:** macOS. The dialog fix there follows rfd's documented async path
  but has only been compiled by CI, not clicked. Needs her test on the Mac. Performance
  of 1080p/4K software decoding on her Mac is also untested.

## Follow-ups

- AVFoundation decoder on macOS (HEVC, ProRes, hardware decode); HAP.
- Per-source playback: speed, in/out points, ping-pong, start offset.
- Inverted cutout (show only inside), "cut only these layers".

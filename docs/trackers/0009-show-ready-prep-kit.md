# 0009: Show-ready output, Show mode, prep kit

- **Status:** done (pending her test on macOS with a projector)
- **Started / finished:** 2026-11-09 / 2026-11-09
- **Driven by:** [pro-features.md](../pro-features.md) P1-1 and P1-2

## Goal

A pro can take NawsMapper to a job. Pick the projector from a list and the output goes
full screen there, pixel for pixel. Show mode locks the editor so nothing on the
projector changes by accident, with a Blackout fade and a Freeze button. A crash doesn't
lose the work. Before the job, she can prepare at home: map over a photo of the venue,
export a template of the surfaces for designing content in Photoshop or After Effects,
and package the project with all its media to move it to the show computer.

## Scope

In:
- **Projector picker**: Output → Projector lists the connected displays (name,
  resolution); the output goes borderless full screen on the chosen one. The choice is
  remembered by display name and reapplied when that display is connected.
- **Pixel-exact check**: when the canvas and the projector differ in size, a warning and
  **Match Canvas to Projector**, which resizes the canvas and scales the surfaces so the
  projected picture stays where it was.
- **Show mode** (`Cmd+Shift+Enter`): the window becomes a show view (preview, clock,
  Blackout, Freeze, Exit); editing, overlays and the cursor on the projector are off;
  Esc, F and double-click on the output do nothing; quitting asks first.
- **Blackout** (`B` in Show mode, Output menu): fade the projector to black and back,
  with an adjustable fade time.
- **Freeze**: the projector holds the current frame while the editor keeps working live.
- **Images decode on background threads**, so an import never stalls the projector.
- **Autosave and recovery**: unsaved changes are written to a recovery file every 30 s;
  after a crash the next start offers to restore them.
- **Export Mapping Template** (PNG or SVG): surface outlines, numbers and names at
  canvas resolution, transparent background.
- **Venue photo**: a photo shown behind the surfaces in the editor only, with opacity and
  fit, saved with the project.
- **Package Project**: copy the project and every image, video and venue photo into a
  new folder, with relative paths.

Out (explicitly deferred):
- Several outputs at once, edge blending (P2-1).
- A master dimmer, white-out, timed blackouts, cues (P1-4).
- Layered PSD template export.
- Identifying displays by flashing a number on each one.

## Design notes

- **Displays.** egui 0.37 can put a viewport in borderless full screen on a monitor by
  index (`ViewportCommand::SetMonitor`), in winit's `available_monitors()` order. The
  root window from `eframe::Frame::winit_window()` gives the same list with names and
  sizes. The wgpu backend ignores `ViewportBuilder::with_monitor` when creating a child
  window, so we send `SetMonitor` (commands sent before the window exists are deferred).
  The choice is stored as name plus the ordinal among displays with that name, so two
  identical projectors stay apart. App-wide setting, not project data: a project moves
  between computers, display names don't.
- **Blackout** is a tint on the image in the output window, not part of the rendered
  texture. The editor keeps showing the content with a "Projector is black" banner. That
  bends design rule 1 deliberately, and is documented in architecture.md.
- **Freeze** copies the output texture once (`Renderer::freeze`) and the projector shows
  the copy, so you can freeze, fix something, and unfreeze without the audience seeing
  the edit.
- **Match Canvas to Projector** scales corners by new/old size per axis. The output used to
  stretch the canvas to the projector, so the projected geometry stays identical.
- **Venue photo** lives in `Canvas::backdrop` (`#[serde(default)]`, no format bump). The
  renderer never sees it; the editor draws it over the output texture at its opacity,
  below the outlines.
- **Template PNG** is rasterized on the CPU with tiny-skia; text uses ab_glyph with egui's
  bundled Ubuntu font, so it needs no system fonts. SVG groups are named after the
  surfaces, so Illustrator/Affinity show one layer per surface.
- **Package** works out a plan as a pure function (unique file names, relative paths,
  missing files), then copies on a thread. The open document is not changed.
- **Recovery** file: `recovery.json` in eframe's storage folder, holding the project and
  its original path. Deleted on save, on a clean quit, and when changes are discarded.

## Tasks

- [x] Core: `Canvas::backdrop`, path resolution on load, `Project::resize_canvas`
- [x] Render: threaded image decoding, `freeze`/`unfreeze`
- [x] Displays: list, resolve the remembered choice, Output menu, toolbar
- [x] Canvas/projector mismatch warning, Match Canvas to Projector
- [x] Show mode view, key guard, quit confirmation
- [x] Blackout with fade, Freeze
- [x] Autosave and recovery prompt
- [x] Template export PNG + SVG
- [x] Venue photo: load, draw, Properties section, View toggle
- [x] Package Project
- [x] Unit tests for the pure parts
- [x] Smoke test in Xvfb (one monitor: Xvfb has a single RandR output)
- [x] Docs: README, user guide, architecture, AGENTS.md rule 1, roadmap, pro-features status
- [ ] On her Mac with a real projector (see Verification)

## Verification

Unit tests (all in `cargo test`, 24 new, 102 in total):
- `om_core::project`: backdrop path resolved on load, absolute paths kept, old files
  get an empty backdrop, `media_paths_mut` covers all media, `resize_canvas` keeps the
  projected picture, `BackdropFit::place` for Fit/Fill/Stretch.
- `displays`: choice follows its display when others are plugged in, identical
  projectors stay apart, descriptions.
- `output`: fade timing, zero fade cuts, size mismatch only with a connected projector,
  "Window" leaves full screen, re-picking re-places.
- `app`: projector choice and fade survive a restart (in-memory eframe storage). This
  caught a real bug: the choice was saved as `Option<…>` but read as the bare type.
- `template`: Layers numbering, label placement, SVG groups/escaping/cutout dashes,
  PNG size, transparency, outline color, fill, label pixels.
- `package`: unique names (case-insensitive), shared files copied once, relative paths,
  missing files reported, safe unique folder names, a written package loads again.
- `recovery`: written while dirty, removed after saving (ordered worker), interval,
  age wording.

Smoke test on Linux in Xvfb (GL backend, i3 on the virtual display only, scratch
`XDG_DATA_HOME`), with a scene holding a 6000×4000 PNG and a venue photo:
- The 124 MB image decoded in the background; the UI kept 60 fps.
- Venue photo drawn at 50 % with Fit letterboxing; not on the projector.
- Output → Projector listed the display; picking it put the output borderless full
  screen at 1600×1000. Esc left full screen; entering Show mode put it back.
- Canvas 1920×1080 vs display 1600×1000: warning in Properties and the toolbar; Match
  Canvas to Projector made it 1600×1000 with the surfaces scaled.
- Show mode: show bar, preview, key hints; outlines gone from the projector.
  Blackout faded over 1 s ("FADING", then "BLACKOUT", projector black). Freeze: 0
  changed pixels on the projector over 1.5 s while the scene animated; the editor
  showed the banner.
- Closing the window in Show mode was cancelled and asked "Quit during the show?".
- Killed the app with unsaved changes; the next start offered to restore them, and
  Restore brought back the 1600×1000 canvas as an unsaved document.
- Export Mapping Template through the GTK dialog: PNG 1600×1000 RGBA, SVG valid XML.
- Package Project through the GTK folder dialog: project with `Media/…` paths, both
  media files copied.

Not verified here (needs her Mac and a projector):
- [ ] The projector shows up by its name and the output lands on it, also after
      unplugging and replugging.
- [ ] Frame rate on the projector while editing on the laptop screen (two vsynced
      windows in one loop).
- [ ] Show mode's key guard with macOS's own shortcuts (Cmd+Q, Cmd+H, Cmd+Tab).

## Follow-ups

- Opening a video (parsing its index) still happens on the UI thread. Fast for normal
  files; a thread like images would make it stall-free too.
- Match Canvas to Projector scales corners but not border widths (canvas pixels).
- Two NawsMapper windows running at once share one recovery file.
- The show bar crowds at narrow window widths (fine at the default 1440 px).
- The app's own log lines were filtered out by default (target `om_app` instead of the
  binary's `nawsmapper`); fixed along the way.
- Deferred: layered PSD template, flashing a number on each display to identify it.

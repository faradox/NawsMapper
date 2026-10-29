# Roadmap

Ordered by expected value for the primary user. Real feedback reorders this
list. Record it in [feedback-log.md](feedback-log.md) first.

## ✅ v0.1: Foundation (done)

Four-corner perspective surfaces (quad/ellipse/triangle masks), snapping and welded
corners, multi-select with field-level multi-edit, layers, sources (color, 7 GPU
patterns, images), Fit/Span mapping, parametric texture animation (scroll, spin, hue,
zoom/opacity LFOs, presets, stagger), blend modes, feather, projector window, undo,
save/load. Tracker: [trackers/0001-foundation.md](trackers/0001-foundation.md).

## ✅ Free Transform (done)

`Cmd+T` bounding box: scale (Shift proportional, Alt from center), rotate (Shift 15°),
move, Enter/Esc. Tracker: [trackers/0002-free-transform.md](trackers/0002-free-transform.md).

## ✅ v0.2: Cutouts, video, working dialogs (done)

Cutout surfaces, H.264 video sources (MP4/MOV), async file dialogs (fixes Save /
Save As / Import), in-app save prompts. Tracker:
[trackers/0003-cutouts-video-dialogs.md](trackers/0003-cutouts-video-dialogs.md).

## ✅ v0.3: Shapes, Pen, Shader Library (done)

11 new built-in shapes, free-form shapes (Pen + Edit Points), 18 new shaders with
palettes and a live-preview Shader Library. Tracker:
[trackers/0004-shapes-pen-shaders.md](trackers/0004-shapes-pen-shaders.md).

## ✅ v0.4: More shaders, Photoshop blend modes (done)

14 new shaders (42 presets), 3 new palettes, all 24 Photoshop blend modes with
`Shift +/-` cycling. Tracker: [trackers/0005-shaders-blend-modes.md](trackers/0005-shaders-blend-modes.md).

## Next candidates (pick from feedback)

### Output & show workflow
- Monitor picker: send output to a chosen display in one click, remember it per project.
- Multiple outputs (two projectors), with edge blending later.
- "Show mode": hide the editor overlays and lock everything.
- Syphon (macOS) / Spout (Windows) / NDI output and input.

### Media
- **Video, next steps**: AVFoundation backend on macOS (HEVC, ProRes, hardware decode),
  HAP, playback speed / in-out points / ping-pong per source, GPU YUV conversion,
  thumbnails. Audio.
- **Cutout options**: invert (show only inside), cut only specific layers, groups.
- GIF animation, image sequences.
- Live camera input.
- Mipmaps for images (smooth minification).
- Media thumbnails in the Sources panel.

### Animation (the main differentiator)
- **Timeline with keyframes** for any property (opacity, corners, colors…), with easing
  curves. Clips and cues.
- Tempo: BPM, tap tempo, LFO rates in beats, audio-reactive modulation (FFT bands).
- Modulation matrix: route any LFO/audio band to any parameter.
- Transitions between sources (fade, wipe, dissolve).
- Scenes/cues: snapshot surface states and crossfade between them.

### Geometry
- Bézier handles on free-form points; shapes with holes; boolean shape operations.
- Mesh warp (grid subdivision) for curved objects.
- Groups (Photoshop-style layer folders) with group transforms.
- Numeric entry in the Free Transform readout (type W/H/angle), Distort/Perspective modes.
- Align and distribute.

### Effects
- Per-surface effect stack (blur, edge glow, kaleidoscope, color grading, invert).
- Import ISF shaders (what HeavyM uses), so she can bring shaders from the web; a
  WGSL editor with live preview. Favorites and user presets in the Shader Library.
- More library shaders driven by her picks; audio-reactive shader parameters.

### macOS polish
- Native menu bar (`muda`) and app icon, file associations (`.omap` double-click).
- Notarized, signed builds.
- Trackpad gestures refinement.

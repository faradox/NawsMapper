# What professionals need: research and priorities

Researched 2026-11-09. The question: which features do professional projection mappers
rely on that NawsMapper lacks? The answer feeds [roadmap.md](roadmap.md). Pick items by
number (e.g. "do P1-3").

**Method.** I read the official documentation of the tools pros use: MadMapper 6 (the
dedicated mapper), Resolume Arena (VJ tool with mapping), Millumin 5 (theatre and show
control). HeavyM (closest to our audience) blocks automated reading, so it is covered
through search results only. I added practitioner guides on show days and permanent
installs, plus forum threads. Most sources are vendors describing their own product.
That's fine for "what exists", but says nothing about quality.

## A strong signal: what competitors charge extra for

Vendors put their "pro" features behind the expensive edition. That tells us what
professionals pay for, and it is exactly where a free tool stands out.

- **Resolume:** output warping, edge blending, DMX and capture cards are Arena only
  (about €800). The cheaper Avenue doesn't have them.
- **HeavyM:** edge blending, startup automation, remote project changes and video
  export are Pro+ only. Pro and Pro+ cost about $500–600.
- **MadMapper** (€479) includes everything in one edition and leads on 3D, scanning and
  LED/DMX.

## Where NawsMapper stands

| Area | What pro tools do | NawsMapper today |
| --- | --- | --- |
| Output | Pick the display, native resolution, several outputs, per-output color/brightness/delay, test card, saved output setups (Resolume) | One window you drag to the projector and press `F`; the canvas is stretched to the window size |
| Warping | Perspective + grid mesh + Bézier mesh per surface *and* per output (MadMapper "unlimited points Bezier meshwarping", Resolume linear/Bézier, Millumin mesh) | Four-corner perspective only (free-form outlines exist, but don't warp the content) |
| Multi-projector | Overlap + soft edge with gamma/power/luminance and black-level compensation (Resolume, Millumin, HeavyM Pro+) | None |
| Show control | Cues (parameter snapshots) with animated transitions, columns/scenes, keyframe timelines with audio, markers, follow actions, MTC/LTC timecode (MadMapper 6, Millumin) | Parametric animation only; one global play clock |
| Video | HAP (GPU decoding, the industry standard for multi-layer playback), ProRes, HEVC, hardware decoding, alpha, image sequences, audio | H.264 only, decoded on the CPU, no audio |
| Interop | Syphon/Spout/NDI in and out, capture cards, live cameras | None |
| External control | MIDI in/out with learn, OSC (+ OSCQuery), Art-Net/sACN, game pads, Stream Deck; Millumin has scripting | None (keyboard only) |
| Tempo / audio | Audio analysis (FFT bands), beat detection, Ableton Link | None |
| Installation | Calendar scheduling (MadMapper reads ICS), silent start, startup automation, projector control (PJLink), backup/recovery, export the show to a video file | Reopens the last project on launch |
| 3D / calibration | OBJ import, 3D calibration (match a model to reality), structured-light "Space Scanner" with a camera (MadMapper) | None |
| Light | DMX fixtures, LED pixel mapping, lasers (MadMapper, Resolume Arena) | None |
| Prep | Export the slice layout as an image for content designers (a feature Resolume users ask for), SVG import, mapping presets | None |
| Organisation | Surface groups, slice presets | Layers, multi-select; no groups |

What we already do better than most: texture animation without keyframes, staggered
chases, field-level multi-edit, Photoshop conventions, 60 built-in shaders with palettes,
animated borders, all 24 blend modes, and the price.

## Prioritized list

Sizes are relative: **S** about one working session, **M** two or three, **L** a week of
sessions, **XL** more. They are guesses until each one gets a tracker.

### P1: Show-ready (without these, a pro can't take it to a job)

**P1-1. Output setup and Show mode** (M) — ✅ done, [tracker 0009](trackers/0009-show-ready-prep-kit.md)
- Pick the projector from a list of connected displays; the output goes full screen
  there, and the choice is remembered per display name.
- Pixel-exact output: render at the projector's native resolution, no stretching to the
  window. Today `output.rs` scales the canvas texture to the window rectangle, so a
  canvas/projector mismatch blurs or distorts lines.
- Show mode: one switch that locks editing, hides every overlay and the cursor on the
  output, and guards against stray clicks and keys. Plus a **Blackout/Fade** panic
  button and a freeze-frame button.
- A steady output: the projector window is drawn in the editor's frame loop, and images
  are decoded on that thread (`TextureCache::load_image`). Decoding on a thread and
  checking frame pacing under load keeps an import or a modal from stuttering the
  projection.
- Autosave and crash recovery.
- *Why first:* every job depends on it, it's the base for P2-1, and it's the cheapest
  item on this list.

**P1-2. Prep kit for designers** (S) — ✅ done (PSD export deferred), [tracker 0009](trackers/0009-show-ready-prep-kit.md)
- **Export mapping template**: a PNG (and SVG) at canvas resolution with every surface's
  outline, name and number, so content can be designed to fit exactly in Photoshop or
  After Effects. Optionally a layered PSD (one layer per surface).
- **Venue photo backdrop**: a photo of the building or object behind the canvas, shown
  only in the editor, for preparing a mapping at home before the site visit.
- **Collect media**: copy every image and video into a folder next to the project
  (like InDesign's Package), so it moves to the show computer without missing files.
- *Why:* this is her Photoshop world, cheap to build, and pros use these on every job.

**P1-3. Mesh warp** (M–L) — ✅ done (per surface; output-level warp comes with P2-1), [tracker 0010](trackers/0010-mesh-warp.md)
- Grid warp per surface (2×2 up to e.g. 16×16 points), Bézier handles for smooth
  curves, add rows and columns, reset. Then the same warp on a whole output (needed by P2-1).
- *Why:* curved and uneven objects (columns, arches, round sculptures, fabric) can't be
  mapped with four corners. Every competitor has it, and it is on our roadmap already.
- *Design note:* today a surface is one draw of two triangles with an inverse homography
  in the fragment shader. A warped surface becomes a subdivided mesh with UVs per vertex
  (Catmull-Rom/Bézier patch evaluated in `om-core`, testable without a GPU). Shapes,
  borders and feather stay in UV space, so they follow the warp.

**P1-4. Scenes and cues** (M) — ✅ done, [tracker 0011](trackers/0011-cues.md)
- Save the state of all (or selected) surfaces as a **cue**. A cue list with **GO**
  (Space or a click) and crossfade times; follow-on cues ("go to the next after 8 s").
- *Why:* that's how a show is run (theatre, events, a facade loop). It also fits
  "animation first": a cue changes palettes, speeds and sources, and the LFOs keep moving.
- *Design note:* `om_core::diff` already walks a surface as a JSON tree. A crossfade
  interpolates the numeric leaves between two trees. The same "parameter path" scheme
  (`surfaces/<id>/opacity`) can later address parameters for OSC/MIDI (P2-3) and
  keyframes (P2-6), so design it once here.

### P2: The pro tier (what competitors charge extra for)

**P2-1. Multiple projectors and edge blending** (L) — needs P1-1, P1-3
- Several outputs, each showing a chosen region of the canvas (Resolume's input map)
  with its own warp (output map).
- Soft edge on the overlaps with gamma, power and luminance curves, plus black-level
  compensation outside the overlap. Per-output brightness, contrast, RGB and delay.
- A test card per output, an identify mode, and saved output setups.
- *Why:* anything bigger than one projector's throw (facades, wide walls, domes) needs
  it. It's the headline Pro+/Arena feature, so being free here stands out most.

**P2-2. Pro video** (L)
- macOS: AVFoundation decoding (ProRes, HEVC, hardware decoding, alpha). Content made in
  After Effects and iPhone footage arrive as ProRes/HEVC; today they are rejected.
- **HAP** on every platform: an open codec (snappy-compressed DXT textures) that goes
  straight to the GPU. It's the standard for many layers of HD/4K video.
- YUV→RGB on the GPU, audio playback with video, image sequences, per-source speed /
  in-out points / ping-pong (already on the roadmap).

**P2-3. MIDI and OSC control** (M)
- **MIDI learn**: right-click any slider or button and move a knob. OSC address for
  every parameter (via the P1-4 parameter paths), OSC out for feedback. Cue GO from MIDI/OSC.
- Later: Art-Net/sACN input so a lighting desk can run cues (theatre).
- *Why:* pros run shows from controllers, lighting desks, QLab or TouchDesigner.
  Crates: `midir` (CoreMIDI on macOS), `rosc`.

**P2-4. Tempo and audio reactivity** (M)
- Tap tempo and BPM; LFO rates and chases in beats; Ableton Link.
- Audio input (microphone or line) split into bass/mid/treble bands that can drive
  opacity, zoom, hue or a shader's detail.
- *Why:* clubs, concerts, VJ work. It's the natural next step for our animation
  strength, and MadMapper and Resolume both have it.

**P2-5. Syphon / Spout / NDI** (M–L)
- Receive video from Resolume, TouchDesigner, VDMX or After Effects as a source, and send
  our output to them. Syphon (macOS) first, Spout on Windows, NDI over the network.
- *Note:* Syphon and Spout need GPU texture sharing (Metal/IOSurface, DX11) next to
  wgpu. NDI's SDK is proprietary; check its license before bundling.

**P2-6. Keyframe timeline** (L) — builds on P1-4's parameter paths
- Keyframes on any parameter with easing curves, an audio track with a waveform to cut
  to, markers. Later MTC/LTC timecode to follow QLab or a lighting desk.
- *Why:* MadMapper 6's headline feature; theatre and music-synced shows need it.

**P2-7. Installation mode** (M)
- Start at login straight into Show mode with the chosen project, a schedule
  (on/off times per weekday), a watchdog that restarts after a crash, and PJLink
  projector power on/off.
- **Export the show to a video file** for a cheap player or a projector's USB port
  (OpenH264 can encode as well as decode, so this fits the current dependencies).
- *Why:* museums, shops, permanent facades. HeavyM charges Pro+ for this.

### P3: Larger bets, after feedback

- **P3-1. Groups** (M): layer folders with group opacity, transform and animation.
  Essential once a facade has 80 surfaces.
- **P3-2. Live camera and capture cards** (M): AVFoundation cameras, Blackmagic devices.
- **P3-3. Remote alignment from a phone or tablet** (M): a small built-in web page to
  pick a surface and nudge corners while standing at the object. MadMapper sells
  hardware (MiniMad) for this.
- **P3-4. 3D object mapping** (XL): import an OBJ, match the virtual projector to the
  real one by clicking 6+ known points, render the model with its textures from the
  projector's view. disguise/MadMapper territory.
- **P3-5. Camera-assisted calibration** (XL): project structured-light patterns and film
  them with a webcam to find surfaces and corners automatically, and later blend
  projectors automatically.
- **P3-6. DMX / LED pixel mapping** (L): sample the canvas into Art-Net fixtures. It's a
  different market (lighting), so it stays low unless asked for.
- Smaller polish that can ride along with other items: numeric corner entry, align and
  distribute, solo a surface while aligning, SVG import as shapes, mipmaps.

## Suggested order

1. **P1-1 Output setup and Show mode**, then **P1-2 Prep kit**. Both are quick and make
   every job safer.
2. **P1-3 Mesh warp**: her objects get rounder, and P2-1 needs it.
3. Ask her what her next real job looks like, then choose:
   - one projector, a show to run → **P1-4 Cues**, then **P2-3 MIDI/OSC**
   - music or club → **P2-4 Tempo/audio**
   - a big wall or facade → **P2-1 Multi-projector**
   - content from After Effects → **P2-2 Pro video**

## Sources

- MadMapper features and documentation: [madmapper.com/madmapper/features](https://madmapper.com/madmapper/features),
  [docs.madmapper.com](https://docs.madmapper.com/), [Video Surfaces](https://docs.madmapper.com/madmapper/6/discover-the-interface/video-surfaces),
  [Mesh Warping tutorial (PDF)](https://madmapper.com/files/05-Mesh%20Warping.pdf),
  [Media formats and codecs](https://docs.madmapper.com/madmapper/6/3.-media/media-formats-and-video-codecs)
- MadMapper 6 coverage: [CDM: timelines, markers, clips](https://cdm.link/madmapper-6-brings-timelines-markers-clips-more/)
- Resolume manual: [Advanced Output](https://resolume.com/support/en/advanced-output),
  [Input maps](https://resolume.com/support/en/input-maps),
  [Output transformation](https://resolume.com/support/en/output-transformation),
  [Edge blending](https://resolume.com/support/en/edge-blending), [DMX](https://resolume.com/support/en/dmx),
  [Screens](https://resolume.com/support/en/screens)
- Resolume forum: [request for slice-layout image export like MadMapper's](https://www.resolume.com/forum/viewtopic.php?p=43179),
  [edge blending discussion](https://www.resolume.com/forum/viewtopic.php?p=37520)
- Millumin: [millumin.com](https://www.millumin.com/), [Outputs](https://help.millumin.com/docs/display/outputs/),
  [Mapping](https://help.millumin.com/docs/display/mapping/), [Soft-edge](https://help.millumin.com/docs/display/softedge/),
  [Interactions](https://help.millumin.com/docs/connect/interactions/), [Timeline](https://help.millumin.com/docs/playback/timeline/)
- HeavyM (via search results; the site blocks automated reading): [product page](https://www.heavym.net/product-new/),
  [HeavyM 2.5 update](https://www.heavym.net/heavym-2-5-update/), [HeavyM Server](https://www.heavym.net/heavym-server/)
- Comparisons and practice: [MadMapper vs Resolume vs TouchDesigner (r/TouchDesigner)](https://reddit.birdcat.cafe/r/TouchDesigner/comments/1l3n1v9/need_advice_upgrading_software_tdmadmapperresolume),
  [Which live visuals software (alltd.org)](https://alltd.org/?p=61918),
  [AVFX projection mapping best practices](https://www.avfx.com/2019/11/projection-mapping-best-practices/),
  [HeavyM: how to prepare your projection mapping](https://www.heavym.net/how-to-prepare-your-projection-mapping-in-2026/)
- HAP: [Green Hippo FlexRes/HAP performance](https://www.manula.com/manuals/green-hippo/hippotizer-v4/4.7.3/en/topic/flexres-performance),
  [HAP on projection-mapping.org](https://projection-mapping.org/?p=1116)
- Installations: [VIOSO EXAPLAY media server guide](https://vioso.com/news/exaplay-3-mondo-dr-media-server-guide/),
  [permanent installation planning](https://42av.pages.dev/projection-mapping)
- Open-source peers: [Splash](https://projection-mapping.org/tools/splash/), [MapMap](https://sourceforge.net/app/mapmap/),
  [ofxPiMapper](https://ofxpimapper.github.io)

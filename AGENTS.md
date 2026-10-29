# AGENTS.md

Guidance for anyone, human or coding agent, working on NawsMapper. Read
this first, then [docs/architecture.md](docs/architecture.md).

## Product context

- **Who it's for:** a designer with a UX background and years of Photoshop,
  new to projection mapping. She learns fast. She tried MadMapper (animation
  too limited) and HeavyM (handling many shapes at once is cumbersome).
- **What wins:** rich, *easy* texture animation; effortless multi-shape editing;
  an interface that feels like a pro creative tool (Photoshop conventions:
  layers, blend modes, Cmd+Z/D/[/], Space-to-pan, panels with collapsible sections).
- **How we build:** small, working increments. Each one is shipped to her,
  her feedback goes into [docs/feedback-log.md](docs/feedback-log.md), and that
  decides the next step. Prefer finishing one feature well over starting three.
- **Primary platform: macOS** (Apple Silicon and Intel). Development happens on
  Linux; Windows is a bonus. Never add a Linux-only dependency without a macOS story.

## Workspace

| Crate | Purpose | May depend on |
| --- | --- | --- |
| `om-core` | Document model (`Project`, `Surface`, `Source`), shapes and free-form paths (`shape`), shader presets (`catalog`), perspective math, animation evaluation, undo history, multi-edit diff. Pure Rust, no GPU/UI. | glam, serde |
| `om-media` | Video decoding: MP4 demux (`mp4`) + H.264 (`openh264`, built from source) on a thread per video. No GPU/UI. | mp4, openh264 |
| `om-render` | wgpu compositor. Renders a `Project` at time *t* into one `Rgba8Unorm` texture. WGSL in `surface.wgsl`. Uploads video frames. | om-core, om-media, wgpu |
| `om-app` | Binary `nawsmapper`. eframe/egui UI, canvas editor, projector window, file I/O dialogs. | everything |

Keep logic that can be tested without a window in `om-core` (or in plain functions
in `om-app` with unit tests). GPU code goes in `om-render`, UI code in `om-app`.

## Dev loop

```sh
cargo run                      # debug build (deps are optimized, see Cargo.toml)
cargo run --release            # what users get
cargo fmt                      # rustfmt.toml: max_width 110
cargo clippy --all-targets -- -D warnings
cargo test                     # includes WGSL validation through naga
```

CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs fmt, clippy with
`-D warnings` and tests on Linux. It builds a universal macOS `.app` (via
`scripts/bundle-macos.sh`) and a Windows `.exe`. Keep all of them green.

### Rules on the development machine

- Do not change system state (packages, services, global config). Hand the
  maintainer the commands instead. Editing files inside this repo is fine.
- Do **not** drive the GUI on the maintainer's live desktop with xdotool or take
  full-screen screenshots. It's a tiling WM with private windows. Use the isolated
  Xvfb display instead (no GPU there, so force GL; no WM, so set focus explicitly):

  ```sh
  Xvfb :99 -screen 0 1600x1000x24 &
  DISPLAY=:99 WGPU_BACKEND=gl XDG_DATA_HOME=/tmp/om-xdg cargo run &
  W=$(DISPLAY=:99 xdotool search --name NawsMapper | head -1)
  DISPLAY=:99 xdotool windowmove $W 0 0 windowsize $W 1600 1000 windowfocus --sync $W
  DISPLAY=:99 import -window root shot.png      # then inspect the PNG
  ```

  Stop both processes afterwards. Always set `XDG_DATA_HOME` to a scratch dir so
  smoke tests never touch the real persisted state. To start with a prepared scene,
  pass a project path: `nawsmapper /tmp/scene.omap`. File dialogs are GTK on Linux
  and work in Xvfb (focus the dialog window by its title, type a full path, Enter).

## Library versions and API notes

We are on **egui/eframe 0.37** and **wgpu 30**. Both changed a lot recently, so
read the source in `~/.cargo/registry/src/*/egui-0.37.0/` rather than going by memory:

- `eframe::App::ui(&mut self, ui: &mut egui::Ui, frame)` replaces `update(ctx, frame)`.
- Panels: `egui::Panel::left/right/top/bottom(id).show(ui, ..)` and
  `CentralPanel::no_frame()/default_margins().show(ui, ..)`. All take a `Ui`.
- Menus: `egui::MenuBar::new().ui(ui, ..)`; `Button::shortcut_text`, `Button::right_text`.
- Emoji/icon glyphs need egui's `monochrome_emoji_fonts` feature (enabled in the workspace).
  Not every glyph exists: check visually before using a new one.
- wgpu 30: `PipelineLayoutDescriptor { bind_group_layouts: &[Option<&_>], immediate_size }`,
  `multiview_mask` on pipelines and render passes, `depth_slice` on color attachments.
- Always take `wgpu` from the workspace (must match `egui-wgpu`'s version).
- egui native textures must be `Rgba8Unorm` (that's why the output texture is).
- **Never call blocking dialogs** (`rfd::FileDialog`, `rfd::MessageDialog`) from the UI.
  On macOS they run a nested event loop inside winit's handler, which winit treats as
  fatal re-entrancy. Use `dialogs::FileDialogs::start` (async rfd, result polled per frame)
  and `Prompt` (egui modal) instead. rfd uses its GTK3 backend on Linux, so dialogs work
  without xdg-desktop-portal or zenity.

## Core design rules

1. **The output texture is the single source of truth.** The editor preview and the
   projector window both show the same texture. Overlays (outlines, handles) are
   drawn by egui on top and are never part of the projected image unless the user
   enables "Outlines on projector".
2. **Animation is a pure function of time** (`om_core::anim::evaluate`). No hidden
   per-frame state, so scrubbing, preview and output always agree.
3. **Undo is snapshot-based.** UI code mutates `doc.project` freely. The app commits
   an undo step when the project changed and no pointer button is down
   (`Document::commit_if_changed`), so a drag becomes one step. Don't add manual
   checkpoint calls.
4. **Multi-edit is generic.** The properties panel edits a copy of the primary
   surface, then `om_core::diff::propagate_changes` copies *only changed fields*
   to the other selected surfaces. New `Surface` fields work automatically, as long
   as they are serde-serializable.
5. **File compatibility.** Every serialized struct uses `#[serde(default)]`. Adding a
   field is fine; renaming or removing one needs a `FORMAT_VERSION` bump and a
   migration in `om_core::project`.
6. **UI actions go through `app::Action`.** Menus, toolbar, panels and shortcuts push
   actions; `NawsMapperApp::run_action` executes them. Shortcuts live in `app::shortcuts`.
   Use `Modifiers::COMMAND` (Cmd on macOS, Ctrl elsewhere).
7. **Coordinates.** Surface corners are canvas pixels, order TL, TR, BR, BL ↔ UV
   (0,0) (1,0) (1,1) (0,1), v pointing down. Colors are straight-alpha sRGB-encoded
   `[f32; 4]`; the shader outputs premultiplied alpha.

## Common tasks

- **New surface property:** add the field to `Surface` (with a default), use it in
  `om_render::surface_uniforms` + `surface.wgsl` if it affects rendering, and add a
  control in `panels/properties.rs`. Multi-edit and undo come for free.
- **New shader (generator pattern):** add a variant to `Pattern` (and to `Pattern::ALL`,
  at the end, because the shader index is the position), a `label`, and a
  `detail_label` if it uses `detail`. Add a `case` in `field()` in `surface.wgsl`
  returning (palette position, intensity); use `p`/`r`/`a` (aspect-corrected) for
  anything round, and keep angle terms seam-free (`aa_square_w`, even multiples for
  palette positions). Add a preset to `om_core::catalog` so it shows in the Shader
  Library (a test checks every pattern has one), then look at it in the library in Xvfb.
  The `shader_indices_are_stable` test guards existing indices.
- **New built-in shape:** add a `Mask` variant (append; it's serialized by name), add it
  to `Mask::SHAPES` and give it `anchors()` in `om_core::shape`. It is drawn as a polygon
  automatically; the `built_in_shapes_fill_the_unit_square` test checks it.
- **New blend mode:** append a `BlendMode` variant, put it in `BlendMode::GROUPS`, then
  either give it a blend state in `om_render::fixed_blend` (only if fixed-function
  blending is exact on premultiplied output) or a number in `backdrop_index` and a
  `case` in `blend()` in `surface.wgsl`. A test checks every mode has exactly one.
- **New media type:** add a `SourceKind` variant (with `media_path()` if file-based),
  load it in `om_render::textures::TextureCache::sync`, accept its extension in
  `NawsMapperApp::import_media`. Decoding that takes time belongs on a thread in `om-media`.
- **New animation parameter:** extend `Animation` and apply it in `anim::evaluate`,
  with a unit test.
- **New canvas tool/mode:** follow Free Transform (`transform.rs`) or the Pen / Edit
  Points tools: pure math functions with unit tests (in `om-core` when the renderer
  needs them too), state on `Editor`, `Drag` variants for its gestures, actions for
  enter/apply/cancel, and leave the mode automatically when the selection changes.
- **New panel or action:** add an `Action` variant, handle it in `run_action`, and wire
  the menu item, shortcut and button.

## Documentation workflow

- Each feature gets a tracker in `docs/trackers/NNNN-short-name.md` (copy
  `docs/trackers/TEMPLATE.md`): goal, scope, design notes, a task checklist, and how it was verified.
  Update it as you go. It's the hand-off document between sessions.
- Record user feedback in `docs/feedback-log.md` with the date and the decision it led to.
- Update `docs/roadmap.md` when priorities change, and `README.md` when user-visible
  features land.

## Commits

Conventional style: `type(scope): imperative summary` (≤ 72 chars), then a bullet
list of concrete changes, ending with the tests added or run. Types: `feat`, `fix`,
`chore`, `docs`, `test`, `refactor`, `perf`. Scopes are crate or feature names
(`core`, `render`, `editor`, `output`, `anim`, `ci`, …). No trailers.

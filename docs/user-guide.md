# NawsMapper user guide: your first projection

This guide assumes you know Photoshop and have never mapped anything. It takes
about 15 minutes.

## The idea in one paragraph

A projector throws a flat rectangle of light. Projection mapping makes that light
**land exactly on objects**: the faces of a box, a window frame, a sculpture.
You do that by drawing **surfaces** (shapes with four draggable corners)
in NawsMapper while looking at the real object, until each surface lines up
with a face of the object. Then you fill the surfaces with **sources**
(colors, patterns, images) and **animate** them.

## The workspace

| Area | Photoshop equivalent | What it does |
| --- | --- | --- |
| Center: canvas | Document window | Exactly what the projector shows. Drag shapes and corners here. |
| Left top: **Layers** | Layers panel | Your surfaces. Top of the list = in front. Eye = show/hide, lock = protect. |
| Left bottom: **Sources** | Swatches / Libraries | Colors, patterns and images you can put on surfaces. |
| Right: **Properties** | Properties panel | Settings of the selected surface(s), or the canvas when nothing is selected. |
| Toolbar | Options bar | Add shapes, play/pause, snapping, projector output. |

## 1. Set up the canvas

Click empty space so nothing is selected. Properties then shows the **Canvas**.
Set the size to your projector's native resolution (often 1920×1080; see the
**Preset** menu).

## 2. Connect the projector

1. Connect the projector and set macOS to **extend** the display (not mirror):
   System Settings → Displays.
2. In NawsMapper: **Output → Open Output Window** (`Cmd+Enter`).
3. Drag that window onto the projector's screen and press **`F`** (or
   double-click it). It goes full screen. `Esc` leaves full screen.

With **Outlines on projector** on, the projector also shows thin outlines of all
surfaces and the corners of the selected ones. That's very helpful while aligning.
Turn it off for the show.

## 3. Map an object

The demo project contains three surfaces forming a box. Put a real box in front
of the projector and:

1. Click **Box Top** on the canvas (or in Layers). Its four corner handles appear.
2. Drag each corner until the projected corner sits exactly on the real corner of
   the box. Watch the box, not the screen.
3. For pixel-precise tweaks, click a corner and use the **arrow keys**
   (`Shift` = 10 px).
4. Do the same for the other faces.

Two helpers make this easy:

- **Snapping:** corners snap to other surfaces' corners and the canvas corners.
- **Welded corners:** once two faces share a corner (snapped together), dragging
  that corner moves *both* faces, so the box stays closed.
  Hold **Alt/Option** while dragging to move one corner alone or to avoid snapping.

Tip: use the **Test Grid** source while aligning. Its lines make distortion obvious.

## 4. Fill the surfaces

- Select a surface and choose a **Source** in Properties, or double-click a source
  in the Sources panel to put it on all selected surfaces.
- **✨ Shaders** (toolbar, or `Cmd+L`) opens the **Shader Library**: live previews of
  animated shaders grouped by mood (Psychedelic, Geometric, Organic, Cosmic, Basics).
  Select surfaces, pick a shader, **Apply**. Double-clicking a preview does the same.
  The shader becomes a source: click it in Sources to change its **Palette**
  (Rainbow, Neon, Fire, Ocean…, or *Two colors* of your own), scale, speed, angle and
  its own extra control (Twist, Mirrors, Arms, Warp…).
- **+ Media…** (or File → Import Images & Videos), or just **drag image and video
  files** onto the window. If surfaces are selected, the media goes straight onto them.
- **Videos** must be H.264 (`.mp4`, `.mov`, `.m4v`): what most cameras, phones in
  "Most Compatible" mode and video exports produce. Videos loop and follow the play
  clock: pause, restart (⏮) and play work for video like for everything else.
  An iPhone's default HEVC video shows a ⚠ in Sources; re-export it as H.264
  (QuickTime Player: File → Export As → 1080p).
- **Texture → Mapping:** *Fit* puts the whole image on each surface; *Span* spreads
  one image across the whole canvas, so several surfaces together show one picture.

- **Blend** (Properties → Appearance) mixes a surface with what is behind it, with the
  same modes and order as Photoshop: Darken/Multiply/Burn, Lighten/Screen/Dodge/Add,
  Overlay/Soft Light/Hard Light…, Difference/Exclusion/Subtract/Divide, and
  Hue/Saturation/Color/Luminosity. Select surfaces and press `Shift +` / `Shift -` to
  step through them while watching the projection. Try a shader on top of a video
  with *Overlay* or *Color*, or a black-and-white shader with *Multiply* as a mask.

## 5. Animate

Open the **Animate** section in Properties. Start with the presets: **Drift, Spin,
Breathe, Pulse, Strobe, Rainbow**. Then fine-tune:

| Control | Effect |
| --- | --- |
| Speed | Plays everything on this surface faster or slower. |
| Scroll | Slides the texture continuously. |
| Spin | Rotates the texture continuously. |
| Hue cycle | Rotates colors around the color wheel. |
| Zoom pulse | Texture breathes in and out (amount, rate, waveform). |
| Opacity pulse | Surface fades or flashes. A *Square* wave is a strobe. |
| Time offset | Shifts this surface's animation in time. |

**Chase effects:** select several surfaces *in the order you want the effect to
travel* (Shift-click one after the other), then press **Stagger** in Animate.
Each surface gets a slightly later time offset, so the motion ripples across them.

## 6. Work with many surfaces at once

- Shift-click or Cmd-click to add surfaces to the selection, or drag a marquee
  over empty canvas. In Layers, Shift-click selects a range.
- With several selected, Properties shows the last one you clicked. **Any value you
  change is applied to all of them**, and only that value. Everything else stays
  as it was on each surface.
- Drag any selected surface to move them all. `Cmd+D` duplicates, `Delete` removes.

## 7. Resize and rotate (Free Transform)

Select one or more surfaces and press **`Cmd+T`** (or click **Transform** in the toolbar).
Just like Photoshop, a box with eight handles appears around the selection:

- Drag a **corner or side handle** to scale. Hold **Shift** to keep the proportions,
  **Alt/Option** to scale from the center.
- Drag **outside** the box to rotate. Hold **Shift** to rotate in 15° steps.
- Drag **inside** the box to move. Arrow keys nudge.
- The toolbar shows width, height and angle. Press **Enter** (or double-click inside,
  or click **Apply**) to finish, or **Esc** / **Cancel** to put everything back.

Several surfaces scale and rotate together as one group, so you can resize a whole
mapped object at once.

## 8. More shapes and your own

- **◆ Shapes** in the toolbar has 14 shapes: rounded rectangle, diamond, pentagon,
  hexagon, octagon, star, burst, heart, cross, arrow, bolt and the basic three. You
  can also switch a surface's **Shape** in Properties. Every shape sits inside the
  surface's four corners, so corner pinning and `Cmd+T` work as usual.
- **Draw your own** with the **Pen** (`P`, or ✒ Pen in the toolbar): click to place
  points; *press and drag* to make a curve point. Click the first point (or press
  `Enter`, or double-click) to finish. `Backspace` removes the last point, `Esc` cancels.
- **Edit Points:** double-click a free-form shape, or select any shape and click
  **✎ Edit Points** in Properties (that turns a star or heart into editable points too).
  Drag points, click an edge to add a point, `Alt`-click (or select and `Delete`) to
  remove one, double-click a point to switch between curve (round) and corner
  (square). `Enter` or `Esc` when done. **All curves / All corners** in Properties
  changes every point at once.
- The texture fits the shape's bounding box, and a free-form shape can be a cutout.

## 9. Cut holes (Cutouts)

A **cutout** is a shape that removes everything below it: the window in a wall, a
plant in front of the screen, the gap between two objects.

- Click **✂ Cutout** in the toolbar (or Surface → Add Cutout). It appears on top of
  the layers. Shape it like any surface (corners, `Cmd+T`, Shape: ellipse/triangle).
- Or turn an existing surface into one: Properties → **Cutout → Cut out of layers below**.
- A cutout cuts **every layer below it** in the Layers panel and nothing above it.
  Drag it up or down the stack (`Cmd+[` / `Cmd+]`) to choose what it cuts.
- **Feather** gives the hole a soft edge, **Opacity** makes it partial, and an
  opacity pulse animates it.
- In the editor, cutouts have a dashed pink outline.

## 10. Save

`Cmd+S` saves an `.omap` project; `Cmd+Shift+S` saves under a new name.
NawsMapper reopens your last project on launch. When you are about to lose
unsaved changes (New, Open, Quit), it asks first.
Keep images and videos next to the project file if you want to move the project
to another computer.

## Shortcut cheat sheet

See the table in the [README](../README.md#keyboard).

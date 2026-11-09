//! Mapping templates: the surfaces' outlines, numbers and names at canvas
//! resolution, for designing content that fits exactly (in Photoshop,
//! After Effects, Illustrator…).
//!
//! PNG is drawn on the CPU with tiny-skia on a transparent background, so it
//! can go on top of the artwork as a guide layer. Text uses egui's bundled
//! font, so the result doesn't depend on the fonts installed. SVG keeps the
//! outlines as vectors, one named group per surface.

use std::fmt::Write as _;

use ab_glyph::{Font, FontRef, OutlineCurve, ScaleFont};
use glam::Vec2;
use om_core::Project;
use tiny_skia::{FillRule, LineJoin, Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform};

/// Distinct outline colors, cycled through the surfaces.
const COLORS: [[u8; 3]; 8] = [
    [45, 160, 255],
    [255, 170, 40],
    [80, 220, 120],
    [240, 80, 160],
    [170, 120, 255],
    [255, 230, 70],
    [60, 220, 220],
    [255, 110, 80],
];
const CUTOUT_COLOR: [u8; 3] = [235, 110, 150];
const FILL_ALPHA: f32 = 0.18;

/// One surface as it appears in the template.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// 1 is the top of the Layers panel (frontmost).
    pub number: usize,
    pub name: String,
    /// Canvas pixels.
    pub outline: Vec<Vec2>,
    pub cutout: bool,
    pub color: [u8; 3],
}

impl Item {
    pub fn label(&self) -> String {
        if self.cutout {
            format!("{} {} (cutout)", self.number, self.name)
        } else {
            format!("{} {}", self.number, self.name)
        }
    }
}

/// The visible surfaces, back to front (drawing order), numbered like the
/// Layers panel (front = 1).
pub fn items(project: &Project) -> Vec<Item> {
    let count = project.surfaces.len();
    project
        .surfaces
        .iter()
        .enumerate()
        .filter(|(_, s)| s.visible)
        .map(|(i, s)| {
            let number = count - i;
            Item {
                number,
                name: s.name.clone(),
                outline: s.outline(),
                cutout: s.cutout,
                color: if s.cutout { CUTOUT_COLOR } else { COLORS[(number - 1) % COLORS.len()] },
            }
        })
        .collect()
}

/// Where a label goes: the area centroid of the outline, or the middle of
/// its bounding box for degenerate shapes.
pub fn label_point(outline: &[Vec2]) -> Vec2 {
    let mut area = 0.0;
    let mut c = Vec2::ZERO;
    for (i, a) in outline.iter().enumerate() {
        let b = outline[(i + 1) % outline.len()];
        let cross = a.perp_dot(b);
        area += cross;
        c += (*a + b) * cross;
    }
    if area.abs() > 1e-3 {
        return c / (3.0 * area);
    }
    let (min, max) = outline
        .iter()
        .fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    if outline.is_empty() { Vec2::ZERO } else { (min + max) * 0.5 }
}

fn font_size(project: &Project) -> f32 {
    (project.canvas.height as f32 / 40.0).clamp(16.0, 64.0)
}

fn footer(project: &Project, title: &str) -> String {
    format!("{title} · {}×{} · NawsMapper mapping template", project.canvas.width, project.canvas.height)
}

// ---- SVG -------------------------------------------------------------------

pub fn svg(project: &Project, title: &str) -> String {
    let (w, h) = (project.canvas.width, project.canvas.height);
    let size = font_size(project);
    let mut out = String::new();
    let _ = writeln!(out, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#
    );
    let _ = writeln!(out, "  <title>{}</title>", xml(&footer(project, title)));
    let text_style = format!(
        r##"font-family="Helvetica, Arial, sans-serif" font-size="{size:.0}" text-anchor="middle" dominant-baseline="middle" fill="#ffffff" stroke="#000000" stroke-width="{:.1}" paint-order="stroke""##,
        size / 8.0
    );
    for item in items(project) {
        let [r, g, b] = item.color;
        let color = format!("#{r:02x}{g:02x}{b:02x}");
        let mut d = String::new();
        for (i, p) in item.outline.iter().enumerate() {
            let _ = write!(d, "{}{:.1} {:.1} ", if i == 0 { "M" } else { "L" }, p.x, p.y);
        }
        d.push('Z');
        let dash = if item.cutout { r#" stroke-dasharray="12 8""# } else { "" };
        let fill = if item.cutout { "none".to_string() } else { color.clone() };
        let at = label_point(&item.outline);
        let _ = writeln!(out, r#"  <g id="{}">"#, xml(&group_id(&item)));
        let _ = writeln!(
            out,
            r#"    <path d="{d}" fill="{fill}" fill-opacity="{FILL_ALPHA}" stroke="{color}" stroke-width="2"{dash}/>"#
        );
        let _ = writeln!(
            out,
            r#"    <text x="{:.1}" y="{:.1}" {text_style}>{}</text>"#,
            at.x,
            at.y,
            xml(&item.label())
        );
        let _ = writeln!(out, "  </g>");
    }
    let _ = writeln!(
        out,
        r##"  <text x="12" y="{:.1}" font-family="Helvetica, Arial, sans-serif" font-size="{:.0}" fill="#ffffff" fill-opacity="0.7">{}</text>"##,
        h as f32 - 12.0,
        size * 0.6,
        xml(&footer(project, title))
    );
    out.push_str("</svg>\n");
    out
}

/// A readable, unique XML id: "03_Box_Top". Design apps use it as the layer name.
fn group_id(item: &Item) -> String {
    let name: String =
        item.name.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).collect();
    format!("{:02}_{name}", item.number)
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ---- PNG -------------------------------------------------------------------

/// The template as PNG file bytes.
pub fn png(project: &Project, title: &str) -> anyhow::Result<Vec<u8>> {
    let (w, h) = (project.canvas.width, project.canvas.height);
    let mut pixmap = Pixmap::new(w, h).ok_or_else(|| anyhow::anyhow!("canvas too large: {w}×{h}"))?;
    let font = FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT)?;
    let size = font_size(project);

    let items = items(project);
    for item in &items {
        let Some(path) = polygon(&item.outline) else { continue };
        let [r, g, b] = item.color;
        if !item.cutout {
            let fill = paint(r, g, b, FILL_ALPHA);
            pixmap.fill_path(&path, &fill, FillRule::EvenOdd, Transform::identity(), None);
        }
        let mut stroke = Stroke { width: 2.0, ..Default::default() };
        if item.cutout {
            stroke.dash = StrokeDash::new(vec![12.0, 8.0], 0.0);
        }
        pixmap.stroke_path(&path, &paint(r, g, b, 1.0), &stroke, Transform::identity(), None);
    }
    // Labels last, so no outline crosses a name.
    for item in &items {
        text(&mut pixmap, &font, &item.label(), label_point(&item.outline), size, true);
    }
    let corner = Vec2::new(12.0, h as f32 - 12.0 - size * 0.3);
    text(&mut pixmap, &font, &footer(project, title), corner, size * 0.6, false);
    Ok(pixmap.encode_png()?)
}

fn paint(r: u8, g: u8, b: u8, alpha: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(r, g, b, (alpha * 255.0).round() as u8);
    p.anti_alias = true;
    p
}

fn polygon(points: &[Vec2]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let (first, rest) = points.split_first()?;
    pb.move_to(first.x, first.y);
    for p in rest {
        pb.line_to(p.x, p.y);
    }
    pb.close();
    pb.finish()
}

/// Draws white text with a dark outline, centered on `at` (or starting at
/// `at` on the baseline when `centered` is false).
fn text(pixmap: &mut Pixmap, font: &FontRef<'_>, s: &str, at: Vec2, size: f32, centered: bool) {
    let scaled = font.as_scaled(size);
    let width: f32 = s.chars().map(|c| scaled.h_advance(font.glyph_id(c))).sum();
    let (mut x, baseline) = if centered {
        (at.x - width * 0.5, at.y + (scaled.ascent() + scaled.descent()) * 0.5)
    } else {
        (at.x, at.y)
    };
    let k = scaled.scale_factor();
    let mut pb = PathBuilder::new();
    let mut prev = None;
    for c in s.chars() {
        let id = font.glyph_id(c);
        if let Some(p) = prev {
            x += scaled.kern(p, id);
        }
        if let Some(outline) = font.outline(id) {
            let map = |p: ab_glyph::Point| (x + p.x * k.horizontal, baseline - p.y * k.vertical);
            let mut last: Option<(f32, f32)> = None;
            for curve in &outline.curves {
                let (start, end) = match curve {
                    OutlineCurve::Line(a, b)
                    | OutlineCurve::Quad(a, _, b)
                    | OutlineCurve::Cubic(a, _, _, b) => (map(*a), map(*b)),
                };
                if last != Some(start) {
                    pb.move_to(start.0, start.1);
                }
                match curve {
                    OutlineCurve::Line(..) => pb.line_to(end.0, end.1),
                    OutlineCurve::Quad(_, c, _) => {
                        let c = map(*c);
                        pb.quad_to(c.0, c.1, end.0, end.1)
                    }
                    OutlineCurve::Cubic(_, c1, c2, _) => {
                        let (c1, c2) = (map(*c1), map(*c2));
                        pb.cubic_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1)
                    }
                }
                last = Some(end);
            }
        }
        x += scaled.h_advance(id);
        prev = Some(id);
    }
    let Some(path) = pb.finish() else { return };
    // A dark halo keeps the text readable on any artwork; the white stroke on
    // top thickens the light font into a faux bold.
    let halo = Stroke { width: (size / 5.0).max(3.0), line_join: LineJoin::Round, ..Default::default() };
    pixmap.stroke_path(&path, &paint(0, 0, 0, 0.85), &halo, Transform::identity(), None);
    let white = paint(255, 255, 255, 1.0);
    pixmap.fill_path(&path, &white, FillRule::Winding, Transform::identity(), None);
    let bold = Stroke { width: (size / 18.0).max(1.0), line_join: LineJoin::Round, ..Default::default() };
    pixmap.stroke_path(&path, &white, &bold, Transform::identity(), None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use om_core::{Surface, SurfaceId};

    fn project() -> Project {
        let mut p = Project::empty();
        p.canvas.width = 400;
        p.canvas.height = 200;
        p.surfaces.push(Surface::rect(
            SurfaceId(1),
            "Back <wall>",
            Vec2::new(20.0, 20.0),
            Vec2::new(160.0, 120.0),
        ));
        let mut hidden = Surface::rect(SurfaceId(2), "Hidden", Vec2::ZERO, Vec2::splat(50.0));
        hidden.visible = false;
        p.surfaces.push(hidden);
        let mut cut = Surface::rect(SurfaceId(3), "Door", Vec2::new(250.0, 40.0), Vec2::new(60.0, 120.0));
        cut.cutout = true;
        p.surfaces.push(cut);
        p
    }

    #[test]
    fn items_are_numbered_like_the_layers_panel() {
        let items = items(&project());
        let labels: Vec<String> = items.iter().map(Item::label).collect();
        // Drawing order (back first); the frontmost surface is number 1.
        assert_eq!(labels, ["3 Back <wall>", "1 Door (cutout)"]);
        assert_eq!(items[1].color, CUTOUT_COLOR);
    }

    #[test]
    fn label_sits_in_the_middle_of_the_shape() {
        let square = [Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)];
        assert!((label_point(&square) - Vec2::splat(5.0)).length() < 1e-4);
        let flat = [Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(20.0, 0.0)];
        assert_eq!(label_point(&flat), Vec2::new(10.0, 0.0));
    }

    #[test]
    fn svg_has_one_named_group_per_visible_surface() {
        let svg = svg(&project(), "Show & Tell");
        assert!(svg.contains(r#"width="400" height="200" viewBox="0 0 400 200""#));
        assert!(svg.contains(r#"<g id="03_Back__wall_">"#), "{svg}");
        assert!(svg.contains(">3 Back &lt;wall&gt;</text>"));
        assert!(svg.contains(r#"stroke-dasharray="12 8""#), "cutouts are dashed");
        assert!(!svg.contains("Hidden"));
        assert!(svg.contains("Show &amp; Tell · 400×200"));
        assert_eq!(svg.matches("<g id=").count(), 2);
    }

    #[test]
    fn png_has_canvas_size_transparent_background_and_outlines() {
        let bytes = png(&project(), "Test").unwrap();
        let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (400, 200));
        assert_eq!(img.get_pixel(390, 5)[3], 0, "outside every surface: transparent");
        // The left edge of "Back" at x = 20 is stroked in its color.
        let edge = img.get_pixel(20, 100);
        assert!(edge[3] > 200 && edge[2] > edge[0], "{edge:?}");
        // Inside: the light fill.
        let inside = img.get_pixel(40, 30);
        assert!(inside[3] > 20 && inside[3] < 80, "{inside:?}");
        // The label is drawn: some pixels around the label point are white.
        let at = label_point(&items(&project())[0].outline);
        let white = (0..40)
            .flat_map(|dx| (0..12).map(move |dy| (dx, dy)))
            .map(|(dx, dy)| img.get_pixel(at.x as u32 - 20 + dx, at.y as u32 - 6 + dy))
            .any(|p| p[0] > 240 && p[1] > 240 && p[2] > 240 && p[3] > 240);
        assert!(white, "label text missing");
    }
}

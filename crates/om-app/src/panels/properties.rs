//! Properties panel. Edits the selected surfaces — all of them at once.
//!
//! The panel shows the primary (last selected) surface. Whatever field the
//! user changes is then copied to every other selected surface, leaving
//! their other settings alone (see `om_core::diff`).

use egui::{RichText, Slider, SliderClamping, Ui};
use glam::Vec2;
use om_core::anim::{Lfo, Wave};
use om_core::{
    BlendMode, Border, BorderEffect, BorderPosition, BorderStyle, Mapping, Mask, Palette, Project, Surface,
    SurfaceId, diff::propagate_changes,
};

use crate::app::{Action, NawsMapperApp};
use crate::widgets::{color_edit, combo, fit_slider, grid, prop_label, section, vec2_drag};

pub fn properties_panel(ui: &mut Ui, app: &mut NawsMapperApp) {
    egui::Panel::right("properties").default_size(300.0).min_size(260.0).show(ui, |ui| {
        super::panel_header(ui, "Properties");
        // The content is laid out in a child that does not report its size,
        // so the panel stays exactly as wide as the user made it: a long file
        // name or a wide row is cut off instead of pushing the panel open.
        let rect = ui.available_rect_before_wrap();
        let mut content = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        content.set_clip_rect(rect.intersect(ui.clip_rect()));
        ui.advance_cursor_after_rect(rect);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut content, |ui| {
            let sel: Vec<SurfaceId> = app.editor.selection.surfaces.clone();
            match app.editor.selection.primary().and_then(|id| app.doc.project.surface(id)).cloned() {
                None => canvas_props(ui, &mut app.doc.project),
                Some(before) => {
                    let mut after = before.clone();
                    surface_props(ui, app, &mut after, sel.len());
                    if after != before {
                        apply_to_selection(&mut app.doc.project, &sel, &before, &after);
                    }
                }
            }
        });
    });
}

fn apply_to_selection(project: &mut Project, sel: &[SurfaceId], before: &Surface, after: &Surface) {
    for s in project.surfaces.iter_mut().filter(|s| sel.contains(&s.id)) {
        if s.id == after.id {
            *s = after.clone();
        } else if let Err(e) = propagate_changes(before, after, s) {
            log::error!("multi-edit failed for {:?}: {e:#}", s.id);
        }
    }
}

fn canvas_props(ui: &mut Ui, project: &mut Project) {
    ui.label(RichText::new("Nothing selected — showing the canvas.").weak());
    ui.add_space(6.0);
    section(ui, "Canvas", |ui| {
        grid(ui, "canvas_props", |ui| {
            prop_label(ui, "Size", "Set this to your projector's native resolution");
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::DragValue::new(&mut project.canvas.width).range(16..=8192).suffix(" px"));
                ui.label("×");
                ui.add(egui::DragValue::new(&mut project.canvas.height).range(16..=8192).suffix(" px"));
            });
            ui.end_row();

            prop_label(ui, "Preset", "");
            combo(ui, "canvas_preset", "Choose…").show_ui(ui, |ui| {
                for (w, h, name) in [
                    (1280, 720, "720p"),
                    (1920, 1080, "1080p (Full HD)"),
                    (1920, 1200, "WUXGA"),
                    (2560, 1440, "1440p"),
                    (3840, 2160, "4K UHD"),
                    (1024, 768, "XGA (4:3)"),
                    (1400, 1050, "SXGA+ (4:3)"),
                ] {
                    if ui.selectable_label(false, format!("{w} × {h}  {name}")).clicked() {
                        project.canvas.width = w;
                        project.canvas.height = h;
                    }
                }
            });
            ui.end_row();

            prop_label(ui, "Background", "What the projector shows where there are no surfaces");
            color_edit(ui, &mut project.canvas.background);
            ui.end_row();
        });
    });
    ui.add_space(10.0);
    ui.label(RichText::new("Tip: click a surface on the canvas or in Layers to edit it. Shift-click to edit several at once.").weak());
}

fn surface_props(ui: &mut Ui, app: &mut NawsMapperApp, s: &mut Surface, count: usize) {
    if count > 1 {
        ui.label(RichText::new(format!("{count} surfaces selected")).strong());
        ui.label(
            RichText::new(format!("Showing \"{}\". Changes apply to all {count}.", s.name)).weak().small(),
        );
        ui.add_space(4.0);
    }

    section(ui, "Surface", |ui| {
        grid(ui, "surface_props", |ui| {
            if count == 1 {
                prop_label(ui, "Name", "");
                ui.text_edit_singleline(&mut s.name);
                ui.end_row();
            }

            prop_label(
                ui,
                "Shape",
                "The shape inside the four corners. It follows corner pinning and transforms.",
            );
            ui.horizontal(|ui| {
                super::shape_icon(ui, s.mask, &s.path, 18.0, ui.visuals().text_color());
                combo(ui, "mask", s.mask.label()).show_ui(ui, |ui| {
                    for m in Mask::SHAPES {
                        ui.selectable_value(&mut s.mask, m, m.label());
                    }
                });
            });
            ui.end_row();

            if count == 1 {
                prop_label(ui, "", "");
                ui.horizontal_wrapped(|ui| {
                    let hint = if s.mask == Mask::Freeform {
                        "Move, add and remove the shape's points (or double-click it on the canvas)"
                    } else {
                        "Turn this shape into points you can move, add and remove"
                    };
                    if ui
                        .add_enabled(!s.locked, egui::Button::new("✎ Edit Points"))
                        .on_hover_text(hint)
                        .clicked()
                    {
                        app.act(Action::EditPoints);
                    }
                    if s.mask == Mask::Freeform {
                        ui.label(RichText::new(format!("{} points", s.path.len())).weak());
                    }
                });
                ui.end_row();
                if s.mask == Mask::Freeform {
                    prop_label(ui, "Points", "Curve points round the outline, corner points keep it sharp");
                    ui.horizontal_wrapped(|ui| {
                        for (label, smooth) in [("All curves", true), ("All corners", false)] {
                            if ui.small_button(label).clicked() {
                                s.path.iter_mut().for_each(|p| p.smooth = smooth);
                                s.fit_frame_to_path();
                            }
                        }
                    });
                    ui.end_row();
                }
            }

            prop_label(
                ui,
                "Cutout",
                "Cut a hole in every surface below this one.\nMove it in Layers to choose what it cuts.",
            );
            ui.checkbox(&mut s.cutout, "Cut out of layers below");
            ui.end_row();

            if !s.cutout {
                prop_label(ui, "Source", "What is shown on the surface");
                let current = s
                    .source
                    .and_then(|id| app.doc.project.source(id))
                    .map(|src| src.name.clone())
                    .unwrap_or_else(|| "None".into());
                combo(ui, "source", current.clone())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.source, None, "None");
                        for src in &app.doc.project.sources {
                            ui.selectable_value(&mut s.source, Some(src.id), &src.name);
                        }
                    })
                    .response
                    .on_hover_text(current);
                ui.end_row();
            }

            prop_label(ui, "", "");
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut s.visible, "Visible");
                ui.checkbox(&mut s.locked, "Locked");
            });
            ui.end_row();
        });
    });

    section(ui, "Appearance", |ui| {
        grid(ui, "appearance_props", |ui| {
            prop_label(ui, "Opacity", "");
            fit_slider(ui, Slider::new(&mut s.opacity, 0.0..=1.0));
            ui.end_row();

            // A cutout has no colors of its own: tint and blend don't apply.
            if !s.cutout {
                prop_label(ui, "Tint", "Multiplies the source colors");
                color_edit(ui, &mut s.tint);
                ui.end_row();

                prop_label(
                    ui,
                    "Blend",
                    "How this surface combines with the ones behind it (Shift + / Shift - to cycle)",
                );
                combo(ui, "blend", s.blend.label()).show_ui(ui, |ui| {
                    for (i, group) in BlendMode::GROUPS.iter().enumerate() {
                        if i > 0 {
                            ui.separator();
                        }
                        for &b in *group {
                            ui.selectable_value(&mut s.blend, b, b.label());
                        }
                    }
                });
                ui.end_row();
            }

            prop_label(ui, "Feather", "Soft edges");
            fit_slider(ui, Slider::new(&mut s.feather, 0.0..=0.5));
            ui.end_row();
        });
    });

    section(ui, "Border", |ui| border_props(ui, &mut s.border));

    if !s.cutout {
        section(ui, "Texture", |ui| {
            grid(ui, "texture_props", |ui| {
                prop_label(
                    ui,
                    "Mapping",
                    "Fit: whole texture on each surface.\nSpan: one texture across the canvas, revealed by surfaces.",
                );
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut s.mapping, Mapping::Surface, "Fit");
                    ui.selectable_value(&mut s.mapping, Mapping::Canvas, "Span");
                });
                ui.end_row();

                prop_label(ui, "Offset", "");
                vec2_drag(ui, &mut s.texture.offset, 0.005, "");
                ui.end_row();

                prop_label(ui, "Scale", "");
                vec2_drag(ui, &mut s.texture.scale, 0.005, "");
                ui.end_row();

                prop_label(ui, "Rotation", "");
                fit_slider(ui, Slider::new(&mut s.texture.rotation, -180.0..=180.0).suffix("°"));
                ui.end_row();

                prop_label(ui, "Flip", "");
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut s.texture.flip_x, "Horizontal");
                    ui.checkbox(&mut s.texture.flip_y, "Vertical");
                });
                ui.end_row();
            });
            if ui.small_button("Reset texture").clicked() {
                s.texture = Default::default();
            }
        });
    }

    section(ui, "Animate", |ui| animation_props(ui, app, s, count));

    if count == 1 {
        section(ui, "Corners", |ui| {
            grid(ui, "corner_props", |ui| {
                for (i, name) in ["Top left", "Top right", "Bottom right", "Bottom left"].iter().enumerate() {
                    prop_label(ui, name, "");
                    vec2_drag(ui, &mut s.corners[i], 0.5, " px");
                    ui.end_row();
                }
            });
            if ui
                .small_button("Square up")
                .on_hover_text("Reset to an upright rectangle with the same bounds")
                .clicked()
            {
                let min = s.corners.iter().copied().reduce(Vec2::min).unwrap();
                let max = s.corners.iter().copied().reduce(Vec2::max).unwrap();
                s.corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
            }
        });
    }
}

fn border_props(ui: &mut Ui, b: &mut Border) {
    ui.checkbox(&mut b.enabled, "Border").on_hover_text(
        "A line along the shape's edge: plain, dashed, or animated.\nSet Source to None for an outline only.",
    );
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Presets").weak());
        for (name, hint, apply) in Border::presets() {
            if ui.small_button(name).on_hover_text(hint).clicked() {
                apply(b);
            }
        }
    });
    if !b.enabled {
        return;
    }
    ui.add_space(4.0);
    grid(ui, "border_props", |ui| {
        prop_label(ui, "Effect", "What the border is made of");
        combo(ui, "border_effect", b.effect.label()).show_ui(ui, |ui| {
            for effect in BorderEffect::ALL {
                ui.selectable_value(&mut b.effect, effect, effect.label());
            }
        });
        ui.end_row();

        if b.effect == BorderEffect::Color {
            prop_label(ui, "Color", "");
            color_edit(ui, &mut b.color);
            ui.end_row();
        } else {
            prop_label(ui, "Palette", "The colors of the effect");
            combo(ui, "border_palette", b.palette.label()).show_ui(ui, |ui| {
                for p in Palette::ALL {
                    ui.horizontal_wrapped(|ui| {
                        super::sources::palette_strip(
                            ui,
                            p.stops(b.color, b.color_b),
                            egui::vec2(36.0, 12.0),
                        );
                        ui.selectable_value(&mut b.palette, p, p.label());
                    });
                }
            });
            ui.end_row();
            if b.palette == Palette::Duotone {
                prop_label(ui, "Colors", "");
                ui.horizontal_wrapped(|ui| {
                    color_edit(ui, &mut b.color);
                    color_edit(ui, &mut b.color_b);
                });
                ui.end_row();
            }
        }

        prop_label(ui, "Width", "Canvas pixels");
        fit_slider(
            ui,
            Slider::new(&mut b.width, 0.5..=100.0)
                .logarithmic(true)
                .clamping(SliderClamping::Never)
                .suffix(" px"),
        );
        ui.end_row();

        prop_label(
            ui,
            "Position",
            "Inside keeps the border on the object.\nCenter and Outside reach past the shape's edge.",
        );
        ui.horizontal_wrapped(|ui| {
            for pos in BorderPosition::ALL {
                ui.selectable_value(&mut b.position, pos, pos.label());
            }
        });
        ui.end_row();

        prop_label(ui, "Glow", "Neon light around the border");
        fit_slider(ui, Slider::new(&mut b.glow, 0.0..=1.0));
        ui.end_row();

        if b.effect.uses_style() {
            prop_label(ui, "Style", "");
            combo(ui, "border_style", b.style.label()).show_ui(ui, |ui| {
                for style in BorderStyle::ALL {
                    ui.selectable_value(&mut b.style, style, style.label());
                }
            });
            ui.end_row();

            if b.style.has_dashes() {
                prop_label(ui, "Dash", "Length of each dash");
                ui.add(egui::DragValue::new(&mut b.dash).range(0.5..=2000.0).speed(0.5).suffix(" px"));
                ui.end_row();
            }
            if b.style.is_broken() {
                prop_label(ui, "Gap", "Space between dashes or dots");
                ui.add(egui::DragValue::new(&mut b.gap).range(0.0..=2000.0).speed(0.5).suffix(" px"));
                ui.end_row();
            }
        }

        if let Some(label) = b.effect.length_label() {
            prop_label(ui, label, "Adjusted slightly so the pattern closes neatly around the shape");
            ui.add(egui::DragValue::new(&mut b.length).range(4.0..=5000.0).speed(1.0).suffix(" px"));
            ui.end_row();
        }

        if b.needs_path() {
            prop_label(
                ui,
                "Flow",
                "Moves everything on the border around the shape, pixels per second.\nNegative values go the other way.",
            );
            fit_slider(
                ui,
                Slider::new(&mut b.flow, -400.0..=400.0).clamping(SliderClamping::Never).suffix(" px/s"),
            );
            ui.end_row();
        }
    });
}

fn animation_props(ui: &mut Ui, app: &mut NawsMapperApp, s: &mut Surface, count: usize) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Presets").weak());
        let a = &mut s.animation;
        if ui.small_button("Drift").clicked() {
            a.scroll = Vec2::new(0.05, 0.02);
        }
        if ui.small_button("Spin").clicked() {
            a.spin = 0.05;
        }
        if ui.small_button("Breathe").clicked() {
            a.zoom = Lfo { wave: Wave::Sine, rate: 0.25, amount: 0.2 };
        }
        if ui.small_button("Pulse").clicked() {
            a.pulse = Lfo { wave: Wave::Sine, rate: 1.0, amount: 0.7 };
        }
        if ui.small_button("Strobe").clicked() {
            a.pulse = Lfo { wave: Wave::Square, rate: 6.0, amount: 1.0 };
        }
        if ui.small_button("Rainbow").clicked() {
            a.hue_speed = 0.1;
        }
        if ui.small_button("Stop").on_hover_text("Remove all animation").clicked() {
            let keep_offset = a.time_offset;
            *a = Default::default();
            a.time_offset = keep_offset;
        }
    });
    ui.add_space(4.0);

    let a = &mut s.animation;
    grid(ui, "anim_props", |ui| {
        prop_label(ui, "Speed", "Playback speed of everything on this surface");
        fit_slider(ui, Slider::new(&mut a.speed, 0.0..=4.0).suffix("×"));
        ui.end_row();

        prop_label(ui, "Scroll", "Texture movement per second");
        vec2_drag(ui, &mut a.scroll, 0.002, "/s");
        ui.end_row();

        prop_label(ui, "Spin", "Texture rotation, turns per second");
        fit_slider(ui, Slider::new(&mut a.spin, -1.0..=1.0).suffix(" turns/s"));
        ui.end_row();

        prop_label(ui, "Hue cycle", "Color wheel rotation, turns per second");
        fit_slider(ui, Slider::new(&mut a.hue_speed, -1.0..=1.0).suffix(" turns/s"));
        ui.end_row();
    });

    ui.add_space(4.0);
    lfo_ui(ui, "Zoom pulse", "zoom", &mut a.zoom, 1.0);
    lfo_ui(ui, "Opacity pulse", "pulse", &mut a.pulse, 1.0);

    ui.add_space(4.0);
    grid(ui, "anim_offset", |ui| {
        prop_label(
            ui,
            "Time offset",
            "Shifts this surface's animation in time. Different offsets on several surfaces make a chase.",
        );
        ui.add(egui::DragValue::new(&mut a.time_offset).speed(0.01).suffix(" s"));
        ui.end_row();
    });

    if count > 1 {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("Stagger")
                .on_hover_text("Give each selected surface an increasing time offset, in the order you selected them — instant chase effect")
                .clicked()
            {
                app.act(Action::Stagger);
            }
            ui.add(egui::DragValue::new(&mut app.stagger_step).speed(0.01).range(-10.0..=10.0).suffix(" s apart"));
        });
    }
}

fn lfo_ui(ui: &mut Ui, title: &str, id: &str, lfo: &mut Lfo, max_amount: f32) {
    ui.label(RichText::new(title).strong());
    grid(ui, id, |ui| {
        prop_label(ui, "Amount", "0 = off");
        fit_slider(ui, Slider::new(&mut lfo.amount, 0.0..=max_amount));
        ui.end_row();

        prop_label(ui, "Rate", "Cycles per second");
        fit_slider(ui, Slider::new(&mut lfo.rate, 0.01..=20.0).logarithmic(true).suffix(" Hz"));
        ui.end_row();

        prop_label(ui, "Wave", "");
        ui.horizontal_wrapped(|ui| {
            for w in Wave::ALL {
                ui.selectable_value(&mut lfo.wave, w, w.label());
            }
        });
        ui.end_row();
    });
}

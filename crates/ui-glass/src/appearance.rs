//! Appearance: a glass panel with every look-and-feel setting in one place, the way System
//! Settings › Appearance gathers them. Changes apply live.

use egui::{Align2, Sense, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_ui_egui::theme::{Animations, Appearance, regular, semibold};

use crate::GlassApp;
use crate::overlay::Glow;
use crate::tokens::{Accent, CanvasTone, Tokens, popover_shape};
use crate::widgets as w;

fn row(ui: &mut Ui, t: &Tokens, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(34.0);
        ui.label(egui::RichText::new(label).font(regular(13.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
}

fn divider(ui: &mut Ui, t: &Tokens) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.edge.gamma_multiply(0.5));
}

fn heading(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(text).font(semibold(11.5)).color(t.text_dim));
    ui.add_space(2.0);
}

pub fn show(g: &mut GlassApp, ctx: &egui::Context, t: &Tokens) {
    let fade = ctx.animate_bool_with_time(egui::Id::new("glass_appearance_fade"), g.glass.appearance_open, 0.16);
    if fade <= 0.0 {
        return;
    }
    if fade < 1.0 {
        ctx.request_repaint();
    }
    let screen = ctx.content_rect();
    let pos = pos2(screen.max.x - 24.0, screen.min.y + 96.0 + (1.0 - fade) * 8.0);
    let area =
        egui::Area::new(egui::Id::new("glass_appearance")).order(egui::Order::Foreground).pivot(Align2::RIGHT_TOP).fixed_pos(pos).show(ctx, |ui| {
            ui.multiply_opacity(fade);
            let bg = ui.painter().add(egui::Shape::Noop);
            let inner = egui::Frame::NONE.inner_margin(egui::Margin { left: 20, right: 20, top: 16, bottom: 18 }).show(ui, |ui| {
                ui.set_width(330.0);
                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Appearance").font(semibold(16.0)).color(t.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if w::icon_capsule(ui, t, "close", "Close", 26.0, false, true).clicked() {
                            g.glass.appearance_open = false;
                        }
                    });
                });

                heading(ui, t, "LOOK");
                let current = match g.app.ui.theme {
                    Appearance::Light => 0,
                    Appearance::Dark => 1,
                    Appearance::System => 2,
                };
                ui.vertical_centered(|ui| {
                    if let Some(i) = w::segmented(ui, t, &["Light", "Dark", "Automatic"], current, 32.0) {
                        let v = ["light", "dark", "system"].get(i).copied().unwrap_or("system");
                        w::run(ui, &mut g.app, "ui.theme", json!({"value": v}));
                    }
                });
                ui.add_space(4.0);
                row(ui, t, "Accent colour", |ui| {
                    ui.spacing_mut().item_spacing = vec2(7.0, 0.0);
                    for a in Accent::ALL.iter().rev() {
                        let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                        let c = a.color(t.dark);
                        ui.painter().circle_filled(r.center(), 8.0, c);
                        let sel = ui.ctx().animate_bool_with_time(resp.id.with("sel"), g.glass.prefs.material.accent == *a, 0.15);
                        if sel > 0.0 {
                            ui.painter().circle_stroke(r.center(), 10.5, egui::Stroke::new(2.0, c.gamma_multiply(sel)));
                            ui.painter().circle_filled(r.center(), 3.0 * sel, egui::Color32::WHITE);
                        }
                        if resp.on_hover_text(a.name()).clicked() {
                            g.glass.prefs.material.accent = *a;
                        }
                    }
                });
                row(ui, t, "Window background", |ui| {
                    ui.spacing_mut().item_spacing = vec2(7.0, 0.0);
                    for tone in CanvasTone::ALL.iter().rev() {
                        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 18.0), Sense::click());
                        ui.painter().rect_filled(r, 6.0, tone.color(t.dark));
                        let on = g.glass.prefs.material.canvas == *tone;
                        let stroke = if on { egui::Stroke::new(2.0, t.accent) } else { egui::Stroke::new(1.0, t.edge) };
                        ui.painter().rect_stroke(r, 6.0, stroke, egui::StrokeKind::Outside);
                        if resp.on_hover_text(tone.name()).clicked() {
                            g.glass.prefs.material.canvas = *tone;
                        }
                    }
                });
                divider(ui, t);

                heading(ui, t, "LIQUID GLASS");
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Clear").font(regular(12.0)).color(t.text_dim));
                    ui.spacing_mut().slider_width = 200.0;
                    let mut tint = g.glass.prefs.material.tint;
                    if ui.add(egui::Slider::new(&mut tint, 0.0..=1.0).show_value(false)).changed() {
                        g.glass.prefs.material.tint = tint;
                    }
                    ui.label(egui::RichText::new("Tinted").font(regular(12.0)).color(t.text_dim));
                });
                row(ui, t, "Reduce transparency", |ui| {
                    let mut on = g.glass.prefs.material.reduce_transparency;
                    if w::switch(ui, t, &mut on).changed() {
                        g.glass.prefs.material.reduce_transparency = on;
                    }
                });
                divider(ui, t);

                heading(ui, t, "TEXT AND MOTION");
                let size = g.app.ui.interface_size.percent();
                let sel = if size < 95 {
                    0
                } else if size > 105 {
                    2
                } else {
                    1
                };
                row(ui, t, "Interface size", |ui| {
                    if let Some(i) = w::segmented(ui, t, &["S", "M", "L"], sel, 28.0) {
                        let p = [90, 100, 115].get(i).copied().unwrap_or(100);
                        w::run(ui, &mut g.app, "ui.interfaceSize", json!({"percent": p}));
                    }
                });
                let motion = match g.app.ui.animations {
                    Animations::System => 0,
                    Animations::On => 1,
                    Animations::Off => 2,
                };
                row(ui, t, "Animations", |ui| {
                    if let Some(i) = w::segmented(ui, t, &["System", "On", "Off"], motion, 28.0) {
                        let v = ["system", "on", "off"].get(i).copied().unwrap_or("system");
                        w::run(ui, &mut g.app, "ui.animations", json!({"value": v}));
                    }
                });
                row(ui, t, "Dark page", |ui| {
                    let mut on = g.app.session.view.dark_mode;
                    if w::switch(ui, t, &mut on).changed() {
                        w::run(ui, &mut g.app, "view.darkMode", json!({}));
                    }
                });
                row(ui, t, "Typewriter mode", |ui| {
                    let mut on = g.glass.prefs.typewriter;
                    if w::switch(ui, t, &mut on).changed() {
                        g.glass.prefs.typewriter = on;
                    }
                });
                divider(ui, t);

                heading(ui, t, "PANELS");
                row(ui, t, "Floating panels", |ui| {
                    let mut on = g.glass.prefs.floating;
                    if w::switch(ui, t, &mut on).changed() {
                        g.glass.prefs.floating = on;
                    }
                });
                ui.label(
                    egui::RichText::new("Side panels float over the page as frosted glass. Drag a panel's inner edge to resize it.")
                        .font(regular(11.5))
                        .color(t.text_dim),
                );
                if !g.glass.prefs.docks.is_empty() && w::quiet_button(ui, t, "Reset Panel Sizes", None, 28.0).clicked() {
                    g.glass.prefs.docks.clear();
                }
                divider(ui, t);

                heading(ui, t, "WRITING");
                let glow = match g.glass.prefs.glow {
                    Glow::Off => 0,
                    Glow::Paragraph => 1,
                    Glow::Sentence => 2,
                };
                row(ui, t, "Reader's Glow", |ui| {
                    if let Some(i) = w::segmented(ui, t, &["Off", "Paragraph", "Sentence"], glow, 28.0) {
                        g.glass.prefs.glow = [Glow::Off, Glow::Paragraph, Glow::Sentence].get(i).copied().unwrap_or(Glow::Off);
                    }
                });
                row(ui, t, "Rhythm marks", |ui| {
                    let mut on = g.glass.prefs.rhythm;
                    if w::switch(ui, t, &mut on).changed() {
                        g.glass.prefs.rhythm = on;
                    }
                });
                row(ui, t, "Layout guides", |ui| {
                    let mut on = g.glass.prefs.grid;
                    if w::switch(ui, t, &mut on).changed() {
                        g.glass.prefs.grid = on;
                    }
                });
                row(ui, t, "Block handles (Writer Pro)", |ui| {
                    let mut on = g.glass.prefs.handles;
                    if w::switch(ui, t, &mut on).changed() {
                        g.glass.prefs.handles = on;
                    }
                });
                divider(ui, t);

                heading(ui, t, "MODE");
                row(ui, t, "Writer Pro", |ui| {
                    let mut on = g.glass.prefs.pro;
                    if w::switch(ui, t, &mut on).changed() {
                        g.set_pro(on);
                    }
                });
                ui.label(
                    egui::RichText::new("Binder, Corkboard, Outliner, notes, [[links]], / commands, Scratchpad, pacing and sprints.")
                        .font(regular(11.5))
                        .color(t.text_dim),
                );
            });
            ui.painter().set(bg, popover_shape(inner.response.rect, 22, t));
        });
    let close = ctx.input(|i| {
        i.key_pressed(egui::Key::Escape) || (i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)))
    });
    if close && g.glass.appearance_open && !ctx.any_popup_open() {
        g.glass.appearance_open = false;
    }
}

//! Colour labels (colour coding, as in Scrivener): the writer's own labels, each a name, a
//! colour and an icon, for characters, points of view, timelines or topics. A label picker for
//! the Inspector and notes, the mark drawn in the Binder and on cards, and the Labels editor.

use egui::{Align2, Color32, Rect, Sense, Ui, pos2, vec2};
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::GlassApp;
use crate::db::Tint;
use crate::glyphs;
use crate::project::{Tag, key};
use crate::tokens::{Tokens, paint_state, radius};

/// A label's colour.
pub fn color(tint: Tint) -> Color32 {
    let [r, g, b] = tint.strong();
    Color32::from_rgb(r, g, b)
}

/// Draw a label's mark (its icon in its colour) in `r`.
pub fn mark(p: &egui::Painter, r: Rect, tag: &Tag) {
    glyphs::paint(p, r, &tag.icon, color(tag.tint));
}

/// A pill showing the label (or "No Label"), `width` wide; click it for [`menu`].
pub fn pill(ui: &mut Ui, t: &Tokens, tag: Option<&Tag>, width: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    paint_state(ui, r, radius::CAPSULE, &resp, false, t);
    ui.painter().rect_stroke(r, 15.0, egui::Stroke::new(0.75, t.edge), egui::StrokeKind::Inside);
    match tag {
        Some(tag) => {
            mark(ui.painter(), Rect::from_center_size(pos2(r.min.x + 16.0, r.center().y), vec2(14.0, 14.0)), tag);
            ui.painter().with_clip_rect(r.shrink(4.0)).text(pos2(r.min.x + 30.0, r.center().y), Align2::LEFT_CENTER, &tag.name, medium(12.0), t.text);
        }
        None => {
            ui.painter().text(pos2(r.min.x + 14.0, r.center().y), Align2::LEFT_CENTER, "No Label", medium(12.0), t.text_dim);
        }
    }
    resp
}

/// What the label menu chose.
pub enum Choice {
    Set(Option<u64>),
    Edit,
}

/// The label menu: every label, No Label, and Edit Labels…
pub fn menu(ui: &mut Ui, t: &Tokens, tags: &[Tag], current: Option<u64>) -> Option<Choice> {
    ui.set_min_width(200.0);
    let mut out = None;
    for tag in tags {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
        paint_state(ui, r, 6, &resp, current == Some(tag.id), t);
        mark(ui.painter(), Rect::from_center_size(pos2(r.min.x + 14.0, r.center().y), vec2(14.0, 14.0)), tag);
        ui.painter().text(pos2(r.min.x + 30.0, r.center().y), Align2::LEFT_CENTER, &tag.name, regular(12.5), t.text);
        if resp.clicked() {
            out = Some(Choice::Set(Some(tag.id)));
        }
    }
    ui.separator();
    if ui.selectable_label(current.is_none(), "No Label").clicked() {
        out = Some(Choice::Set(None));
    }
    if ui.button("Edit Labels…").clicked() {
        out = Some(Choice::Edit);
    }
    if out.is_some() {
        ui.close();
    }
    out
}

/// The Labels editor: rename, recolour, change the icon, add and delete labels.
pub fn editor(g: &mut GlassApp, ctx: &egui::Context, t: &Tokens) {
    if !g.glass.labels_open {
        return;
    }
    let pkey = key(&g.app.session);
    let tags: Vec<Tag> = g.glass.projects.by_doc.entry(pkey.clone()).or_default().tags().to_vec();
    let screen = ctx.content_rect();
    let mut close = false;
    egui::Area::new(egui::Id::new("glass_labels_scrim")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(50));
        if ui.allocate_rect(screen, Sense::click()).clicked() {
            close = true;
        }
    });
    egui::Area::new(egui::Id::new("glass_labels")).order(egui::Order::Tooltip).pivot(Align2::CENTER_CENTER).fixed_pos(screen.center()).show(
        ctx,
        |ui| {
            let bg = ui.painter().add(egui::Shape::Noop);
            let inner = egui::Frame::NONE.inner_margin(22).show(ui, |ui| {
                ui.set_width(440.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Labels").font(semibold(17.0)).color(t.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::widgets::icon_capsule(ui, t, "close", "Done", 28.0, false, true).clicked() {
                            close = true;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(
                        "Colour-code sections and notes: characters, points of view, timelines, topics. Each label has a colour and an icon.",
                    )
                    .font(regular(12.0))
                    .color(t.text_dim),
                );
                ui.add_space(12.0);
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    for tag in &tags {
                        ui.horizontal(|ui| {
                            // The icon: click for the icon and colour picker.
                            let (r, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
                            paint_state(ui, r, 8, &resp, false, t);
                            mark(ui.painter(), r.shrink(8.0), tag);
                            egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                                ui.label(egui::RichText::new("Colour").font(medium(11.0)).color(t.text_dim));
                                ui.horizontal_wrapped(|ui| {
                                    ui.set_max_width(220.0);
                                    for tint in Tint::ALL {
                                        let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                                        ui.painter().circle_filled(r.center(), 9.0, color(tint));
                                        if tint == tag.tint {
                                            ui.painter().circle_stroke(r.center(), 11.0, egui::Stroke::new(2.0, t.accent));
                                        }
                                        if resp.on_hover_text(tint.name()).clicked()
                                            && let Some(x) = g.glass.projects.by_doc.get_mut(&pkey).and_then(|p| p.tag_mut(tag.id))
                                        {
                                            x.tint = tint;
                                        }
                                    }
                                });
                                ui.add_space(6.0);
                                ui.label(egui::RichText::new("Icon").font(medium(11.0)).color(t.text_dim));
                                ui.horizontal_wrapped(|ui| {
                                    ui.set_max_width(220.0);
                                    for icon in glyphs::LABELS {
                                        let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
                                        paint_state(ui, r, 6, &resp, icon == tag.icon, t);
                                        glyphs::paint(ui.painter(), r.shrink(7.0), icon, color(tag.tint));
                                        if resp.clicked()
                                            && let Some(x) = g.glass.projects.by_doc.get_mut(&pkey).and_then(|p| p.tag_mut(tag.id))
                                        {
                                            x.icon = icon.to_string();
                                        }
                                    }
                                });
                            });
                            let mut name = tag.name.clone();
                            if ui.add(egui::TextEdit::singleline(&mut name).desired_width(300.0).margin(vec2(8.0, 6.0))).changed()
                                && let Some(x) = g.glass.projects.by_doc.get_mut(&pkey).and_then(|p| p.tag_mut(tag.id))
                            {
                                x.name = name.chars().take(80).collect();
                            }
                            if crate::widgets::icon_capsule(ui, t, "close", "Delete this label", 24.0, false, true).clicked()
                                && let Some(p) = g.glass.projects.by_doc.get_mut(&pkey)
                            {
                                p.remove_tag(tag.id);
                            }
                        });
                        ui.add_space(4.0);
                    }
                });
                ui.add_space(8.0);
                if crate::widgets::quiet_button(ui, t, "+ New Label", Some(130.0), 30.0).clicked()
                    && let Some(p) = g.glass.projects.by_doc.get_mut(&pkey)
                {
                    let n = p.tags.len();
                    p.add_tag("New label", Tint::nth(n + 3), glyphs::LABELS.get(n % glyphs::LABELS.len()).copied().unwrap_or("circle"));
                }
            });
            ui.painter().set(bg, crate::tokens::popover_shape(inner.response.rect, 20, t));
        },
    );
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        g.glass.labels_open = false;
    }
}

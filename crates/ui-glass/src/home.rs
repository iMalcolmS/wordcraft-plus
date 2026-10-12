//! Home: where the app starts. A greeting, today's writing, new documents from templates, recent
//! files, and the switch to Writer Pro.

use egui::{Align2, Rect, Sense, Ui, pos2, vec2};
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::GlassApp;
use crate::chrome::{group_digits, ring};
use crate::goals::minutes_text;
use crate::tokens::{Tokens, glass_shape, paint_state, radius};
use crate::widgets as w;

const TEMPLATES: [(&str, &str); 5] =
    [("Blank", "blank"), ("Studio Handbook", "sample"), ("Letter", "letter"), ("Résumé", "resume"), ("Report", "report")];

fn greeting() -> &'static str {
    let hour = wordcraft_engine::cmd::now_iso().get(11..13).and_then(|h| h.parse::<u32>().ok()).unwrap_or(9);
    if hour < 12 {
        "Good morning"
    } else if hour < 18 {
        "Good afternoon"
    } else {
        "Good evening"
    }
}

/// A glass card laid out by `add`, `width` wide.
fn card(ui: &mut Ui, t: &Tokens, width: f32, add: impl FnOnce(&mut Ui)) -> egui::Response {
    let bg = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Frame::NONE.inner_margin(20).show(ui, |ui| {
        ui.set_width(width - 40.0);
        add(ui);
    });
    ui.painter().set(bg, glass_shape(inner.response.rect, radius::PANEL, t));
    inner.response
}

fn section_title(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add_space(26.0);
    ui.label(egui::RichText::new(text).font(semibold(15.0)).color(t.text));
    ui.add_space(10.0);
}

/// A paper tile: a little page (blank, or with lines suggesting text) and its name.
fn template_tile(ui: &mut Ui, t: &Tokens, label: &str, kind: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(132.0, 196.0), Sense::click());
    let page = Rect::from_min_size(r.min, vec2(132.0, 170.0));
    let lift = ui.ctx().animate_bool_with_time(resp.id.with("lift"), resp.hovered(), 0.15);
    let page = page.translate(vec2(0.0, -3.0 * lift));
    let shadow = egui::epaint::Shadow { offset: [0, (4.0 + 6.0 * lift) as i8], blur: (14.0 + 12.0 * lift) as u8, spread: 0, color: t.shadow };
    ui.painter().add(shadow.as_shape(page, egui::CornerRadius::same(8)));
    ui.painter().rect_filled(page, 8.0, egui::Color32::WHITE);
    let ink = egui::Color32::from_gray(205);
    let x0 = page.min.x + 16.0;
    match kind {
        "blank" => {
            let c = page.center();
            let st = egui::Stroke::new(1.6, t.accent);
            ui.painter().line_segment([pos2(c.x - 9.0, c.y), pos2(c.x + 9.0, c.y)], st);
            ui.painter().line_segment([pos2(c.x, c.y - 9.0), pos2(c.x, c.y + 9.0)], st);
        }
        _ => {
            let title_w = if kind == "letter" { 46.0 } else { 70.0 };
            ui.painter().rect_filled(Rect::from_min_size(pos2(x0, page.min.y + 20.0), vec2(title_w, 6.0)), 2.0, egui::Color32::from_gray(90));
            for k in 0..9 {
                let y = page.min.y + 40.0 + k as f32 * 12.0;
                let w = if k % 4 == 3 { 60.0 } else { 100.0 };
                ui.painter().rect_filled(Rect::from_min_size(pos2(x0, y), vec2(w, 3.0)), 1.5, ink);
            }
        }
    }
    if lift > 0.0 {
        ui.painter().rect_stroke(page, 8.0, egui::Stroke::new(2.0, t.accent.gamma_multiply(lift)), egui::StrokeKind::Outside);
    }
    ui.painter().text(pos2(r.center().x, r.max.y - 10.0), Align2::CENTER_CENTER, label, medium(12.5), t.text);
    resp
}

pub fn show(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let width = (ui.available_width() - 48.0).clamp(320.0, 980.0);
        let margin = ((ui.available_width() - width) / 2.0).max(24.0);
        ui.horizontal_top(|ui| {
            ui.add_space(margin);
            ui.vertical(|ui| {
                ui.set_max_width(width);
                ui.add_space(36.0);
                let author = g.app.session.author.split_whitespace().next().unwrap_or("").to_string();
                let hello = if author.is_empty() { greeting().to_string() } else { format!("{}, {author}", greeting()) };
                ui.label(egui::RichText::new(hello).font(semibold(30.0)).color(t.text));
                ui.add_space(2.0);
                ui.label(egui::RichText::new("What will you write today?").font(regular(15.0)).color(t.text_dim));
                ui.add_space(22.0);

                // Today.
                let today = GlassApp::today();
                let goals = g.glass.prefs.goals.clone();
                let day = goals.today(&today);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = vec2(16.0, 0.0);
                    let half = (width - 16.0) / 2.0;
                    card(ui, t, half, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::hover());
                            ring(ui.painter(), r.center(), 30.0, 7.0, goals.progress(&today), t.fill, t.accent);
                            ui.add_space(12.0);
                            ui.vertical(|ui| {
                                ui.add_space(6.0);
                                ui.label(egui::RichText::new("Today").font(medium(12.0)).color(t.text_dim));
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} of {} words",
                                        group_digits(day.words.max(0)),
                                        group_digits(i64::from(goals.daily))
                                    ))
                                    .font(semibold(18.0))
                                    .color(t.text),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{} writing · {} day streak", minutes_text(day.secs), goals.streak(&today)))
                                        .font(regular(12.5))
                                        .color(t.text_dim),
                                );
                            });
                        });
                    });
                    card(ui, t, half, |ui| {
                        ui.label(egui::RichText::new("This week").font(medium(12.0)).color(t.text_dim));
                        ui.add_space(8.0);
                        let week = goals.week(&today);
                        let max = week.iter().map(|(_, w)| *w).max().unwrap_or(0).max(i64::from(goals.daily)).max(1) as f32;
                        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::hover());
                        let col = r.width() / 7.0;
                        for (i, (letter, words)) in week.iter().enumerate() {
                            let x = r.min.x + col * i as f32;
                            let h = ((*words as f32 / max) * 30.0).max(4.0);
                            let bar = Rect::from_min_max(pos2(x + col * 0.3, r.min.y + 32.0 - h), pos2(x + col * 0.7, r.min.y + 32.0));
                            ui.painter().rect_filled(bar, 4.0, if i == 6 { t.accent } else { t.fill });
                            ui.painter().text(
                                pos2(bar.center().x, r.min.y + 44.0),
                                Align2::CENTER_CENTER,
                                letter.to_string(),
                                regular(10.5),
                                t.text_dim,
                            );
                        }
                    });
                });

                // New.
                section_title(ui, t, "New");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(22.0, 18.0);
                    for (label, kind) in TEMPLATES {
                        if template_tile(ui, t, label, kind).clicked() {
                            g.new_from_template(kind);
                        }
                    }
                });

                // Recent.
                section_title(ui, t, "Recent");
                let recent: Vec<String> = g.app.ui.recent.iter().take(8).cloned().collect();
                let mut open: Option<String> = None;
                if recent.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Documents you open will appear here.").font(regular(13.0)).color(t.text_dim));
                        ui.add_space(12.0);
                        if w::quiet_button(ui, t, "Open…", Some(96.0), 30.0).clicked() {
                            open = Some(String::new());
                        }
                    });
                } else {
                    card(ui, t, width, |ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                        for path in &recent {
                            let p = std::path::Path::new(path);
                            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
                            let folder = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
                            paint_state(ui, r, 10, &resp, false, t);
                            let icon = Rect::from_min_size(pos2(r.min.x + 12.0, r.center().y - 12.0), vec2(19.0, 24.0));
                            ui.painter().rect_filled(icon, 3.0, egui::Color32::WHITE);
                            ui.painter().rect_stroke(icon, 3.0, egui::Stroke::new(1.0, t.edge), egui::StrokeKind::Outside);
                            ui.painter().text(pos2(r.min.x + 44.0, r.center().y - 8.0), Align2::LEFT_CENTER, &name, medium(13.0), t.text);
                            ui.painter().text(pos2(r.min.x + 44.0, r.center().y + 9.0), Align2::LEFT_CENTER, &folder, regular(11.5), t.text_dim);
                            if resp.clicked() {
                                open = Some(path.clone());
                            }
                        }
                        ui.add_space(6.0);
                        if w::quiet_button(ui, t, "Open…", Some(110.0), 32.0).clicked() {
                            open = Some(String::new());
                        }
                    });
                }
                match open {
                    Some(p) if p.is_empty() => g.open_dialog(),
                    Some(p) => g.open_path(&p),
                    None => {}
                }

                // Writer Pro.
                section_title(ui, t, "Writer Pro");
                card(ui, t, width, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(width - 140.0);
                            ui.label(egui::RichText::new("Tools for long writing").font(semibold(15.0)).color(t.text));
                            ui.add_space(4.0);
                            let text = "Binder, Corkboard and Outliner for your chapters, notes for characters and places, \
                                    [[wiki links]] and / commands as you type, a Scratchpad for cut text, pacing analysis \
                                    and writing sprints.";
                            ui.add(egui::Label::new(egui::RichText::new(text).font(regular(13.0)).color(t.text_dim)).wrap());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let mut on = g.glass.prefs.pro;
                            if w::switch(ui, t, &mut on).changed() {
                                g.set_pro(on);
                            }
                        });
                    });
                });
                ui.add_space(14.0);
                if w::quiet_button(ui, t, "Appearance…", Some(130.0), 32.0).clicked() {
                    g.glass.appearance_open = true;
                }
                ui.add_space(48.0);
            });
        });
    });
}

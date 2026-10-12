//! The title bar, the status capsule and the writing-goal popover.

use egui::{Align, Align2, Color32, Layout, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_ui_egui::chrome::MAC_CONTENT_LEFT;
use wordcraft_ui_egui::theme::{medium, regular, semibold};
use wordcraft_ui_egui::{WordApp, icons, widgets as wc};

use crate::goals::minutes_text;
use crate::ribbon::Local;
use crate::tokens::{Tokens, glass_shape, paint_fill, paint_glass, radius};
use crate::widgets as w;
use crate::{GlassApp, Panel};

/// A glass capsule holding `n` square buttons of side `size`, laid out left to right.
fn button_capsule(ui: &mut Ui, t: &Tokens, n: usize, size: f32, add: impl FnOnce(&mut Ui)) -> Rect {
    let w = n as f32 * size + n.saturating_sub(1) as f32 * 2.0 + 6.0;
    let (r, _) = ui.allocate_exact_size(vec2(w, size + 6.0), Sense::hover());
    ui.painter().add(glass_shape(r, radius::CAPSULE, t));
    ui.scope_builder(egui::UiBuilder::new().max_rect(r.shrink(3.0)).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
        add(ui);
    });
    r
}

/// The title bar: Quick Access capsule, the document's name, and the writing tools, search and
/// Share on the right. On macOS it shares the band with the window's traffic lights.
pub fn title_bar(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let mac = g.app.integrated_titlebar;
    let item_h = if mac { 28.0 } else { 34.0 };
    let left = if mac { MAC_CONTENT_LEFT as i8 } else { 10 };
    egui::Panel::top("glass_title")
        .show_separator_line(false)
        .exact_size(item_h + if mac { 6.0 } else { 16.0 })
        .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin { left, right: 10, top: if mac { 0 } else { 8 }, bottom: 0 }))
        .show(ui, |ui| {
            let full = ui.max_rect();
            let drag = ui.interact(full, ui.id().with("glass_title_drag"), Sense::click_and_drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            let row = Rect::from_min_size(full.min, vec2(full.width(), item_h));
            let mut left_end = row.min.x;
            let mut right_start = row.max.x;
            ui.scope_builder(egui::UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)), |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                // Quick Access Toolbar (Save, Undo, Redo, or the user's own).
                let ids: Vec<String> = g.app.session.ribbon.qat().into_iter().map(str::to_string).collect();
                let r = button_capsule(ui, t, ids.len(), item_h - 6.0, |ui| {
                    for id in &ids {
                        let label = g.app.session.registry.get(id).map(|s| s.label).unwrap_or(id.as_str()).to_string();
                        w::command_capsule(ui, &mut g.app, t, wc::command_icon(id), &label, id, item_h - 6.0);
                    }
                });
                left_end = r.max.x;
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                    if w::accent_button(ui, t, Some("share"), "Share", None, item_h - 2.0).on_hover_text("Save or export a copy to share").clicked() {
                        w::run(ui, &mut g.app, "file.share", json!({}));
                    }
                    // Search: the command palette.
                    let (sr, sresp) = ui.allocate_exact_size(vec2(200.0, item_h - 2.0), Sense::click());
                    paint_glass(ui.painter(), sr, radius::CAPSULE, t);
                    if sresp.hovered() {
                        paint_fill(ui.painter(), sr, radius::CAPSULE, t.hover);
                    }
                    icons::paint(
                        ui.painter(),
                        Rect::from_center_size(pos2(sr.min.x + 18.0, sr.center().y), vec2(15.0, 15.0)),
                        "search",
                        t.text_dim,
                        t.accent,
                    );
                    ui.painter().text(pos2(sr.min.x + 32.0, sr.center().y), Align2::LEFT_CENTER, "Search commands", regular(12.5), t.text_dim);
                    if sresp.on_hover_text("Find any command").clicked() {
                        w::run(ui, &mut g.app, "ui.dialog", json!({"name": "commands"}));
                    }
                    // Writing tools.
                    let r = button_capsule(ui, t, 4, item_h - 6.0, |ui| {
                        let s = item_h - 6.0;
                        let tw = g.glass.prefs.typewriter;
                        if w::icon_capsule(ui, t, "focus", "Typewriter mode: dims everything but the lines you are writing", s, tw, true).clicked() {
                            g.toggle(Local::Typewriter);
                        }
                        w::icon_capsule(ui, t, "split", "Split View is coming in a later version", s, false, false);
                        let snaps = g.glass.panel == Some(Panel::Snapshots);
                        if w::icon_capsule(ui, t, "screenshot", "Snapshots", s, snaps, true).clicked() {
                            g.toggle(Local::Snapshots);
                        }
                        let hist = g.glass.panel == Some(Panel::History);
                        if w::icon_capsule(ui, t, "replay", "Version History", s, hist, true).clicked() {
                            g.toggle(Local::History);
                        }
                    });
                    right_start = r.min.x;
                });
            });
            // The document's name, centred like a macOS window title, when there is room.
            let state = if g.app.session.dirty {
                "Edited"
            } else if g.app.session.path.is_some() {
                "Saved"
            } else {
                "Not saved"
            };
            let name = g.app.title_stem();
            let name_w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(name.clone(), semibold(13.0), t.text).size().x);
            let cx = row.center().x;
            if cx - name_w / 2.0 > left_end + 12.0 && cx + name_w / 2.0 < right_start - 12.0 {
                let two_lines = item_h >= 32.0;
                if two_lines {
                    ui.painter().text(pos2(cx, row.center().y - 7.0), Align2::CENTER_CENTER, &name, semibold(13.0), t.text);
                    ui.painter().text(pos2(cx, row.center().y + 8.0), Align2::CENTER_CENTER, state, regular(11.0), t.text_dim);
                } else {
                    let text = format!("{name} — {state}");
                    ui.painter().text(row.center(), Align2::CENTER_CENTER, text, semibold(12.5), t.text);
                }
            }
        });
}

/// Paint a progress ring: a full track and an arc for `progress` (0–1), starting at the top.
pub fn ring(p: &egui::Painter, c: Pos2, r: f32, width: f32, progress: f32, track: Color32, fill: Color32) {
    p.circle_stroke(c, r, Stroke::new(width, track));
    let progress = progress.clamp(0.0, 1.0);
    if progress <= 0.0 {
        return;
    }
    let n = ((64.0 * progress).ceil() as usize).max(2);
    let start = -std::f32::consts::FRAC_PI_2;
    let points: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = start + std::f32::consts::TAU * progress * (i as f32 / n as f32);
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    p.add(Shape::line(points, Stroke::new(width, fill)));
}

fn status_text(ui: &mut Ui, t: &Tokens, s: &str) -> egui::Response {
    let font = regular(12.0);
    let w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(s.to_string(), font.clone(), t.text_dim).size().x);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::click());
    ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, s, font, t.text_dim);
    resp
}

fn divider(ui: &mut Ui, t: &Tokens) {
    let (r, _) = ui.allocate_exact_size(vec2(1.0, 16.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.edge);
}

fn view_button(ui: &mut Ui, app: &mut WordApp, t: &Tokens, icon: &str, tip: &str, id: &str, on: bool) {
    let resp = w::icon_capsule(ui, t, icon, tip, 28.0, on, true);
    if resp.clicked() {
        w::run(ui, app, id, json!({}));
    }
}

/// The status capsule, centred at the bottom: goal ring, writing time, page, words, language,
/// views, zoom and Focus.
pub fn status_bar(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    egui::Panel::bottom("glass_status").show_separator_line(false).exact_size(52.0).frame(egui::Frame::NONE.fill(t.canvas)).show(ui, |ui| {
        let full = ui.max_rect();
        let wid = ui.id().with("glass_status_width");
        let last_w: f32 = ui.data(|d| d.get_temp(wid)).unwrap_or(760.0);
        let x = (full.center().x - last_w / 2.0).max(full.min.x + 10.0);
        let h = 38.0;
        let rect = Rect::from_min_size(pos2(x, full.center().y - h / 2.0 - 2.0), vec2((full.max.x - 10.0 - x).max(0.0), h));
        let bg = ui.painter().add(Shape::Noop);
        let used = ui
            .scope_builder(egui::UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)), |ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 0.0);
                ui.add_space(6.0);
                let today = GlassApp::today();
                let goals = &g.glass.prefs.goals;
                let done = goals.today(&today);
                let progress = goals.progress(&today);
                // Goal ring and words written today.
                let label = format!("{} / {}", group_digits(done.words.max(0)), group_digits(i64::from(goals.daily)));
                let lw = ui.ctx().fonts_mut(|f| f.layout_no_wrap(label.clone(), medium(12.0), t.text).size().x);
                let (gr, gresp) = ui.allocate_exact_size(vec2(lw + 40.0, 28.0), Sense::click());
                if let Some(fill) = crate::tokens::interaction_fill(&gresp, g.glass.goal_open, t) {
                    paint_fill(ui.painter(), gr, radius::CAPSULE, fill);
                }
                ring(ui.painter(), pos2(gr.min.x + 16.0, gr.center().y), 8.0, 2.6, progress, t.fill, t.accent);
                ui.painter().text(pos2(gr.min.x + 30.0, gr.center().y), Align2::LEFT_CENTER, label, medium(12.0), t.text);
                g.glass.ring_rect = Some(gr);
                if gresp.on_hover_text("Words written today against your daily goal").clicked() {
                    g.glass.goal_open = !g.glass.goal_open;
                }
                status_text(ui, t, &format!("{} today", minutes_text(done.secs)));
                divider(ui, t);
                let l = g.app.session.layout();
                let page = l.caret_on(&g.app.session.sel.focus, g.app.session.page_hint).map(|c| c.page + 1).unwrap_or(1);
                if status_text(ui, t, &format!("Page {page} of {}", l.pages.len())).on_hover_text("Go To").clicked() {
                    w::run(ui, &mut g.app, "ui.dialog", json!({"name": "goto"}));
                }
                let words = g.app.cached_word_count();
                if status_text(ui, t, &format!("{} words", group_digits(words as i64))).on_hover_text("Word Count").clicked() {
                    w::run(ui, &mut g.app, "ui.dialog", json!({"name": "wordCount"}));
                }
                if let Some((msg, at)) = g.app.status_msg.clone()
                    && wordcraft_ui_egui::now_ms() - at < 6000.0
                {
                    divider(ui, t);
                    let short: String = msg.chars().take(60).collect();
                    let font = medium(12.0);
                    let mw = ui.ctx().fonts_mut(|f| f.layout_no_wrap(short.clone(), font.clone(), t.accent).size().x);
                    let (r, _) = ui.allocate_exact_size(vec2(mw, 20.0), Sense::hover());
                    ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, short, font, t.accent);
                }
                divider(ui, t);
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                let v = &g.app.session.view;
                let (read, print, web) = (
                    v.read_mode,
                    v.mode == wordcraft_engine::layout::ViewMode::Print && !v.read_mode,
                    v.mode == wordcraft_engine::layout::ViewMode::Web,
                );
                view_button(ui, &mut g.app, t, "readMode", "Read Mode", "view.readMode", read);
                view_button(ui, &mut g.app, t, "printLayout", "Print Layout", "view.printLayout", print);
                view_button(ui, &mut g.app, t, "webLayout", "Web Layout", "view.webLayout", web);
                ui.add_space(8.0);
                divider(ui, t);
                ui.add_space(6.0);
                view_button(ui, &mut g.app, t, "minus", "Zoom Out", "view.zoomOut", false);
                let mut z = g.app.canvas.scale / wordcraft_ui_egui::canvas::PX_PER_PT;
                ui.spacing_mut().slider_width = 96.0;
                if ui.add(egui::Slider::new(&mut z, 0.1..=5.0).logarithmic(true).show_value(false)).changed() {
                    w::run(ui, &mut g.app, "view.zoom", json!({"value": (z * 100.0).round()}));
                }
                view_button(ui, &mut g.app, t, "plus", "Zoom In", "view.zoomIn", false);
                let pct = format!("{}%", (g.app.canvas.scale / wordcraft_ui_egui::canvas::PX_PER_PT * 100.0).round());
                if status_text(ui, t, &pct).on_hover_text("Zoom").clicked() {
                    w::run(ui, &mut g.app, "ui.dialog", json!({"name": "zoom"}));
                }
                ui.add_space(8.0);
                let focus = g.app.session.view.focus_mode;
                let label = if focus { "Exit Focus" } else { "Focus" };
                let resp = if focus { w::accent_button(ui, t, Some("focus"), label, None, 30.0) } else { w::quiet_button(ui, t, label, None, 30.0) };
                if resp.on_hover_text("Hide the ribbon and panes, leaving only the page").clicked() {
                    w::run(ui, &mut g.app, "view.focus", json!({}));
                }
                ui.add_space(4.0);
            })
            .response
            .rect;
        let used = Rect::from_min_max(pos2(rect.min.x, rect.min.y), pos2(used.max.x, rect.max.y));
        ui.painter().set(bg, glass_shape(used, radius::CAPSULE, t));
        g.glass.status_rect = Some(used);
        if (used.width() - last_w).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(wid, used.width()));
            ui.ctx().request_repaint();
        }
    });
}

/// 1234567 → "1,234,567".
pub fn group_digits(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// The writing-goal popover, above the status capsule.
pub fn goal_popover(g: &mut GlassApp, ctx: &egui::Context, t: &Tokens) {
    if !g.glass.goal_open {
        return;
    }
    let Some(anchor) = g.glass.ring_rect else { return };
    let width = 300.0;
    let pos = pos2(anchor.min.x, anchor.min.y - 12.0);
    let today = GlassApp::today();
    let area = egui::Area::new(egui::Id::new("glass_goal_popover")).order(egui::Order::Foreground).pivot(Align2::LEFT_BOTTOM).fixed_pos(pos).show(
        ctx,
        |ui| {
            let bg = ui.painter().add(Shape::Noop);
            let inner = egui::Frame::NONE.inner_margin(16).show(ui, |ui| {
                ui.set_width(width - 32.0);
                ui.spacing_mut().item_spacing = vec2(8.0, 12.0);
                let goals = &g.glass.prefs.goals;
                let day = goals.today(&today);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
                    ring(ui.painter(), r.center(), 26.0, 6.0, goals.progress(&today), t.fill, t.accent);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(group_digits(day.words.max(0))).font(semibold(24.0)).color(t.text));
                        let pct = (goals.progress(&today) * 100.0).round();
                        ui.label(
                            egui::RichText::new(format!("of {} words today · {pct}%", group_digits(i64::from(goals.daily))))
                                .font(regular(11.5))
                                .color(t.text_dim),
                        );
                    });
                });
                ui.columns(3, |cols| {
                    let stats = [
                        (group_digits(g.glass.tracker.session_words.max(0)), "this session"),
                        (minutes_text(day.secs), "writing today"),
                        (
                            match goals.streak(&today) {
                                1 => "1 day".to_string(),
                                n => format!("{n} days"),
                            },
                            "writing streak",
                        ),
                    ];
                    for (col, (n, l)) in cols.iter_mut().zip(stats) {
                        col.spacing_mut().item_spacing = vec2(0.0, 1.0);
                        col.label(egui::RichText::new(n).font(semibold(15.0)).color(t.text));
                        col.label(egui::RichText::new(l).font(regular(10.5)).color(t.text_dim));
                    }
                });
                // The last seven days.
                let week = goals.week(&today);
                let max = week.iter().map(|(_, w)| *w).max().unwrap_or(0).max(i64::from(goals.daily)).max(1) as f32;
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::hover());
                let col_w = r.width() / 7.0;
                for (i, (letter, words)) in week.iter().enumerate() {
                    let x = r.min.x + col_w * i as f32;
                    let bar_h = ((*words as f32 / max) * 40.0).max(4.0);
                    let bar = Rect::from_min_max(pos2(x + 6.0, r.min.y + 44.0 - bar_h), pos2(x + col_w - 6.0, r.min.y + 44.0));
                    let fill = if i == 6 {
                        t.accent
                    } else if *words >= i64::from(goals.daily) {
                        t.accent.gamma_multiply(0.45)
                    } else {
                        t.fill
                    };
                    ui.painter().rect_filled(bar, 4.0, fill);
                    ui.painter().text(pos2(bar.center().x, r.min.y + 56.0), Align2::CENTER_CENTER, letter.to_string(), regular(10.0), t.text_dim);
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Daily goal").font(regular(12.0)).color(t.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut daily = g.glass.prefs.goals.daily;
                        if ui.add(egui::DragValue::new(&mut daily).range(50..=20_000).speed(10).suffix(" words")).changed() {
                            g.glass.prefs.goals.daily = daily;
                        }
                    });
                });
            });
            ui.painter().set(bg, crate::tokens::popover_shape(inner.response.rect, radius::PANEL, t));
        },
    );
    // A click anywhere else closes it.
    let clicked_outside =
        ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p)));
    if clicked_outside {
        g.glass.goal_open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_are_grouped() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(1000), "1,000");
        assert_eq!(group_digits(-1234567), "-1,234,567");
    }
}

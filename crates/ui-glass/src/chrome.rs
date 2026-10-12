//! The title bar, the status capsule and the writing-goal popover.

use egui::{Align, Align2, Color32, Layout, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_ui_egui::theme::{medium, regular, semibold};
use wordcraft_ui_egui::{WordApp, widgets as wc};

use crate::goals::minutes_text;
use crate::ribbon::Local;
use crate::tokens::{Tokens, glass_shape, radius};
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

/// Let the window be moved by dragging empty chrome, and zoomed by double-clicking it, as macOS
/// title bars and toolbars do.
fn window_drag(ui: &mut Ui, rect: Rect, salt: &str) {
    let drag = ui.interact(rect, ui.id().with(salt), Sense::click_and_drag());
    if drag.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
    }
}

/// macOS: the title band beside the traffic lights, holding only the document's name and state,
/// like any Mac window. (Elsewhere the system title bar shows the name.)
pub fn title_band(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    if !g.app.integrated_titlebar {
        return;
    }
    egui::Panel::top("glass_title").show_separator_line(false).exact_size(28.0).frame(egui::Frame::NONE.fill(t.canvas)).show(ui, |ui| {
        let full = ui.max_rect();
        window_drag(ui, full, "glass_title_drag");
        let state = if g.app.session.dirty {
            "Edited"
        } else if g.app.session.path.is_some() {
            "Saved"
        } else {
            "Not saved"
        };
        let name = g.app.title_stem();
        let mut job = egui::text::LayoutJob::default();
        job.append(&name, 0.0, egui::TextFormat { font_id: semibold(13.0), color: t.text, ..Default::default() });
        job.append(&format!("  —  {state}"), 0.0, egui::TextFormat { font_id: regular(13.0), color: t.text_dim, ..Default::default() });
        let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
        let pos = pos2(full.center().x - galley.size().x / 2.0, full.center().y - galley.size().y / 2.0);
        ui.painter().galley(pos, galley, t.text);
    });
}

/// The toolbar: Quick Access on the left, the tabs in the middle, writing tools, search and Share
/// on the right, all on one line, the way a Mac app's unified toolbar reads.
pub fn toolbar(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let mac = g.app.integrated_titlebar;
    egui::Panel::top("glass_toolbar")
        .show_separator_line(false)
        .exact_size(if mac { 48.0 } else { 56.0 })
        .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin { left: 14, right: 14, top: if mac { 2 } else { 10 }, bottom: 8 }))
        .show(ui, |ui| {
            let full = ui.max_rect();
            window_drag(ui, full, "glass_toolbar_drag");
            let h = 34.0;
            let row = Rect::from_min_size(pos2(full.min.x, full.center().y - h / 2.0), vec2(full.width(), h));
            let mut left_end = row.min.x;
            let mut right_start = row.max.x;
            ui.scope_builder(egui::UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)), |ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 0.0);
                let ids: Vec<String> = g.app.session.ribbon.qat().into_iter().map(str::to_string).collect();
                let r = button_capsule(ui, t, ids.len(), h - 6.0, |ui| {
                    for id in &ids {
                        let label = g.app.session.registry.get(id).map(|s| s.label).unwrap_or(id.as_str()).to_string();
                        w::command_capsule(ui, &mut g.app, t, wc::command_icon(id), &label, id, h - 6.0);
                    }
                });
                left_end = r.max.x;
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = vec2(10.0, 0.0);
                    if w::accent_button(ui, t, Some("share"), "Share", None, h).on_hover_text("Save or export a copy to share").clicked() {
                        w::run(ui, &mut g.app, "file.share", json!({}));
                    }
                    let r = button_capsule(ui, t, 1, h - 6.0, |ui| {
                        if w::icon_capsule(ui, t, "search", "Search commands", h - 6.0, false, true).clicked() {
                            w::run(ui, &mut g.app, "ui.dialog", json!({"name": "commands"}));
                        }
                    });
                    let _ = r;
                    let r = button_capsule(ui, t, 4, h - 6.0, |ui| {
                        let s = h - 6.0;
                        let tw = g.glass.prefs.typewriter;
                        if w::icon_capsule(ui, t, "focus", "Typewriter mode: fades everything but the lines you're writing", s, tw, true).clicked() {
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
            let space = Rect::from_min_max(pos2(left_end + 16.0, row.min.y), pos2(right_start - 16.0, row.max.y));
            tab_strip(g, ui, t, space);
        });
}

/// Tabs that live behind "More" unless one of them is open.
const EXTRA_TABS: [&str; 2] = ["Developer", "Zotero"];

/// The tabs as a glass segmented control. The selected tab's pill glides to the tab you pick.
fn tab_strip(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, space: Rect) {
    let current = g.app.ui.tab.clone();
    let all = crate::ribbon::tabs(&g.app);
    let mut names: Vec<String> = all.iter().filter(|n| !EXTRA_TABS.contains(&n.as_str()) || **n == current).cloned().collect();
    let extras: Vec<String> = all.iter().filter(|n| EXTRA_TABS.contains(&n.as_str()) && **n != current).cloned().collect();
    if !extras.is_empty() {
        names.push("More".into());
    }
    let font = medium(13.0);
    let mut pad = 28.0;
    let text_w: Vec<f32> = names.iter().map(|n| ui.ctx().fonts_mut(|f| f.layout_no_wrap(n.clone(), font.clone(), t.text).size().x)).collect();
    let mut total: f32 = text_w.iter().sum::<f32>() + pad * names.len() as f32 + 6.0;
    if total > space.width() {
        pad = 16.0;
        total = text_w.iter().sum::<f32>() + pad * names.len() as f32 + 6.0;
    }
    let track = Rect::from_center_size(space.center(), vec2(total.min(space.width().max(0.0)), space.height()));
    ui.painter().add(glass_shape(track, radius::CAPSULE, t));
    let mut rects = Vec::with_capacity(names.len());
    let mut x = track.min.x + 3.0;
    for tw in &text_w {
        let r = Rect::from_min_size(pos2(x, track.min.y + 3.0), vec2(tw + pad, track.height() - 6.0));
        rects.push(r);
        x += tw + pad;
    }
    let selected = if g.glass.file_menu { names.iter().position(|n| n == "File") } else { names.iter().position(|n| *n == current) };
    if let Some(sel) = selected.and_then(|i| rects.get(i)) {
        let ctx = ui.ctx().clone();
        let px = ctx.animate_value_with_time(egui::Id::new("glass_tab_pill_x"), sel.min.x - track.min.x, 0.24);
        let pw = ctx.animate_value_with_time(egui::Id::new("glass_tab_pill_w"), sel.width(), 0.24);
        let pill = Rect::from_min_size(pos2(track.min.x + px, sel.min.y), vec2(pw, sel.height()));
        crate::tokens::paint_pill(ui.painter(), pill, t);
    }
    let clip = ui.clip_rect();
    for (i, (name, r)) in names.iter().zip(rects.iter()).enumerate() {
        if !clip.intersects(*r) || r.max.x > track.max.x + 1.0 {
            continue;
        }
        let resp = ui.interact(*r, ui.id().with(("glass_tab", name.as_str())), Sense::click());
        let on = selected == Some(i);
        let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
        let c = if on { t.text } else { lerp_color(t.text_dim, t.text, hover) };
        let label = if name == "More" { "More ⌄".to_string() } else { name.clone() };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, label, font.clone(), c);
        if name == "More" {
            egui::Popup::menu(&resp).show(|ui| {
                ui.set_min_width(170.0);
                for e in &extras {
                    if ui.button(e.as_str()).clicked() {
                        g.app.ui.tab = e.clone();
                        ui.close();
                    }
                }
            });
            continue;
        }
        if resp.clicked() {
            if name == "File" {
                g.glass.file_menu = !g.glass.file_menu;
                g.glass.file_anchor = Some(*r);
            } else {
                g.glass.file_menu = false;
                g.app.ui.tab = name.clone();
                if g.app.session.view.focus_mode {
                    w::run(ui, &mut g.app, "view.focus", json!({"value": false}));
                }
            }
        }
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_premultiplied(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()), mix(a.a(), b.a()))
}

/// One row of the File menu: a label and, on the right, its shortcut.
fn menu_row(ui: &mut Ui, t: &Tokens, label: &str, shortcut: &str, enabled: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    if enabled {
        crate::tokens::paint_state(ui, r, 8, &resp, false, t);
    }
    let c = if enabled { t.text } else { t.text_disabled };
    ui.painter().text(pos2(r.min.x + 12.0, r.center().y), Align2::LEFT_CENTER, label, regular(13.0), c);
    if !shortcut.is_empty() {
        ui.painter().text(pos2(r.max.x - 12.0, r.center().y), Align2::RIGHT_CENTER, shortcut, regular(12.0), t.text_dim);
    }
    resp
}

fn menu_section(ui: &mut Ui, t: &Tokens, title: &str) {
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    ui.painter().text(pos2(r.min.x + 12.0, r.center().y), Align2::LEFT_CENTER, title, semibold(11.0), t.text_dim);
}

fn menu_separator(ui: &mut Ui, t: &Tokens) {
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r.shrink2(vec2(10.0, 0.0)), 0.0, t.edge.gamma_multiply(0.6));
    ui.add_space(4.0);
}

/// The File menu: a glass menu under the File tab instead of a full-window takeover, the way a Mac
/// app's File menu works. Pages that need the whole window (templates, print, options) still open
/// in the full File view.
pub fn file_menu(g: &mut GlassApp, ctx: &egui::Context, t: &Tokens) {
    let open = g.glass.file_menu;
    let fade = ctx.animate_bool_with_time(egui::Id::new("glass_file_menu_fade"), open, 0.14);
    if fade <= 0.0 {
        return;
    }
    let Some(anchor) = g.glass.file_anchor else { return };
    let pos = pos2(anchor.min.x - 4.0, anchor.max.y + 10.0 - (1.0 - fade) * 6.0);
    let area = egui::Area::new(egui::Id::new("glass_file_menu")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        ui.multiply_opacity(fade);
        let bg = ui.painter().add(Shape::Noop);
        let inner = egui::Frame::NONE.inner_margin(6).show(ui, |ui| {
            ui.set_width(268.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let mut action: Option<(&str, serde_json::Value)> = None;
            let mut page: Option<&str> = None;
            let sc = |app: &WordApp, id: &str| wc::shortcut_text(app, id);
            if menu_row(ui, t, "New", &sc(&g.app, "file.new"), true).clicked() {
                action = Some(("file.new", json!({})));
            }
            if menu_row(ui, t, "New from Template…", "", true).clicked() {
                page = Some("new");
            }
            if menu_row(ui, t, "Open…", &sc(&g.app, "file.open"), true).clicked() {
                action = Some(("file.open", json!({})));
            }
            let recent: Vec<String> = g.app.ui.recent.iter().take(5).cloned().collect();
            if !recent.is_empty() {
                menu_section(ui, t, "Open Recent");
                for path in recent {
                    let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
                    if menu_row(ui, t, &name, "", true).on_hover_text(&path).clicked() {
                        g.glass.recent_pick = Some(path);
                    }
                }
            }
            menu_separator(ui, t);
            if menu_row(ui, t, "Save", &sc(&g.app, "file.save"), true).clicked() {
                action = Some(("file.save", json!({})));
            }
            if menu_row(ui, t, "Save As…", &sc(&g.app, "file.saveAs"), true).clicked() {
                action = Some(("file.saveAs", json!({})));
            }
            if menu_row(ui, t, "Export as PDF…", "", true).clicked() {
                page = Some("export");
            }
            if menu_row(ui, t, "Print…", &sc(&g.app, "file.print"), true).clicked() {
                page = Some("print");
            }
            menu_separator(ui, t);
            if menu_row(ui, t, "Document Info", "", true).clicked() {
                page = Some("info");
            }
            if menu_row(ui, t, "Settings…", "", true).clicked() {
                page = Some("options");
            }
            if let Some((id, p)) = action {
                g.glass.file_menu = false;
                w::run(ui, &mut g.app, id, p);
            }
            if let Some(pg) = page {
                g.glass.file_menu = false;
                g.app.ui.backstage = true;
                g.app.ui.backstage_page = pg.to_string();
            }
        });
        ui.painter().set(bg, crate::tokens::popover_shape(inner.response.rect, 16, t));
    });
    if let Some(path) = g.glass.recent_pick.take() {
        g.glass.file_menu = false;
        if let Err(e) = g.app.run("file.open", json!({"path": path})) {
            g.app.status(e);
        }
    }
    let close = ctx.input(|i| {
        i.key_pressed(egui::Key::Escape)
            || (i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p)))
    });
    if close && open {
        g.glass.file_menu = false;
    }
    if fade < 1.0 {
        ctx.request_repaint();
    }
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
                crate::tokens::paint_state(ui, gr, radius::CAPSULE, &gresp, g.glass.goal_open, t);
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
                    // Messages fade in, stay, and fade out over their last second.
                    let age = (wordcraft_ui_egui::now_ms() - at) as f32 / 1000.0;
                    let alpha = (age / 0.2).min(1.0).min((6.0 - age).clamp(0.0, 1.0));
                    ui.ctx().request_repaint();
                    divider(ui, t);
                    let short: String = msg.chars().take(60).collect();
                    let font = medium(12.0);
                    let mw = ui.ctx().fonts_mut(|f| f.layout_no_wrap(short.clone(), font.clone(), t.accent).size().x);
                    let (r, _) = ui.allocate_exact_size(vec2(mw, 20.0), Sense::hover());
                    ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, short, font, t.accent.gamma_multiply(alpha));
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
    // Fades and rises into place, and back down when it closes.
    let fade = ctx.animate_bool_with_time(egui::Id::new("glass_goal_fade"), g.glass.goal_open, 0.16);
    if fade <= 0.0 {
        return;
    }
    if fade < 1.0 {
        ctx.request_repaint();
    }
    let Some(anchor) = g.glass.ring_rect else { return };
    let width = 300.0;
    let pos = pos2(anchor.min.x, anchor.min.y - 12.0 + (1.0 - fade) * 8.0);
    let today = GlassApp::today();
    let area = egui::Area::new(egui::Id::new("glass_goal_popover")).order(egui::Order::Foreground).pivot(Align2::LEFT_BOTTOM).fixed_pos(pos).show(
        ctx,
        |ui| {
            ui.multiply_opacity(fade);
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

//! Writer Pro: the Binder, the Corkboard and Outliner views, notes, the Scratchpad, pacing analysis
//! and writing sprints. Scrivener's way of seeing a manuscript, with Notion's and Obsidian's ways
//! of writing in it (see `inline.rs`).

use egui::{Align2, Rect, Sense, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_engine::doc::{Pos, StoryRef};
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::analysis::{Metrics, analyze};
use crate::project::{NoteKind, Section, Status, key, sections};
use crate::tokens::{Tokens, glass_shape, paint_state, radius};
use crate::widgets as w;
use crate::{GlassApp, Panel, View};

/// A status's colour (the dot and chip on cards).
pub fn status_color(s: Status, t: &Tokens) -> egui::Color32 {
    match s {
        Status::ToDo => t.text_disabled,
        Status::Draft => egui::Color32::from_rgb(0xE0, 0x9B, 0x2D),
        Status::Revised => egui::Color32::from_rgb(0x3D, 0x8B, 0xE8),
        Status::Done => egui::Color32::from_rgb(0x34, 0xA8, 0x5A),
    }
}

/// Put the caret at a section's heading and show it on the page.
pub fn go_to(g: &mut GlassApp, sec: &Section) {
    let pos = Pos { story: StoryRef::Body, path: sec.path.clone(), off: 0 };
    if let Err(e) = g.app.run("caret.set", json!({ "pos": pos })) {
        g.app.status(e);
    }
    g.app.canvas.scroll_to_caret = true;
    g.app.canvas.want_focus = true;
    g.set_view(View::Page);
}

/// The section the caret is in.
fn current_section(g: &GlassApp, secs: &[Section]) -> Option<usize> {
    let caret = &g.app.session.sel.focus.path;
    secs.iter().rposition(|s| s.path <= *caret)
}

/// Which sections the Corkboard shows: chapters (the top level) or every section.
fn board_sections(g: &GlassApp, secs: &[Section]) -> Vec<usize> {
    let base = secs.iter().map(|s| s.level).min().unwrap_or(0);
    secs.iter().enumerate().filter(|(_, s)| g.glass.board_all || s.level == base).map(|(i, _)| i).collect()
}

/// Move a section (with everything under it) `by` places among its siblings.
fn move_section(g: &mut GlassApp, sec: &Section, by: i64) {
    let pos = Pos { story: StoryRef::Body, path: sec.path.clone(), off: 0 };
    if g.app.run("caret.set", json!({ "pos": pos })).is_err() {
        return;
    }
    let id = if by < 0 { "outline.moveUp" } else { "outline.moveDown" };
    for _ in 0..by.unsigned_abs() {
        if let Err(e) = g.app.run(id, json!({})) {
            g.app.status(format!("Couldn't move “{}” further: {e}", sec.title));
            break;
        }
    }
}

/// The Corkboard: each chapter (or scene) as an index card. Drag cards to reorder the manuscript;
/// click a card's status to move it along; double-click to open it.
pub fn corkboard(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let secs = sections(&g.app.session);
    let shown = board_sections(g, &secs);
    let pkey = key(&g.app.session);
    view_header(g, ui, t, "Corkboard", &format!("{} cards", shown.len()), true);
    if shown.is_empty() {
        empty(ui, t, "No chapters yet", "Give your chapters and scenes Heading styles and they appear here as cards.");
        return;
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let card = vec2(236.0, 170.0);
        let gap = 18.0;
        let left = ui.max_rect().min.x + 28.0;
        let cols = (((ui.available_width() - 56.0 + gap) / (card.x + gap)).floor() as usize).max(1);
        let rows = shown.len().div_ceil(cols);
        let top = ui.cursor().min.y + 8.0;
        ui.allocate_exact_size(vec2(ui.available_width(), rows as f32 * (card.y + gap) + 40.0), Sense::hover());
        let slot = |k: usize| Rect::from_min_size(pos2(left + (k % cols) as f32 * (card.x + gap), top + (k / cols) as f32 * (card.y + gap)), card);
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let mut dragging: Option<(usize, egui::Vec2)> = None;
        let mut released: Option<(usize, usize)> = None;
        let mut open: Option<usize> = None;
        let mut cycle: Option<String> = None;
        for (k, &i) in shown.iter().enumerate() {
            let Some(sec) = secs.get(i) else { continue };
            let r = slot(k);
            let resp = ui.interact(r, ui.id().with(("card", &sec.title, k)), Sense::click_and_drag());
            let meta = g.glass.projects.by_doc.get(&pkey).map(|p| p.card(&sec.title)).unwrap_or_default();
            if resp.dragged() {
                dragging = Some((k, resp.drag_delta()));
                continue;
            }
            if resp.drag_stopped()
                && let Some(p) = pointer
            {
                let to = (0..shown.len()).min_by(|a, b| slot(*a).center().distance(p).total_cmp(&slot(*b).center().distance(p))).unwrap_or(k);
                released = Some((k, to));
            }
            if resp.double_clicked() {
                open = Some(i);
            }
            let chip = draw_card(ui, t, r, sec, &meta, resp.hovered(), 0.0);
            let cresp = ui.interact(chip, ui.id().with(("chip", k)), Sense::click()).on_hover_text("Change status");
            if cresp.clicked() {
                cycle = Some(sec.title.clone());
            }
        }
        // The card being dragged follows the pointer, lifted above the others.
        if let Some((k, _)) = dragging
            && let (Some(&i), Some(p)) = (shown.get(k), pointer)
            && let Some(sec) = secs.get(i)
        {
            let to = (0..shown.len()).min_by(|a, b| slot(*a).center().distance(p).total_cmp(&slot(*b).center().distance(p))).unwrap_or(k);
            let target = slot(to);
            ui.painter().rect_stroke(target.expand(4.0), 16.0, egui::Stroke::new(2.0, t.accent.gamma_multiply(0.6)), egui::StrokeKind::Outside);
            let meta = g.glass.projects.by_doc.get(&pkey).map(|p| p.card(&sec.title)).unwrap_or_default();
            let r = Rect::from_center_size(p, card);
            let layer = ui.painter().clone().with_layer_id(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("card_drag")));
            draw_card_on(&layer, t, r, sec, &meta, true, 1.0);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        if let Some(title) = cycle {
            let c = g.glass.projects.by_doc.entry(pkey.clone()).or_default().card_mut(&title);
            c.status = c.status.next();
        }
        if let Some(sec) = open.and_then(|i| secs.get(i)) {
            go_to(g, sec);
        }
        if let Some((from, to)) = released
            && from != to
            && let Some(sec) = shown.get(from).and_then(|i| secs.get(*i))
        {
            move_section(g, sec, to as i64 - from as i64);
        }
    });
}

fn draw_card(ui: &Ui, t: &Tokens, r: Rect, sec: &Section, meta: &crate::project::Card, hovered: bool, lift: f32) -> Rect {
    let lift = lift.max(ui.ctx().animate_bool_with_time(egui::Id::new(("card_lift", &sec.title)), hovered, 0.15));
    draw_card_on(ui.painter(), t, r.translate(vec2(0.0, -2.0 * lift)), sec, meta, false, lift)
}

/// An index card: status chip, title, synopsis, word count. Returns the chip's rect.
fn draw_card_on(p: &egui::Painter, t: &Tokens, r: Rect, sec: &Section, meta: &crate::project::Card, dragged: bool, lift: f32) -> Rect {
    let shadow = egui::epaint::Shadow { offset: [0, (3.0 + 7.0 * lift) as i8], blur: (12.0 + 16.0 * lift) as u8, spread: 0, color: t.shadow };
    p.add(shadow.as_shape(r, egui::CornerRadius::same(14)));
    p.rect_filled(r, 14.0, t.paper);
    if dragged {
        p.rect_stroke(r, 14.0, egui::Stroke::new(1.5, t.accent), egui::StrokeKind::Outside);
    } else {
        p.rect_stroke(r, 14.0, egui::Stroke::new(0.75, t.edge), egui::StrokeKind::Outside);
    }
    // Index-card rule under the title.
    let x0 = r.min.x + 16.0;
    let color = status_color(meta.status, t);
    let chip_w = p.ctx().fonts_mut(|f| f.layout_no_wrap(meta.status.name().to_string(), medium(10.5), t.text).size().x) + 22.0;
    let chip = Rect::from_min_size(pos2(x0, r.min.y + 14.0), vec2(chip_w, 20.0));
    p.rect_filled(chip, 10.0, color.gamma_multiply(0.16));
    p.circle_filled(pos2(chip.min.x + 9.0, chip.center().y), 3.0, color);
    p.text(pos2(chip.min.x + 16.0, chip.center().y), Align2::LEFT_CENTER, meta.status.name(), medium(10.5), t.text);
    let title: String =
        if sec.title.chars().count() > 30 { format!("{}…", sec.title.chars().take(30).collect::<String>()) } else { sec.title.clone() };
    p.text(pos2(x0, r.min.y + 52.0), Align2::LEFT_CENTER, title, semibold(14.0), t.text);
    p.line_segment(
        [pos2(x0, r.min.y + 66.0), pos2(r.max.x - 16.0, r.min.y + 66.0)],
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 80, 80, 70)),
    );
    let synopsis = if meta.synopsis.trim().is_empty() { sec.opening.clone() } else { meta.synopsis.clone() };
    let mut job =
        egui::text::LayoutJob::single_section(synopsis, egui::TextFormat { font_id: regular(12.0), color: t.text_dim, ..Default::default() });
    job.wrap.max_width = r.width() - 32.0;
    job.wrap.max_rows = 4;
    let galley = p.ctx().fonts_mut(|f| f.layout_job(job));
    p.galley(pos2(x0, r.min.y + 74.0), galley, t.text_dim);
    p.text(
        pos2(r.max.x - 16.0, r.max.y - 14.0),
        Align2::RIGHT_CENTER,
        format!("{} words", crate::chrome::group_digits(sec.words as i64)),
        regular(11.0),
        t.text_dim,
    );
    chip
}

fn view_header(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, title: &str, sub: &str, board: bool) {
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(28.0);
        ui.label(egui::RichText::new(title).font(semibold(22.0)).color(t.text));
        ui.add_space(8.0);
        ui.label(egui::RichText::new(sub).font(regular(13.0)).color(t.text_dim));
        if board {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(28.0);
                let sel = usize::from(g.glass.board_all);
                if let Some(i) = w::segmented(ui, t, &["Chapters", "All Sections"], sel, 30.0) {
                    g.glass.board_all = i == 1;
                }
            });
        }
    });
    ui.add_space(10.0);
}

fn empty(ui: &mut Ui, t: &Tokens, title: &str, text: &str) {
    ui.add_space(80.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(title).font(semibold(17.0)).color(t.text));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(text).font(regular(13.0)).color(t.text_dim));
    });
}

/// The Outliner: every section with its status, word count and synopsis, editable in place.
pub fn outliner(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let secs = sections(&g.app.session);
    let total: usize = secs.iter().map(|s| s.words).sum();
    view_header(g, ui, t, "Outliner", &format!("{} sections · {} words", secs.len(), crate::chrome::group_digits(total as i64)), false);
    if secs.is_empty() {
        empty(ui, t, "Nothing to outline yet", "Headings become rows here, with a status, a word count and a synopsis.");
        return;
    }
    let pkey = key(&g.app.session);
    let base = secs.iter().map(|s| s.level).min().unwrap_or(0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(28.0);
            let width = (ui.available_width() - 28.0).max(400.0);
            let bg = ui.painter().add(egui::Shape::Noop);
            let resp = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.set_width(width - 28.0);
                ui.vertical(|ui| {
                    let cols = [0.34, 0.14, 0.1];
                    let w0 = (width - 28.0) * cols[0];
                    let w1 = (width - 28.0) * cols[1];
                    let w2 = (width - 28.0) * cols[2];
                    ui.horizontal(|ui| {
                        for (label, wd) in [("TITLE", w0), ("STATUS", w1), ("WORDS", w2), ("SYNOPSIS", 0.0)] {
                            let (r, _) = ui.allocate_exact_size(vec2(if wd > 0.0 { wd } else { ui.available_width() }, 26.0), Sense::hover());
                            ui.painter().text(pos2(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, label, semibold(11.0), t.text_dim);
                        }
                    });
                    let mut go: Option<usize> = None;
                    for (i, s) in secs.iter().enumerate() {
                        let card = g.glass.projects.by_doc.get(&pkey).map(|p| p.card(&s.title)).unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                            let indent = f32::from(s.level.saturating_sub(base).min(5)) * 16.0;
                            let (r, resp) = ui.allocate_exact_size(vec2(w0, 36.0), Sense::click());
                            paint_state(ui, r, 8, &resp, false, t);
                            let font = if s.level == base { semibold(13.0) } else { regular(13.0) };
                            let max = ((w0 - indent - 16.0) / 7.0).max(4.0) as usize;
                            let title: String = if s.title.chars().count() > max {
                                format!("{}…", s.title.chars().take(max).collect::<String>())
                            } else {
                                s.title.clone()
                            };
                            ui.painter().text(pos2(r.min.x + 8.0 + indent, r.center().y), Align2::LEFT_CENTER, title, font, t.text);
                            if resp.on_hover_text("Open").clicked() {
                                go = Some(i);
                            }
                            let (r, resp) = ui.allocate_exact_size(vec2(w1, 36.0), Sense::click());
                            let c = status_color(card.status, t);
                            let chip = Rect::from_min_size(pos2(r.min.x + 6.0, r.center().y - 10.0), vec2(w1 - 16.0, 20.0));
                            ui.painter().rect_filled(chip, 10.0, c.gamma_multiply(if resp.hovered() { 0.26 } else { 0.16 }));
                            ui.painter().circle_filled(pos2(chip.min.x + 10.0, chip.center().y), 3.0, c);
                            ui.painter().text(
                                pos2(chip.min.x + 18.0, chip.center().y),
                                Align2::LEFT_CENTER,
                                card.status.name(),
                                medium(11.0),
                                t.text,
                            );
                            if resp.on_hover_text("Change status").clicked() {
                                let m = g.glass.projects.by_doc.entry(pkey.clone()).or_default().card_mut(&s.title);
                                m.status = m.status.next();
                            }
                            let (r, _) = ui.allocate_exact_size(vec2(w2, 36.0), Sense::hover());
                            ui.painter().text(
                                pos2(r.max.x - 14.0, r.center().y),
                                Align2::RIGHT_CENTER,
                                crate::chrome::group_digits(s.words as i64),
                                regular(12.5),
                                t.text_dim,
                            );
                            ui.add_space(10.0);
                            let mut syn = card.synopsis.clone();
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut syn)
                                    .hint_text(egui::RichText::new(&s.opening).color(t.text_disabled))
                                    .desired_width(ui.available_width() - 8.0)
                                    .frame(egui::Frame::NONE)
                                    .font(regular(12.5)),
                            );
                            if edit.changed() {
                                g.glass.projects.by_doc.entry(pkey.clone()).or_default().card_mut(&s.title).synopsis = syn;
                            }
                        });
                        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                        ui.painter().rect_filled(r, 0.0, t.edge.gamma_multiply(0.35));
                    }
                    if let Some(sec) = go.and_then(|i| secs.get(i)) {
                        go_to(g, sec);
                    }
                });
            });
            ui.painter().set(bg, glass_shape(resp.response.rect, radius::PANEL, t));
        });
    });
}

/// The right-hand panel for Pro tools: the Scratchpad, Pacing, or a note.
pub fn side(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let kind = g.glass.panel.filter(|p| !p.is_versions()).or(g.glass.last_side);
    match kind {
        Some(Panel::Pacing) => pacing(g, ui, t),
        Some(Panel::Note(id)) => note(g, ui, t, id),
        Some(Panel::Inspector) => crate::scriv::inspector(g, ui, t),
        Some(Panel::Reference(kind)) => crate::scriv::reference(g, ui, t, kind),
        _ => scratchpad(g, ui, t),
    }
}

fn panel_title(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, title: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).font(semibold(15.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if w::icon_capsule(ui, t, "close", "Close", 24.0, false, true).clicked() {
                g.glass.panel = None;
            }
        });
    });
    ui.add_space(8.0);
}

fn hint(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).font(regular(11.5)).color(t.text_dim)).wrap());
}

/// Cut the selection to the Scratchpad: out of the manuscript, but kept.
pub fn cut_to_scratchpad(g: &mut GlassApp) {
    let text = g.app.session.selected_text();
    if text.trim().is_empty() {
        g.app.status("Select the text you want to set aside first");
        return;
    }
    let secs = sections(&g.app.session);
    let from = current_section(g, &secs).and_then(|i| secs.get(i)).map(|s| s.title.clone()).unwrap_or_default();
    let date = wordcraft_engine::cmd::now_iso();
    let pkey = key(&g.app.session);
    g.glass.projects.by_doc.entry(pkey).or_default().add_scrap(&text, &from, &date);
    if let Err(e) = g.app.run("text.delete", json!({})) {
        g.app.status(e);
        return;
    }
    g.app.status("Moved to the Scratchpad");
}

fn scratchpad(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    panel_title(g, ui, t, "Scratchpad");
    let width = ui.available_width();
    let has_sel = !g.app.session.sel.is_collapsed();
    if w::accent_button(ui, t, Some("cut"), "Cut Selection to Scratchpad", Some(width), 34.0).clicked() {
        if has_sel {
            cut_to_scratchpad(g);
        } else {
            g.app.status("Select the text you want to set aside first");
        }
    }
    ui.add_space(6.0);
    hint(ui, t, "Trimmed scenes and cut lines wait here instead of being deleted, so you can bring them back any time.");
    ui.add_space(8.0);
    let pkey = key(&g.app.session);
    let scraps = g.glass.projects.by_doc.get(&pkey).map(|p| p.scraps.clone()).unwrap_or_default();
    if scraps.is_empty() {
        hint(ui, t, "Nothing set aside yet.");
    }
    let mut insert: Option<String> = None;
    let mut remove: Option<usize> = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 8.0);
        for (i, s) in scraps.iter().enumerate() {
            let bg = ui.painter().add(egui::Shape::Noop);
            let resp = egui::Frame::NONE.inner_margin(12).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let preview: String = s.text.chars().take(320).collect();
                let mut job =
                    egui::text::LayoutJob::single_section(preview, egui::TextFormat { font_id: regular(12.5), color: t.text, ..Default::default() });
                job.wrap.max_width = ui.available_width();
                job.wrap.max_rows = 5;
                ui.label(job);
                let when = s.date.get(..10).unwrap_or("");
                let from = if s.from.is_empty() { String::new() } else { format!("From “{}” · ", s.from) };
                ui.label(egui::RichText::new(format!("{from}{when}")).font(regular(11.0)).color(t.text_dim));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                    if w::quiet_button(ui, t, "Insert at Cursor", None, 28.0).clicked() {
                        insert = Some(s.text.clone());
                    }
                    if w::quiet_button(ui, t, "Copy", None, 28.0).clicked() {
                        ui.ctx().copy_text(s.text.clone());
                    }
                    if w::quiet_button(ui, t, "Delete", None, 28.0).clicked() {
                        remove = Some(i);
                    }
                });
            });
            ui.painter().set(bg, egui::Shape::rect_filled(resp.response.rect, egui::CornerRadius::same(14), t.fill));
        }
    });
    if let Some(text) = insert
        && let Err(e) = g.app.run("text.insert", json!({ "text": text }))
    {
        g.app.status(e);
    }
    if let Some(i) = remove
        && let Some(p) = g.glass.projects.by_doc.get_mut(&pkey)
        && i < p.scraps.len()
    {
        p.scraps.remove(i);
    }
}

/// Analysis, recomputed only when the document changes.
fn metrics(g: &mut GlassApp) -> Vec<(Section, Metrics)> {
    let rev = (g.app.session.document_id(), g.app.session.rev());
    if g.glass.pacing_cache.0 != rev {
        let secs = sections(&g.app.session);
        let rows: Vec<(Section, Metrics)> = secs
            .into_iter()
            .map(|s| {
                let m = analyze(&s.text);
                (s, m)
            })
            .collect();
        g.glass.pacing_cache = (rev, rows);
    }
    g.glass.pacing_cache.1.clone()
}

fn pacing(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    panel_title(g, ui, t, "Pacing");
    hint(ui, t, "Each row is a section; stronger colour means more. Click a row to go there.");
    ui.add_space(10.0);
    let rows = metrics(g);
    if rows.is_empty() {
        hint(ui, t, "Add headings to your chapters to compare them.");
        return;
    }
    let cols: [(&str, &str, fn(&Metrics) -> f32); 4] = [
        ("Len", "Average sentence length (words)", |m| m.avg_sentence),
        ("Var", "Sentence variety: how much sentence length changes", |m| m.variety),
        ("Filt", "Filter words per 1,000 words (noticed, felt, sentiu…)", |m| m.filter_per_k),
        ("Dlg", "Dialogue share of the text", |m| m.dialogue * 100.0),
    ];
    let max: Vec<f32> = cols.iter().map(|(_, _, f)| rows.iter().map(|(_, m)| f(m)).fold(0.0_f32, f32::max).max(0.001)).collect();
    let name_w = ui.available_width() - 4.0 * 44.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.allocate_exact_size(vec2(name_w, 20.0), Sense::hover());
        for (label, tip, _) in &cols {
            let (r, resp) = ui.allocate_exact_size(vec2(44.0, 20.0), Sense::hover());
            ui.painter().text(r.center(), Align2::CENTER_CENTER, *label, semibold(10.5), t.text_dim);
            resp.on_hover_text(*tip);
        }
    });
    let warm = egui::Color32::from_rgb(0xE0, 0x7B, 0x1F);
    let mut go: Option<Section> = None;
    egui::ScrollArea::vertical().max_height(ui.available_height() * 0.62).show(ui, |ui| {
        for (s, m) in &rows {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let (r, resp) = ui.allocate_exact_size(vec2(name_w, 30.0), Sense::click());
                paint_state(ui, r, 8, &resp, false, t);
                let max_chars = ((name_w - 16.0) / 6.6).max(4.0) as usize;
                let title: String = if s.title.chars().count() > max_chars {
                    format!("{}…", s.title.chars().take(max_chars).collect::<String>())
                } else {
                    s.title.clone()
                };
                ui.painter().text(pos2(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, title, regular(12.0), t.text);
                if resp.clicked() {
                    go = Some(s.clone());
                }
                for (k, (_, _, f)) in cols.iter().enumerate() {
                    let v = f(m);
                    let a = (v / max.get(k).copied().unwrap_or(1.0)).clamp(0.0, 1.0);
                    let (r, resp) = ui.allocate_exact_size(vec2(44.0, 30.0), Sense::hover());
                    let cell = r.shrink2(vec2(3.0, 4.0));
                    ui.painter().rect_filled(cell, 6.0, warm.gamma_multiply(0.12 + 0.75 * a));
                    ui.painter().text(
                        cell.center(),
                        Align2::CENTER_CENTER,
                        format!("{v:.0}"),
                        medium(11.0),
                        if a > 0.6 { egui::Color32::WHITE } else { t.text },
                    );
                    resp.on_hover_text(format!("{v:.1}"));
                }
            });
        }
    });
    if let Some(s) = go {
        go_to(g, &s);
    }
    ui.add_space(12.0);
    ui.label(egui::RichText::new("Filter words in this document").font(semibold(12.0)).color(t.text));
    ui.add_space(4.0);
    let mut all: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for (_, m) in &rows {
        for (w, n) in &m.filters {
            *all.entry(w.clone()).or_default() += n;
        }
    }
    let mut all: Vec<(String, usize)> = all.into_iter().collect();
    all.sort_by_key(|a| std::cmp::Reverse(a.1));
    if all.is_empty() {
        hint(ui, t, "None found. Nice and direct.");
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (word, n) in all.iter().take(14) {
            let text = format!("{word}  {n}");
            let tw = ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.clone(), regular(11.5), t.text).size().x);
            let (r, _) = ui.allocate_exact_size(vec2(tw + 18.0, 24.0), Sense::hover());
            ui.painter().rect_filled(r, 12.0, t.fill);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, text, regular(11.5), t.text);
        }
    });
}

fn note(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, id: u64) {
    let pkey = key(&g.app.session);
    let Some(n) = g.glass.projects.by_doc.get(&pkey).and_then(|p| p.notes.iter().find(|n| n.id == id)).cloned() else {
        panel_title(g, ui, t, "Note");
        hint(ui, t, "This note belongs to another document.");
        return;
    };
    panel_title(g, ui, t, n.kind.name());
    let mut title = n.title.clone();
    let mut body = n.body.clone();
    let tr = ui.add(egui::TextEdit::singleline(&mut title).font(semibold(17.0)).frame(egui::Frame::NONE).desired_width(ui.available_width()));
    ui.add_space(6.0);
    let hint_text = match n.kind {
        NoteKind::Character => "Age, look, voice, what they want, what they fear, first appearance…",
        NoteKind::Place => "What it looks like, sounds like, smells like; who goes there…",
        NoteKind::Research => "Notes, quotes, links and sources…",
    };
    let br =
        ui.add(egui::TextEdit::multiline(&mut body).hint_text(hint_text).desired_width(ui.available_width()).desired_rows(18).font(regular(13.0)));
    if (tr.changed() || br.changed())
        && let Some(m) = g.glass.projects.by_doc.get_mut(&pkey).and_then(|p| p.note_mut(id))
    {
        m.title = title;
        m.body = body;
    }
    ui.add_space(10.0);
    if w::quiet_button(ui, t, "Delete Note", None, 30.0).clicked() {
        if let Some(p) = g.glass.projects.by_doc.get_mut(&pkey) {
            p.remove_note(id);
        }
        g.glass.panel = None;
    }
}

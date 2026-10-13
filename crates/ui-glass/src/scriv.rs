//! Writer Pro, laid out the way Scrivener lays out a manuscript:
//!
//! - a **Binder** tree with folder and document icons and disclosure triangles,
//! - an **editor header** over the page (back and forward through the sections you visited, the
//!   section's name and status, previous and next section, the split pane and the Inspector),
//! - the **Inspector** on the right: the section's synopsis, its notes on a cream card, and its
//!   label and status,
//! - a **reference pane** (the split), listing Characters, Places, Research or the manuscript's
//!   sections with their synopses, with the selected one open beneath,
//! - **Quick Reference**: a section in a small floating window, to read while writing another,
//! - **sections on the page** shown the Scrivenings way: each heading in a soft box tinted with
//!   its label, a dashed rule between sections.

use std::collections::BTreeSet;

use egui::{Align2, Color32, Painter, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_engine::doc::{Path, Pos, StoryRef};
use wordcraft_engine::layout::DocLayout;
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::pro::{go_to, status_color};
use crate::project::{Label, NoteKind, Section, Status, key, sections};
use crate::tokens::{Tokens, paint_state, radius};
use crate::widgets as w;
use crate::{GlassApp, Panel, View};

/// Writer Pro's Scrivener-style state.
#[derive(Default)]
pub struct Scriv {
    /// Sections visited (heading paths), and where we are in that list.
    history: Vec<Path>,
    pos: usize,
    /// Binder rows folded shut (by heading text).
    pub folded: BTreeSet<String>,
    /// The note selected in the reference pane.
    pub ref_note: Option<u64>,
    /// The section shown in Quick Reference (its heading path).
    pub quick: Option<Path>,
}

// ---------------------------------------------------------------------------------------------
// Icons, drawn in code.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Manuscript,
    Folder,
    Doc,
    Character,
    Place,
    Research,
    Scratch,
}

pub fn icon(p: &Painter, r: Rect, kind: Icon, t: &Tokens) {
    let r = Rect::from_center_size(r.center(), vec2(14.0, 14.0));
    let line = Stroke::new(1.0, t.text_dim);
    match kind {
        Icon::Folder => {
            let blue = Color32::from_rgb(0x6C, 0xB4, 0xEE);
            let edge = Stroke::new(1.0, Color32::from_rgb(0x3A, 0x86, 0xC8));
            let tab = Rect::from_min_size(pos2(r.min.x, r.min.y + 1.5), vec2(6.0, 3.0));
            p.rect_filled(tab, 1.0, blue);
            let body = Rect::from_min_max(pos2(r.min.x, r.min.y + 3.5), pos2(r.max.x, r.max.y - 1.0));
            p.rect_filled(body, 2.0, blue);
            p.rect_stroke(body, 2.0, edge, egui::StrokeKind::Inside);
        }
        Icon::Doc => {
            let page = Rect::from_min_max(pos2(r.min.x + 2.0, r.min.y), pos2(r.max.x - 2.0, r.max.y));
            p.rect_filled(page, 1.5, Color32::WHITE);
            p.rect_stroke(page, 1.5, Stroke::new(1.0, Color32::from_gray(150)), egui::StrokeKind::Inside);
            for k in 0..3 {
                let y = page.min.y + 4.0 + k as f32 * 3.0;
                p.line_segment([pos2(page.min.x + 2.5, y), pos2(page.max.x - 2.5, y)], Stroke::new(0.8, Color32::from_gray(185)));
            }
        }
        Icon::Manuscript => {
            for k in [2.0f32, 0.0] {
                let page = Rect::from_min_max(pos2(r.min.x + 1.0 + k, r.min.y + k), pos2(r.max.x - 3.0 + k, r.max.y - 2.0 + k));
                p.rect_filled(page, 1.5, Color32::WHITE);
                p.rect_stroke(page, 1.5, Stroke::new(1.0, Color32::from_rgb(0x3A, 0x86, 0xC8)), egui::StrokeKind::Inside);
            }
        }
        Icon::Character => {
            let card = r.shrink(0.5);
            p.rect_filled(card, 2.5, Color32::from_rgb(0xF2, 0xC2, 0x7B));
            p.circle_filled(pos2(card.center().x, card.min.y + 5.0), 2.6, Color32::from_rgb(0x8A, 0x55, 0x1E));
            let body = Rect::from_min_max(pos2(card.center().x - 4.0, card.min.y + 8.5), pos2(card.center().x + 4.0, card.max.y - 1.0));
            p.rect_filled(body, 3.0, Color32::from_rgb(0x8A, 0x55, 0x1E));
        }
        Icon::Place => {
            let card = r.shrink(0.5);
            p.rect_filled(card, 2.5, Color32::from_rgb(0xBF, 0xE3, 0xA8));
            let a = pos2(card.min.x + 1.5, card.max.y - 2.0);
            p.add(egui::Shape::convex_polygon(
                vec![a, pos2(card.min.x + 5.5, card.min.y + 5.0), pos2(card.min.x + 9.5, card.max.y - 2.0)],
                Color32::from_rgb(0x3E, 0x8E, 0x3E),
                Stroke::NONE,
            ));
            p.add(egui::Shape::convex_polygon(
                vec![pos2(card.min.x + 6.0, card.max.y - 2.0), pos2(card.min.x + 9.5, card.min.y + 7.0), pos2(card.max.x - 1.0, card.max.y - 2.0)],
                Color32::from_rgb(0x2E, 0x6E, 0x2E),
                Stroke::NONE,
            ));
        }
        Icon::Research => {
            let book = r.shrink(1.0);
            p.rect_filled(book, 1.5, Color32::from_rgb(0xD9, 0x8E, 0x5B));
            p.rect_filled(Rect::from_min_size(book.min, vec2(2.5, book.height())), 1.0, Color32::from_rgb(0x9A, 0x55, 0x2A));
            p.line_segment([pos2(book.min.x + 5.0, book.min.y + 4.0), pos2(book.max.x - 2.0, book.min.y + 4.0)], Stroke::new(1.0, Color32::WHITE));
        }
        Icon::Scratch => {
            let can = Rect::from_min_max(pos2(r.min.x + 2.5, r.min.y + 3.5), pos2(r.max.x - 2.5, r.max.y));
            p.rect_stroke(can, 1.5, line, egui::StrokeKind::Inside);
            p.line_segment([pos2(r.min.x + 1.0, r.min.y + 2.5), pos2(r.max.x - 1.0, r.min.y + 2.5)], line);
            p.line_segment([pos2(r.center().x - 2.0, r.min.y + 0.5), pos2(r.center().x + 2.0, r.min.y + 0.5)], line);
        }
    }
}

fn disclosure(p: &Painter, c: egui::Pos2, open: bool, color: Color32) {
    let pts = if open {
        vec![pos2(c.x - 3.5, c.y - 2.0), pos2(c.x + 3.5, c.y - 2.0), pos2(c.x, c.y + 2.5)]
    } else {
        vec![pos2(c.x - 2.0, c.y - 3.5), pos2(c.x + 2.5, c.y), pos2(c.x - 2.0, c.y + 3.5)]
    };
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

// ---------------------------------------------------------------------------------------------
// Sections and history.

/// The section the caret is in.
pub fn current(g: &GlassApp, secs: &[Section]) -> Option<usize> {
    let caret = &g.app.session.sel.focus.path;
    secs.iter().rposition(|s| s.path <= *caret)
}

/// Keep the list of visited sections up to date (called once a frame).
pub fn track(g: &mut GlassApp) {
    let secs = sections(&g.app.session);
    let Some(path) = current(g, &secs).and_then(|i| secs.get(i)).map(|s| s.path.clone()) else { return };
    let h = &mut g.glass.scriv;
    if h.history.get(h.pos) == Some(&path) {
        return;
    }
    if !h.history.is_empty() {
        h.history.truncate(h.pos + 1);
    }
    h.history.push(path);
    if h.history.len() > 60 {
        h.history.remove(0);
    }
    h.pos = h.history.len() - 1;
}

fn go_path(g: &mut GlassApp, path: &Path) {
    let pos = Pos { story: StoryRef::Body, path: path.clone(), off: 0 };
    if let Err(e) = g.app.run("caret.set", json!({ "pos": pos })) {
        g.app.status(e);
    }
    g.app.canvas.scroll_to_caret = true;
    g.app.canvas.want_focus = true;
}

fn small_button(ui: &mut Ui, t: &Tokens, icon: &str, tip: &str, enabled: bool) -> bool {
    w::icon_capsule(ui, t, icon, tip, 24.0, false, enabled).clicked() && enabled
}

// ---------------------------------------------------------------------------------------------
// The editor header.

/// The bar over the page: ‹ › history, the section's icon, name and status, ⌃ ⌄ sections, and
/// the split and Inspector switches.
pub fn editor_bar(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let secs = sections(&g.app.session);
    let cur = current(g, &secs);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    let bar = r.shrink2(vec2(16.0, 3.0));
    ui.painter().add(crate::tokens::glass_shape(bar, radius::CAPSULE, t));
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(vec2(8.0, 2.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let ui = &mut child;
    ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
    let (pos, len) = (g.glass.scriv.pos, g.glass.scriv.history.len());
    if small_button(ui, t, "chevronLeft", "Back", pos > 0) {
        g.glass.scriv.pos -= 1;
        if let Some(p) = g.glass.scriv.history.get(g.glass.scriv.pos).cloned() {
            go_path(g, &p);
        }
    }
    if small_button(ui, t, "chevronRight", "Forward", pos + 1 < len) {
        g.glass.scriv.pos += 1;
        if let Some(p) = g.glass.scriv.history.get(g.glass.scriv.pos).cloned() {
            go_path(g, &p);
        }
    }
    ui.add_space(8.0);
    let (ir, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    icon(ui.painter(), ir, Icon::Doc, t);
    ui.add_space(4.0);
    let title = cur.and_then(|i| secs.get(i)).map(|s| s.title.clone()).unwrap_or_else(|| g.tab_title(g.docs.active));
    ui.label(egui::RichText::new(&title).font(medium(12.5)).color(t.text));
    if let Some(s) = cur.and_then(|i| secs.get(i)) {
        let card = g.glass.projects.by_doc.get(&key(&g.app.session)).map(|p| p.card(&s.title)).unwrap_or_default();
        ui.add_space(6.0);
        let c = status_color(card.status, t);
        let text = card.status.name();
        let tw = ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.to_string(), medium(10.5), t.text).size().x);
        let (chip, _) = ui.allocate_exact_size(vec2(tw + 20.0, 18.0), Sense::hover());
        ui.painter().rect_filled(chip, 9.0, c.gamma_multiply(0.16));
        ui.painter().circle_filled(pos2(chip.min.x + 8.0, chip.center().y), 2.8, c);
        ui.painter().text(pos2(chip.min.x + 14.0, chip.center().y), Align2::LEFT_CENTER, text, medium(10.5), t.text_dim);
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
        let inspector = g.glass.panel == Some(Panel::Inspector);
        if w::icon_capsule(ui, t, "help", "Inspector: synopsis, notes, label and status", 24.0, inspector, true).clicked() {
            g.glass.panel = if inspector { None } else { Some(Panel::Inspector) };
        }
        let split = matches!(g.glass.panel, Some(Panel::Reference(_)));
        if w::icon_capsule(ui, t, "split", "Split: Characters, Places, Research or the manuscript beside the page", 24.0, split, true).clicked() {
            g.glass.panel = if split { None } else { Some(Panel::Reference(RefView::Document(None))) };
        }
        ui.add_space(6.0);
        let next = cur.map_or(0, |i| i + 1);
        if small_button(ui, t, "chevronDown", "Next section", next < secs.len())
            && let Some(s) = secs.get(next)
        {
            go_to(g, s);
        }
        let prev = cur.and_then(|i| i.checked_sub(1));
        if small_button(ui, t, "chevronUp", "Previous section", prev.is_some())
            && let Some(s) = prev.and_then(|i| secs.get(i))
        {
            go_to(g, s);
        }
    });
}

// ---------------------------------------------------------------------------------------------
// The Inspector.

fn heading_row(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add_space(6.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    disclosure(ui.painter(), pos2(r.min.x + 6.0, r.center().y), true, t.text_dim);
    ui.painter().text(pos2(r.min.x + 16.0, r.center().y), Align2::LEFT_CENTER, text, semibold(11.5), t.text_dim);
    ui.add_space(2.0);
}

fn menu_pill(ui: &mut Ui, t: &Tokens, text: &str, dot: Option<Color32>, width: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click());
    paint_state(ui, r, 8, &resp, false, t);
    let mut x = r.min.x + 8.0;
    if let Some(c) = dot {
        ui.painter().circle_filled(pos2(x + 4.0, r.center().y), 4.0, c);
        x += 14.0;
    }
    ui.painter().text(pos2(x, r.center().y), Align2::LEFT_CENTER, text, regular(12.0), t.text);
    ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, "⌃⌄", regular(9.0), t.text_dim);
    resp
}

/// The Inspector: the current section's synopsis and notes, its label and status.
pub fn inspector(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Inspector").font(semibold(15.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if w::icon_capsule(ui, t, "close", "Close", 24.0, false, true).clicked() {
                g.glass.panel = None;
            }
        });
    });
    let secs = sections(&g.app.session);
    let Some(sec) = current(g, &secs).and_then(|i| secs.get(i)).cloned() else {
        ui.add_space(8.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new("Put the caret in a section — text under a heading — to see its synopsis, notes, label and status.")
                    .font(regular(12.0))
                    .color(t.text_dim),
            )
            .wrap(),
        );
        return;
    };
    let pkey = key(&g.app.session);
    let card = g.glass.projects.by_doc.get(&pkey).map(|p| p.card(&sec.title)).unwrap_or_default();
    let width = ui.available_width();
    // Synopsis.
    heading_row(ui, t, "Synopsis");
    let bg = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Frame::NONE.inner_margin(10).show(ui, |ui| {
        ui.set_width(width - 20.0);
        ui.label(egui::RichText::new(&sec.title).font(semibold(13.5)).color(egui::Color32::from_gray(30)));
        ui.add_space(2.0);
        let mut syn = card.synopsis.clone();
        let edit = ui.add(
            egui::TextEdit::multiline(&mut syn)
                .hint_text(
                    egui::RichText::new(if sec.opening.is_empty() { "What happens here, in a line or two…" } else { &sec.opening })
                        .color(egui::Color32::from_gray(150)),
                )
                .frame(egui::Frame::NONE)
                .text_color(egui::Color32::from_gray(45))
                .font(regular(12.5))
                .desired_rows(5)
                .desired_width(width - 20.0),
        );
        if edit.changed() {
            g.glass.projects.by_doc.entry(pkey.clone()).or_default().card_mut(&sec.title).synopsis = syn;
        }
    });
    ui.painter().set(bg, egui::Shape::rect_filled(inner.response.rect, 8.0, egui::Color32::WHITE));
    // Notes, on cream paper.
    heading_row(ui, t, "Notes");
    let cream = if t.dark { egui::Color32::from_rgb(0x3A, 0x36, 0x28) } else { egui::Color32::from_rgb(0xFB, 0xF5, 0xE1) };
    let ink = if t.dark { egui::Color32::from_rgb(0xEC, 0xE6, 0xD2) } else { egui::Color32::from_rgb(0x3A, 0x33, 0x22) };
    let bg = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Frame::NONE.inner_margin(10).show(ui, |ui| {
        ui.set_width(width - 20.0);
        let mut notes = card.notes.clone();
        let edit = ui.add(
            egui::TextEdit::multiline(&mut notes)
                .hint_text(egui::RichText::new("Notes for this section: what to fix, what to remember…").color(ink.gamma_multiply(0.5)))
                .frame(egui::Frame::NONE)
                .text_color(ink)
                .font(regular(12.5))
                .desired_rows(12)
                .desired_width(width - 20.0),
        );
        if edit.changed() {
            g.glass.projects.by_doc.entry(pkey.clone()).or_default().card_mut(&sec.title).notes = notes;
        }
    });
    ui.painter().set(bg, egui::Shape::rect_filled(inner.response.rect, 8.0, cream));
    ui.add_space(10.0);
    // Label and status.
    ui.horizontal(|ui| {
        let half = (width - 8.0) / 2.0;
        let project = g.glass.projects.by_doc.entry(pkey.clone()).or_default();
        let tags = project.tags().to_vec();
        let tag = project.card_tag(&card);
        let resp = crate::labels::pill(ui, t, tag.as_ref(), half);
        egui::Popup::menu(&resp).show(|ui| match crate::labels::menu(ui, t, &tags, card.tag) {
            Some(crate::labels::Choice::Set(id)) => {
                let c = g.glass.projects.by_doc.entry(pkey.clone()).or_default().card_mut(&sec.title);
                c.tag = id;
                c.label = Label::None;
            }
            Some(crate::labels::Choice::Edit) => g.glass.labels_open = true,
            None => {}
        });
        let resp = menu_pill(ui, t, card.status.name(), Some(status_color(card.status, t)), half);
        egui::Popup::menu(&resp).show(|ui| {
            for st in Status::ALL {
                if ui.selectable_label(st == card.status, st.name()).clicked() {
                    g.glass.projects.by_doc.entry(pkey.clone()).or_default().card_mut(&sec.title).status = st;
                    ui.close();
                }
            }
        });
    });
}

// ---------------------------------------------------------------------------------------------
// The reference pane (the split).

/// What the split shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefView {
    /// A document's pages, read beside the one being written: this document (`None`), or
    /// another open tab (by its id).
    Document(Option<u64>),
    /// The manuscript's sections with their synopses.
    Manuscript,
    /// A notes folder.
    Notes(NoteKind),
}

impl RefView {
    fn name(self, g: &GlassApp) -> String {
        match self {
            RefView::Document(None) => "This Document".into(),
            RefView::Document(Some(uid)) => {
                g.docs.tabs.iter().position(|x| x.uid == uid).map(|i| g.tab_title(i)).unwrap_or_else(|| "Document".into())
            }
            RefView::Manuscript => "Manuscript".into(),
            RefView::Notes(k) => k.plural().into(),
        }
    }
    fn icon(self) -> Icon {
        match self {
            RefView::Document(_) => Icon::Doc,
            RefView::Manuscript => Icon::Manuscript,
            RefView::Notes(k) => kind_icon(k),
        }
    }
}

/// The split: another document (or another part of this one) beside the page, as Scrivener's
/// split editor; or the manuscript's sections, or a notes folder. Pick what it shows from the
/// menu in its header.
pub fn reference(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, view: RefView) {
    ui.horizontal(|ui| {
        let (ir, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        icon(ui.painter(), ir, view.icon(), t);
        let name = view.name(g);
        let font = semibold(14.0);
        let tw = ui.ctx().fonts_mut(|f| f.layout_no_wrap(format!("{name}  ⌄"), font.clone(), t.text).size().x).min(ui.available_width() - 40.0);
        let (r, resp) = ui.allocate_exact_size(vec2(tw + 12.0, 26.0), Sense::click());
        paint_state(ui, r, 8, &resp, false, t);
        ui.painter().text(pos2(r.min.x + 6.0, r.center().y), Align2::LEFT_CENTER, format!("{name}  ⌄"), font, t.text);
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(220.0);
            ui.label(egui::RichText::new("DOCUMENTS").font(semibold(10.5)).color(t.text_dim));
            if ui.selectable_label(view == RefView::Document(None), "This Document").clicked() {
                g.glass.panel = Some(Panel::Reference(RefView::Document(None)));
                ui.close();
            }
            let tabs: Vec<(u64, String)> =
                (0..g.docs.tabs.len()).filter(|i| *i != g.docs.active).filter_map(|i| g.docs.tabs.get(i).map(|x| (x.uid, g.tab_title(i)))).collect();
            for (uid, title) in tabs {
                if ui.selectable_label(view == RefView::Document(Some(uid)), title).clicked() {
                    g.glass.panel = Some(Panel::Reference(RefView::Document(Some(uid))));
                    ui.close();
                }
            }
            ui.separator();
            ui.label(egui::RichText::new("PROJECT").font(semibold(10.5)).color(t.text_dim));
            for (v, label) in [
                (RefView::Manuscript, "Manuscript"),
                (RefView::Notes(NoteKind::Character), "Characters"),
                (RefView::Notes(NoteKind::Place), "Places"),
                (RefView::Notes(NoteKind::Research), "Research"),
            ] {
                if ui.selectable_label(view == v, label).clicked() {
                    g.glass.panel = Some(Panel::Reference(v));
                    g.glass.scriv.ref_note = None;
                    ui.close();
                }
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if w::icon_capsule(ui, t, "close", "Close the split", 24.0, false, true).clicked() {
                g.glass.panel = None;
            }
            // Side by side, or one above the other.
            let below = g.glass.prefs.split_below;
            let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
            paint_state(ui, r, radius::CAPSULE, &resp, false, t);
            crate::glyphs::paint(ui.painter(), r.shrink(6.0), if below { "splitV" } else { "splitH" }, t.text_dim);
            if resp.on_hover_text(if below { "Split side by side" } else { "Split top and bottom" }).clicked() {
                g.glass.prefs.split_below = !below;
            }
            // Swap: the document in the pane comes into the editor, and this one goes to the pane.
            if let RefView::Document(Some(uid)) = view {
                let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                paint_state(ui, r, radius::CAPSULE, &resp, false, t);
                crate::glyphs::paint(ui.painter(), r.shrink(6.0), "swap", t.text_dim);
                if resp.on_hover_text("Swap: edit this document, and see the other one here").clicked() {
                    swap_split(g, uid);
                }
            }
        });
    });
    ui.add_space(8.0);
    let kind = match view {
        RefView::Document(uid) => {
            document_view(g, ui, t, uid);
            return;
        }
        RefView::Manuscript => None,
        RefView::Notes(k) => Some(k),
    };
    let pkey = key(&g.app.session);
    match kind {
        None => {
            let secs = sections(&g.app.session);
            let cur = current(g, &secs);
            let mut go: Option<usize> = None;
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (i, s) in secs.iter().enumerate() {
                    let card = g.glass.projects.by_doc.get(&pkey).map(|p| p.card(&s.title)).unwrap_or_default();
                    let tag = g.glass.projects.by_doc.get(&pkey).and_then(|p| p.card_tag(&card));
                    let syn = if card.synopsis.trim().is_empty() { s.opening.clone() } else { card.synopsis.clone() };
                    if item(ui, t, Icon::Doc, &s.title, &syn, cur == Some(i), tag.map(|x| crate::labels::color(x.tint))).clicked() {
                        go = Some(i);
                    }
                }
                if secs.is_empty() {
                    ui.label(egui::RichText::new("Headings become sections here.").font(regular(12.0)).color(t.text_dim));
                }
            });
            if let Some(s) = go.and_then(|i| secs.get(i)) {
                go_to(g, s);
            }
        }
        Some(kind) => {
            let notes: Vec<(u64, String, String)> = g
                .glass
                .projects
                .by_doc
                .get(&pkey)
                .map(|p| p.notes.iter().filter(|n| n.kind == kind).map(|n| (n.id, n.title.clone(), n.body.clone())).collect())
                .unwrap_or_default();
            let list_h = if g.glass.scriv.ref_note.is_some() { (ui.available_height() * 0.45).max(120.0) } else { ui.available_height() - 40.0 };
            egui::ScrollArea::vertical().id_salt("glass_ref_list").max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
                for (id, title, body) in &notes {
                    let on = g.glass.scriv.ref_note == Some(*id);
                    let syn: String = body.lines().filter(|l| !l.trim().is_empty()).take(3).collect::<Vec<_>>().join(" ");
                    if item(ui, t, kind_icon(kind), title, &syn, on, None).clicked() {
                        g.glass.scriv.ref_note = if on { None } else { Some(*id) };
                    }
                }
                if notes.is_empty() {
                    ui.label(egui::RichText::new(format!("No {} yet.", kind.plural().to_lowercase())).font(regular(12.0)).color(t.text_dim));
                }
            });
            ui.horizontal(|ui| {
                if w::icon_capsule(ui, t, "plus", &format!("New {}", kind.name()), 24.0, false, true).clicked() {
                    let id = g.glass.projects.by_doc.entry(pkey.clone()).or_default().add_note(kind, &format!("New {}", kind.name()));
                    g.glass.scriv.ref_note = Some(id);
                }
                ui.label(egui::RichText::new(format!("{} items", notes.len())).font(regular(11.0)).color(t.text_dim));
            });
            if let Some(id) = g.glass.scriv.ref_note
                && let Some((_, title, body)) = notes.iter().find(|n| n.0 == id).cloned()
            {
                ui.add_space(6.0);
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                ui.painter().rect_filled(r, 0.0, t.edge.gamma_multiply(0.5));
                ui.add_space(6.0);
                let mut title = title;
                let mut body = body;
                let a =
                    ui.add(egui::TextEdit::singleline(&mut title).font(semibold(15.0)).frame(egui::Frame::NONE).desired_width(ui.available_width()));
                let b = ui.add(egui::TextEdit::multiline(&mut body).font(regular(12.5)).desired_rows(10).desired_width(ui.available_width()));
                if (a.changed() || b.changed())
                    && let Some(n) = g.glass.projects.by_doc.get_mut(&pkey).and_then(|p| p.note_mut(id))
                {
                    n.title = title;
                    n.body = body;
                }
            }
        }
    }
}

fn kind_icon(k: NoteKind) -> Icon {
    match k {
        NoteKind::Character => Icon::Character,
        NoteKind::Place => Icon::Place,
        NoteKind::Research => Icon::Research,
    }
}

/// Swap the split: tab `uid` comes into the editor and the open document goes to the pane.
pub fn swap_split(g: &mut GlassApp, uid: u64) {
    let here = g.docs.tabs.get(g.docs.active).map(|t| t.uid);
    let Some(i) = g.docs.tabs.iter().position(|t| t.uid == uid) else { return };
    g.switch_to(i);
    g.glass.panel = Some(Panel::Reference(RefView::Document(here)));
}

/// A reference row: icon, title (bold), and two lines of synopsis.
fn item(ui: &mut Ui, t: &Tokens, kind: Icon, title: &str, synopsis: &str, on: bool, label: Option<Color32>) -> egui::Response {
    let width = ui.available_width();
    let font = regular(11.5);
    let galley = ui.ctx().fonts_mut(|f| {
        let mut job = egui::text::LayoutJob::single_section(
            synopsis.to_string(),
            egui::TextFormat { font_id: font.clone(), color: t.text_dim, ..Default::default() },
        );
        job.wrap.max_width = width - 40.0;
        job.wrap.max_rows = 2;
        f.layout_job(job)
    });
    let h = 26.0 + if synopsis.is_empty() { 0.0 } else { galley.size().y + 4.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(width, h), Sense::click());
    paint_state(ui, r, 8, &resp, on, t);
    icon(ui.painter(), Rect::from_min_size(pos2(r.min.x + 6.0, r.min.y + 5.0), vec2(16.0, 16.0)), kind, t);
    let tc = if on { t.accent } else { t.text };
    ui.painter().text(pos2(r.min.x + 28.0, r.min.y + 13.0), Align2::LEFT_CENTER, title, semibold(12.5), tc);
    if let Some(c) = label {
        ui.painter().circle_filled(pos2(r.max.x - 10.0, r.min.y + 13.0), 4.0, c);
    }
    if !synopsis.is_empty() {
        ui.painter().galley(pos2(r.min.x + 28.0, r.min.y + 24.0), galley, t.text_dim);
    }
    resp
}

/// A document's pages, rendered by the engine, scrolling at the pane's width. Click a page of
/// this document to go there; for another document, Open brings it into the editor.
fn document_view(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, uid: Option<u64>) {
    let tab = match uid {
        None => None,
        Some(u) => match g.docs.tabs.iter().position(|x| x.uid == u) {
            Some(i) if i != g.docs.active => Some(i),
            Some(_) => None,
            None => {
                ui.label(egui::RichText::new("That document was closed.").font(regular(12.0)).color(t.text_dim));
                return;
            }
        },
    };
    let pages = match tab {
        None => g.app.session.layout().pages.len(),
        Some(i) => g.docs.stash_mut(i).map(|s| s.layout().pages.len()).unwrap_or(0),
    };
    if let Some(i) = tab {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{pages} page{}", if pages == 1 { "" } else { "s" })).font(regular(11.5)).color(t.text_dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if w::quiet_button(ui, t, "Open in Editor", None, 26.0).clicked() {
                    g.switch_to(i);
                    g.glass.panel = Some(Panel::Reference(RefView::Document(None)));
                }
            });
        });
        ui.add_space(6.0);
    }
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let mut go: Option<usize> = None;
    egui::ScrollArea::vertical().id_salt(("glass_split_pages", uid)).auto_shrink([false, false]).show(ui, |ui| {
        // Below the page the pane is wide: pages keep a readable width, centred.
        let full = ui.available_width() - 4.0;
        let w = full.min(760.0);
        let pad = ((full - w) / 2.0).max(0.0);
        for p in 0..pages {
            let tex = g.thumbnail(&ctx, tab, p, (w * ppp) as u32);
            let h = tex.as_ref().map(|x| w * x.size_vec2().y / x.size_vec2().x.max(1.0)).unwrap_or(w * 1.294);
            let (r, resp) = ui.allocate_exact_size(vec2(full, h), Sense::click());
            let r = Rect::from_min_size(pos2(r.min.x + pad, r.min.y), vec2(w, h)).shrink2(vec2(2.0, 0.0));
            let shadow = egui::epaint::Shadow { offset: [0, 2], blur: 8, spread: 0, color: t.shadow };
            ui.painter().add(shadow.as_shape(r, egui::CornerRadius::same(3)));
            match &tex {
                Some(tex) => {
                    ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                None => {
                    ui.painter().rect_filled(r, 3.0, Color32::WHITE);
                }
            }
            if resp.hovered() && tab.is_none() {
                ui.painter().rect_stroke(r, 3.0, Stroke::new(1.5, t.accent.gamma_multiply(0.6)), egui::StrokeKind::Outside);
            }
            if resp.on_hover_text(if tab.is_none() { "Go to this page" } else { "Double-click to open" }).clicked() && tab.is_none() {
                go = Some(p);
            }
            ui.add_space(12.0);
        }
    });
    if let Some(p) = go {
        let _ = g.app.run("edit.goto", json!({"page": p + 1}));
        g.app.canvas.want_focus = true;
    }
}

// ---------------------------------------------------------------------------------------------
// The Binder.

fn row(ui: &mut Ui, t: &Tokens, depth: usize, kind: Icon, text: &str, fold: Option<bool>, on: bool, dot: Option<Color32>) -> (egui::Response, bool) {
    row_sensing(ui, t, depth, kind, text, fold, on, dot, Sense::click())
}

/// A Binder row that can also be dragged.
#[allow(clippy::too_many_arguments)]
fn row_sensing(
    ui: &mut Ui,
    t: &Tokens,
    depth: usize,
    kind: Icon,
    text: &str,
    fold: Option<bool>,
    on: bool,
    dot: Option<Color32>,
    sense: Sense,
) -> (egui::Response, bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), sense);
    paint_state(ui, r, 6, &resp, on, t);
    let x = r.min.x + 4.0 + depth as f32 * 14.0;
    let mut toggled = false;
    if let Some(open) = fold {
        let tri = Rect::from_center_size(pos2(x + 5.0, r.center().y), vec2(14.0, 18.0));
        let tresp = ui.interact(tri, resp.id.with("fold"), Sense::click());
        disclosure(ui.painter(), tri.center(), open, if tresp.hovered() { t.text } else { t.text_dim });
        toggled = tresp.clicked();
    }
    icon(ui.painter(), Rect::from_min_size(pos2(x + 12.0, r.center().y - 8.0), vec2(16.0, 16.0)), kind, t);
    let max = ((r.max.x - x - 44.0) / 6.8).max(3.0) as usize;
    let shown: String = if text.chars().count() > max { format!("{}…", text.chars().take(max).collect::<String>()) } else { text.to_string() };
    ui.painter().text(
        pos2(x + 33.0, r.center().y),
        Align2::LEFT_CENTER,
        shown,
        if on { medium(12.5) } else { regular(12.5) },
        if on { t.accent } else { t.text },
    );
    if let Some(c) = dot {
        ui.painter().circle_filled(pos2(r.max.x - 10.0, r.center().y), 3.5, c);
    }
    (resp, toggled)
}

/// The section `i` sits under (`None`: the top).
fn parent_of(secs: &[crate::project::Section], i: usize) -> Option<usize> {
    let level = secs.get(i)?.level;
    secs.iter().take(i).rposition(|s| s.level < level)
}

/// The siblings of section `i` (same level, same parent), in order.
fn siblings(secs: &[crate::project::Section], i: usize) -> Vec<usize> {
    let Some(level) = secs.get(i).map(|s| s.level) else { return Vec::new() };
    let parent = parent_of(secs, i);
    (0..secs.len()).filter(|j| secs.get(*j).is_some_and(|s| s.level == level) && parent_of(secs, *j) == parent).collect()
}

/// Where dropping section `from` on row `over` puts it: the sibling whose part holds that row.
pub(crate) fn drop_target(secs: &[crate::project::Section], from: usize, over: usize) -> Option<usize> {
    let sib = siblings(secs, from);
    if sib.contains(&over) {
        return Some(over);
    }
    // A row inside a sibling's part counts as that sibling.
    let first = *sib.first()?;
    let last_end = sib
        .last()
        .map(|l| (l + 1..secs.len()).find(|j| secs.get(*j).is_some_and(|s| s.level <= secs.get(*l).map_or(0, |x| x.level))).unwrap_or(secs.len()))?;
    if over < first || over >= last_end {
        return None;
    }
    sib.iter().copied().rfind(|s| *s <= over)
}

/// How many places among its siblings section `from` moves to reach `to`.
pub(crate) fn sibling_offset(secs: &[crate::project::Section], from: usize, to: usize) -> Option<i64> {
    let sib = siblings(secs, from);
    let a = sib.iter().position(|s| *s == from)? as i64;
    let b = sib.iter().position(|s| *s == to)? as i64;
    Some(b - a)
}

/// The Binder: Manuscript (sections as folders and documents), open documents, Characters,
/// Places, Research and the Scratchpad, each with its icon.
pub fn binder(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Binder").font(semibold(15.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if w::icon_capsule(ui, t, "close", "Hide Binder", 24.0, false, true).clicked() {
                g.glass.prefs.binder = false;
            }
        });
    });
    ui.add_space(6.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
        let secs = sections(&g.app.session);
        let cur = current(g, &secs);
        let pkey = key(&g.app.session);
        let base = secs.iter().map(|s| s.level).min().unwrap_or(0);
        // Manuscript.
        let man_open = !g.glass.scriv.folded.contains("\u{0}manuscript");
        let (_, tog) = row(ui, t, 0, Icon::Manuscript, "Manuscript", Some(man_open), false, None);
        if tog {
            toggle_fold(g, "\u{0}manuscript");
        }
        let mut go: Option<usize> = None;
        let mut action: Option<(usize, u8)> = None;
        // Sections can be dragged to a new place among their siblings.
        let mut shown_rows: Vec<(usize, Rect)> = Vec::new();
        let mut drag_from: Option<usize> = None;
        let mut dropped = false;
        if man_open {
            let mut hidden_below: Option<u8> = None;
            for (i, s) in secs.iter().enumerate() {
                if let Some(l) = hidden_below {
                    if s.level > l {
                        continue;
                    }
                    hidden_below = None;
                }
                let has_children = secs.get(i + 1).is_some_and(|n| n.level > s.level);
                let open = !g.glass.scriv.folded.contains(&s.title);
                let card = g.glass.projects.by_doc.get(&pkey).map(|p| p.card(&s.title)).unwrap_or_default();
                let tag = g.glass.projects.by_doc.get(&pkey).and_then(|p| p.card_tag(&card));
                let depth = 1 + usize::from(s.level.saturating_sub(base).min(5));
                let (resp, tog) = row_sensing(
                    ui,
                    t,
                    depth,
                    if has_children { Icon::Folder } else { Icon::Doc },
                    &s.title,
                    has_children.then_some(open),
                    cur == Some(i) && g.glass.view == View::Page,
                    if tag.is_some() { None } else { (card.status != Status::ToDo).then(|| status_color(card.status, t)) },
                    Sense::click_and_drag(),
                );
                shown_rows.push((i, resp.rect));
                if resp.dragged() || resp.drag_stopped() {
                    drag_from = Some(i);
                    dropped |= resp.drag_stopped();
                }
                if let Some(tag) = &tag {
                    crate::labels::mark(
                        ui.painter(),
                        Rect::from_center_size(pos2(resp.rect.max.x - 12.0, resp.rect.center().y), vec2(12.0, 12.0)),
                        tag,
                    );
                }
                if tog {
                    toggle_fold(g, &s.title);
                } else if resp.clicked() {
                    if ui.input(|i| i.modifiers.alt) {
                        g.glass.scriv.quick = Some(s.path.clone());
                    } else {
                        go = Some(i);
                    }
                }
                resp.context_menu(|ui| {
                    if ui.button("Go to Section").clicked() {
                        action = Some((i, 0));
                        ui.close();
                    }
                    if ui.button("Open in Quick Reference").clicked() {
                        action = Some((i, 1));
                        ui.close();
                    }
                    if ui.button("Show in Inspector").clicked() {
                        action = Some((i, 2));
                        ui.close();
                    }
                });
                if has_children && !open {
                    hidden_below = Some(s.level);
                }
            }
            if secs.is_empty() {
                ui.label(egui::RichText::new("      Headings become chapters and scenes here.").font(regular(11.5)).color(t.text_dim));
            }
            if let Some(from) = drag_from {
                let pointer = ui.ctx().pointer_interact_pos();
                let over = pointer.and_then(|p| shown_rows.iter().find(|(_, r)| p.y >= r.min.y && p.y < r.max.y).map(|(j, _)| *j));
                let to = over.and_then(|j| drop_target(&secs, from, j));
                // A line where it will land, and the dragged row's title under the pointer.
                if let (Some(to), Some(p)) = (to, pointer)
                    && let Some((_, r)) = shown_rows.iter().find(|(j, _)| *j == to)
                {
                    let y = if to > from { r.max.y } else { r.min.y };
                    ui.painter().hline(r.min.x + 6.0..=r.max.x - 6.0, y, Stroke::new(2.0, t.accent));
                    if let Some(sec) = secs.get(from) {
                        let layer = ui.painter().clone().with_layer_id(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("binder_drag")));
                        let label: String = sec.title.chars().take(30).collect();
                        let galley = layer.layout_no_wrap(label, medium(12.0), t.text);
                        let br = Rect::from_min_size(p + vec2(12.0, -10.0), galley.size() + vec2(16.0, 8.0));
                        layer.add(crate::tokens::popover_shape(br, 8, t));
                        layer.galley(br.min + vec2(8.0, 4.0), galley, t.text);
                    }
                }
                if dropped
                    && let Some(to) = to
                    && to != from
                {
                    let by = sibling_offset(&secs, from, to);
                    if let (Some(sec), Some(by)) = (secs.get(from).cloned(), by) {
                        crate::pro::move_section(g, &sec, by);
                    }
                } else if dropped && over.is_some() && to.is_none() {
                    g.app.status("Sections move among the ones at the same level: drag it beside a sibling");
                }
            }
        }
        match action {
            Some((i, 1)) => g.glass.scriv.quick = secs.get(i).map(|s| s.path.clone()),
            Some((i, 2)) => {
                go = Some(i);
                g.glass.panel = Some(Panel::Inspector);
            }
            Some((i, _)) => go = Some(i),
            None => {}
        }
        if let Some(s) = go.and_then(|i| secs.get(i)) {
            go_to(g, s);
        }
        ui.add_space(6.0);
        // Open documents.
        let mut switch: Option<usize> = None;
        for i in 0..g.docs.tabs.len() {
            let on = i == g.docs.active;
            if row(ui, t, 0, Icon::Doc, &g.tab_title(i), None, on, None).0.clicked() {
                switch = Some(i);
            }
        }
        if let Some(i) = switch {
            g.switch_to(i);
        }
        ui.add_space(6.0);
        // Notes folders.
        for kind in NoteKind::ALL {
            let fold_key = format!("\u{0}{}", kind.plural());
            let open = !g.glass.scriv.folded.contains(&fold_key);
            let (resp, tog) = row(
                ui,
                t,
                0,
                kind_icon(kind),
                kind.plural(),
                Some(open),
                matches!(g.glass.panel, Some(Panel::Reference(RefView::Notes(k))) if k == kind),
                None,
            );
            // A + appears on the folder's row.
            let plus = Rect::from_center_size(pos2(resp.rect.max.x - 12.0, resp.rect.center().y), vec2(20.0, 20.0));
            let presp = ui.interact(plus, resp.id.with("plus"), Sense::click()).on_hover_text(format!("New {}", kind.name()));
            if presp.hovered() || resp.hovered() {
                let st = Stroke::new(1.3, t.text_dim);
                let c = plus.center();
                ui.painter().line_segment([pos2(c.x - 4.0, c.y), pos2(c.x + 4.0, c.y)], st);
                ui.painter().line_segment([pos2(c.x, c.y - 4.0), pos2(c.x, c.y + 4.0)], st);
            }
            if presp.clicked() {
                let id = g.glass.projects.by_doc.entry(pkey.clone()).or_default().add_note(kind, &format!("New {}", kind.name()));
                g.glass.panel = Some(Panel::Note(id));
            } else if tog {
                toggle_fold(g, &fold_key);
            } else if resp.clicked() {
                g.glass.panel = Some(Panel::Reference(RefView::Notes(kind)));
            }
            if open {
                let notes: Vec<(u64, String, Option<crate::project::Tag>)> = g
                    .glass
                    .projects
                    .by_doc
                    .get(&pkey)
                    .map(|p| {
                        p.notes.iter().filter(|n| n.kind == kind).map(|n| (n.id, n.title.clone(), n.tag.and_then(|id| p.tag(id)).cloned())).collect()
                    })
                    .unwrap_or_default();
                for (id, title, tag) in notes {
                    let on = g.glass.panel == Some(Panel::Note(id));
                    let (resp, _) = row(ui, t, 1, Icon::Doc, &title, None, on, None);
                    if let Some(tag) = &tag {
                        crate::labels::mark(
                            ui.painter(),
                            Rect::from_center_size(pos2(resp.rect.max.x - 12.0, resp.rect.center().y), vec2(12.0, 12.0)),
                            tag,
                        );
                    }
                    if resp.clicked() {
                        g.glass.panel = if on { None } else { Some(Panel::Note(id)) };
                    }
                }
            }
        }
        ui.add_space(6.0);
        let n = g.glass.projects.by_doc.get(&pkey).map(|p| p.scraps.len()).unwrap_or(0);
        let on = g.glass.panel == Some(Panel::Scratchpad);
        if row(ui, t, 0, Icon::Scratch, &format!("Scratchpad ({n})"), None, on, None).0.clicked() {
            g.glass.panel = if on { None } else { Some(Panel::Scratchpad) };
        }
    });
}

fn toggle_fold(g: &mut GlassApp, k: &str) {
    if !g.glass.scriv.folded.remove(k) {
        g.glass.scriv.folded.insert(k.to_string());
    }
}

// ---------------------------------------------------------------------------------------------
// Quick Reference.

/// A section in a small floating window, to read while writing elsewhere. Drag it by its title.
pub fn quick_reference(g: &mut GlassApp, ctx: &egui::Context, t: &Tokens) {
    let Some(path) = g.glass.scriv.quick.clone() else { return };
    let secs = sections(&g.app.session);
    let Some(i) = secs.iter().position(|s| s.path == path) else {
        g.glass.scriv.quick = None;
        return;
    };
    let Some(sec) = secs.get(i).cloned() else { return };
    let screen = ctx.content_rect();
    let mut close = false;
    let mut goto: Option<usize> = None;
    let area = egui::Area::new(egui::Id::new("glass_quick_reference"))
        .order(egui::Order::Foreground)
        .movable(true)
        .default_pos(pos2(screen.center().x - 250.0, (screen.max.y - 470.0).max(screen.min.y + 120.0)))
        .show(ctx, |ui| {
            let bg = ui.painter().add(egui::Shape::Noop);
            let inner = egui::Frame::NONE.inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 12 }).show(ui, |ui| {
                ui.set_width(500.0);
                // Title bar, tinted like Scrivener's.
                let (bar, _) = ui.allocate_exact_size(vec2(500.0, 36.0), Sense::hover());
                ui.painter().rect_filled(bar, egui::CornerRadius { nw: 14, ne: 14, sw: 0, se: 0 }, t.accent.gamma_multiply(0.12));
                ui.painter().hline(bar.x_range(), bar.max.y, Stroke::new(1.0, t.accent.gamma_multiply(0.35)));
                let mut bar_ui = ui.new_child(
                    egui::UiBuilder::new().max_rect(bar.shrink2(vec2(10.0, 4.0))).layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                bar_ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                if small_button(&mut bar_ui, t, "chevronLeft", "Previous section", i > 0) {
                    goto = i.checked_sub(1);
                }
                if small_button(&mut bar_ui, t, "chevronRight", "Next section", i + 1 < secs.len()) {
                    goto = Some(i + 1);
                }
                bar_ui.add_space(6.0);
                let (ir, _) = bar_ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                icon(bar_ui.painter(), ir, Icon::Doc, t);
                bar_ui.add_space(4.0);
                bar_ui.label(egui::RichText::new(&sec.title).font(medium(13.0)).color(t.text));
                bar_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if w::icon_capsule(ui, t, "close", "Close Quick Reference", 24.0, false, true).clicked() {
                        close = true;
                    }
                });
                ui.add_space(10.0);
                egui::Frame::NONE.inner_margin(egui::Margin::symmetric(22, 0)).show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                        ui.set_width(456.0);
                        ui.spacing_mut().item_spacing = vec2(0.0, 10.0);
                        if sec.text.trim().is_empty() {
                            ui.label(egui::RichText::new("This section is empty.").font(regular(13.0)).color(t.text_dim));
                        }
                        for para in sec.text.lines().filter(|l| !l.trim().is_empty()) {
                            ui.add(egui::Label::new(egui::RichText::new(para.trim()).font(regular(14.5)).color(t.text)).wrap());
                        }
                    });
                });
            });
            ui.painter().set(bg, crate::tokens::popover_shape(inner.response.rect, 14, t));
        });
    let _ = area;
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape) && i.modifiers.alt) {
        g.glass.scriv.quick = None;
    }
    if let Some(n) = goto.and_then(|n| secs.get(n)) {
        g.glass.scriv.quick = Some(n.path.clone());
    }
}

// ---------------------------------------------------------------------------------------------
// Sections on the page (Scrivenings).

/// Each heading in a soft box tinted with its label, and a dashed rule between sections.
pub fn section_boxes(
    g: &mut GlassApp,
    painter: &Painter,
    layout: &DocLayout,
    blocks: &[usize],
    t: &Tokens,
    rects_of: &mut dyn FnMut(&mut GlassApp, usize) -> Vec<(usize, Rect)>,
) {
    let pkey = key(&g.app.session);
    let first_heading = sections(&g.app.session).first().map(|s| s.path.last());
    for &b in blocks {
        let path = Path::top(b);
        let Some(p) = g.app.session.doc.para(StoryRef::Body, &path) else { continue };
        let rp = g.app.session.doc.styles.resolve_para(&p.props);
        if rp.outline_level.is_none() || rp.style.starts_with("TOC") || p.plain_text().trim().is_empty() {
            continue;
        }
        let title = p.plain_text().trim().to_string();
        let card = g.glass.projects.by_doc.get(&pkey).map(|x| x.card(&title)).unwrap_or_default();
        let tag = g.glass.projects.by_doc.get(&pkey).and_then(|x| x.card_tag(&card));
        let tint = tag.as_ref().map(|x| crate::labels::color(x.tint)).unwrap_or(t.accent);
        let strength = if tag.is_none() { 0.55 } else { 1.0 };
        let rects = rects_of(g, b);
        let Some((page, _)) = rects.first().copied() else { continue };
        let Some(u) = rects.iter().filter(|(p, _)| *p == page).map(|(_, r)| *r).reduce(|a, c| a.union(c)) else { continue };
        let (Some(pr), Some(pg)) = (g.app.canvas.page_rects.get(page), layout.pages.get(page)) else { continue };
        let s = pr.width() / pg.w.max(1.0);
        let left = pr.min.x + pg.body.x * s;
        let right = left + pg.body.w * s;
        let bx = Rect::from_min_max(pos2(left - 8.0, u.min.y - 4.0), pos2(right + 8.0, u.max.y + 4.0));
        painter.rect_filled(bx, 5.0, tint.gamma_multiply(0.10 * strength));
        painter.rect_stroke(bx, 5.0, Stroke::new(1.0, tint.gamma_multiply(0.45 * strength)), egui::StrokeKind::Inside);
        if Some(b) != first_heading {
            let y = bx.min.y - 14.0;
            painter.extend(egui::Shape::dashed_line(&[pos2(left, y), pos2(right, y)], Stroke::new(1.0, t.text_dim.gamma_multiply(0.45)), 6.0, 5.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binder_drops_land_among_siblings() {
        let sec = |level: u8| Section { path: Path::top(0), level, title: String::new(), words: 0, opening: String::new(), text: String::new() };
        // Ch1 (0), Sc1.1 (1), Sc1.2 (2), Ch2 (3), Sc2.1 (4), Ch3 (5).
        let secs = vec![sec(0), sec(1), sec(1), sec(0), sec(1), sec(0)];
        assert_eq!(drop_target(&secs, 0, 5), Some(5));
        assert_eq!(sibling_offset(&secs, 0, 5), Some(2));
        // A row inside Chapter 2 counts as Chapter 2.
        assert_eq!(drop_target(&secs, 0, 4), Some(3));
        assert_eq!(drop_target(&secs, 5, 1), Some(0));
        assert_eq!(sibling_offset(&secs, 5, 0), Some(-2));
        // Scenes move within their chapter only.
        assert_eq!(drop_target(&secs, 1, 2), Some(2));
        assert_eq!(drop_target(&secs, 1, 4), None);
    }

    #[test]
    fn swapping_the_split_brings_the_other_document_into_the_editor() {
        use wordcraft_engine::Session;
        let mut g =
            crate::GlassApp::new(wordcraft_ui_egui::WordApp::new(Session::new(wordcraft_engine::sample::sample_document()), Default::default()));
        g.new_tab(Session::new(wordcraft_engine::doc::Document::new()));
        let (first, second) = (g.docs.tabs[0].uid, g.docs.tabs[1].uid);
        g.glass.panel = Some(Panel::Reference(RefView::Document(Some(first))));
        swap_split(&mut g, first);
        assert_eq!(g.docs.active, 0);
        assert!(g.app.session.word_count() > 0);
        assert_eq!(g.glass.panel, Some(Panel::Reference(RefView::Document(Some(second)))));
    }
}

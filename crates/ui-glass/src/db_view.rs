//! The database page: a database's name and description, its views as tabs, a toolbar (search,
//! filter, sort, group, properties, import/export, New), and the open view: Table, Board,
//! Gallery, List or Calendar. A record opens in a side panel (every property, editable) and from
//! there as a page.
//!
//! The database is taken out of its tab while it is drawn and put back after, so the page can
//! edit it freely; anything that needs the rest of the app (opening a page, a file dialog, Mail
//! Merge) is returned as an [`Action`] and done afterwards.

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::db::{Cell, Database, Filter, Kind, Layout, Op, Sort, Tint, View};
use crate::glyphs;
use crate::tokens::{Tokens, paint_state, radius};

/// What the page asks the app to do once drawing is done.
pub enum Action {
    /// Open a record as a page.
    OpenPage(u64),
    /// Pick a file to import (CSV, spreadsheet, vCard) into this database.
    Import,
    /// Save the records shown as CSV.
    ExportCsv(Vec<u64>),
    /// Use the records shown as Mail Merge recipients and switch to a document.
    MailMerge(Vec<u64>),
    /// Show a message in the status bar.
    Status(String),
}

/// The page's own state (shared by the open database tabs).
#[derive(Default)]
pub struct DbUi {
    pub search: String,
    /// The cell being typed in: (record, property, text, focused yet).
    editing: Option<(u64, u64, String, bool)>,
    /// The record open in the side panel.
    pub peek: Option<u64>,
    /// The Calendar's month (year, month).
    month: Option<(i32, u32)>,
    /// A view or property being renamed.
    rename: Option<(u64, String)>,
    /// The name typed for a new property, and its kind.
    new_prop: String,
    new_kind: Kind,
    /// A new option's name being typed in a select menu.
    new_option: String,
}

fn c32(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// A tint's chip colours (background, text) for the theme.
pub fn chip_colors(tint: Tint, t: &Tokens) -> (Color32, Color32) {
    let strong = c32(tint.strong());
    if t.dark { (strong.gamma_multiply(0.35), Color32::from_gray(235)) } else { (c32(tint.soft()), strong.linear_multiply(0.8)) }
}

/// A rounded chip with a label; returns its width.
fn chip(ui: &Ui, at: egui::Pos2, text: &str, tint: Tint, t: &Tokens, max_w: f32) -> f32 {
    let (bg, fg) = chip_colors(tint, t);
    let font = medium(11.5);
    let galley = ui.painter().layout(text.to_string(), font, fg, f32::INFINITY);
    let w = (galley.size().x + 14.0).min(max_w.max(20.0));
    let r = Rect::from_min_size(pos2(at.x, at.y - 10.0), vec2(w, 20.0));
    ui.painter().rect_filled(r, 6.0, bg);
    ui.painter().with_clip_rect(r.shrink2(vec2(6.0, 0.0))).galley(pos2(r.min.x + 7.0, r.center().y - galley.size().y / 2.0), galley, fg);
    w
}

/// Option chips for a cell, left to right from `at`, within `max_w`.
fn chips(ui: &Ui, db: &Database, prop: u64, names: &[String], at: egui::Pos2, max_w: f32, t: &Tokens) {
    let mut x = at.x;
    for n in names {
        let room = at.x + max_w - x;
        if room < 24.0 {
            break;
        }
        x += chip(ui, pos2(x, at.y), n, db.option_tint(prop, n), t, room) + 5.0;
    }
}

/// A cell's value drawn in `r` (not editing).
fn draw_value(ui: &Ui, db: &Database, row: u64, prop: u64, kind: Kind, r: Rect, t: &Tokens) {
    let y = r.center().y;
    let x = r.min.x + 8.0;
    let painter = ui.painter().with_clip_rect(r);
    match (kind, db.cell(row, prop)) {
        (_, None) => {}
        (Kind::Checkbox, Some(Cell::Bool(b))) => checkbox_mark(&painter, pos2(x + 8.0, y), *b, t),
        (Kind::Rating, Some(Cell::Number(n))) => stars(&painter, pos2(x, y), *n as i32, t),
        (_, Some(Cell::Options(o))) => chips(ui, db, prop, o, pos2(x, y), r.width() - 16.0, t),
        (Kind::Url | Kind::Email, Some(c)) => {
            painter.text(pos2(x, y), Align2::LEFT_CENTER, c.text(), regular(12.5), t.accent);
        }
        (Kind::Number, Some(c)) => {
            painter.text(pos2(r.max.x - 8.0, y), Align2::RIGHT_CENTER, c.text(), regular(12.5), t.text);
        }
        (Kind::Date, Some(c)) => {
            painter.text(pos2(x, y), Align2::LEFT_CENTER, pretty_date(&c.text()), regular(12.5), t.text);
        }
        (Kind::Title, Some(c)) => {
            painter.text(pos2(x, y), Align2::LEFT_CENTER, c.text(), medium(13.0), t.text);
        }
        (_, Some(c)) => {
            let text = c.text().replace('\n', " ");
            painter.text(pos2(x, y), Align2::LEFT_CENTER, text, regular(12.5), t.text);
        }
    }
}

fn checkbox_mark(p: &egui::Painter, c: egui::Pos2, on: bool, t: &Tokens) {
    let r = Rect::from_center_size(c, vec2(15.0, 15.0));
    if on {
        p.rect_filled(r, 4.0, t.accent);
        let s = Stroke::new(1.8, t.on_accent);
        p.line_segment([pos2(r.min.x + 3.5, r.center().y), pos2(r.min.x + 6.5, r.max.y - 4.0)], s);
        p.line_segment([pos2(r.min.x + 6.5, r.max.y - 4.0), pos2(r.max.x - 3.0, r.min.y + 3.5)], s);
    } else {
        p.rect_stroke(r, 4.0, Stroke::new(1.2, t.text_dim), egui::StrokeKind::Inside);
    }
}

fn stars(p: &egui::Painter, at: egui::Pos2, n: i32, t: &Tokens) {
    for i in 0..5 {
        let r = Rect::from_center_size(pos2(at.x + 8.0 + i as f32 * 16.0, at.y), vec2(13.0, 13.0));
        glyphs::paint(p, r, "star", if i < n { Color32::from_rgb(0xE8, 0xA3, 0x17) } else { t.fill });
    }
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const MONTH_NAMES: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// `2026-10-12` → `12 Oct 2026`.
pub fn pretty_date(iso: &str) -> String {
    let mut it = iso.split('-');
    let (Some(y), Some(m), Some(d)) = (it.next(), it.next(), it.next()) else { return iso.to_string() };
    let m = m.parse::<usize>().ok().and_then(|m| MONTHS.get(m.wrapping_sub(1))).copied().unwrap_or("");
    format!("{} {m} {y}", d.trim_start_matches('0'))
}

/// The icon of a property kind.
fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Title => "doc",
        Kind::Text => "list",
        Kind::Number => "tag",
        Kind::Select | Kind::Status => "circle",
        Kind::MultiSelect => "tag",
        Kind::Date => "calendar",
        Kind::Checkbox => "check",
        Kind::Url => "globe",
        Kind::Email => "pen",
        Kind::Phone => "person",
        Kind::Rating => "star",
    }
}

/// Draw the page for `db`.
pub fn show(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens) -> Vec<Action> {
    let mut actions = Vec::new();
    if db.views.is_empty() {
        db.add_view("Table", Layout::Table);
    }
    let vi = db_current(db);
    let full = ui.max_rect();
    let peek_w = if st.peek.is_some() { 400.0_f32.min(full.width() * 0.45) } else { 0.0 };
    let main = Rect::from_min_max(full.min, pos2(full.max.x - peek_w, full.max.y));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(main).layout(egui::Layout::top_down(egui::Align::Min)));
    let ui2 = &mut child;
    egui::ScrollArea::vertical().id_salt("glass_db_scroll").auto_shrink([false, false]).show(ui2, |ui| {
        let margin = (ui.available_width() * 0.06).clamp(24.0, 96.0);
        egui::Frame::NONE.inner_margin(egui::Margin { left: margin as i8, right: (margin * 0.6) as i8, top: 26, bottom: 40 }).show(ui, |ui| {
            header(db, st, ui, t);
            ui.add_space(14.0);
            view_bar(db, st, ui, t, vi, &mut actions);
            ui.add_space(8.0);
            let Some(view) = db.views.get(vi).cloned() else { return };
            let ids = db.visible(&view, &st.search);
            match view.layout {
                Layout::Table => table(db, st, ui, t, &view, &ids, &mut actions),
                Layout::Board => board(db, st, ui, t, vi, &view, &ids),
                Layout::Gallery => gallery(db, st, ui, t, &view, &ids),
                Layout::List => list(db, st, ui, t, &view, &ids),
                Layout::Calendar => calendar(db, st, ui, t, vi, &view, &ids),
            }
        });
    });
    if st.peek.is_some() {
        let r = Rect::from_min_max(pos2(main.max.x, full.min.y), full.max);
        peek(db, st, ui, t, r, &mut actions);
    }
    actions
}

/// The open view's index.
fn db_current(db: &Database) -> usize {
    db.current.min(db.views.len().saturating_sub(1))
}

fn header(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::click());
        paint_state(ui, r, 12, &resp, false, t);
        glyphs::paint(ui.painter(), r.shrink(8.0), &db.icon, t.accent);
        egui::Popup::menu(&resp).show(|ui| {
            ui.label(egui::RichText::new("Icon").font(medium(11.5)).color(t.text_dim));
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(6.0 * 34.0);
                for g in glyphs::ALL {
                    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
                    paint_state(ui, r, 8, &resp, db.icon == g, t);
                    glyphs::paint(ui.painter(), r.shrink(7.0), g, t.text);
                    if resp.clicked() {
                        db.icon = g.to_string();
                        db.touch();
                    }
                }
            });
        });
        ui.add_space(8.0);
        let mut name = db.name.clone();
        let edit = egui::TextEdit::singleline(&mut name)
            .font(semibold(28.0))
            .frame(egui::Frame::NONE)
            .desired_width(ui.available_width())
            .hint_text("Untitled");
        if ui.add(edit).changed() {
            db.name = name;
            db.touch();
        }
    });
    let mut desc = db.description.clone();
    let edit = egui::TextEdit::multiline(&mut desc)
        .font(regular(13.5))
        .text_color(t.text_dim)
        .frame(egui::Frame::NONE)
        .desired_rows(1)
        .desired_width(ui.available_width().min(760.0))
        .hint_text("Add a description");
    if ui.add(edit).changed() {
        db.description = desc;
        db.touch();
    }
    let _ = st;
}

/// The views as tabs, then the toolbar.
fn view_bar(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, vi: usize, actions: &mut Vec<Action>) {
    let row_h = 30.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
        let mut select: Option<usize> = None;
        let mut delete: Option<usize> = None;
        let mut duplicate: Option<usize> = None;
        for (i, v) in db.views.clone().iter().enumerate() {
            let galley_w = ui.painter().layout_no_wrap(v.name.clone(), medium(12.5), t.text).size().x;
            let (r, resp) = ui.allocate_exact_size(vec2(galley_w + 40.0, row_h), Sense::click());
            let on = i == vi;
            if on {
                crate::tokens::paint_pill(ui.painter(), r, t);
            } else {
                paint_state(ui, r, radius::CAPSULE, &resp, false, t);
            }
            let c = if on { t.text } else { t.text_dim };
            glyphs::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 17.0, r.center().y), vec2(14.0, 14.0)), v.layout.icon(), c);
            ui.painter().text(pos2(r.min.x + 30.0, r.center().y), Align2::LEFT_CENTER, &v.name, if on { semibold(12.5) } else { medium(12.5) }, c);
            if resp.clicked() {
                select = Some(i);
            }
            resp.context_menu(|ui| {
                ui.set_min_width(180.0);
                let mut name = st.rename.as_ref().filter(|(id, _)| *id == v.id).map(|(_, n)| n.clone()).unwrap_or_else(|| v.name.clone());
                if ui.add(egui::TextEdit::singleline(&mut name).desired_width(170.0)).changed() {
                    st.rename = Some((v.id, name.clone()));
                    if let Some(view) = db.views.get_mut(i) {
                        view.name = name;
                    }
                    db.touch();
                }
                ui.separator();
                ui.label(egui::RichText::new("Layout").font(medium(11.0)).color(t.text_dim));
                for l in Layout::ALL {
                    if ui.selectable_label(v.layout == l, l.name()).clicked() {
                        set_layout(db, i, l);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Duplicate view").clicked() {
                    duplicate = Some(i);
                    ui.close();
                }
                if db.views.len() > 1 && ui.button("Delete view").clicked() {
                    delete = Some(i);
                    ui.close();
                }
            });
        }
        // Add a view.
        let (r, resp) = ui.allocate_exact_size(vec2(row_h, row_h), Sense::click());
        paint_state(ui, r, radius::CAPSULE, &resp, false, t);
        plus(ui.painter(), r.center(), t.text_dim);
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(170.0);
            ui.label(egui::RichText::new("New view").font(medium(11.0)).color(t.text_dim));
            for l in Layout::ALL {
                if icon_row(ui, t, l.icon(), l.name()).clicked() {
                    let name = l.name().to_string();
                    db.add_view(&name, l);
                    select = Some(db.views.len() - 1);
                    ui.close();
                }
            }
        });
        if let Some(i) = duplicate
            && let Some(mut v) = db.views.get(i).cloned()
        {
            let id = db.add_view(&format!("{} copy", v.name), v.layout);
            if let Some(last) = db.views.last_mut() {
                v.id = id;
                v.name = last.name.clone();
                *last = v;
            }
            select = Some(db.views.len() - 1);
        }
        if let Some(i) = delete {
            db.views.remove(i);
            db.current = db.current.min(db.views.len().saturating_sub(1));
            db.touch();
        }
        if let Some(i) = select {
            db.current = i;
            db.touch();
        }
        // Toolbar, on the right.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
            if crate::widgets::accent_button(ui, t, None, "New", Some(70.0), row_h).clicked() {
                let id = new_record(db, db_current(db), None);
                st.peek = Some(id);
            }
            let vi = db_current(db);
            more_menu(db, st, ui, t, vi, actions);
            properties_menu(db, ui, t, vi);
            if db.views.get(vi).is_some_and(|v| matches!(v.layout, Layout::Board | Layout::Table)) {
                group_menu(db, ui, t, vi);
            }
            if db.views.get(vi).is_some_and(|v| v.layout == Layout::Calendar) {
                date_menu(db, ui, t, vi);
            }
            sort_menu(db, ui, t, vi);
            filter_menu(db, ui, t, vi);
            let search = egui::TextEdit::singleline(&mut st.search).hint_text("Search").desired_width(150.0).margin(vec2(10.0, 6.0));
            ui.add(search);
        });
    });
}

fn plus(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    let s = Stroke::new(1.5, color);
    p.line_segment([pos2(c.x - 5.0, c.y), pos2(c.x + 5.0, c.y)], s);
    p.line_segment([pos2(c.x, c.y - 5.0), pos2(c.x, c.y + 5.0)], s);
}

/// A menu row with a glyph.
fn icon_row(ui: &mut Ui, t: &Tokens, icon: &str, label: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(160.0), 26.0), Sense::click());
    paint_state(ui, r, 6, &resp, false, t);
    glyphs::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 14.0, r.center().y), vec2(14.0, 14.0)), icon, t.text_dim);
    ui.painter().text(pos2(r.min.x + 30.0, r.center().y), Align2::LEFT_CENTER, label, regular(12.5), t.text);
    resp
}

/// A toolbar button with a glyph and a label (accented when `on`).
fn tool(ui: &mut Ui, t: &Tokens, icon: &str, label: &str, on: bool) -> egui::Response {
    let w = ui.painter().layout_no_wrap(label.to_string(), medium(12.0), t.text).size().x + 36.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
    paint_state(ui, r, radius::CAPSULE, &resp, on, t);
    let c = if on { t.accent } else { t.text_dim };
    glyphs::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 15.0, r.center().y), vec2(13.0, 13.0)), icon, c);
    ui.painter().text(pos2(r.min.x + 27.0, r.center().y), Align2::LEFT_CENTER, label, medium(12.0), if on { t.accent } else { t.text });
    resp
}

fn set_layout(db: &mut Database, i: usize, l: Layout) {
    let group =
        db.props.iter().find(|p| p.kind.groups() && p.kind != Kind::Checkbox).or_else(|| db.props.iter().find(|p| p.kind.groups())).map(|p| p.id);
    let date = db.props.iter().find(|p| p.kind == Kind::Date).map(|p| p.id);
    if let Some(v) = db.views.get_mut(i) {
        v.layout = l;
        if l == Layout::Board && v.group_by.is_none() {
            v.group_by = group;
        }
        if l == Layout::Calendar && v.date_prop.is_none() {
            v.date_prop = date;
        }
    }
    db.touch();
}

fn filter_menu(db: &mut Database, ui: &mut Ui, t: &Tokens, vi: usize) {
    let n = db.views.get(vi).map(|v| v.filters.len()).unwrap_or(0);
    let label = if n == 0 { "Filter".to_string() } else { format!("Filter · {n}") };
    let resp = tool(ui, t, "list", &label, n > 0);
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(420.0);
        let props: Vec<(u64, String)> = db.props.iter().map(|p| (p.id, p.name.clone())).collect();
        let mut remove = None;
        let filters = db.views.get(vi).map(|v| v.filters.clone()).unwrap_or_default();
        let mut changed = false;
        let mut new_filters = filters.clone();
        for (i, f) in filters.iter().enumerate() {
            ui.horizontal(|ui| {
                let Some(nf) = new_filters.get_mut(i) else { return };
                let pname = props.iter().find(|(id, _)| *id == f.prop).map(|(_, n)| n.clone()).unwrap_or_default();
                egui::ComboBox::from_id_salt(("glass_db_fprop", i)).selected_text(pname).width(130.0).show_ui(ui, |ui| {
                    for (id, n) in &props {
                        if ui.selectable_label(*id == f.prop, n).clicked() {
                            nf.prop = *id;
                            changed = true;
                        }
                    }
                });
                egui::ComboBox::from_id_salt(("glass_db_fop", i)).selected_text(f.op.name()).width(120.0).show_ui(ui, |ui| {
                    for op in Op::ALL {
                        if ui.selectable_label(op == f.op, op.name()).clicked() {
                            nf.op = op;
                            changed = true;
                        }
                    }
                });
                if f.op.takes_value() && ui.add(egui::TextEdit::singleline(&mut nf.value).desired_width(110.0)).changed() {
                    changed = true;
                }
                if ui.small_button("✕").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            new_filters.remove(i);
            changed = true;
        }
        if filters.is_empty() {
            ui.label(egui::RichText::new("No filters: every record shows.").font(regular(12.0)).color(t.text_dim));
        }
        if ui.button("+ Add a filter").clicked()
            && let Some(p) = db.props.first().map(|p| p.id)
        {
            new_filters.push(Filter { prop: p, op: Op::Contains, value: String::new() });
            changed = true;
        }
        if changed && let Some(v) = db.views.get_mut(vi) {
            v.filters = new_filters;
            db.touch();
        }
    });
}

fn sort_menu(db: &mut Database, ui: &mut Ui, t: &Tokens, vi: usize) {
    let n = db.views.get(vi).map(|v| v.sorts.len()).unwrap_or(0);
    let resp = tool(ui, t, "sort", if n == 0 { "Sort" } else { "Sorted" }, n > 0);
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(300.0);
        let props: Vec<(u64, String)> = db.props.iter().map(|p| (p.id, p.name.clone())).collect();
        let sorts = db.views.get(vi).map(|v| v.sorts.clone()).unwrap_or_default();
        let mut new = sorts.clone();
        let mut changed = false;
        let mut remove = None;
        for (i, s) in sorts.iter().enumerate() {
            ui.horizontal(|ui| {
                let Some(ns) = new.get_mut(i) else { return };
                let pname = props.iter().find(|(id, _)| *id == s.prop).map(|(_, n)| n.clone()).unwrap_or_default();
                egui::ComboBox::from_id_salt(("glass_db_sprop", i)).selected_text(pname).width(140.0).show_ui(ui, |ui| {
                    for (id, n) in &props {
                        if ui.selectable_label(*id == s.prop, n).clicked() {
                            ns.prop = *id;
                            changed = true;
                        }
                    }
                });
                if ui.selectable_label(!s.desc, "Ascending").clicked() {
                    ns.desc = false;
                    changed = true;
                }
                if ui.selectable_label(s.desc, "Descending").clicked() {
                    ns.desc = true;
                    changed = true;
                }
                if ui.small_button("✕").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            new.remove(i);
            changed = true;
        }
        if ui.button("+ Add a sort").clicked()
            && let Some(p) = db.props.first().map(|p| p.id)
        {
            new.push(Sort { prop: p, desc: false });
            changed = true;
        }
        if changed && let Some(v) = db.views.get_mut(vi) {
            v.sorts = new;
            db.touch();
        }
    });
}

fn group_menu(db: &mut Database, ui: &mut Ui, t: &Tokens, vi: usize) {
    let cur = db.views.get(vi).and_then(|v| v.group_by);
    let resp = tool(ui, t, "board", "Group", cur.is_some());
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(180.0);
        ui.label(egui::RichText::new("Group by").font(medium(11.0)).color(t.text_dim));
        let mut pick: Option<Option<u64>> = None;
        if ui.selectable_label(cur.is_none(), "None").clicked() {
            pick = Some(None);
        }
        for p in db.props.iter().filter(|p| p.kind.groups()) {
            if ui.selectable_label(cur == Some(p.id), &p.name).clicked() {
                pick = Some(Some(p.id));
            }
        }
        if let Some(p) = pick
            && let Some(v) = db.views.get_mut(vi)
        {
            v.group_by = p;
            db.touch();
        }
    });
}

fn date_menu(db: &mut Database, ui: &mut Ui, t: &Tokens, vi: usize) {
    let cur = db.views.get(vi).and_then(|v| v.date_prop);
    let resp = tool(ui, t, "calendar", "Dates", false);
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(180.0);
        ui.label(egui::RichText::new("Show by").font(medium(11.0)).color(t.text_dim));
        let mut pick = None;
        for p in db.props.iter().filter(|p| p.kind == Kind::Date) {
            if ui.selectable_label(cur == Some(p.id), &p.name).clicked() {
                pick = Some(p.id);
            }
        }
        if let Some(p) = pick
            && let Some(v) = db.views.get_mut(vi)
        {
            v.date_prop = Some(p);
            db.touch();
        }
    });
}

fn properties_menu(db: &mut Database, ui: &mut Ui, t: &Tokens, vi: usize) {
    let resp = tool(ui, t, "table", "Properties", false);
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(220.0);
        ui.label(egui::RichText::new("Shown in this view").font(medium(11.0)).color(t.text_dim));
        let hidden = db.views.get(vi).map(|v| v.hidden.clone()).unwrap_or_default();
        let mut toggle = None;
        for p in &db.props {
            if p.kind == Kind::Title {
                continue;
            }
            let mut on = !hidden.contains(&p.id);
            if ui.checkbox(&mut on, &p.name).changed() {
                toggle = Some(p.id);
            }
        }
        if let Some(id) = toggle
            && let Some(v) = db.views.get_mut(vi)
        {
            if let Some(i) = v.hidden.iter().position(|h| *h == id) {
                v.hidden.remove(i);
            } else {
                v.hidden.push(id);
            }
            db.touch();
        }
    });
}

fn more_menu(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, vi: usize, actions: &mut Vec<Action>) {
    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    paint_state(ui, r, radius::CAPSULE, &resp, false, t);
    for dx in [-5.0, 0.0, 5.0] {
        ui.painter().circle_filled(pos2(r.center().x + dx, r.center().y), 1.7, t.text_dim);
    }
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(230.0);
        let shown = || -> Vec<u64> { db.views.get(vi).map(|v| db.visible(v, &st.search)).unwrap_or_default() };
        if icon_row(ui, t, "folder", "Import CSV, spreadsheet or contacts…").clicked() {
            actions.push(Action::Import);
            ui.close();
        }
        if icon_row(ui, t, "doc", "Export as CSV…").clicked() {
            actions.push(Action::ExportCsv(shown()));
            ui.close();
        }
        if icon_row(ui, t, "people", "Use for Mail Merge").on_hover_text("The records shown become the recipients of a mail merge").clicked() {
            actions.push(Action::MailMerge(shown()));
            ui.close();
        }
    });
}

/// A new record in view `vi`, given the view's group value (Board column) or date (Calendar).
fn new_record(db: &mut Database, vi: usize, group: Option<(u64, Option<String>)>) -> u64 {
    let id = db.add_row();
    if id == 0 {
        return 0;
    }
    // Filters that say "is X" fill the new record so it shows in the view.
    if let Some(v) = db.views.get(vi).cloned() {
        for f in v.filters.iter().filter(|f| f.op == Op::Is && !f.value.is_empty()) {
            db.set_text(id, f.prop, &f.value);
        }
    }
    if let Some((prop, value)) = group {
        db.move_to_group(id, prop, value.as_deref());
    }
    id
}

// ---------------------------------------------------------------------------------------------
// Cells.

/// An editable cell in `r`.
fn cell(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, row: u64, prop: u64, r: Rect) {
    let Some(kind) = db.prop(prop).map(|p| p.kind) else { return };
    let id = ui.id().with(("glass_db_cell", row, prop));
    let editing = st.editing.as_ref().is_some_and(|(r2, p2, _, _)| *r2 == row && *p2 == prop);
    if editing && !kind.has_options() && !matches!(kind, Kind::Checkbox | Kind::Rating) {
        let Some((_, _, text, focused)) = st.editing.as_mut() else { return };
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(2.0, 3.0))));
        let te = egui::TextEdit::singleline(text).id(id.with("edit")).desired_width(r.width() - 8.0).margin(vec2(6.0, 4.0)).font(regular(12.5));
        let resp = child.add(te);
        if !*focused {
            resp.request_focus();
            *focused = true;
        }
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if escape {
            st.editing = None;
        } else if resp.lost_focus() || enter {
            let text = text.clone();
            db.set_text(row, prop, &text);
            st.editing = None;
        }
        return;
    }
    let resp = ui.interact(r, id, Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r.shrink(1.0), 4.0, t.hover);
    }
    draw_value(ui, db, row, prop, kind, r, t);
    match kind {
        Kind::Checkbox => {
            if resp.clicked() {
                let on = matches!(db.cell(row, prop), Some(Cell::Bool(true)));
                db.set(row, prop, Some(Cell::Bool(!on)));
            }
        }
        Kind::Rating => {
            if resp.clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                let n = (((p.x - r.min.x - 8.0) / 16.0).floor() as i32 + 1).clamp(0, 5);
                let cur = match db.cell(row, prop) {
                    Some(Cell::Number(x)) => *x as i32,
                    _ => 0,
                };
                let n = if n == cur { 0 } else { n };
                db.set(row, prop, if n == 0 { None } else { Some(Cell::Number(f64::from(n))) });
            }
        }
        Kind::Select | Kind::Status | Kind::MultiSelect => {
            egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| options_menu(db, st, ui, t, row, prop));
        }
        Kind::Date => {
            if resp.clicked() {
                st.editing = Some((row, prop, db.text(row, prop), false));
            }
            if resp.secondary_clicked() {
                db.set(row, prop, Some(Cell::Text(crate::db::today())));
            }
        }
        _ => {
            if resp.clicked() {
                st.editing = Some((row, prop, db.text(row, prop), false));
            }
        }
    }
}

/// The options of a select, to pick (or toggle, for a Multi-select), recolour or create.
fn options_menu(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, row: u64, prop: u64) {
    ui.set_min_width(230.0);
    let Some(p) = db.prop(prop).cloned() else { return };
    let current: Vec<String> = match db.cell(row, prop) {
        Some(Cell::Options(o)) => o.clone(),
        _ => Vec::new(),
    };
    let te = egui::TextEdit::singleline(&mut st.new_option).hint_text("Search or create an option").desired_width(220.0);
    let resp = ui.add(te);
    let q = st.new_option.trim().to_lowercase();
    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !q.is_empty() {
        let name = st.new_option.trim().to_string();
        db.toggle_option(row, prop, &name);
        st.new_option.clear();
        if p.kind != Kind::MultiSelect {
            ui.close();
        }
    }
    ui.add_space(4.0);
    let mut recolor: Option<(usize, Tint)> = None;
    for (i, o) in p.options.iter().enumerate().filter(|(_, o)| q.is_empty() || o.name.to_lowercase().contains(&q)) {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
        paint_state(ui, r, 6, &resp, current.contains(&o.name), t);
        chip(ui, pos2(r.min.x + 8.0, r.center().y), &o.name, o.tint, t, r.width() - 40.0);
        if current.contains(&o.name) {
            ui.painter().text(pos2(r.max.x - 12.0, r.center().y), Align2::CENTER_CENTER, "✓", medium(12.0), t.accent);
        }
        if resp.clicked() {
            db.toggle_option(row, prop, &o.name);
            if p.kind != Kind::MultiSelect {
                ui.close();
            }
        }
        resp.context_menu(|ui| {
            ui.label(egui::RichText::new("Colour").font(medium(11.0)).color(t.text_dim));
            for tint in Tint::ALL {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    ui.painter().rect_filled(r, 4.0, c32(tint.strong()));
                    if ui.selectable_label(tint == o.tint, tint.name()).clicked() {
                        recolor = Some((i, tint));
                    }
                });
            }
        });
    }
    if !q.is_empty() && !p.options.iter().any(|o| o.name.to_lowercase() == q) {
        let label = format!("Create “{}”", st.new_option.trim());
        if ui.button(label).clicked() {
            let name = st.new_option.trim().to_string();
            db.toggle_option(row, prop, &name);
            st.new_option.clear();
        }
    }
    if let Some((i, tint)) = recolor
        && let Some(o) = db.prop_mut(prop).and_then(|p| p.options.get_mut(i))
    {
        o.tint = tint;
        db.touch();
    }
}

// ---------------------------------------------------------------------------------------------
// Table.

fn table(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, view: &View, ids: &[u64], actions: &mut Vec<Action>) {
    let cols: Vec<(u64, f32, Kind, String)> =
        db.props.iter().filter(|p| !view.hidden.contains(&p.id)).map(|p| (p.id, p.width, p.kind, p.name.clone())).collect();
    let total: f32 = cols.iter().map(|c| c.1).sum::<f32>() + 44.0 + 40.0;
    let row_h = 36.0;
    egui::ScrollArea::horizontal().id_salt("glass_db_table_h").auto_shrink([false, true]).show(ui, |ui| {
        let width = total.max(ui.available_width());
        // Header.
        let (hr, _) = ui.allocate_exact_size(vec2(width, 34.0), Sense::hover());
        let mut x = hr.min.x + 24.0;
        let mut resize: Option<(u64, f32)> = None;
        for (pid, w, kind, name) in &cols {
            let r = Rect::from_min_size(pos2(x, hr.min.y), vec2(*w, hr.height()));
            let resp = ui.interact(r, ui.id().with(("glass_db_head", pid)), Sense::click());
            paint_state(ui, r.shrink(2.0), 6, &resp, false, t);
            glyphs::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 14.0, r.center().y), vec2(12.0, 12.0)), kind_icon(*kind), t.text_dim);
            ui.painter().with_clip_rect(r.shrink(2.0)).text(pos2(r.min.x + 26.0, r.center().y), Align2::LEFT_CENTER, name, medium(12.0), t.text_dim);
            egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| prop_menu(db, st, ui, t, *pid));
            // Drag the right edge to resize.
            let edge = Rect::from_min_max(pos2(r.max.x - 3.0, r.min.y), pos2(r.max.x + 3.0, r.max.y));
            let er = ui.interact(edge, ui.id().with(("glass_db_resize", pid)), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if er.dragged() {
                resize = Some((*pid, (*w + er.drag_delta().x).clamp(60.0, 900.0)));
            }
            if er.hovered() || er.dragged() {
                ui.painter().vline(r.max.x, r.y_range(), Stroke::new(2.0, t.accent));
            }
            x += w;
        }
        // Add a property.
        let r = Rect::from_min_size(pos2(x, hr.min.y), vec2(40.0, hr.height()));
        let resp = ui.interact(r, ui.id().with("glass_db_addprop"), Sense::click()).on_hover_text("Add a property");
        paint_state(ui, r.shrink(2.0), 6, &resp, false, t);
        plus(ui.painter(), r.center(), t.text_dim);
        egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| add_prop_menu(db, st, ui, t));
        if let Some((pid, w)) = resize
            && let Some(p) = db.prop_mut(pid)
        {
            p.width = w;
            db.touch();
        }
        ui.painter().hline(hr.x_range(), hr.max.y, Stroke::new(1.0, t.edge));
        // Rows, grouped when the view groups.
        let groups = match view.group_by {
            Some(g) => db.groups(ids, g).into_iter().filter(|(_, v)| !v.is_empty()).collect(),
            None => vec![(None, ids.to_vec())],
        };
        let mut drop: Option<(u64, Option<u64>)> = None;
        for (gname, gids) in &groups {
            if view.group_by.is_some() {
                ui.add_space(10.0);
                let (r, _) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
                let tint = view.group_by.zip(gname.as_ref()).map(|(p, n)| db.option_tint(p, n)).unwrap_or_default();
                let w = chip(ui, pos2(r.min.x + 24.0, r.center().y), gname.as_deref().unwrap_or("No value"), tint, t, 240.0);
                ui.painter().text(pos2(r.min.x + 32.0 + w, r.center().y), Align2::LEFT_CENTER, gids.len().to_string(), regular(12.0), t.text_dim);
            }
            for rid in gids {
                let (rr, rresp) = ui.allocate_exact_size(vec2(width, row_h), Sense::hover());
                let hovered = ui.rect_contains_pointer(rr);
                if st.peek == Some(*rid) {
                    ui.painter().rect_filled(rr, 6.0, t.accent_soft);
                }
                // The grip moves the record (manual order shows when no sort is set).
                let grip = Rect::from_min_size(rr.min, vec2(22.0, row_h));
                let gresp = ui.interact(grip, ui.id().with(("glass_db_grip", rid)), Sense::drag());
                if hovered || gresp.dragged() {
                    for (dx, dy) in [(-3.0, -5.0), (3.0, -5.0), (-3.0, 0.0), (3.0, 0.0), (-3.0, 5.0), (3.0, 5.0)] {
                        ui.painter().circle_filled(pos2(grip.center().x + dx, grip.center().y + dy), 1.3, t.text_dim);
                    }
                }
                gresp.dnd_set_drag_payload(*rid);
                if let Some(p) = rresp.dnd_hover_payload::<u64>()
                    && *p != *rid
                {
                    ui.painter().hline(rr.x_range(), rr.min.y, Stroke::new(2.0, t.accent));
                }
                if let Some(p) = rresp.dnd_release_payload::<u64>() {
                    drop = Some((*p, Some(*rid)));
                }
                let mut x = rr.min.x + 24.0;
                for (pid, w, kind, _) in &cols {
                    let r = Rect::from_min_size(pos2(x, rr.min.y), vec2(*w, row_h));
                    cell(db, st, ui, t, *rid, *pid, r);
                    // "Open" on the title cell.
                    if *kind == Kind::Title && hovered && st.editing.as_ref().is_none_or(|e| e.0 != *rid) {
                        let b = Rect::from_min_size(pos2(r.max.x - 62.0, r.center().y - 11.0), vec2(56.0, 22.0));
                        let bresp = ui.interact(b, ui.id().with(("glass_db_open", rid)), Sense::click());
                        ui.painter().rect_filled(b, 6.0, if bresp.hovered() { t.pill } else { t.glass });
                        ui.painter().rect_stroke(b, 6.0, Stroke::new(0.75, t.edge), egui::StrokeKind::Inside);
                        ui.painter().text(b.center(), Align2::CENTER_CENTER, "Open", medium(11.5), t.text);
                        if bresp.clicked() {
                            st.peek = Some(*rid);
                        }
                    }
                    ui.painter().vline(r.max.x, r.y_range(), Stroke::new(0.5, t.edge.gamma_multiply(0.6)));
                    x += w;
                }
                rresp.context_menu(|ui| row_menu(db, st, ui, *rid, actions));
                ui.painter().hline(rr.x_range(), rr.max.y, Stroke::new(0.5, t.edge));
            }
        }
        if let Some((moved, before)) = drop
            && view.sorts.is_empty()
        {
            db.move_row(moved, before);
        }
        // New.
        let (r, resp) = ui.allocate_exact_size(vec2(width, row_h), Sense::click());
        paint_state(ui, r, 6, &resp, false, t);
        plus(ui.painter(), pos2(r.min.x + 36.0, r.center().y), t.text_dim);
        ui.painter().text(pos2(r.min.x + 50.0, r.center().y), Align2::LEFT_CENTER, "New", regular(12.5), t.text_dim);
        if resp.clicked() {
            let vi = db_current(db);
            let id = new_record(db, vi, None);
            if let Some(tp) = db.title_prop() {
                st.editing = Some((id, tp, String::new(), false));
            }
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new(format!("{} of {} records", ids.len(), db.rows.len())).font(regular(11.5)).color(t.text_dim));
    });
}

fn row_menu(db: &mut Database, st: &mut DbUi, ui: &mut Ui, rid: u64, actions: &mut Vec<Action>) {
    ui.set_min_width(170.0);
    if ui.button("Open in side panel").clicked() {
        st.peek = Some(rid);
        ui.close();
    }
    if ui.button("Open as page").clicked() {
        actions.push(Action::OpenPage(rid));
        ui.close();
    }
    if ui.button("Duplicate").clicked() {
        db.duplicate_row(rid);
        ui.close();
    }
    if ui.button("Delete").clicked() {
        db.delete_row(rid);
        if st.peek == Some(rid) {
            st.peek = None;
        }
        ui.close();
    }
}

/// A column header's menu: rename, change the type, sort, hide, delete, and edit options.
fn prop_menu(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, pid: u64) {
    ui.set_min_width(220.0);
    let Some(p) = db.prop(pid).cloned() else { return };
    let mut name = st.rename.as_ref().filter(|(id, _)| *id == pid).map(|(_, n)| n.clone()).unwrap_or_else(|| p.name.clone());
    if ui.add(egui::TextEdit::singleline(&mut name).desired_width(210.0)).changed() {
        st.rename = Some((pid, name.clone()));
        if let Some(p) = db.prop_mut(pid) {
            p.name = name;
        }
        db.touch();
    }
    ui.add_space(4.0);
    if p.kind != Kind::Title {
        ui.menu_button(format!("Type: {}", p.kind.name()), |ui| {
            for k in Kind::PICKABLE {
                if icon_row(ui, t, kind_icon(k), k.name()).clicked() {
                    db.set_kind(pid, k);
                    ui.close();
                }
            }
        });
    }
    let vi = db_current(db);
    if ui.button("Sort ascending").clicked() {
        if let Some(v) = db.views.get_mut(vi) {
            v.sorts = vec![Sort { prop: pid, desc: false }];
        }
        db.touch();
        ui.close();
    }
    if ui.button("Sort descending").clicked() {
        if let Some(v) = db.views.get_mut(vi) {
            v.sorts = vec![Sort { prop: pid, desc: true }];
        }
        db.touch();
        ui.close();
    }
    if p.kind != Kind::Title && ui.button("Hide in this view").clicked() {
        if let Some(v) = db.views.get_mut(vi) {
            v.hidden.push(pid);
        }
        db.touch();
        ui.close();
    }
    if p.kind != Kind::Title && ui.button("Delete property").clicked() {
        db.delete_prop(pid);
        ui.close();
    }
}

fn add_prop_menu(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens) {
    ui.set_min_width(220.0);
    ui.add(egui::TextEdit::singleline(&mut st.new_prop).hint_text("Property name").desired_width(210.0));
    ui.add_space(4.0);
    for k in Kind::PICKABLE {
        if icon_row(ui, t, kind_icon(k), k.name()).clicked() {
            let name = if st.new_prop.trim().is_empty() { k.name().to_string() } else { st.new_prop.trim().to_string() };
            db.add_prop(&name, k);
            st.new_prop.clear();
            st.new_kind = k;
            ui.close();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Board, Gallery, List.

/// The properties a card shows (not the title, not hidden, with a value).
fn card_lines(db: &Database, view: &View, rid: u64, skip: Option<u64>) -> Vec<(u64, Kind)> {
    db.props
        .iter()
        .filter(|p| p.kind != Kind::Title && Some(p.id) != skip && !view.hidden.contains(&p.id))
        .filter(|p| db.cell(rid, p.id).is_some_and(|c| !c.is_empty()))
        .map(|p| (p.id, p.kind))
        .take(4)
        .collect()
}

/// A record card `w` wide at the cursor; returns its response.
fn card(db: &mut Database, st: &DbUi, ui: &mut Ui, t: &Tokens, view: &View, rid: u64, w: f32, skip: Option<u64>, cover: bool) -> egui::Response {
    let lines = card_lines(db, view, rid, skip);
    let cover_h = if cover { 72.0 } else { 0.0 };
    let h = cover_h + 40.0 + lines.len() as f32 * 24.0 + 8.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
    let lift = ui.ctx().animate_bool_with_time(resp.id.with("lift"), resp.hovered(), 0.12);
    let shadow = egui::epaint::Shadow { offset: [0, (1.0 + 3.0 * lift) as i8], blur: (6.0 + 8.0 * lift) as u8, spread: 0, color: t.shadow };
    ui.painter().add(shadow.as_shape(r, CornerRadius::same(12)));
    ui.painter().rect_filled(r, 12.0, if t.dark { Color32::from_gray(44) } else { Color32::WHITE });
    if st.peek == Some(rid) {
        ui.painter().rect_stroke(r, 12.0, Stroke::new(2.0, t.accent), egui::StrokeKind::Outside);
    } else {
        ui.painter().rect_stroke(r, 12.0, Stroke::new(0.75, t.edge), egui::StrokeKind::Inside);
    }
    if cover {
        let cr = Rect::from_min_size(r.min, vec2(w, cover_h));
        let tint = first_tint(db, rid);
        // A soft cover in the record's colour, rounded with the card, with a lighter sweep.
        ui.painter().rect_filled(cr, egui::CornerRadius { nw: 12, ne: 12, sw: 0, se: 0 }, c32(tint.soft()));
        let sweep = Rect::from_min_max(pos2(cr.min.x + cr.width() * 0.45, cr.min.y), cr.max);
        ui.painter().rect_filled(sweep, egui::CornerRadius { nw: 0, ne: 12, sw: 0, se: 0 }, c32(tint.strong()).gamma_multiply(0.18));
        let initial: String = db.title(rid).chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
        ui.painter().text(pos2(cr.min.x + 16.0, cr.max.y - 8.0), Align2::LEFT_BOTTOM, initial, semibold(26.0), c32(tint.strong()));
    }
    let title_y = r.min.y + cover_h + 20.0;
    ui.painter().with_clip_rect(r.shrink(8.0)).text(pos2(r.min.x + 14.0, title_y), Align2::LEFT_CENTER, db.title(rid), semibold(13.0), t.text);
    let mut y = title_y + 26.0;
    for (pid, kind) in lines {
        let lr = Rect::from_min_size(pos2(r.min.x + 6.0, y - 12.0), vec2(w - 12.0, 24.0));
        draw_value(ui, db, rid, pid, kind, lr, t);
        y += 24.0;
    }
    resp
}

/// The colour of a record's first select-like value (for card covers and calendar chips).
fn first_tint(db: &Database, rid: u64) -> Tint {
    for p in db.props.iter().filter(|p| p.kind.has_options()) {
        if let Some(Cell::Options(o)) = db.cell(rid, p.id)
            && let Some(n) = o.first()
        {
            return db.option_tint(p.id, n);
        }
    }
    Tint::nth(rid as usize)
}

fn board(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, vi: usize, view: &View, ids: &[u64]) {
    let Some(gp) = view.group_by.filter(|g| db.prop(*g).is_some()) else {
        ui.add_space(20.0);
        ui.label(
            egui::RichText::new("Group this board by a Select, Status or Checkbox property (Group, above).").font(regular(13.0)).color(t.text_dim),
        );
        if ui.button("Add a Status property").clicked() {
            let p = db.add_prop("Status", Kind::Status);
            if let Some(v) = db.views.get_mut(vi) {
                v.group_by = Some(p);
            }
        }
        return;
    };
    let groups = db.groups(ids, gp);
    let col_w = 268.0;
    let mut moved: Option<(u64, Option<String>)> = None;
    let mut create: Option<Option<String>> = None;
    egui::ScrollArea::horizontal().id_salt("glass_db_board").auto_shrink([false, true]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(14.0, 0.0);
            for (gname, gids) in &groups {
                if gname.is_none() && gids.is_empty() {
                    continue;
                }
                let col = ui.vertical(|ui| {
                    ui.set_width(col_w);
                    let (hr, _) = ui.allocate_exact_size(vec2(col_w, 30.0), Sense::hover());
                    let tint = gname.as_ref().map(|n| db.option_tint(gp, n)).unwrap_or_default();
                    let w = chip(ui, pos2(hr.min.x + 4.0, hr.center().y), gname.as_deref().unwrap_or("No value"), tint, t, 200.0);
                    ui.painter().text(
                        pos2(hr.min.x + 12.0 + w, hr.center().y),
                        Align2::LEFT_CENTER,
                        gids.len().to_string(),
                        regular(12.0),
                        t.text_dim,
                    );
                    ui.add_space(6.0);
                    ui.spacing_mut().item_spacing = vec2(0.0, 8.0);
                    for rid in gids {
                        let resp = card(db, st, ui, t, view, *rid, col_w, Some(gp), false);
                        resp.dnd_set_drag_payload(*rid);
                        if resp.clicked() {
                            st.peek = Some(*rid);
                        }
                    }
                    let (r, resp) = ui.allocate_exact_size(vec2(col_w, 32.0), Sense::click());
                    paint_state(ui, r, 8, &resp, false, t);
                    plus(ui.painter(), pos2(r.min.x + 16.0, r.center().y), t.text_dim);
                    ui.painter().text(pos2(r.min.x + 30.0, r.center().y), Align2::LEFT_CENTER, "New", regular(12.5), t.text_dim);
                    if resp.clicked() {
                        create = Some(gname.clone());
                    }
                    ui.add_space(30.0);
                });
                // The whole column takes drops.
                let zone = col.response.rect.expand(4.0);
                let zresp = ui.interact(zone, ui.id().with(("glass_db_col", gname.clone())), Sense::hover());
                if zresp.dnd_hover_payload::<u64>().is_some() {
                    ui.painter().rect_filled(zone, 14.0, t.accent_soft);
                }
                if let Some(p) = zresp.dnd_release_payload::<u64>() {
                    moved = Some((*p, gname.clone()));
                }
                let bg = ui.painter().add(egui::Shape::Noop);
                ui.painter().set(bg, egui::Shape::rect_filled(zone, 14.0, t.fill.gamma_multiply(0.5)));
            }
        });
    });
    if let Some((rid, g)) = moved {
        db.move_to_group(rid, gp, g.as_deref());
    }
    if let Some(g) = create {
        let id = new_record(db, vi, Some((gp, g)));
        st.peek = Some(id);
    }
}

fn gallery(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, view: &View, ids: &[u64]) {
    let w = 230.0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(16.0, 16.0);
        for rid in ids {
            ui.vertical(|ui| {
                ui.set_width(w);
                if card(db, st, ui, t, view, *rid, w, None, true).clicked() {
                    st.peek = Some(*rid);
                }
            });
        }
    });
    if ids.is_empty() {
        empty(ui, t);
    }
}

fn list(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, view: &View, ids: &[u64]) {
    for rid in ids {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
        paint_state(ui, r, 8, &resp, st.peek == Some(*rid), t);
        glyphs::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 18.0, r.center().y), vec2(15.0, 15.0)), "doc", t.text_dim);
        ui.painter().text(pos2(r.min.x + 36.0, r.center().y), Align2::LEFT_CENTER, db.title(*rid), medium(13.0), t.text);
        // Properties, right-aligned.
        let mut x = r.max.x - 10.0;
        for (pid, kind) in card_lines(db, view, *rid, None).into_iter().rev() {
            let w = if kind.has_options() { 120.0 } else { 130.0 };
            let cr = Rect::from_min_size(pos2(x - w, r.min.y), vec2(w, r.height()));
            if cr.min.x < r.min.x + 260.0 {
                break;
            }
            draw_value(ui, db, *rid, pid, kind, cr, t);
            x -= w + 6.0;
        }
        if resp.clicked() {
            st.peek = Some(*rid);
        }
        ui.painter().hline(r.x_range(), r.max.y, Stroke::new(0.5, t.edge));
    }
    if ids.is_empty() {
        empty(ui, t);
    }
}

fn empty(ui: &mut Ui, t: &Tokens) {
    ui.add_space(24.0);
    ui.label(egui::RichText::new("No records here yet. Click New to add one.").font(regular(13.0)).color(t.text_dim));
}

// ---------------------------------------------------------------------------------------------
// Calendar.

fn calendar(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, vi: usize, view: &View, ids: &[u64]) {
    let Some(dp) = view.date_prop.filter(|d| db.prop(*d).is_some()) else {
        ui.add_space(20.0);
        ui.label(egui::RichText::new("A calendar needs a Date property.").font(regular(13.0)).color(t.text_dim));
        if ui.button("Add a Date property").clicked() {
            let p = db.add_prop("Date", Kind::Date);
            if let Some(v) = db.views.get_mut(vi) {
                v.date_prop = Some(p);
            }
        }
        return;
    };
    let today = crate::db::today();
    let ty: i32 = today.get(..4).and_then(|s| s.parse().ok()).unwrap_or(2026);
    let tm: u32 = today.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(1);
    let (y, m) = *st.month.get_or_insert((ty, tm));
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{} {y}", MONTH_NAMES.get((m.max(1) - 1) as usize).copied().unwrap_or("")))
                .font(semibold(17.0))
                .color(t.text),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::widgets::icon_capsule(ui, t, "chevronRight", "Next month", 28.0, false, true).clicked() {
                st.month = Some(if m == 12 { (y + 1, 1) } else { (y, m + 1) });
            }
            if crate::widgets::quiet_button(ui, t, "Today", Some(64.0), 28.0).clicked() {
                st.month = Some((ty, tm));
            }
            if crate::widgets::icon_capsule(ui, t, "chevronLeft", "Previous month", 28.0, false, true).clicked() {
                st.month = Some(if m == 1 { (y - 1, 12) } else { (y, m - 1) });
            }
        });
    });
    ui.add_space(8.0);
    let w = ui.available_width();
    let col = w / 7.0;
    let (hr, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    for (i, d) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].iter().enumerate() {
        ui.painter().text(pos2(hr.min.x + col * i as f32 + 8.0, hr.center().y), Align2::LEFT_CENTER, *d, medium(11.5), t.text_dim);
    }
    let first = crate::db::weekday(y, m, 1) as i32;
    let days = crate::db::days_in_month(y, m) as i32;
    let weeks = ((first + days + 6) / 7).max(4);
    let cell_h = 112.0;
    let (gr, _) = ui.allocate_exact_size(vec2(w, cell_h * weeks as f32), Sense::hover());
    let mut by_day: std::collections::BTreeMap<String, Vec<u64>> = Default::default();
    for rid in ids {
        let d = db.text(*rid, dp);
        if !d.is_empty() {
            by_day.entry(d).or_default().push(*rid);
        }
    }
    let mut moved: Option<(u64, String)> = None;
    let mut create: Option<String> = None;
    for k in 0..weeks * 7 {
        let day = k - first + 1;
        let r = Rect::from_min_size(pos2(gr.min.x + col * (k % 7) as f32, gr.min.y + cell_h * (k / 7) as f32), vec2(col, cell_h));
        ui.painter().rect_stroke(r, 0.0, Stroke::new(0.5, t.edge), egui::StrokeKind::Inside);
        if day < 1 || day > days {
            ui.painter().rect_filled(r.shrink(0.5), 0.0, t.fill.gamma_multiply(0.4));
            continue;
        }
        let iso = format!("{y:04}-{m:02}-{day:02}");
        let resp = ui.interact(r, ui.id().with(("glass_db_day", k)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r.shrink(1.0), 4.0, t.hover);
        }
        if resp.dnd_hover_payload::<u64>().is_some() {
            ui.painter().rect_filled(r.shrink(1.0), 4.0, t.accent_soft);
        }
        if let Some(p) = resp.dnd_release_payload::<u64>() {
            moved = Some((*p, iso.clone()));
        }
        if resp.double_clicked() {
            create = Some(iso.clone());
        }
        let is_today = iso == today;
        let num = Rect::from_center_size(pos2(r.max.x - 16.0, r.min.y + 15.0), vec2(22.0, 22.0));
        if is_today {
            ui.painter().circle_filled(num.center(), 11.0, t.accent);
        }
        ui.painter().text(num.center(), Align2::CENTER_CENTER, day.to_string(), medium(12.0), if is_today { t.on_accent } else { t.text_dim });
        let mut cy = r.min.y + 32.0;
        let entries = by_day.get(&iso).cloned().unwrap_or_default();
        let n = entries.len();
        for (i, rid) in entries.into_iter().enumerate() {
            if cy + 22.0 > r.max.y && i + 1 < n {
                ui.painter().text(pos2(r.min.x + 8.0, cy + 8.0), Align2::LEFT_CENTER, format!("+{} more", n - i), regular(11.0), t.text_dim);
                break;
            }
            let er = Rect::from_min_size(pos2(r.min.x + 4.0, cy), vec2(col - 8.0, 20.0));
            let eresp = ui.interact(er, ui.id().with(("glass_db_event", rid)), Sense::click_and_drag());
            eresp.dnd_set_drag_payload(rid);
            let (bg, fg) = chip_colors(first_tint(db, rid), t);
            ui.painter().rect_filled(er, 5.0, if eresp.hovered() { bg.gamma_multiply(0.85) } else { bg });
            ui.painter().with_clip_rect(er.shrink(3.0)).text(
                pos2(er.min.x + 6.0, er.center().y),
                Align2::LEFT_CENTER,
                db.title(rid),
                medium(11.5),
                fg,
            );
            if eresp.clicked() {
                st.peek = Some(rid);
            }
            cy += 23.0;
        }
    }
    if let Some((rid, iso)) = moved {
        db.set(rid, dp, Some(Cell::Text(iso)));
    }
    if let Some(iso) = create {
        let id = new_record(db, vi, None);
        db.set(id, dp, Some(Cell::Text(iso)));
        st.peek = Some(id);
    }
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Double-click a day to add a record; drag records between days.").font(regular(11.5)).color(t.text_dim));
}

// ---------------------------------------------------------------------------------------------
// The side panel.

fn peek(db: &mut Database, st: &mut DbUi, ui: &mut Ui, t: &Tokens, r: Rect, actions: &mut Vec<Action>) {
    let Some(rid) = st.peek.filter(|id| db.row(*id).is_some()) else {
        st.peek = None;
        return;
    };
    let card = r.shrink2(vec2(10.0, 10.0));
    ui.painter().add(crate::tokens::glass_shape(card, radius::PANEL, t));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(card.shrink(18.0)).layout(egui::Layout::top_down(egui::Align::Min)));
    let ui = &mut child;
    ui.horizontal(|ui| {
        if crate::widgets::accent_button(ui, t, None, "Open as page", Some(120.0), 28.0).clicked() {
            actions.push(Action::OpenPage(rid));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::widgets::icon_capsule(ui, t, "close", "Close", 28.0, false, true).clicked() {
                st.peek = None;
            }
            let (b, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
            paint_state(ui, b, radius::CAPSULE, &resp, false, t);
            for dx in [-5.0, 0.0, 5.0] {
                ui.painter().circle_filled(pos2(b.center().x + dx, b.center().y), 1.7, t.text_dim);
            }
            egui::Popup::menu(&resp).show(|ui| row_menu(db, st, ui, rid, actions));
        });
    });
    ui.add_space(14.0);
    egui::ScrollArea::vertical().id_salt("glass_db_peek").auto_shrink([false, false]).show(ui, |ui| {
        if let Some(tp) = db.title_prop() {
            let mut title = db.text(rid, tp);
            let te = egui::TextEdit::multiline(&mut title)
                .font(semibold(22.0))
                .frame(egui::Frame::NONE)
                .desired_rows(1)
                .desired_width(ui.available_width())
                .hint_text("Untitled");
            if ui.add(te).changed() {
                db.set_text(rid, tp, &title);
            }
        }
        ui.add_space(10.0);
        let props: Vec<(u64, Kind, String)> = db.props.iter().filter(|p| p.kind != Kind::Title).map(|p| (p.id, p.kind, p.name.clone())).collect();
        for (pid, kind, name) in props {
            let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
            glyphs::paint(ui.painter(), Rect::from_center_size(pos2(row.min.x + 8.0, row.center().y), vec2(12.0, 12.0)), kind_icon(kind), t.text_dim);
            ui.painter().with_clip_rect(Rect::from_min_max(row.min, pos2(row.min.x + 128.0, row.max.y))).text(
                pos2(row.min.x + 22.0, row.center().y),
                Align2::LEFT_CENTER,
                &name,
                regular(12.5),
                t.text_dim,
            );
            let vr = Rect::from_min_max(pos2(row.min.x + 132.0, row.min.y), row.max);
            cell(db, st, ui, t, rid, pid, vr);
        }
        ui.add_space(12.0);
        let created = db.row(rid).map(|r| r.created.clone()).unwrap_or_default();
        if !created.is_empty() {
            ui.label(egui::RichText::new(format!("Created {}", pretty_date(&created))).font(regular(11.5)).color(t.text_dim));
        }
        if db.row(rid).and_then(|r| r.page.clone()).is_some() {
            ui.label(egui::RichText::new("Has a page").font(regular(11.5)).color(t.text_dim));
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn dates_read_nicely() {
        assert_eq!(super::pretty_date("2026-10-02"), "2 Oct 2026");
        assert_eq!(super::pretty_date("nonsense"), "nonsense");
        assert_eq!(super::pretty_date("2026-13-01"), "1  2026");
    }
}

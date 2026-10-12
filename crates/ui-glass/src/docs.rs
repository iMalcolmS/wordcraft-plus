//! Document tabs, as in Notion or Safari: several documents open in one window, a Home tab to
//! start from, and a tab strip to move between them.
//!
//! The editor works on one session at a time, so the open tab's document lives in the app and the
//! others wait in their tabs; switching swaps them (with their scroll position and canvas state).
//! Each document keeps its own undo history, selection and versions.

use egui::{Align2, Rect, Sense, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_engine::Session;
use wordcraft_ui_egui::canvas::CanvasState;
use wordcraft_ui_egui::chrome::MAC_CONTENT_LEFT;
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::tokens::{Tokens, paint_pill, paint_state, radius};
use crate::{GlassApp, View};

/// A tab: the document waiting in it (`None` for the open tab, whose document is in the app).
pub struct DocTab {
    pub uid: u64,
    stash: Option<(Session, CanvasState)>,
    /// A database tab's database (its session is an unused blank document).
    pub db: Option<Box<crate::db::Database>>,
}

#[derive(Default)]
pub struct Docs {
    pub tabs: Vec<DocTab>,
    pub active: usize,
    next_uid: u64,
    /// A tab waiting for "Close without saving?" to be answered.
    pub confirm_close: Option<usize>,
    /// When the open document last changed (for the fade-in).
    pub switched_at: f64,
    /// The tab under the pointer and since when (its preview shows after a moment).
    hover: Option<(u64, f64)>,
}

impl Docs {
    /// The document waiting in tab `i` (not the open one).
    pub fn stash_mut(&mut self, i: usize) -> Option<&mut Session> {
        self.tabs.get_mut(i).and_then(|t| t.stash.as_mut()).map(|(s, _)| s)
    }
}

/// The file's name without its extension, else the document's title, else "Untitled".
fn stem(s: &Session) -> String {
    if let Some(n) = s.path.as_ref().and_then(|p| p.file_stem()) {
        return n.to_string_lossy().to_string();
    }
    let title = s.doc.core.title.trim();
    if title.is_empty() { "Untitled".into() } else { title.to_string() }
}

impl GlassApp {
    pub(crate) fn ensure_tabs(&mut self) {
        if self.docs.tabs.is_empty() {
            self.docs.next_uid += 1;
            self.docs.tabs.push(DocTab { uid: self.docs.next_uid, stash: None, db: None });
            self.docs.active = 0;
        }
    }

    /// The document in tab `i`.
    pub fn tab_session(&self, i: usize) -> Option<&Session> {
        if i == self.docs.active { Some(&self.app.session) } else { self.docs.tabs.get(i).and_then(|t| t.stash.as_ref()).map(|(s, _)| s) }
    }

    pub fn tab_title(&self, i: usize) -> String {
        if let Some(db) = self.docs.tabs.get(i).and_then(|t| t.db.as_ref()) {
            return if db.name.trim().is_empty() { "Untitled database".into() } else { db.name.clone() };
        }
        self.tab_session(i).map(stem).unwrap_or_default()
    }

    /// Whether tab `i` holds a database.
    pub fn is_db_tab(&self, i: usize) -> bool {
        self.docs.tabs.get(i).is_some_and(|t| t.db.is_some())
    }

    /// Whether tab `i` has unsaved changes (a database saves itself, so only one without a
    /// place to go counts).
    pub fn tab_dirty(&self, i: usize) -> bool {
        match self.docs.tabs.get(i).and_then(|t| t.db.as_ref()) {
            Some(db) => db.dirty && db.path.is_none() && self.workspace.is_none(),
            None => self.tab_session(i).is_some_and(|s| s.dirty),
        }
    }

    /// Bring tab `i`'s document into the editor.
    pub fn switch_to(&mut self, i: usize) {
        self.ensure_tabs();
        if i == self.docs.active || i >= self.docs.tabs.len() {
            if self.glass.view == View::Home {
                self.glass.view = self.glass.doc_view;
            }
            return;
        }
        let Some((session, canvas)) = self.docs.tabs.get_mut(i).and_then(|t| t.stash.take()) else { return };
        let old_s = std::mem::replace(&mut self.app.session, session);
        let old_c = std::mem::replace(&mut self.app.canvas, canvas);
        if let Some(t) = self.docs.tabs.get_mut(self.docs.active) {
            t.stash = Some((old_s, old_c));
        }
        self.docs.active = i;
        self.after_switch();
    }

    fn after_switch(&mut self) {
        // Per-document leftovers that must not leak into the next document.
        self.app.dialog = None;
        self.app.word_count = (u64::MAX, 0);
        self.app.canvas.want_focus = true;
        self.app.canvas.scroll_to_caret = true;
        self.glass.selected_version = None;
        self.glass.tracker.forget_document();
        self.docs.switched_at = wordcraft_ui_egui::now_ms() / 1000.0;
        if self.glass.view == View::Home {
            self.glass.view = self.glass.doc_view;
        }
    }

    /// Open `session` in a new tab and show it.
    pub fn new_tab(&mut self, session: Session) {
        self.ensure_tabs();
        self.docs.next_uid += 1;
        self.docs.tabs.push(DocTab { uid: self.docs.next_uid, stash: Some((session, CanvasState::default())), db: None });
        let i = self.docs.tabs.len() - 1;
        self.switch_to(i);
        self.glass.view = self.glass.doc_view;
    }

    /// Open `session` in a new tab behind the open one (for a page made from a block). Returns
    /// the tab's id.
    pub fn add_background_tab(&mut self, session: Session) -> u64 {
        self.ensure_tabs();
        self.docs.next_uid += 1;
        let uid = self.docs.next_uid;
        let at = (self.docs.active + 1).min(self.docs.tabs.len());
        self.docs.tabs.insert(at, DocTab { uid, stash: Some((session, CanvasState::default())), db: None });
        uid
    }

    /// Whether the open tab holds an untouched blank document (worth reusing).
    pub(crate) fn active_is_pristine(&self) -> bool {
        !self.is_db_tab(self.docs.active) && self.app.session.path.is_none() && !self.app.session.dirty && self.app.session.word_count() == 0
    }

    /// Open the file at `path`: switch to its tab if it is open, else open it in a new tab (or in
    /// the open tab if that is an untouched blank document).
    pub fn open_path(&mut self, path: &str) {
        self.ensure_tabs();
        let p = std::path::Path::new(path);
        if p.extension().is_some_and(|x| x == crate::db::EXT) {
            self.open_database_path(p);
            return;
        }
        if let Some(i) = (0..self.docs.tabs.len()).find(|i| self.tab_session(*i).and_then(|s| s.path.as_deref()) == Some(p)) {
            self.switch_to(i);
            self.glass.view = self.glass.doc_view;
            return;
        }
        if !self.active_is_pristine() {
            self.new_tab(Session::new(wordcraft_engine::doc::Document::new()));
        }
        self.glass.view = self.glass.doc_view;
        if let Err(e) = self.app.run("file.open", json!({"path": path})) {
            self.app.status(e);
        }
    }

    /// Show the system's Open dialog for a new tab.
    pub fn open_dialog(&mut self) {
        if !self.active_is_pristine() {
            self.new_tab(Session::new(wordcraft_engine::doc::Document::new()));
        }
        self.glass.view = self.glass.doc_view;
        if let Err(e) = self.app.run("file.open", json!({})) {
            self.app.status(e);
        }
    }

    /// A new document from a template (`blank`, `sample`, `letter`, `resume`, `report`).
    pub fn new_from_template(&mut self, template: &str) {
        self.ensure_tabs();
        let mut s = Session::new(wordcraft_engine::doc::Document::new());
        if template != "blank"
            && let Err(e) = s.run("file.new", &json!({"template": template}))
        {
            self.app.status(e.to_string());
        }
        s.path = None;
        if self.active_is_pristine() {
            self.app.session = s;
            self.after_switch();
            self.glass.view = self.glass.doc_view;
        } else {
            self.new_tab(s);
        }
    }

    /// The window is closing: true when it may. Documents with unsaved changes in other tabs are
    /// asked about one at a time first, so none is lost behind the open one.
    pub fn close_requested(&mut self) -> bool {
        let n = self.docs.tabs.len();
        self.save_databases(true);
        let others = (0..n).any(|i| i != self.docs.active && self.tab_dirty(i));
        if !others {
            return self.app.close_requested();
        }
        let first = (0..n).find(|i| self.tab_dirty(*i));
        if let Some(i) = first {
            self.switch_to(i);
            self.docs.confirm_close = Some(self.docs.active);
        }
        false
    }

    /// Close tab `i`, asking first when its document has unsaved changes.
    pub fn close_tab(&mut self, i: usize, force: bool) {
        self.ensure_tabs();
        self.save_databases(true);
        let dirty = self.tab_dirty(i);
        if dirty && !force && !self.is_db_tab(i) {
            self.docs.confirm_close = Some(i);
            return;
        }
        if self.docs.tabs.len() == 1 {
            // The last tab: leave a fresh blank document and go Home.
            self.app.session = Session::new(wordcraft_engine::doc::Document::new());
            self.app.canvas = CanvasState::default();
            self.after_switch();
            self.glass.view = View::Home;
            return;
        }
        if i == self.docs.active {
            let next = if i + 1 < self.docs.tabs.len() { i + 1 } else { i.saturating_sub(1) };
            self.switch_to(next);
        }
        if i < self.docs.tabs.len() {
            self.docs.tabs.remove(i);
            if self.docs.active > i {
                self.docs.active -= 1;
            }
        }
    }
}

/// Draw a small house (the Home tab's icon).
fn house(p: &egui::Painter, c: egui::Pos2, color: egui::Color32) {
    let s = egui::Stroke::new(1.5, color);
    let pts = [pos2(c.x - 6.0, c.y - 0.5), pos2(c.x, c.y - 6.0), pos2(c.x + 6.0, c.y - 0.5)];
    p.line_segment([pts[0], pts[1]], s);
    p.line_segment([pts[1], pts[2]], s);
    let body = Rect::from_min_max(pos2(c.x - 4.5, c.y - 1.5), pos2(c.x + 4.5, c.y + 5.5));
    p.rect_stroke(body, 1.5, s, egui::StrokeKind::Middle);
}

/// The tab strip: Home, each open document, and +. On macOS it sits beside the traffic lights.
pub fn strip(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    g.ensure_tabs();
    let mac = g.app.integrated_titlebar;
    let height = if mac { 30.0 } else { 40.0 };
    egui::Panel::top("glass_doc_tabs")
        .show_separator_line(false)
        .exact_size(height)
        .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin {
            left: if mac { MAC_CONTENT_LEFT as i8 } else { 14 },
            right: 14,
            top: if mac { 2 } else { 8 },
            bottom: 0,
        }))
        .show(ui, |ui| {
            let full = ui.max_rect();
            // Empty space moves the window, as a title bar does.
            let drag = ui.interact(full, ui.id().with("glass_tabs_drag"), Sense::click_and_drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            let h = if mac { 24.0 } else { 28.0 };
            let y = if mac { full.min.y + 12.0 - h / 2.0 } else { full.center().y - h / 2.0 };
            let mut x = full.min.x;
            // Home.
            let home = Rect::from_min_size(pos2(x, y), vec2(h + 8.0, h));
            let resp = ui.interact(home, ui.id().with("glass_home_tab"), Sense::click()).on_hover_text("Home");
            if g.glass.view == View::Home {
                paint_pill(ui.painter(), home, t);
            } else {
                paint_state(ui, home, radius::CAPSULE, &resp, false, t);
            }
            house(ui.painter(), home.center(), if g.glass.view == View::Home { t.accent } else { t.text_dim });
            if resp.clicked() {
                g.set_view(View::Home);
            }
            x = home.max.x + 6.0;
            // Documents.
            let n = g.docs.tabs.len();
            let room = (full.max.x - x - 40.0).max(80.0);
            let tab_w = (room / n.max(1) as f32).clamp(96.0, 230.0);
            let mut action: Option<(usize, bool)> = None;
            let mut hovered: Option<(usize, u64, Rect)> = None;
            for i in 0..n {
                let r = Rect::from_min_size(pos2(x, y), vec2(tab_w - 4.0, h));
                if r.max.x > full.max.x - 36.0 {
                    break;
                }
                let uid = g.docs.tabs.get(i).map(|t| t.uid).unwrap_or_default();
                let resp = ui.interact(r, ui.id().with(("glass_doc_tab", uid)), Sense::click());
                let on = i == g.docs.active && g.glass.view != View::Home;
                if on {
                    paint_pill(ui.painter(), r, t);
                } else {
                    paint_state(ui, r, radius::CAPSULE, &resp, false, t);
                }
                let title = g.tab_title(i);
                let is_db = g.is_db_tab(i);
                let dirty = g.tab_dirty(i) || (!is_db && g.tab_session(i).is_some_and(|s| s.dirty));
                let max_chars = ((tab_w - 44.0) / 7.0).max(4.0) as usize;
                let shown: String = if title.chars().count() > max_chars {
                    format!("{}…", title.chars().take(max_chars).collect::<String>())
                } else {
                    title.clone()
                };
                let font = if on { semibold(12.5) } else { medium(12.5) };
                let text_x = if is_db {
                    let icon = g.docs.tabs.get(i).and_then(|t| t.db.as_ref()).map(|d| d.icon.clone()).unwrap_or_default();
                    crate::glyphs::paint(
                        ui.painter(),
                        Rect::from_center_size(pos2(r.min.x + 19.0, r.center().y), vec2(13.0, 13.0)),
                        &icon,
                        if on { t.accent } else { t.text_dim },
                    );
                    r.min.x + 32.0
                } else {
                    r.min.x + 12.0
                };
                ui.painter().text(pos2(text_x, r.center().y), Align2::LEFT_CENTER, shown, font, if on { t.text } else { t.text_dim });
                // The close button, or the unsaved dot when not hovered.
                let close = Rect::from_center_size(pos2(r.max.x - 13.0, r.center().y), vec2(18.0, 18.0));
                let cresp = ui.interact(close, ui.id().with(("glass_doc_close", uid)), Sense::click());
                if resp.hovered() || cresp.hovered() || on {
                    if cresp.hovered() {
                        ui.painter().circle_filled(close.center(), 9.0, t.hover);
                    }
                    let c = close.center();
                    let st = egui::Stroke::new(1.3, t.text_dim);
                    ui.painter().line_segment([pos2(c.x - 3.5, c.y - 3.5), pos2(c.x + 3.5, c.y + 3.5)], st);
                    ui.painter().line_segment([pos2(c.x + 3.5, c.y - 3.5), pos2(c.x - 3.5, c.y + 3.5)], st);
                } else if dirty {
                    ui.painter().circle_filled(close.center(), 3.5, t.text_dim);
                }
                if resp.hovered() && !cresp.hovered() {
                    hovered = Some((i, uid, r));
                }
                if cresp.on_hover_text("Close tab").clicked() {
                    action = Some((i, true));
                } else if resp.clicked() {
                    action = Some((i, false));
                }
                x = r.max.x + 4.0;
            }
            // New tab.
            let plus = Rect::from_min_size(pos2(x + 2.0, y), vec2(h, h));
            let presp = ui.interact(plus, ui.id().with("glass_new_tab"), Sense::click()).on_hover_text("New document");
            paint_state(ui, plus, radius::CAPSULE, &presp, false, t);
            let c = plus.center();
            let st = egui::Stroke::new(1.5, t.text_dim);
            ui.painter().line_segment([pos2(c.x - 5.0, c.y), pos2(c.x + 5.0, c.y)], st);
            ui.painter().line_segment([pos2(c.x, c.y - 5.0), pos2(c.x, c.y + 5.0)], st);
            if presp.clicked() {
                g.new_tab(Session::new(wordcraft_engine::doc::Document::new()));
            }
            let now = ui.input(|i| i.time);
            g.docs.hover = match (hovered, g.docs.hover) {
                (Some((_, uid, _)), Some((h, since))) if h == uid => Some((h, since)),
                (Some((_, uid, _)), _) => Some((uid, now)),
                (None, _) => None,
            };
            if let (Some((i, _, r)), Some((_, since))) = (hovered, g.docs.hover)
                && action.is_none()
                && !g.is_db_tab(i)
            {
                preview(g, ui.ctx(), t, i, r, now - since);
            }
            match action {
                Some((i, true)) => g.close_tab(i, false),
                Some((i, false)) => g.switch_to(i),
                None => {}
            }
        });
}

/// A tab's hover preview: a live thumbnail of the page its document is on, its name and state.
/// It appears after a short pause and fades and rises in, as Safari's tab previews do.
fn preview(g: &mut GlassApp, ctx: &egui::Context, t: &Tokens, i: usize, tab: Rect, hovered_for: f64) {
    const DELAY: f64 = 0.35;
    if hovered_for < DELAY {
        ctx.request_repaint_after(std::time::Duration::from_millis(60));
        return;
    }
    let a = (((hovered_for - DELAY) / 0.16) as f32).clamp(0.0, 1.0);
    let e = 1.0 - (1.0 - a).powi(3);
    if a < 1.0 {
        ctx.request_repaint();
    }
    let Some((page, pages, words, dirty, name)) = g.tab_session(i).map(|s| (s.page_hint, 0usize, s.word_count(), s.dirty, stem(s))) else { return };
    let pages =
        if i == g.docs.active { g.app.session.layout().pages.len() } else { g.docs.stash_mut(i).map(|s| s.layout().pages.len()).unwrap_or(pages) };
    let ppp = ctx.pixels_per_point();
    let w = 196.0;
    let tex = g.thumbnail(ctx, Some(i), page.min(pages.saturating_sub(1)), (w * ppp) as u32);
    let pos = pos2(tab.min.x, tab.max.y + 8.0 + (1.0 - e) * 6.0);
    egui::Area::new(egui::Id::new("glass_tab_preview")).order(egui::Order::Tooltip).interactable(false).fixed_pos(pos).show(ctx, |ui| {
        ui.multiply_opacity(e);
        let bg = ui.painter().add(egui::Shape::Noop);
        let inner = egui::Frame::NONE.inner_margin(10).show(ui, |ui| {
            ui.set_width(w);
            ui.spacing_mut().item_spacing = vec2(0.0, 6.0);
            let h = tex.as_ref().map(|x| x.size_vec2()).map(|s| (w * s.y / s.x.max(1.0)).min(260.0)).unwrap_or(w * 1.29);
            let (r, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
            let shadow = egui::epaint::Shadow { offset: [0, 2], blur: 8, spread: 0, color: t.shadow };
            ui.painter().add(shadow.as_shape(r, egui::CornerRadius::same(6)));
            match &tex {
                Some(tex) => {
                    // Show the top of the page when it is taller than the card.
                    let uv_h = (h / (w * tex.size_vec2().y / tex.size_vec2().x.max(1.0))).min(1.0);
                    let mut mesh = egui::Mesh::with_texture(tex.id());
                    mesh.add_rect_with_uv(r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, uv_h)), egui::Color32::WHITE);
                    ui.painter().add(egui::Shape::mesh(mesh));
                }
                None => {
                    ui.painter().rect_filled(r, 6.0, egui::Color32::WHITE);
                }
            }
            ui.painter().rect_stroke(r, 6.0, egui::Stroke::new(0.75, t.edge), egui::StrokeKind::Outside);
            ui.label(egui::RichText::new(&name).font(semibold(12.5)).color(t.text));
            let state = if dirty { " · Edited" } else { "" };
            ui.label(
                egui::RichText::new(format!("Page {} of {} · {} words{state}", page + 1, pages.max(1), crate::chrome::group_digits(words as i64)))
                    .font(regular(11.0))
                    .color(t.text_dim),
            );
        });
        ui.painter().set(bg, crate::tokens::frost_shape(inner.response.rect, 14, t));
    });
}

/// "Close without saving?" for a tab with unsaved changes.
pub fn confirm_close(g: &mut GlassApp, ctx: &egui::Context, t: &Tokens) {
    let Some(i) = g.docs.confirm_close else { return };
    let title = g.tab_title(i);
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("glass_close_scrim")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(60));
        ui.allocate_rect(screen, Sense::click());
    });
    egui::Area::new(egui::Id::new("glass_close_sheet")).order(egui::Order::Tooltip).pivot(Align2::CENTER_CENTER).fixed_pos(screen.center()).show(
        ctx,
        |ui| {
            let bg = ui.painter().add(egui::Shape::Noop);
            let inner = egui::Frame::NONE.inner_margin(22).show(ui, |ui| {
                ui.set_width(340.0);
                ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
                ui.label(egui::RichText::new(format!("Save changes to “{title}”?")).font(semibold(15.0)).color(t.text));
                ui.label(egui::RichText::new("Your changes will be lost if you close without saving.").font(regular(12.5)).color(t.text_dim));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if crate::widgets::quiet_button(ui, t, "Don't Save", Some(104.0), 32.0).clicked() {
                        g.docs.confirm_close = None;
                        g.close_tab(i, true);
                    }
                    ui.add_space(ui.available_width() - 2.0 * 96.0 - 8.0);
                    if crate::widgets::quiet_button(ui, t, "Cancel", Some(96.0), 32.0).clicked() {
                        g.docs.confirm_close = None;
                    }
                    if crate::widgets::accent_button(ui, t, None, "Save", Some(96.0), 32.0).clicked() {
                        g.docs.confirm_close = None;
                        g.switch_to(i);
                        crate::widgets::run(ui, &mut g.app, "file.save", json!({}));
                    }
                });
            });
            ui.painter().set(bg, crate::tokens::popover_shape(inner.response.rect, 20, t));
        },
    );
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        g.docs.confirm_close = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_ui_egui::{Services, WordApp};

    fn glass() -> GlassApp {
        GlassApp::new(WordApp::new(Session::new(wordcraft_engine::sample::sample_document()), Services::default()))
    }

    #[test]
    fn tabs_keep_their_own_documents() {
        let mut g = glass();
        let first = g.app.session.word_count();
        g.new_tab(Session::new(wordcraft_engine::doc::Document::new()));
        assert_eq!(g.docs.tabs.len(), 2);
        assert_eq!(g.app.session.word_count(), 0);
        g.switch_to(0);
        assert_eq!(g.app.session.word_count(), first);
        g.switch_to(1);
        assert_eq!(g.app.session.word_count(), 0);
    }

    #[test]
    fn closing_a_clean_tab_shows_its_neighbour_and_the_last_tab_goes_home() {
        let mut g = glass();
        g.app.session.dirty = false;
        g.new_tab(Session::new(wordcraft_engine::doc::Document::new()));
        g.close_tab(1, false);
        assert_eq!(g.docs.tabs.len(), 1);
        assert!(g.app.session.word_count() > 0);
        g.close_tab(0, false);
        assert_eq!(g.docs.tabs.len(), 1);
        assert_eq!(g.glass.view, View::Home);
    }

    #[test]
    fn closing_an_edited_tab_asks_first() {
        let mut g = glass();
        g.app.session.dirty = true;
        g.close_tab(0, false);
        assert_eq!(g.docs.confirm_close, Some(0));
    }

    #[test]
    fn templates_reuse_an_untouched_blank_tab() {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_engine::doc::Document::new()), Services::default()));
        g.new_from_template("letter");
        assert_eq!(g.docs.tabs.len(), 1);
        assert!(g.app.session.word_count() > 0);
        g.new_from_template("resume");
        assert_eq!(g.docs.tabs.len(), 2);
    }
}

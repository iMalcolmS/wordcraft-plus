//! Choose a Template: a sheet over the window with the categories on the left and every
//! template on the right, each shown as a real thumbnail of its first page (rendered by the
//! engine) or, for a database, a small picture of its records. Click to select, double-click or
//! Create to make one. My Templates lists documents and databases saved with Save as Template.

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{Align2, Color32, Rect, Sense, Stroke, TextureHandle, Ui, pos2, vec2};
use wordcraft_engine::Session;
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::GlassApp;
use crate::db::Database;
use crate::glyphs;
use crate::templates::{self, Category, Made};
use crate::tokens::{Tokens, paint_state, radius};

/// A template to make: a built-in one or a saved file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Pick {
    Builtin(&'static str),
    Saved(PathBuf),
}

/// The chooser's state while it is open.
#[derive(Default)]
pub struct Chooser {
    pub open: bool,
    /// The category shown (`None`: all).
    pub category: Option<Category>,
    pub selected: Option<Pick>,
    pub search: String,
    pub opened_at: f64,
    /// Thumbnails, by template (`None`: it couldn't be drawn).
    thumbs: HashMap<Pick, Option<TextureHandle>>,
    /// Saved templates and when they were listed.
    saved: (f64, Vec<(String, PathBuf, Made)>),
}

impl GlassApp {
    /// Show the template chooser (on `category`, or all).
    pub fn choose_template(&mut self, category: Option<Category>) {
        let c = &mut self.glass.chooser;
        c.open = true;
        c.category = category;
        c.selected = None;
        c.search.clear();
        c.opened_at = wordcraft_ui_egui::now_ms() / 1000.0;
        c.saved.0 = 0.0;
    }

    /// The folder Save as Template writes to.
    pub fn templates_dir(&self) -> Option<PathBuf> {
        self.workspace.as_ref().map(|w| w.join("Templates"))
    }

    /// Make a new document or database from a template.
    pub fn create_from(&mut self, pick: &Pick) {
        match pick {
            Pick::Builtin(id) => {
                let Some(t) = templates::find(id) else { return };
                match t.made {
                    Made::Database => {
                        if let Some(db) = templates::database(id) {
                            self.open_database(db);
                        }
                    }
                    made => {
                        let Some(mut doc) = templates::document(id) else { return };
                        if doc.core.title.trim().is_empty() && *id != "blank" {
                            doc.core.title = String::new();
                        }
                        self.new_document(doc, made == Made::PageLayout);
                    }
                }
            }
            Pick::Saved(path) => {
                if path.extension().is_some_and(|x| x == crate::db::EXT) {
                    match Database::open(path) {
                        Ok(mut db) => {
                            db.path = None;
                            for r in &mut db.rows {
                                r.page = None;
                            }
                            self.open_database(db);
                        }
                        Err(e) => self.app.status(e),
                    }
                } else {
                    match wordcraft_engine::io::open_path(path) {
                        Ok(doc) => self.new_document(doc, false),
                        Err(e) => self.app.status(e),
                    }
                }
            }
        }
        self.glass.chooser.open = false;
    }

    /// Open `doc` as a new, untitled document (in the open tab when that is an untouched blank
    /// one). `layout`: a page layout document (the Format inspector opens with it).
    pub fn new_document(&mut self, doc: wordcraft_doc::Document, layout: bool) {
        let mut s = Session::new(doc);
        s.path = None;
        s.dirty = false;
        let id = s.document_id();
        if layout {
            self.glass.layout_docs.insert(id);
        }
        self.ensure_tabs();
        if self.active_is_pristine() {
            self.app.session = s;
            self.app.canvas = Default::default();
            self.glass.view = View::Page;
            self.docs.switched_at = wordcraft_ui_egui::now_ms() / 1000.0;
            self.app.canvas.want_focus = true;
        } else {
            self.new_tab(s);
        }
        self.glass.view = crate::View::Page;
        if layout {
            self.glass.format_open = true;
        }
    }

    /// Save the open document or database as a template in the workspace's Templates folder.
    pub fn save_as_template(&mut self) {
        let Some(dir) = self.templates_dir() else {
            self.app.status("There is no workspace folder to keep templates in");
            return;
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.app.status(format!("Couldn't save the template: {e}"));
            return;
        }
        let result = if let Some(db) = self.active_db() {
            let mut copy = db.clone();
            let path = dir.join(format!("{}.{}", crate::workspace::file_stem(&copy.name), crate::db::EXT));
            copy.save_to(&path).map(|_| path)
        } else {
            let name = self.tab_title(self.docs.active);
            let path = dir.join(format!("{}.docx", crate::workspace::file_stem(&name)));
            wordcraft_engine::io::save_bytes("t.docx", &self.app.session.doc)
                .and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()))
                .map(|_| path)
        };
        match result {
            Ok(p) => {
                self.glass.chooser.saved.0 = 0.0;
                self.app.status(format!("Saved as a template: {}", p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()));
            }
            Err(e) => self.app.status(format!("Couldn't save the template: {e}")),
        }
    }

    fn saved_templates(&mut self) -> Vec<(String, PathBuf, Made)> {
        let now = wordcraft_ui_egui::now_ms() / 1000.0;
        if self.glass.chooser.saved.0 > 0.0 && now - self.glass.chooser.saved.0 < 5.0 {
            return self.glass.chooser.saved.1.clone();
        }
        let mut out = Vec::new();
        if let Some(dir) = self.templates_dir()
            && let Ok(entries) = std::fs::read_dir(dir)
        {
            for e in entries.flatten().take(300) {
                let p = e.path();
                let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                let made = match ext.as_str() {
                    "docx" | "dotx" => Made::Document,
                    x if x == crate::db::EXT => Made::Database,
                    _ => continue,
                };
                let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                out.push((name, p, made));
            }
        }
        out.sort_by_key(|a| a.0.to_lowercase());
        self.glass.chooser.saved = (now, out.clone());
        out
    }

    /// A thumbnail for `pick`, rendered when the frame's budget allows.
    fn template_thumb(&mut self, ctx: &egui::Context, pick: &Pick) -> Option<TextureHandle> {
        if let Some(t) = self.glass.chooser.thumbs.get(pick) {
            return t.clone();
        }
        if self.glass.thumbs.budget == 0 {
            ctx.request_repaint();
            return None;
        }
        self.glass.thumbs.budget -= 1;
        let doc = match pick {
            Pick::Builtin(id) => templates::document(id),
            Pick::Saved(p) => wordcraft_engine::io::open_path(p).ok(),
        };
        let tex = doc.and_then(|doc| {
            let mut s = Session::new(doc);
            let layout = s.layout();
            let page = layout.pages.first()?;
            let scale = 300.0 / page.w.max(1.0);
            let img = wordcraft_engine::render::render_page(&s.doc, page, scale, &wordcraft_engine::render::RenderOptions::default());
            let px = img.to_straight();
            let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &px);
            Some(ctx.load_texture(format!("glass_template_{pick:?}"), ci, egui::TextureOptions::LINEAR))
        });
        self.glass.chooser.thumbs.insert(pick.clone(), tex.clone());
        tex
    }
}

use crate::View;

/// Draw the chooser when it is open.
pub fn show(g: &mut GlassApp, ctx: &egui::Context, t: &Tokens) {
    if !g.glass.chooser.open {
        return;
    }
    let since = wordcraft_ui_egui::now_ms() / 1000.0 - g.glass.chooser.opened_at;
    let a = ((since / 0.2) as f32).clamp(0.0, 1.0);
    let e = 1.0 - (1.0 - a).powi(3);
    if a < 1.0 {
        ctx.request_repaint();
    }
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("glass_chooser_scrim")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha((70.0 * e) as u8));
        if ui.allocate_rect(screen, Sense::click()).clicked() {
            g.glass.chooser.open = false;
        }
    });
    let size = vec2((screen.width() - 80.0).clamp(560.0, 1240.0), (screen.height() - 80.0).clamp(420.0, 860.0));
    let rect = Rect::from_center_size(screen.center() + vec2(0.0, (1.0 - e) * 16.0), size);
    let mut create: Option<Pick> = None;
    egui::Area::new(egui::Id::new("glass_chooser")).order(egui::Order::Tooltip).fixed_pos(rect.min).show(ctx, |ui| {
        ui.multiply_opacity(e);
        // An opaque sheet: nothing of the window shows through the templates.
        ui.painter().rect_filled(rect, 22.0, if t.dark { Color32::from_gray(34) } else { Color32::from_gray(250) });
        ui.painter().add(crate::tokens::popover_shape(rect, 22, t));
        ui.allocate_rect(rect, Sense::hover());
        // Sidebar.
        let side = Rect::from_min_max(rect.min, pos2(rect.min.x + 236.0, rect.max.y));
        ui.painter().rect_filled(side.shrink(6.0), egui::CornerRadius { nw: 18, sw: 18, ne: 10, se: 10 }, t.fill.gamma_multiply(0.7));
        let mut y = side.min.y + 22.0;
        let items: Vec<(Option<Category>, &str, &str)> =
            std::iter::once((None, "All Templates", "gallery")).chain(Category::ALL.iter().map(|c| (Some(*c), c.name(), c.icon()))).collect();
        for (cat, name, icon) in items {
            let r = Rect::from_min_size(pos2(side.min.x + 14.0, y), vec2(side.width() - 28.0, 32.0));
            let resp = ui.interact(r, ui.id().with(("glass_chooser_cat", name)), Sense::click());
            let on = g.glass.chooser.category == cat;
            paint_state(ui, r, 9, &resp, on, t);
            glyphs::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 17.0, r.center().y), vec2(16.0, 16.0)), icon, t.accent);
            ui.painter().text(pos2(r.min.x + 36.0, r.center().y), Align2::LEFT_CENTER, name, if on { semibold(13.0) } else { medium(13.0) }, t.text);
            if resp.clicked() {
                g.glass.chooser.category = cat;
            }
            y += 34.0;
        }
        // Header.
        let main = Rect::from_min_max(pos2(side.max.x, rect.min.y), rect.max);
        let head = Rect::from_min_size(main.min, vec2(main.width(), 56.0));
        ui.painter().text(head.center(), Align2::CENTER_CENTER, "Choose a Template", semibold(15.0), t.text);
        let search_r = Rect::from_min_size(pos2(head.max.x - 230.0, head.center().y - 15.0), vec2(210.0, 30.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(search_r));
        child.add(egui::TextEdit::singleline(&mut g.glass.chooser.search).hint_text("Search templates").desired_width(210.0).margin(vec2(10.0, 6.0)));
        ui.painter().hline(main.x_range(), head.max.y, Stroke::new(0.75, t.edge));
        // Footer.
        let foot = Rect::from_min_max(pos2(main.min.x, main.max.y - 58.0), main.max);
        ui.painter().hline(main.x_range(), foot.min.y, Stroke::new(0.75, t.edge));
        let mut fchild =
            ui.new_child(egui::UiBuilder::new().max_rect(foot.shrink2(vec2(20.0, 13.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
        let can = g.glass.chooser.selected.is_some();
        if crate::widgets::accent_button(&mut fchild, t, None, "Create", Some(96.0), 32.0).clicked() && can {
            create = g.glass.chooser.selected.clone();
        }
        if crate::widgets::quiet_button(&mut fchild, t, "Cancel", Some(90.0), 32.0).clicked() {
            g.glass.chooser.open = false;
        }
        if !g.is_db_tab(g.docs.active) || g.active_db().is_some() {
            fchild.add_space(fchild.available_width() - 210.0);
            if crate::widgets::quiet_button(&mut fchild, t, "Save Current as Template", Some(200.0), 32.0)
                .on_hover_text("Keep the open document or database as a template in My Templates")
                .clicked()
            {
                g.save_as_template();
            }
        }
        // Templates.
        let body = Rect::from_min_max(pos2(main.min.x, head.max.y), pos2(main.max.x, foot.min.y));
        let mut bchild = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
        egui::ScrollArea::vertical().id_salt("glass_chooser_scroll").auto_shrink([false, false]).show(&mut bchild, |ui| {
            egui::Frame::NONE.inner_margin(egui::Margin { left: 28, right: 20, top: 18, bottom: 24 }).show(ui, |ui| {
                sections(g, ui, t, &mut create);
            });
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        g.glass.chooser.open = false;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Enter))
        && let Some(p) = g.glass.chooser.selected.clone()
    {
        create = Some(p);
    }
    if let Some(p) = create {
        g.create_from(&p);
    }
}

fn sections(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, create: &mut Option<Pick>) {
    let q = g.glass.chooser.search.trim().to_lowercase();
    let only = g.glass.chooser.category;
    let saved = g.saved_templates();
    let mut shown_any = false;
    for cat in Category::ALL {
        if only.is_some_and(|c| c != cat) {
            continue;
        }
        let mut items: Vec<(Pick, String, Made)> = if cat == Category::Mine {
            saved.iter().map(|(n, p, m)| (Pick::Saved(p.clone()), n.clone(), *m)).collect()
        } else {
            templates::ALL.iter().filter(|x| x.category == cat).map(|x| (Pick::Builtin(x.id), x.name.to_string(), x.made)).collect()
        };
        if !q.is_empty() {
            items.retain(|(_, n, _)| n.to_lowercase().contains(&q) || cat.name().to_lowercase().contains(&q));
        }
        if items.is_empty() && !(cat == Category::Mine && only == Some(Category::Mine)) {
            continue;
        }
        shown_any = true;
        ui.label(egui::RichText::new(cat.name()).font(semibold(19.0)).color(t.text));
        if !cat.blurb().is_empty() {
            ui.add_space(2.0);
            ui.label(egui::RichText::new(cat.blurb()).font(regular(12.5)).color(t.text_dim));
        }
        ui.add_space(12.0);
        if items.is_empty() {
            ui.label(
                egui::RichText::new("Nothing here yet: open a document or database and choose Save Current as Template.")
                    .font(regular(12.5))
                    .color(t.text_dim),
            );
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(22.0, 22.0);
            for (pick, name, made) in items {
                if tile(g, ui, t, &pick, &name, made, create) {
                    g.glass.chooser.selected = Some(pick.clone());
                }
            }
        });
        ui.add_space(28.0);
    }
    if !shown_any {
        ui.label(egui::RichText::new("No templates match.").font(regular(13.0)).color(t.text_dim));
    }
}

/// A template tile: its thumbnail and name. True when clicked.
pub(crate) fn tile(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, pick: &Pick, name: &str, made: Made, create: &mut Option<Pick>) -> bool {
    let w = 150.0;
    let landscape = matches!(pick, Pick::Builtin(id) if matches!(*id, "blank-landscape" | "certificate" | "award"));
    let page_h = if landscape { w * 0.773 } else { w * 1.294 };
    let (r, resp) = ui.allocate_exact_size(vec2(w, 194.0 + 34.0), Sense::click());
    let page = Rect::from_min_size(pos2(r.min.x, r.min.y + (194.0 - page_h) / 2.0 + if landscape { 20.0 } else { 0.0 }), vec2(w, page_h));
    let lift = ui.ctx().animate_bool_with_time(resp.id.with("lift"), resp.hovered(), 0.14);
    let page = page.translate(vec2(0.0, -3.0 * lift));
    let shadow = egui::epaint::Shadow { offset: [0, (3.0 + 5.0 * lift) as i8], blur: (10.0 + 10.0 * lift) as u8, spread: 0, color: t.shadow };
    ui.painter().add(shadow.as_shape(page, egui::CornerRadius::same(4)));
    ui.painter().rect_filled(page, 4.0, Color32::WHITE);
    match made {
        Made::Database => database_art(g, ui, t, pick, page),
        _ => match g.template_thumb(ui.ctx(), pick) {
            Some(tex) => {
                ui.painter().image(tex.id(), page, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                ui.painter().text(page.center(), Align2::CENTER_CENTER, "…", medium(16.0), Color32::from_gray(170));
            }
        },
    }
    let selected = g.glass.chooser.selected.as_ref() == Some(pick);
    if selected {
        ui.painter().rect_stroke(page.expand(3.0), 6.0, Stroke::new(3.0, t.accent), egui::StrokeKind::Outside);
    } else {
        ui.painter().rect_stroke(page, 4.0, Stroke::new(0.75, t.edge), egui::StrokeKind::Outside);
    }
    let label_r = Rect::from_min_size(pos2(r.min.x - 6.0, r.min.y + 198.0), vec2(w + 12.0, 30.0));
    let galley = ui.painter().layout(name.to_string(), medium(12.5), t.text, label_r.width());
    ui.painter().galley(pos2(label_r.center().x - galley.size().x / 2.0, label_r.min.y), galley, t.text);
    if resp.double_clicked() {
        *create = Some(pick.clone());
    }
    resp.clicked()
}

/// A database template's picture: its name and a few of its records as a mini table.
fn database_art(_g: &mut GlassApp, ui: &Ui, t: &Tokens, pick: &Pick, page: Rect) {
    let db = match pick {
        Pick::Builtin(id) => templates::database(id),
        Pick::Saved(p) => Database::open(p).ok(),
    };
    let Some(db) = db else { return };
    let p = ui.painter().with_clip_rect(page.shrink(1.0));
    glyphs::paint(&p, Rect::from_min_size(pos2(page.min.x + 12.0, page.min.y + 14.0), vec2(18.0, 18.0)), &db.icon, t.accent);
    p.text(pos2(page.min.x + 12.0, page.min.y + 46.0), Align2::LEFT_CENTER, &db.name, semibold(12.0), Color32::from_gray(30));
    let mut y = page.min.y + 64.0;
    let ink = Color32::from_gray(225);
    p.hline(page.min.x + 10.0..=page.max.x - 10.0, y, Stroke::new(0.75, ink));
    for r in db.rows.iter().take(6) {
        y += 18.0;
        let title = db.title(r.id);
        p.with_clip_rect(Rect::from_min_max(page.min, pos2(page.max.x - 46.0, page.max.y))).text(
            pos2(page.min.x + 12.0, y - 9.0),
            Align2::LEFT_CENTER,
            title,
            regular(8.5),
            Color32::from_gray(60),
        );
        // The record's first option, as a chip.
        let chip = db.props.iter().filter(|pr| pr.kind.has_options()).find_map(|pr| match r.cells.get(&pr.id) {
            Some(crate::db::Cell::Options(o)) => o.first().map(|n| db.option_tint(pr.id, n)),
            _ => None,
        });
        if let Some(tint) = chip {
            let (bg, _) = crate::db_view::chip_colors(tint, &Tokens::new(false, Default::default()));
            p.rect_filled(Rect::from_min_size(pos2(page.max.x - 42.0, y - 14.0), vec2(30.0, 9.0)), 3.0, bg);
        }
        p.hline(page.min.x + 10.0..=page.max.x - 10.0, y, Stroke::new(0.5, ink));
    }
    let views: Vec<&str> = db.views.iter().map(|v| v.layout.icon()).collect();
    let mut x = page.min.x + 12.0;
    for v in views.iter().take(5) {
        glyphs::paint(&p, Rect::from_min_size(pos2(x, page.max.y - 24.0), vec2(12.0, 12.0)), v, Color32::from_gray(150));
        x += 18.0;
    }
    let _ = radius::ROW;
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_ui_egui::{Services, WordApp};

    fn glass() -> GlassApp {
        GlassApp::new(WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default()))
    }

    #[test]
    fn templates_make_documents_layouts_and_databases() {
        let mut g = glass();
        g.create_from(&Pick::Builtin("essay"));
        assert_eq!(g.docs.tabs.len(), 1, "the blank tab is reused");
        assert!(g.app.session.word_count() > 0);
        assert!(g.app.session.path.is_none());
        g.create_from(&Pick::Builtin("event-flyer"));
        assert_eq!(g.docs.tabs.len(), 2);
        assert!(g.glass.layout_docs.contains(&g.app.session.document_id()));
        g.create_from(&Pick::Builtin("db-reading"));
        assert!(g.is_db_tab(g.docs.active));
        assert_eq!(g.tab_title(g.docs.active), "Reading List");
        assert!(!g.glass.chooser.open);
    }

    #[test]
    fn templates_can_be_saved_and_reused() {
        let dir = std::env::temp_dir().join(format!("wordcraft-tpl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut g = glass();
        g.set_workspace(Some(dir.clone()));
        g.create_from(&Pick::Builtin("cover-letter"));
        g.save_as_template();
        let saved = g.saved_templates();
        assert_eq!(saved.len(), 1);
        let words = g.app.session.word_count();
        g.create_from(&Pick::Saved(saved[0].1.clone()));
        assert_eq!(g.app.session.word_count(), words);
        assert!(g.app.session.path.is_none(), "a template opens as a new, untitled document");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

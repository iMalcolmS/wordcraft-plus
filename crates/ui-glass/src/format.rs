//! The Format inspector, for page layout (as a page layout app's sidebar): with a picture, shape
//! or text box selected, its Style (fill, border), Arrange (size, position on the page,
//! rotation, wrap, order, alignment) and, for text, quick text formatting. With nothing
//! selected, it adds things to the page: text boxes, shapes, your pictures, and pictures drawn in
//! code (dunes, waves, lights…), each placed in front of the text, free to move.
//!
//! Every change runs a WordCraft command, so it is undoable and works the same from scripts.

use egui::{Align2, Color32, Rect, Sense, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_doc::para::{Anchor, InlineObject, OBJ, ShapeKind, Wrap};
use wordcraft_engine::Selection;
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::GlassApp;
use crate::templates::Art;
use crate::tokens::{Tokens, paint_state, radius};

/// The inspector's tabs.
const TABS: [&str; 3] = ["Style", "Arrange", "Text"];

/// Colours offered for fills, borders and text (our own palette).
const PALETTE: [u32; 16] = [
    0xFFFFFF, 0xF4F1DE, 0xFBE8D3, 0xDDEDE4, 0xDBEAF4, 0xEAE2F2, 0xF5E0EB, 0xE9E9E7, 0x1F1F1F, 0xC46A38, 0xE07A5F, 0x448361, 0x1C6A93, 0x3D405B,
    0x9065B0, 0xC14C8A,
];

/// Pictures that can be drawn onto the page.
const ARTS: [(Art, &str); 6] =
    [(Art::Dunes, "Dunes"), (Art::Waves, "Waves"), (Art::Hills, "Hills"), (Art::Lights, "Lights"), (Art::Mosaic, "Mosaic"), (Art::Wash, "Wash")];

/// The selected floating or inline object, if the selection is exactly one.
pub fn selected_object(g: &GlassApp) -> Option<InlineObject> {
    let s = &g.app.session;
    let (a, b) = s.sel.ordered();
    if a.story != b.story || a.path != b.path || b.off != a.off + OBJ.len_utf8() {
        return None;
    }
    s.doc.para_at(&a)?.object_at(a.off).cloned()
}

fn run(g: &mut GlassApp, id: &str, p: Value) {
    if let Err(e) = g.app.run(id, p) {
        g.app.status(e);
    }
}

fn hex(c: u32) -> String {
    format!("{c:06X}")
}

/// A row of colour swatches; returns the one clicked (`None` for "none" when offered).
fn swatches(ui: &mut Ui, t: &Tokens, current: Option<u32>, none: bool) -> Option<Option<u32>> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        if none {
            let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
            ui.painter().rect_stroke(r.shrink(1.0), 6.0, egui::Stroke::new(1.0, t.text_dim), egui::StrokeKind::Inside);
            ui.painter().line_segment(
                [r.left_bottom() + vec2(4.0, -4.0), r.right_top() + vec2(-4.0, 4.0)],
                egui::Stroke::new(1.5, Color32::from_rgb(0xD4, 0x4C, 0x47)),
            );
            if resp.on_hover_text("None").clicked() {
                picked = Some(None);
            }
        }
        for c in PALETTE {
            let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
            let col = Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
            ui.painter().rect_filled(r.shrink(1.0), 6.0, col);
            ui.painter().rect_stroke(r.shrink(1.0), 6.0, egui::Stroke::new(0.75, t.edge), egui::StrokeKind::Inside);
            if current == Some(c) || resp.hovered() {
                ui.painter().rect_stroke(r.expand(1.0), 7.0, egui::Stroke::new(2.0, t.accent), egui::StrokeKind::Outside);
            }
            if resp.clicked() {
                picked = Some(Some(c));
            }
        }
    });
    picked
}

fn label(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new(text).font(semibold(12.0)).color(t.text));
    ui.add_space(4.0);
}

/// A tile button with a glyph (and a label under it).
fn tile(ui: &mut Ui, t: &Tokens, icon: &str, text: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(74.0, 62.0), Sense::click());
    paint_state(ui, r, 12, &resp, false, t);
    ui.painter().rect_stroke(r, 12.0, egui::Stroke::new(0.75, t.edge), egui::StrokeKind::Inside);
    crate::glyphs::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 22.0), vec2(20.0, 20.0)), icon, t.accent);
    ui.painter().text(pos2(r.center().x, r.max.y - 14.0), Align2::CENTER_CENTER, text, medium(11.0), t.text);
    resp
}

/// The inspector's content (in its dock).
pub fn panel(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Format").font(semibold(15.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::widgets::icon_capsule(ui, t, "close", "Close", 26.0, false, true).clicked() {
                g.glass.format_open = false;
            }
        });
    });
    ui.add_space(8.0);
    let obj = selected_object(g);
    let tab_i = TABS.iter().position(|x| *x == g.glass.format_tab).unwrap_or(if obj.is_some() { 0 } else { 2 });
    if let Some(i) = crate::widgets::segmented(ui, t, &TABS, tab_i, 28.0) {
        g.glass.format_tab = TABS.get(i).copied().unwrap_or("Style").to_string();
    }
    ui.add_space(6.0);
    egui::ScrollArea::vertical().id_salt("glass_format").auto_shrink([false, false]).show(ui, |ui| match (TABS.get(tab_i).copied(), obj) {
        (Some("Text"), _) => text_tab(g, ui, t),
        (_, None) => add_tab(g, ui, t),
        (Some("Arrange"), Some(o)) => arrange_tab(g, ui, t, &o),
        (_, Some(o)) => style_tab(g, ui, t, &o),
    });
}

/// Nothing selected: add things to the page.
fn add_tab(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    ui.label(egui::RichText::new("Select a picture, shape or text box to format it, or add one:").font(regular(12.0)).color(t.text_dim));
    label(ui, t, "Add to the page");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        if tile(ui, t, "doc", "Text box").clicked() {
            add_floating(g, "insert.textBox", json!({"text": "Your text here", "width": 220, "height": 80}), (220.0, 80.0));
        }
        for (kind, icon, name) in
            [("rectangle", "table", "Rectangle"), ("ellipse", "circle", "Circle"), ("star", "star", "Star"), ("roundRect", "check", "Rounded")]
        {
            if tile(ui, t, icon, name).clicked() {
                let kind = if kind == "roundRect" { "roundedRectangle" } else { kind };
                add_floating(g, "insert.shape", json!({"kind": kind, "width": 140, "height": 140, "fill": "1C6A93"}), (140.0, 140.0));
            }
        }
        if tile(ui, t, "layout", "Picture…").clicked() {
            run(g, "insert.picture", json!({}));
        }
    });
    label(ui, t, "Drawn pictures");
    ui.label(egui::RichText::new("Original artwork drawn by WordCraft, free to use anywhere.").font(regular(11.5)).color(t.text_dim));
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for (art, name) in ARTS {
            let (r, resp) = ui.allocate_exact_size(vec2(74.0, 60.0), Sense::click());
            let tex = art_texture(ui.ctx(), art);
            ui.painter().image(
                tex.id(),
                r.shrink2(vec2(0.0, 8.0)).translate(vec2(0.0, -8.0)),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            if resp.hovered() {
                ui.painter().rect_stroke(r, 6.0, egui::Stroke::new(2.0, t.accent), egui::StrokeKind::Outside);
            }
            ui.painter().text(pos2(r.center().x, r.max.y - 6.0), Align2::CENTER_CENTER, name, regular(10.5), t.text_dim);
            if resp.clicked() {
                add_art(g, art);
            }
        }
    });
    label(ui, t, "Page");
    if let Some(c) = swatches(ui, t, None, true) {
        run(g, "design.pageColor", json!({"color": c.map(hex).unwrap_or_else(|| "none".into())}));
    }
    ui.add_space(8.0);
    let mut grid = g.glass.prefs.grid;
    if ui.checkbox(&mut grid, "Show layout guides").changed() {
        g.glass.prefs.grid = grid;
    }
}

/// The selected object's style.
fn style_tab(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, o: &InlineObject) {
    let (fill, stroke, width) = match o {
        InlineObject::Shape { fill, stroke, stroke_width, .. } => (*fill, *stroke, *stroke_width),
        _ => (None, None, 0.0),
    };
    let as_u32 = |c: Option<wordcraft_doc::props::Rgb>| c.map(|c| (u32::from(c.0) << 16) | (u32::from(c.1) << 8) | u32::from(c.2));
    if matches!(o, InlineObject::Shape { .. }) {
        label(ui, t, "Fill");
        if let Some(c) = swatches(ui, t, as_u32(fill), true) {
            run(g, "shape.fill", json!({"color": c.map(hex).unwrap_or_else(|| "none".into())}));
        }
        label(ui, t, "Border");
        if let Some(c) = swatches(ui, t, as_u32(stroke), true) {
            run(g, "shape.outline", json!({"color": c.map(hex).unwrap_or_else(|| "none".into())}));
        }
        let mut w = width;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Width").font(regular(12.0)).color(t.text_dim));
            if ui.add(egui::Slider::new(&mut w, 0.0..=12.0).suffix(" pt")).drag_stopped() {
                run(g, "shape.outline", json!({"width": w}));
            }
        });
    } else {
        label(ui, t, "Picture");
        ui.horizontal_wrapped(|ui| {
            for (name, id, p) in [
                ("Border", "picture.border", json!({"color": "1F1F1F", "width": 1})),
                ("Reset", "picture.reset", json!({})),
                ("Crop…", "picture.crop", json!({})),
                ("Alt text…", "picture.altText", json!({})),
            ] {
                if crate::widgets::quiet_button(ui, t, name, None, 28.0).clicked() {
                    run(g, id, p);
                }
            }
        });
    }
}

/// The selected object's size, place, rotation, wrap and order.
fn arrange_tab(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, o: &InlineObject) {
    let (w, h, float) = match o {
        InlineObject::Shape { w, h, float, .. }
        | InlineObject::Image { w, h, float, .. }
        | InlineObject::Graphic { w, h, float, .. }
        | InlineObject::Group { w, h, float, .. } => (*w, *h, float.clone()),
        _ => return,
    };
    label(ui, t, "Size");
    let (mut nw, mut nh) = (w, h);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("W");
        changed |= ui.add(egui::DragValue::new(&mut nw).range(4.0..=2000.0).speed(1.0).suffix(" pt")).drag_stopped();
        ui.label("H");
        changed |= ui.add(egui::DragValue::new(&mut nh).range(4.0..=2000.0).speed(1.0).suffix(" pt")).drag_stopped();
    });
    if changed || (nw - w).abs() > 0.5 || (nh - h).abs() > 0.5 {
        run(g, "arrange.bounds", json!({"width": nw, "height": nh}));
    }
    let on_page = float.wrap != Wrap::Inline && float.h_rel == Anchor::Page && float.v_rel == Anchor::Page;
    label(ui, t, "Position on the page");
    if on_page {
        let (mut x, mut y) = (float.x, float.y);
        ui.horizontal(|ui| {
            ui.label("X");
            let a = ui.add(egui::DragValue::new(&mut x).range(-2000.0..=4000.0).speed(1.0).suffix(" pt"));
            ui.label("Y");
            let b = ui.add(egui::DragValue::new(&mut y).range(-2000.0..=4000.0).speed(1.0).suffix(" pt"));
            if (a.changed() || b.changed()) && (a.drag_stopped() || b.drag_stopped() || !(a.dragged() || b.dragged())) {
                run(g, "arrange.bounds", json!({"x": x, "y": y, "page": g.app.session.page_hint}));
            }
        });
    } else if crate::widgets::quiet_button(ui, t, "Place freely on the page", None, 28.0).clicked() {
        run(g, "arrange.wrap", json!({"wrap": "inFrontOfText"}));
        run(g, "arrange.bounds", json!({"x": 72, "y": 72, "page": g.app.session.page_hint}));
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for (name, v) in [("Left", "topLeft"), ("Centre", "middleCenter"), ("Right", "topRight"), ("Bottom", "bottomCenter")] {
            if crate::widgets::quiet_button(ui, t, name, None, 26.0).on_hover_text("Align within the margins").clicked() {
                run(g, "arrange.position", json!({"preset": v}));
            }
        }
    });
    label(ui, t, "Rotate");
    let mut deg = float.rot;
    if ui.add(egui::Slider::new(&mut deg, 0.0..=359.0).suffix("°")).drag_stopped() {
        run(g, "arrange.rotation", json!({"degrees": deg}));
    }
    label(ui, t, "Text wrap");
    let wraps = [
        ("In line", "inline", Wrap::Inline),
        ("Around", "square", Wrap::Square),
        ("Behind", "behindText", Wrap::BehindText),
        ("In front", "inFrontOfText", Wrap::InFrontOfText),
    ];
    let cur = wraps.iter().position(|w| w.2 == float.wrap).unwrap_or(1);
    let names: Vec<&str> = wraps.iter().map(|w| w.0).collect();
    if let Some(i) = crate::widgets::segmented(ui, t, &names, cur, 28.0)
        && let Some(w) = wraps.get(i)
    {
        run(g, "arrange.wrap", json!({"wrap": w.1}));
    }
    label(ui, t, "Order");
    ui.horizontal(|ui| {
        if crate::widgets::quiet_button(ui, t, "Bring to Front", None, 28.0).clicked() {
            run(g, "arrange.bringForward", json!({}));
        }
        if crate::widgets::quiet_button(ui, t, "Send to Back", None, 28.0).clicked() {
            run(g, "arrange.sendBackward", json!({}));
        }
    });
    ui.add_space(6.0);
    if crate::widgets::quiet_button(ui, t, "Delete", None, 28.0).clicked() {
        run(g, "edit.delete", json!({}));
    }
}

/// Text formatting, for the text at the caret (or in the selected text box).
fn text_tab(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    label(ui, t, "Size");
    ui.horizontal_wrapped(|ui| {
        for s in [11, 14, 18, 24, 36, 48, 72] {
            if crate::widgets::quiet_button(ui, t, &s.to_string(), Some(38.0), 28.0).clicked() {
                run(g, "format.size", json!({"size": s}));
            }
        }
    });
    label(ui, t, "Style");
    ui.horizontal(|ui| {
        for (icon, tip, id) in
            [("bold", "Bold", "format.bold"), ("italic", "Italic", "format.italic"), ("underline", "Underline", "format.underline")]
        {
            if crate::widgets::icon_capsule(ui, t, icon, tip, 30.0, false, true).clicked() {
                run(g, id, json!({}));
            }
        }
        ui.add_space(8.0);
        for (icon, tip, a) in [("alignLeft", "Left", "left"), ("alignCenter", "Centre", "center"), ("alignRight", "Right", "right")] {
            if crate::widgets::icon_capsule(ui, t, icon, tip, 30.0, false, true).clicked() {
                run(g, "para.align", json!({"value": a}));
            }
        }
    });
    label(ui, t, "Colour");
    if let Some(Some(c)) = swatches(ui, t, None, false) {
        run(g, "format.color", json!({"color": hex(c)}));
    }
    label(ui, t, "Font");
    ui.horizontal_wrapped(|ui| {
        for f in ["Georgia", "Aptos", "Helvetica Neue", "Courier New"] {
            if crate::widgets::quiet_button(ui, t, f, None, 28.0).clicked() {
                run(g, "format.font", json!({"name": f}));
            }
        }
    });
}

/// Insert an object with `cmd` at the caret, then float it in front of the text near the top
/// of the page in view, so it can be moved anywhere.
pub fn add_floating(g: &mut GlassApp, cmd: &str, params: Value, (w, h): (f32, f32)) {
    let (start, _) = g.app.session.sel.ordered();
    g.app.session.sel = Selection::caret(start.clone());
    if let Err(e) = g.app.run(cmd, params) {
        g.app.status(e);
        return;
    }
    let end = wordcraft_doc::Pos { off: start.off + OBJ.len_utf8(), ..start.clone() };
    if g.app.session.doc.para_at(&start).and_then(|p| p.object_at(start.off)).is_none() {
        return;
    }
    g.app.session.sel = Selection { anchor: start, focus: end };
    let page = g.app.session.page_hint;
    let s = &g.app.session.doc.last_section;
    let (x, y) = (((s.page_w - w) / 2.0).max(0.0), ((s.page_h - h) * 0.3).max(0.0));
    run(g, "arrange.wrap", json!({"wrap": "inFrontOfText"}));
    run(g, "arrange.bounds", json!({"x": x, "y": y, "page": page}));
}

/// Add a drawn picture to the page, behind the text, across the page's width.
fn add_art(g: &mut GlassApp, art: Art) {
    let (pw, ph) = (900, 560);
    let Ok(png) = wordcraft_ui_egui::paste_picture::png_from_rgba(pw, ph, crate::templates::draw(art, pw, ph)) else { return };
    let data = wordcraft_engine::cmd::insert::base64_encode(&png);
    let w = g.app.session.doc.last_section.page_w;
    let h = w * ph as f32 / pw as f32;
    add_floating(g, "insert.picture", json!({"data": data, "width": w, "alt": "Drawn picture"}), (w, h));
    run(g, "arrange.wrap", json!({"wrap": "behindText"}));
}

/// A small preview of `art` (cached).
fn art_texture(ctx: &egui::Context, art: Art) -> egui::TextureHandle {
    let id = egui::Id::new(("glass_art_preview", format!("{art:?}")));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let (w, h) = (148, 88);
    let px = crate::templates::draw(art, w, h);
    let tex = ctx.load_texture(format!("glass_art_{art:?}"), egui::ColorImage::from_rgba_unmultiplied([w, h], &px), egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

/// Whether the page layout tools belong with the open document (a page layout, or the
/// inspector turned on).
pub fn wanted(g: &GlassApp) -> bool {
    g.glass.format_open && !g.is_db_tab(g.docs.active)
}

/// Turn the Format inspector on or off (and remember the document as a page layout).
pub fn toggle(g: &mut GlassApp) {
    g.glass.format_open = !g.glass.format_open;
    let id = g.app.session.document_id();
    if g.glass.format_open {
        g.glass.layout_docs.insert(id);
    }
    let _ = (radius::ROW, ShapeKind::Rectangle);
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_engine::Session;
    use wordcraft_ui_egui::{Services, WordApp};

    #[test]
    fn objects_are_added_in_front_of_the_text_and_can_be_arranged() {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default()));
        add_floating(&mut g, "insert.shape", json!({"kind": "ellipse", "width": 100, "height": 100}), (100.0, 100.0));
        let o = selected_object(&g).expect("the new shape is selected");
        match o {
            InlineObject::Shape { float, .. } => assert_eq!(float.wrap, Wrap::InFrontOfText),
            other => panic!("{other:?}"),
        }
        run(&mut g, "shape.fill", json!({"color": "E07A5F"}));
        add_art(&mut g, Art::Waves);
        assert!(!g.app.session.doc.media.is_empty());
        add_floating(&mut g, "insert.textBox", json!({"text": "Hello"}), (144.0, 72.0));
        assert!(g.app.session.word_count() >= 1);
    }
}

//! Glass controls: capsule icon buttons, ribbon buttons and menus, segmented controls.
//!
//! Every button runs a WordCraft command by id through [`WordApp::run`], exactly like the original
//! ribbon, so shortcuts, undo, dialogs for missing input and the Quick Access Toolbar menu all
//! keep working.

use egui::{Align2, Rect, Response, Sense, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_ui_egui::theme::{medium, regular};
use wordcraft_ui_egui::{WordApp, icons, widgets as wc};

use crate::tokens::{Tokens, paint_fill, paint_pill, radius};

/// Parse a JSON params literal from the ribbon tables (an empty or bad literal is `{}`).
pub fn params(s: &str) -> Value {
    if s.is_empty() { json!({}) } else { serde_json::from_str(s).unwrap_or_else(|_| json!({})) }
}

/// Run a command the user clicked, reporting a failure in the status bar instead of failing
/// silently. Copy and Cut also put the text on the system clipboard, as the original ribbon does.
pub fn run(ui: &Ui, app: &mut WordApp, id: &str, p: Value) {
    match app.run(id, p) {
        Ok(_) => {
            if matches!(id, "edit.copy" | "edit.cut") {
                ui.ctx().copy_text(app.session.clipboard_text.clone());
            }
        }
        Err(e) => app.status(e),
    }
}

/// The tooltip for a command: its label (or `tip`) and its shortcut.
fn tooltip(app: &WordApp, resp: Response, tip: &str, id: &str) -> Response {
    let sc = wc::shortcut_text(app, id);
    if sc.is_empty() { resp.on_hover_text(tip) } else { resp.on_hover_text(format!("{tip} ({sc})")) }
}

/// A round, icon-only capsule button (title bar, status bar). Returns the response; the caller
/// decides what a click does.
pub fn icon_capsule(ui: &mut Ui, t: &Tokens, icon: &str, tip: &str, size: f32, on: bool, enabled: bool) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    if enabled {
        crate::tokens::paint_state(ui, r, radius::CAPSULE, &resp, on, t);
    }
    let c = if !enabled {
        t.text_disabled
    } else if on {
        t.accent
    } else {
        t.text
    };
    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(size * 0.52, size * 0.52)), icon, c, if enabled { t.accent } else { c });
    resp.on_hover_text(tip)
}

/// A capsule icon button that runs a command.
pub fn command_capsule(ui: &mut Ui, app: &mut WordApp, t: &Tokens, icon: &str, tip: &str, id: &str, size: f32) -> Response {
    let enabled = wc::enabled(app, id);
    let resp = icon_capsule(ui, t, icon, tip, size, false, enabled);
    let resp = tooltip(app, resp, tip, id);
    wc::command_menu(&resp, app, id);
    if resp.clicked() && enabled {
        run(ui, app, id, json!({}));
    }
    resp
}

/// Height of a large ribbon button.
pub const BIG_H: f32 = 70.0;
/// Height of one small ribbon button (three stack in a column).
pub const SMALL_H: f32 = 22.0;

/// A large ribbon button: 24 pt icon over its label. `menu` adds a chevron and leaves the click to
/// the caller (which opens the menu).
#[allow(clippy::too_many_arguments)]
pub fn big(ui: &mut Ui, app: &mut WordApp, t: &Tokens, icon: &str, label: &str, id: &str, p: Value, menu: bool, on: bool) -> Response {
    let enabled = wc::enabled(app, id) || id == "ui.dialog";
    let font = regular(11.0);
    let lines: Vec<&str> = label.split('\n').collect();
    let text_w = lines.iter().map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap((*l).to_string(), font.clone(), t.text).size().x)).fold(0.0, f32::max);
    let w = (text_w + if menu { 22.0 } else { 14.0 }).max(50.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, BIG_H), Sense::click());
    if enabled {
        crate::tokens::paint_state(ui, r, radius::BUTTON, &resp, on, t);
    }
    let (c, a) = if enabled { (t.text, t.accent) } else { (t.text_disabled, t.text_disabled) };
    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 19.0), vec2(26.0, 26.0)), icon, c, a);
    let mut y = r.min.y + 37.0;
    for (i, l) in lines.iter().enumerate() {
        let txt = if menu && i + 1 == lines.len() { format!("{l} ⌄") } else { (*l).to_string() };
        ui.painter().text(pos2(r.center().x, y), Align2::CENTER_TOP, txt, font.clone(), c);
        y += 12.5;
    }
    let tip = label.replace('\n', " ");
    let resp = tooltip(app, resp, &tip, id);
    wc::command_menu(&resp, app, id);
    if resp.clicked() && enabled && !menu {
        run(ui, app, id, p);
    }
    resp
}

/// A small ribbon button: 16 pt icon and its label, in a column of three.
#[allow(clippy::too_many_arguments)]
pub fn small(ui: &mut Ui, app: &mut WordApp, t: &Tokens, icon: &str, label: &str, id: &str, p: Value, menu: bool, on: bool) -> Response {
    let enabled = wc::enabled(app, id) || id == "ui.dialog";
    let font = regular(11.5);
    let text = if menu { format!("{label} ⌄") } else { label.to_string() };
    let text_w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.clone(), font.clone(), t.text).size().x);
    let (r, resp) = ui.allocate_exact_size(vec2(text_w + 32.0, SMALL_H), Sense::click());
    if enabled {
        crate::tokens::paint_state(ui, r, 8, &resp, on, t);
    }
    let (c, a) = if enabled { (t.text, t.accent) } else { (t.text_disabled, t.text_disabled) };
    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 13.0, r.center().y), vec2(16.0, 16.0)), icon, c, a);
    ui.painter().text(pos2(r.min.x + 26.0, r.center().y), Align2::LEFT_CENTER, text, font, c);
    let resp = tooltip(app, resp, label, id);
    wc::command_menu(&resp, app, id);
    if resp.clicked() && enabled && !menu {
        run(ui, app, id, p);
    }
    resp
}

/// A segmented control: a quiet track with the selected segment raised on a pill. Returns the
/// index clicked this frame.
pub fn segmented(ui: &mut Ui, t: &Tokens, items: &[&str], selected: usize, height: f32) -> Option<usize> {
    let font = medium(12.5);
    let widths: Vec<f32> =
        items.iter().map(|s| ui.ctx().fonts_mut(|f| f.layout_no_wrap((*s).to_string(), font.clone(), t.text).size().x) + 22.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let (track, _) = ui.allocate_exact_size(vec2(total, height), Sense::hover());
    paint_fill(ui.painter(), track, radius::CAPSULE, t.fill);
    let mut x = track.min.x + 3.0;
    let mut clicked = None;
    // Two controls in one place can share a label ("Off"): tell them apart by all their items.
    let salt = items.join("|");
    for (i, (label, w)) in items.iter().zip(widths.iter()).enumerate() {
        let r = Rect::from_min_size(pos2(x, track.min.y + 3.0), vec2(*w, height - 6.0));
        let resp = ui.interact(r, ui.id().with(("segment", &salt, *label)), Sense::click());
        if i == selected {
            paint_pill(ui.painter(), r, t);
        } else if resp.hovered() {
            paint_fill(ui.painter(), r, radius::CAPSULE, t.hover);
        }
        let c = if i == selected { t.text } else { t.text_dim };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *label, font.clone(), c);
        if resp.clicked() {
            clicked = Some(i);
        }
        x += w;
    }
    clicked
}

/// A filled accent capsule button with a label (Share, Take Snapshot, Restore).
pub fn accent_button(ui: &mut Ui, t: &Tokens, icon: Option<&str>, label: &str, width: Option<f32>, height: f32) -> Response {
    let font = medium(12.5);
    let text_w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(label.to_string(), font.clone(), t.on_accent).size().x);
    let w = width.unwrap_or(text_w + if icon.is_some() { 42.0 } else { 28.0 });
    let (r, resp) = ui.allocate_exact_size(vec2(w, height), Sense::click());
    let fill = if resp.is_pointer_button_down_on() {
        t.accent.gamma_multiply(0.85)
    } else if resp.hovered() {
        t.accent.gamma_multiply(0.93)
    } else {
        t.accent
    };
    paint_fill(ui.painter(), r, radius::CAPSULE, fill);
    let content_w = text_w + if icon.is_some() { 22.0 } else { 0.0 };
    let mut x = r.center().x - content_w / 2.0;
    if let Some(icon) = icon {
        icons::paint(ui.painter(), Rect::from_min_size(pos2(x, r.center().y - 8.0), vec2(16.0, 16.0)), icon, t.on_accent, t.on_accent);
        x += 22.0;
    }
    ui.painter().text(pos2(x, r.center().y), Align2::LEFT_CENTER, label, font, t.on_accent);
    resp
}

/// A quiet capsule button with a label (Compare, Change Goal).
pub fn quiet_button(ui: &mut Ui, t: &Tokens, label: &str, width: Option<f32>, height: f32) -> Response {
    let font = medium(12.0);
    let text_w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(label.to_string(), font.clone(), t.text).size().x);
    let (r, resp) = ui.allocate_exact_size(vec2(width.unwrap_or(text_w + 26.0), height), Sense::click());
    let fill = if resp.is_pointer_button_down_on() {
        t.pressed
    } else if resp.hovered() {
        t.hover
    } else {
        t.fill
    };
    paint_fill(ui.painter(), r, radius::CAPSULE, fill);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, font, t.text);
    resp
}

/// A Mac-style switch. Flips `on` when clicked (the response is marked changed); the knob slides.
pub fn switch(ui: &mut Ui, t: &Tokens, on: &mut bool) -> Response {
    let (r, mut resp) = ui.allocate_exact_size(vec2(42.0, 24.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let a = ui.ctx().animate_bool_with_time(resp.id.with("switch"), *on, 0.18);
    let off = t.pressed;
    let track = if a <= 0.0 { off } else { lerp(off, t.accent, a) };
    ui.painter().rect_filled(r, egui::CornerRadius::same(12), track);
    let x = r.min.x + 12.0 + a * (r.width() - 24.0);
    let knob = egui::epaint::Shadow { offset: [0, 1], blur: 4, spread: 0, color: egui::Color32::from_black_alpha(50) };
    let kr = Rect::from_center_size(pos2(x, r.center().y), vec2(20.0, 20.0));
    ui.painter().add(knob.as_shape(kr, egui::CornerRadius::same(10)));
    ui.painter().circle_filled(kr.center(), 10.0, egui::Color32::WHITE);
    resp
}

fn lerp(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    egui::Color32::from_rgba_premultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), m(a.a(), b.a()))
}

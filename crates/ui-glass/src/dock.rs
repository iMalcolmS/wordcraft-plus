//! Side panels (Navigation, Binder, Versions, and Writer Pro's notes, Scratchpad and Pacing) share
//! one shell: a glass card at the window's edge that slides in and out, and that the writer can
//! make narrower or wider by dragging its inner edge (double-click the edge to reset).
//!
//! Panels sit beside the page by default. With **Floating panels** on (Appearance) they float
//! over the page as frosted glass instead, so the page keeps its full width and stays visible
//! around and faintly beneath them.

use egui::{Rect, Sense, Ui, pos2, vec2};

use crate::GlassApp;
use crate::tokens::{Tokens, frost_shape, glass_shape, radius};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// A side panel: its name (for its saved width), side and width range.
pub struct Dock {
    pub id: &'static str,
    pub side: Side,
    pub default: f32,
    pub min: f32,
    pub max: f32,
}

pub const NAVIGATION: Dock = Dock { id: "navigation", side: Side::Left, default: 250.0, min: 170.0, max: 440.0 };
pub const BINDER: Dock = Dock { id: "binder", side: Side::Left, default: 260.0, min: 180.0, max: 440.0 };
pub const VERSIONS: Dock = Dock { id: "versions", side: Side::Right, default: 310.0, min: 240.0, max: 480.0 };
pub const SIDE: Dock = Dock { id: "writer", side: Side::Right, default: 330.0, min: 250.0, max: 520.0 };

/// Space between a card and the window edge.
const EDGE: f32 = 12.0;
/// Padding inside a card.
const PAD: f32 = 14.0;

/// The width the writer chose for `d`, within its range.
pub fn width(g: &GlassApp, d: &Dock) -> f32 {
    g.glass.prefs.docks.get(d.id).copied().filter(|w| w.is_finite()).unwrap_or(d.default).clamp(d.min, d.max)
}

/// Show dock `d` (sliding to `expanded`), its content laid out by `add`. `area` is the window's
/// content area under the toolbar and ribbon, where floating panels go.
pub fn show(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, d: &Dock, expanded: bool, area: Rect, add: impl FnOnce(&mut GlassApp, &mut Ui)) {
    let w = width(g, d);
    if g.glass.prefs.floating {
        floating(g, ui.ctx(), t, d, expanded, area, w, add);
        return;
    }
    let mut open = expanded;
    let panel = match d.side {
        Side::Left => egui::Panel::left(format!("glass_dock_{}", d.id)),
        Side::Right => egui::Panel::right(format!("glass_dock_{}", d.id)),
    };
    let margin = match d.side {
        Side::Left => egui::Margin { left: EDGE as i8, right: 0, top: 2, bottom: 8 },
        Side::Right => egui::Margin { left: 0, right: EDGE as i8, top: 2, bottom: 8 },
    };
    panel
        .exact_size(w + EDGE)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(margin))
        .show_collapsible(ui, &mut open, |ui| {
            let card = ui.max_rect();
            ui.painter().add(glass_shape(card, radius::PANEL, t));
            content(g, ui, card, add);
            resize_handle(g, ui, t, d, card);
        });
}

/// Lay out a card's content inside it, clipped to it, never growing it.
fn content(g: &mut GlassApp, ui: &mut Ui, card: Rect, add: impl FnOnce(&mut GlassApp, &mut Ui)) {
    let inner = card.shrink(PAD);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(card.intersect(ui.clip_rect()));
    child.set_max_width(inner.width());
    add(g, &mut child);
}

#[allow(clippy::too_many_arguments)]
fn floating(
    g: &mut GlassApp,
    ctx: &egui::Context,
    t: &Tokens,
    d: &Dock,
    expanded: bool,
    area: Rect,
    w: f32,
    add: impl FnOnce(&mut GlassApp, &mut Ui),
) {
    let a = ctx.animate_bool_with_time(egui::Id::new(("glass_float", d.id)), expanded, 0.24);
    if a <= 0.0 {
        return;
    }
    if a < 1.0 {
        ctx.request_repaint();
    }
    // Ease out: fast at first, settling into place.
    let e = 1.0 - (1.0 - a).powi(3);
    let slide = (1.0 - e) * (w + 2.0 * EDGE);
    let top = area.min.y + 4.0;
    let bottom = (area.max.y - 8.0).max(top + 120.0);
    let x = match d.side {
        Side::Left => area.min.x + EDGE - slide,
        Side::Right => area.max.x - EDGE - w + slide,
    };
    let card = Rect::from_min_max(pos2(x, top), pos2(x + w, bottom));
    egui::Area::new(egui::Id::new(("glass_float_area", d.id))).order(egui::Order::Middle).fixed_pos(card.min).interactable(true).show(ctx, |ui| {
        ui.set_clip_rect(card.expand(40.0));
        ui.multiply_opacity(e);
        // The card takes the pointer, so clicks and scrolling don't reach the page beneath.
        // (Allocated, so the area itself covers the card: the page can't take presses under it.)
        let _ = ui.allocate_rect(card, Sense::click_and_drag());
        ui.painter().add(frost_shape(card, radius::PANEL, t));
        content(g, ui, card, add);
        resize_handle(g, ui, t, d, card);
    });
}

/// The card's inner edge: drag to resize (the width is saved), double-click to reset. A small
/// grip fades in when the pointer is near, as on macOS sidebars.
fn resize_handle(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, d: &Dock, card: Rect) {
    let x = match d.side {
        Side::Left => card.max.x,
        Side::Right => card.min.x,
    };
    let zone = Rect::from_min_max(pos2(x - 5.0, card.min.y + 24.0), pos2(x + 5.0, card.max.y - 24.0));
    let resp = ui.interact(zone, egui::Id::new(("glass_dock_resize", d.id)), Sense::click_and_drag());
    let active = resp.hovered() || resp.dragged();
    if active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    let shown = ui.ctx().animate_bool_with_time(resp.id.with("grip"), active, 0.15);
    if shown > 0.0 {
        let grip = Rect::from_center_size(pos2(x, card.center().y), vec2(4.0, 36.0 + 8.0 * shown));
        ui.painter().rect_filled(grip, 2.0, t.text_dim.gamma_multiply(0.55 * shown));
    }
    if resp.double_clicked() {
        g.glass.prefs.docks.remove(d.id);
        return;
    }
    if resp.dragged() {
        let dx = resp.drag_delta().x;
        let now = width(g, d);
        let next = match d.side {
            Side::Left => now + dx,
            Side::Right => now - dx,
        };
        g.glass.prefs.docks.insert(d.id.to_string(), next.clamp(d.min, d.max));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_engine::Session;
    use wordcraft_ui_egui::{Services, WordApp};

    #[test]
    fn widths_stay_in_range() {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_engine::doc::Document::new()), Services::default()));
        assert_eq!(width(&g, &NAVIGATION), NAVIGATION.default);
        g.glass.prefs.docks.insert("navigation".into(), 10.0);
        assert_eq!(width(&g, &NAVIGATION), NAVIGATION.min);
        g.glass.prefs.docks.insert("navigation".into(), f32::NAN);
        assert_eq!(width(&g, &NAVIGATION), NAVIGATION.default);
    }
}

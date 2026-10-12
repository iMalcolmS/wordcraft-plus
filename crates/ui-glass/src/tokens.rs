//! Design tokens for the Glass front end, and the glass material itself.
//!
//! Every colour, radius and spacing value the Glass UI draws with lives here, so the look can be
//! tuned in one place. The values come from the design mockup (see `DESIGN.md`).
//!
//! egui has no background blur, so the glass is drawn as a layered material instead: a soft drop
//! shadow, a translucent fill tinted by the user's Glass setting, a bright inner rim (the
//! specular highlight) and a thin dark outer edge (the darkened edge iOS 27 added for contrast).
//! "Reduce Transparency" swaps the material for a solid surface with a stronger outline.

use egui::epaint::{CornerRadius, Shadow};
use egui::{Color32, Painter, Rect, Stroke, StrokeKind};

/// Corner radii. Nested shapes are concentric: each inner radius is its container's radius minus
/// the padding between them, so the curves stay parallel.
pub mod radius {
    /// Capsules: title-bar items, tabs, status bar, buttons.
    pub const CAPSULE: u8 = 255;
    /// The ribbon card.
    pub const RIBBON: u8 = 26;
    /// Groups inside the ribbon (ribbon radius minus its 8 pt padding).
    pub const GROUP: u8 = 18;
    /// Buttons inside a group (group radius minus its 6 pt padding).
    pub const BUTTON: u8 = 12;
    /// Side panels (navigation, versions) and popovers.
    pub const PANEL: u8 = 24;
    /// Rows and cards inside a side panel.
    pub const ROW: u8 = 12;
}

/// Spacing scale (points).
pub mod space {
    /// Gap between the window edge and floating glass, and between glass elements.
    pub const EDGE: f32 = 10.0;
    /// Gap between items inside a capsule.
    pub const ITEM: f32 = 6.0;
    /// Inner padding of the ribbon card.
    pub const RIBBON_PAD: f32 = 8.0;
    /// Inner padding of a ribbon group.
    pub const GROUP_PAD: f32 = 6.0;
}

/// The default accent: a calm system blue.
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(0x0A, 0x6C, 0xDB);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x3B, 0x8E, 0xF0);

/// User-adjustable material settings, kept with the Glass preferences.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Material {
    /// 0.0 = clear, 1.0 = fully tinted (the iOS 27 Liquid Glass slider).
    pub tint: f32,
    /// Accessibility: solid surfaces instead of translucent glass.
    pub reduce_transparency: bool,
    /// The accent colour, as in System Settings › Appearance.
    pub accent: Accent,
    /// The tone of the window background the pages sit on.
    pub canvas: CanvasTone,
}

impl Default for Material {
    fn default() -> Self {
        Material { tint: 0.55, reduce_transparency: false, accent: Accent::Blue, canvas: CanvasTone::Neutral }
    }
}

/// Accent colours, as macOS offers them (slightly deepened so white text on them stays readable).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Accent {
    #[default]
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Green,
    Graphite,
}

impl Accent {
    pub const ALL: [Accent; 7] = [Accent::Blue, Accent::Purple, Accent::Pink, Accent::Red, Accent::Orange, Accent::Green, Accent::Graphite];

    pub fn name(self) -> &'static str {
        match self {
            Accent::Blue => "Blue",
            Accent::Purple => "Purple",
            Accent::Pink => "Pink",
            Accent::Red => "Red",
            Accent::Orange => "Orange",
            Accent::Green => "Green",
            Accent::Graphite => "Graphite",
        }
    }

    /// The colour in light or dark mode.
    pub fn color(self, dark: bool) -> Color32 {
        let (l, d) = match self {
            Accent::Blue => (ACCENT_LIGHT, ACCENT_DARK),
            Accent::Purple => (Color32::from_rgb(0x8E, 0x44, 0xC9), Color32::from_rgb(0xB3, 0x6B, 0xEA)),
            Accent::Pink => (Color32::from_rgb(0xD6, 0x33, 0x6C), Color32::from_rgb(0xF0, 0x62, 0x92)),
            Accent::Red => (Color32::from_rgb(0xD4, 0x3C, 0x33), Color32::from_rgb(0xF2, 0x68, 0x5F)),
            Accent::Orange => (Color32::from_rgb(0xC2, 0x62, 0x0A), Color32::from_rgb(0xF0, 0xA0, 0x4B)),
            Accent::Green => (Color32::from_rgb(0x24, 0x89, 0x4A), Color32::from_rgb(0x4C, 0xC2, 0x7A)),
            Accent::Graphite => (Color32::from_rgb(0x63, 0x63, 0x69), Color32::from_rgb(0xA0, 0xA0, 0xA6)),
        };
        if dark { d } else { l }
    }
}

/// The window background's tone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CanvasTone {
    #[default]
    Neutral,
    Warm,
    Cool,
    Paper,
}

impl CanvasTone {
    pub const ALL: [CanvasTone; 4] = [CanvasTone::Neutral, CanvasTone::Warm, CanvasTone::Cool, CanvasTone::Paper];

    pub fn name(self) -> &'static str {
        match self {
            CanvasTone::Neutral => "Neutral",
            CanvasTone::Warm => "Warm",
            CanvasTone::Cool => "Cool",
            CanvasTone::Paper => "Paper",
        }
    }

    pub fn color(self, dark: bool) -> Color32 {
        match (self, dark) {
            (CanvasTone::Neutral, false) => Color32::from_rgb(0xE6, 0xE7, 0xEB),
            (CanvasTone::Warm, false) => Color32::from_rgb(0xEC, 0xE7, 0xDF),
            (CanvasTone::Cool, false) => Color32::from_rgb(0xE1, 0xE6, 0xEE),
            (CanvasTone::Paper, false) => Color32::from_rgb(0xF2, 0xF0, 0xEA),
            (CanvasTone::Neutral, true) => Color32::from_rgb(0x12, 0x12, 0x14),
            (CanvasTone::Warm, true) => Color32::from_rgb(0x17, 0x14, 0x10),
            (CanvasTone::Cool, true) => Color32::from_rgb(0x0F, 0x13, 0x19),
            (CanvasTone::Paper, true) => Color32::from_rgb(0x1A, 0x1A, 0x18),
        }
    }
}

/// Black or white, whichever reads better on `c`.
fn readable_on(c: Color32) -> Color32 {
    let l = 0.2126 * f32::from(c.r()) + 0.7152 * f32::from(c.g()) + 0.0722 * f32::from(c.b());
    if l > 150.0 { Color32::from_rgb(0x11, 0x11, 0x14) } else { Color32::WHITE }
}

/// Every colour the Glass UI uses.
#[derive(Clone, Copy, Debug)]
pub struct Tokens {
    pub dark: bool,
    /// The window background the pages sit on.
    pub canvas: Color32,
    /// Glass fill, already tinted by [`Material::tint`].
    pub glass: Color32,
    /// Frosted glass for panels that float over the page: dense enough that the text behind
    /// reads as soft shapes, not words.
    pub frost: Color32,
    /// Inner rim (specular highlight) of glass shapes.
    pub rim: Color32,
    /// Thin outer edge of glass shapes.
    pub edge: Color32,
    pub shadow: Color32,
    /// Quiet fill for groups, fields and segmented controls on top of glass.
    pub fill: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    /// The raised pill of a selected tab or segment.
    pub pill: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_disabled: Color32,
    pub accent: Color32,
    /// The accent at low opacity: selected rows, toggled buttons.
    pub accent_soft: Color32,
    pub on_accent: Color32,
    /// White paper, for style-gallery cards.
    pub paper: Color32,
}

impl Tokens {
    pub fn new(dark: bool, m: Material) -> Tokens {
        let tint = m.tint.clamp(0.0, 1.0);
        if dark {
            let accent = m.accent.color(true);
            let glass = if m.reduce_transparency {
                Color32::from_rgb(0x26, 0x26, 0x2A)
            } else {
                Color32::from_rgba_unmultiplied(0x2E, 0x2E, 0x33, alpha(0.42 + tint * 0.50))
            };
            Tokens {
                dark,
                canvas: m.canvas.color(true),
                glass,
                frost: if m.reduce_transparency {
                    Color32::from_rgb(0x26, 0x26, 0x2A)
                } else {
                    Color32::from_rgba_unmultiplied(0x2A, 0x2A, 0x2F, alpha(0.80 + tint * 0.14))
                },
                rim: Color32::from_rgba_unmultiplied(255, 255, 255, if m.reduce_transparency { 0 } else { 30 }),
                edge: Color32::from_rgba_unmultiplied(0, 0, 0, if m.reduce_transparency { 255 } else { 150 }),
                shadow: Color32::from_rgba_unmultiplied(0, 0, 0, 110),
                fill: Color32::from_rgba_unmultiplied(255, 255, 255, 16),
                hover: Color32::from_rgba_unmultiplied(255, 255, 255, 26),
                pressed: Color32::from_rgba_unmultiplied(255, 255, 255, 40),
                pill: Color32::from_rgba_unmultiplied(255, 255, 255, 44),
                text: Color32::from_rgb(0xF2, 0xF2, 0xF4),
                text_dim: Color32::from_rgb(0xA6, 0xA6, 0xAC),
                text_disabled: Color32::from_rgb(0x66, 0x66, 0x6C),
                accent,
                accent_soft: Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 48),
                on_accent: readable_on(accent),
                paper: Color32::from_rgb(0x1E, 0x1E, 0x21),
            }
        } else {
            let accent = m.accent.color(false);
            let glass = if m.reduce_transparency {
                Color32::from_rgb(0xF4, 0xF4, 0xF6)
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, alpha(0.30 + tint * 0.62))
            };
            Tokens {
                dark,
                canvas: m.canvas.color(false),
                glass,
                frost: if m.reduce_transparency {
                    Color32::from_rgb(0xF4, 0xF4, 0xF6)
                } else {
                    Color32::from_rgba_unmultiplied(0xF7, 0xF7, 0xF9, alpha(0.78 + tint * 0.15))
                },
                rim: Color32::from_rgba_unmultiplied(255, 255, 255, if m.reduce_transparency { 0 } else { 230 }),
                edge: Color32::from_rgba_unmultiplied(0, 0, 0, if m.reduce_transparency { 60 } else { 34 }),
                shadow: Color32::from_rgba_unmultiplied(0, 0, 0, 28),
                fill: Color32::from_rgba_unmultiplied(0, 0, 0, 11),
                hover: Color32::from_rgba_unmultiplied(0, 0, 0, 18),
                pressed: Color32::from_rgba_unmultiplied(0, 0, 0, 30),
                pill: Color32::WHITE,
                text: Color32::from_rgb(0x1D, 0x1D, 0x1F),
                text_dim: Color32::from_rgb(0x5F, 0x5F, 0x65),
                text_disabled: Color32::from_rgb(0xA4, 0xA4, 0xAA),
                accent,
                accent_soft: Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 34),
                on_accent: readable_on(accent),
                paper: Color32::WHITE,
            }
        }
    }
}

fn alpha(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn corner(r: u8, rect: Rect) -> CornerRadius {
    // A capsule's radius is half its height; egui caps radii at 255.
    if r == radius::CAPSULE { CornerRadius::same((rect.height() / 2.0).clamp(0.0, 255.0) as u8) } else { CornerRadius::same(r) }
}

/// A piece of glass as one shape: shadow, tinted fill, inner rim highlight and outer edge. Useful
/// when the glass must sit behind content that is laid out first (reserve a shape slot, then set
/// it to this once the content's rect is known).
pub fn glass_shape(rect: Rect, r: u8, t: &Tokens) -> egui::Shape {
    glass_shape_with(rect, r, t, t.glass)
}

/// Glass that floats over the page itself (popovers): without a real blur, text behind it
/// would compete with its own, so it is nearly opaque whatever the Glass setting.
pub fn popover_shape(rect: Rect, r: u8, t: &Tokens) -> egui::Shape {
    let [r_, g_, b_, a_] = t.glass.to_srgba_unmultiplied();
    let fill = Color32::from_rgba_unmultiplied(r_, g_, b_, a_.max(252));
    glass_shape_with(rect, r, t, fill)
}

fn glass_shape_with(rect: Rect, r: u8, t: &Tokens, fill: Color32) -> egui::Shape {
    let cr = corner(r, rect);
    let shadow = Shadow { offset: [0, 6], blur: 24, spread: 0, color: t.shadow };
    egui::Shape::Vec(vec![
        shadow.as_shape(rect, cr).into(),
        egui::Shape::rect_filled(rect, cr, fill),
        egui::Shape::rect_stroke(rect.shrink(0.5), cr, Stroke::new(1.0, t.rim), StrokeKind::Inside),
        egui::Shape::rect_stroke(rect, cr, Stroke::new(0.75, t.edge), StrokeKind::Outside),
    ])
}

/// Frosted glass floating over the page: a deeper shadow, the frost fill, a bright rim, and a
/// faint sheen across the top, as light catches a pane.
pub fn frost_shape(rect: Rect, r: u8, t: &Tokens) -> egui::Shape {
    let cr = corner(r, rect);
    let shadow = Shadow { offset: [0, 10], blur: 34, spread: 0, color: t.shadow.gamma_multiply(1.4) };
    // Inset by the corner radius so the sheen never pokes past the rounded corners.
    let inset = f32::from(cr.nw).min(rect.width() / 2.0);
    let sheen = Rect::from_min_max(
        egui::pos2(rect.min.x + inset, rect.min.y + 1.0),
        egui::pos2(rect.max.x - inset, rect.min.y + 64.0_f32.min(rect.height())),
    );
    let mut mesh = egui::Mesh::default();
    let top = Color32::from_white_alpha(if t.dark { 10 } else { 70 });
    mesh.colored_vertex(sheen.left_top(), top);
    mesh.colored_vertex(sheen.right_top(), top);
    mesh.colored_vertex(sheen.right_bottom(), Color32::TRANSPARENT);
    mesh.colored_vertex(sheen.left_bottom(), Color32::TRANSPARENT);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    egui::Shape::Vec(vec![
        shadow.as_shape(rect, cr).into(),
        egui::Shape::rect_filled(rect, cr, t.frost),
        egui::Shape::mesh(mesh),
        egui::Shape::rect_stroke(rect.shrink(0.5), cr, Stroke::new(1.0, t.rim), StrokeKind::Inside),
        egui::Shape::rect_stroke(rect, cr, Stroke::new(0.75, t.edge), StrokeKind::Outside),
    ])
}

/// The font for small-caps section titles in menus.
pub fn small_caps() -> egui::FontId {
    wordcraft_ui_egui::theme::semibold(10.5)
}

/// Paint a piece of glass now.
pub fn paint_glass(p: &Painter, rect: Rect, r: u8, t: &Tokens) {
    p.add(glass_shape(rect, r, t));
}

/// A quiet fill on top of glass (groups, fields, segmented controls).
pub fn paint_fill(p: &Painter, rect: Rect, r: u8, color: Color32) {
    p.rect_filled(rect, corner(r, rect), color);
}

/// The raised pill a selected tab or segment sits on.
pub fn paint_pill(p: &Painter, rect: Rect, t: &Tokens) {
    let cr = corner(radius::CAPSULE, rect);
    let shadow = Shadow { offset: [0, 1], blur: 4, spread: 0, color: t.shadow };
    p.add(shadow.as_shape(rect, cr));
    p.rect_filled(rect, cr, t.pill);
}

/// Paint an interactive control's background for its state, animated: hover fades in and out
/// (120 ms), the "on" tint eases in (180 ms) and a press darkens at once, as macOS controls do.
pub fn paint_state(ui: &egui::Ui, rect: Rect, r: u8, resp: &egui::Response, on: bool, t: &Tokens) {
    let ctx = ui.ctx();
    let hover = ctx.animate_bool_with_time(resp.id.with("glass_hover"), resp.hovered(), 0.12);
    let lit = ctx.animate_bool_with_time(resp.id.with("glass_on"), on, 0.18);
    let cr = corner(r, rect);
    if lit > 0.0 {
        ui.painter().rect_filled(rect, cr, t.accent_soft.gamma_multiply(lit));
    }
    if hover > 0.0 {
        ui.painter().rect_filled(rect, cr, t.hover.gamma_multiply(hover));
    }
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, cr, t.pressed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_makes_glass_more_opaque() {
        let clear = Tokens::new(false, Material { tint: 0.0, reduce_transparency: false, ..Material::default() });
        let tinted = Tokens::new(false, Material { tint: 1.0, reduce_transparency: false, ..Material::default() });
        assert!(tinted.glass.a() > clear.glass.a());
    }

    #[test]
    fn reduce_transparency_is_solid() {
        for dark in [false, true] {
            let t = Tokens::new(dark, Material { tint: 0.0, reduce_transparency: true, ..Material::default() });
            assert_eq!(t.glass.a(), 255);
        }
    }

    #[test]
    fn out_of_range_tint_is_clamped() {
        let t = Tokens::new(false, Material { tint: 7.0, reduce_transparency: false, ..Material::default() });
        assert_eq!(t.glass.a(), alpha(0.92));
    }
}

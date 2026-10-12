//! Typewriter mode: the line being written stays near the middle of the window, and the rest of
//! the page fades softly towards the paper, so the eye rests on the sentence in progress.
//!
//! - **Centring**: when the caret moves (typing, arrow keys, a click), the page glides so the
//!   caret's line settles a little above the middle. It uses the canvas's own wheel scrolling (a
//!   fractional wheel step fed in before the canvas reads its input), so it only scrolls while
//!   the pointer rests over the page — exactly when a wheel would.
//! - **Fade**: drawn over the pages in the paper's own colour (white, or the dark page's grey),
//!   with soft edges, gliding with the caret instead of jumping line to line.

use egui::{Color32, Rect, Ui, pos2};
use wordcraft_ui_egui::{WordApp, canvas, now_ms};

/// How far the clear band reaches above and below the caret's line, in line heights.
const ABOVE: f32 = 1.3;
const BELOW: f32 = 2.3;
/// Where the caret's line settles, as a fraction of the window's height from the top.
const TARGET: f32 = 0.42;
/// How long after the caret moves the page keeps gliding towards it (seconds).
const GLIDE_SECS: f64 = 0.7;

/// The band left clear around a caret at `top` with line height `h`.
pub fn band(top: f32, h: f32) -> (f32, f32) {
    (top - h * ABOVE, top + h * BELOW)
}

/// Typewriter scrolling state.
#[derive(Default)]
pub struct Typewriter {
    /// The caret and revision last seen, and when they last changed.
    last: Option<(wordcraft_engine::doc::Pos, u64)>,
    moved_at: f64,
}

/// Before the canvas: glide the page so the caret's line moves towards the middle.
pub fn center(tw: &mut Typewriter, app: &mut WordApp, ui: &Ui) {
    let now = now_ms() / 1000.0;
    let key = (app.session.sel.focus.clone(), app.session.rev());
    if tw.last.as_ref() != Some(&key) {
        tw.last = Some(key);
        tw.moved_at = now;
    }
    if now - tw.moved_at > GLIDE_SECS {
        return;
    }
    let (Some(area), Some((caret, h))) = (app.canvas.canvas_rect, canvas::caret_screen(app)) else { return };
    let target = area.min.y + area.height() * TARGET;
    let diff = target - (caret.y + h / 2.0);
    if diff.abs() < 1.0 {
        return;
    }
    // Ease: a fraction of the remaining distance each frame.
    let step = diff * 0.22;
    let notch = wordcraft_ui_egui::scroll::notch_px(app.canvas.scale / wordcraft_ui_egui::canvas::PX_PER_PT).max(1.0);
    let mut lines = step / notch;
    // Whole steps are eased in by the canvas as wheel notches; a fractional step applies at once.
    if lines.fract() == 0.0 {
        lines += 0.001;
    }
    ui.ctx().input_mut(|i| {
        i.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, lines),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        })
    });
    ui.ctx().request_repaint();
}

/// After the canvas: fade everything but the lines around the caret.
pub fn overlay(app: &mut WordApp, ui: &Ui, strength: f32) {
    let Some(area) = app.canvas.canvas_rect else { return };
    let Some((caret, h)) = canvas::caret_screen(app) else { return };
    let h = h.clamp(6.0, 200.0);
    let ctx = ui.ctx().clone();
    // Glide the band relative to the canvas, so it follows the caret smoothly.
    let rel = caret.y - area.min.y;
    let rel = ctx.animate_value_with_time(egui::Id::new("glass_typewriter_band"), rel, 0.14);
    let (top, bottom) = band(area.min.y + rel, h);
    let paper = if app.session.view.dark_mode {
        let d = wordcraft_engine::render::DARK_PAPER;
        Color32::from_rgb(d, d, d)
    } else {
        Color32::WHITE
    };
    let veil = paper.gamma_multiply(0.78 * strength.clamp(0.0, 1.0));
    let clear = paper.gamma_multiply(0.0);
    let feather = (h * 1.2).clamp(12.0, 60.0);
    let painter = ui.painter().with_clip_rect(area);
    for page in app.canvas.page_rects.iter().filter(|r| r.intersects(area)) {
        let solid_top = top - feather;
        if solid_top > page.min.y {
            painter.rect_filled(Rect::from_min_max(page.min, pos2(page.max.x, solid_top.min(page.max.y))), 0.0, veil);
        }
        fade(
            &painter,
            Rect::from_min_max(pos2(page.min.x, solid_top.max(page.min.y)), pos2(page.max.x, top.clamp(page.min.y, page.max.y))),
            veil,
            clear,
        );
        let solid_bottom = bottom + feather;
        fade(
            &painter,
            Rect::from_min_max(pos2(page.min.x, bottom.clamp(page.min.y, page.max.y)), pos2(page.max.x, solid_bottom.min(page.max.y))),
            clear,
            veil,
        );
        if solid_bottom < page.max.y {
            painter.rect_filled(Rect::from_min_max(pos2(page.min.x, solid_bottom.max(page.min.y)), page.max), 0.0, veil);
        }
    }
}

/// A vertical gradient from `top` to `bottom` colour.
fn fade(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    if r.height() <= 0.0 || r.width() <= 0.0 {
        return;
    }
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bottom);
    mesh.colored_vertex(r.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    #[test]
    fn band_surrounds_the_caret() {
        let (a, b) = super::band(100.0, 20.0);
        assert!(a < 100.0 && b > 120.0);
    }
}

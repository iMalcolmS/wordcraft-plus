//! Typewriter mode: everything but the lines around the caret fades back, so the sentence being
//! written stands out. The overlay is drawn over the pages after the canvas paints them, so it
//! needs nothing from the canvas but the caret's position and the page rectangles.

use egui::{Color32, Rect, Ui, pos2};
use wordcraft_ui_egui::{WordApp, canvas};

/// How far the clear band reaches above and below the caret, in caret heights.
const ABOVE: f32 = 2.2;
const BELOW: f32 = 3.2;

/// The band left clear around a caret at `top` with line height `h`.
pub fn band(top: f32, h: f32) -> (f32, f32) {
    (top - h * ABOVE, top + h * BELOW)
}

pub fn overlay(app: &mut WordApp, ui: &Ui, strength: f32) {
    let Some(area) = app.canvas.canvas_rect else { return };
    let Some((caret, h)) = canvas::caret_screen(app) else { return };
    let h = h.clamp(6.0, 200.0);
    let (top, bottom) = band(caret.y, h);
    // Fade towards the paper: white on light pages, the dark paper in Switch Modes.
    let veil = if app.session.view.dark_mode {
        Color32::from_rgba_unmultiplied(18, 18, 20, 175)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 170)
    };
    let veil = veil.gamma_multiply(strength.clamp(0.0, 1.0));
    let painter = ui.painter().with_clip_rect(area);
    for page in app.canvas.page_rects.iter().filter(|r| r.intersects(area)) {
        if top > page.min.y {
            painter.rect_filled(Rect::from_min_max(page.min, pos2(page.max.x, top.min(page.max.y))), 0.0, veil);
        }
        if bottom < page.max.y {
            painter.rect_filled(Rect::from_min_max(pos2(page.min.x, bottom.max(page.min.y)), page.max), 0.0, veil);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn band_surrounds_the_caret() {
        let (a, b) = super::band(100.0, 20.0);
        assert!(a < 100.0 && b > 120.0);
    }
}

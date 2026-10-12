//! Small line icons drawn in code for Glass's own features (databases, views, labels): no icon
//! files, no third-party art. Each is drawn in a square `r` with strokes about 1/12 of its size,
//! in one colour, in the same rounded, open style as the rest of the interface.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

/// Every glyph, for pickers (labels and database icons).
pub const ALL: [&str; 24] = [
    "flag", "star", "heart", "bookmark", "tag", "circle", "pin", "bulb", "person", "people", "book", "flask", "cap", "check", "calendar", "clock",
    "globe", "pen", "folder", "database", "music", "film", "leaf", "bolt",
];

/// Glyphs offered for labels (colour coding).
pub const LABELS: [&str; 12] = ["flag", "star", "heart", "bookmark", "tag", "circle", "pin", "bulb", "person", "globe", "clock", "bolt"];

fn pt(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.min.x + x * r.width(), r.min.y + y * r.height())
}

fn poly(p: &Painter, r: Rect, pts: &[(f32, f32)], closed: bool, s: Stroke) {
    let v: Vec<Pos2> = pts.iter().map(|(x, y)| pt(r, *x, *y)).collect();
    if closed {
        p.add(Shape::closed_line(v, s));
    } else {
        p.add(Shape::line(v, s));
    }
}

fn filled(p: &Painter, r: Rect, pts: &[(f32, f32)], c: Color32) {
    let v: Vec<Pos2> = pts.iter().map(|(x, y)| pt(r, *x, *y)).collect();
    p.add(Shape::convex_polygon(v, c, Stroke::NONE));
}

/// Draw glyph `name` in `r`. Unknown names draw a dot.
pub fn paint(p: &Painter, r: Rect, name: &str, c: Color32) {
    let w = (r.width() / 11.0).clamp(1.0, 2.6);
    let s = Stroke::new(w, c);
    let cen = r.center();
    let u = r.width();
    match name {
        "flag" => {
            p.line_segment([pt(r, 0.25, 0.12), pt(r, 0.25, 0.9)], s);
            filled(p, r, &[(0.25, 0.14), (0.8, 0.14), (0.66, 0.33), (0.8, 0.52), (0.25, 0.52)], c);
        }
        "star" => {
            let mut v = Vec::new();
            for i in 0..10 {
                let a = std::f32::consts::PI * (i as f32) / 5.0 - std::f32::consts::FRAC_PI_2;
                let rad = if i % 2 == 0 { u * 0.44 } else { u * 0.19 };
                v.push(pos2(cen.x + rad * a.cos(), cen.y + 0.03 * u + rad * a.sin()));
            }
            // A star is not convex: fan it from the centre.
            let mut mesh = egui::Mesh::default();
            mesh.colored_vertex(pos2(cen.x, cen.y + 0.03 * u), c);
            for q in &v {
                mesh.colored_vertex(*q, c);
            }
            for i in 0..10u32 {
                mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
            }
            p.add(Shape::mesh(mesh));
        }
        "heart" => {
            p.circle_filled(pt(r, 0.34, 0.38), u * 0.18, c);
            p.circle_filled(pt(r, 0.66, 0.38), u * 0.18, c);
            filled(p, r, &[(0.17, 0.45), (0.83, 0.45), (0.5, 0.86)], c);
        }
        "bookmark" => filled(p, r, &[(0.27, 0.1), (0.73, 0.1), (0.73, 0.9), (0.5, 0.7), (0.27, 0.9)], c),
        "tag" => {
            poly(p, r, &[(0.12, 0.14), (0.52, 0.14), (0.88, 0.5), (0.52, 0.86), (0.12, 0.48)], true, s);
            p.circle_filled(pt(r, 0.32, 0.33), u * 0.07, c);
        }
        "circle" => {
            p.circle_filled(cen, u * 0.36, c);
        }
        "pin" => {
            p.circle_stroke(pt(r, 0.5, 0.38), u * 0.24, s);
            p.circle_filled(pt(r, 0.5, 0.38), u * 0.08, c);
            poly(p, r, &[(0.31, 0.53), (0.5, 0.9), (0.69, 0.53)], false, s);
        }
        "bulb" => {
            p.circle_stroke(pt(r, 0.5, 0.4), u * 0.26, s);
            p.line_segment([pt(r, 0.38, 0.76), pt(r, 0.62, 0.76)], s);
            p.line_segment([pt(r, 0.42, 0.88), pt(r, 0.58, 0.88)], s);
        }
        "person" => {
            p.circle_stroke(pt(r, 0.5, 0.32), u * 0.17, s);
            poly(p, r, &[(0.18, 0.9), (0.24, 0.66), (0.4, 0.57), (0.6, 0.57), (0.76, 0.66), (0.82, 0.9)], false, s);
        }
        "people" => {
            p.circle_stroke(pt(r, 0.36, 0.34), u * 0.14, s);
            poly(p, r, &[(0.1, 0.88), (0.15, 0.66), (0.28, 0.58), (0.44, 0.58), (0.57, 0.66), (0.62, 0.88)], false, s);
            p.circle_stroke(pt(r, 0.68, 0.3), u * 0.12, s);
            poly(p, r, &[(0.66, 0.52), (0.8, 0.54), (0.88, 0.62), (0.92, 0.8)], false, s);
        }
        "book" => {
            poly(
                p,
                r,
                &[
                    (0.5, 0.24),
                    (0.3, 0.16),
                    (0.1, 0.18),
                    (0.1, 0.8),
                    (0.3, 0.78),
                    (0.5, 0.86),
                    (0.7, 0.78),
                    (0.9, 0.8),
                    (0.9, 0.18),
                    (0.7, 0.16),
                    (0.5, 0.24),
                ],
                true,
                s,
            );
            p.line_segment([pt(r, 0.5, 0.24), pt(r, 0.5, 0.86)], s);
        }
        "flask" => {
            poly(p, r, &[(0.38, 0.1), (0.38, 0.4), (0.15, 0.84), (0.2, 0.9), (0.8, 0.9), (0.85, 0.84), (0.62, 0.4), (0.62, 0.1)], false, s);
            p.line_segment([pt(r, 0.32, 0.1), pt(r, 0.68, 0.1)], s);
            filled(p, r, &[(0.27, 0.62), (0.73, 0.62), (0.82, 0.84), (0.18, 0.84)], c.gamma_multiply(0.5));
        }
        "cap" => {
            filled(p, r, &[(0.5, 0.18), (0.95, 0.38), (0.5, 0.58), (0.05, 0.38)], c);
            poly(p, r, &[(0.24, 0.48), (0.24, 0.72), (0.5, 0.82), (0.76, 0.72), (0.76, 0.48)], false, s);
            p.line_segment([pt(r, 0.9, 0.4), pt(r, 0.9, 0.7)], s);
        }
        "check" => {
            p.rect_stroke(r.shrink(u * 0.12), u * 0.16, s, egui::StrokeKind::Middle);
            poly(p, r, &[(0.3, 0.5), (0.45, 0.65), (0.72, 0.36)], false, s);
        }
        "calendar" => {
            let b = Rect::from_min_max(pt(r, 0.12, 0.2), pt(r, 0.88, 0.88));
            p.rect_stroke(b, u * 0.1, s, egui::StrokeKind::Middle);
            p.line_segment([pt(r, 0.12, 0.38), pt(r, 0.88, 0.38)], s);
            p.line_segment([pt(r, 0.32, 0.1), pt(r, 0.32, 0.27)], s);
            p.line_segment([pt(r, 0.68, 0.1), pt(r, 0.68, 0.27)], s);
            for (x, y) in [(0.32, 0.55), (0.5, 0.55), (0.68, 0.55), (0.32, 0.72), (0.5, 0.72)] {
                p.circle_filled(pt(r, x, y), u * 0.045, c);
            }
        }
        "clock" => {
            p.circle_stroke(cen, u * 0.38, s);
            poly(p, r, &[(0.5, 0.26), (0.5, 0.5), (0.66, 0.6)], false, s);
        }
        "globe" => {
            p.circle_stroke(cen, u * 0.38, s);
            p.line_segment([pt(r, 0.12, 0.5), pt(r, 0.88, 0.5)], s);
            let e = Rect::from_center_size(cen, vec2(u * 0.36, u * 0.76));
            p.add(egui::epaint::EllipseShape::stroke(e.center(), e.size() / 2.0, s));
        }
        "pen" => {
            poly(p, r, &[(0.2, 0.8), (0.24, 0.62), (0.68, 0.18), (0.82, 0.32), (0.38, 0.76), (0.2, 0.8)], true, s);
            p.line_segment([pt(r, 0.6, 0.26), pt(r, 0.74, 0.4)], s);
        }
        "folder" => {
            poly(p, r, &[(0.1, 0.24), (0.38, 0.24), (0.46, 0.32), (0.9, 0.32), (0.9, 0.82), (0.1, 0.82)], true, s);
        }
        "database" => {
            let top = Rect::from_center_size(pt(r, 0.5, 0.24), vec2(u * 0.7, u * 0.22));
            p.add(egui::epaint::EllipseShape::stroke(top.center(), top.size() / 2.0, s));
            p.line_segment([pt(r, 0.15, 0.24), pt(r, 0.15, 0.76)], s);
            p.line_segment([pt(r, 0.85, 0.24), pt(r, 0.85, 0.76)], s);
            for y in [0.5, 0.76] {
                let a = pt(r, 0.15, y);
                let pts: Vec<Pos2> = (0..=12)
                    .map(|i| {
                        let t = i as f32 / 12.0 * std::f32::consts::PI;
                        pos2(a.x + u * 0.35 * (1.0 - t.cos()), a.y + u * 0.11 * t.sin())
                    })
                    .collect();
                p.add(Shape::line(pts, s));
            }
        }
        "music" => {
            p.circle_filled(pt(r, 0.3, 0.76), u * 0.12, c);
            p.circle_filled(pt(r, 0.74, 0.68), u * 0.12, c);
            poly(p, r, &[(0.4, 0.76), (0.4, 0.2), (0.84, 0.12), (0.84, 0.68)], false, s);
        }
        "film" => {
            p.rect_stroke(Rect::from_min_max(pt(r, 0.14, 0.14), pt(r, 0.86, 0.86)), u * 0.08, s, egui::StrokeKind::Middle);
            p.line_segment([pt(r, 0.32, 0.14), pt(r, 0.32, 0.86)], s);
            p.line_segment([pt(r, 0.68, 0.14), pt(r, 0.68, 0.86)], s);
        }
        "leaf" => {
            poly(p, r, &[(0.18, 0.84), (0.2, 0.5), (0.4, 0.24), (0.84, 0.14), (0.78, 0.56), (0.54, 0.78), (0.18, 0.84)], true, s);
            p.line_segment([pt(r, 0.18, 0.84), pt(r, 0.6, 0.4)], s);
        }
        "bolt" => filled(p, r, &[(0.56, 0.08), (0.2, 0.56), (0.48, 0.56), (0.44, 0.92), (0.8, 0.42), (0.52, 0.42)], c),
        // Views.
        "table" => {
            let b = r.shrink(u * 0.12);
            p.rect_stroke(b, u * 0.08, s, egui::StrokeKind::Middle);
            p.line_segment([pt(r, 0.12, 0.38), pt(r, 0.88, 0.38)], s);
            p.line_segment([pt(r, 0.12, 0.62), pt(r, 0.88, 0.62)], s);
            p.line_segment([pt(r, 0.4, 0.12), pt(r, 0.4, 0.88)], s);
        }
        "board" => {
            for (x, h) in [(0.12, 0.7), (0.4, 0.5), (0.68, 0.62)] {
                p.rect_stroke(Rect::from_min_size(pt(r, x, 0.14), vec2(u * 0.2, u * h)), u * 0.05, s, egui::StrokeKind::Middle);
            }
        }
        "gallery" => {
            for (x, y) in [(0.12, 0.12), (0.54, 0.12), (0.12, 0.54), (0.54, 0.54)] {
                p.rect_stroke(Rect::from_min_size(pt(r, x, y), vec2(u * 0.34, u * 0.34)), u * 0.06, s, egui::StrokeKind::Middle);
            }
        }
        "list" => {
            for y in [0.26, 0.5, 0.74] {
                p.circle_filled(pt(r, 0.18, y), u * 0.05, c);
                p.line_segment([pt(r, 0.32, y), pt(r, 0.88, y)], s);
            }
        }
        "layout" => {
            p.rect_stroke(Rect::from_min_max(pt(r, 0.14, 0.1), pt(r, 0.86, 0.9)), u * 0.06, s, egui::StrokeKind::Middle);
            filled(p, r, &[(0.24, 0.2), (0.76, 0.2), (0.76, 0.46), (0.24, 0.46)], c.gamma_multiply(0.55));
            p.line_segment([pt(r, 0.24, 0.6), pt(r, 0.76, 0.6)], s);
            p.line_segment([pt(r, 0.24, 0.74), pt(r, 0.6, 0.74)], s);
        }
        "doc" => {
            poly(p, r, &[(0.22, 0.1), (0.6, 0.1), (0.8, 0.3), (0.8, 0.9), (0.22, 0.9)], true, s);
            p.line_segment([pt(r, 0.34, 0.5), pt(r, 0.68, 0.5)], s);
            p.line_segment([pt(r, 0.34, 0.66), pt(r, 0.68, 0.66)], s);
        }
        "sidebar" => {
            p.rect_stroke(r.shrink(u * 0.12), u * 0.08, s, egui::StrokeKind::Middle);
            p.line_segment([pt(r, 0.38, 0.12), pt(r, 0.38, 0.88)], s);
        }
        "sort" => {
            poly(p, r, &[(0.3, 0.84), (0.3, 0.16), (0.14, 0.32)], false, s);
            poly(p, r, &[(0.7, 0.16), (0.7, 0.84), (0.86, 0.68)], false, s);
        }
        "swap" => {
            poly(p, r, &[(0.18, 0.34), (0.82, 0.34), (0.66, 0.18)], false, s);
            poly(p, r, &[(0.82, 0.66), (0.18, 0.66), (0.34, 0.82)], false, s);
        }
        "splitV" => {
            p.rect_stroke(r.shrink(u * 0.12), u * 0.08, s, egui::StrokeKind::Middle);
            p.line_segment([pt(r, 0.5, 0.12), pt(r, 0.5, 0.88)], s);
        }
        "splitH" => {
            p.rect_stroke(r.shrink(u * 0.12), u * 0.08, s, egui::StrokeKind::Middle);
            p.line_segment([pt(r, 0.12, 0.5), pt(r, 0.88, 0.5)], s);
        }
        _ => {
            p.circle_filled(cen, u * 0.2, c);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_glyph_draws() {
        let ctx = egui::Context::default();
        let p = egui::Painter::new(ctx, egui::LayerId::background(), egui::Rect::EVERYTHING);
        let r = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(16.0, 16.0));
        for g in
            super::ALL.iter().chain(&["table", "board", "gallery", "list", "layout", "doc", "nope", "sort", "sidebar", "swap", "splitV", "splitH"])
        {
            super::paint(&p, r, g, egui::Color32::BLACK);
            super::paint(&p, egui::Rect::NOTHING, g, egui::Color32::BLACK);
        }
    }
}

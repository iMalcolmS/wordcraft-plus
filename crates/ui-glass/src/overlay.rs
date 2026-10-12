//! What Glass draws on and around the page, over the editor's own canvas:
//!
//! - **Block handles** (Writer Pro): a grip beside the paragraph under the pointer. Drag it to move
//!   the paragraph — a frosted copy follows the pointer, a line shows where it will land, and the
//!   page scrolls when you near its edge. Click it to select the paragraph.
//! - **Reader's Glow**: everything but the paragraph (or sentence) you are writing fades back. The
//!   lit band glides from one to the next.
//! - **Pulse**: a soft glow when a paragraph changes style or lands after a move.
//! - **Rhythm marks**: a thin bar in the margin by dense paragraphs, and a gradient underline under
//!   words repeated close together.
//! - **Layout guides**: the page's margins, centre line and a baseline grid.
//!
//! All of it reads the layout through the editor's public API and edits only through commands.

use std::sync::Arc;

use egui::{Color32, Pos2, Rect, Sense, Ui, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::json;
use wordcraft_engine::Selection;
use wordcraft_engine::doc::{Path, Pos, StoryRef};
use wordcraft_engine::layout::DocLayout;
use wordcraft_ui_egui::now_ms;
use wordcraft_ui_egui::theme::{medium, regular};

use crate::tokens::{Tokens, frost_shape};
use crate::{GlassApp, View};

/// Reader's Glow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Glow {
    #[default]
    Off,
    Paragraph,
    Sentence,
}

/// A paragraph being dragged by its handle.
#[derive(Clone, Debug)]
pub struct BlockDrag {
    /// Its top-level block index.
    pub from: usize,
    /// Its opening words, shown on the copy that follows the pointer.
    pub text: String,
    /// Where it would land now (insert before this block).
    pub target: Option<usize>,
}

/// One rhythm mark: a dense paragraph, or a repeated word.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    Dense { block: usize, level: u8, tip: String },
    Repeat { a: Pos, b: Pos },
}

#[derive(Default)]
pub struct Overlay {
    pub drag: Option<BlockDrag>,
    /// A paragraph glowing (since when).
    pub pulse: Option<(Path, f64)>,
    /// The caret paragraph's style last frame (document, path, style).
    last_style: Option<(u64, Path, Option<String>)>,
    /// Rhythm marks for a (document, revision).
    marks: Option<((u64, u64), Arc<Vec<Mark>>)>,
    /// The block menu (Turn into…), open for a block, under a point.
    pub menu: Option<(usize, Pos2)>,
}

/// How long a pulse lasts (seconds).
const PULSE_SECS: f64 = 0.9;

// ---------------------------------------------------------------------------------------------
// Geometry: document ranges on screen.

/// Points per page point for page `i` on screen.
fn page_scale(rects: &[Rect], layout: &DocLayout, i: usize) -> Option<f32> {
    let r = rects.get(i)?;
    let p = layout.pages.get(i)?;
    (p.w > 0.0).then(|| r.width() / p.w)
}

fn to_screen(rects: &[Rect], layout: &DocLayout, page: usize, r: &wordcraft_geom::Rect) -> Option<Rect> {
    let pr = rects.get(page)?;
    let s = page_scale(rects, layout, page)?;
    Some(Rect::from_min_size(pos2(pr.min.x + r.x * s, pr.min.y + r.y * s), vec2(r.w * s, r.h * s)))
}

/// The screen rects (one per line piece) of the text from `a` to `b`, with their pages.
fn range_rects(g: &mut GlassApp, layout: &DocLayout, a: &Pos, b: &Pos) -> Vec<(usize, Rect)> {
    let rects = &g.app.canvas.page_rects;
    let mut out: Vec<(usize, Rect)> = layout
        .selection_rects(&g.app.session.doc, a, b, g.app.session.page_hint)
        .into_iter()
        .filter_map(|(p, r)| to_screen(rects, layout, p, &r).map(|s| (p, s)))
        .collect();
    if out.is_empty() {
        // An empty paragraph has no text to select: use its caret, to the body's right edge.
        if let Some(c) = layout.caret_on(a, g.app.session.page_hint)
            && let Some(page) = layout.pages.get(c.page)
        {
            let w = (page.body.x + page.body.w - c.x).max(8.0);
            let r = wordcraft_geom::Rect { x: c.x, y: c.top, w, h: c.height.max(6.0) };
            if let Some(s) = to_screen(rects, layout, c.page, &r) {
                out.push((c.page, s));
            }
        }
    }
    out
}

fn para_len(g: &GlassApp, path: &Path) -> Option<usize> {
    g.app.session.doc.para(StoryRef::Body, path).map(|p| p.text.len())
}

/// A top-level paragraph's lines on screen.
fn block_rects(g: &mut GlassApp, layout: &DocLayout, block: usize) -> Vec<(usize, Rect)> {
    let path = Path::top(block);
    let Some(len) = para_len(g, &path) else { return Vec::new() };
    range_rects(g, layout, &Pos::new(StoryRef::Body, path.clone(), 0), &Pos::new(StoryRef::Body, path, len))
}

/// Pages on screen now.
fn visible_pages(g: &GlassApp, clip: Rect) -> Vec<usize> {
    g.app.canvas.page_rects.iter().enumerate().filter(|(_, r)| r.intersects(clip)).map(|(i, _)| i).collect()
}

/// Top-level paragraphs on the pages on screen.
fn visible_blocks(g: &GlassApp, layout: &DocLayout, pages: &[usize]) -> Vec<usize> {
    let n = g.app.session.doc.story(StoryRef::Body).map(|b| b.len()).unwrap_or(0);
    let mut out = Vec::new();
    for &p in pages {
        let start = layout.pages.get(p).map(|x| x.first_block).unwrap_or(0);
        let end = layout.pages.get(p + 1).map(|x| x.first_block).unwrap_or(n.saturating_sub(1)).min(n.saturating_sub(1));
        for b in start..=end.max(start) {
            if b < n && !out.contains(&b) && g.app.session.doc.para(StoryRef::Body, &Path::top(b)).is_some() {
                out.push(b);
            }
        }
    }
    out
}

fn union(rs: &[(usize, Rect)], page: usize) -> Option<Rect> {
    rs.iter().filter(|(p, _)| *p == page).map(|(_, r)| *r).reduce(|a, b| a.union(b))
}

fn paper(g: &GlassApp) -> Color32 {
    if g.app.session.view.dark_mode {
        let d = wordcraft_engine::render::DARK_PAPER;
        Color32::from_rgb(d, d, d)
    } else {
        Color32::WHITE
    }
}

fn gradient_rect(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bottom);
    mesh.colored_vertex(r.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
}

fn hgradient(p: &egui::Painter, r: Rect, left: Color32, right: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), left);
    mesh.colored_vertex(r.right_top(), right);
    mesh.colored_vertex(r.right_bottom(), right);
    mesh.colored_vertex(r.left_bottom(), left);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
}

// ---------------------------------------------------------------------------------------------

/// Before the page is drawn: while a block is dragged near the page's top or bottom edge, scroll.
pub fn before_canvas(g: &mut GlassApp, ui: &mut Ui) {
    if g.glass.overlay.drag.is_none() {
        return;
    }
    let (Some(clip), Some(p)) = (g.app.canvas.canvas_rect, ui.input(|i| i.pointer.hover_pos())) else { return };
    let zone = 56.0;
    let speed = if p.y < clip.min.y + zone {
        (clip.min.y + zone - p.y) / zone
    } else if p.y > clip.max.y - zone {
        -(p.y - (clip.max.y - zone)) / zone
    } else {
        0.0
    };
    if speed != 0.0 {
        // Fed to the canvas as a fractional wheel step, which it applies at once.
        let notch = wordcraft_ui_egui::scroll::notch_px(g.app.canvas.scale / wordcraft_ui_egui::canvas::PX_PER_PT).max(1.0);
        let lines = speed.clamp(-1.0, 1.0) * 14.0 / notch + 0.001;
        ui.ctx().input_mut(|i| {
            i.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: vec2(0.0, lines),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            })
        });
        ui.ctx().request_repaint();
    }
}

/// After the page: everything above.
pub fn show(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let Some(clip) = g.app.canvas.canvas_rect else { return };
    if g.glass.view != View::Page || g.app.session.view.read_mode {
        g.glass.overlay.drag = None;
        return;
    }
    let layout = g.app.session.layout();
    let pages = visible_pages(g, clip);
    let painter = ui.painter().with_clip_rect(clip);
    track_style(g);
    if g.glass.prefs.grid {
        guides(g, &painter, &layout, &pages, t);
    }
    glow(g, ui, &painter, &layout, &pages);
    if g.glass.prefs.rhythm {
        rhythm(g, ui, &painter, &layout, &pages, t);
    }
    let blocks = visible_blocks(g, &layout, &pages);
    if g.glass.prefs.pro && g.glass.prefs.scrivenings {
        crate::scriv::section_boxes(g, &painter, &layout, &blocks, t, &mut |g, b| block_rects(g, &layout, b));
    }
    page_links(g, ui, &painter, &layout, &blocks, t);
    pulse(g, ui, &painter, &layout, t);
    if g.glass.prefs.handles {
        handles(g, ui, &painter, &layout, &pages, clip, t);
    } else {
        g.glass.overlay.drag = None;
    }
}

// ---------------------------------------------------------------------------------------------
// Pulse.

/// A paragraph that changes style (by any route: ribbon, shortcut, `/` command) glows briefly.
fn track_style(g: &mut GlassApp) {
    let focus = g.app.session.sel.focus.clone();
    if focus.story != StoryRef::Body {
        return;
    }
    let doc = g.app.session.document_id();
    let style = g.app.session.doc.para(StoryRef::Body, &focus.path).map(|p| p.props.style.clone());
    let Some(style) = style else { return };
    if let Some((d, path, old)) = &g.glass.overlay.last_style
        && *d == doc
        && *path == focus.path
        && *old != style
    {
        g.glass.overlay.pulse = Some((focus.path.clone(), now_ms() / 1000.0));
    }
    g.glass.overlay.last_style = Some((doc, focus.path, style));
}

fn pulse(g: &mut GlassApp, ui: &Ui, painter: &egui::Painter, layout: &DocLayout, t: &Tokens) {
    let Some((path, at)) = g.glass.overlay.pulse.clone() else { return };
    let age = now_ms() / 1000.0 - at;
    if !(0.0..PULSE_SECS).contains(&age) {
        g.glass.overlay.pulse = None;
        return;
    }
    let p = (age / PULSE_SECS) as f32;
    let fade = (1.0 - p).powi(2);
    let grow = 2.0 + 6.0 * (1.0 - (1.0 - p).powi(3));
    let Some(len) = para_len(g, &path) else { return };
    let rects = range_rects(g, layout, &Pos::new(StoryRef::Body, path.clone(), 0), &Pos::new(StoryRef::Body, path, len));
    let pages: Vec<usize> = rects.iter().map(|(p, _)| *p).collect();
    for page in pages {
        if let Some(r) = union(&rects, page) {
            let r = r.expand2(vec2(grow + 6.0, grow));
            painter.rect_filled(r, 8.0, t.accent.gamma_multiply(0.10 * fade));
            painter.rect_stroke(r, 8.0, egui::Stroke::new(1.5, t.accent.gamma_multiply(0.55 * fade)), egui::StrokeKind::Outside);
        }
    }
    ui.ctx().request_repaint();
}

// ---------------------------------------------------------------------------------------------
// Reader's Glow.

/// The sentence around byte `off` in `text`: from after the previous sentence end to the next
/// one (inclusive).
pub fn sentence_at(text: &str, off: usize) -> (usize, usize) {
    let off = off.min(text.len());
    let ends = |c: char| matches!(c, '.' | '!' | '?' | '…');
    let mut start = 0;
    let mut prev_end = false;
    for (i, c) in text.char_indices() {
        if i >= off {
            break;
        }
        if prev_end && c.is_whitespace() {
            start = i + c.len_utf8();
        }
        prev_end = ends(c) || (prev_end && matches!(c, '"' | '”' | '’' | '»' | ')'));
    }
    // Skip the spaces that open the sentence.
    while let Some(c) = text.get(start..).and_then(|s| s.chars().next()) {
        if c.is_whitespace() && start < off {
            start += c.len_utf8();
        } else {
            break;
        }
    }
    let mut end = text.len();
    let mut seen_end = false;
    for (i, c) in text.get(off..).unwrap_or("").char_indices() {
        if seen_end && c.is_whitespace() {
            end = off + i;
            break;
        }
        seen_end = ends(c) || (seen_end && matches!(c, '"' | '”' | '’' | '»' | ')'));
    }
    (start.min(end), end)
}

fn glow(g: &mut GlassApp, ui: &Ui, painter: &egui::Painter, layout: &DocLayout, pages: &[usize]) {
    let mode = g.glass.prefs.glow;
    let ctx = ui.ctx().clone();
    let strength = ctx.animate_bool_with_time(egui::Id::new("glass_glow_on"), mode != Glow::Off, 0.3);
    if strength <= 0.0 {
        return;
    }
    if strength < 1.0 {
        ctx.request_repaint();
    }
    let focus = g.app.session.sel.focus.clone();
    if focus.story != StoryRef::Body {
        return;
    }
    let Some(text) = g.app.session.doc.para(StoryRef::Body, &focus.path).map(|p| p.text.clone()) else { return };
    let (a, b) = match mode {
        Glow::Sentence => sentence_at(&text, focus.off),
        _ => (0, text.len()),
    };
    let rects = range_rects(g, layout, &Pos::new(StoryRef::Body, focus.path.clone(), a), &Pos::new(StoryRef::Body, focus.path.clone(), b));
    let Some(caret_page) = layout.caret_on(&focus, g.app.session.page_hint).map(|c| c.page) else { return };
    let on_page: Vec<Rect> = rects.iter().filter(|(p, _)| *p == caret_page).map(|(_, r)| *r).collect();
    let (Some(first), Some(last)) = (on_page.first().copied(), on_page.last().copied()) else { return };
    let Some(pr) = g.app.canvas.page_rects.get(caret_page).copied() else { return };
    let Some(s) = page_scale(&g.app.canvas.page_rects, layout, caret_page) else { return };
    // Glide in page coordinates, so scrolling never makes the band lag.
    let anim = |name: &str, v: f32| {
        let page_v = (v - pr.min.y) / s;
        pr.min.y + ctx.animate_value_with_time(egui::Id::new(("glass_glow", name, caret_page)), page_v, 0.18) * s
    };
    let animx = |name: &str, v: f32| {
        let page_v = (v - pr.min.x) / s;
        pr.min.x + ctx.animate_value_with_time(egui::Id::new(("glass_glow", name, caret_page)), page_v, 0.18) * s
    };
    let top = anim("top", first.min.y);
    let bottom = anim("bottom", last.max.y);
    let first_x = animx("fx", first.min.x);
    let first_bottom = anim("fb", first.max.y);
    let last_x = animx("lx", last.max.x);
    let last_top = anim("lt", last.min.y);
    let veil = paper(g).gamma_multiply(0.74 * strength);
    let clear = paper(g).gamma_multiply(0.0);
    let feather = 14.0;
    for &p in pages {
        let Some(r) = g.app.canvas.page_rects.get(p).copied() else { continue };
        if p != caret_page {
            painter.rect_filled(r, 0.0, veil);
            continue;
        }
        let t0 = (top - 4.0).max(r.min.y);
        let b0 = (bottom + 4.0).min(r.max.y);
        if t0 - feather > r.min.y {
            painter.rect_filled(Rect::from_min_max(r.min, pos2(r.max.x, t0 - feather)), 0.0, veil);
        }
        gradient_rect(painter, Rect::from_min_max(pos2(r.min.x, (t0 - feather).max(r.min.y)), pos2(r.max.x, t0)), veil, clear);
        gradient_rect(painter, Rect::from_min_max(pos2(r.min.x, b0), pos2(r.max.x, (b0 + feather).min(r.max.y))), clear, veil);
        if b0 + feather < r.max.y {
            painter.rect_filled(Rect::from_min_max(pos2(r.min.x, b0 + feather), r.max), 0.0, veil);
        }
        // Within the band: the line before the lit text starts, and after it ends.
        if first_x > r.min.x + 1.0 {
            painter.rect_filled(Rect::from_min_max(pos2(r.min.x, t0), pos2(first_x - 1.0, first_bottom)), 0.0, veil);
        }
        if last_x < r.max.x - 1.0 {
            painter.rect_filled(Rect::from_min_max(pos2(last_x + 1.0, last_top), pos2(r.max.x, b0)), 0.0, veil);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Rhythm marks.

const STOP: &[&str] = &[
    "that", "this", "with", "have", "from", "they", "were", "what", "when", "your", "will", "there", "their", "which", "about", "would", "could",
    "should", "into", "than", "then", "them", "been", "some", "more", "very", "just", "like", "also", "only", "over", "each", "other", "these",
    "those", "because", "where", "while", "para", "como", "mais", "pelo", "pela", "isso", "esse", "essa", "este", "esta", "quando", "porque",
    "muito", "ainda", "também", "sobre", "entre", "depois", "antes", "onde", "seus", "suas", "dele", "dela", "eles", "elas", "está", "estava",
    "foram", "seria", "tinha", "sido",
];

/// Words of `text` with their byte ranges (lowercased), letters and digits only.
fn words(text: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
        let word_char = c.is_alphanumeric() || c == '\'' || c == '’';
        match (start, word_char) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                if let Some(w) = text.get(s..i) {
                    out.push((s, i, w.trim_matches(|c| c == '\'' || c == '’').to_lowercase()));
                }
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// Rhythm marks for one paragraph's text: its density level, and repeated words' ranges.
pub fn paragraph_marks(text: &str) -> (u8, String, Vec<(usize, usize)>) {
    let ws = words(text);
    let n = ws.len();
    let sentences = text.split(['.', '!', '?', '…']).filter(|s| s.chars().any(char::is_alphanumeric)).count().max(1);
    let avg = n as f32 / sentences as f32;
    let level = if avg > 30.0 || n > 220 {
        2
    } else if avg > 22.0 || n > 150 {
        1
    } else {
        0
    };
    let tip = format!("{n} words · {sentences} sentences · {avg:.0} words per sentence");
    let mut flagged: Vec<String> = Vec::new();
    for (i, (_, _, w)) in ws.iter().enumerate() {
        if w.chars().count() < 4 || STOP.contains(&w.as_str()) || flagged.contains(w) {
            continue;
        }
        let count = ws.iter().filter(|x| x.2 == *w).count();
        let close = ws.iter().enumerate().skip(i + 1).take(10).any(|(_, x)| x.2 == *w);
        if count >= 3 || close {
            flagged.push(w.clone());
        }
    }
    let ranges: Vec<(usize, usize)> = ws.iter().filter(|x| flagged.contains(&x.2)).map(|x| (x.0, x.1)).take(16).collect();
    (level, tip, ranges)
}

fn marks(g: &mut GlassApp) -> Arc<Vec<Mark>> {
    let key = (g.app.session.document_id(), g.app.session.rev());
    if let Some((k, m)) = &g.glass.overlay.marks
        && *k == key
    {
        return m.clone();
    }
    let doc = &g.app.session.doc;
    let n = doc.story(StoryRef::Body).map(|b| b.len()).unwrap_or(0).min(800);
    let mut out = Vec::new();
    for b in 0..n {
        let path = Path::top(b);
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        let (level, tip, ranges) = paragraph_marks(&p.text);
        if !p.text.trim().is_empty() {
            out.push(Mark::Dense { block: b, level, tip });
        }
        for (a, z) in ranges {
            out.push(Mark::Repeat { a: Pos::new(StoryRef::Body, path.clone(), a), b: Pos::new(StoryRef::Body, path.clone(), z) });
        }
    }
    let m = Arc::new(out);
    g.glass.overlay.marks = Some((key, m.clone()));
    m
}

fn rhythm(g: &mut GlassApp, ui: &mut Ui, painter: &egui::Painter, layout: &DocLayout, pages: &[usize], t: &Tokens) {
    let all = marks(g);
    let blocks = visible_blocks(g, layout, pages);
    let warm = [Color32::from_rgb(0x34, 0xA8, 0x5A), Color32::from_rgb(0xE0, 0x9B, 0x2D), Color32::from_rgb(0xE0, 0x4F, 0x3F)];
    for m in all.iter() {
        match m {
            Mark::Dense { block, level, tip } if blocks.contains(block) => {
                let rects = block_rects(g, layout, *block);
                for p in rects.iter().map(|(p, _)| *p).collect::<std::collections::BTreeSet<_>>() {
                    let (Some(u), Some(page), Some(s), Some(pr)) =
                        (union(&rects, p), layout.pages.get(p), page_scale(&g.app.canvas.page_rects, layout, p), g.app.canvas.page_rects.get(p))
                    else {
                        continue;
                    };
                    let x = pr.min.x + page.body.x * s - 9.0;
                    let bar = Rect::from_min_max(pos2(x - 1.5, u.min.y + 2.0), pos2(x + 1.5, u.max.y - 2.0));
                    let c = warm.get(usize::from(*level)).copied().unwrap_or(t.accent);
                    painter.rect_filled(bar, 1.5, c.gamma_multiply(if *level == 0 { 0.28 } else { 0.75 }));
                    let hit = ui.interact(bar.expand2(vec2(5.0, 0.0)), egui::Id::new(("glass_rhythm", *block, p)), Sense::hover());
                    hit.on_hover_text(tip);
                }
            }
            Mark::Repeat { a, b } if blocks.contains(&a.path.last()) => {
                for (_, r) in range_rects(g, layout, a, b) {
                    let line = Rect::from_min_max(pos2(r.min.x, r.max.y - 1.5), pos2(r.max.x, r.max.y + 0.5));
                    hgradient(painter, line, t.accent.gamma_multiply(0.85), Color32::from_rgb(0xE0, 0x4F, 0x9B).gamma_multiply(0.85));
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Layout guides.

fn dashed(p: &egui::Painter, a: Pos2, b: Pos2, stroke: egui::Stroke) {
    p.extend(egui::Shape::dashed_line(&[a, b], stroke, 5.0, 4.0));
}

fn guides(g: &GlassApp, painter: &egui::Painter, layout: &DocLayout, pages: &[usize], t: &Tokens) {
    for &i in pages {
        let (Some(pr), Some(page), Some(s)) = (g.app.canvas.page_rects.get(i), layout.pages.get(i), page_scale(&g.app.canvas.page_rects, layout, i))
        else {
            continue;
        };
        let body = Rect::from_min_size(pos2(pr.min.x + page.body.x * s, pr.min.y + page.body.y * s), vec2(page.body.w * s, page.body.h * s));
        // A baseline grid every 12 pt inside the text area.
        let step = 12.0 * s;
        if step > 4.0 {
            let mut y = body.min.y + step;
            while y < body.max.y {
                painter.hline(body.x_range(), y, egui::Stroke::new(0.5, t.accent.gamma_multiply(0.07)));
                y += step;
            }
        }
        let st = egui::Stroke::new(1.0, t.accent.gamma_multiply(0.45));
        dashed(painter, pos2(body.min.x, pr.min.y), pos2(body.min.x, pr.max.y), st);
        dashed(painter, pos2(body.max.x, pr.min.y), pos2(body.max.x, pr.max.y), st);
        dashed(painter, pos2(pr.min.x, body.min.y), pos2(pr.max.x, body.min.y), st);
        dashed(painter, pos2(pr.min.x, body.max.y), pos2(pr.max.x, body.max.y), st);
        let mid = egui::Stroke::new(1.0, Color32::from_rgb(0xE0, 0x4F, 0x9B).gamma_multiply(0.35));
        dashed(painter, pos2(body.center().x, body.min.y), pos2(body.center().x, body.max.y), mid);
    }
}

// ---------------------------------------------------------------------------------------------
// Block handles.

/// Where a dragged block would land for a pointer at `p`: before the block under it if the
/// pointer is in its top half, else after it.
fn drop_target(g: &mut GlassApp, layout: &DocLayout, blocks: &[usize], p: Pos2) -> Option<(usize, Rect)> {
    let mut best: Option<(f32, usize, Rect)> = None;
    for &b in blocks {
        let rects = block_rects(g, layout, b);
        for page in rects.iter().map(|(p, _)| *p).collect::<std::collections::BTreeSet<_>>() {
            let Some(u) = union(&rects, page) else { continue };
            if p.x < u.min.x - 200.0 || p.x > u.max.x + 200.0 {
                continue;
            }
            let d = if p.y < u.min.y {
                u.min.y - p.y
            } else if p.y > u.max.y {
                p.y - u.max.y
            } else {
                0.0
            };
            if best.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
                best = Some((d, b, u));
            }
        }
    }
    let (_, b, u) = best?;
    let before = p.y < u.center().y;
    let y = if before { u.min.y - 4.0 } else { u.max.y + 4.0 };
    Some((if before { b } else { b + 1 }, Rect::from_min_max(pos2(u.min.x - 6.0, y - 1.0), pos2(u.max.x.max(u.min.x + 120.0), y + 1.0))))
}

/// Move top-level block `from` so it lands before block `target`. Returns its new index.
pub fn move_block(g: &mut GlassApp, from: usize, target: usize) -> Result<usize, String> {
    if target == from || target == from + 1 {
        return Ok(from);
    }
    let pos = Pos::new(StoryRef::Body, Path::top(from), 0);
    g.app.session.sel = Selection::caret(pos);
    let (id, steps) = if target < from { ("para.moveUp", from - target) } else { ("para.moveDown", target - from - 1) };
    let mut at = from;
    for _ in 0..steps {
        g.app.run(id, json!({}))?;
        at = if target < from { at - 1 } else { at + 1 };
    }
    Ok(at)
}

#[allow(clippy::too_many_arguments)]
fn handles(g: &mut GlassApp, ui: &mut Ui, painter: &egui::Painter, layout: &DocLayout, pages: &[usize], clip: Rect, t: &Tokens) {
    let ctx = ui.ctx().clone();
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let blocks = visible_blocks(g, layout, pages);
    // A drag in progress: the copy follows, the line shows where it lands.
    if let Some(mut drag) = g.glass.overlay.drag.clone() {
        let (down, esc) = ctx.input(|i| (i.pointer.primary_down(), i.key_pressed(egui::Key::Escape)));
        if esc {
            g.glass.overlay.drag = None;
            return;
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        // The paragraph being moved fades back where it was.
        for (_, r) in block_rects(g, layout, drag.from) {
            painter.rect_filled(r.expand(2.0), 4.0, paper(g).gamma_multiply(0.6));
        }
        if let Some(p) = pointer {
            let target = drop_target(g, layout, &blocks, p);
            drag.target = target.as_ref().map(|(b, _)| *b);
            if let Some((_, line)) = target {
                let y = crate::tokens::glide(&ctx, egui::Id::new("glass_drop_y"), line.center().y, 0.14);
                let line = Rect::from_center_size(pos2(line.center().x, y), line.size());
                painter.rect_filled(line, 1.0, t.accent);
                painter.circle_stroke(pos2(line.min.x - 3.0, y), 3.5, egui::Stroke::new(1.5, t.accent));
            }
            ghost(&ctx, &drag.text, p, t);
        }
        if down {
            g.glass.overlay.drag = Some(drag);
            ctx.request_repaint();
        } else {
            g.glass.overlay.drag = None;
            if let Some(target) = drag.target {
                match move_block(g, drag.from, target) {
                    Ok(at) => {
                        if at != drag.from {
                            g.glass.overlay.pulse = Some((Path::top(at), now_ms() / 1000.0));
                        }
                    }
                    Err(e) => g.app.status(format!("Couldn't move the paragraph there: {e}")),
                }
                g.app.canvas.want_focus = true;
            }
        }
        return;
    }
    // The paragraph under the pointer.
    let mut hovered: Option<(usize, Rect)> = None;
    if let Some(p) = pointer.filter(|p| clip.contains(*p)) {
        for &b in &blocks {
            let rects = block_rects(g, layout, b);
            if let Some((page, first)) = rects.first().copied() {
                let Some(u) = union(&rects, page) else { continue };
                let zone = Rect::from_min_max(pos2(u.min.x - 48.0, u.min.y - 2.0), pos2(u.max.x, u.max.y + 2.0));
                let body_left = layout
                    .pages
                    .get(page)
                    .zip(page_scale(&g.app.canvas.page_rects, layout, page))
                    .zip(g.app.canvas.page_rects.get(page))
                    .map(|((pg, s), pr)| pr.min.x + pg.body.x * s)
                    .unwrap_or(first.min.x);
                if zone.contains(p) {
                    let h = first.height().clamp(16.0, 26.0);
                    hovered = Some((
                        b,
                        Rect::from_min_size(pos2(first.min.x.min(body_left) - 36.0, first.min.y + (first.height() - h) / 2.0), vec2(18.0, h)),
                    ));
                    break;
                }
            }
        }
    }
    let shown = ctx.animate_bool_with_time(egui::Id::new("glass_handle_shown"), hovered.is_some(), 0.14);
    let Some((block, at)) = hovered else { return };
    let y = crate::tokens::glide(&ctx, egui::Id::new("glass_handle_y"), at.min.y, 0.16);
    let r = Rect::from_min_size(pos2(at.min.x, y), at.size());
    let resp = egui::Area::new(egui::Id::new("glass_block_handle"))
        .order(egui::Order::Middle)
        .fixed_pos(r.min)
        .show(&ctx, |ui| {
            ui.multiply_opacity(shown);
            let resp = ui.allocate_rect(r, Sense::click_and_drag());
            let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.1);
            if hover > 0.0 {
                ui.painter().rect_filled(r, 5.0, t.hover.gamma_multiply(hover * 1.5));
            }
            let c = if resp.hovered() { t.text } else { t.text_dim };
            for row in 0..3 {
                for col in 0..2 {
                    let p = pos2(r.center().x - 2.75 + col as f32 * 5.5, r.center().y - 5.0 + row as f32 * 5.0);
                    ui.painter().circle_filled(p, 1.3, c);
                }
            }
            resp
        })
        .inner;
    if resp.hovered() {
        ctx.set_cursor_icon(egui::CursorIcon::Grab);
    }
    let resp = resp.on_hover_text("Drag to move · Click to select");
    if resp.drag_started() {
        let text = g.app.session.doc.para(StoryRef::Body, &Path::top(block)).map(|p| p.plain_text()).unwrap_or_default();
        let text: String = text.trim().chars().take(140).collect();
        // Start the copy where the pointer is, then let it trail.
        if let Some(p) = pointer {
            ctx.animate_value_with_time(egui::Id::new("glass_ghost_x"), p.x, 0.0);
            ctx.animate_value_with_time(egui::Id::new("glass_ghost_y"), p.y, 0.0);
        }
        g.glass.overlay.drag = Some(BlockDrag { from: block, text, target: None });
    } else if resp.clicked() {
        // As in Notion: the handle's menu turns the block into something else.
        g.glass.overlay.menu = Some((block, pos2(r.max.x + 4.0, r.min.y)));
    }
}

/// Where a heading's section ends: the next heading at its level or above (top-level blocks).
fn section_end(g: &GlassApp, b: usize) -> usize {
    let doc = &g.app.session.doc;
    let n = doc.story(StoryRef::Body).map(|x| x.len()).unwrap_or(0);
    let level_of = |i: usize| doc.para(StoryRef::Body, &Path::top(i)).and_then(|p| doc.styles.resolve_para(&p.props).outline_level);
    match level_of(b) {
        Some(l) => (b + 1..n).find(|i| level_of(*i).is_some_and(|x| x <= l)).unwrap_or(n),
        None => b + 1,
    }
}

/// The URL a page link carries: the page's tab.
pub const PAGE_SCHEME: &str = "glass-page:";

/// Notion's "Turn into page": the block (a heading takes its whole section with it) moves to a
/// new document in its own tab, and a link to it takes its place. Undo brings the text back.
pub fn turn_into_page(g: &mut GlassApp, b: usize) -> Result<(), String> {
    let end = section_end(g, b);
    let doc = &g.app.session.doc;
    let title = doc.para(StoryRef::Body, &Path::top(b)).map(|p| p.plain_text().trim().to_string()).unwrap_or_default();
    let title = if title.is_empty() { "Untitled page".to_string() } else { title.chars().take(120).collect() };
    let last = end.checked_sub(1).ok_or("nothing to move")?;
    let last_len = doc.para(StoryRef::Body, &Path::top(last)).map(|p| p.text.len()).ok_or("a table ends this section: move it out first")?;
    // The new page: the same document, keeping only these blocks.
    let mut page = doc.clone();
    {
        let body = page.story_mut(StoryRef::Body).map_err(|e| e.to_string())?;
        let kept: Vec<_> = body.get(b..end).map(|x| x.to_vec()).unwrap_or_default();
        *body = kept;
    }
    page.core.title = title.clone();
    // Here: the blocks go, an empty paragraph stays for the link.
    g.app.session.sel = Selection { anchor: Pos::new(StoryRef::Body, Path::top(b), 0), focus: Pos::new(StoryRef::Body, Path::top(last), last_len) };
    if b != last || last_len > 0 {
        g.app.run("text.delete", json!({}))?;
    }
    g.app.session.sel = Selection::caret(Pos::new(StoryRef::Body, Path::top(b), 0));
    let _ = g.app.run("para.normal", json!({}));
    let uid = g.add_background_tab(wordcraft_engine::Session::new(page));
    g.app.run("insert.link", json!({"url": format!("{PAGE_SCHEME}{uid}"), "text": title}))?;
    g.glass.overlay.pulse = Some((Path::top(b), now_ms() / 1000.0));
    g.app.status(format!("“{title}” is now a page of its own — click its link to open it"));
    Ok(())
}

/// The page a link at `pos` opens, if it is a page link.
fn page_link_at(g: &GlassApp, pos: &Pos) -> Option<u64> {
    let para = g.app.session.doc.para_at(pos)?;
    for off in [pos.off, para.prev_boundary(pos.off)] {
        if off < para.text.len()
            && let Some(url) = para.props_of_char(off).link.as_deref()
            && let Some(uid) = url.strip_prefix(PAGE_SCHEME)
        {
            return uid.parse().ok();
        }
    }
    None
}

/// Page links: a page icon beside each, and a click opens the page.
fn page_links(g: &mut GlassApp, ui: &mut Ui, painter: &egui::Painter, layout: &DocLayout, blocks: &[usize], t: &Tokens) {
    for &b in blocks {
        let path = Path::top(b);
        let is_page = g
            .app
            .session
            .doc
            .para(StoryRef::Body, &path)
            .is_some_and(|p| (0..p.text.len().min(8)).any(|o| p.props_of_char(o).link.as_deref().is_some_and(|u| u.starts_with(PAGE_SCHEME))));
        if !is_page {
            continue;
        }
        if let Some((_, first)) = block_rects(g, layout, b).first().copied() {
            let r = Rect::from_center_size(pos2(first.min.x - 14.0, first.center().y), vec2(16.0, 16.0));
            crate::scriv::icon(painter, r, crate::scriv::Icon::Doc, t);
        }
    }
    // A plain click (not a drag, not a selection) on a page link opens it.
    let (released, dragged, p) = ui.input(|i| (i.pointer.primary_released(), i.pointer.is_decidedly_dragging(), i.pointer.interact_pos()));
    if released
        && !dragged
        && let Some(p) = p
        && g.app.canvas.canvas_rect.is_some_and(|c| c.contains(p))
        && let Some(pos) = wordcraft_ui_egui::canvas::pos_from_screen(&mut g.app, p)
        && let Some(uid) = page_link_at(g, &pos)
    {
        match g.docs.tabs.iter().position(|x| x.uid == uid) {
            Some(i) => g.switch_to(i),
            None => g.app.status("That page was closed. Pages you saved open from File › Open."),
        }
    }
}

/// The block menu: turn the block into another kind, into a page, or move, copy or delete it.
pub fn block_menu(g: &mut GlassApp, ctx: &egui::Context, t: &Tokens) {
    let Some((b, at)) = g.glass.overlay.menu else { return };
    let path = Path::top(b);
    let Some(len) = para_len(g, &path) else {
        g.glass.overlay.menu = None;
        return;
    };
    let heading =
        g.app.session.doc.para(StoryRef::Body, &path).is_some_and(|p| g.app.session.doc.styles.resolve_para(&p.props).outline_level.is_some());
    let fade = ctx.animate_bool_with_time(egui::Id::new(("glass_block_menu", b)), true, 0.12);
    let mut pick: Option<&str> = None;
    let area = egui::Area::new(egui::Id::new("glass_block_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(at.x, at.y - 4.0 * (1.0 - fade)))
        .show(ctx, |ui| {
            ui.multiply_opacity(fade);
            let bg = ui.painter().add(egui::Shape::Noop);
            let inner = egui::Frame::NONE.inner_margin(6).show(ui, |ui| {
                ui.set_width(220.0);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let section = |ui: &mut Ui, text: &str| {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
                    ui.painter().text(pos2(r.min.x + 10.0, r.center().y), egui::Align2::LEFT_CENTER, text, crate::tokens::small_caps(), t.text_dim);
                };
                let item = |ui: &mut Ui, glyph: &str, text: &str| -> bool {
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
                    crate::tokens::paint_state(ui, r, 7, &resp, false, t);
                    ui.painter().text(pos2(r.min.x + 18.0, r.center().y), egui::Align2::CENTER_CENTER, glyph, medium(12.0), t.text_dim);
                    ui.painter().text(pos2(r.min.x + 34.0, r.center().y), egui::Align2::LEFT_CENTER, text, regular(13.0), t.text);
                    resp.clicked()
                };
                section(ui, "TURN INTO");
                for (glyph, text, id) in [
                    ("T", "Text", "text"),
                    ("H1", "Heading 1", "h1"),
                    ("H2", "Heading 2", "h2"),
                    ("H3", "Heading 3", "h3"),
                    ("❝", "Quote", "quote"),
                    ("•", "Bulleted list", "bullets"),
                    ("1.", "Numbered list", "numbers"),
                    ("☐", "To-do", "todo"),
                ] {
                    if item(ui, glyph, text) {
                        pick = Some(id);
                    }
                }
                if item(ui, "↗", if heading { "Turn into page (with its section)" } else { "Turn into page" }) {
                    pick = Some("page");
                }
                ui.add_space(4.0);
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                ui.painter().rect_filled(r.shrink2(vec2(8.0, 0.0)), 0.0, t.edge.gamma_multiply(0.6));
                ui.add_space(4.0);
                if heading && item(ui, "↗", "Open in Quick Reference") {
                    pick = Some("quick");
                }
                for (glyph, text, id) in [
                    ("+", "Duplicate", "dup"),
                    ("↑", "Move up", "up"),
                    ("↓", "Move down", "down"),
                    ("≡", "Select", "select"),
                    ("⌫", "Delete", "delete"),
                ] {
                    if item(ui, glyph, text) {
                        pick = Some(id);
                    }
                }
            });
            ui.painter().set(bg, crate::tokens::popover_shape(inner.response.rect, 12, t));
        });
    if fade < 1.0 {
        ctx.request_repaint();
    }
    let outside = ctx.input(|i| {
        i.key_pressed(egui::Key::Escape) || (i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)))
    });
    let Some(pick) = pick else {
        if outside {
            g.glass.overlay.menu = None;
        }
        return;
    };
    g.glass.overlay.menu = None;
    let start = Pos::new(StoryRef::Body, path.clone(), 0);
    let end = Pos::new(StoryRef::Body, path.clone(), len);
    g.app.session.sel = Selection::caret(start.clone());
    let r: Result<(), String> = (|| match pick {
        "text" => g.app.run("para.normal", json!({})).map(|_| ()),
        "h1" => g.app.run("para.heading1", json!({})).map(|_| ()),
        "h2" => g.app.run("para.heading2", json!({})).map(|_| ()),
        "h3" => g.app.run("para.heading3", json!({})).map(|_| ()),
        "quote" => g.app.run("para.style", json!({"style": "Quote"})).map(|_| ()),
        "bullets" => g.app.run("para.bullets", json!({})).map(|_| ()),
        "numbers" => g.app.run("para.numbering", json!({})).map(|_| ()),
        "todo" => g.app.run("developer.checkBox", json!({})).map(|_| ()),
        "page" => turn_into_page(g, b),
        "quick" => {
            g.glass.scriv.quick = Some(path.clone());
            Ok(())
        }
        "up" => move_block(g, b, b.saturating_sub(1)).map(|_| ()),
        "down" => move_block(g, b, b + 2).map(|_| ()),
        "select" => {
            g.app.session.sel = Selection { anchor: start.clone(), focus: end.clone() };
            Ok(())
        }
        "dup" => {
            let style = g.app.session.doc.para(StoryRef::Body, &path).and_then(|p| p.props.style.clone());
            g.app.session.sel = Selection { anchor: start.clone(), focus: end.clone() };
            g.app.run("edit.copy", json!({}))?;
            g.app.session.sel = Selection::caret(end.clone());
            g.app.run("text.newParagraph", json!({}))?;
            if len > 0 {
                g.app.session.run("edit.paste", &json!({})).map_err(|e| e.to_string())?;
            }
            if let Some(st) = style {
                let _ = g.app.run("para.style", json!({"style": st}));
            }
            Ok(())
        }
        "delete" => {
            let n = g.app.session.doc.story(StoryRef::Body).map(|x| x.len()).unwrap_or(0);
            let next = Pos::new(StoryRef::Body, Path::top(b + 1), 0);
            let next_is_para = g.app.session.doc.para(StoryRef::Body, &Path::top(b + 1)).is_some();
            g.app.session.sel = if b + 1 < n && next_is_para {
                Selection { anchor: start.clone(), focus: next }
            } else {
                Selection { anchor: start.clone(), focus: end.clone() }
            };
            g.app.run("text.delete", json!({})).map(|_| ())
        }
        _ => Ok(()),
    })();
    if let Err(e) = r {
        g.app.status(e);
    } else if !matches!(pick, "delete" | "page" | "quick" | "select") {
        g.glass.overlay.pulse = Some((path, now_ms() / 1000.0));
    }
    g.app.canvas.want_focus = true;
}

/// The frosted copy of a dragged paragraph, trailing the pointer a little.
fn ghost(ctx: &egui::Context, text: &str, p: Pos2, t: &Tokens) {
    let x = ctx.animate_value_with_time(egui::Id::new("glass_ghost_x"), p.x + 16.0, 0.09);
    let y = ctx.animate_value_with_time(egui::Id::new("glass_ghost_y"), p.y - 12.0, 0.09);
    let lift = ctx.animate_bool_with_time(egui::Id::new("glass_ghost_lift"), true, 0.15);
    egui::Area::new(egui::Id::new("glass_block_ghost")).order(egui::Order::Tooltip).interactable(false).fixed_pos(pos2(x, y)).show(ctx, |ui| {
        ui.multiply_opacity(0.92 * lift);
        let bg = ui.painter().add(egui::Shape::Noop);
        let inner = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.set_max_width(320.0);
            let shown = if text.is_empty() { "Empty paragraph" } else { text };
            ui.add(egui::Label::new(egui::RichText::new(shown).font(regular(12.5)).color(t.text)).wrap().truncate());
            ui.label(egui::RichText::new("Drop to move").font(medium(10.5)).color(t.text_dim));
        });
        ui.painter().set(bg, frost_shape(inner.response.rect, 12, t));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_engine::Session;
    use wordcraft_ui_egui::{Services, WordApp};

    #[test]
    fn sentences_are_found_around_the_caret() {
        let text = "One two. Three four five! Six?";
        assert_eq!(&text[sentence_at(text, 0).0..sentence_at(text, 0).1], "One two.");
        let (a, b) = sentence_at(text, 12);
        assert_eq!(&text[a..b], "Three four five!");
        let (a, b) = sentence_at(text, text.len());
        assert_eq!(&text[a..b], "Six?");
        assert_eq!(sentence_at("", 5), (0, 0));
        let (a, b) = sentence_at("Olá. Ação…", 7);
        assert!("Olá. Ação…".get(a..b).is_some());
    }

    #[test]
    fn repeated_words_and_density_are_marked() {
        let (level, _, ranges) = paragraph_marks("The garden was quiet. The garden slept. A garden waits.");
        assert_eq!(level, 0);
        assert_eq!(ranges.len(), 3);
        let long = "word ".repeat(40) + ".";
        assert_eq!(paragraph_marks(&long).0, 2);
        assert!(paragraph_marks("").2.is_empty());
    }

    #[test]
    fn blocks_move_down_and_up() {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_engine::doc::Document::new()), Services::default()));
        for w in ["Alpha", "Beta", "Gamma", "Delta"] {
            g.app.run("text.insert", json!({"text": w})).unwrap();
            g.app.run("text.newParagraph", json!({})).unwrap();
        }
        let text = |g: &GlassApp, i: usize| g.app.session.doc.para(StoryRef::Body, &Path::top(i)).map(|p| p.plain_text()).unwrap_or_default();
        assert_eq!(move_block(&mut g, 0, 3).unwrap(), 2);
        assert_eq!((text(&g, 0), text(&g, 1), text(&g, 2)), ("Beta".into(), "Gamma".into(), "Alpha".into()));
        assert_eq!(move_block(&mut g, 3, 0).unwrap(), 0);
        assert_eq!(text(&g, 0), "Delta");
        assert_eq!(move_block(&mut g, 1, 2).unwrap(), 1);
    }
}

#[cfg(test)]
mod page_tests {
    use super::*;
    use wordcraft_engine::Session;
    use wordcraft_ui_egui::{Services, WordApp};

    #[test]
    fn a_heading_turns_into_a_page_with_its_section() {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_engine::sample::sample_document()), Services::default()));
        let secs = crate::project::sections(&g.app.session);
        let target = secs.iter().find(|s| s.level > secs.iter().map(|x| x.level).min().unwrap_or(0)).unwrap().clone();
        let b = target.path.last();
        let before = crate::project::sections(&g.app.session).len();
        turn_into_page(&mut g, b).unwrap();
        // The section left this document; a link to the page stands in its place.
        assert_eq!(crate::project::sections(&g.app.session).len(), before - 1);
        let p = g.app.session.doc.para(StoryRef::Body, &Path::top(b)).unwrap();
        assert!(p.props_of_char(0).link.as_deref().unwrap().starts_with(PAGE_SCHEME));
        // The page is a tab of its own, starting with the heading.
        assert_eq!(g.docs.tabs.len(), 2);
        let page = g.docs.stash_mut(1).unwrap();
        assert_eq!(page.doc.para(StoryRef::Body, &Path::top(0)).unwrap().plain_text().trim(), target.title);
        // Undo brings the section back.
        g.app.run("edit.undo", json!({})).unwrap();
        while crate::project::sections(&g.app.session).len() < before && g.app.run("edit.undo", json!({})).is_ok() {}
        assert_eq!(crate::project::sections(&g.app.session).len(), before);
    }
}

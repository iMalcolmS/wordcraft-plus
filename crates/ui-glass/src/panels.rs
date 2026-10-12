//! Side panels: Navigation (headings and pages) on the left, Snapshots and Version History on the
//! right. Both are floating glass cards inset from the window edge.

use egui::text::LayoutJob;
use egui::{Align2, Rect, Sense, Shape, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_engine::doc::{Document, Pos, StoryRef};
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::tokens::{Tokens, paint_fill, radius};
use crate::widgets as w;
use crate::{AUTO_LABEL, GlassApp, Panel};

/// A header: a title (or a segmented control) and a close button. Returns true when closed.
fn close_button(ui: &mut Ui, t: &Tokens) -> bool {
    w::icon_capsule(ui, t, "close", "Close", 26.0, false, true).clicked()
}

/// A selectable row: rounded, highlighted when `on`.
fn row(ui: &mut Ui, t: &Tokens, text: &str, indent: f32, bold: bool, on: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    crate::tokens::paint_state(ui, r, radius::ROW, &resp, on, t);
    let font = if bold || on { semibold(12.5) } else { regular(12.5) };
    let color = if on { t.accent } else { t.text };
    let max_chars = ((r.width() - indent - 20.0) / 6.5).max(4.0) as usize;
    let shown: String =
        if text.chars().count() > max_chars { format!("{}…", text.chars().take(max_chars).collect::<String>()) } else { text.to_string() };
    ui.painter().text(pos2(r.min.x + 12.0 + indent, r.center().y), Align2::LEFT_CENTER, shown, font, color);
    resp
}

/// The Navigation pane's views: page thumbnails, headings, comments, search.
const NAV_TABS: [(&str, &str, &str); 4] = [
    ("pages", "multiplePages", "Pages"),
    ("headings", "outline", "Headings"),
    ("comments", "showComments", "Comments"),
    ("search", "find", "Search"),
];

/// View › Navigation Pane, as glass: page thumbnails, headings, comments or search, picked by
/// icon tabs whose highlight glides between them.
pub fn navigation(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    if !NAV_TABS.iter().any(|(k, _, _)| *k == g.app.ui.nav_tab) {
        g.app.ui.nav_tab = "pages".into();
    }
    ui.horizontal(|ui| {
        let size = vec2(38.0, 30.0);
        let (track, _) = ui.allocate_exact_size(vec2(size.x * NAV_TABS.len() as f32 + 6.0, size.y + 6.0), Sense::hover());
        crate::tokens::paint_fill(ui.painter(), track, radius::CAPSULE, t.fill);
        let sel = NAV_TABS.iter().position(|(k, _, _)| *k == g.app.ui.nav_tab).unwrap_or(0);
        let x = crate::tokens::glide(ui.ctx(), egui::Id::new("glass_nav_pill"), sel as f32, 0.28);
        let pill = Rect::from_min_size(pos2(track.min.x + 3.0 + x * size.x, track.min.y + 3.0), size);
        crate::tokens::paint_pill(ui.painter(), pill, t);
        for (i, (key, icon, tip)) in NAV_TABS.iter().enumerate() {
            let r = Rect::from_min_size(pos2(track.min.x + 3.0 + i as f32 * size.x, track.min.y + 3.0), size);
            let resp = ui.interact(r, ui.id().with(("glass_nav_tab", *key)), Sense::click()).on_hover_text(*tip);
            let c = if i == sel {
                t.accent
            } else if resp.hovered() {
                t.text
            } else {
                t.text_dim
            };
            wordcraft_ui_egui::icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(18.0, 18.0)), icon, c, t.accent);
            if resp.clicked() {
                g.app.ui.nav_tab = (*key).into();
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if close_button(ui, t) {
                w::run(ui, &mut g.app, "view.navigationPane", json!({"value": false}));
            }
        });
    });
    ui.add_space(10.0);
    let tab = g.app.ui.nav_tab.clone();
    if tab == "search" {
        search(g, ui, t);
        return;
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
        match tab.as_str() {
            "pages" => pages(g, ui, t),
            "comments" => comments(g, ui, t),
            _ => headings(g, ui, t),
        }
    });
}

/// The document's comments: author, text, and a click goes to where each is anchored.
fn comments(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let list = g.app.session.run("review.comments", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    if list.is_empty() {
        hint(ui, t, "No comments. Select text and use Review › New Comment to add one.");
        return;
    }
    for c in list {
        let author = c.get("author").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let text = c.get("text").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        let resolved = c.get("resolved").and_then(|v| v.as_bool()).unwrap_or(false);
        let anchor: Option<Pos> = c.get("anchor").cloned().and_then(|a| serde_json::from_value(a).ok());
        let bg = ui.painter().add(Shape::Noop);
        let resp = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(if author.is_empty() { "Comment" } else { &author }).font(semibold(12.0)).color(t.text));
            ui.add(
                egui::Label::new(egui::RichText::new(clip(&text, 160)).font(regular(12.0)).color(if resolved { t.text_dim } else { t.text })).wrap(),
            );
        });
        let r = resp.response.rect;
        let click = ui.interact(r, ui.id().with(("glass_comment", r.min.y as i32)), Sense::click());
        let fill = if click.hovered() { t.hover } else { t.fill };
        ui.painter().set(bg, Shape::rect_filled(r, egui::CornerRadius::same(10), fill));
        if click.clicked()
            && let Some(p) = anchor
        {
            w::run(ui, &mut g.app, "caret.set", json!({ "pos": p }));
            g.app.canvas.scroll_to_caret = true;
            g.app.canvas.want_focus = true;
        }
        ui.add_space(4.0);
    }
}

/// Byte ranges in `text` where `query` occurs, ignoring case.
pub fn find_all(text: &str, query: &str) -> Vec<(usize, usize)> {
    let q: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if q.is_empty() {
        return Vec::new();
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut k = 0;
        let mut j = i;
        while k < q.len() && j < chars.len() {
            let Some(&(_, c)) = chars.get(j) else { break };
            let lc: Vec<char> = c.to_lowercase().collect();
            if q.get(k..k + lc.len()) != Some(lc.as_slice()) {
                break;
            }
            k += lc.len();
            j += 1;
        }
        if k == q.len() {
            let start = chars.get(i).map(|c| c.0).unwrap_or(0);
            let end = chars.get(j).map(|c| c.0).unwrap_or(text.len());
            out.push((start, end));
            i = j.max(i + 1);
        } else {
            i += 1;
        }
        if out.len() >= 500 {
            break;
        }
    }
    out
}

/// Search the document: every match with a snippet; a click selects it on the page.
fn search(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let qid = egui::Id::new("glass_nav_search");
    let mut q: String = ui.data(|d| d.get_temp(qid)).unwrap_or_default();
    let field = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Search document").desired_width(ui.available_width()).margin(vec2(10.0, 7.0)));
    if field.changed() {
        ui.data_mut(|d| d.insert_temp(qid, q.clone()));
    }
    ui.add_space(8.0);
    if q.trim().is_empty() {
        hint(ui, t, "Type to find words and phrases; each match shows with the text around it.");
        return;
    }
    let doc = &g.app.session.doc;
    let mut hits: Vec<(Pos, Pos, String)> = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        for (a, b) in find_all(&p.text, q.trim()) {
            let before: String = p.text.get(..a).unwrap_or("").chars().rev().take(28).collect::<Vec<_>>().into_iter().rev().collect();
            let hit = p.text.get(a..b).unwrap_or("");
            let after: String = p.text.get(b..).unwrap_or("").chars().take(40).collect();
            let clean = |s: &str| s.replace(wordcraft_engine::doc::para::OBJ, "");
            hits.push((
                Pos::new(StoryRef::Body, path.clone(), a),
                Pos::new(StoryRef::Body, path.clone(), b),
                format!("{}\u{1}{}\u{1}{}", clean(&before), clean(hit), clean(&after)),
            ));
            if hits.len() >= 300 {
                break;
            }
        }
    }
    ui.label(egui::RichText::new(format!("{} result{}", hits.len(), if hits.len() == 1 { "" } else { "s" })).font(semibold(11.5)).color(t.text_dim));
    ui.add_space(4.0);
    let mut go: Option<(Pos, Pos)> = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (a, b, snippet) in &hits {
            let mut parts = snippet.split('\u{1}');
            let (pre, hit, post) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            let mut job = LayoutJob::default();
            job.wrap.max_width = ui.available_width() - 20.0;
            job.wrap.max_rows = 2;
            let base = egui::TextFormat { font_id: regular(12.0), color: t.text_dim, ..Default::default() };
            job.append(pre.trim_start(), 0.0, base.clone());
            job.append(hit, 0.0, egui::TextFormat { color: t.text, background: t.accent_soft, font_id: semibold(12.0), ..Default::default() });
            job.append(post, 0.0, base);
            let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), galley.size().y + 14.0), Sense::click());
            crate::tokens::paint_state(ui, r, 8, &resp, false, t);
            ui.painter().galley(pos2(r.min.x + 10.0, r.min.y + 7.0), galley, t.text);
            if resp.clicked() {
                go = Some((a.clone(), b.clone()));
            }
        }
    });
    if let Some((a, b)) = go {
        g.app.session.sel = wordcraft_engine::Selection { anchor: a, focus: b };
        g.app.canvas.scroll_to_caret = true;
        g.app.canvas.want_focus = true;
    }
}

fn headings(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let caret = g.app.session.sel.focus.clone();
    let mut current = None;
    let mut items = Vec::new();
    for path in g.app.session.doc.para_paths(StoryRef::Body) {
        let Some(p) = g.app.session.doc.para(StoryRef::Body, &path) else { continue };
        let rp = g.app.session.doc.styles.resolve_para(&p.props);
        if let Some(level) = rp.outline_level {
            let text = p.plain_text().trim().to_string();
            if text.is_empty() || rp.style.starts_with("TOC") {
                continue;
            }
            if path <= caret.path {
                current = Some(items.len());
            }
            items.push((path, level, text));
        }
    }
    if items.is_empty() {
        let mut job = LayoutJob::single_section(
            "Headings you add with the Heading styles (Home › Styles) appear here, so you can jump between chapters and scenes.".into(),
            egui::TextFormat { font_id: regular(12.0), color: t.text_dim, ..Default::default() },
        );
        job.wrap.max_width = ui.available_width() - 16.0;
        ui.add_space(6.0);
        egui::Frame::NONE.inner_margin(egui::Margin { left: 8, right: 8, top: 0, bottom: 0 }).show(ui, |ui| {
            ui.add(egui::Label::new(job).wrap());
        });
        return;
    }
    for (i, (path, level, text)) in items.into_iter().enumerate() {
        let indent = f32::from(level.min(6)) * 14.0;
        if row(ui, t, &text, indent, level == 0, current == Some(i)).clicked() {
            w::run(ui, &mut g.app, "caret.set", json!({"pos": Pos { story: StoryRef::Body, path, off: 0 }}));
            g.app.canvas.want_focus = true;
            g.app.canvas.scroll_to_caret = true;
        }
    }
}

fn pages(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let count = g.app.session.layout().pages.len();
    let current = g.app.session.page_hint;
    // As many columns as fit the panel's width: narrow panels show one, wide ones three or four.
    let gap = 12.0;
    let avail = ui.available_width() - 8.0;
    let cols = ((avail + gap) / (96.0 + gap)).floor().clamp(1.0, 4.0) as usize;
    let tw = ((avail - gap * (cols as f32 - 1.0)) / cols as f32).min(150.0);
    let ppp = ui.ctx().pixels_per_point();
    let ctx = ui.ctx().clone();
    ui.add_space(6.0);
    let mut i = 0;
    while i < count {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(gap, 0.0);
            ui.add_space(4.0);
            for _ in 0..cols {
                if i >= count {
                    break;
                }
                let tex = g.thumbnail(&ctx, None, i, (tw * ppp) as u32);
                let th = tex.as_ref().map(|x| tw * x.size_vec2().y / x.size_vec2().x.max(1.0)).unwrap_or(tw * 1.294);
                let (r, resp) = ui.allocate_exact_size(vec2(tw, th + 20.0), Sense::click());
                let page = Rect::from_min_size(r.min, vec2(tw, th));
                let lift = ui.ctx().animate_bool_with_time(resp.id.with("lift"), resp.hovered(), 0.12);
                let page = page.translate(vec2(0.0, -2.0 * lift));
                let shadow =
                    egui::epaint::Shadow { offset: [0, (1.0 + 3.0 * lift) as i8], blur: (4.0 + 8.0 * lift) as u8, spread: 0, color: t.shadow };
                ui.painter().add(shadow.as_shape(page, egui::CornerRadius::same(4)));
                match &tex {
                    Some(tex) => {
                        ui.painter().image(tex.id(), page, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
                    }
                    None => {
                        ui.painter().rect_filled(page, 4.0, egui::Color32::WHITE);
                    }
                }
                let stroke = if i == current { egui::Stroke::new(2.0, t.accent) } else { egui::Stroke::new(0.75, t.edge) };
                ui.painter().rect_stroke(page, 4.0, stroke, egui::StrokeKind::Outside);
                let c = if i == current { t.accent } else { t.text_dim };
                ui.painter().text(pos2(page.center().x, r.max.y - 8.0), Align2::CENTER_CENTER, (i + 1).to_string(), medium(11.0), c);
                if resp.on_hover_text(format!("Page {}", i + 1)).clicked() {
                    w::run(ui, &mut g.app, "edit.goto", json!({"page": i + 1}));
                }
                i += 1;
            }
        });
        ui.add_space(10.0);
    }
}

/// `2026-10-12T09:26:00Z` → ("2026-10-12", "09:26").
fn split_date(iso: &str) -> (String, String) {
    (iso.get(..10).unwrap_or(iso).to_string(), iso.get(11..16).unwrap_or("").to_string())
}

/// "Today", "Yesterday" or the date.
fn day_label(date: &str, today: &str) -> String {
    if date == today {
        return "Today".into();
    }
    let yesterday = crate::goals::date_of((wordcraft_ui_egui::now_ms() / 1000.0) as i64 - 86_400);
    if date == yesterday { "Yesterday".into() } else { date.to_string() }
}

fn body_paragraphs(doc: &Document) -> Vec<String> {
    doc.para_paths(StoryRef::Body).iter().filter_map(|p| doc.para(StoryRef::Body, p).map(|x| x.plain_text().to_string())).collect()
}

fn words_in(doc: &Document) -> usize {
    wordcraft_engine::doc::count_words(&doc.plain_text(StoryRef::Body))
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() > n { format!("{}…", s.chars().take(n).collect::<String>()) } else { s.to_string() }
}

/// What changed between a saved version and the document now: the first paragraph that differs,
/// old text struck through, new text underlined.
fn diff_preview(ui: &mut Ui, t: &Tokens, old: &Document, now: &Document) {
    let (a, b) = (body_paragraphs(old), body_paragraphs(now));
    let first = (0..a.len().max(b.len())).find(|i| a.get(*i) != b.get(*i));
    let mut job = LayoutJob::default();
    job.wrap.max_width = ui.available_width() - 20.0;
    let base = egui::TextFormat { font_id: egui::FontId::new(12.5, egui::FontFamily::Proportional), color: t.text, ..Default::default() };
    match first {
        None => job.append("No changes since this version.", 0.0, egui::TextFormat { color: t.text_dim, ..base.clone() }),
        Some(i) => {
            let red = if t.dark { egui::Color32::from_rgb(0xFF, 0x9B, 0x92) } else { egui::Color32::from_rgb(0xA1, 0x27, 0x1F) };
            let green = if t.dark { egui::Color32::from_rgb(0x86, 0xE3, 0xAE) } else { egui::Color32::from_rgb(0x0F, 0x63, 0x34) };
            if let Some(o) = a.get(i).filter(|s| !s.trim().is_empty()) {
                job.append(&clip(o, 140), 0.0, egui::TextFormat { color: red, strikethrough: egui::Stroke::new(1.0, red), ..base.clone() });
                job.append("\n", 0.0, base.clone());
            }
            if let Some(n) = b.get(i).filter(|s| !s.trim().is_empty()) {
                job.append(&clip(n, 140), 0.0, egui::TextFormat { color: green, underline: egui::Stroke::new(1.0, green), ..base.clone() });
            }
        }
    }
    let bg = ui.painter().add(Shape::Noop);
    let resp = egui::Frame::NONE.inner_margin(10).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(job).wrap());
    });
    ui.painter().set(bg, Shape::rect_filled(resp.response.rect, egui::CornerRadius::same(10), t.fill));
}

/// Snapshots and Version History.
pub fn versions(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    {
        {
            {
                ui.horizontal(|ui| {
                    let sel = usize::from(g.glass.panel.filter(|p| p.is_versions()).or(g.glass.last_panel) == Some(Panel::History));
                    if let Some(i) = w::segmented(ui, t, &["Snapshots", "History"], sel, 30.0) {
                        g.glass.panel = Some(if i == 1 { Panel::History } else { Panel::Snapshots });
                        g.glass.selected_version = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if close_button(ui, t) {
                            g.glass.panel = None;
                        }
                    });
                });
                ui.add_space(10.0);
                match g.glass.panel.filter(|p| p.is_versions()) {
                    Some(Panel::History) => history(g, ui, t),
                    None if g.glass.last_panel == Some(Panel::History) => history(g, ui, t),
                    _ => snapshots(g, ui, t),
                }
            }
        }
    }
}

fn hint(ui: &mut Ui, t: &Tokens, text: &str) {
    let mut job = LayoutJob::single_section(text.into(), egui::TextFormat { font_id: regular(11.5), color: t.text_dim, ..Default::default() });
    job.wrap.max_width = ui.available_width() - 8.0;
    egui::Frame::NONE.inner_margin(egui::Margin { left: 4, right: 4, top: 0, bottom: 0 }).show(ui, |ui| {
        ui.add(egui::Label::new(job).wrap());
    });
}

fn snapshots(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    ui.spacing_mut().item_spacing = vec2(0.0, 8.0);
    let field = egui::TextEdit::singleline(&mut g.glass.snapshot_name)
        .hint_text("Name this snapshot (optional)")
        .desired_width(ui.available_width() - 20.0)
        .margin(vec2(10.0, 7.0));
    let resp = ui.add(field);
    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let width = ui.available_width();
    if w::accent_button(ui, t, Some("screenshot"), "Take Snapshot", Some(width), 36.0).clicked() || enter {
        let n = g.app.session.versions.iter().filter(|(l, _, _)| l != AUTO_LABEL).count() + 1;
        let name = g.glass.snapshot_name.trim();
        let label = if name.is_empty() { format!("Snapshot {n}") } else { name.to_string() };
        match g.app.session.run("file.versions", &json!({"save": label})) {
            Ok(_) => {
                g.app.status(format!("Snapshot “{label}” saved"));
                g.glass.snapshot_name.clear();
                g.glass.selected_version = g.app.session.versions.len().checked_sub(1);
            }
            Err(e) => g.app.status(e.to_string()),
        }
    }
    hint(
        ui,
        t,
        "A snapshot keeps a copy of the whole document before you rewrite it, so you can compare or go back. Snapshots last until you close the document.",
    );
    ui.add_space(4.0);
    let list: Vec<(usize, String, String)> = g
        .app
        .session
        .versions
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, (l, _, _))| l != AUTO_LABEL)
        .map(|(i, (l, d, _))| (i, l.clone(), d.clone()))
        .collect();
    if list.is_empty() {
        hint(ui, t, "No snapshots yet.");
        return;
    }
    let today = GlassApp::today();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (i, label, date) in list {
            let on = g.glass.selected_version == Some(i);
            let (d, time) = split_date(&date);
            let words = g.app.session.versions.get(i).map(|(_, _, doc)| words_in(doc)).unwrap_or(0);
            let meta = format!("{}, {time} UTC · {} words", day_label(&d, &today), crate::chrome::group_digits(words as i64));
            version_card(g, ui, t, i, &label, &meta, on);
        }
    });
}

/// A snapshot or version: its name and date; selected, it shows what changed and Restore.
fn version_card(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, i: usize, label: &str, meta: &str, on: bool) {
    let bg = ui.painter().add(Shape::Noop);
    let resp = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(0.0, 6.0);
        let (r, click) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
        ui.painter().text(pos2(r.min.x, r.min.y + 8.0), Align2::LEFT_CENTER, clip(label, 34), semibold(13.0), t.text);
        ui.painter().text(pos2(r.min.x, r.min.y + 26.0), Align2::LEFT_CENTER, meta, regular(11.5), t.text_dim);
        if click.clicked() {
            g.glass.selected_version = if on { None } else { Some(i) };
        }
        if on {
            ui.label(egui::RichText::new("Changes since this version").font(regular(11.5)).color(t.text_dim));
            if let Some(old) = g.app.session.versions.get(i).map(|(_, _, d)| d.clone()) {
                let now = g.app.session.doc.clone();
                diff_preview(ui, t, &old, &now);
            }
            let width = ui.available_width();
            if w::accent_button(ui, t, None, "Restore This Version", Some(width), 32.0).on_hover_text("You can undo a restore with Undo").clicked() {
                restore(g, i, label);
            }
        }
    });
    let fill = if on { t.paper } else { t.fill };
    let r = resp.response.rect;
    let mut shapes = vec![Shape::rect_filled(r, egui::CornerRadius::same(14), fill)];
    if on {
        shapes.push(Shape::rect_stroke(r, egui::CornerRadius::same(14), egui::Stroke::new(1.5, t.accent), egui::StrokeKind::Inside));
    }
    ui.painter().set(bg, Shape::Vec(shapes));
}

fn restore(g: &mut GlassApp, i: usize, label: &str) {
    match g.app.run("file.versions", json!({"restore": i})) {
        Ok(_) => {
            g.app.status(format!("Restored “{label}”. Undo brings your text back."));
            g.app.canvas.scroll_to_caret = true;
        }
        Err(e) => g.app.status(e),
    }
}

fn history(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    hint(ui, t, "WordCraft Glass keeps a version every ten minutes while you write, plus every snapshot you take. Pick one to see what changed.");
    ui.add_space(6.0);
    let today = GlassApp::today();
    let list: Vec<(usize, String, String)> =
        g.app.session.versions.iter().enumerate().rev().map(|(i, (l, d, _))| (i, l.clone(), d.clone())).collect();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
        // The document as it is now.
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
        ui.painter().circle_filled(pos2(r.min.x + 16.0, r.center().y), 4.0, t.accent);
        ui.painter().text(pos2(r.min.x + 30.0, r.center().y), Align2::LEFT_CENTER, "Now", semibold(12.5), t.text);
        badge(ui, t, r, "Current", true);
        let mut last_day = String::new();
        if list.is_empty() {
            ui.add_space(6.0);
            hint(ui, t, "No earlier versions yet.");
        }
        for (i, label, date) in list {
            let (d, time) = split_date(&date);
            if d != last_day {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(day_label(&d, &today)).font(semibold(11.0)).color(t.text_dim));
                last_day = d;
            }
            let on = g.glass.selected_version == Some(i);
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
            crate::tokens::paint_state(ui, r, radius::ROW, &resp, on, t);
            ui.painter().circle_filled(pos2(r.min.x + 16.0, r.center().y), 4.0, if on { t.accent } else { t.text_disabled });
            let auto = label == AUTO_LABEL;
            ui.painter().text(
                pos2(r.min.x + 30.0, r.center().y),
                Align2::LEFT_CENTER,
                format!("{time} UTC"),
                if on { semibold(12.5) } else { regular(12.5) },
                t.text,
            );
            badge(ui, t, r, if auto { "Auto-save" } else { "Snapshot" }, false);
            if resp.on_hover_text(if auto { label.clone() } else { format!("Snapshot: {label}") }).clicked() {
                g.glass.selected_version = if on { None } else { Some(i) };
            }
            if on {
                ui.add_space(4.0);
                if let Some(old) = g.app.session.versions.get(i).map(|(_, _, d)| d.clone()) {
                    let now = g.app.session.doc.clone();
                    diff_preview(ui, t, &old, &now);
                }
                let width = ui.available_width();
                if w::accent_button(ui, t, None, "Restore This Version", Some(width), 32.0).clicked() {
                    restore(g, i, &label);
                }
                ui.add_space(6.0);
            }
        }
    });
}

fn badge(ui: &Ui, t: &Tokens, row: Rect, text: &str, accent: bool) {
    let font = medium(10.5);
    let w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), t.text).size().x) + 16.0;
    let r = Rect::from_center_size(pos2(row.max.x - w / 2.0 - 8.0, row.center().y), vec2(w, 18.0));
    paint_fill(ui.painter(), r, radius::CAPSULE, if accent { t.accent_soft } else { t.fill });
    ui.painter().text(r.center(), Align2::CENTER_CENTER, text, font, if accent { t.accent } else { t.text_dim });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates_split_safely() {
        assert_eq!(split_date("2026-10-12T09:26:00Z"), ("2026-10-12".into(), "09:26".into()));
        assert_eq!(split_date("junk"), ("junk".into(), String::new()));
    }

    #[test]
    fn clipping_respects_characters() {
        assert_eq!(clip("Inês", 3), "Inê…");
        assert_eq!(clip("ok", 5), "ok");
    }
}

#[cfg(test)]
mod search_tests {
    use super::find_all;

    #[test]
    fn finds_every_match_ignoring_case() {
        assert_eq!(find_all("Ação, ação e AÇÃO.", "ação").len(), 3);
        assert_eq!(find_all("aaa", "aa"), vec![(0, 2)]);
        assert!(find_all("abc", "").is_empty());
        assert!(find_all("", "x").is_empty());
    }
}

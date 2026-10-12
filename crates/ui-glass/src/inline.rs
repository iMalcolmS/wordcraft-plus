//! Writer Pro, as you type: `[[` links to another part of the manuscript (as Obsidian does) and
//! `/` at the start of a paragraph turns it into anything (as Notion does).
//!
//! Both open a small glass menu at the caret that filters as you keep typing. ↑ ↓ move, Return or
//! Tab picks, Escape closes. What gets written is plain Word: a `[[link]]` becomes an ordinary
//! hyperlink to a bookmark on the heading (⌘-click follows it, and it survives .docx), and a
//! `/command` runs the same command the ribbon does.

use egui::{Align2, Rect, Sense, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_engine::Selection;
use wordcraft_engine::Session;
use wordcraft_engine::doc::{Pos, StoryRef};
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::GlassApp;
use crate::project::sections;
use crate::tokens::{Tokens, popover_shape};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Link,
    Slash,
}

/// A trigger found before the caret: its kind, where it starts and what was typed after it.
#[derive(Clone, Debug, PartialEq)]
pub struct Trigger {
    pub kind: Kind,
    pub start: Pos,
    pub query: String,
}

#[derive(Default)]
pub struct Inline {
    /// The highlighted row.
    pub selected: usize,
    /// Escape closed the menu for the trigger starting here (until the caret leaves it).
    dismissed: Option<Pos>,
    /// Whether a menu is showing (the keys it uses are held back from the page).
    pub open: bool,
    /// Keys taken from the page for the menu this frame.
    pub keys: Vec<egui::Key>,
}

/// A `/` command: label, hint, command id and params.
const SLASH: &[(&str, &str, &str, &str)] = &[
    ("Text", "Plain paragraph", "para.normal", ""),
    ("Heading 1", "Chapter or big section", "para.heading1", ""),
    ("Heading 2", "Scene or medium section", "para.heading2", ""),
    ("Heading 3", "Small section", "para.heading3", ""),
    ("Bulleted list", "A simple list", "para.bullets", ""),
    ("Numbered list", "A list with numbers", "para.numbering", ""),
    ("To-do", "A check box to tick", "developer.checkBox", ""),
    ("Quote", "Set off a quotation", "para.style", r#"{"style": "Quote"}"#),
    ("Divider", "A horizontal line", "insert.horizontalLine", ""),
    ("Table", "Three by three", "insert.table", r#"{"rows": 3, "cols": 3}"#),
    ("Image", "From a file", "insert.picture", ""),
    ("Equation", "Math, typed linearly", "insert.equation", ""),
    ("Page break", "Start a new page", "insert.pageBreak", ""),
    ("Footnote", "A note at the foot of the page", "references.footnote", ""),
    ("Comment", "A note in the margin", "review.newComment", ""),
];

const MAX_QUERY: usize = 40;

/// Find a `[[` or `/` trigger right before the caret.
pub fn detect(s: &Session) -> Option<Trigger> {
    let focus = &s.sel.focus;
    if s.sel.anchor != *focus || focus.story != StoryRef::Body {
        return None;
    }
    let para = s.doc.para(StoryRef::Body, &focus.path)?;
    let before = para.text.get(..focus.off)?;
    // `/` typed at the very start of a paragraph.
    if let Some(q) = before.strip_prefix('/')
        && q.chars().count() <= MAX_QUERY
        && q.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-')
        && !q.starts_with(' ')
    {
        return Some(Trigger { kind: Kind::Slash, start: Pos::new(StoryRef::Body, focus.path.clone(), 0), query: q.to_string() });
    }
    // `[[` with no closing `]` after it.
    let at = before.rfind("[[")?;
    let q = before.get(at + 2..)?;
    if q.contains(']') || q.contains(wordcraft_engine::doc::para::OBJ) || q.chars().count() > MAX_QUERY {
        return None;
    }
    Some(Trigger { kind: Kind::Link, start: Pos::new(StoryRef::Body, focus.path.clone(), at), query: q.to_string() })
}

/// One row of the menu: label, hint, and what picking it does.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    Heading { title: String, path: wordcraft_engine::doc::Path },
    Command { id: &'static str, params: &'static str },
}

fn matches(label: &str, query: &str) -> Option<usize> {
    let l = label.to_lowercase();
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(1);
    }
    if l.starts_with(&q) {
        return Some(0);
    }
    // Initials: "h1" for Heading 1, "pb" for Page break.
    let initials: String = l
        .split_whitespace()
        .filter_map(|w| if w.chars().all(|c| c.is_ascii_digit()) { Some(w.to_string()) } else { w.chars().next().map(String::from) })
        .collect();
    if initials == q.replace(' ', "") {
        return Some(0);
    }
    if l.split_whitespace().any(|w| w.starts_with(&q)) {
        return Some(1);
    }
    l.contains(&q).then_some(2)
}

/// The rows for a trigger, best matches first.
pub fn rows(s: &Session, tr: &Trigger) -> Vec<(String, String, Pick)> {
    let mut out: Vec<(usize, String, String, Pick)> = Vec::new();
    match tr.kind {
        Kind::Link => {
            for sec in sections(s) {
                if sec.path == tr.start.path {
                    continue;
                }
                if let Some(rank) = matches(&sec.title, &tr.query) {
                    let hint = format!("Heading {} · {} words", sec.level.saturating_add(1), sec.words);
                    out.push((rank, sec.title.clone(), hint, Pick::Heading { title: sec.title, path: sec.path }));
                }
            }
        }
        Kind::Slash => {
            for (label, hint, id, params) in SLASH {
                if let Some(rank) = matches(label, &tr.query) {
                    out.push((rank, (*label).to_string(), (*hint).to_string(), Pick::Command { id, params }));
                }
            }
        }
    }
    out.sort_by_key(|r| r.0);
    out.into_iter().take(8).map(|(_, a, b, c)| (a, b, c)).collect()
}

/// A bookmark name for a heading: hidden (leading underscore), letters and digits only.
pub fn bookmark_name(title: &str) -> String {
    let mut name = String::from("_wl_");
    for w in title.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        let mut cs = w.chars();
        if let Some(f) = cs.next() {
            name.extend(f.to_uppercase());
            name.push_str(&cs.as_str().to_lowercase());
        }
        if name.len() >= 36 {
            break;
        }
    }
    // Bookmark names are at most 40 bytes; cut at a character boundary.
    while name.len() > 40 {
        name.pop();
    }
    name
}

fn params(p: &str) -> Value {
    if p.is_empty() { json!({}) } else { serde_json::from_str(p).unwrap_or_else(|_| json!({})) }
}

/// Replace the trigger text with what was picked.
pub fn apply(g: &mut GlassApp, tr: &Trigger, pick: &Pick) -> Result<(), String> {
    let caret = g.app.session.sel.focus.clone();
    // Remove the typed trigger (`[[que` or `/que`).
    g.app.session.sel = Selection { anchor: tr.start.clone(), focus: caret };
    g.app.run("text.delete", json!({}))?;
    match pick {
        Pick::Command { id, params: p } => {
            g.app.run(id, params(p))?;
        }
        Pick::Heading { title, path } => {
            let here = g.app.session.sel.focus.clone();
            // Reuse a bookmark already on the heading, or add one at its start.
            let existing = g.app.session.doc.bookmarks().into_iter().find(|(_, p)| p.path == *path).map(|(n, _)| n);
            let name = match existing {
                Some(n) => n,
                None => {
                    let name = bookmark_name(title);
                    g.app.session.sel = Selection::caret(Pos::new(StoryRef::Body, path.clone(), 0));
                    let r = g.app.run("insert.bookmark", json!({"name": name}));
                    // The bookmark adds two marks at the heading's start; a caret in the same
                    // paragraph moves with its text.
                    let mut back = here.clone();
                    if back.path == *path {
                        back.off += 2 * wordcraft_engine::doc::para::OBJ.len_utf8();
                    }
                    g.app.session.sel = Selection::caret(back);
                    r?;
                    name
                }
            };
            g.app.run("insert.link", json!({"url": format!("#{name}"), "text": title}))?;
        }
    }
    Ok(())
}

/// Hold back the keys an open menu uses, so the page doesn't also act on them.
pub fn take_keys(g: &mut GlassApp, raw: &mut egui::RawInput) {
    if !g.glass.inline.open {
        return;
    }
    let keys = [egui::Key::ArrowUp, egui::Key::ArrowDown, egui::Key::Enter, egui::Key::Tab, egui::Key::Escape];
    let mut taken = Vec::new();
    raw.events.retain(|e| match e {
        egui::Event::Key { key, pressed, modifiers, .. } if keys.contains(key) && !modifiers.command && !modifiers.alt => {
            if *pressed {
                taken.push(*key);
            }
            false
        }
        _ => true,
    });
    g.glass.inline.keys.extend(taken);
}

/// After the page: open, drive and draw the menu.
pub fn show(g: &mut GlassApp, ctx: &egui::Context, t: &Tokens) {
    let keys = std::mem::take(&mut g.glass.inline.keys);
    let trigger = if g.glass.prefs.pro && !g.app.session.view.read_mode { detect(&g.app.session) } else { None };
    let Some(tr) = trigger.filter(|tr| g.glass.inline.dismissed.as_ref() != Some(&tr.start)) else {
        if trigger_gone(g) {
            g.glass.inline.dismissed = None;
        }
        g.glass.inline.open = false;
        g.glass.inline.selected = 0;
        return;
    };
    let list = rows(&g.app.session, &tr);
    if list.is_empty() && tr.kind == Kind::Slash {
        g.glass.inline.open = false;
        return;
    }
    let n = list.len();
    let mut sel = g.glass.inline.selected.min(n.saturating_sub(1));
    let mut pick: Option<usize> = None;
    for k in keys {
        match k {
            egui::Key::ArrowDown if n > 0 => sel = (sel + 1) % n,
            egui::Key::ArrowUp if n > 0 => sel = (sel + n - 1) % n,
            egui::Key::Enter | egui::Key::Tab if n > 0 => pick = Some(sel),
            egui::Key::Escape => {
                g.glass.inline.dismissed = Some(tr.start.clone());
                g.glass.inline.open = false;
                return;
            }
            _ => {}
        }
    }
    let Some((caret, line_h)) = wordcraft_ui_egui::canvas::caret_screen(&mut g.app) else {
        g.glass.inline.open = false;
        return;
    };
    g.glass.inline.open = true;
    let fade = ctx.animate_bool_with_time(egui::Id::new(("glass_inline_fade", tr.kind == Kind::Link)), true, 0.12);
    let screen = ctx.content_rect();
    let height = 44.0 + n.max(1) as f32 * 40.0;
    let below = caret.y + line_h + 6.0 + height < screen.max.y - 8.0;
    let y = if below { caret.y + line_h + 6.0 + (1.0 - fade) * -4.0 } else { caret.y - 6.0 - height };
    let x = caret.x.clamp(screen.min.x + 8.0, (screen.max.x - 300.0).max(screen.min.x + 8.0));
    egui::Area::new(egui::Id::new("glass_inline_menu")).order(egui::Order::Foreground).fixed_pos(pos2(x - 12.0, y)).show(ctx, |ui| {
        ui.multiply_opacity(fade);
        let bg = ui.painter().add(egui::Shape::Noop);
        let inner = egui::Frame::NONE.inner_margin(6).show(ui, |ui| {
            ui.set_width(288.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let title = match tr.kind {
                Kind::Link => "LINK TO",
                Kind::Slash => "TURN INTO",
            };
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
            ui.painter().text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, title, semibold(10.5), t.text_dim);
            if n == 0 {
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
                let msg = if tr.query.is_empty() { "Add headings to link to them." } else { "No heading matches." };
                ui.painter().text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, msg, regular(12.5), t.text_dim);
            }
            for (i, (label, hint, _)) in list.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                if resp.hovered() && ui.input(|inp| inp.pointer.delta() != egui::Vec2::ZERO) {
                    sel = i;
                }
                let on = ctx.animate_bool_with_time(egui::Id::new(("glass_inline_row", i)), i == sel, 0.08);
                if on > 0.0 {
                    ui.painter().rect_filled(r, 9.0, t.accent.gamma_multiply(on));
                }
                let (c, d) = if i == sel { (t.on_accent, t.on_accent.gamma_multiply(0.8)) } else { (t.text, t.text_dim) };
                let icon = Rect::from_center_size(pos2(r.min.x + 20.0, r.center().y), vec2(24.0, 24.0));
                let glyph = match tr.kind {
                    Kind::Link => "↗",
                    Kind::Slash => label.get(..1).unwrap_or("·"),
                };
                ui.painter().rect_filled(icon, 6.0, if i == sel { t.on_accent.gamma_multiply(0.18) } else { t.fill });
                ui.painter().text(icon.center(), Align2::CENTER_CENTER, glyph, medium(12.0), c);
                let shown: String =
                    if label.chars().count() > 30 { format!("{}…", label.chars().take(30).collect::<String>()) } else { label.clone() };
                ui.painter().text(pos2(r.min.x + 42.0, r.center().y - 7.0), Align2::LEFT_CENTER, shown, medium(13.0), c);
                ui.painter().text(pos2(r.min.x + 42.0, r.center().y + 9.0), Align2::LEFT_CENTER, hint, regular(11.0), d);
                if resp.clicked() {
                    pick = Some(i);
                }
            }
        });
        ui.painter().set(bg, popover_shape(inner.response.rect, 14, t));
    });
    g.glass.inline.selected = sel;
    if let Some(i) = pick
        && let Some((_, _, p)) = list.get(i)
    {
        if let Err(e) = apply(g, &tr, p) {
            g.app.status(e);
        }
        g.glass.inline.open = false;
        g.glass.inline.selected = 0;
        g.app.canvas.want_focus = true;
    }
    if fade < 1.0 {
        ctx.request_repaint();
    }
}

/// Whether the trigger Escape dismissed is gone (so the next one opens the menu again).
fn trigger_gone(g: &GlassApp) -> bool {
    match (&g.glass.inline.dismissed, detect(&g.app.session)) {
        (Some(d), Some(tr)) => tr.start != *d,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_ui_egui::{Services, WordApp};

    fn glass(text: &str) -> GlassApp {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_engine::sample::sample_document()), Services::default()));
        g.app.run("select.all", json!({})).unwrap();
        g.app.run("text.delete", json!({})).unwrap();
        g.app.run("text.insert", json!({"text": text})).unwrap();
        g
    }

    #[test]
    fn finds_link_and_slash_triggers() {
        let g = glass("See [[Wel");
        let tr = detect(&g.app.session).unwrap();
        assert_eq!(tr.kind, Kind::Link);
        assert_eq!(tr.query, "Wel");
        let g = glass("/head");
        let tr = detect(&g.app.session).unwrap();
        assert_eq!(tr.kind, Kind::Slash);
        assert_eq!(rows(&g.app.session, &tr).first().map(|r| r.0.as_str()), Some("Heading 1"));
        assert!(detect(&glass("a/b").app.session).is_none());
        assert!(detect(&glass("[[done]] and").app.session).is_none());
    }

    #[test]
    fn slash_command_turns_the_paragraph_into_a_heading() {
        let mut g = glass("/h1");
        let tr = detect(&g.app.session).unwrap();
        let pick = rows(&g.app.session, &tr).into_iter().next().unwrap().2;
        apply(&mut g, &tr, &pick).unwrap();
        let p = g.app.session.doc.para(StoryRef::Body, &g.app.session.sel.focus.path).unwrap();
        assert_eq!(p.plain_text(), "");
        assert!(g.app.session.doc.styles.resolve_para(&p.props).outline_level.is_some());
    }

    #[test]
    fn a_wikilink_becomes_a_link_to_a_bookmark_on_the_heading() {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_engine::sample::sample_document()), Services::default()));
        let secs = sections(&g.app.session);
        // A new paragraph at the start of the document.
        g.app.session.sel = Selection::caret(Pos::new(StoryRef::Body, secs.first().unwrap().path.clone(), 0));
        g.app.run("text.newParagraph", json!({})).unwrap();
        g.app.run("caret.up", json!({})).unwrap();
        g.app.run("para.normal", json!({})).unwrap();
        let target = sections(&g.app.session).last().unwrap().clone();
        let word = target.title.split_whitespace().next().unwrap().to_string();
        g.app.run("text.insert", json!({"text": format!("[[{word}")})).unwrap();
        let tr = detect(&g.app.session).unwrap();
        let list = rows(&g.app.session, &tr);
        let pick = list.iter().find(|r| r.0 == target.title).map(|r| r.2.clone()).unwrap();
        apply(&mut g, &tr, &pick).unwrap();
        let marks = g.app.session.doc.bookmarks();
        assert!(marks.iter().any(|(n, p)| n.starts_with("_wl_") && p.path == target.path));
        let here = g.app.session.doc.para(StoryRef::Body, &g.app.session.sel.focus.path).unwrap().plain_text();
        assert!(here.contains(&target.title) && !here.contains("[["), "{here}");
    }

    #[test]
    fn bookmark_names_are_valid() {
        assert_eq!(bookmark_name("The Long Night!"), "_wl_TheLongNight");
        assert!(bookmark_name(&"word ".repeat(40)).len() <= 40);
    }
}

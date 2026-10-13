//! Research mode's Binder: the paper rather than a novel. Where Writer Pro's Binder holds the
//! manuscript, characters and places, this one holds what a paper is made of:
//!
//! - **Paper**: the sections with their word counts (drag to reorder), and the usual parts it
//!   still lacks (Abstract, Introduction, Methods, Results, Discussion, Conclusion, References),
//!   each a click away.
//! - **Figures & Tables**, **Equations** and **Code**: every numbered caption, equation and code
//!   block, in order; click to go there (▶ runs a code block).
//! - **Sources**: the works cited, with how often, and the sources never cited; click to step
//!   through a work's citations.
//! - **Notes** (research notes), **Reviews** (the reviewer response logs, with how many are
//!   done), the **Scratchpad** (snippets, quotes, equations) and the open documents.

use std::collections::BTreeMap;

use egui::{Rect, Sense, Stroke, Ui, pos2, vec2};
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::{Path, Pos, StoryRef};
use wordcraft_engine::Selection;
use wordcraft_ui_egui::theme::{regular, semibold};

use crate::project::{NoteKind, key};
use crate::research::{RView, doc_citations, lookup};
use crate::scriv::{Icon, count_badge, row, toggle_fold};
use crate::tokens::Tokens;
use crate::widgets as w;
use crate::{GlassApp, Panel};

/// A numbered caption, an equation, or a code block: what it says and where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub text: String,
    pub path: Path,
    /// The caption's label (Figure, Table…), or the code block's id.
    pub label: String,
}

/// The body's numbered captions (paragraphs holding a `SEQ` field), in order.
pub fn captions(doc: &wordcraft_doc::Document) -> Vec<Item> {
    let mut out = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        let seq = p.objects.iter().find_map(|o| match o {
            InlineObject::Field { instr, .. } => {
                let mut w = instr.split_whitespace();
                w.next().is_some_and(|x| x.eq_ignore_ascii_case("SEQ")).then(|| w.next().unwrap_or("").to_string())
            }
            _ => None,
        });
        if let Some(label) = seq {
            out.push(Item { text: p.plain_text().trim().to_string(), path, label });
            if out.len() >= 500 {
                break;
            }
        }
    }
    out
}

/// The body's equations, in order.
pub fn equations(doc: &wordcraft_doc::Document) -> Vec<Item> {
    let mut out = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        for o in &p.objects {
            if let InlineObject::Equation { linear, .. } = o {
                out.push(Item { text: linear.clone(), path: path.clone(), label: String::new() });
            }
        }
        if out.len() >= 500 {
            break;
        }
    }
    out
}

/// The works cited, in order of first citation: (tag, label, title, where it's cited).
fn cited(g: &mut GlassApp) -> Vec<(String, String, String, Vec<Pos>)> {
    let mut order: Vec<String> = Vec::new();
    let mut at: BTreeMap<String, Vec<Pos>> = BTreeMap::new();
    let mut inline: BTreeMap<String, crate::refs::Ref> = BTreeMap::new();
    for c in doc_citations(&g.app.session.doc) {
        for (k, tag) in c.tags.iter().enumerate() {
            if !at.contains_key(tag) {
                order.push(tag.clone());
            }
            at.entry(tag.clone()).or_default().push(c.pos.clone());
            if let Some(r) = c.inline_refs.get(k) {
                inline.insert(tag.clone(), r.clone());
            }
        }
    }
    order
        .into_iter()
        .map(|tag| {
            let r = inline.get(&tag).cloned().or_else(|| lookup(g, &tag));
            let (label, title) = r.map(|r| (r.label(), r.title)).unwrap_or_else(|| (tag.clone(), String::new()));
            let places = at.remove(&tag).unwrap_or_default();
            (tag, label, title, places)
        })
        .collect()
}

fn go(g: &mut GlassApp, pos: Pos) {
    g.app.session.sel = Selection::caret(pos);
    g.app.canvas.scroll_to_caret = true;
    g.app.canvas.want_focus = true;
    g.glass.view = crate::View::Page;
}

/// A folder row: open or closed, with an optional count and a + button. Returns (clicked, plus).
fn folder(
    g: &mut GlassApp,
    ui: &mut Ui,
    t: &Tokens,
    icon: Icon,
    name: &str,
    fold_key: &str,
    count: Option<usize>,
    plus: Option<&str>,
) -> (bool, bool) {
    let open = !g.glass.scriv.folded.contains(fold_key);
    let (resp, tog) = row(ui, t, 0, icon, name, Some(open), false, None);
    let mut plus_clicked = false;
    if let Some(tip) = plus {
        let r = Rect::from_center_size(pos2(resp.rect.max.x - 12.0, resp.rect.center().y), vec2(20.0, 20.0));
        let presp = ui.interact(r, resp.id.with("plus"), Sense::click()).on_hover_text(tip);
        if presp.hovered() || resp.hovered() {
            let st = Stroke::new(1.3, t.text_dim);
            let c = r.center();
            ui.painter().line_segment([pos2(c.x - 4.0, c.y), pos2(c.x + 4.0, c.y)], st);
            ui.painter().line_segment([pos2(c.x, c.y - 4.0), pos2(c.x, c.y + 4.0)], st);
        } else if let Some(n) = count {
            count_badge(ui, t, resp.rect, &n.to_string());
        }
        plus_clicked = presp.clicked();
    } else if let Some(n) = count {
        count_badge(ui, t, resp.rect, &n.to_string());
    }
    if tog {
        toggle_fold(g, fold_key);
    }
    (resp.clicked() && !plus_clicked, plus_clicked)
}

fn is_open(g: &GlassApp, fold_key: &str) -> bool {
    !g.glass.scriv.folded.contains(fold_key)
}

fn empty_hint(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.label(egui::RichText::new(format!("      {text}")).font(regular(11.5)).color(t.text_dim));
}

/// Research mode's Binder.
pub fn show(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Research Binder").font(semibold(15.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if w::icon_capsule(ui, t, "close", "Hide Binder", 24.0, false, true).clicked() {
                g.glass.prefs.binder = false;
            }
        });
    });
    ui.add_space(6.0);
    egui::ScrollArea::vertical().id_salt("glass_research_binder").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
        crate::scriv::manuscript(g, ui, t, true);
        ui.add_space(6.0);
        let doc_cursor = g.app.session.sel.focus.path.clone();
        let mut goto: Option<Pos> = None;

        // Figures & Tables.
        let caps = captions(&g.app.session.doc);
        let (_, plus) =
            folder(g, ui, t, Icon::Figure, "Figures & Tables", "\u{0}r-figures", Some(caps.len()), Some("New figure caption at the cursor"));
        if plus && let Err(e) = g.app.run("references.caption", serde_json::json!({"label": "Figure"})) {
            g.app.status(e);
        }
        if is_open(g, "\u{0}r-figures") {
            if caps.is_empty() {
                empty_hint(ui, t, "Captions (Figure 1, Table 1…) list here.");
            }
            for c in &caps {
                let icon = if c.label.eq_ignore_ascii_case("table") { Icon::Table } else { Icon::Figure };
                let text = if c.text.is_empty() { c.label.clone() } else { c.text.clone() };
                if row(ui, t, 1, icon, &text, None, c.path == doc_cursor, None).0.on_hover_text(&c.text).clicked() {
                    goto = Some(Pos::new(StoryRef::Body, c.path.clone(), 0));
                }
            }
        }

        // Equations.
        let eqs = equations(&g.app.session.doc);
        let (_, plus) = folder(g, ui, t, Icon::Equation, "Equations", "\u{0}r-equations", Some(eqs.len()), Some("Formula library"));
        if plus {
            g.research_panel(RView::Formulas);
        }
        if is_open(g, "\u{0}r-equations") {
            if eqs.is_empty() {
                empty_hint(ui, t, "Type $x^2$ or /eq-… to add one.");
            }
            for (k, e) in eqs.iter().enumerate().take(200) {
                let text = format!("({}) {}", k + 1, e.text);
                if row(ui, t, 1, Icon::Equation, &text, None, e.path == doc_cursor, None).0.clicked() {
                    goto = Some(Pos::new(StoryRef::Body, e.path.clone(), 0));
                }
            }
        }

        // Code.
        let blocks = crate::codeblocks::code_blocks(&g.app.session.doc);
        let (_, plus) = folder(g, ui, t, Icon::Code, "Code", "\u{0}r-code", Some(blocks.len()), Some("New code block"));
        if plus {
            g.research_panel(RView::Code);
        }
        let mut run: Option<u64> = None;
        if is_open(g, "\u{0}r-code") {
            if blocks.is_empty() {
                empty_hint(ui, t, "Type /python, /r, /matlab or /julia.");
            }
            for cb in &blocks {
                let first: String = cb.code.lines().find(|l| !l.trim().is_empty()).unwrap_or("(empty)").trim().chars().take(18).collect();
                let text = format!("{} · {first}", cb.lang.name());
                let inside = doc_cursor.0.first().is_some_and(|b| *b as usize == cb.block);
                let (resp, _) = row(ui, t, 1, Icon::Code, &text, None, inside, None);
                // ▶ on the right runs it.
                let pr = Rect::from_center_size(pos2(resp.rect.max.x - 12.0, resp.rect.center().y), vec2(20.0, 20.0));
                let presp = ui.interact(pr, resp.id.with("run"), Sense::click()).on_hover_text("Run");
                let c = pr.center();
                let busy = g.code_running(cb.id);
                let col = if presp.hovered() { t.accent } else { t.text_dim };
                if busy {
                    ui.painter().circle_stroke(c, 4.0, Stroke::new(1.3, t.accent));
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                } else {
                    ui.painter().add(egui::Shape::convex_polygon(
                        vec![pos2(c.x - 3.0, c.y - 4.0), pos2(c.x + 4.0, c.y), pos2(c.x - 3.0, c.y + 4.0)],
                        col,
                        Stroke::NONE,
                    ));
                }
                if presp.clicked() {
                    run = Some(cb.id);
                } else if resp.clicked()
                    && let Some(p) = g.app.session.doc.para_paths(StoryRef::Body).into_iter().find(|p| crate::top_of(p) == cb.block)
                {
                    goto = Some(Pos::new(StoryRef::Body, p, 0));
                }
            }
        }
        if let Some(id) = run {
            g.run_code_block(id);
        }
        ui.add_space(6.0);

        // Sources.
        let works = cited(g);
        let uncited: Vec<(String, String)> = {
            let tags: Vec<&String> = works.iter().map(|w| &w.0).collect();
            g.app
                .session
                .doc
                .sources
                .iter()
                .filter(|s| !tags.contains(&&s.tag))
                .map(|s| (s.tag.clone(), crate::refs::Ref::from_source(s).label()))
                .collect()
        };
        let (clicked, plus) = folder(g, ui, t, Icon::Source, "Sources", "\u{0}r-sources", Some(works.len()), Some("Library"));
        if plus || clicked {
            g.research_panel(RView::Library);
        }
        if is_open(g, "\u{0}r-sources") {
            if works.is_empty() {
                empty_hint(ui, t, "Cite with [@ or from the Library.");
            }
            let step_id = egui::Id::new("glass_rbinder_steps");
            let mut steps: BTreeMap<String, usize> = ui.data(|d| d.get_temp(step_id)).unwrap_or_default();
            for (tag, label, title, places) in &works {
                let (resp, _) = row(ui, t, 1, Icon::Source, label, None, false, None);
                count_badge(ui, t, resp.rect, &format!("×{}", places.len()));
                let resp = if title.is_empty() { resp } else { resp.on_hover_text(title) };
                if resp.clicked() && !places.is_empty() {
                    let k = steps.entry(tag.clone()).or_insert(0);
                    goto = places.get(*k % places.len()).cloned();
                    *k += 1;
                }
            }
            ui.data_mut(|d| d.insert_temp(step_id, steps));
            for (_, label) in &uncited {
                let (resp, _) = row(ui, t, 1, Icon::Missing, label, None, false, None);
                count_badge(ui, t, resp.rect, "not cited");
                if resp.on_hover_text("In the document's sources, but never cited").clicked() {
                    g.research_panel(RView::Check);
                }
            }
        }

        // Research notes.
        let pkey = key(&g.app.session);
        let notes: Vec<(u64, String)> = g
            .glass
            .projects
            .by_doc
            .get(&pkey)
            .map(|p| p.notes.iter().filter(|n| n.kind == NoteKind::Research).map(|n| (n.id, n.title.clone())).collect())
            .unwrap_or_default();
        let (_, plus) = folder(g, ui, t, Icon::Research, "Notes", "\u{0}r-notes", Some(notes.len()), Some("New note"));
        if plus {
            let id = g.glass.projects.by_doc.entry(pkey.clone()).or_default().add_note(NoteKind::Research, "New note");
            g.glass.panel = Some(Panel::Note(id));
        }
        if is_open(g, "\u{0}r-notes") {
            for (id, title) in notes {
                let on = g.glass.panel == Some(Panel::Note(id));
                if row(ui, t, 1, Icon::Doc, &title, None, on, None).0.clicked() {
                    g.glass.panel = if on { None } else { Some(Panel::Note(id)) };
                }
            }
        }

        // Reviews: the response logs open in tabs.
        let logs: Vec<(usize, String, usize, usize)> = (0..g.docs.tabs.len())
            .filter_map(|i| {
                let d = g.docs.tabs.get(i)?.db.as_deref()?;
                d.prop_named("Response")?;
                d.prop_named("Comment")?;
                let done = d.prop_named("Status").map(|p| d.rows.iter().filter(|r| d.text(r.id, p) == "Done").count()).unwrap_or(0);
                Some((i, d.name.clone(), done, d.rows.len()))
            })
            .collect();
        let (_, plus) = folder(g, ui, t, Icon::Review, "Reviews", "\u{0}r-reviews", None, Some("Import a review"));
        if plus {
            g.research_act(crate::ribbon::RAct::ImportReview);
        }
        let mut switch: Option<usize> = None;
        if is_open(g, "\u{0}r-reviews") {
            if logs.is_empty() {
                let (resp, _) = row(ui, t, 1, Icon::Missing, "Import a review…", None, false, None);
                if resp.clicked() {
                    g.research_act(crate::ribbon::RAct::ImportReview);
                }
            }
            for (i, name, done, total) in &logs {
                let (resp, _) = row(ui, t, 1, Icon::Review, name, None, *i == g.docs.active, None);
                count_badge(ui, t, resp.rect, &format!("{done}/{total} done"));
                if resp.clicked() {
                    switch = Some(*i);
                }
            }
            if !logs.is_empty() {
                let (resp, _) = row(ui, t, 1, Icon::Doc, "Write the Response Letter", None, false, None);
                if resp.clicked() {
                    g.response_letter();
                }
            }
        }
        ui.add_space(6.0);

        // Scratchpad.
        let n = g.glass.projects.by_doc.get(&pkey).map(|p| p.scraps.len()).unwrap_or(0);
        let on = g.glass.panel == Some(Panel::Scratchpad);
        let (resp, _) = row(ui, t, 0, Icon::Scratch, "Scratchpad", None, on, None);
        count_badge(ui, t, resp.rect, &n.to_string());
        if resp.on_hover_text("Snippets, quotes and equations").clicked() {
            g.glass.panel = if on { None } else { Some(Panel::Scratchpad) };
        }
        ui.add_space(6.0);

        // Open documents (chapters, appendices, supplementary material).
        let docs: Vec<usize> = (0..g.docs.tabs.len()).filter(|i| !g.is_db_tab(*i)).collect();
        if docs.len() > 1 {
            ui.label(egui::RichText::new("  OPEN DOCUMENTS").font(semibold(10.0)).color(t.text_dim));
            for i in docs {
                if row(ui, t, 0, Icon::Doc, &g.tab_title(i), None, i == g.docs.active, None).0.clicked() {
                    switch = Some(i);
                }
            }
        }
        if let Some(i) = switch {
            g.switch_to(i);
        }
        if let Some(p) = goto {
            go(g, p);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wordcraft_engine::Session;

    #[test]
    fn captions_and_equations_are_listed_in_order() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.run("references.caption", &json!({"label": "Figure", "text": "Map"})).unwrap();
        s.run("text.newParagraph", &json!({})).unwrap();
        s.run("insert.equation", &json!({"latex": "x^2", "edit": false})).unwrap();
        s.run("text.newParagraph", &json!({})).unwrap();
        s.run("references.caption", &json!({"label": "Table", "text": "Sites"})).unwrap();
        let caps = captions(&s.doc);
        assert_eq!(caps.len(), 2);
        assert!(caps[0].text.starts_with("Figure 1") && caps[1].text.starts_with("Table 1"), "{caps:?}");
        assert_eq!(caps[1].label, "Table");
        assert_eq!(equations(&s.doc).len(), 1);
    }

    #[test]
    fn a_paper_lacking_parts_is_told_which() {
        let sec = |title: &str| crate::project::Section {
            path: Path::top(0),
            level: 0,
            title: title.into(),
            words: 0,
            opening: String::new(),
            text: String::new(),
        };
        let (gaps, pt) = crate::scriv::paper_gaps("Abstract\nText", &[sec("1. Introduction"), sec("2. Methods"), sec("References")]);
        assert!(!pt);
        assert_eq!(gaps, [3, 4, 5], "Results, Discussion and Conclusion are missing");
        let (gaps, pt) = crate::scriv::paper_gaps(
            "",
            &[sec("1 INTRODUÇÃO"), sec("2 METODOLOGIA"), sec("3 RESULTADOS E DISCUSSÃO"), sec("CONSIDERAÇÕES FINAIS"), sec("REFERÊNCIAS")],
        );
        assert!(pt);
        assert_eq!(gaps, [0], "only the Resumo is missing");
    }
}

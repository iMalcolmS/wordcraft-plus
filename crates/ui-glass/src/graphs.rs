//! Research mode: two maps of a paper.
//!
//! - **Citation graph**: the works the document cites as circles (bigger when cited more),
//!   joined when they are cited together in a paragraph, share an author, or share keywords.
//!   Click one to see it and step through where it is cited.
//! - **Argument map**: the paper's paragraphs sorted into Hypothesis → Evidence → Limitation →
//!   Conclusion by the words they use ("we hypothesize", "results show", "however", "we
//!   conclude", in English and Portuguese), each role settable by hand. Click a card to go to
//!   its paragraph.

use std::collections::{BTreeMap, HashMap};

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use wordcraft_doc::{Path, Pos, StoryRef};
use wordcraft_engine::Selection;
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::GlassApp;
use crate::refs::Ref;
use crate::research::{doc_citations, lookup};
use crate::tokens::{Tokens, paint_state};

/// The layout kept between frames.
#[derive(Default)]
pub struct Graphs {
    /// Node positions (0..1 square) and velocities, by key.
    pos: HashMap<String, (Pos2, egui::Vec2)>,
    /// The graph computed for a (document, revision).
    graph: Option<((u64, u64), Graph)>,
    pub selected: Option<String>,
    /// Which of the selected work's citations was gone to last.
    pub step: usize,
    /// Include the whole library, not only what the document cites.
    pub whole_library: bool,
}

/// A work in the graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub key: String,
    pub r: Ref,
    /// Where it is cited.
    pub at: Vec<Pos>,
}

/// Why two works are joined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    CitedTogether,
    SameAuthor,
    SameTopic,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    /// (a, b, weight, strongest reason).
    pub edges: Vec<(usize, usize, f32, Why)>,
}

/// Last names in a reference's authors.
fn surnames(r: &Ref) -> Vec<String> {
    r.authors.iter().map(|a| a.split(',').next().unwrap_or(a).trim().to_lowercase()).filter(|s| s.len() > 1).collect()
}

fn keywords(r: &Ref) -> Vec<String> {
    r.keywords.iter().map(|k| k.trim().to_lowercase()).filter(|k| !k.is_empty()).collect()
}

/// Build the graph from works and where they're cited (`together`: groups cited in one
/// paragraph).
pub fn build(nodes: Vec<Node>, together: &[Vec<String>]) -> Graph {
    let index: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.key.as_str(), i)).collect();
    let mut w: BTreeMap<(usize, usize), (f32, Why)> = BTreeMap::new();
    let mut add = |a: usize, b: usize, by: f32, why: Why| {
        if a == b {
            return;
        }
        let k = (a.min(b), a.max(b));
        let e = w.entry(k).or_insert((0.0, why));
        e.0 += by;
        if why == Why::CitedTogether {
            e.1 = why;
        }
    };
    for group in together {
        let ids: Vec<usize> = group.iter().filter_map(|k| index.get(k.as_str()).copied()).collect();
        for (x, a) in ids.iter().enumerate() {
            for b in ids.iter().skip(x + 1) {
                add(*a, *b, 1.0, Why::CitedTogether);
            }
        }
    }
    let names: Vec<Vec<String>> = nodes.iter().map(|n| surnames(&n.r)).collect();
    let kws: Vec<Vec<String>> = nodes.iter().map(|n| keywords(&n.r)).collect();
    let n = nodes.len().min(600);
    for a in 0..n {
        for b in a + 1..n {
            let (na, nb) = (names.get(a), names.get(b));
            if let (Some(na), Some(nb)) = (na, nb)
                && na.iter().any(|x| nb.contains(x))
            {
                add(a, b, 0.6, Why::SameAuthor);
            }
            if let (Some(ka), Some(kb)) = (kws.get(a), kws.get(b)) {
                let shared = ka.iter().filter(|x| kb.contains(x)).count();
                if shared > 0 {
                    add(a, b, 0.3 * shared as f32, Why::SameTopic);
                }
            }
        }
    }
    Graph { nodes, edges: w.into_iter().map(|((a, b), (wt, why))| (a, b, wt, why)).collect() }
}

/// The graph of the open document.
fn graph(g: &mut GlassApp) -> Graph {
    let key = (g.app.session.document_id(), g.app.session.rev() ^ u64::from(g.glass.graphs.whole_library));
    if let Some((k, gr)) = &g.glass.graphs.graph
        && *k == key
    {
        return gr.clone();
    }
    let cites = doc_citations(&g.app.session.doc);
    let mut nodes: Vec<Node> = Vec::new();
    let mut together: BTreeMap<Path, Vec<String>> = BTreeMap::new();
    for c in &cites {
        for (k, tag) in c.tags.iter().enumerate() {
            together.entry(c.pos.path.clone()).or_default().push(tag.clone());
            match nodes.iter_mut().find(|n| &n.key == tag) {
                Some(n) => n.at.push(c.pos.clone()),
                None => {
                    let r = c.inline_refs.get(k).cloned().or_else(|| lookup(g, tag)).unwrap_or_else(|| Ref {
                        key: tag.clone(),
                        title: tag.clone(),
                        ..Default::default()
                    });
                    nodes.push(Node { key: tag.clone(), r, at: vec![c.pos.clone()] });
                }
            }
        }
    }
    let sources: Vec<Ref> = g.app.session.doc.sources.iter().map(Ref::from_source).collect();
    let mut extra: Vec<Ref> = sources;
    if g.glass.graphs.whole_library {
        extra.extend(g.library_mut().refs.iter().cloned());
    }
    for r in extra {
        if !nodes.iter().any(|n| n.key == r.key) && nodes.len() < 400 {
            nodes.push(Node { key: r.key.clone(), r, at: Vec::new() });
        }
    }
    let groups: Vec<Vec<String>> = together.into_values().collect();
    let gr = build(nodes, &groups);
    g.glass.graphs.graph = Some((key, gr.clone()));
    gr
}

/// One step of a force layout over the unit square; returns how much moved.
fn settle(pos: &mut HashMap<String, (Pos2, egui::Vec2)>, gr: &Graph) -> f32 {
    let n = gr.nodes.len();
    if n == 0 {
        return 0.0;
    }
    let k = (1.0 / n as f32).sqrt() * 0.9;
    let p: Vec<Pos2> = gr.nodes.iter().map(|nd| pos.get(&nd.key).map(|v| v.0).unwrap_or(pos2(0.5, 0.5))).collect();
    let mut f = vec![egui::Vec2::ZERO; n];
    for a in 0..n.min(400) {
        for b in a + 1..n.min(400) {
            let (Some(pa), Some(pb)) = (p.get(a), p.get(b)) else { continue };
            let d = *pa - *pb;
            let dist = d.length().max(0.01);
            let push = d / dist * (k * k / dist);
            if let Some(x) = f.get_mut(a) {
                *x += push;
            }
            if let Some(x) = f.get_mut(b) {
                *x -= push;
            }
        }
    }
    for (a, b, w, _) in &gr.edges {
        let (Some(pa), Some(pb)) = (p.get(*a), p.get(*b)) else { continue };
        let d = *pb - *pa;
        let dist = d.length().max(0.01);
        let pull = d / dist * (dist * dist / k) * w.min(3.0) * 0.5;
        if let Some(x) = f.get_mut(*a) {
            *x += pull;
        }
        if let Some(x) = f.get_mut(*b) {
            *x -= pull;
        }
    }
    let mut moved = 0.0;
    for (i, nd) in gr.nodes.iter().enumerate() {
        let (Some(p0), Some(fi)) = (p.get(i), f.get(i)) else { continue };
        // Gravity to the middle keeps loose works on the page.
        let g = (pos2(0.5, 0.5) - *p0) * 0.02;
        let entry = pos.entry(nd.key.clone()).or_insert((*p0, egui::Vec2::ZERO));
        let v = (entry.1 + *fi * 0.02 + g) * 0.8;
        let v = if v.length() > 0.03 { v / v.length() * 0.03 } else { v };
        entry.1 = v;
        entry.0 = pos2((p0.x + v.x).clamp(0.04, 0.96), (p0.y + v.y).clamp(0.04, 0.96));
        moved += v.length();
    }
    moved
}

/// Where a key starts: spread on a circle by its hash, so the layout is the same every time.
fn seed(key: &str, i: usize, n: usize) -> Pos2 {
    let h = key.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    let a = (i as f32 / n.max(1) as f32) * std::f32::consts::TAU + (h % 100) as f32 * 0.01;
    pos2(0.5 + 0.3 * a.cos(), 0.5 + 0.3 * a.sin())
}

fn go_to(g: &mut GlassApp, at: &Pos) {
    g.app.session.sel = Selection::caret(at.clone());
    g.app.canvas.scroll_to_caret = true;
    g.glass.view = crate::View::Page;
}

/// The citation graph panel.
pub fn citation_graph(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let mut whole = g.glass.graphs.whole_library;
    if ui.checkbox(&mut whole, "Include my whole library").changed() {
        g.glass.graphs.whole_library = whole;
    }
    let gr = graph(g);
    if gr.nodes.is_empty() {
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    "Cite works from the Library and they appear here, joined when they're cited together, share an author or share keywords.",
                )
                .font(regular(12.0))
                .color(t.text_dim),
            )
            .wrap(),
        );
        return;
    }
    let n = gr.nodes.len();
    for (i, nd) in gr.nodes.iter().enumerate() {
        g.glass.graphs.pos.entry(nd.key.clone()).or_insert((seed(&nd.key, i, n), egui::Vec2::ZERO));
    }
    let mut moved = 0.0;
    for _ in 0..4 {
        moved = settle(&mut g.glass.graphs.pos, &gr);
    }
    if moved > 0.002 {
        ui.ctx().request_repaint();
    }
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, w.min(420.0)), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 12.0, t.fill);
    let inner = rect.shrink(14.0);
    let at = |p: Pos2| pos2(inner.min.x + p.x * inner.width(), inner.min.y + p.y * inner.height());
    let pts: Vec<Pos2> = gr.nodes.iter().map(|nd| at(g.glass.graphs.pos.get(&nd.key).map(|v| v.0).unwrap_or(pos2(0.5, 0.5)))).collect();
    let sel = g.glass.graphs.selected.clone();
    let sel_i = sel.as_ref().and_then(|k| gr.nodes.iter().position(|n| &n.key == k));
    for (a, b, wt, why) in &gr.edges {
        let (Some(pa), Some(pb)) = (pts.get(*a), pts.get(*b)) else { continue };
        let lit = sel_i.is_some_and(|s| s == *a || s == *b);
        let c = match why {
            Why::CitedTogether => t.accent,
            Why::SameAuthor => Color32::from_rgb(0x9B, 0x6B, 0xC9),
            Why::SameTopic => Color32::from_rgb(0x44, 0x83, 0x61),
        };
        let alpha = if sel_i.is_none() || lit { 0.75 } else { 0.15 };
        painter.line_segment([*pa, *pb], Stroke::new((0.8 + wt.min(4.0) * 0.6) * if lit { 1.4 } else { 1.0 }, c.gamma_multiply(alpha)));
    }
    let hover = resp.hover_pos();
    let mut hovered: Option<usize> = None;
    for (i, nd) in gr.nodes.iter().enumerate() {
        let Some(p) = pts.get(i) else { continue };
        let r = 4.0 + (nd.at.len() as f32).sqrt() * 3.0;
        let on = sel_i == Some(i);
        let fill = if nd.at.is_empty() { t.text_dim.gamma_multiply(0.5) } else { t.accent };
        painter.circle(*p, r, if on { fill } else { fill.gamma_multiply(0.85) }, Stroke::new(if on { 2.5 } else { 1.0 }, t.paper));
        if hover.is_some_and(|h| h.distance(*p) < r + 3.0) {
            hovered = Some(i);
        }
        if nd.at.len() >= 2 || on || n <= 12 {
            painter.text(*p + vec2(0.0, r + 7.0), Align2::CENTER_CENTER, nd.r.label(), regular(10.0), t.text_dim);
        }
    }
    if let Some(i) = hovered
        && let Some(nd) = gr.nodes.get(i)
    {
        resp.clone().on_hover_text(format!("{}\n{}", nd.r.label(), nd.r.title));
    }
    if resp.clicked() {
        g.glass.graphs.selected = hovered.and_then(|i| gr.nodes.get(i)).map(|n| n.key.clone());
        g.glass.graphs.step = 0;
    }
    // Dragging a node moves it.
    if resp.dragged()
        && let (Some(i), Some(h)) = (hovered.or(sel_i), hover)
        && let Some(nd) = gr.nodes.get(i)
    {
        let p = pos2(((h.x - inner.min.x) / inner.width()).clamp(0.04, 0.96), ((h.y - inner.min.y) / inner.height()).clamp(0.04, 0.96));
        g.glass.graphs.pos.insert(nd.key.clone(), (p, egui::Vec2::ZERO));
        g.glass.graphs.selected = Some(nd.key.clone());
    }
    ui.add_space(6.0);
    // Legend.
    ui.horizontal_wrapped(|ui| {
        for (c, s) in [
            (t.accent, "cited together"),
            (Color32::from_rgb(0x9B, 0x6B, 0xC9), "same author"),
            (Color32::from_rgb(0x44, 0x83, 0x61), "shared keywords"),
        ] {
            let (r, _) = ui.allocate_exact_size(vec2(14.0, 10.0), Sense::hover());
            ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(2.0, c));
            ui.label(egui::RichText::new(s).font(regular(10.5)).color(t.text_dim));
        }
    });
    ui.label(
        egui::RichText::new(format!("{} works · {} links · grey: listed but not cited", n, gr.edges.len())).font(regular(10.5)).color(t.text_dim),
    );
    ui.add_space(6.0);
    if let Some(nd) = sel_i.and_then(|i| gr.nodes.get(i)) {
        ui.label(egui::RichText::new(nd.r.label()).font(medium(11.5)).color(t.accent));
        ui.add(egui::Label::new(egui::RichText::new(&nd.r.title).font(semibold(13.0)).color(t.text)).wrap());
        let linked: Vec<String> = gr
            .edges
            .iter()
            .filter_map(|(a, b, _, _)| {
                if Some(*a) == sel_i {
                    Some(*b)
                } else if Some(*b) == sel_i {
                    Some(*a)
                } else {
                    None
                }
            })
            .filter_map(|j| gr.nodes.get(j).map(|n| n.r.label()))
            .collect();
        if !linked.is_empty() {
            ui.add(egui::Label::new(egui::RichText::new(format!("Linked to {}", linked.join(", "))).font(regular(11.5)).color(t.text_dim)).wrap());
        }
        ui.horizontal(|ui| {
            if !nd.at.is_empty()
                && crate::widgets::quiet_button(
                    ui,
                    t,
                    &format!("Go to Citation {} of {}", g.glass.graphs.step % nd.at.len() + 1, nd.at.len()),
                    None,
                    28.0,
                )
                .clicked()
            {
                let k = g.glass.graphs.step % nd.at.len();
                if let Some(p) = nd.at.get(k) {
                    go_to(g, p);
                }
                g.glass.graphs.step += 1;
            }
            if nd.at.is_empty() && crate::widgets::quiet_button(ui, t, "Cite Here", None, 28.0).clicked() {
                let r = nd.r.clone();
                g.cite(&r);
            }
        });
    }
}

// ---------------------------------------------------------------------------------------------
// The argument map.

/// A paragraph's part in the argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Hypothesis,
    Evidence,
    Limitation,
    Conclusion,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Hypothesis, Role::Evidence, Role::Limitation, Role::Conclusion];

    pub fn name(self) -> &'static str {
        match self {
            Role::Hypothesis => "Hypothesis",
            Role::Evidence => "Evidence",
            Role::Limitation => "Limitation",
            Role::Conclusion => "Conclusion",
        }
    }

    fn color(self) -> Color32 {
        match self {
            Role::Hypothesis => Color32::from_rgb(0x2F, 0x6F, 0xDB),
            Role::Evidence => Color32::from_rgb(0x44, 0x83, 0x61),
            Role::Limitation => Color32::from_rgb(0xD9, 0x73, 0x0D),
            Role::Conclusion => Color32::from_rgb(0x9B, 0x6B, 0xC9),
        }
    }

    fn from_name(s: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == s)
    }

    fn cues(self) -> &'static [&'static str] {
        match self {
            Role::Hypothesis => &[
                "we hypothesize",
                "we hypothesise",
                "hypothesis",
                "we propose",
                "we expect",
                "we predict",
                "we argue",
                "our aim",
                "this paper asks",
                "research question",
                "hipótese",
                "propomos",
                "esperamos que",
                "supomos",
                "objetivo deste",
                "questão de pesquisa",
            ],
            Role::Evidence => &[
                "results show",
                "results indicate",
                "we found",
                "we observed",
                "the data",
                "our data",
                "figure ",
                "table ",
                "significant",
                "p <",
                "p<",
                "measured",
                "increased by",
                "decreased by",
                "os resultados",
                "observamos",
                "encontramos",
                "os dados",
                "figura ",
                "tabela ",
                "significativ",
            ],
            Role::Limitation => &[
                "limitation",
                "however,",
                "although",
                "a caveat",
                "future work",
                "further research",
                "cannot rule out",
                "small sample",
                "bias",
                "limitação",
                "limitações",
                "no entanto",
                "contudo",
                "embora",
                "trabalhos futuros",
                "viés",
            ],
            Role::Conclusion => &[
                "in conclusion",
                "we conclude",
                "to conclude",
                "therefore",
                "thus,",
                "overall,",
                "in summary",
                "taken together",
                "these findings suggest",
                "em conclusão",
                "concluímos",
                "portanto",
                "assim,",
                "em suma",
                "em resumo",
            ],
        }
    }
}

/// The role a paragraph's words suggest (the one with the most cues; ties go to the later role,
/// so "however, the results show" counts as a limitation).
pub fn classify(text: &str) -> Option<Role> {
    let s = text.to_lowercase();
    let mut best: Option<(usize, Role)> = None;
    for r in Role::ALL {
        let n = r.cues().iter().filter(|c| s.contains(*c)).count();
        if n > 0 && best.is_none_or(|(b, _)| n >= b) {
            best = Some((n, r));
        }
    }
    best.map(|(_, r)| r)
}

/// A paragraph's key for hand-set roles: its first words.
fn role_key(text: &str) -> String {
    text.split_whitespace().take(10).collect::<Vec<_>>().join(" ")
}

/// A card in the argument map.
#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    pub role: Role,
    pub text: String,
    pub path: Path,
    /// Set by hand.
    pub manual: bool,
}

/// The argument map of the open document.
pub fn claims(g: &GlassApp) -> Vec<Claim> {
    let doc = &g.app.session.doc;
    let pkey = crate::project::key(&g.app.session);
    let roles = g.glass.projects.by_doc.get(&pkey).map(|p| p.roles.clone()).unwrap_or_default();
    let mut out = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        if path.depth() > 0 {
            continue;
        }
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        if doc.styles.resolve_para(&p.props).outline_level.is_some() {
            continue;
        }
        let text = p.plain_text();
        if text.split_whitespace().count() < 5 {
            continue;
        }
        let key = role_key(&text);
        let (role, manual) = match roles.get(&key) {
            Some(r) if r == "none" => continue,
            Some(r) => match Role::from_name(r) {
                Some(r) => (Some(r), true),
                None => (classify(&text), false),
            },
            None => (classify(&text), false),
        };
        if let Some(role) = role {
            out.push(Claim { role, text, path, manual });
        }
        if out.len() >= 400 {
            break;
        }
    }
    out
}

/// The argument map panel: the four roles in order, joined by arrows.
pub fn argument_map(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let all = claims(g);
    ui.add(
        egui::Label::new(
            egui::RichText::new(
                "Paragraphs sorted by the part they play. Right-click a card to change it; set the role of the paragraph at the cursor below.",
            )
            .font(regular(11.5))
            .color(t.text_dim),
        )
        .wrap(),
    );
    ui.add_space(4.0);
    let pkey = crate::project::key(&g.app.session);
    let caret = g.app.session.sel.focus.path.clone();
    let caret_text = g.app.session.doc.para(StoryRef::Body, &caret).map(|p| p.plain_text()).unwrap_or_default();
    let mut set: Option<(String, &'static str)> = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("This paragraph:").font(regular(11.5)).color(t.text_dim));
        for r in Role::ALL {
            if ui.small_button(r.name()).clicked() && !caret_text.trim().is_empty() {
                set = Some((role_key(&caret_text), r.name()));
            }
        }
        if ui.small_button("None").clicked() && !caret_text.trim().is_empty() {
            set = Some((role_key(&caret_text), "none"));
        }
    });
    ui.add_space(6.0);
    let mut go: Option<Path> = None;
    egui::ScrollArea::vertical().id_salt("glass_argument").auto_shrink([false, false]).show(ui, |ui| {
        for (k, role) in Role::ALL.into_iter().enumerate() {
            if k > 0 {
                // An arrow down to the next role.
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
                let c = t.text_dim;
                let x = r.center().x;
                ui.painter().line_segment([pos2(x, r.min.y + 2.0), pos2(x, r.max.y - 4.0)], Stroke::new(1.5, c));
                ui.painter().line_segment([pos2(x - 4.0, r.max.y - 8.0), pos2(x, r.max.y - 3.0)], Stroke::new(1.5, c));
                ui.painter().line_segment([pos2(x + 4.0, r.max.y - 8.0), pos2(x, r.max.y - 3.0)], Stroke::new(1.5, c));
            }
            let mine: Vec<&Claim> = all.iter().filter(|c| c.role == role).collect();
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 4.5, role.color());
                ui.label(egui::RichText::new(format!("{} · {}", role.name().to_uppercase(), mine.len())).font(semibold(10.5)).color(t.text_dim));
            });
            if mine.is_empty() {
                ui.label(
                    egui::RichText::new(format!("No {} found yet", role.name().to_lowercase()))
                        .font(regular(11.5))
                        .color(t.text_dim.gamma_multiply(0.7)),
                );
            }
            for c in mine.iter().take(40) {
                let w = ui.available_width();
                let excerpt: String = c.text.chars().take(150).collect();
                let galley =
                    ui.painter().layout(format!("{excerpt}{}", if c.text.chars().count() > 150 { "…" } else { "" }), regular(12.0), t.text, w - 22.0);
                let (r, resp) = ui.allocate_exact_size(vec2(w, galley.size().y + 14.0), Sense::click());
                ui.painter().rect_filled(r, 10.0, t.fill);
                paint_state(ui, r, 10, &resp, c.path == caret, t);
                ui.painter().rect_filled(Rect::from_min_size(r.min + vec2(0.0, 6.0), vec2(3.0, r.height() - 12.0)), 2.0, role.color());
                ui.painter().galley(pos2(r.min.x + 12.0, r.min.y + 7.0), galley, t.text);
                if c.manual {
                    ui.painter().circle_filled(pos2(r.max.x - 7.0, r.min.y + 7.0), 2.5, role.color());
                }
                if resp.clicked() {
                    go = Some(c.path.clone());
                }
                resp.context_menu(|ui| {
                    for r in Role::ALL {
                        if ui.button(format!("Mark as {}", r.name())).clicked() {
                            set = Some((role_key(&c.text), r.name()));
                            ui.close();
                        }
                    }
                    if ui.button("Leave out of the map").clicked() {
                        set = Some((role_key(&c.text), "none"));
                        ui.close();
                    }
                    if c.manual && ui.button("Let WordCraft decide").clicked() {
                        set = Some((role_key(&c.text), ""));
                        ui.close();
                    }
                });
            }
        }
    });
    if let Some((key, role)) = set {
        let p = g.glass.projects.by_doc.entry(pkey).or_default();
        if role.is_empty() {
            p.roles.remove(&key);
        } else if p.roles.len() < 2000 {
            p.roles.insert(key, role.to_string());
        }
    }
    if let Some(path) = go {
        go_to(g, &Pos::new(StoryRef::Body, path, 0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_are_sorted_into_roles() {
        assert_eq!(classify("We hypothesize that shade lowers soil temperature."), Some(Role::Hypothesis));
        assert_eq!(classify("The results show a significant drop (p < 0.01)."), Some(Role::Evidence));
        assert_eq!(classify("However, the small sample is a limitation."), Some(Role::Limitation));
        assert_eq!(classify("Em conclusão, a sombra importa."), Some(Role::Conclusion));
        assert_eq!(classify("The walk took an hour."), None);
    }

    #[test]
    fn works_cited_together_are_joined() {
        let r = |k: &str, a: &str, kw: &[&str]| Node {
            key: k.into(),
            r: Ref { key: k.into(), authors: vec![a.into()], keywords: kw.iter().map(|s| s.to_string()).collect(), ..Default::default() },
            at: Vec::new(),
        };
        let g = build(
            vec![r("a", "Silva, Ana", &["shade"]), r("b", "Lee, Jo", &["shade"]), r("c", "Silva, Rui", &[]), r("d", "Ng, Al", &[])],
            &[vec!["a".into(), "d".into()]],
        );
        let has = |x: usize, y: usize, why: Why| g.edges.iter().any(|(a, b, _, w)| (*a, *b) == (x, y) && *w == why);
        assert!(has(0, 3, Why::CitedTogether));
        assert!(has(0, 2, Why::SameAuthor));
        assert!(has(0, 1, Why::SameTopic));
        assert!(!g.edges.iter().any(|(a, b, _, _)| (*a, *b) == (1, 3)));
        let mut pos = HashMap::new();
        for (i, n) in g.nodes.iter().enumerate() {
            pos.insert(n.key.clone(), (seed(&n.key, i, 4), egui::Vec2::ZERO));
        }
        for _ in 0..200 {
            settle(&mut pos, &g);
        }
        assert!(pos.values().all(|(p, _)| (0.0..=1.0).contains(&p.x) && (0.0..=1.0).contains(&p.y)));
    }
}

//! Research mode: tools for academic writing, turned on like Writer Pro. It adds the Research
//! tab and these panels and marks:
//!
//! - **Library**: the reference library ([`crate::refs`]): import BibTeX, RIS, CSL-JSON and
//!   EndNote XML, link a file a reference manager keeps up to date, sync from Zotero; search;
//!   click a reference to cite it (WordCraft's own `CITATION` fields, so the bibliography and its
//!   twelve styles work as in Word).
//! - **Hover cards**: point at a citation to see its reference: authors, title, where and when
//!   it was published, the abstract, the DOI, and whether the bibliography lists it.
//! - **Citation check**: citations without a source, sources never cited, a missing
//!   bibliography, and sentences that make a claim ("studies show…", "according to…",
//!   percentages) with no citation, marked on the page.
//!
//! The other Research tools live in their own modules and are reached from here.

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_doc::para::{InlineObject, OBJ};
use wordcraft_doc::{Path, Pos, StoryRef};
use wordcraft_engine::Selection;
use wordcraft_ui_egui::FileDialogRequest;
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::refs::{self, Library, Ref};
use crate::tokens::{Tokens, paint_state, radius};
use crate::workspace::PickFor;
use crate::{GlassApp, Panel};

/// What Research mode's side panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RView {
    Library,
    Check,
    Formulas,
    Graph,
    Argument,
    Data,
    Code,
    Submit,
}

impl RView {
    pub fn title(self) -> &'static str {
        match self {
            RView::Library => "Library",
            RView::Check => "Citation Check",
            RView::Formulas => "Formulas",
            RView::Graph => "Citation Graph",
            RView::Argument => "Argument Map",
            RView::Data => "Linked Data",
            RView::Code => "Code",
            RView::Submit => "Submit",
        }
    }
}

/// Research mode's state.
#[derive(Default)]
pub struct Research {
    /// The library, loaded on first use.
    pub library: Option<Library>,
    pub search: String,
    /// A Zotero sync running in the background.
    zotero: Option<std::sync::mpsc::Receiver<Result<Vec<Ref>, String>>>,
    /// When linked files were last checked (seconds).
    synced_at: f64,
    /// Citation check results for a (document, revision).
    check: Option<((u64, u64), std::sync::Arc<Check>)>,
    /// A citation under the pointer and since when (for its card).
    hover: Option<(Path, usize, f64)>,
}

// ---------------------------------------------------------------------------------------------
// The library.

impl GlassApp {
    fn library_path(&self) -> Option<std::path::PathBuf> {
        self.workspace.as_ref().map(|w| w.join("Library").join("library.json"))
    }

    /// The reference library (loaded from the workspace the first time).
    pub fn library_mut(&mut self) -> &mut Library {
        if self.glass.research.library.is_none() {
            let lib = self.library_path().map(|p| Library::load(&p)).unwrap_or_default();
            self.glass.research.library = Some(lib);
        }
        self.glass.research.library.get_or_insert_with(Library::default)
    }

    /// Save the library if it changed; read linked files that changed; pick up a Zotero sync.
    pub(crate) fn research_tick(&mut self) {
        let now = wordcraft_ui_egui::now_ms() / 1000.0;
        if self.glass.research.library.is_some() && now - self.glass.research.synced_at > 3.0 {
            self.glass.research.synced_at = now;
            let n = self.library_mut().sync_linked();
            if n > 0 {
                self.app.status(format!("Library: {n} references updated from a linked file"));
            }
        }
        if let Some(rx) = &self.glass.research.zotero {
            match rx.try_recv() {
                Ok(Ok(refs)) => {
                    self.glass.research.zotero = None;
                    let (a, u) = self.library_mut().merge(refs);
                    self.app.status(format!("Zotero: {a} new references, {u} updated"));
                }
                Ok(Err(e)) => {
                    self.glass.research.zotero = None;
                    self.app.status(e);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.glass.research.zotero = None,
            }
        }
        let path = self.library_path();
        if let (Some(p), Some(lib)) = (path, self.glass.research.library.as_mut())
            && lib.dirty
            && let Err(e) = lib.save(&p)
        {
            lib.dirty = false;
            self.app.status(format!("Couldn't save the library: {e}"));
        }
    }

    /// Start a Zotero sync in the background.
    pub fn sync_zotero(&mut self) {
        if self.glass.research.zotero.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(refs::fetch_zotero());
        });
        self.glass.research.zotero = Some(rx);
        self.app.status("Asking Zotero for your library…");
    }

    /// Cite `r` at the caret (adding it to the document's sources).
    pub fn cite(&mut self, r: &Ref) {
        if let Err(e) = self.app.run("references.citation", json!({"source": r.to_source(), "tag": r.key})) {
            self.app.status(e);
        }
    }

    /// Import references from a file into the library.
    pub fn import_refs(&mut self, path: &std::path::Path, link: bool) {
        let result = if link {
            self.library_mut().link(path)
        } else {
            std::fs::metadata(path)
                .map_err(|e| e.to_string())
                .and_then(|m| if m.len() > refs::MAX_FILE { Err("the file is too large".to_string()) } else { Ok(()) })
                .and_then(|_| std::fs::read(path).map_err(|e| e.to_string()))
                .and_then(|b| refs::parse_file(&path.to_string_lossy(), &b))
                .map(|r| self.library_mut().merge(r))
        };
        match result {
            Ok((a, u)) => self.app.status(format!(
                "{a} references added{}{}",
                if u > 0 { format!(", {u} updated") } else { String::new() },
                if link { "; the file is linked" } else { "" }
            )),
            Err(e) => self.app.status(format!("Couldn't read references: {e}")),
        }
    }

    /// Turn Research mode on or off.
    pub fn set_research(&mut self, on: bool) {
        self.glass.prefs.research = on;
        if on {
            if self.glass.view != crate::View::Home {
                self.app.ui.tab = "Research".into();
            }
            self.app.status("Research mode is on: Library, citations, figures, LaTeX, code and submission tools");
        } else {
            if self.app.ui.tab == "Research" {
                self.app.ui.tab = "Home".into();
            }
            if matches!(self.glass.panel, Some(Panel::Research(_))) {
                self.glass.panel = None;
            }
        }
    }

    /// Show (or hide) a Research panel.
    pub fn research_panel(&mut self, v: RView) {
        self.glass.panel = if self.glass.panel == Some(Panel::Research(v)) { None } else { Some(Panel::Research(v)) };
    }
}

/// The citation styles WordCraft's bibliography knows.
pub const STYLES: [&str; 12] = [
    "APA",
    "Chicago",
    "GB/T 7714",
    "GOST (name sort)",
    "GOST (title sort)",
    "Harvard (Anglia)",
    "IEEE",
    "ISO 690 (author-date)",
    "ISO 690 (numerical)",
    "MLA",
    "SIST02",
    "Turabian",
];

fn header(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, title: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).font(semibold(15.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::widgets::icon_capsule(ui, t, "close", "Close", 26.0, false, true).clicked() {
                g.glass.panel = None;
            }
        });
    });
    ui.add_space(6.0);
}

fn hint(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).font(regular(12.0)).color(t.text_dim)).wrap());
}

/// The side panel for Research mode.
pub fn panel(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, v: RView) {
    header(g, ui, t, v.title());
    match v {
        RView::Library => library(g, ui, t),
        RView::Check => check_panel(g, ui, t),
        RView::Formulas => crate::formulas::panel(g, ui, t),
        RView::Graph => crate::graphs::citation_graph(g, ui, t),
        RView::Argument => crate::graphs::argument_map(g, ui, t),
        RView::Data => crate::figures::data_panel(g, ui, t),
        RView::Code => crate::codeblocks::panel(g, ui, t),
        RView::Submit => crate::submit::panel(g, ui, t),
    }
}

fn library(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        if crate::widgets::quiet_button(ui, t, "Import…", None, 28.0).on_hover_text("BibTeX, RIS (EndNote, Mendeley, Zotero), CSL-JSON or EndNote XML").clicked() {
            g.ask_file(FileDialogRequest::Open { purpose: "references".into() }, PickFor::ImportRefs);
        }
        if crate::widgets::quiet_button(ui, t, "Link a File…", None, 28.0)
            .on_hover_text("A .bib or .ris file your reference manager keeps up to date (Zotero with Better BibTeX, JabRef, Mendeley): read again whenever it changes")
            .clicked()
        {
            g.ask_file(FileDialogRequest::Open { purpose: "references".into() }, PickFor::LinkRefs);
        }
        let busy = g.glass.research.zotero.is_some();
        if crate::widgets::quiet_button(ui, t, if busy { "Syncing…" } else { "Sync Zotero" }, None, 28.0)
            .on_hover_text("Read your library from the Zotero app on this computer (Zotero 7: Settings › Advanced › Allow other applications to communicate with Zotero)")
            .clicked()
        {
            g.sync_zotero();
        }
        if crate::widgets::quiet_button(ui, t, "Export .bib…", None, 28.0).clicked() {
            let bib = g.library_mut().to_bibtex(None);
            g.ask_file(FileDialogRequest::Save { name: "references.bib".into() }, PickFor::SaveText(bib));
        }
    });
    ui.add_space(8.0);
    // Style and bibliography.
    ui.horizontal(|ui| {
        let cur = g.app.session.bib_style.clone();
        egui::ComboBox::from_id_salt("glass_cite_style").selected_text(format!("Style: {cur}")).width(170.0).show_ui(ui, |ui| {
            for s in STYLES {
                if ui.selectable_label(cur == s, s).clicked()
                    && let Err(e) = g.app.run("references.citationStyle", json!({"style": s}))
                {
                    g.app.status(e);
                }
            }
        });
        if crate::widgets::quiet_button(ui, t, "Insert Bibliography", None, 28.0).clicked()
            && let Err(e) = g.app.run("references.bibliography", json!({}))
        {
            g.app.status(e);
        }
    });
    ui.add_space(8.0);
    let mut search = g.glass.research.search.clone();
    if ui
        .add(
            egui::TextEdit::singleline(&mut search)
                .hint_text("Search authors, titles, keywords, DOI")
                .desired_width(ui.available_width())
                .margin(vec2(10.0, 7.0)),
        )
        .changed()
    {
        g.glass.research.search = search.clone();
    }
    ui.add_space(6.0);
    let q = search.trim().to_lowercase();
    let lib = g.library_mut().clone();
    let shown: Vec<&Ref> = lib.refs.iter().filter(|r| q.is_empty() || r.haystack().contains(&q)).take(400).collect();
    // References this document already cites that aren't in the library yet.
    let in_doc: Vec<wordcraft_doc::Source> = g.app.session.doc.sources.iter().filter(|s| lib.find(&s.tag).is_none()).cloned().collect();
    ui.label(
        egui::RichText::new(format!(
            "{} references{}",
            lib.refs.len(),
            if lib.linked.is_empty() { String::new() } else { format!(" · {} linked file(s)", lib.linked.len()) }
        ))
        .font(regular(11.5))
        .color(t.text_dim),
    );
    ui.add_space(4.0);
    let mut cite: Option<Ref> = None;
    let mut remove: Option<String> = None;
    let mut add: Option<Ref> = None;
    egui::ScrollArea::vertical().id_salt("glass_library").auto_shrink([false, false]).show(ui, |ui| {
        if lib.refs.is_empty() {
            hint(
                ui,
                t,
                "Your library is empty. Import a .bib or .ris file exported from Zotero, Mendeley, EndNote or a database; link one your reference manager keeps up to date; or sync from Zotero.",
            );
        }
        for r in &shown {
            let resp = ref_row(ui, t, r);
            let resp = if r.abstract_.is_empty() { resp } else { resp.on_hover_text(r.abstract_.chars().take(600).collect::<String>()) };
            if resp.clicked() {
                cite = Some((*r).clone());
            }
            resp.context_menu(|ui| {
                if ui.button("Cite here").clicked() {
                    cite = Some((*r).clone());
                    ui.close();
                }
                if ui.button("Copy citation key").clicked() {
                    ui.ctx().copy_text(r.key.clone());
                    ui.close();
                }
                if ui.button("Copy BibTeX").clicked() {
                    ui.ctx().copy_text(r.to_bibtex());
                    ui.close();
                }
                if !r.doi.is_empty() && ui.button("Open DOI").clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://doi.org/{}", r.doi)));
                    ui.close();
                }
                if ui.button("Remove from library").clicked() {
                    remove = Some(r.key.clone());
                    ui.close();
                }
            });
        }
        if !in_doc.is_empty() {
            ui.add_space(10.0);
            ui.label(egui::RichText::new("IN THIS DOCUMENT, NOT IN THE LIBRARY").font(semibold(10.5)).color(t.text_dim));
            for s in &in_doc {
                let r = Ref::from_source(s);
                if ref_row(ui, t, &r).on_hover_text("Click to add it to your library").clicked() {
                    add = Some(r);
                }
            }
        }
    });
    if let Some(r) = cite {
        g.cite(&r);
    }
    if let Some(k) = remove {
        g.library_mut().remove(&k);
    }
    if let Some(r) = add {
        g.library_mut().merge(vec![r]);
    }
}

/// A reference in a list: title, then authors · year · where.
fn ref_row(ui: &mut Ui, t: &Tokens, r: &Ref) -> egui::Response {
    let w = ui.available_width();
    let title = if r.title.is_empty() { r.key.clone() } else { r.title.clone() };
    let galley = ui.painter().layout(title, medium(12.5), t.text, w - 16.0);
    let h = galley.size().y + 24.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    paint_state(ui, rect, 8, &resp, false, t);
    ui.painter().galley(pos2(rect.min.x + 8.0, rect.min.y + 5.0), galley, t.text);
    let meta = [r.short_authors(), r.year.clone(), r.container.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
    ui.painter().with_clip_rect(rect.shrink(2.0)).text(
        pos2(rect.min.x + 8.0, rect.max.y - 9.0),
        Align2::LEFT_CENTER,
        meta,
        regular(11.0),
        t.text_dim,
    );
    resp
}

// ---------------------------------------------------------------------------------------------
// Citations in the document.

/// A citation in the document: where it is and what it cites.
#[derive(Clone, Debug, PartialEq)]
pub struct DocCite {
    pub pos: Pos,
    /// Its end (the field's end, or the object after it).
    pub end: Pos,
    pub tags: Vec<String>,
    /// References carried by the citation itself (Zotero's fields keep their item data).
    pub inline_refs: Vec<Ref>,
}

/// The tags a `CITATION` field cites (`CITATION silva2024 \p 4 \m lee2019`).
pub fn citation_tags(instr: &str) -> Vec<String> {
    let mut words = instr.split_whitespace();
    if !words.next().is_some_and(|w| w.eq_ignore_ascii_case("CITATION")) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut take_next = true;
    for w in words {
        if w.eq_ignore_ascii_case("\\m") {
            take_next = true;
            continue;
        }
        if w.starts_with('\\') {
            take_next = false;
            continue;
        }
        if take_next {
            out.push(w.trim_matches('"').to_string());
            take_next = false;
        }
    }
    out
}

/// Every citation in the body: `CITATION` fields and Zotero's citation fields.
pub fn doc_citations(doc: &wordcraft_doc::Document) -> Vec<DocCite> {
    let mut out = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        let offs = p.object_offsets();
        for (k, off) in offs.iter().enumerate() {
            let pos = Pos::new(StoryRef::Body, path.clone(), *off);
            match p.object_at(*off) {
                Some(InlineObject::Field { instr, .. }) if instr.trim_start().to_ascii_uppercase().starts_with("CITATION") => {
                    let end = Pos { off: off + OBJ.len_utf8(), ..pos.clone() };
                    out.push(DocCite { pos, end, tags: citation_tags(instr), inline_refs: Vec::new() });
                }
                Some(InlineObject::FieldStart { instr, .. }) if instr.contains("ZOTERO_ITEM") => {
                    let json_part = instr.find('{').and_then(|i| instr.get(i..)).unwrap_or("");
                    let refs: Vec<Ref> = serde_json::from_str::<serde_json::Value>(json_part)
                        .ok()
                        .and_then(|v| v.get("citationItems").and_then(|c| c.as_array()).cloned())
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|it| it.get("itemData").and_then(refs::from_csl))
                        .collect();
                    // The field's end: the next FieldEnd in this paragraph.
                    let end_off = offs
                        .iter()
                        .skip(k + 1)
                        .find(|o| matches!(p.object_at(**o), Some(InlineObject::FieldEnd)))
                        .map(|o| o + OBJ.len_utf8())
                        .unwrap_or(p.text.len());
                    let end = Pos { off: end_off, ..pos.clone() };
                    out.push(DocCite { pos, end, tags: refs.iter().map(|r| r.key.clone()).collect(), inline_refs: refs });
                }
                _ => {}
            }
        }
        if out.len() > 20_000 {
            break;
        }
    }
    out
}

/// What a citation tag refers to: the document's source, filled in from the library.
pub fn lookup(g: &mut GlassApp, tag: &str) -> Option<Ref> {
    let from_doc = g.app.session.doc.sources.iter().find(|s| s.tag == tag).map(Ref::from_source);
    let from_lib = g.library_mut().find(tag).cloned();
    match (from_doc, from_lib) {
        (Some(mut d), Some(l)) => {
            if d.abstract_.is_empty() {
                d.abstract_ = l.abstract_;
            }
            if d.doi.is_empty() {
                d.doi = l.doi;
            }
            Some(d)
        }
        (d, l) => d.or(l),
    }
}

// ---------------------------------------------------------------------------------------------
// The citation check.

/// One problem the check found.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub kind: FindingKind,
    pub text: String,
    /// Where to go (`None`: the whole document).
    pub at: Option<(Pos, Pos)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindingKind {
    /// A citation whose source isn't in the document.
    NoSource,
    /// A source the document lists but never cites.
    Uncited,
    /// Citations but no bibliography.
    NoBibliography,
    /// A sentence that seems to need a citation.
    Claim,
}

#[derive(Debug, Default)]
pub struct Check {
    pub findings: Vec<Finding>,
}

/// Words that usually introduce a claim needing a source (English and Portuguese).
const CUES: [&str; 34] = [
    "studies show",
    "studies have shown",
    "research shows",
    "research suggests",
    "research indicates",
    "evidence suggests",
    "evidence shows",
    "it has been shown",
    "it has been reported",
    "it has been argued",
    "it is well known",
    "it is widely",
    "according to",
    "previous studies",
    "previous research",
    "prior work",
    "has been widely",
    "experts agree",
    "scholars argue",
    "researchers found",
    "data show",
    "estudos mostram",
    "estudos indicam",
    "pesquisas mostram",
    "pesquisas indicam",
    "segundo ",
    "de acordo com",
    "evidências sugerem",
    "a literatura",
    "estudos anteriores",
    "investigações anteriores",
    "sabe-se que",
    "é amplamente",
    "os dados mostram",
];

/// Sentences of `text` as byte ranges.
pub fn sentences(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (k, (i, c)) in chars.iter().enumerate() {
        if matches!(c, '.' | '!' | '?') && chars.get(k + 1).is_none_or(|(_, n)| n.is_whitespace()) {
            let end = i + c.len_utf8();
            if text.get(start..end).is_some_and(|s| !s.trim().is_empty()) {
                out.push((start, end));
            }
            start = end;
        }
    }
    if text.get(start..).is_some_and(|s| !s.trim().is_empty()) {
        out.push((start, text.len()));
    }
    out
}

/// Whether a sentence makes a claim that wants a source.
pub fn claims(sentence: &str) -> bool {
    let s = sentence.to_lowercase();
    if CUES.iter().any(|c| s.contains(c)) {
        return true;
    }
    // A percentage or a statistic.
    let b = s.as_bytes();
    b.windows(2).any(|w| w.first().is_some_and(u8::is_ascii_digit) && w.get(1) == Some(&b'%')) || s.contains(" per cent") || s.contains(" por cento")
}

/// Run the check on the open document.
pub fn run_check(g: &mut GlassApp) -> std::sync::Arc<Check> {
    let key = (g.app.session.document_id(), g.app.session.rev());
    if let Some((k, c)) = &g.glass.research.check
        && *k == key
    {
        return c.clone();
    }
    let doc = &g.app.session.doc;
    let cites = doc_citations(doc);
    let mut findings = Vec::new();
    let tags: std::collections::HashSet<&str> = doc.sources.iter().map(|s| s.tag.as_str()).collect();
    for c in &cites {
        if c.inline_refs.is_empty() {
            for tag in c.tags.iter().filter(|t| !tags.contains(t.as_str())) {
                findings.push(Finding {
                    kind: FindingKind::NoSource,
                    text: format!("“{tag}” is cited but has no source"),
                    at: Some((c.pos.clone(), c.end.clone())),
                });
            }
        }
    }
    let cited: std::collections::HashSet<&str> = cites.iter().flat_map(|c| c.tags.iter().map(String::as_str)).collect();
    for s in &doc.sources {
        if !cited.contains(s.tag.as_str()) {
            findings.push(Finding {
                kind: FindingKind::Uncited,
                text: format!("“{}” is a source but is never cited", if s.title.is_empty() { &s.tag } else { &s.title }),
                at: None,
            });
        }
    }
    let has_bib = doc.para_paths(StoryRef::Body).iter().any(|p| {
        doc.para(StoryRef::Body, p).is_some_and(|para| {
            para.object_offsets().iter().any(|o| match para.object_at(*o) {
                Some(InlineObject::Field { instr, .. }) | Some(InlineObject::FieldStart { instr, .. }) => {
                    let u = instr.to_ascii_uppercase();
                    u.contains("BIBLIOGRAPHY") || u.contains("ZOTERO_BIBL")
                }
                _ => false,
            })
        })
    });
    if !cites.is_empty() && !has_bib {
        findings.push(Finding { kind: FindingKind::NoBibliography, text: "The document cites sources but has no bibliography yet".into(), at: None });
    }
    // Claims without a citation in the same sentence.
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        let style = doc.styles.resolve_para(&p.props);
        if style.outline_level.is_some() {
            continue;
        }
        let offs = p.object_offsets();
        let cite_offs: Vec<usize> = cites.iter().filter(|c| c.pos.path == path).map(|c| c.pos.off).collect();
        let _ = offs;
        for (a, b) in sentences(&p.text) {
            let Some(sent) = p.text.get(a..b) else { continue };
            if claims(sent) && !cite_offs.iter().any(|o| (a..b + 2).contains(o)) {
                let excerpt: String = sent.trim().chars().take(90).collect();
                findings.push(Finding {
                    kind: FindingKind::Claim,
                    text: format!("Needs a citation? “{excerpt}{}”", if sent.trim().chars().count() > 90 { "…" } else { "" }),
                    at: Some((Pos::new(StoryRef::Body, path.clone(), a), Pos::new(StoryRef::Body, path.clone(), b))),
                });
            }
            if findings.len() > 2000 {
                break;
            }
        }
    }
    let c = std::sync::Arc::new(Check { findings });
    g.glass.research.check = Some((key, c.clone()));
    c
}

fn check_panel(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let c = run_check(g);
    let mut marks = g.glass.prefs.cite_marks;
    if ui.checkbox(&mut marks, "Mark sentences that need a citation").changed() {
        g.glass.prefs.cite_marks = marks;
    }
    ui.add_space(6.0);
    if c.findings.is_empty() {
        hint(ui, t, "No problems found: every citation has a source, every source is cited, and no claim is left without a citation.");
        return;
    }
    ui.label(egui::RichText::new(format!("{} things to look at", c.findings.len())).font(regular(11.5)).color(t.text_dim));
    let mut go: Option<(Pos, Pos)> = None;
    let mut bib = false;
    egui::ScrollArea::vertical().id_salt("glass_cite_check").auto_shrink([false, false]).show(ui, |ui| {
        for f in &c.findings {
            let color = match f.kind {
                FindingKind::NoSource | FindingKind::NoBibliography => Color32::from_rgb(0xD4, 0x4C, 0x47),
                FindingKind::Uncited => Color32::from_rgb(0x78, 0x77, 0x74),
                FindingKind::Claim => Color32::from_rgb(0xD9, 0x73, 0x0D),
            };
            let w = ui.available_width();
            let galley = ui.painter().layout(f.text.clone(), regular(12.0), t.text, w - 24.0);
            let (r, resp) = ui.allocate_exact_size(vec2(w, galley.size().y + 12.0), Sense::click());
            paint_state(ui, r, 8, &resp, false, t);
            ui.painter().circle_filled(pos2(r.min.x + 9.0, r.min.y + 12.0), 3.5, color);
            ui.painter().galley(pos2(r.min.x + 20.0, r.min.y + 6.0), galley, t.text);
            if resp.clicked() {
                match (&f.at, f.kind) {
                    (Some(at), _) => go = Some(at.clone()),
                    (None, FindingKind::NoBibliography) => bib = true,
                    _ => {}
                }
            }
        }
    });
    if let Some((a, b)) = go {
        g.app.session.sel = Selection { anchor: a, focus: b };
        g.app.canvas.scroll_to_caret = true;
        g.glass.view = crate::View::Page;
    }
    if bib && let Err(e) = g.app.run("references.bibliography", json!({})) {
        g.app.status(e);
    }
}

// ---------------------------------------------------------------------------------------------
// On the page: claim marks and hover cards.

/// Research mode's marks over the page, and the card for the citation under the pointer.
pub fn on_page(
    g: &mut GlassApp,
    ui: &Ui,
    painter: &egui::Painter,
    rects_of: &mut dyn FnMut(&mut GlassApp, &Pos, &Pos) -> Vec<(usize, Rect)>,
    t: &Tokens,
) {
    if g.glass.prefs.cite_marks {
        let c = run_check(g);
        let amber = Color32::from_rgb(0xD9, 0x73, 0x0D);
        for f in c.findings.iter().filter(|f| f.kind == FindingKind::Claim).take(300) {
            let Some((a, b)) = &f.at else { continue };
            for (_, r) in rects_of(g, a, b) {
                if !r.intersects(painter.clip_rect()) {
                    continue;
                }
                let y = r.max.y - 1.0;
                let mut x = r.min.x;
                while x < r.max.x {
                    painter.circle_filled(pos2(x, y), 0.9, amber);
                    x += 3.5;
                }
            }
        }
    }
    if !g.glass.prefs.hover_cards {
        return;
    }
    let Some(p) = ui.ctx().pointer_hover_pos() else {
        g.glass.research.hover = None;
        return;
    };
    if !painter.clip_rect().contains(p) || ui.ctx().egui_is_using_pointer() {
        g.glass.research.hover = None;
        return;
    }
    let cites = doc_citations(&g.app.session.doc);
    let mut hit: Option<(DocCite, Rect)> = None;
    for c in cites.into_iter().take(5000) {
        if let Some((_, r)) = rects_of(g, &c.pos, &c.end).into_iter().find(|(_, r)| r.expand(2.0).contains(p)) {
            hit = Some((c, r));
            break;
        }
    }
    let Some((c, r)) = hit else {
        g.glass.research.hover = None;
        return;
    };
    let now = ui.input(|i| i.time);
    let since = match &g.glass.research.hover {
        Some((path, off, at)) if *path == c.pos.path && *off == c.pos.off => *at,
        _ => {
            g.glass.research.hover = Some((c.pos.path.clone(), c.pos.off, now));
            now
        }
    };
    painter.rect_stroke(r.expand(1.5), 4.0, Stroke::new(1.0, t.accent.gamma_multiply(0.6)), egui::StrokeKind::Outside);
    if now - since < 0.25 {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
        return;
    }
    let refs: Vec<Ref> = if c.inline_refs.is_empty() { c.tags.iter().filter_map(|tag| lookup(g, tag)).collect() } else { c.inline_refs.clone() };
    let missing: Vec<String> = c.tags.iter().filter(|t| !refs.iter().any(|r| &r.key == *t)).cloned().collect();
    let in_bib = c.inline_refs.is_empty() && c.tags.iter().all(|t| g.app.session.doc.sources.iter().any(|s| &s.tag == t));
    card(ui.ctx(), t, r, &refs, &missing, in_bib, now - since);
}

/// The hover card under (or over) a citation.
fn card(ctx: &egui::Context, t: &Tokens, at: Rect, refs: &[Ref], missing: &[String], in_bib: bool, age: f64) {
    let a = (((age - 0.25) / 0.14) as f32).clamp(0.0, 1.0);
    if a < 1.0 {
        ctx.request_repaint();
    }
    let w = 360.0;
    let screen = ctx.content_rect();
    let x = at.min.x.clamp(screen.min.x + 8.0, (screen.max.x - w - 8.0).max(screen.min.x + 8.0));
    let below = at.max.y + 8.0 + (1.0 - a) * 6.0;
    egui::Area::new(egui::Id::new("glass_cite_card")).order(egui::Order::Tooltip).interactable(true).fixed_pos(pos2(x, below)).show(ctx, |ui| {
        ui.multiply_opacity(a);
        let bg = ui.painter().add(egui::Shape::Noop);
        let inner = egui::Frame::NONE.inner_margin(14).show(ui, |ui| {
            ui.set_width(w - 28.0);
            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
            for (i, r) in refs.iter().enumerate().take(4) {
                if i > 0 {
                    ui.separator();
                }
                ui.label(egui::RichText::new(r.label()).font(medium(11.5)).color(t.accent));
                ui.add(egui::Label::new(egui::RichText::new(&r.title).font(semibold(13.5)).color(t.text)).wrap());
                let names = if r.authors.len() > 6 {
                    format!("{} et al.", r.authors.iter().take(6).cloned().collect::<Vec<_>>().join("; "))
                } else {
                    r.authors.join("; ")
                };
                if !names.is_empty() {
                    ui.add(egui::Label::new(egui::RichText::new(names).font(regular(11.5)).color(t.text_dim)).wrap());
                }
                let place = [
                    r.container.clone(),
                    r.year.clone(),
                    if r.volume.is_empty() { String::new() } else { format!("vol. {}", r.volume) },
                    r.pages.clone(),
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
                if !place.is_empty() {
                    ui.label(egui::RichText::new(place).font(regular(11.5)).color(t.text_dim));
                }
                if !r.abstract_.is_empty() {
                    let abs: String = r.abstract_.chars().take(320).collect();
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("{abs}{}", if r.abstract_.chars().count() > 320 { "…" } else { "" }))
                                .font(regular(12.0))
                                .color(t.text),
                        )
                        .wrap(),
                    );
                }
                ui.horizontal(|ui| {
                    if !r.doi.is_empty() && ui.link(egui::RichText::new(format!("doi:{}", r.doi)).font(regular(11.5))).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://doi.org/{}", r.doi)));
                    } else if !r.url.is_empty() && ui.link(egui::RichText::new("Open link").font(regular(11.5))).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(r.url.clone()));
                    }
                });
            }
            for m in missing {
                ui.label(
                    egui::RichText::new(format!("“{m}” has no source: add it in the Library or with References › Manage Sources"))
                        .font(regular(12.0))
                        .color(Color32::from_rgb(0xD4, 0x4C, 0x47)),
                );
            }
            if !refs.is_empty() {
                let (txt, c) =
                    if in_bib { ("In the bibliography", Color32::from_rgb(0x44, 0x83, 0x61)) } else { ("Cited through Zotero", t.text_dim) };
                ui.label(egui::RichText::new(txt).font(medium(11.0)).color(c));
            }
        });
        ui.painter().set(bg, crate::tokens::popover_shape(inner.response.rect, 14, t));
    });
    let _ = radius::ROW;
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_engine::Session;
    use wordcraft_ui_egui::{Services, WordApp};

    #[test]
    fn citation_tags_are_read() {
        assert_eq!(citation_tags("CITATION silva2024 \\p 4 \\l 2057"), ["silva2024"]);
        assert_eq!(citation_tags(" CITATION a \\m b \\m c"), ["a", "b", "c"]);
        assert!(citation_tags("PAGE").is_empty());
    }

    #[test]
    fn claims_are_spotted_in_two_languages() {
        assert!(claims("Studies show that shade cools streets."));
        assert!(claims("Cerca de 40% das árvores morreram."));
        assert!(claims("Segundo a literatura, o efeito é pequeno."));
        assert!(!claims("We walked to the park."));
        assert_eq!(sentences("One. Two? Three"), [(0, 4), (4, 9), (9, 15)]);
    }

    #[test]
    fn citing_from_the_library_and_checking_the_document() {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default()));
        g.library_mut().merge(refs::parse_bibtex(
            "@article{silva2024, author={Silva, Ana}, title={Shade}, year={2024}, abstract={Trees cool soil.}, doi={10.1/x}}",
        ));
        g.app.run("text.insert", json!({"text": "Studies show shade helps. We agree "})).unwrap();
        let r = g.library_mut().refs[0].clone();
        g.cite(&r);
        let cites = doc_citations(&g.app.session.doc);
        assert_eq!(cites.len(), 1);
        assert_eq!(cites[0].tags, ["silva2024"]);
        let l = lookup(&mut g, "silva2024").unwrap();
        assert_eq!(l.abstract_, "Trees cool soil.", "the library fills in what the document's source lacks");
        let c = run_check(&mut g);
        let kinds: Vec<FindingKind> = c.findings.iter().map(|f| f.kind).collect();
        assert!(kinds.contains(&FindingKind::Claim), "{kinds:?}");
        assert!(kinds.contains(&FindingKind::NoBibliography));
        assert!(!kinds.contains(&FindingKind::NoSource));
        g.app.run("references.bibliography", json!({})).unwrap();
        let c = run_check(&mut g);
        assert!(!c.findings.iter().any(|f| f.kind == FindingKind::NoBibliography));
    }
}

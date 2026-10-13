//! Research mode: getting a paper out of the door.
//!
//! - **Blind review copy**: a copy with the authors taken out: the file's author and "last
//!   modified by", comment and tracked-change authors, custom properties, and the author names
//!   and affiliations you list, wherever they appear in the text.
//! - **LaTeX package**: a `.zip` with `main.tex`, `references.bib` and a `figures/` folder.
//!   Citations become `\cite{key}`, pictures `\includegraphics`, the bibliography
//!   `\bibliography{references}`, and code blocks `verbatim`.
//! - **Submission PDF**: tracked changes accepted, comments left out, fonts embedded; blind if
//!   you ask.
//! - **Compile**: several documents (open tabs and files) into one PDF, each starting on a new
//!   page.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use egui::{Ui, vec2};
use serde_json::json;
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::{Block, Document, Pos, StoryRef};
use wordcraft_engine::{Selection, Session};
use wordcraft_ui_egui::FileDialogRequest;
use wordcraft_ui_egui::theme::{regular, semibold};

use crate::GlassApp;
use crate::codeblocks::{CODE_TAG, OUT_TAG};
use crate::refs::Ref;
use crate::tokens::Tokens;
use crate::workspace::PickFor;

/// The Submit panel's choices.
#[derive(Default)]
pub struct Submit {
    /// Author names to take out of a blind copy (one per line).
    pub names: String,
    /// Affiliations to take out (one per line).
    pub affiliations: String,
    /// Exports (LaTeX, PDF) are blind too.
    pub blind_exports: bool,
    /// Tabs (by uid) to compile, in order; empty: every document tab.
    pub include: Vec<u64>,
    /// Files added to the compilation.
    pub files: Vec<PathBuf>,
    pub prefilled: bool,
}

fn lines(s: &str) -> Vec<String> {
    s.lines().map(str::trim).filter(|l| l.chars().count() >= 2).map(str::to_string).collect()
}

// ---------------------------------------------------------------------------------------------
// Blind review.

/// A copy of `doc` with its authors taken out.
pub fn anonymize(doc: &Document, names: &[String], affiliations: &[String]) -> Document {
    let mut d = doc.clone();
    d.core.creator.clear();
    d.core.last_modified_by.clear();
    d.custom_props.clear();
    let mut who: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for c in d.comments.values_mut() {
        let n = who.len() + 1;
        let label = who.entry(c.author.clone()).or_insert_with(|| format!("Reviewer {n}")).clone();
        c.author = label;
        c.initials = "R".into();
    }
    for r in &mut d.revisions {
        r.author = "Author".into();
    }
    let mut s = Session::new(d);
    for (list, with) in [(names, "[Author]"), (affiliations, "[Affiliation]")] {
        for n in list {
            let _ = s.run("edit.replaceAll", &json!({"text": n, "with": with, "matchCase": false}));
        }
    }
    // The document's own sources can name the authors too, but those are other people's work
    // and stay.
    s.doc
}

// ---------------------------------------------------------------------------------------------
// LaTeX.

/// What a token in the exported text stands for.
enum Token {
    Cite(Vec<String>),
    Figure(String),
    Bibliography,
    Code(String),
}

fn token(n: usize) -> String {
    format!("WCTOKQ{n}QEND")
}

/// The extension a picture's bytes want.
fn picture_ext(key: &str, bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "jpg"
    } else if bytes.starts_with(b"%PDF") {
        "pdf"
    } else {
        let k = key.to_ascii_lowercase();
        if k.ends_with(".jpg") || k.ends_with(".jpeg") { "jpg" } else { "png" }
    }
}

/// A key LaTeX accepts in `\cite{}`.
fn cite_key(k: &str) -> String {
    let s: String = k.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_' | '.')).collect();
    if s.is_empty() { "ref".into() } else { s }
}

/// The `\bibliographystyle` closest to a WordCraft citation style.
fn bst(style: &str) -> &'static str {
    match style {
        "APA" | "Harvard (Anglia)" | "Chicago" | "Turabian" | "ISO 690 (author-date)" => "apalike",
        "IEEE" | "ISO 690 (numerical)" | "GB/T 7714" => "ieeetr",
        _ => "plain",
    }
}

/// The LaTeX package for `doc`: `main.tex`, `references.bib`, `figures/…` as a zip.
pub fn latex_package(doc: &Document, library: &[Ref], style: &str) -> Result<Vec<u8>, String> {
    let mut s = Session::new(doc.clone());
    let mut tokens: Vec<Token> = Vec::new();
    let mut figures: Vec<(String, Arc<Vec<u8>>)> = Vec::new();
    let mut extra_refs: Vec<Ref> = Vec::new();
    // Code blocks and their outputs become verbatim text (bottom up, so indexes hold).
    let code_tables: Vec<(usize, String)> = s
        .doc
        .body
        .iter()
        .enumerate()
        .filter_map(|(i, b)| match b.as_ref() {
            Block::Table(t) if t.props.description.as_deref().is_some_and(|d| d.starts_with(CODE_TAG) || d.starts_with(OUT_TAG)) => {
                let text = t
                    .rows
                    .first()
                    .and_then(|r| r.cells.first())
                    .map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect::<Vec<_>>().join("\n"))
                    .unwrap_or_default();
                Some((i, text))
            }
            _ => None,
        })
        .collect();
    for (i, text) in code_tables.into_iter().rev() {
        let n = tokens.len();
        tokens.push(Token::Code(text));
        let mut b = std::mem::take(&mut s.doc.body);
        if let Some(slot) = b.get_mut(i) {
            *slot = Arc::new(Block::Para(wordcraft_doc::Paragraph::with_text(&token(n), Default::default())));
        }
        s.doc.body = b;
    }
    // The bibliography: its heading paragraph and the entries after it.
    let mut i = 0;
    while i < s.doc.body.len() {
        let is_bib = s.doc.body.get(i).and_then(|b| b.as_para()).is_some_and(|p| {
            p.objects.iter().any(|o| matches!(o, InlineObject::Field { instr, .. } if instr.trim_start().starts_with("BIBLIOGRAPHY")))
        });
        if is_bib {
            let n = tokens.len();
            tokens.push(Token::Bibliography);
            let mut b = std::mem::take(&mut s.doc.body);
            if let Some(slot) = b.get_mut(i) {
                *slot = Arc::new(Block::Para(wordcraft_doc::Paragraph::with_text(&token(n), Default::default())));
            }
            while b.get(i + 1).and_then(|x| x.as_para()).is_some_and(|p| p.props.style.as_deref() == Some("ListEntry")) {
                b.remove(i + 1);
            }
            s.doc.body = b;
        }
        i += 1;
    }
    // Citations and pictures, from the end back.
    let mut edits: Vec<(Pos, Pos, usize)> = Vec::new();
    for c in crate::research::doc_citations(&s.doc) {
        let n = tokens.len();
        extra_refs.extend(c.inline_refs.iter().cloned());
        tokens.push(Token::Cite(c.tags.clone()));
        edits.push((c.pos, c.end, n));
    }
    for path in s.doc.para_paths(StoryRef::Body) {
        let Some(p) = s.doc.para(StoryRef::Body, &path) else { continue };
        for off in p.object_offsets() {
            if let Some(InlineObject::Image { media, .. }) = p.object_at(off)
                && let Some(bytes) = s.doc.media.get(media)
            {
                let n = tokens.len();
                let name = format!("figures/fig{}.{}", figures.len() + 1, picture_ext(media, bytes));
                figures.push((name.clone(), bytes.clone()));
                tokens.push(Token::Figure(name));
                let pos = Pos::new(StoryRef::Body, path.clone(), off);
                let end = Pos { off: off + wordcraft_doc::para::OBJ.len_utf8(), ..pos.clone() };
                edits.push((pos, end, n));
            }
        }
    }
    edits.sort_by(|a, b| b.0.path.cmp(&a.0.path).then(b.0.off.cmp(&a.0.off)));
    for (a, b, n) in edits {
        s.sel = Selection { anchor: a, focus: b };
        let _ = s.run("text.insert", &json!({"text": token(n)}));
    }
    let mut tex = wordcraft_formats::latex::export(&s.doc);
    let mut used_bib = false;
    for (n, tk) in tokens.iter().enumerate() {
        let t = token(n);
        let with = match tk {
            Token::Cite(tags) => format!("\\cite{{{}}}", tags.iter().map(|k| cite_key(k)).collect::<Vec<_>>().join(",")),
            Token::Figure(name) => format!("\\includegraphics[width=\\linewidth]{{{name}}}"),
            Token::Bibliography => {
                used_bib = true;
                format!("\\bibliographystyle{{{}}}\n\\bibliography{{references}}", bst(style))
            }
            Token::Code(code) => {
                let code = code.replace("\\end{verbatim}", "\\end {verbatim}");
                format!("\\begin{{verbatim}}\n{code}\n\\end{{verbatim}}")
            }
        };
        tex = tex.replace(&t, &with);
    }
    let cites = tokens.iter().any(|t| matches!(t, Token::Cite(_)));
    if cites && !used_bib {
        tex = tex.replace("\\end{document}", &format!("\\bibliographystyle{{{}}}\n\\bibliography{{references}}\n\\end{{document}}", bst(style)));
    }
    if !figures.is_empty() && !tex.contains("{graphicx}") {
        tex = tex.replacen("\\usepackage{hyperref}", "\\usepackage{graphicx}\n\\usepackage{hyperref}", 1);
    }
    // references.bib: the document's sources, what Zotero fields carry, and library entries
    // for anything else it cites.
    let mut refs: Vec<Ref> = doc.sources.iter().map(Ref::from_source).collect();
    for r in extra_refs {
        if !refs.iter().any(|x| x.key == r.key) {
            refs.push(r);
        }
    }
    for t in &tokens {
        if let Token::Cite(tags) = t {
            for tag in tags {
                if !refs.iter().any(|x| &x.key == tag)
                    && let Some(r) = library.iter().find(|r| &r.key == tag)
                {
                    refs.push(r.clone());
                }
            }
        }
    }
    let bib: String = refs
        .iter()
        .map(|r| {
            let mut r = r.clone();
            r.key = cite_key(&r.key);
            r.to_bibtex()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    let mut put = |name: &str, bytes: &[u8]| -> Result<(), String> {
        zip.start_file(name, opts).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())
    };
    put("main.tex", tex.as_bytes())?;
    put("references.bib", bib.as_bytes())?;
    for (name, bytes) in &figures {
        put(name, bytes)?;
    }
    zip.finish().map(|c| c.into_inner()).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------------------------
// Compiling several documents.

fn rename_media(b: &mut Block, prefix: &str, depth: usize) {
    match b {
        Block::Para(p) => {
            for o in &mut p.objects {
                if let InlineObject::Image { media, .. } = o {
                    *media = format!("{prefix}{media}");
                }
            }
        }
        Block::Table(t) if depth < 16 => {
            for row in &mut t.rows {
                for cell in &mut row.cells {
                    for b in &mut cell.blocks {
                        rename_media(Arc::make_mut(b), prefix, depth + 1);
                    }
                }
            }
        }
        Block::Table(_) => {}
    }
}

/// `docs` one after another, each starting on a new page.
pub fn compile(docs: &[Document]) -> Result<Document, String> {
    let Some(first) = docs.first() else { return Err("nothing to compile".into()) };
    let mut s = Session::new(first.clone());
    for (k, d) in docs.iter().enumerate().skip(1) {
        let prefix = format!("c{k}_");
        let mut frag = d.body_fragment();
        for b in &mut frag.blocks {
            rename_media(b, &prefix, 0);
        }
        for (key, bytes) in &d.media {
            s.doc.media.insert(format!("{prefix}{key}"), bytes.clone());
        }
        for src in &d.sources {
            if !s.doc.sources.iter().any(|x| x.tag == src.tag) {
                s.doc.sources.push(src.clone());
            }
        }
        s.run("caret.docEnd", &json!({})).map_err(|e| e.to_string())?;
        s.run("insert.pageBreak", &json!({})).map_err(|e| e.to_string())?;
        s.clipboard = Some(frag);
        s.run("edit.paste", &json!({})).map_err(|e| e.to_string())?;
    }
    Ok(s.doc)
}

impl GlassApp {
    /// The document as it goes out: blind when asked.
    fn outgoing(&self) -> Document {
        let sub = &self.glass.submit;
        if sub.blind_exports { anonymize(&self.app.session.doc, &lines(&sub.names), &lines(&sub.affiliations)) } else { self.app.session.doc.clone() }
    }

    /// Open a blind review copy in a new tab.
    pub fn blind_copy(&mut self) {
        let sub = &self.glass.submit;
        let doc = anonymize(&self.app.session.doc, &lines(&sub.names), &lines(&sub.affiliations));
        let mut s = Session::new(doc);
        s.dirty = true;
        self.new_tab(s);
        self.app.status("Blind review copy: authors, affiliations and file details taken out. Save it under a new name.");
    }

    /// The LaTeX package for the open document.
    pub fn latex_zip(&mut self) -> Result<Vec<u8>, String> {
        let lib = self.library_mut().refs.clone();
        let style = self.app.session.bib_style.clone();
        latex_package(&self.outgoing(), &lib, &style)
    }

    /// Write a submission PDF of the open document to `path`.
    pub fn submission_pdf(&mut self, path: &str) {
        let mut s = Session::new(self.outgoing());
        let _ = s.run("review.acceptAll", &json!({}));
        let _ = s.run("references.updateFields", &json!({}));
        match s.run("file.exportPdf", &json!({"path": path, "markup": false})) {
            Ok(_) => self.app.status(format!("Submission PDF saved: {path}")),
            Err(e) => self.app.status(format!("Couldn't save the PDF: {e}")),
        }
    }

    /// The documents chosen for compiling, in order.
    fn compile_docs(&self) -> Vec<Document> {
        let sub = &self.glass.submit;
        let mut out = Vec::new();
        for (i, tab) in self.docs.tabs.iter().enumerate() {
            if tab.db.is_some() || !(sub.include.is_empty() || sub.include.contains(&tab.uid)) {
                continue;
            }
            if let Some(s) = self.tab_session(i) {
                out.push(s.doc.clone());
            }
        }
        if self.docs.tabs.is_empty() {
            out.push(self.app.session.doc.clone());
        }
        for f in &sub.files {
            match wordcraft_engine::io::open_path(f) {
                Ok(d) => out.push(d),
                Err(e) => log::warn!("compile: {}: {e}", f.display()),
            }
        }
        out
    }

    /// Compile the chosen documents into one PDF at `path`.
    pub fn compile_pdf(&mut self, path: &str) {
        let docs = self.compile_docs();
        let n = docs.len();
        let r = compile(&docs).and_then(|d| {
            let mut s = Session::new(d);
            let _ = s.run("references.updateFields", &json!({}));
            s.run("file.exportPdf", &json!({"path": path, "markup": false})).map(|_| ()).map_err(|e| e.to_string())
        });
        match r {
            Ok(()) => self.app.status(format!("{n} documents compiled into {path}")),
            Err(e) => self.app.status(format!("Couldn't compile: {e}")),
        }
    }

    /// Open the compilation as a document to look over before exporting.
    pub fn compile_to_tab(&mut self) {
        match compile(&self.compile_docs()) {
            Ok(d) => {
                let mut s = Session::new(d);
                s.dirty = true;
                self.new_tab(s);
            }
            Err(e) => self.app.status(format!("Couldn't compile: {e}")),
        }
    }
}

/// The Submit panel.
pub fn panel(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    if !g.glass.submit.prefilled {
        g.glass.submit.prefilled = true;
        let me = g.app.session.doc.core.creator.trim().to_string();
        if !me.is_empty() && g.glass.submit.names.is_empty() {
            g.glass.submit.names = me;
        }
    }
    let label = |ui: &mut Ui, s: &str| {
        ui.label(egui::RichText::new(s).font(semibold(10.5)).color(t.text_dim));
    };
    let hint = |ui: &mut Ui, s: &str| {
        ui.add(egui::Label::new(egui::RichText::new(s).font(regular(11.5)).color(t.text_dim)).wrap());
    };
    egui::ScrollArea::vertical().id_salt("glass_submit").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        label(ui, "BLIND PEER REVIEW");
        hint(ui, "Author names to take out (one per line):");
        ui.add(egui::TextEdit::multiline(&mut g.glass.submit.names).desired_rows(2).desired_width(ui.available_width()));
        hint(ui, "Affiliations to take out (one per line):");
        ui.add(egui::TextEdit::multiline(&mut g.glass.submit.affiliations).desired_rows(2).desired_width(ui.available_width()));
        if crate::widgets::quiet_button(ui, t, "Make a Blind Review Copy", None, 28.0)
            .on_hover_text("A new document without the authors: file details, comment and change authors, and the names above")
            .clicked()
        {
            g.blind_copy();
        }
        ui.checkbox(&mut g.glass.submit.blind_exports, "Exports below are blind too");
        ui.add_space(8.0);
        label(ui, "EXPORT");
        ui.horizontal_wrapped(|ui| {
            if crate::widgets::quiet_button(ui, t, "LaTeX Package…", None, 28.0)
                .on_hover_text("main.tex, references.bib and figures/ in a .zip")
                .clicked()
            {
                match g.latex_zip() {
                    Ok(bytes) => {
                        let name = format!("{}-latex.zip", g.app.title_stem());
                        g.ask_file(FileDialogRequest::Save { name }, PickFor::SaveBytes(bytes));
                    }
                    Err(e) => g.app.status(format!("Couldn't make the LaTeX package: {e}")),
                }
            }
            if crate::widgets::quiet_button(ui, t, "Submission PDF…", None, 28.0)
                .on_hover_text("Changes accepted, no comments, fonts embedded")
                .clicked()
            {
                let name = format!("{}.pdf", g.app.title_stem());
                g.ask_file(FileDialogRequest::Save { name }, PickFor::SubmissionPdf);
            }
        });
        let c = crate::research::run_check(g);
        let serious = c.findings.iter().filter(|f| f.kind != crate::research::FindingKind::Claim).count();
        if serious > 0 {
            ui.label(
                egui::RichText::new(format!("Citation Check has {serious} thing(s) to fix before submitting"))
                    .font(regular(11.5))
                    .color(egui::Color32::from_rgb(0xD4, 0x4C, 0x47)),
            );
        }
        ui.add_space(8.0);
        label(ui, "COMPILE INTO ONE PDF");
        hint(ui, "Chapters, papers or appendices one after another, each from a new page.");
        let tabs: Vec<(u64, String)> =
            (0..g.docs.tabs.len()).filter(|i| !g.is_db_tab(*i)).filter_map(|i| g.docs.tabs.get(i).map(|tab| (tab.uid, g.tab_title(i)))).collect();
        let all = g.glass.submit.include.is_empty();
        for (uid, title) in &tabs {
            let mut on = all || g.glass.submit.include.contains(uid);
            if ui.checkbox(&mut on, title).changed() {
                let inc = &mut g.glass.submit.include;
                if inc.is_empty() {
                    *inc = tabs.iter().map(|(u, _)| *u).collect();
                }
                if on {
                    inc.push(*uid);
                } else {
                    inc.retain(|u| u != uid);
                }
                if inc.is_empty() {
                    inc.push(u64::MAX);
                }
            }
        }
        let mut drop: Option<usize> = None;
        for (k, f) in g.glass.submit.files.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                        .font(regular(12.0))
                        .color(t.text),
                );
                if ui.small_button("Remove").clicked() {
                    drop = Some(k);
                }
            });
        }
        if let Some(k) = drop {
            g.glass.submit.files.remove(k);
        }
        ui.horizontal_wrapped(|ui| {
            if crate::widgets::quiet_button(ui, t, "Add a File…", None, 28.0).clicked() {
                g.ask_file(FileDialogRequest::Open { purpose: "open".into() }, PickFor::AddToCompile);
            }
            if crate::widgets::quiet_button(ui, t, "Preview", None, 28.0).on_hover_text("Open the compiled document in a new tab").clicked() {
                g.compile_to_tab();
            }
            if crate::widgets::quiet_button(ui, t, "Compile PDF…", None, 28.0).clicked() {
                g.ask_file(FileDialogRequest::Save { name: "Compiled.pdf".into() }, PickFor::CompilePdf);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with(text: &str) -> Document {
        let mut s = Session::new(Document::new());
        s.run("text.insert", &json!({"text": text})).unwrap();
        s.doc
    }

    #[test]
    fn a_blind_copy_has_no_authors() {
        let mut d = doc_with("Ana Silva, University of Lisbon. Results follow.");
        d.core.creator = "Ana Silva".into();
        d.core.last_modified_by = "Ana Silva".into();
        let b = anonymize(&d, &["Ana Silva".into()], &["University of Lisbon".into()]);
        assert!(b.core.creator.is_empty() && b.core.last_modified_by.is_empty());
        let text = b.plain_text(StoryRef::Body);
        assert!(!text.contains("Silva") && !text.contains("Lisbon"), "{text}");
        assert!(text.contains("[Author]") && text.contains("[Affiliation]") && text.contains("Results follow."));
    }

    #[test]
    fn the_latex_package_cites_and_lists_its_references() {
        let mut s = Session::new(Document::new());
        s.run("text.insert", &json!({"text": "Trees cool streets "})).unwrap();
        let r = crate::refs::parse_bibtex("@article{silva2024, author={Silva, Ana}, title={Shade}, journal={Urban Climate}, year={2024}}").remove(0);
        s.run("references.citation", &json!({"source": r.to_source(), "tag": r.key})).unwrap();
        s.run("text.insert", &json!({"text": "."})).unwrap();
        s.run("text.newParagraph", &json!({})).unwrap();
        s.run("references.bibliography", &json!({})).unwrap();
        let zip = latex_package(&s.doc, &[], "APA").unwrap();
        let mut a = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
        let mut tex = String::new();
        std::io::Read::read_to_string(&mut a.by_name("main.tex").unwrap(), &mut tex).unwrap();
        assert!(tex.contains("\\cite{silva2024}"), "{tex}");
        assert!(tex.contains("\\bibliography{references}") && tex.contains("apalike"));
        assert!(!tex.contains("WCTOK"));
        let mut bib = String::new();
        std::io::Read::read_to_string(&mut a.by_name("references.bib").unwrap(), &mut bib).unwrap();
        assert!(bib.contains("@article{silva2024"), "{bib}");
    }

    #[test]
    fn documents_compile_one_after_another() {
        let d = compile(&[doc_with("Chapter one"), doc_with("Chapter two")]).unwrap();
        let text = d.plain_text(StoryRef::Body);
        let (a, b) = (text.find("Chapter one").unwrap(), text.find("Chapter two").unwrap());
        assert!(a < b);
        assert!(compile(&[]).is_err());
    }
}

//! Figures, tables and data for Research mode.
//!
//! - **Captions that number themselves**: Figure, Table and Equation captions are Word's `SEQ`
//!   fields; whenever their order changes (a section moved, a figure added) the numbers and the
//!   cross-references to them are brought up to date.
//! - **Linked data**: a table or a chart made from a data file (CSV/TSV, Excel, OpenDocument,
//!   or what an R or Python script writes). When the file changes, the table or chart in the
//!   document updates by itself. The link is kept in the table's or chart's alt-text description
//!   (`wcplus-data:` and the path), so it travels with the `.docx`.

use egui::Ui;
use serde_json::{Value, json};
use wordcraft_doc::para::{InlineObject, OBJ};
use wordcraft_doc::{Block, Pos, StoryRef};
use wordcraft_engine::Selection;
use wordcraft_ui_egui::FileDialogRequest;
use wordcraft_ui_egui::theme::{medium, regular};

use crate::GlassApp;
use crate::tokens::Tokens;
use crate::workspace::PickFor;

/// The alt-text prefix that marks a linked table or chart.
pub const TAG: &str = "wcplus-data:";
/// Most rows and columns a linked table takes.
const MAX_ROWS: usize = 300;
const MAX_COLS: usize = 20;

/// Something in the document linked to a data file.
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    /// A table: its top-level block index.
    Table { block: usize, path: String },
    /// A chart: where its object is.
    Chart { at: Pos, path: String },
}

impl Link {
    pub fn path(&self) -> &str {
        match self {
            Link::Table { path, .. } | Link::Chart { path, .. } => path,
        }
    }
}

/// Every linked table and chart in the body.
pub fn links(doc: &wordcraft_doc::Document) -> Vec<Link> {
    let mut out = Vec::new();
    for (i, b) in doc.body.iter().enumerate() {
        if let Block::Table(t) = b.as_ref()
            && let Some(p) = t.props.description.as_deref().and_then(|d| d.strip_prefix(TAG))
        {
            out.push(Link::Table { block: i, path: p.trim().to_string() });
        }
    }
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        for off in p.object_offsets() {
            if let Some(InlineObject::Graphic { alt, .. }) = p.object_at(off)
                && let Some(f) = alt.strip_prefix(TAG)
            {
                out.push(Link::Chart { at: Pos::new(StoryRef::Body, path.clone(), off), path: f.trim().to_string() });
            }
        }
    }
    out
}

/// The rows of a data file (capped).
pub fn read_rows(path: &str) -> Result<Vec<Vec<String>>, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > crate::db::MAX_FILE {
        return Err("the file is too large".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let rows = crate::db::table_from_file(path, &bytes)?;
    let rows: Vec<Vec<String>> =
        rows.into_iter().filter(|r| r.iter().any(|c| !c.trim().is_empty())).take(MAX_ROWS).map(|r| r.into_iter().take(MAX_COLS).collect()).collect();
    if rows.is_empty() {
        return Err("the file has no rows".into());
    }
    Ok(rows)
}

/// Chart data from rows: the first column names the categories, each other numeric column is
/// a series named by its header.
pub fn chart_data(rows: &[Vec<String>]) -> Value {
    let header = rows.first().cloned().unwrap_or_default();
    let body = rows.get(1..).unwrap_or_default();
    let categories: Vec<String> = body.iter().map(|r| r.first().cloned().unwrap_or_default()).collect();
    let mut series = Vec::new();
    for c in 1..header.len().max(1) {
        let values: Vec<Value> =
            body.iter().map(|r| r.get(c).and_then(|v| crate::db::parse_number(v)).map(|n| json!(n)).unwrap_or(Value::Null)).collect();
        if values.iter().any(|v| !v.is_null()) {
            series.push(json!({"name": header.get(c).cloned().unwrap_or_else(|| format!("Series {c}")), "values": values}));
        }
    }
    json!({"categories": categories, "series": series})
}

/// The text of a table, row by row.
fn table_text(t: &wordcraft_doc::Table) -> Vec<Vec<String>> {
    t.rows
        .iter()
        .map(|r| {
            r.cells
                .iter()
                .map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect::<Vec<_>>().join("\n").trim().to_string())
                .collect()
        })
        .collect()
}

fn trimmed(rows: &[Vec<String>]) -> Vec<Vec<String>> {
    rows.iter().map(|r| r.iter().map(|c| c.trim().to_string()).collect()).collect()
}

impl GlassApp {
    /// Write `rows` as a table at the caret, linked to `path`.
    fn put_table(&mut self, rows: &[Vec<String>], path: &str) -> Result<(), String> {
        let (start, _) = self.app.session.sel.ordered();
        self.app.session.sel = Selection::caret(start.clone());
        // Start the table in a paragraph of its own.
        if self.app.session.doc.para_at(&start).is_some_and(|p| !p.text.trim().is_empty()) {
            self.app.run("text.newParagraph", json!({}))?;
        }
        let first = self.app.session.sel.focus.clone();
        for (i, r) in rows.iter().enumerate() {
            if i > 0 {
                self.app.run("text.newParagraph", json!({}))?;
            }
            let line: Vec<String> = r.iter().map(|c| c.replace(['\t', '\n'], " ")).collect();
            self.app.run("text.insert", json!({"text": line.join("\t")}))?;
        }
        let last = self.app.session.sel.focus.clone();
        self.app.session.sel = Selection { anchor: Pos { off: 0, ..first }, focus: last };
        self.app.run("table.fromText", json!({"separator": "tab"}))?;
        self.app.run("table.altText", json!({"title": "Linked data", "text": format!("{TAG}{path}")}))?;
        let _ = self.app.run("table.repeatHeader", json!({"value": true}));
        Ok(())
    }

    /// Insert a table made from a data file, linked to it.
    pub fn link_table(&mut self, path: &str) {
        let r = read_rows(path).and_then(|rows| self.put_table(&rows, path));
        match r {
            Ok(()) => {
                self.remember_mtime(path);
                self.app.status("Linked table inserted: it updates when the file changes");
            }
            Err(e) => self.app.status(format!("Couldn't link the table: {e}")),
        }
    }

    /// Insert a chart made from a data file, linked to it.
    pub fn link_chart(&mut self, path: &str) {
        let rows = match read_rows(path) {
            Ok(r) => r,
            Err(e) => {
                self.app.status(format!("Couldn't link the chart: {e}"));
                return;
            }
        };
        let (start, _) = self.app.session.sel.ordered();
        self.app.session.sel = Selection::caret(start.clone());
        let title = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if let Err(e) = self.app.run("insert.chart", json!({"type": "column", "data": chart_data(&rows), "title": title})) {
            self.app.status(e);
            return;
        }
        if self.app.session.doc.para_at(&start).and_then(|p| p.object_at(start.off)).is_some() {
            self.app.session.sel = Selection { anchor: start.clone(), focus: Pos { off: start.off + OBJ.len_utf8(), ..start } };
            let _ = self.app.run("object.altText", json!({"text": format!("{TAG}{path}")}));
        }
        self.remember_mtime(path);
        self.app.status("Linked chart inserted: it updates when the file changes");
    }

    fn remember_mtime(&mut self, path: &str) {
        let m = file_mtime(path);
        self.glass.data_mtimes.insert(path.to_string(), m);
    }

    /// Bring linked tables and charts up to date with their files (`force`: even unchanged).
    pub fn refresh_links(&mut self, force: bool) -> usize {
        let mut n = 0;
        let saved_sel = self.app.session.sel.clone();
        for link in links(&self.app.session.doc).into_iter().rev() {
            let path = link.path().to_string();
            let m = file_mtime(&path);
            if m == 0 || (!force && self.glass.data_mtimes.get(&path) == Some(&m)) {
                continue;
            }
            self.glass.data_mtimes.insert(path.clone(), m);
            let Ok(rows) = read_rows(&path) else { continue };
            match link {
                Link::Table { block, .. } => {
                    let current = match self.app.session.doc.body.get(block).map(|b| b.as_ref()) {
                        Some(Block::Table(t)) => table_text(t),
                        _ => continue,
                    };
                    if trimmed(&current) == trimmed(&rows) {
                        continue;
                    }
                    // Into the table, delete it, and write it again where it was.
                    let Some(cell) = self.app.session.doc.para_paths(StoryRef::Body).into_iter().find(|p| crate::top_of(p) == block) else {
                        continue;
                    };
                    self.app.session.sel = Selection::caret(Pos::new(StoryRef::Body, cell, 0));
                    if self.app.run("table.deleteTable", json!({})).is_err() {
                        continue;
                    }
                    if self.put_table(&rows, &path).is_ok() {
                        n += 1;
                    }
                }
                Link::Chart { at, .. } => {
                    self.app.session.sel = Selection { anchor: at.clone(), focus: Pos { off: at.off + OBJ.len_utf8(), ..at } };
                    if self.app.run("chart.editData", json!({"data": chart_data(&rows)})).is_ok() {
                        n += 1;
                    }
                }
            }
        }
        self.app.session.sel = saved_sel;
        if self.app.session.doc.para_at(&self.app.session.sel.focus).is_none() {
            let _ = self.app.run("caret.docStart", json!({}));
        }
        n
    }

    /// Keep captions numbered in order and linked data fresh (Research mode, a few times a
    /// second at most).
    pub(crate) fn figures_tick(&mut self) {
        let now = wordcraft_ui_egui::now_ms() / 1000.0;
        if now - self.glass.figures_at < 2.0 {
            return;
        }
        self.glass.figures_at = now;
        if seq_out_of_order(&self.app.session.doc) {
            let _ = self.app.run("references.updateFields", json!({}));
        }
        if self.glass.prefs.auto_refresh_data {
            let n = self.refresh_links(false);
            if n > 0 {
                self.app.status(format!("{n} linked table(s) or chart(s) updated from their files"));
            }
        }
    }
}

fn file_mtime(path: &str) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Whether the body's simple `SEQ` fields (captions) show numbers out of order: each label's
/// fields should read 1, 2, 3… in document order.
pub fn seq_out_of_order(doc: &wordcraft_doc::Document) -> bool {
    let mut counts: std::collections::HashMap<String, u32> = Default::default();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        for off in p.object_offsets() {
            if let Some(InlineObject::Field { instr, result, .. }) = p.object_at(off) {
                let mut w = instr.split_whitespace();
                if !w.next().is_some_and(|x| x.eq_ignore_ascii_case("SEQ")) {
                    continue;
                }
                // Formats other than arabic numbers, and restarts, are left to Update Fields.
                if instr.contains("\\s") || instr.contains("\\r") || (instr.contains("\\*") && !instr.to_ascii_uppercase().contains("ARABIC")) {
                    continue;
                }
                let label = w.next().unwrap_or("").to_string();
                let n = counts.entry(label).or_insert(0);
                *n += 1;
                if result.trim() != n.to_string() {
                    return true;
                }
            }
        }
    }
    false
}

pub fn data_panel(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    ui.label(egui::RichText::new("CAPTIONS").font(medium(11.0)).color(t.text_dim));
    ui.add(
        egui::Label::new(
            egui::RichText::new("Numbers update themselves when figures and tables move; cross-references follow.")
                .font(regular(12.0))
                .color(t.text_dim),
        )
        .wrap(),
    );
    ui.horizontal_wrapped(|ui| {
        for (label, what) in [("Figure Caption", "Figure"), ("Table Caption", "Table"), ("Equation Caption", "Equation")] {
            if crate::widgets::quiet_button(ui, t, label, None, 28.0).clicked()
                && let Err(e) = g.app.run("references.caption", json!({"label": what}))
            {
                g.app.status(e);
            }
        }
        if crate::widgets::quiet_button(ui, t, "Cross-reference…", None, 28.0).clicked()
            && let Err(e) = g.app.run("insert.crossReference", json!({}))
        {
            g.app.status(e);
        }
        if crate::widgets::quiet_button(ui, t, "Update Numbers", None, 28.0).clicked()
            && let Err(e) = g.app.run("references.updateFields", json!({}))
        {
            g.app.status(e);
        }
    });
    ui.add_space(12.0);
    ui.label(egui::RichText::new("LINKED DATA").font(medium(11.0)).color(t.text_dim));
    ui.add(
        egui::Label::new(egui::RichText::new("Make a table or chart from a CSV, Excel or OpenDocument file (or what your R or Python script writes). It updates when the file changes.").font(regular(12.0)).color(t.text_dim))
            .wrap(),
    );
    ui.horizontal_wrapped(|ui| {
        if crate::widgets::quiet_button(ui, t, "Table from File…", None, 28.0).clicked() {
            g.ask_file(FileDialogRequest::Open { purpose: "data".into() }, PickFor::LinkTable);
        }
        if crate::widgets::quiet_button(ui, t, "Chart from File…", None, 28.0).clicked() {
            g.ask_file(FileDialogRequest::Open { purpose: "data".into() }, PickFor::LinkChart);
        }
        if crate::widgets::quiet_button(ui, t, "Refresh All", None, 28.0).clicked() {
            let n = g.refresh_links(true);
            g.app.status(format!("{n} linked table(s) or chart(s) updated"));
        }
    });
    let mut auto = g.glass.prefs.auto_refresh_data;
    if ui.checkbox(&mut auto, "Update automatically when files change").changed() {
        g.glass.prefs.auto_refresh_data = auto;
    }
    ui.add_space(6.0);
    let list = links(&g.app.session.doc);
    if list.is_empty() {
        ui.label(egui::RichText::new("No linked tables or charts yet.").font(regular(12.0)).color(t.text_dim));
    }
    for l in list {
        let (kind, path) = match &l {
            Link::Table { path, .. } => ("Table", path.clone()),
            Link::Chart { path, .. } => ("Chart", path.clone()),
        };
        let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(path.clone());
        let ok = std::path::Path::new(&path).is_file();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(kind).font(medium(12.0)).color(t.accent));
            ui.label(egui::RichText::new(&name).font(regular(12.0)).color(if ok { t.text } else { egui::Color32::from_rgb(0xD4, 0x4C, 0x47) }))
                .on_hover_text(if ok { path.clone() } else { format!("{path} (not found)") });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_engine::Session;
    use wordcraft_ui_egui::{Services, WordApp};

    #[test]
    fn a_linked_table_follows_its_file() {
        let dir = std::env::temp_dir().join(format!("wordcraft-data-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("results.csv");
        std::fs::write(&csv, "Site,Moisture\nA,18.4\nB,24.9\n").unwrap();
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default()));
        let path = csv.to_string_lossy().to_string();
        g.link_table(&path);
        let l = links(&g.app.session.doc);
        assert_eq!(l.len(), 1, "{:?}", g.app.status_msg);
        assert_eq!(l[0].path(), path);
        let Some(Block::Table(t)) = g.app.session.doc.body.iter().map(|b| b.as_ref()).find(|b| matches!(b, Block::Table(_))) else {
            panic!("no table")
        };
        assert_eq!(table_text(t), [["Site", "Moisture"], ["A", "18.4"], ["B", "24.9"]]);
        std::fs::write(&csv, "Site,Moisture\nA,18.4\nB,24.9\nC,31.2\n").unwrap();
        assert_eq!(g.refresh_links(true), 1);
        let Some(Block::Table(t)) = g.app.session.doc.body.iter().map(|b| b.as_ref()).find(|b| matches!(b, Block::Table(_))) else {
            panic!("no table")
        };
        assert_eq!(table_text(t).len(), 4);
        assert_eq!(links(&g.app.session.doc).len(), 1, "still linked");
        g.app.session.sel = Selection::caret(Pos::new(StoryRef::Body, wordcraft_doc::Path::top(g.app.session.doc.body.len() - 1), 0));
        g.link_chart(&path);
        assert_eq!(links(&g.app.session.doc).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn captions_out_of_order_are_noticed() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.run("references.caption", &json!({"label": "Figure", "text": "first"})).unwrap();
        s.run("text.newParagraph", &json!({})).unwrap();
        s.run("references.caption", &json!({"label": "Figure", "text": "second"})).unwrap();
        assert!(!seq_out_of_order(&s.doc));
        // Swap the two caption paragraphs: their numbers now read 2, 1.
        let caps: Vec<usize> =
            s.doc.body.iter().enumerate().filter(|(_, b)| b.as_para().is_some_and(|p| p.plain_text().contains("Figure"))).map(|(i, _)| i).collect();
        s.doc.body.swap(caps[0], caps[1]);
        let firsts: Vec<String> = s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect();
        if firsts.iter().filter(|t| t.contains("Figure")).count() == 2 {
            assert!(seq_out_of_order(&s.doc), "{firsts:?}");
            s.run("references.updateFields", &json!({})).unwrap();
            assert!(!seq_out_of_order(&s.doc));
        }
        assert!(chart_data(&[vec!["x".into(), "y".into()], vec!["a".into(), "1".into()]])["series"][0]["values"][0] == 1.0);
    }
}

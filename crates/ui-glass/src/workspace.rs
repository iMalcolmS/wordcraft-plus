//! The workspace: where databases live (a `Databases` folder, by default in
//! `Documents/WordCraft`), database tabs, saving them as they change, and what a database page
//! asks of the app: opening a record as a page, importing and exporting files, and handing
//! records to Mail Merge.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use serde_json::json;
use wordcraft_engine::Session;
use wordcraft_ui_egui::{FileDialogAnswer, FileDialogRequest};

use crate::db::{self, Database};
use crate::db_view::Action;
use crate::{GlassApp, View};

/// What a file picked in a Glass file dialog is for.
pub enum PickFor {
    /// Open a saved database, or a table or contacts as a new one.
    OpenDatabase,
    /// Add the file's records to the database in tab `uid`.
    ImportInto(u64),
    /// Write this CSV there.
    SaveCsv(String),
}

/// A Glass file dialog waiting for its answer.
pub struct PendingPick {
    rx: Receiver<FileDialogAnswer>,
    what: PickFor,
}

/// Saves wait this long after the last change (seconds), so typing doesn't write on every key.
const SAVE_AFTER: f64 = 1.0;

/// `Documents/WordCraft` in the home folder, if there is a home folder.
pub fn default_workspace() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|h| !h.is_empty())?;
    Some(PathBuf::from(home).join("Documents").join("WordCraft"))
}

/// A file name made from `name`: letters, digits, spaces and a few marks, at most 80 characters.
pub fn file_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')' | '\'') { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let s: String = s.trim_matches('.').chars().take(80).collect();
    if s.is_empty() { "Untitled".into() } else { s }
}

/// `dir/stem.ext`, or `dir/stem 2.ext`… when that is taken.
fn unique(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..10_000).map(|n| dir.join(format!("{stem} {n}.{ext}"))).find(|p| !p.exists()).unwrap_or(first)
}

impl GlassApp {
    /// Where databases and their pages are kept (`None`: nowhere; databases then live only in
    /// their tabs until exported).
    pub fn set_workspace(&mut self, dir: Option<PathBuf>) {
        self.workspace = dir;
    }

    pub fn databases_dir(&self) -> Option<PathBuf> {
        self.workspace.as_ref().map(|w| w.join("Databases"))
    }

    /// The databases saved in the workspace: (name, path, icon), newest first. Read from disk at
    /// most every few seconds.
    pub fn workspace_databases(&mut self) -> Vec<(String, PathBuf, String)> {
        let now = wordcraft_ui_egui::now_ms() / 1000.0;
        if now - self.glass.db_list.0 < 3.0 && self.glass.db_list.0 > 0.0 {
            return self.glass.db_list.1.clone();
        }
        let mut out: Vec<(std::time::SystemTime, String, PathBuf, String)> = Vec::new();
        if let Some(dir) = self.databases_dir()
            && let Ok(entries) = std::fs::read_dir(&dir)
        {
            for e in entries.flatten().take(500) {
                let p = e.path();
                if p.extension().is_some_and(|x| x == db::EXT)
                    && let Ok(d) = Database::open(&p)
                {
                    let when = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                    out.push((when, d.name, p, d.icon));
                }
            }
        }
        out.sort_by_key(|b| std::cmp::Reverse(b.0));
        let list: Vec<(String, PathBuf, String)> = out.into_iter().map(|(_, n, p, i)| (n, p, i)).collect();
        self.glass.db_list = (now, list.clone());
        list
    }

    /// The open tab's database.
    pub fn active_db_mut(&mut self) -> Option<&mut Database> {
        let i = self.docs.active;
        self.docs.tabs.get_mut(i).and_then(|t| t.db.as_deref_mut())
    }

    pub fn active_db(&self) -> Option<&Database> {
        self.docs.tabs.get(self.docs.active).and_then(|t| t.db.as_deref())
    }

    /// Open `db` in a tab (the open one when it is an untouched blank document), giving it a
    /// file in the workspace when it has none.
    pub fn open_database(&mut self, mut db: Database) {
        if let Some(p) = db.path.clone()
            && let Some(i) =
                (0..self.docs.tabs.len()).find(|i| self.docs.tabs.get(*i).and_then(|t| t.db.as_ref()).and_then(|d| d.path.as_ref()) == Some(&p))
        {
            self.switch_to(i);
            return;
        }
        if db.path.is_none()
            && let Some(dir) = self.databases_dir()
        {
            db.path = Some(unique(&dir, &file_stem(&db.name), db::EXT));
            db.dirty = true;
        }
        self.ensure_tabs();
        if !self.active_is_pristine() {
            self.new_tab(Session::new(wordcraft_engine::doc::Document::new()));
        }
        let i = self.docs.active;
        if let Some(t) = self.docs.tabs.get_mut(i) {
            t.db = Some(Box::new(db));
        }
        self.glass.dbui = Default::default();
        self.glass.view = View::Page;
        self.docs.switched_at = wordcraft_ui_egui::now_ms() / 1000.0;
        self.save_databases(true);
    }

    /// Open a database file (`.wcdb`), or import a table or contacts file as a new database.
    pub fn open_database_path(&mut self, path: &Path) {
        let is_db = path.extension().is_some_and(|x| x == db::EXT);
        let result = if is_db {
            Database::open(path)
        } else {
            let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Imported".into());
            std::fs::metadata(path)
                .map_err(|e| e.to_string())
                .and_then(|m| if m.len() > db::MAX_FILE { Err("the file is too large".to_string()) } else { Ok(()) })
                .and_then(|_| std::fs::read(path).map_err(|e| e.to_string()))
                .and_then(|bytes| db::table_from_file(&path.to_string_lossy(), &bytes))
                .map(|table| {
                    let mut d = Database::from_table(&name, &table);
                    if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("vcf")) {
                        d.icon = "people".into();
                    }
                    d
                })
        };
        match result {
            Ok(d) => self.open_database(d),
            Err(e) => self.app.status(format!("Couldn't open {}: {e}", path.display())),
        }
    }

    /// Save databases that changed: all of them when `force`, else those left alone for a
    /// moment.
    pub fn save_databases(&mut self, force: bool) {
        let now = wordcraft_ui_egui::now_ms() / 1000.0;
        let any = self.docs.tabs.iter().any(|t| t.db.as_ref().is_some_and(|d| d.dirty));
        if !any {
            self.glass.db_dirty_since = None;
            return;
        }
        let since = *self.glass.db_dirty_since.get_or_insert(now);
        if !force && now - since < SAVE_AFTER {
            return;
        }
        self.glass.db_dirty_since = None;
        let dir = self.databases_dir();
        let mut errors = Vec::new();
        for t in &mut self.docs.tabs {
            let Some(d) = t.db.as_deref_mut().filter(|d| d.dirty) else { continue };
            if d.path.is_none()
                && let Some(dir) = &dir
            {
                d.path = Some(unique(dir, &file_stem(&d.name), db::EXT));
            }
            if let Some(p) = d.path.clone()
                && let Err(e) = d.save_to(&p)
            {
                errors.push(e);
                d.dirty = false;
            }
        }
        self.glass.db_list.0 = 0.0;
        if let Some(e) = errors.first() {
            self.app.status(format!("Couldn't save the database: {e}"));
        }
    }

    /// Draw the open database tab, then do what it asked.
    pub(crate) fn database_page(&mut self, ui: &mut egui::Ui, t: &crate::tokens::Tokens) {
        let i = self.docs.active;
        let Some(mut d) = self.docs.tabs.get_mut(i).and_then(|t| t.db.take()) else { return };
        let actions = crate::db_view::show(&mut d, &mut self.glass.dbui, ui, t);
        if let Some(tab) = self.docs.tabs.get_mut(i) {
            tab.db = Some(d);
        }
        for a in actions {
            self.database_action(a);
        }
    }

    fn database_action(&mut self, a: Action) {
        match a {
            Action::OpenPage(rid) => self.open_record_page(rid),
            Action::Import => {
                let uid = self.docs.tabs.get(self.docs.active).map(|t| t.uid).unwrap_or_default();
                self.ask_file(FileDialogRequest::Open { purpose: "database".into() }, PickFor::ImportInto(uid));
            }
            Action::ExportCsv(ids) => {
                let Some(d) = self.active_db() else { return };
                let csv = d.to_csv(Some(&ids));
                let name = format!("{}.csv", file_stem(&d.name));
                self.ask_file(FileDialogRequest::Save { name }, PickFor::SaveCsv(csv));
            }
            Action::MailMerge(ids) => self.merge_from_database(&ids),
            Action::Status(s) => self.app.status(s),
        }
    }

    /// Ask for a file through the host's dialog (or its blocking picker), to finish with
    /// [`Self::finish_pick`].
    pub fn ask_file(&mut self, req: FileDialogRequest, what: PickFor) {
        if self.glass.pick.is_some() {
            return;
        }
        if let Some(show) = &self.app.services.file_dialog {
            let rx = show(req);
            self.glass.pick = Some(PendingPick { rx, what });
            return;
        }
        let picked = match &req {
            FileDialogRequest::Open { purpose } => self.app.services.pick_open.as_ref().and_then(|f| f(purpose)),
            FileDialogRequest::Save { name } => self.app.services.pick_save.as_ref().and_then(|f| f(name)),
        };
        if let Some(p) = picked {
            self.finish_pick(what, &p);
        }
    }

    /// Pick up a Glass file dialog's answer (every frame).
    pub(crate) fn poll_pick(&mut self) {
        let Some(p) = self.glass.pick.as_ref() else { return };
        let answer = match p.rx.try_recv() {
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Ok(None),
        };
        let Some(PendingPick { what, .. }) = self.glass.pick.take() else { return };
        match answer {
            Ok(Some(path)) => self.finish_pick(what, &path),
            Ok(None) => {}
            Err(e) => self.app.status(format!("No file dialog: {e}")),
        }
    }

    fn finish_pick(&mut self, what: PickFor, path: &str) {
        match what {
            PickFor::OpenDatabase => self.open_database_path(Path::new(path)),
            PickFor::ImportInto(uid) => {
                let table = std::fs::metadata(path)
                    .map_err(|e| e.to_string())
                    .and_then(|m| if m.len() > db::MAX_FILE { Err("the file is too large".to_string()) } else { Ok(()) })
                    .and_then(|_| std::fs::read(path).map_err(|e| e.to_string()))
                    .and_then(|b| db::table_from_file(path, &b));
                match table {
                    Ok(table) => {
                        let n = self
                            .docs
                            .tabs
                            .iter_mut()
                            .find(|t| t.uid == uid)
                            .and_then(|t| t.db.as_deref_mut())
                            .map(|d| d.append_table(&table))
                            .unwrap_or(0);
                        self.app.status(format!("Imported {n} records"));
                    }
                    Err(e) => self.app.status(format!("Couldn't import: {e}")),
                }
            }
            PickFor::SaveCsv(csv) => match std::fs::write(path, csv) {
                Ok(()) => self.app.status(format!("Exported to {path}")),
                Err(e) => self.app.status(format!("Couldn't export: {e}")),
            },
        }
    }

    /// The folder a database's pages are kept in.
    fn pages_dir(d: &Database) -> Option<PathBuf> {
        let p = d.path.as_ref()?;
        let stem = p.file_stem()?.to_string_lossy().to_string();
        Some(p.parent()?.join(format!("{stem} pages")))
    }

    /// Open a record of the open database as a page: its `.docx` if it has one, else a new
    /// document with its title and properties, saved beside the database.
    pub fn open_record_page(&mut self, rid: u64) {
        let Some(d) = self.active_db() else { return };
        let dir = Self::pages_dir(d);
        let existing = d.row(rid).and_then(|r| r.page.clone()).zip(dir.clone()).map(|(f, dir)| dir.join(f)).filter(|p| p.is_file());
        if let Some(p) = existing {
            self.open_path(&p.to_string_lossy());
            return;
        }
        let doc = record_document(d, rid);
        let title = d.title(rid);
        let mut s = Session::new(doc);
        let mut saved_as = None;
        if let Some(dir) = dir {
            let _ = std::fs::create_dir_all(&dir);
            let path = unique(&dir, &file_stem(&title), "docx");
            match s.run("file.saveAs", &json!({"path": path.to_string_lossy()})) {
                Ok(_) => saved_as = path.file_name().map(|f| f.to_string_lossy().to_string()),
                Err(e) => self.app.status(format!("Couldn't save the page: {e}")),
            }
        }
        if let Some(f) = saved_as
            && let Some(d) = self.active_db_mut()
            && let Some(r) = d.row_mut(rid)
        {
            r.page = Some(f);
            d.touch();
        }
        self.save_databases(true);
        self.new_tab(s);
    }

    /// Use records of the open database as Mail Merge recipients: in the document tab last used
    /// (or a new one), with the Mailings tab open.
    pub fn merge_from_database(&mut self, ids: &[u64]) {
        let Some(d) = self.active_db() else { return };
        let (headers, rows) = d.merge_table(Some(ids));
        let n = rows.len();
        let name = d.name.clone();
        let target = (0..self.docs.tabs.len()).rev().find(|i| !self.is_db_tab(*i));
        match target {
            Some(i) => self.switch_to(i),
            None => self.new_tab(Session::new(wordcraft_engine::doc::Document::new())),
        }
        self.glass.view = View::Page;
        match self.app.run("mailings.recipients", json!({"rows": rows, "fields": headers})) {
            Ok(_) => {
                self.app.ui.tab = "Mailings".into();
                self.app.status(format!("{n} recipients from “{name}”: insert merge fields, then Finish & Merge"));
            }
            Err(e) => self.app.status(format!("Mail Merge: {e}")),
        }
    }
}

/// A record's page: its title, its properties, and a Notes heading to write under.
pub fn record_document(d: &Database, rid: u64) -> wordcraft_doc::Document {
    use wordcraft_doc::props::CharProps;
    use wordcraft_doc::{Block, Document, Paragraph};
    let mut doc = Document::new();
    let title = d.title(rid);
    doc.core.title = title.clone();
    let mut blocks: Vec<Block> = vec![Block::Para(Paragraph::with_text(&title, CharProps::default()).styled("Title"))];
    for p in d.props.iter().filter(|p| p.kind != db::Kind::Title) {
        let v = d.text(rid, p.id);
        if v.trim().is_empty() {
            continue;
        }
        let mut para = Paragraph::with_text(&format!("{}: ", p.name), CharProps { bold: Some(true), ..Default::default() });
        let at = para.len();
        let _ = para.insert_text(at, &v, &CharProps::default());
        para.props.space_after = Some(2.0);
        blocks.push(Block::Para(para));
    }
    blocks.push(Block::Para(Paragraph::with_text("Notes", CharProps::default()).styled("Heading1")));
    blocks.push(Block::Para(Paragraph::with_text("", CharProps::default())));
    doc.body = blocks.into_iter().map(std::sync::Arc::new).collect();
    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_ui_egui::{Services, WordApp};

    fn glass() -> GlassApp {
        GlassApp::new(WordApp::new(Session::new(wordcraft_engine::doc::Document::new()), Services::default()))
    }

    #[test]
    fn file_stems_are_safe() {
        assert_eq!(file_stem("Reading / list: 2026?"), "Reading list 2026");
        assert_eq!(file_stem("..."), "Untitled");
        assert_eq!(file_stem("  "), "Untitled");
        assert!(file_stem(&"x".repeat(500)).len() <= 80);
    }

    #[test]
    fn databases_open_in_tabs_save_and_reopen() {
        let dir = std::env::temp_dir().join(format!("wordcraft-ws-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut g = glass();
        g.set_workspace(Some(dir.clone()));
        let mut d = Database::new("Reading list");
        d.add_row_with(&[("Name", "Silent Spring")]);
        g.open_database(d);
        assert_eq!(g.docs.tabs.len(), 1, "the untouched blank tab is reused");
        assert!(g.is_db_tab(0));
        assert_eq!(g.tab_title(0), "Reading list");
        let path = g.active_db().and_then(|d| d.path.clone()).unwrap();
        assert!(path.is_file(), "saved at once");
        // A second open of the same file switches to its tab.
        g.new_tab(Session::new(wordcraft_engine::doc::Document::new()));
        g.open_database_path(&path);
        assert_eq!(g.docs.active, 0);
        assert_eq!(g.workspace_databases().len(), 1);
        // A record opens as a page saved beside the database.
        let rid = g.active_db().map(|d| d.rows[0].id).unwrap();
        g.open_record_page(rid);
        assert!(!g.is_db_tab(g.docs.active));
        assert!(g.app.session.path.as_ref().is_some_and(|p| p.is_file()));
        g.switch_to(0);
        assert!(g.active_db().and_then(|d| d.row(rid)).and_then(|r| r.page.clone()).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn records_become_merge_recipients() {
        let mut g = glass();
        let mut d = Database::new("Guests");
        d.add_prop("Email", db::Kind::Email);
        d.add_row_with(&[("Name", "Ada"), ("Email", "ada@example.org")]);
        d.add_row_with(&[("Name", "Alan"), ("Email", "alan@example.org")]);
        g.open_database(d);
        let ids: Vec<u64> = g.active_db().map(|d| d.rows.iter().map(|r| r.id).collect()).unwrap_or_default();
        g.merge_from_database(&ids);
        assert!(!g.is_db_tab(g.docs.active));
        assert_eq!(g.app.session.merge.rows.len(), 2);
        assert_eq!(g.app.ui.tab, "Mailings");
    }
}

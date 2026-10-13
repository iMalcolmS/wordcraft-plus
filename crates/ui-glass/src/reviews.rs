//! Research mode: the reviewer response log. Reviewers' comments are records in a database
//! (reviewer, comment, your response, where the manuscript changed, status), imported from the
//! review text or from the comments in the manuscript, and turned into a response letter.

use std::path::Path;

use serde_json::json;
use wordcraft_engine::Session;

use crate::GlassApp;
use crate::db::{Database, Kind, Layout, Tint};

/// The name the log's database gets.
pub const NAME: &str = "Reviewer Responses";

/// A new, empty response log (with one example record).
pub fn database() -> Database {
    let mut db = Database::new(NAME);
    db.icon = "list".into();
    db.description = "Every reviewer comment with your response, where the manuscript changed, and whether it's done. Research › Response Letter writes the letter from it.".into();
    db.add_select(
        "Reviewer",
        Kind::Select,
        &[("Editor", Tint::Gray), ("Reviewer 1", Tint::Blue), ("Reviewer 2", Tint::Purple), ("Reviewer 3", Tint::Teal)],
    );
    db.add_select("Status", Kind::Status, &[("To do", Tint::Gray), ("Drafted", Tint::Orange), ("Done", Tint::Green)]);
    db.add_prop("Comment", Kind::Text);
    db.add_prop("Response", Kind::Text);
    db.add_prop("Changes", Kind::Text);
    db.add_row_with(&[
        ("Name", "R1.1"),
        ("Reviewer", "Reviewer 1"),
        ("Status", "To do"),
        ("Comment", "The sample size is small; please justify it or discuss the limitation."),
        ("Response", ""),
        ("Changes", ""),
    ]);
    db.add_view("By status", Layout::Board);
    db
}

/// A comment found in a review.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewComment {
    pub reviewer: String,
    pub number: String,
    pub text: String,
}

/// Whether a line starts a reviewer's section ("Reviewer 2", "Referee #1:", "Revisor 3",
/// "Editor's comments"), and the reviewer's name.
fn reviewer_heading(line: &str) -> Option<String> {
    let l = line.trim().trim_start_matches(['#', '*', '-', ' ']).trim();
    let lower = l.to_lowercase();
    if l.chars().count() > 60 {
        return None;
    }
    for (word, canon) in
        [("reviewer", "Reviewer"), ("referee", "Reviewer"), ("revisor", "Reviewer"), ("parecerista", "Reviewer"), ("avaliador", "Reviewer")]
    {
        if let Some(rest) = lower.strip_prefix(word) {
            let id: String = rest.trim_start_matches([' ', '#', '.']).chars().take_while(|c| c.is_alphanumeric()).collect();
            if !id.is_empty() {
                return Some(format!("{canon} {}", id.to_uppercase()));
            }
        }
    }
    if lower.starts_with("editor") || lower.starts_with("associate editor") || lower.starts_with("comments from the editor") {
        return Some("Editor".into());
    }
    None
}

/// Whether a line starts a numbered comment ("1.", "2)", "Comment 3:", "R1.4", "• "), and the
/// rest of the line.
fn numbered(line: &str) -> Option<(String, String)> {
    let l = line.trim_start();
    for bullet in ["• ", "- ", "* "] {
        if let Some(rest) = l.strip_prefix(bullet) {
            return Some((String::new(), rest.trim().to_string()));
        }
    }
    let lower = l.to_lowercase();
    let body = ["comment", "point", "comentário", "questão"]
        .iter()
        .find_map(|w| lower.strip_prefix(w).map(|r| l.get(l.len() - r.len()..).unwrap_or(r)))
        .unwrap_or(l);
    let body = body.trim_start();
    let body = body.strip_prefix('R').filter(|r| r.starts_with(|c: char| c.is_ascii_digit())).unwrap_or(body);
    let num: String = body.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let num = num.trim_end_matches('.').to_string();
    if num.is_empty() || num.len() > 8 {
        return None;
    }
    let rest = body
        .get(
            body.len()
                .min(body.char_indices().take_while(|(_, c)| c.is_ascii_digit() || *c == '.').map(|(i, c)| i + c.len_utf8()).last().unwrap_or(0))..,
        )
        .unwrap_or("");
    let rest = rest.trim_start();
    // "1. Text", "1) Text", "1: Text", "1 - Text".
    let rest = rest.strip_prefix([')', ':', '-', '–']).unwrap_or(rest).trim_start();
    if rest.is_empty() && !l.ends_with(':') {
        return None;
    }
    // A plain "2024 was…" is not a comment number.
    if !body.trim_start().starts_with(|c: char| c.is_ascii_digit()) || (num.len() >= 4 && !num.contains('.')) {
        return None;
    }
    Some((num, rest.to_string()))
}

/// The comments in a review's text.
pub fn parse_reviews(text: &str) -> Vec<ReviewComment> {
    let mut out: Vec<ReviewComment> = Vec::new();
    let mut reviewer = "Reviewer 1".to_string();
    let mut current: Option<ReviewComment> = None;
    let mut count = 0usize;
    let flush = |c: &mut Option<ReviewComment>, out: &mut Vec<ReviewComment>| {
        if let Some(mut c) = c.take() {
            c.text = c.text.trim().to_string();
            if !c.text.is_empty() && out.len() < 2000 {
                out.push(c);
            }
        }
    };
    for line in text.lines() {
        if let Some(r) = reviewer_heading(line) {
            flush(&mut current, &mut out);
            reviewer = r;
            count = 0;
            continue;
        }
        if let Some((num, rest)) = numbered(line) {
            flush(&mut current, &mut out);
            count += 1;
            current = Some(ReviewComment { reviewer: reviewer.clone(), number: if num.is_empty() { count.to_string() } else { num }, text: rest });
            continue;
        }
        if line.trim().is_empty() {
            // A blank line ends an unnumbered paragraph-style comment.
            if current.as_ref().is_some_and(|c| !c.text.is_empty()) {
                flush(&mut current, &mut out);
            }
            continue;
        }
        match current.as_mut() {
            Some(c) => {
                c.text.push(' ');
                c.text.push_str(line.trim());
            }
            None => {
                count += 1;
                current = Some(ReviewComment { reviewer: reviewer.clone(), number: count.to_string(), text: line.trim().to_string() });
            }
        }
    }
    flush(&mut current, &mut out);
    out
}

/// "Reviewer 2", "3" → "R2.3".
fn label(reviewer: &str, number: &str) -> String {
    let who = if reviewer == "Editor" { "E".to_string() } else { format!("R{}", reviewer.rsplit(' ').next().unwrap_or("")) };
    format!("{who}.{number}")
}

/// Add comments to a response log.
pub fn add_comments(db: &mut Database, comments: &[ReviewComment]) -> usize {
    let mut n = 0;
    for c in comments {
        let name = label(&c.reviewer, &c.number);
        db.add_row_with(&[("Name", &name), ("Reviewer", &c.reviewer), ("Status", "To do"), ("Comment", &c.text)]);
        n += 1;
    }
    n
}

/// The response letter for a log: a heading per reviewer, each comment in italics, the
/// response, and where it changed.
pub fn letter(db: &Database, title: &str) -> Result<Session, String> {
    let mut s = Session::new(wordcraft_doc::Document::new());
    let (rev, com, res, chg) = (db.prop_named("Reviewer"), db.prop_named("Comment"), db.prop_named("Response"), db.prop_named("Changes"));
    let text = |row: u64, p: Option<u64>| p.map(|p| db.text(row, p)).unwrap_or_default();
    let run = |s: &mut Session, id: &str, v: serde_json::Value| s.run(id, &v).map(|_| ()).map_err(|e| e.to_string());
    run(&mut s, "text.insert", json!({"text": "Response to Reviewers"}))?;
    run(&mut s, "para.style", json!({"style": "Title"}))?;
    run(&mut s, "text.newParagraph", json!({}))?;
    run(&mut s, "para.style", json!({"style": "Normal"}))?;
    if !title.trim().is_empty() {
        run(&mut s, "text.insert", json!({"text": format!("Manuscript: “{}”", title.trim())}))?;
        run(&mut s, "text.newParagraph", json!({}))?;
    }
    run(
        &mut s,
        "text.insert",
        json!({"text": "We thank the editor and the reviewers for their careful reading and helpful comments. Each comment is quoted in italics, followed by our response and the changes made to the manuscript."}),
    )?;
    // Reviewers in the order they first appear.
    let mut order: Vec<String> = Vec::new();
    for r in &db.rows {
        let who = text(r.id, rev);
        if !order.contains(&who) {
            order.push(who);
        }
    }
    for who in order {
        run(&mut s, "text.newParagraph", json!({}))?;
        run(&mut s, "text.insert", json!({"text": if who.is_empty() { "Comments" } else { who.as_str() }}))?;
        run(&mut s, "para.heading1", json!({}))?;
        for r in db.rows.iter().filter(|r| text(r.id, rev) == who) {
            run(&mut s, "text.newParagraph", json!({}))?;
            run(&mut s, "para.style", json!({"style": "Normal"}))?;
            run(&mut s, "format.italic", json!({"value": true}))?;
            run(&mut s, "text.insert", json!({"text": format!("{}: {}", db.title(r.id), text(r.id, com))}))?;
            run(&mut s, "format.italic", json!({"value": false}))?;
            run(&mut s, "text.newParagraph", json!({}))?;
            let response = text(r.id, res);
            run(&mut s, "format.bold", json!({"value": true}))?;
            run(&mut s, "text.insert", json!({"text": "Response: "}))?;
            run(&mut s, "format.bold", json!({"value": false}))?;
            run(&mut s, "text.insert", json!({"text": if response.trim().is_empty() { "[to be written]" } else { response.as_str() }}))?;
            let changes = text(r.id, chg);
            if !changes.trim().is_empty() {
                run(&mut s, "text.newParagraph", json!({}))?;
                run(&mut s, "text.insert", json!({"text": format!("Changes: {changes}")}))?;
            }
        }
    }
    s.dirty = true;
    Ok(s)
}

impl GlassApp {
    /// The open response log, or a new one opened in a tab.
    fn response_log(&mut self) -> Option<&mut Database> {
        let is_log = |d: &Database| d.prop_named("Response").is_some() && d.prop_named("Comment").is_some();
        if !self.active_db().is_some_and(is_log) {
            match (0..self.docs.tabs.len()).find(|i| self.docs.tabs.get(*i).and_then(|t| t.db.as_deref()).is_some_and(is_log)) {
                Some(i) => self.switch_to(i),
                None => {
                    let mut db = database();
                    db.rows.clear();
                    self.open_database(db);
                }
            }
        }
        self.active_db_mut()
    }

    /// Read reviewers' comments from a file (text, Markdown, Word…) into the response log.
    pub fn import_reviews(&mut self, path: &Path) {
        let text = match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
            Some("txt" | "md" | "csv" | "tsv") => std::fs::metadata(path)
                .map_err(|e| e.to_string())
                .and_then(|m| if m.len() > 8 << 20 { Err("the file is too large".to_string()) } else { Ok(()) })
                .and_then(|_| std::fs::read(path).map_err(|e| e.to_string()))
                .map(|b| String::from_utf8_lossy(&b).into_owned()),
            _ => wordcraft_engine::io::open_path(path).map(|d| d.plain_text(wordcraft_doc::StoryRef::Body)),
        };
        match text {
            Ok(t) => {
                let comments = parse_reviews(&t);
                match self.response_log() {
                    Some(db) => {
                        let n = add_comments(db, &comments);
                        self.app.status(format!("{n} reviewer comments added to the response log"));
                    }
                    None => self.app.status("Couldn't open the response log"),
                }
            }
            Err(e) => self.app.status(format!("Couldn't read the review: {e}")),
        }
    }

    /// Put the open manuscript's comments (from its reviewers) in the response log.
    pub fn comments_to_log(&mut self) {
        let doc = &self.app.session.doc;
        let mut by: Vec<String> = Vec::new();
        let mut comments = Vec::new();
        let mut count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for c in doc.comments.values().filter(|c| c.parent.is_none()) {
            if !by.contains(&c.author) {
                by.push(c.author.clone());
            }
            let k = by.iter().position(|a| a == &c.author).unwrap_or(0) + 1;
            let n = count.entry(c.author.clone()).or_insert(0);
            *n += 1;
            let text = doc
                .parts
                .get(&c.part)
                .map(|p| p.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect::<Vec<_>>().join(" "))
                .unwrap_or_default();
            comments.push(ReviewComment { reviewer: format!("Reviewer {k}"), number: n.to_string(), text });
        }
        if comments.is_empty() {
            self.app.status("This document has no comments to add");
            return;
        }
        match self.response_log() {
            Some(db) => {
                let n = add_comments(db, &comments);
                self.app.status(format!("{n} comments added to the response log"));
            }
            None => self.app.status("Couldn't open the response log"),
        }
    }

    /// Write the response letter from the open (or first) response log, in a new tab.
    pub fn response_letter(&mut self) {
        let db = (0..self.docs.tabs.len())
            .filter_map(|i| self.docs.tabs.get(i).and_then(|t| t.db.as_ref()))
            .find(|d| d.prop_named("Response").is_some() && d.prop_named("Comment").is_some())
            .map(|d| (**d).clone());
        let Some(db) = db else {
            self.app.status("Open your response log first (or import a review to start one)");
            return;
        };
        let title = self.app.session.doc.core.title.clone();
        match letter(&db, &title) {
            Ok(s) => {
                self.new_tab(s);
                self.app.status("Response letter written from the log: fill in any [to be written] responses");
            }
            Err(e) => self.app.status(format!("Couldn't write the letter: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REVIEW: &str = "Reviewer #1\n\n1. The sample is small.\nPlease justify it.\n2) Figure 3 is unclear.\n\nReviewer 2:\nComment 1: Cite recent work on shade.\n\nThe discussion is long.\n";

    #[test]
    fn reviews_are_split_into_comments() {
        let c = parse_reviews(REVIEW);
        assert_eq!(c.len(), 4, "{c:?}");
        assert_eq!(c[0], ReviewComment { reviewer: "Reviewer 1".into(), number: "1".into(), text: "The sample is small. Please justify it.".into() });
        assert_eq!(c[1].number, "2");
        assert_eq!(c[2].reviewer, "Reviewer 2");
        assert_eq!(c[2].text, "Cite recent work on shade.");
        assert_eq!(c[3].number, "2");
        assert!(numbered("2024 was a hot year").is_none());
        assert_eq!(label("Reviewer 2", "3"), "R2.3");
    }

    #[test]
    fn the_log_becomes_a_letter() {
        let mut db = database();
        db.rows.clear();
        add_comments(&mut db, &parse_reviews(REVIEW));
        let r = db.rows[0].id;
        let p = db.prop_named("Response").unwrap();
        db.set_text(r, p, "We added a power analysis.");
        let s = letter(&db, "Shade").unwrap();
        let text = s.doc.plain_text(wordcraft_doc::StoryRef::Body);
        assert!(text.contains("Reviewer 1") && text.contains("Reviewer 2"));
        assert!(text.contains("R1.1: The sample is small."));
        assert!(text.contains("Response: We added a power analysis."));
        assert!(text.contains("[to be written]"));
    }
}

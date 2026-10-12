//! Writer Pro project data: everything a writer keeps around a manuscript that isn't the
//! manuscript itself. Notes (characters, places, research), the Scratchpad of cut text, and each
//! section's card (status, synopsis, label), as in Scrivener's binder and corkboard.
//!
//! Projects are kept per document in Glass's settings (`glass-projects.json`), keyed by the
//! document's file path, so nothing extra is written next to the writer's files.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use wordcraft_engine::Session;
use wordcraft_engine::doc::{Path, StoryRef, count_words};

/// Limits, so a hand-edited or runaway file can't grow without bound.
const MAX_NOTES: usize = 2000;
const MAX_SCRAPS: usize = 2000;
const MAX_TEXT: usize = 200_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteKind {
    #[default]
    Research,
    Character,
    Place,
}

impl NoteKind {
    pub const ALL: [NoteKind; 3] = [NoteKind::Character, NoteKind::Place, NoteKind::Research];

    pub fn name(self) -> &'static str {
        match self {
            NoteKind::Research => "Research",
            NoteKind::Character => "Character",
            NoteKind::Place => "Place",
        }
    }

    pub fn plural(self) -> &'static str {
        match self {
            NoteKind::Research => "Research",
            NoteKind::Character => "Characters",
            NoteKind::Place => "Places",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Note {
    pub id: u64,
    pub kind: NoteKind,
    pub title: String,
    pub body: String,
}

/// Text cut to the Scratchpad: kept, never lost.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scrap {
    pub text: String,
    /// The section it came from.
    pub from: String,
    /// When it was cut (ISO, UTC).
    pub date: String,
}

/// Where a section stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    #[default]
    ToDo,
    Draft,
    Revised,
    Done,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::ToDo => "To Do",
            Status::Draft => "First Draft",
            Status::Revised => "Revised",
            Status::Done => "Done",
        }
    }

    pub fn next(self) -> Status {
        match self {
            Status::ToDo => Status::Draft,
            Status::Draft => Status::Revised,
            Status::Revised => Status::Done,
            Status::Done => Status::ToDo,
        }
    }

    pub const ALL: [Status; 4] = [Status::ToDo, Status::Draft, Status::Revised, Status::Done];
}

/// A section's card: its status and synopsis, keyed by its heading's text.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Card {
    pub status: Status,
    pub synopsis: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub notes: Vec<Note>,
    pub scraps: Vec<Scrap>,
    pub cards: BTreeMap<String, Card>,
    next_id: u64,
}

impl Project {
    pub fn add_note(&mut self, kind: NoteKind, title: &str) -> u64 {
        self.next_id = self.next_id.saturating_add(1).max(self.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1);
        let id = self.next_id;
        if self.notes.len() < MAX_NOTES {
            self.notes.push(Note { id, kind, title: title.to_string(), body: String::new() });
        }
        id
    }

    pub fn note_mut(&mut self, id: u64) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.id == id)
    }

    pub fn remove_note(&mut self, id: u64) {
        self.notes.retain(|n| n.id != id);
    }

    pub fn add_scrap(&mut self, text: &str, from: &str, date: &str) {
        if text.trim().is_empty() {
            return;
        }
        let text: String = text.chars().take(MAX_TEXT).collect();
        self.scraps.insert(0, Scrap { text, from: from.to_string(), date: date.to_string() });
        self.scraps.truncate(MAX_SCRAPS);
    }

    pub fn card(&self, title: &str) -> Card {
        self.cards.get(title).cloned().unwrap_or_default()
    }

    pub fn card_mut(&mut self, title: &str) -> &mut Card {
        self.cards.entry(title.to_string()).or_default()
    }
}

/// All projects, keyed by document.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Projects {
    pub by_doc: BTreeMap<String, Project>,
}

/// The key a document's project is filed under: its path, or its session id while unsaved.
pub fn key(s: &Session) -> String {
    match &s.path {
        Some(p) => p.to_string_lossy().to_string(),
        None => format!("untitled:{}", s.document_id()),
    }
}

/// One section of the manuscript: a heading and the text under it, up to the next heading of the
/// same or a higher level.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub path: Path,
    pub level: u8,
    pub title: String,
    pub words: usize,
    /// The section's first sentence, as a default synopsis.
    pub opening: String,
    /// The section's body text (for analysis).
    pub text: String,
}

/// The manuscript's sections, in order (headings with an outline level, Table of Contents
/// entries left out).
pub fn sections(s: &Session) -> Vec<Section> {
    let doc = &s.doc;
    let mut out: Vec<Section> = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        let rp = doc.styles.resolve_para(&p.props);
        let text = p.plain_text().to_string();
        match rp.outline_level {
            Some(level) if !rp.style.starts_with("TOC") && !text.trim().is_empty() => {
                out.push(Section { path, level, title: text.trim().to_string(), words: 0, opening: String::new(), text: String::new() });
            }
            _ => {
                if let Some(sec) = out.last_mut() {
                    sec.words += count_words(&text);
                    if sec.opening.is_empty() && !text.trim().is_empty() {
                        sec.opening = first_sentence(&text);
                    }
                    if sec.text.len() < MAX_TEXT {
                        sec.text.push_str(&text);
                        sec.text.push('\n');
                    }
                }
            }
        }
    }
    out
}

/// The first sentence of `text`, at most 160 characters.
pub fn first_sentence(text: &str) -> String {
    let t = text.trim();
    let end = t.char_indices().find(|(_, c)| matches!(c, '.' | '!' | '?')).map(|(i, c)| i + c.len_utf8()).unwrap_or(t.len());
    let s = t.get(..end).unwrap_or(t);
    if s.chars().count() > 160 { format!("{}…", s.chars().take(160).collect::<String>()) } else { s.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_follow_headings() {
        let s = Session::new(wordcraft_engine::sample::sample_document());
        let secs = sections(&s);
        assert!(secs.len() >= 3);
        assert!(secs.iter().any(|x| x.words > 0));
        assert!(secs.iter().all(|x| !x.title.is_empty()));
    }

    #[test]
    fn notes_get_unique_ids_and_scraps_are_kept_newest_first() {
        let mut p = Project::default();
        let a = p.add_note(NoteKind::Character, "Inês");
        let b = p.add_note(NoteKind::Place, "Porto");
        assert_ne!(a, b);
        p.add_scrap("first", "Ch 1", "d");
        p.add_scrap("second", "Ch 1", "d");
        p.add_scrap("   ", "Ch 1", "d");
        assert_eq!(p.scraps.len(), 2);
        assert_eq!(p.scraps.first().map(|s| s.text.as_str()), Some("second"));
    }

    #[test]
    fn statuses_cycle() {
        let mut s = Status::ToDo;
        for _ in 0..4 {
            s = s.next();
        }
        assert_eq!(s, Status::ToDo);
    }

    #[test]
    fn first_sentence_is_safe_on_any_text() {
        assert_eq!(first_sentence("Olá. Tudo bem?"), "Olá.");
        assert_eq!(first_sentence("no stop"), "no stop");
        assert_eq!(first_sentence(""), "");
    }
}

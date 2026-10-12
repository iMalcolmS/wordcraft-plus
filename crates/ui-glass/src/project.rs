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
    /// Its colour label ([`Tag`] id).
    pub tag: Option<u64>,
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

/// A colour label for a section, as in Scrivener: it tints the section's heading on the page,
/// its card and its row in the Binder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Label {
    #[default]
    None,
    Rose,
    Amber,
    Lime,
    Sky,
    Violet,
}

impl Label {
    pub const ALL: [Label; 6] = [Label::None, Label::Rose, Label::Amber, Label::Lime, Label::Sky, Label::Violet];

    pub fn name(self) -> &'static str {
        match self {
            Label::None => "No Label",
            Label::Rose => "Rose",
            Label::Amber => "Amber",
            Label::Lime => "Lime",
            Label::Sky => "Sky",
            Label::Violet => "Violet",
        }
    }

    /// The label's colour (`None` has none).
    pub fn rgb(self) -> Option<(u8, u8, u8)> {
        match self {
            Label::None => None,
            Label::Rose => Some((0xE8, 0x5D, 0x75)),
            Label::Amber => Some((0xE0, 0x9B, 0x2D)),
            Label::Lime => Some((0x5C, 0xB8, 0x5C)),
            Label::Sky => Some((0x3D, 0x9B, 0xE8)),
            Label::Violet => Some((0x9B, 0x6B, 0xE0)),
        }
    }
}

/// A section's card: its status, label, synopsis and notes, keyed by its heading's text.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Card {
    pub status: Status,
    pub synopsis: String,
    pub label: Label,
    /// The section's own notes (the Inspector's Notes).
    pub notes: String,
    /// Its colour label ([`Tag`] id); takes the place of the older fixed `label`.
    pub tag: Option<u64>,
}

/// A colour label the writer defines, as in Scrivener: a name, a colour and an icon (a flag, a
/// star, a person…), for characters, points of view, timelines, topics or anything else. It
/// marks sections in the Binder, on the Corkboard and on the page, and notes in the Binder.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tag {
    pub id: u64,
    pub name: String,
    pub tint: crate::db::Tint,
    /// A glyph name (`crate::glyphs::LABELS`).
    pub icon: String,
}

/// The labels a new project starts with (the writer renames, recolours or deletes them).
pub const DEFAULT_TAGS: [(&str, crate::db::Tint, &str); 7] = [
    ("Main plot", crate::db::Tint::Blue, "flag"),
    ("Subplot", crate::db::Tint::Purple, "bookmark"),
    ("Flashback", crate::db::Tint::Orange, "clock"),
    ("POV: protagonist", crate::db::Tint::Red, "person"),
    ("POV: other", crate::db::Tint::Teal, "globe"),
    ("Important", crate::db::Tint::Yellow, "star"),
    ("Needs research", crate::db::Tint::Gray, "bulb"),
];

/// Most labels a project keeps.
const MAX_TAGS: usize = 200;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub notes: Vec<Note>,
    pub scraps: Vec<Scrap>,
    pub cards: BTreeMap<String, Card>,
    /// The colour labels (seeded with [`DEFAULT_TAGS`] the first time they're asked for).
    pub tags: Vec<Tag>,
    tags_seeded: bool,
    next_id: u64,
}

impl Project {
    pub fn add_note(&mut self, kind: NoteKind, title: &str) -> u64 {
        let id = self.fresh_id();
        if self.notes.len() < MAX_NOTES {
            self.notes.push(Note { id, kind, title: title.to_string(), body: String::new(), tag: None });
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

    fn fresh_id(&mut self) -> u64 {
        let max = self.notes.iter().map(|n| n.id).chain(self.tags.iter().map(|t| t.id)).max().unwrap_or(0);
        self.next_id = self.next_id.saturating_add(1).max(max.saturating_add(1));
        self.next_id
    }

    /// The colour labels, with the defaults added the first time.
    pub fn tags(&mut self) -> &[Tag] {
        if !self.tags_seeded {
            self.tags_seeded = true;
            if self.tags.is_empty() {
                for (name, tint, icon) in DEFAULT_TAGS {
                    self.add_tag(name, tint, icon);
                }
            }
        }
        &self.tags
    }

    pub fn tag(&self, id: u64) -> Option<&Tag> {
        self.tags.iter().find(|t| t.id == id)
    }

    pub fn tag_mut(&mut self, id: u64) -> Option<&mut Tag> {
        self.tags.iter_mut().find(|t| t.id == id)
    }

    /// Add a label; returns its id (0 when there are already too many).
    pub fn add_tag(&mut self, name: &str, tint: crate::db::Tint, icon: &str) -> u64 {
        if self.tags.len() >= MAX_TAGS {
            return 0;
        }
        let id = self.fresh_id();
        let name: String = name.chars().take(80).collect();
        self.tags.push(Tag { id, name, tint, icon: icon.to_string() });
        id
    }

    /// Delete a label, and take it off the sections and notes that had it.
    pub fn remove_tag(&mut self, id: u64) {
        self.tags_seeded = true;
        self.tags.retain(|t| t.id != id);
        for c in self.cards.values_mut() {
            if c.tag == Some(id) {
                c.tag = None;
            }
        }
        for n in &mut self.notes {
            if n.tag == Some(id) {
                n.tag = None;
            }
        }
    }

    /// A section's label as (name, colour, icon): its own label, or the older fixed one.
    pub fn card_tag(&self, c: &Card) -> Option<Tag> {
        if let Some(t) = c.tag.and_then(|id| self.tag(id)) {
            return Some(t.clone());
        }
        let tint = match c.label {
            Label::None => return None,
            Label::Rose => crate::db::Tint::Red,
            Label::Amber => crate::db::Tint::Orange,
            Label::Lime => crate::db::Tint::Green,
            Label::Sky => crate::db::Tint::Blue,
            Label::Violet => crate::db::Tint::Purple,
        };
        Some(Tag { id: 0, name: c.label.name().into(), tint, icon: "circle".into() })
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
    fn labels_start_with_defaults_and_deleting_one_unlabels() {
        let mut p = Project::default();
        assert_eq!(p.tags().len(), DEFAULT_TAGS.len());
        let id = p.tags()[0].id;
        p.card_mut("Chapter 1").tag = Some(id);
        let n = p.add_note(NoteKind::Character, "Mara");
        assert_ne!(n, id, "notes and labels never share an id");
        if let Some(note) = p.note_mut(n) {
            note.tag = Some(id);
        }
        assert_eq!(p.card_tag(&p.card("Chapter 1")).map(|t| t.name), Some("Main plot".into()));
        p.remove_tag(id);
        assert_eq!(p.card("Chapter 1").tag, None);
        assert_eq!(p.notes[0].tag, None);
        assert_eq!(p.tags().len(), DEFAULT_TAGS.len() - 1, "the defaults don't come back");
        // The older fixed labels still show.
        let old = Card { label: Label::Sky, ..Default::default() };
        assert!(p.card_tag(&old).is_some());
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

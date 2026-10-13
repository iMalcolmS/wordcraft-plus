//! The reference library (Research mode's citation hub): every source you cite, kept once for
//! all your documents in the workspace (`Documents/WordCraft/Library/library.json`).
//!
//! References come in from the formats reference managers export (BibTeX `.bib`, RIS `.ris`
//! from EndNote, Mendeley, Zotero and most databases, CSL-JSON, EndNote XML), from a linked file
//! a manager keeps up to date (Zotero with Better BibTeX, JabRef, Mendeley's BibTeX sync): it is
//! read again whenever it changes, and from the Zotero app itself through its local API.
//! Inserting a citation hands the reference to WordCraft's own citations (`CITATION` fields and
//! the document's sources), so the bibliography, the styles and Word all work as usual.
//!
//! Every parser here reads hostile input: sizes are capped and malformed entries are skipped.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Most references kept.
pub const MAX_REFS: usize = 50_000;
/// Largest file read.
pub const MAX_FILE: u64 = 64 << 20;
/// Longest field kept.
const MAX_FIELD: usize = 32 * 1024;

/// A reference.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ref {
    /// The citation key (`silva2024`), unique in the library.
    pub key: String,
    /// BibTeX's entry type: article, book, inproceedings, incollection, thesis, report, misc…
    pub kind: String,
    /// Authors as "Last, First".
    pub authors: Vec<String>,
    pub title: String,
    pub year: String,
    /// Journal, book or proceedings title.
    pub container: String,
    pub volume: String,
    pub issue: String,
    pub pages: String,
    pub publisher: String,
    pub doi: String,
    pub url: String,
    #[serde(rename = "abstract")]
    pub abstract_: String,
    pub keywords: Vec<String>,
    /// Where it came from (`bibtex`, `ris`, `zotero`, `manual`…).
    pub origin: String,
}

fn cap(s: &str) -> String {
    let t = s.trim();
    if t.len() <= MAX_FIELD {
        return t.to_string();
    }
    let mut end = MAX_FIELD;
    while end > 0 && !t.is_char_boundary(end) {
        end -= 1;
    }
    t.get(..end).unwrap_or("").to_string()
}

impl Ref {
    /// "Silva" / "Silva & Lee" / "Silva et al."
    pub fn short_authors(&self) -> String {
        let last = |a: &String| a.split(',').next().unwrap_or(a).trim().to_string();
        match self.authors.as_slice() {
            [] => String::new(),
            [a] => last(a),
            [a, b] => format!("{} & {}", last(a), last(b)),
            [a, ..] => format!("{} et al.", last(a)),
        }
    }

    /// "(Silva, 2024)"
    pub fn label(&self) -> String {
        let who = self.short_authors();
        match (who.is_empty(), self.year.is_empty()) {
            (false, false) => format!("({who}, {})", self.year),
            (false, true) => format!("({who})"),
            (true, false) => format!("({})", self.year),
            (true, true) => format!("({})", self.key),
        }
    }

    /// WordCraft's source kind for the BibTeX type.
    fn source_kind(&self) -> &'static str {
        match self.kind.to_ascii_lowercase().as_str() {
            "book" | "booklet" => "book",
            "incollection" | "inbook" => "bookSection",
            "inproceedings" | "conference" | "proceedings" => "conference",
            "techreport" | "report" | "phdthesis" | "mastersthesis" | "thesis" => "report",
            "online" | "webpage" | "electronic" => "website",
            "misc" if !self.url.is_empty() && self.container.is_empty() => "website",
            "article" | "" => "article",
            _ => "other",
        }
    }

    /// The reference as a WordCraft source (JSON, for `references.citation`).
    pub fn to_source(&self) -> Value {
        json!({
            "tag": self.key,
            "kind": self.source_kind(),
            "author": self.authors.join("; "),
            "title": self.title,
            "year": self.year,
            "publisher": self.publisher,
            "journal": self.container,
            "volume": self.volume,
            "issue": self.issue,
            "pages": self.pages,
            "url": self.url,
            "doi": self.doi,
        })
    }

    /// The reference from a document's source (for references already in a document).
    pub fn from_source(s: &wordcraft_doc::Source) -> Ref {
        Ref {
            key: s.tag.clone(),
            kind: match s.kind.as_str() {
                "book" => "book",
                "bookSection" => "incollection",
                "conference" => "inproceedings",
                "report" => "techreport",
                "website" => "online",
                _ => "article",
            }
            .into(),
            authors: s.author.split(';').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect(),
            title: s.title.clone(),
            year: s.year.clone(),
            container: s.journal.clone(),
            volume: s.volume.clone(),
            issue: s.issue.clone(),
            pages: s.pages.clone(),
            publisher: s.publisher.clone(),
            doi: s.doi.clone(),
            url: s.url.clone(),
            origin: "document".into(),
            ..Default::default()
        }
    }

    /// The reference as a BibTeX entry.
    pub fn to_bibtex(&self) -> String {
        let mut o = format!("@{}{{{},\n", if self.kind.is_empty() { "article" } else { &self.kind }, bib_key(&self.key));
        let journal_field = if self.kind == "inproceedings" || self.kind == "incollection" { "booktitle" } else { "journal" };
        let fields: [(&str, String); 12] = [
            ("author", self.authors.join(" and ")),
            ("title", self.title.clone()),
            (journal_field, self.container.clone()),
            ("year", self.year.clone()),
            ("volume", self.volume.clone()),
            ("number", self.issue.clone()),
            ("pages", self.pages.replace('–', "--")),
            ("publisher", self.publisher.clone()),
            ("doi", self.doi.clone()),
            ("url", self.url.clone()),
            ("abstract", self.abstract_.clone()),
            ("keywords", self.keywords.join(", ")),
        ];
        for (k, v) in fields.iter().filter(|(_, v)| !v.trim().is_empty()) {
            o.push_str(&format!("  {k} = {{{}}},\n", bib_escape(v)));
        }
        o.push_str("}\n");
        o
    }

    /// Text searched by the library's search field.
    pub fn haystack(&self) -> String {
        format!("{} {} {} {} {} {} {}", self.key, self.authors.join(" "), self.title, self.year, self.container, self.doi, self.keywords.join(" "))
            .to_lowercase()
    }
}

/// A BibTeX key: letters, digits and `:-_.` only.
fn bib_key(k: &str) -> String {
    let s: String = k.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_' | '.')).collect();
    if s.is_empty() { "ref".into() } else { s }
}

fn bib_escape(s: &str) -> String {
    // Braces must balance inside a braced value: drop any that don't.
    let mut depth = 0i32;
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '{' => {
                depth += 1;
                out.push(c);
            }
            '}' if depth > 0 => {
                depth -= 1;
                out.push(c);
            }
            '}' => {}
            c => out.push(c),
        }
    }
    for _ in 0..depth {
        out.push('}');
    }
    out
}

/// A key made from the first author's surname, the year and the title's first word
/// (`silva2024shade`).
pub fn make_key(r: &Ref) -> String {
    let last: String = r
        .authors
        .first()
        .map(|a| a.split(',').next().unwrap_or(a))
        .unwrap_or("ref")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    let word: String = r
        .title
        .split_whitespace()
        .find(|w| w.chars().count() > 3)
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    let year: String = r.year.chars().filter(char::is_ascii_digit).take(4).collect();
    let k = format!("{}{year}{word}", if last.is_empty() { "ref".into() } else { last });
    k.chars().take(60).collect()
}

/// "Ada Lovelace" → "Lovelace, Ada"; "Lovelace, Ada" stays.
pub fn normalize_name(n: &str) -> String {
    let n = n.trim().trim_matches(['{', '}']);
    if n.contains(',') || n.is_empty() {
        return n.to_string();
    }
    let parts: Vec<&str> = n.split_whitespace().collect();
    match parts.split_last() {
        Some((last, first)) if !first.is_empty() => format!("{last}, {}", first.join(" ")),
        _ => n.to_string(),
    }
}

// ---------------------------------------------------------------------------------------------
// BibTeX.

/// BibTeX's accents and escapes → text (`{\'e}` → é, `\&` → &, `--` → –).
fn latex_text(s: &str) -> String {
    const ACCENTS: [(&str, &str); 24] = [
        ("\\'a", "á"),
        ("\\'e", "é"),
        ("\\'i", "í"),
        ("\\'o", "ó"),
        ("\\'u", "ú"),
        ("\\`a", "à"),
        ("\\`e", "è"),
        ("\\^a", "â"),
        ("\\^e", "ê"),
        ("\\^o", "ô"),
        ("\\~a", "ã"),
        ("\\~o", "õ"),
        ("\\~n", "ñ"),
        ("\\\"a", "ä"),
        ("\\\"o", "ö"),
        ("\\\"u", "ü"),
        ("\\c{c}", "ç"),
        ("\\c c", "ç"),
        ("\\ss", "ß"),
        ("\\&", "&"),
        ("\\%", "%"),
        ("\\_", "_"),
        ("\\$", "$"),
        ("\\#", "#"),
    ];
    let mut t = s.to_string();
    for (a, b) in ACCENTS {
        t = t.replace(&format!("{{{a}}}"), b).replace(a, b);
    }
    let t = t.replace("---", "—").replace("--", "–");
    let t: String = t.chars().filter(|c| *c != '{' && *c != '}').collect();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse BibTeX (`@article{key, field = {value}, …}`), skipping `@string`, `@comment` and
/// `@preamble` and anything malformed.
pub fn parse_bibtex(src: &str) -> Vec<Ref> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(at) = b.get(i..).and_then(|r| r.iter().position(|c| *c == b'@')).map(|p| p + i) {
        i = at + 1;
        let ty_end = b.get(i..).and_then(|r| r.iter().position(|c| *c == b'{' || *c == b'(')).map(|p| p + i);
        let Some(open) = ty_end else { break };
        let kind = String::from_utf8_lossy(b.get(i..open).unwrap_or_default()).trim().to_ascii_lowercase();
        if kind.is_empty() || kind.len() > 30 || !kind.chars().all(|c| c.is_ascii_alphabetic()) {
            continue;
        }
        // The entry body: up to the matching close.
        let mut depth = 0i32;
        let mut end = None;
        for (k, c) in b.iter().enumerate().skip(open) {
            match c {
                b'{' | b'(' => depth += 1,
                b'}' | b')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(k);
                        break;
                    }
                }
                _ => {}
            }
        }
        // An entry that never closes: skip its `@` and look for the next entry.
        let Some(end) = end else {
            i = open + 1;
            continue;
        };
        i = end + 1;
        if matches!(kind.as_str(), "string" | "comment" | "preamble") {
            continue;
        }
        let body = String::from_utf8_lossy(b.get(open + 1..end).unwrap_or_default()).to_string();
        let Some((key, rest)) = body.split_once(',') else { continue };
        let mut r = Ref { key: cap(key.trim()), kind: kind.clone(), origin: "bibtex".into(), ..Default::default() };
        for (name, value) in bib_fields(rest) {
            let v = cap(&latex_text(&value));
            match name.as_str() {
                "author" => r.authors = v.split(" and ").map(normalize_name).filter(|a| !a.is_empty()).take(200).collect(),
                "title" => r.title = v,
                "year" => r.year = v,
                "date" if r.year.is_empty() => r.year = v.chars().take(4).collect(),
                "journal" | "journaltitle" | "booktitle" => r.container = v,
                "volume" => r.volume = v,
                "number" | "issue" => r.issue = v,
                "pages" => r.pages = v,
                "publisher" | "school" | "institution" => r.publisher = v,
                "doi" => r.doi = v,
                "url" => r.url = v,
                "abstract" => r.abstract_ = v,
                "keywords" => r.keywords = v.split([',', ';']).map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).take(50).collect(),
                _ => {}
            }
        }
        if !r.key.is_empty() {
            out.push(r);
        }
        if out.len() >= MAX_REFS {
            break;
        }
    }
    out
}

/// `name = {value}` / `"value"` / `number` pairs of a BibTeX entry.
fn bib_fields(s: &str) -> Vec<(String, String)> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        while c.get(i).is_some_and(|x| x.is_whitespace() || *x == ',') {
            i += 1;
        }
        let start = i;
        while c.get(i).is_some_and(|x| *x != '=' && *x != ',') {
            i += 1;
        }
        if c.get(i) != Some(&'=') {
            break;
        }
        let name: String = c.get(start..i).map(|x| x.iter().collect::<String>()).unwrap_or_default().trim().to_ascii_lowercase();
        i += 1;
        while c.get(i).is_some_and(|x| x.is_whitespace()) {
            i += 1;
        }
        let mut value = String::new();
        // Values can be joined with `#`.
        loop {
            match c.get(i) {
                Some('{') => {
                    let mut depth = 0;
                    let s0 = i;
                    while let Some(&x) = c.get(i) {
                        if x == '{' {
                            depth += 1;
                        } else if x == '}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        i += 1;
                    }
                    value.extend(c.get(s0 + 1..i).unwrap_or_default());
                    i += 1;
                }
                Some('"') => {
                    i += 1;
                    let s0 = i;
                    while c.get(i).is_some_and(|x| *x != '"') {
                        i += 1;
                    }
                    value.extend(c.get(s0..i).unwrap_or_default());
                    i += 1;
                }
                Some(_) => {
                    let s0 = i;
                    while c.get(i).is_some_and(|x| *x != ',' && *x != '#' && !x.is_whitespace()) {
                        i += 1;
                    }
                    value.extend(c.get(s0..i).unwrap_or_default());
                }
                None => break,
            }
            while c.get(i).is_some_and(|x| x.is_whitespace()) {
                i += 1;
            }
            if c.get(i) == Some(&'#') {
                i += 1;
                while c.get(i).is_some_and(|x| x.is_whitespace()) {
                    i += 1;
                }
                continue;
            }
            break;
        }
        if !name.is_empty() {
            out.push((name, value));
        }
        if out.len() > 200 {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// RIS.

/// Parse RIS (`TY  - JOUR` … `ER  -`), as EndNote, Mendeley, Zotero and most databases export.
pub fn parse_ris(src: &str) -> Vec<Ref> {
    let mut out = Vec::new();
    let mut cur: Option<Ref> = None;
    for line in src.lines() {
        let line = line.trim_end_matches('\r');
        let (tag, value) = match (line.get(..2), line.get(2..6)) {
            (Some(t), Some(sep)) if sep.trim_start().starts_with('-') => (t, line.get(6..).unwrap_or("").trim()),
            (Some(t), _) if line.trim_end() == format!("{t}  -").trim_end() => (t, ""),
            _ => continue,
        };
        let v = cap(value);
        match tag {
            "TY" => {
                let kind = match v.as_str() {
                    "JOUR" | "JFULL" | "EJOUR" | "MGZN" | "NEWS" => "article",
                    "BOOK" | "EBOOK" => "book",
                    "CHAP" | "ECHAP" => "incollection",
                    "CONF" | "CPAPER" => "inproceedings",
                    "THES" => "phdthesis",
                    "RPRT" => "techreport",
                    "ELEC" | "WEB" => "online",
                    _ => "misc",
                };
                cur = Some(Ref { kind: kind.into(), origin: "ris".into(), ..Default::default() });
            }
            "ER" => {
                if let Some(mut r) = cur.take() {
                    if r.key.is_empty() {
                        r.key = make_key(&r);
                    }
                    out.push(r);
                    if out.len() >= MAX_REFS {
                        break;
                    }
                }
            }
            _ => {
                let Some(r) = cur.as_mut() else { continue };
                match tag {
                    "AU" | "A1" | "A2" if r.authors.len() < 200 => r.authors.push(normalize_name(&v)),
                    "TI" | "T1" => r.title = v,
                    "PY" | "Y1" | "DA" if r.year.is_empty() => r.year = v.chars().filter(char::is_ascii_digit).take(4).collect(),
                    "JO" | "JF" | "T2" | "JA" | "BT" if r.container.is_empty() => r.container = v,
                    "VL" => r.volume = v,
                    "IS" => r.issue = v,
                    "SP" => r.pages = if r.pages.is_empty() { v } else { format!("{v}{}", r.pages) },
                    "EP" => r.pages = if r.pages.is_empty() { format!("–{v}") } else { format!("{}–{v}", r.pages) },
                    "PB" => r.publisher = v,
                    "DO" => r.doi = v,
                    "UR" | "L2" if r.url.is_empty() => r.url = v,
                    "AB" | "N2" => r.abstract_ = v,
                    "KW" if r.keywords.len() < 50 => r.keywords.push(v),
                    "ID" => r.key = v,
                    _ => {}
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// CSL-JSON (Zotero, Mendeley, citation.js…).

fn csl_names(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .take(200)
                .filter_map(|n| {
                    let family = n.get("family").and_then(Value::as_str).unwrap_or("");
                    let given = n.get("given").and_then(Value::as_str).unwrap_or("");
                    let literal = n.get("literal").and_then(Value::as_str).unwrap_or("");
                    match (family.is_empty(), literal.is_empty()) {
                        (false, _) if given.is_empty() => Some(family.to_string()),
                        (false, _) => Some(format!("{family}, {given}")),
                        (true, false) => Some(literal.to_string()),
                        _ => None,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One CSL-JSON item.
pub fn from_csl(it: &Value) -> Option<Ref> {
    let s = |k: &str| {
        it.get(k)
            .map(|v| match v {
                Value::String(s) => cap(s),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            })
            .unwrap_or_default()
    };
    let kind = match s("type").as_str() {
        "article-journal" | "article-magazine" | "article-newspaper" | "article" => "article",
        "book" => "book",
        "chapter" => "incollection",
        "paper-conference" => "inproceedings",
        "thesis" => "phdthesis",
        "report" => "techreport",
        "webpage" | "post-weblog" => "online",
        _ => "misc",
    };
    let year = it
        .get("issued")
        .and_then(|d| d.get("date-parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get(0))
        .map(|y| match y {
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            _ => String::new(),
        })
        .or_else(|| it.get("issued").and_then(|d| d.get("raw")).and_then(Value::as_str).map(|r| r.chars().take(4).collect()))
        .unwrap_or_default();
    let mut r = Ref {
        key: s("citation-key").or_else_empty(s("id")),
        kind: kind.into(),
        authors: csl_names(it.get("author").unwrap_or(&Value::Null)),
        title: s("title"),
        year,
        container: s("container-title"),
        volume: s("volume"),
        issue: s("issue"),
        pages: s("page"),
        publisher: s("publisher"),
        doi: s("DOI"),
        url: s("URL"),
        abstract_: s("abstract"),
        keywords: s("keyword").split([',', ';']).map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect(),
        origin: "csl".into(),
    };
    if r.title.is_empty() && r.authors.is_empty() {
        return None;
    }
    // Zotero's item ids (`http://zotero.org/…`, `12345`) make poor keys.
    if r.key.is_empty() || r.key.contains('/') || r.key.chars().all(|c| c.is_ascii_digit()) {
        r.key = make_key(&r);
    }
    Some(r)
}

trait OrElseEmpty {
    fn or_else_empty(self, other: String) -> String;
}

impl OrElseEmpty for String {
    fn or_else_empty(self, other: String) -> String {
        if self.is_empty() { other } else { self }
    }
}

/// Parse CSL-JSON: an array of items, or `{"items": [...]}` as Zotero's API sends.
pub fn parse_csl_json(src: &str) -> Vec<Ref> {
    let Ok(v) = serde_json::from_str::<Value>(src) else { return Vec::new() };
    let items = v.as_array().cloned().or_else(|| v.get("items").and_then(Value::as_array).cloned()).unwrap_or_default();
    items.iter().take(MAX_REFS).filter_map(from_csl).collect()
}

// ---------------------------------------------------------------------------------------------
// EndNote XML.

fn xml_text(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    cap(&out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'"))
}

/// Everything between `<tag…>` and `</tag>`, each occurrence.
fn xml_all<'a>(s: &'a str, tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut rest = s;
    while let Some(a) = rest.find(&open) {
        let Some(after) = rest.get(a..) else { break };
        let Some(gt) = after.find('>') else { break };
        let Some(body_start) = after.get(gt + 1..) else { break };
        let Some(end) = body_start.find(&close) else { break };
        out.push(body_start.get(..end).unwrap_or(""));
        rest = body_start.get(end + close.len()..).unwrap_or("");
        if out.len() > MAX_REFS {
            break;
        }
    }
    out
}

/// Parse EndNote's XML export (`<records><record>…`).
pub fn parse_endnote_xml(src: &str) -> Vec<Ref> {
    xml_all(src, "record")
        .into_iter()
        .filter_map(|rec| {
            let one = |tag: &str| xml_all(rec, tag).first().map(|x| xml_text(x)).unwrap_or_default();
            let kind = match xml_all(rec, "ref-type").first().map(|x| xml_text(x)).unwrap_or_default().to_lowercase().as_str() {
                k if k.contains("book section") => "incollection",
                k if k.contains("book") => "book",
                k if k.contains("conference") => "inproceedings",
                k if k.contains("thesis") => "phdthesis",
                k if k.contains("report") => "techreport",
                k if k.contains("web") => "online",
                _ => "article",
            };
            let authors_block = xml_all(rec, "authors").first().copied().unwrap_or("");
            let mut r = Ref {
                kind: kind.into(),
                authors: xml_all(authors_block, "author")
                    .into_iter()
                    .map(|a| normalize_name(&xml_text(a)))
                    .filter(|a| !a.is_empty())
                    .take(200)
                    .collect(),
                title: one("title"),
                year: one("year"),
                container: one("secondary-title"),
                volume: one("volume"),
                issue: one("number"),
                pages: one("pages"),
                publisher: one("publisher"),
                doi: one("electronic-resource-num"),
                url: one("url"),
                abstract_: one("abstract"),
                keywords: xml_all(rec, "keyword").into_iter().map(xml_text).take(50).collect(),
                origin: "endnote".into(),
                ..Default::default()
            };
            if r.title.is_empty() {
                return None;
            }
            r.key = make_key(&r);
            Some(r)
        })
        .collect()
}

/// References from a file's bytes, by its extension (or its content).
pub fn parse_file(name: &str, bytes: &[u8]) -> Result<Vec<Ref>, String> {
    if bytes.len() as u64 > MAX_FILE {
        return Err("the file is too large".into());
    }
    let text = String::from_utf8_lossy(bytes);
    let ext = Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let refs = match ext.as_str() {
        "bib" | "bibtex" => parse_bibtex(&text),
        "ris" | "txt" if text.contains("TY  -") => parse_ris(&text),
        "json" => parse_csl_json(&text),
        "xml" => parse_endnote_xml(&text),
        _ if text.contains("TY  -") => parse_ris(&text),
        _ if text.trim_start().starts_with('[') || text.trim_start().starts_with('{') => parse_csl_json(&text),
        _ if text.contains("<record") => parse_endnote_xml(&text),
        _ => parse_bibtex(&text),
    };
    if refs.is_empty() {
        return Err("no references found (BibTeX, RIS, CSL-JSON or EndNote XML)".into());
    }
    Ok(refs)
}

// ---------------------------------------------------------------------------------------------
// The library.

/// A linked file, read again when it changes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Linked {
    pub path: PathBuf,
    /// Its modification time when last read (seconds since 1970).
    pub mtime: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub refs: Vec<Ref>,
    pub linked: Vec<Linked>,
    #[serde(skip)]
    pub dirty: bool,
}

fn mtime(p: &Path) -> u64 {
    std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

impl Library {
    /// Add references, merging duplicates (same DOI, or same key): an existing one is updated
    /// with fields it lacked. Keys are made unique. Returns (added, updated).
    pub fn merge(&mut self, refs: Vec<Ref>) -> (usize, usize) {
        let (mut added, mut updated) = (0, 0);
        for mut r in refs {
            let doi = r.doi.trim().to_lowercase();
            let same = self
                .refs
                .iter()
                .position(|x| (!doi.is_empty() && x.doi.trim().to_lowercase() == doi) || (!r.key.is_empty() && x.key == r.key && x.title == r.title));
            match same.and_then(|i| self.refs.get_mut(i)) {
                Some(x) => {
                    let fill = |a: &mut String, b: &String| {
                        if a.trim().is_empty() && !b.trim().is_empty() {
                            *a = b.clone();
                        }
                    };
                    fill(&mut x.abstract_, &r.abstract_);
                    fill(&mut x.doi, &r.doi);
                    fill(&mut x.url, &r.url);
                    fill(&mut x.container, &r.container);
                    fill(&mut x.pages, &r.pages);
                    if x.keywords.is_empty() {
                        x.keywords = r.keywords.clone();
                    }
                    updated += 1;
                }
                None => {
                    if self.refs.len() >= MAX_REFS {
                        break;
                    }
                    if r.key.trim().is_empty() {
                        r.key = make_key(&r);
                    }
                    let base = r.key.clone();
                    let mut n = 1;
                    while self.refs.iter().any(|x| x.key == r.key) {
                        n += 1;
                        r.key = format!("{base}{}", char::from(b'a' + ((n - 2) % 26) as u8));
                        if n > 60 {
                            r.key = format!("{base}-{n}");
                        }
                    }
                    self.refs.push(r);
                    added += 1;
                }
            }
        }
        if added + updated > 0 {
            self.dirty = true;
        }
        (added, updated)
    }

    pub fn find(&self, key: &str) -> Option<&Ref> {
        self.refs.iter().find(|r| r.key == key)
    }

    pub fn remove(&mut self, key: &str) {
        self.refs.retain(|r| r.key != key);
        self.dirty = true;
    }

    /// Link a file (re-read when it changes); returns what was read.
    pub fn link(&mut self, path: &Path) -> Result<(usize, usize), String> {
        let n = self.read_file(path)?;
        if !self.linked.iter().any(|l| l.path == path) {
            self.linked.push(Linked { path: path.to_path_buf(), mtime: mtime(path) });
        }
        self.dirty = true;
        Ok(n)
    }

    fn read_file(&mut self, path: &Path) -> Result<(usize, usize), String> {
        let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        if len > MAX_FILE {
            return Err("the file is too large".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let refs = parse_file(&path.to_string_lossy(), &bytes)?;
        Ok(self.merge(refs))
    }

    /// Read linked files that changed since; returns how many references came in or changed.
    pub fn sync_linked(&mut self) -> usize {
        let mut n = 0;
        for i in 0..self.linked.len() {
            let Some(l) = self.linked.get(i).cloned() else { continue };
            let m = mtime(&l.path);
            if m != 0 && m != l.mtime {
                if let Ok((a, u)) = self.read_file(&l.path) {
                    n += a + u;
                }
                if let Some(x) = self.linked.get_mut(i) {
                    x.mtime = m;
                }
                self.dirty = true;
            }
        }
        n
    }

    /// All references as BibTeX.
    pub fn to_bibtex(&self, keys: Option<&[String]>) -> String {
        self.refs.iter().filter(|r| keys.is_none_or(|k| k.contains(&r.key))).map(Ref::to_bibtex).collect::<Vec<_>>().join("\n")
    }

    pub fn load(path: &Path) -> Library {
        std::fs::read(path).ok().filter(|b| b.len() as u64 <= MAX_FILE).and_then(|b| serde_json::from_slice::<Library>(&b).ok()).unwrap_or_default()
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| e.to_string())?;
        self.dirty = false;
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Zotero's local API.

/// Ask the Zotero app on this computer for its library (Zotero 7's local API: Settings ›
/// Advanced › "Allow other applications on this computer to communicate with Zotero"). Runs a
/// plain HTTP request to `127.0.0.1:23119`; returns the references or why it couldn't.
pub fn fetch_zotero() -> Result<Vec<Ref>, String> {
    use std::io::{Read, Write};
    let mut all = Vec::new();
    let mut start = 0;
    loop {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 23119));
        let mut s = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(2)).map_err(|_| {
            "Zotero isn't running, or its local API is off (Zotero › Settings › Advanced › Allow other applications to communicate with Zotero)"
                .to_string()
        })?;
        let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(20)));
        let req = format!(
            "GET /api/users/0/items/top?format=csljson&limit=100&start={start} HTTP/1.1\r\nHost: 127.0.0.1:23119\r\nZotero-API-Version: 3\r\nConnection: close\r\n\r\n"
        );
        s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        s.take(MAX_FILE).read_to_end(&mut buf).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&buf);
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
            let status = head.lines().next().unwrap_or("no answer").to_string();
            return Err(format!("Zotero answered {status}"));
        }
        let body = if head.to_ascii_lowercase().contains("transfer-encoding: chunked") { dechunk(body) } else { body.to_string() };
        let refs = parse_csl_json(&body);
        let n = refs.len();
        all.extend(refs.into_iter().map(|mut r| {
            r.origin = "zotero".into();
            r
        }));
        if n < 100 || all.len() >= MAX_REFS {
            break;
        }
        start += 100;
    }
    Ok(all)
}

/// Undo HTTP chunked transfer encoding.
fn dechunk(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some((size, after)) = rest.split_once("\r\n") {
        let Ok(n) = usize::from_str_radix(size.trim().split(';').next().unwrap_or("0"), 16) else { break };
        if n == 0 {
            break;
        }
        let Some(chunk) = after.get(..n) else {
            out.push_str(after);
            break;
        };
        out.push_str(chunk);
        rest = after.get(n..).unwrap_or("").trim_start_matches("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIB: &str = r#"
@string{nat = "Nature"}
@article{silva2024,
  author = {Silva, Ana and Jo{\~a}o Pereira and Lee, Min},
  title = {{Shade} and Soil Moisture in Urban Parks},
  journal = nat # " Cities",
  year = 2024,
  volume = {11}, number = {2}, pages = {1--14},
  doi = {10.1000/xyz123},
  abstract = {We measured soil moisture under 40 park trees.},
  keywords = {soil; shade, cities}
}
@comment{ignored}
@inproceedings{chen19, title="A Review", author="Chen, Wei", booktitle="Proc. Soil", year="2019"}
@broken{
"#;

    #[test]
    fn bibtex_is_read() {
        let r = parse_bibtex(BIB);
        assert_eq!(r.len(), 2);
        let a = &r[0];
        assert_eq!(a.key, "silva2024");
        assert_eq!(a.authors, ["Silva, Ana", "Pereira, João", "Lee, Min"]);
        assert_eq!(a.title, "Shade and Soil Moisture in Urban Parks");
        assert_eq!(a.container, "nat Cities", "macros aren't expanded, but nothing is lost");
        assert_eq!(a.pages, "1–14");
        assert_eq!(a.keywords, ["soil", "shade", "cities"]);
        assert_eq!(a.label(), "(Silva et al., 2024)");
        assert_eq!(r[1].container, "Proc. Soil");
        // Round trip.
        let again = parse_bibtex(&a.to_bibtex());
        assert_eq!(again[0].authors, a.authors);
        assert_eq!(again[0].doi, a.doi);
    }

    #[test]
    fn ris_csl_and_endnote_are_read() {
        let ris = "TY  - JOUR\r\nAU  - Silva, Ana\r\nAU  - Lee, Min\r\nTI  - Shade\r\nPY  - 2024/05/01\r\nJO  - Urban Ecology\r\nSP  - 1\r\nEP  - 14\r\nDO  - 10.1/x\r\nKW  - soil\r\nER  - \r\nTY  - BOOK\r\nAU  - Meadows, Donella\r\nTI  - Thinking in Systems\r\nPY  - 2008\r\nER  -\r\n";
        let r = parse_ris(ris);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].pages, "1–14");
        assert_eq!(r[0].year, "2024");
        assert_eq!(r[1].kind, "book");
        assert!(!r[1].key.is_empty());
        let csl = r#"{"items":[{"id":"http://zotero.org/1","type":"article-journal","title":"Shade","author":[{"family":"Silva","given":"Ana"}],"issued":{"date-parts":[[2024,5]]},"container-title":"Urban Ecology","DOI":"10.1/x"}]}"#;
        let c = parse_csl_json(csl);
        assert_eq!(c[0].authors, ["Silva, Ana"]);
        assert_eq!(c[0].year, "2024");
        assert_eq!(c[0].key, "silva2024shade");
        let xml = "<xml><records><record><ref-type name=\"Journal Article\">17</ref-type><contributors><authors><author><style>Silva, Ana</style></author></authors></contributors><titles><title><style>Shade &amp; Soil</style></title><secondary-title>Eco</secondary-title></titles><dates><year>2024</year></dates></record></records></xml>";
        let e = parse_endnote_xml(xml);
        assert_eq!(e[0].title, "Shade & Soil");
        assert_eq!(e[0].authors, ["Silva, Ana"]);
        assert_eq!(e[0].container, "Eco");
    }

    #[test]
    fn the_library_merges_duplicates_and_keeps_keys_unique() {
        let mut lib = Library::default();
        assert_eq!(lib.merge(parse_bibtex(BIB)), (2, 0));
        let mut dup = parse_bibtex(BIB);
        dup[0].abstract_.clear();
        assert_eq!(lib.merge(dup), (0, 2), "same DOI / key: updated, not added");
        let other = Ref { key: "silva2024".into(), title: "Another paper".into(), ..Default::default() };
        lib.merge(vec![other]);
        assert_eq!(lib.refs.len(), 3);
        assert_ne!(lib.refs[2].key, "silva2024");
        let src = lib.refs[0].to_source();
        assert_eq!(src["kind"], "article");
        assert_eq!(src["author"], "Silva, Ana; Pereira, João; Lee, Min");
        let back = Ref::from_source(&serde_json::from_value(src).unwrap());
        assert_eq!(back.authors, lib.refs[0].authors);
    }

    #[test]
    fn linked_files_sync_when_they_change() {
        let dir = std::env::temp_dir().join(format!("wordcraft-refs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("library.bib");
        std::fs::write(&p, BIB).unwrap();
        let mut lib = Library::default();
        assert_eq!(lib.link(&p).unwrap(), (2, 0));
        assert_eq!(lib.sync_linked(), 0, "unchanged");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&p, format!("{BIB}\n@book{{meadows08, title={{Thinking in Systems}}, author={{Meadows, Donella}}, year=2008}}")).unwrap();
        assert!(lib.sync_linked() >= 1);
        assert!(lib.find("meadows08").is_some());
        let save = dir.join("lib.json");
        lib.save(&save).unwrap();
        assert_eq!(Library::load(&save).refs.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn junk_never_panics() {
        for j in [
            "@",
            "@{",
            "@article{",
            "@article{k, a = {",
            "TY  -",
            "AU  - x",
            "[1,2]",
            "{\"items\": 5}",
            "<record><title>",
            "\u{0}\u{FFFF}",
            "@a(k, t = \"x\" # )",
        ] {
            let _ = parse_bibtex(j);
            let _ = parse_ris(j);
            let _ = parse_csl_json(j);
            let _ = parse_endnote_xml(j);
            let _ = parse_file("x", j.as_bytes());
            let _ = dechunk(j);
        }
        assert_eq!(dechunk("5\r\nhello\r\n0\r\n\r\n"), "hello");
    }
}

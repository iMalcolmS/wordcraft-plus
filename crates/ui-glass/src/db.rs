//! Databases, as in Notion: a table of records whose columns have types (text, number, select,
//! date, checkbox…), shown through saved views (Table, Board, Gallery, List, Calendar) that each
//! keep their own filters, sorts and grouping. Every record can open as a page, a real `.docx`
//! beside the database.
//!
//! A database is saved as JSON (`.wcdb`) in the workspace folder. It imports and exports CSV,
//! reads spreadsheets (`.xlsx`, `.ods`) and contacts (`.vcf`, as exported by Contacts on macOS
//! and most address books), and gives its rows to Mail Merge as recipients.
//!
//! Everything read from a file is hostile: sizes are capped, numbers are parsed leniently, and no
//! lookup indexes past the end.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Most records a database holds (more are left out on import).
pub const MAX_ROWS: usize = 100_000;
/// Most properties (columns).
pub const MAX_PROPS: usize = 300;
/// Longest text a cell keeps.
pub const MAX_CELL: usize = 64 * 1024;
/// Largest file read.
pub const MAX_FILE: u64 = 64 << 20;
/// Most options a select keeps.
const MAX_OPTIONS: usize = 500;
/// The file extension of a saved database.
pub const EXT: &str = "wcdb";

/// A colour for select options, labels and cards (the same ten as Notion's tags, drawn from our
/// own palette).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tint {
    #[default]
    Gray,
    Brown,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    Red,
    Teal,
}

impl Tint {
    pub const ALL: [Tint; 10] =
        [Tint::Gray, Tint::Brown, Tint::Orange, Tint::Yellow, Tint::Green, Tint::Blue, Tint::Purple, Tint::Pink, Tint::Red, Tint::Teal];

    /// A strong version of the colour (dots, icons, text on light chips), sRGB.
    pub fn strong(self) -> [u8; 3] {
        match self {
            Tint::Gray => [0x78, 0x77, 0x74],
            Tint::Brown => [0x9F, 0x6B, 0x53],
            Tint::Orange => [0xD9, 0x73, 0x0D],
            Tint::Yellow => [0xCB, 0x91, 0x2F],
            Tint::Green => [0x44, 0x83, 0x61],
            Tint::Blue => [0x33, 0x7E, 0xA9],
            Tint::Purple => [0x90, 0x65, 0xB0],
            Tint::Pink => [0xC1, 0x4C, 0x8A],
            Tint::Red => [0xD4, 0x4C, 0x47],
            Tint::Teal => [0x2A, 0x8F, 0x8A],
        }
    }

    /// A soft version (chip backgrounds), sRGB.
    pub fn soft(self) -> [u8; 3] {
        match self {
            Tint::Gray => [0xE9, 0xE9, 0xE7],
            Tint::Brown => [0xF1, 0xE5, 0xDE],
            Tint::Orange => [0xFA, 0xE6, 0xD3],
            Tint::Yellow => [0xFB, 0xF0, 0xD3],
            Tint::Green => [0xDD, 0xED, 0xE4],
            Tint::Blue => [0xDB, 0xEA, 0xF4],
            Tint::Purple => [0xEA, 0xE2, 0xF2],
            Tint::Pink => [0xF5, 0xE0, 0xEB],
            Tint::Red => [0xF8, 0xDF, 0xDD],
            Tint::Teal => [0xD8, 0xEE, 0xEC],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Tint::Gray => "Gray",
            Tint::Brown => "Brown",
            Tint::Orange => "Orange",
            Tint::Yellow => "Yellow",
            Tint::Green => "Green",
            Tint::Blue => "Blue",
            Tint::Purple => "Purple",
            Tint::Pink => "Pink",
            Tint::Red => "Red",
            Tint::Teal => "Teal",
        }
    }

    /// The `i`th colour, cycling (new options get the next one).
    pub fn nth(i: usize) -> Tint {
        Tint::ALL.get(i % Tint::ALL.len()).copied().unwrap_or_default()
    }
}

/// What a property holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// The record's name (exactly one per database, the first column).
    Title,
    #[default]
    Text,
    Number,
    Select,
    MultiSelect,
    /// A select whose options are stages (To do, In progress, Done).
    Status,
    /// `YYYY-MM-DD`.
    Date,
    Checkbox,
    Url,
    Email,
    Phone,
    /// 0 to 5 stars.
    Rating,
}

impl Kind {
    /// The kinds offered when adding or changing a property.
    pub const PICKABLE: [Kind; 11] = [
        Kind::Text,
        Kind::Number,
        Kind::Select,
        Kind::MultiSelect,
        Kind::Status,
        Kind::Date,
        Kind::Checkbox,
        Kind::Url,
        Kind::Email,
        Kind::Phone,
        Kind::Rating,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Title => "Title",
            Kind::Text => "Text",
            Kind::Number => "Number",
            Kind::Select => "Select",
            Kind::MultiSelect => "Multi-select",
            Kind::Status => "Status",
            Kind::Date => "Date",
            Kind::Checkbox => "Checkbox",
            Kind::Url => "URL",
            Kind::Email => "Email",
            Kind::Phone => "Phone",
            Kind::Rating => "Rating",
        }
    }

    /// Whether values come from a list of options.
    pub fn has_options(self) -> bool {
        matches!(self, Kind::Select | Kind::MultiSelect | Kind::Status)
    }

    /// Whether a Board can group by it (one option per record).
    pub fn groups(self) -> bool {
        matches!(self, Kind::Select | Kind::Status | Kind::Checkbox)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SelectOption {
    pub name: String,
    #[serde(default)]
    pub tint: Tint,
}

/// A column.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prop {
    pub id: u64,
    pub name: String,
    pub kind: Kind,
    pub options: Vec<SelectOption>,
    /// Column width in the Table view (points).
    pub width: f32,
}

/// A value. Selects hold option names; dates `YYYY-MM-DD`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v", rename_all = "camelCase")]
pub enum Cell {
    Text(String),
    Number(f64),
    Bool(bool),
    Options(Vec<String>),
}

impl Cell {
    /// The value as plain text (for display, search, CSV and Mail Merge).
    pub fn text(&self) -> String {
        match self {
            Cell::Text(s) => s.clone(),
            Cell::Number(n) => fmt_number(*n),
            Cell::Bool(b) => if *b { "Yes" } else { "No" }.into(),
            Cell::Options(o) => o.join(", "),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Cell::Text(s) => s.trim().is_empty(),
            Cell::Number(_) => false,
            Cell::Bool(b) => !*b,
            Cell::Options(o) => o.is_empty(),
        }
    }
}

/// A number as people write it: no trailing `.0`, at most four decimals.
pub fn fmt_number(n: f64) -> String {
    if !n.is_finite() {
        return String::new();
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return format!("{n:.0}");
    }
    let s = format!("{n:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A record.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Row {
    pub id: u64,
    pub cells: BTreeMap<u64, Cell>,
    /// When it was made (`YYYY-MM-DD`).
    pub created: String,
    /// Its page: a `.docx` file name in the database's pages folder, once opened.
    pub page: Option<String>,
}

/// How a view shows the records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    #[default]
    Table,
    Board,
    Gallery,
    List,
    Calendar,
}

impl Layout {
    pub const ALL: [Layout; 5] = [Layout::Table, Layout::Board, Layout::Gallery, Layout::List, Layout::Calendar];

    pub fn name(self) -> &'static str {
        match self {
            Layout::Table => "Table",
            Layout::Board => "Board",
            Layout::Gallery => "Gallery",
            Layout::List => "List",
            Layout::Calendar => "Calendar",
        }
    }

    /// The icon drawn for it (`crate::db_view::icon`).
    pub fn icon(self) -> &'static str {
        match self {
            Layout::Table => "table",
            Layout::Board => "board",
            Layout::Gallery => "gallery",
            Layout::List => "list",
            Layout::Calendar => "calendar",
        }
    }
}

/// A filter condition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Op {
    #[default]
    Contains,
    Is,
    IsNot,
    IsEmpty,
    IsNotEmpty,
    Greater,
    Less,
}

impl Op {
    pub const ALL: [Op; 7] = [Op::Contains, Op::Is, Op::IsNot, Op::IsEmpty, Op::IsNotEmpty, Op::Greater, Op::Less];

    pub fn name(self) -> &'static str {
        match self {
            Op::Contains => "contains",
            Op::Is => "is",
            Op::IsNot => "is not",
            Op::IsEmpty => "is empty",
            Op::IsNotEmpty => "is not empty",
            Op::Greater => "is after / greater than",
            Op::Less => "is before / less than",
        }
    }

    /// Whether the condition takes a value.
    pub fn takes_value(self) -> bool {
        !matches!(self, Op::IsEmpty | Op::IsNotEmpty)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Filter {
    pub prop: u64,
    pub op: Op,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sort {
    pub prop: u64,
    pub desc: bool,
}

/// A saved view.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct View {
    pub id: u64,
    pub name: String,
    pub layout: Layout,
    pub filters: Vec<Filter>,
    pub sorts: Vec<Sort>,
    /// Board columns (and Table groups) come from this property.
    pub group_by: Option<u64>,
    /// The Calendar places records by this date.
    pub date_prop: Option<u64>,
    /// Properties not shown (Table columns, card lines).
    pub hidden: Vec<u64>,
}

/// A database.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Database {
    pub name: String,
    pub description: String,
    /// An icon name from `crate::db_view::icon` (a book, a flask, a person…).
    pub icon: String,
    pub props: Vec<Prop>,
    pub rows: Vec<Row>,
    pub views: Vec<View>,
    /// The view showing (index into `views`).
    pub current: usize,
    next_id: u64,
    /// Where it is saved (not written to the file).
    #[serde(skip)]
    pub path: Option<std::path::PathBuf>,
    /// Changed since it was saved.
    #[serde(skip)]
    pub dirty: bool,
    /// Bumped by every change (for caches).
    #[serde(skip)]
    pub rev: u64,
}

/// Today's date, `YYYY-MM-DD` (UTC).
pub fn today() -> String {
    wordcraft_engine::cmd::now_iso().get(..10).unwrap_or("1970-01-01").to_string()
}

/// Cut `s` to at most `max` bytes at a character boundary.
fn cap(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.get(..end).unwrap_or("").to_string()
}

impl Database {
    /// An empty database with a Title column and one Table view.
    pub fn new(name: &str) -> Self {
        let mut db = Database { name: name.into(), icon: "database".into(), ..Default::default() };
        db.add_prop("Name", Kind::Title);
        db.add_view("Table", Layout::Table);
        db
    }

    fn next(&mut self) -> u64 {
        self.next_id = self.next_id.saturating_add(1);
        self.next_id
    }

    /// Record a change.
    pub fn touch(&mut self) {
        self.dirty = true;
        self.rev = self.rev.wrapping_add(1);
    }

    /// Add a property; returns its id (0 when the database already has [`MAX_PROPS`]). A second
    /// Title becomes Text.
    pub fn add_prop(&mut self, name: &str, kind: Kind) -> u64 {
        if self.props.len() >= MAX_PROPS {
            return 0;
        }
        let kind = if kind == Kind::Title && self.props.iter().any(|p| p.kind == Kind::Title) { Kind::Text } else { kind };
        let id = self.next();
        let mut p = Prop { id, name: cap(name.trim(), 200), kind, options: Vec::new(), width: if kind == Kind::Title { 260.0 } else { 170.0 } };
        if kind == Kind::Status {
            p.options = vec![
                SelectOption { name: "To do".into(), tint: Tint::Gray },
                SelectOption { name: "In progress".into(), tint: Tint::Blue },
                SelectOption { name: "Done".into(), tint: Tint::Green },
            ];
        }
        if p.name.is_empty() {
            p.name = kind.name().to_string();
        }
        self.props.push(p);
        self.touch();
        id
    }

    /// Add a property with options (a Select, Multi-select or Status).
    pub fn add_select(&mut self, name: &str, kind: Kind, options: &[(&str, Tint)]) -> u64 {
        let id = self.add_prop(name, kind);
        if let Some(p) = self.prop_mut(id) {
            p.options = options.iter().map(|(n, t)| SelectOption { name: (*n).into(), tint: *t }).collect();
        }
        id
    }

    pub fn add_view(&mut self, name: &str, layout: Layout) -> u64 {
        let id = self.next();
        let group_by = if layout == Layout::Board { self.props.iter().find(|p| p.kind.groups()).map(|p| p.id) } else { None };
        let date_prop = if layout == Layout::Calendar { self.props.iter().find(|p| p.kind == Kind::Date).map(|p| p.id) } else { None };
        self.views.push(View { id, name: name.into(), layout, group_by, date_prop, ..Default::default() });
        self.touch();
        id
    }

    pub fn prop(&self, id: u64) -> Option<&Prop> {
        self.props.iter().find(|p| p.id == id)
    }

    pub fn prop_mut(&mut self, id: u64) -> Option<&mut Prop> {
        self.props.iter_mut().find(|p| p.id == id)
    }

    /// The Title property's id.
    pub fn title_prop(&self) -> Option<u64> {
        self.props.iter().find(|p| p.kind == Kind::Title).map(|p| p.id)
    }

    /// The property named `name` (case-insensitive).
    pub fn prop_named(&self, name: &str) -> Option<u64> {
        self.props.iter().find(|p| p.name.eq_ignore_ascii_case(name.trim())).map(|p| p.id)
    }

    pub fn row(&self, id: u64) -> Option<&Row> {
        self.rows.iter().find(|r| r.id == id)
    }

    pub fn row_mut(&mut self, id: u64) -> Option<&mut Row> {
        self.rows.iter_mut().find(|r| r.id == id)
    }

    /// Add an empty record (at the end); returns its id (0 when full).
    pub fn add_row(&mut self) -> u64 {
        if self.rows.len() >= MAX_ROWS {
            return 0;
        }
        let id = self.next();
        self.rows.push(Row { id, created: today(), ..Default::default() });
        self.touch();
        id
    }

    /// Add a record from (property name, text) pairs; unknown names are ignored. Returns its id.
    pub fn add_row_with(&mut self, values: &[(&str, &str)]) -> u64 {
        let id = self.add_row();
        for (name, text) in values {
            if let Some(p) = self.prop_named(name) {
                self.set_text(id, p, text);
            }
        }
        id
    }

    pub fn delete_row(&mut self, id: u64) {
        self.rows.retain(|r| r.id != id);
        self.touch();
    }

    /// Copy a record (without its page) right after it; returns the copy's id.
    pub fn duplicate_row(&mut self, id: u64) -> u64 {
        if self.rows.len() >= MAX_ROWS {
            return 0;
        }
        let Some(i) = self.rows.iter().position(|r| r.id == id) else { return 0 };
        let new_id = self.next();
        let mut copy = self.rows.get(i).cloned().unwrap_or_default();
        copy.id = new_id;
        copy.page = None;
        copy.created = today();
        self.rows.insert(i + 1, copy);
        self.touch();
        new_id
    }

    pub fn delete_prop(&mut self, id: u64) {
        if self.prop(id).is_some_and(|p| p.kind == Kind::Title) {
            return;
        }
        self.props.retain(|p| p.id != id);
        for r in &mut self.rows {
            r.cells.remove(&id);
        }
        for v in &mut self.views {
            v.filters.retain(|f| f.prop != id);
            v.sorts.retain(|s| s.prop != id);
            v.hidden.retain(|h| *h != id);
            if v.group_by == Some(id) {
                v.group_by = None;
            }
            if v.date_prop == Some(id) {
                v.date_prop = None;
            }
        }
        self.touch();
    }

    /// Change a property's kind, converting its values (text that doesn't fit is dropped).
    pub fn set_kind(&mut self, id: u64, kind: Kind) {
        let Some(old) = self.prop(id).map(|p| p.kind) else { return };
        if old == kind || old == Kind::Title || kind == Kind::Title {
            return;
        }
        let texts: Vec<(u64, String)> = self.rows.iter().map(|r| (r.id, r.cells.get(&id).map(Cell::text).unwrap_or_default())).collect();
        if let Some(p) = self.prop_mut(id) {
            p.kind = kind;
            if !kind.has_options() {
                p.options.clear();
            }
        }
        for (rid, text) in texts {
            if let Some(r) = self.row_mut(rid) {
                r.cells.remove(&id);
            }
            if !text.is_empty() {
                self.set_text(rid, id, &text);
            }
        }
        self.touch();
    }

    /// The option `name` of property `prop`, added (with the next colour) when missing.
    pub fn ensure_option(&mut self, prop: u64, name: &str) {
        let name = cap(name.trim(), 200);
        if name.is_empty() {
            return;
        }
        if let Some(p) = self.prop_mut(prop)
            && p.kind.has_options()
            && !p.options.iter().any(|o| o.name == name)
            && p.options.len() < MAX_OPTIONS
        {
            let tint = Tint::nth(p.options.len() + 1);
            p.options.push(SelectOption { name, tint });
        }
    }

    /// The colour of option `name` of `prop`.
    pub fn option_tint(&self, prop: u64, name: &str) -> Tint {
        self.prop(prop).and_then(|p| p.options.iter().find(|o| o.name == name)).map(|o| o.tint).unwrap_or_default()
    }

    pub fn cell(&self, row: u64, prop: u64) -> Option<&Cell> {
        self.row(row)?.cells.get(&prop)
    }

    /// A cell as text.
    pub fn text(&self, row: u64, prop: u64) -> String {
        self.cell(row, prop).map(Cell::text).unwrap_or_default()
    }

    /// The record's title (or "Untitled").
    pub fn title(&self, row: u64) -> String {
        let t = self.title_prop().map(|p| self.text(row, p)).unwrap_or_default();
        if t.trim().is_empty() { "Untitled".into() } else { t }
    }

    /// Set a cell directly.
    pub fn set(&mut self, row: u64, prop: u64, cell: Option<Cell>) {
        if self.prop(prop).is_none() {
            return;
        }
        if let Some(r) = self.row_mut(row) {
            match cell {
                Some(c) => {
                    let c = match c {
                        Cell::Text(s) => Cell::Text(cap(&s, MAX_CELL)),
                        other => other,
                    };
                    r.cells.insert(prop, c);
                }
                None => {
                    r.cells.remove(&prop);
                }
            }
        }
        self.touch();
    }

    /// Set a cell from text, as typed or read from a file: parsed for the property's kind
    /// (numbers, dates, yes/no, options split on commas for a Multi-select).
    pub fn set_text(&mut self, row: u64, prop: u64, text: &str) {
        let Some(kind) = self.prop(prop).map(|p| p.kind) else { return };
        let t = text.trim();
        let cell = if t.is_empty() {
            None
        } else {
            match kind {
                Kind::Number => parse_number(t).map(Cell::Number),
                Kind::Rating => parse_number(t).map(|n| Cell::Number(n.round().clamp(0.0, 5.0))),
                Kind::Checkbox => Some(Cell::Bool(parse_bool(t))),
                Kind::Date => parse_date(t).map(Cell::Text),
                Kind::Select | Kind::Status => {
                    self.ensure_option(prop, t);
                    Some(Cell::Options(vec![cap(t, 200)]))
                }
                Kind::MultiSelect => {
                    let names: Vec<String> = t.split([',', ';']).map(|s| cap(s.trim(), 200)).filter(|s| !s.is_empty()).take(MAX_OPTIONS).collect();
                    for n in &names {
                        self.ensure_option(prop, n);
                    }
                    Some(Cell::Options(names))
                }
                Kind::Title | Kind::Text | Kind::Url | Kind::Email | Kind::Phone => Some(Cell::Text(t.to_string())),
            }
        };
        self.set(row, prop, cell);
    }

    /// Toggle option `name` in a Multi-select (or pick it in a Select).
    pub fn toggle_option(&mut self, row: u64, prop: u64, name: &str) {
        let Some(kind) = self.prop(prop).map(|p| p.kind) else { return };
        self.ensure_option(prop, name);
        let mut cur = match self.cell(row, prop) {
            Some(Cell::Options(o)) => o.clone(),
            _ => Vec::new(),
        };
        if kind == Kind::MultiSelect {
            if let Some(i) = cur.iter().position(|o| o == name) {
                cur.remove(i);
            } else {
                cur.push(name.to_string());
            }
        } else if cur.first().map(String::as_str) == Some(name) {
            cur.clear();
        } else {
            cur = vec![name.to_string()];
        }
        self.set(row, prop, if cur.is_empty() { None } else { Some(Cell::Options(cur)) });
    }

    /// The records a view shows, in order: filtered, matched against `search`, then sorted.
    pub fn visible(&self, view: &View, search: &str) -> Vec<u64> {
        let q = search.trim().to_lowercase();
        let mut ids: Vec<u64> = self
            .rows
            .iter()
            .filter(|r| view.filters.iter().all(|f| self.passes(r, f)))
            .filter(|r| q.is_empty() || r.cells.values().any(|c| c.text().to_lowercase().contains(&q)))
            .map(|r| r.id)
            .collect();
        if !view.sorts.is_empty() {
            ids.sort_by(|a, b| {
                for s in &view.sorts {
                    let o = self.compare(*a, *b, s.prop);
                    let o = if s.desc { o.reverse() } else { o };
                    if o != std::cmp::Ordering::Equal {
                        return o;
                    }
                }
                std::cmp::Ordering::Equal
            });
        }
        ids
    }

    /// Order two records by a property (empty values last).
    fn compare(&self, a: u64, b: u64, prop: u64) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self.cell(a, prop), self.cell(b, prop)) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(Cell::Number(x)), Some(Cell::Number(y))) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
            (Some(Cell::Bool(x)), Some(Cell::Bool(y))) => y.cmp(x),
            (Some(Cell::Options(x)), Some(Cell::Options(y))) => {
                // Options sort in the order they are listed (To do, In progress, Done).
                let rank = |v: &Vec<String>| {
                    v.first().and_then(|n| self.prop(prop).and_then(|p| p.options.iter().position(|o| &o.name == n))).unwrap_or(usize::MAX)
                };
                rank(x).cmp(&rank(y))
            }
            (Some(x), Some(y)) => x.text().to_lowercase().cmp(&y.text().to_lowercase()),
        }
    }

    fn passes(&self, row: &Row, f: &Filter) -> bool {
        let Some(prop) = self.prop(f.prop) else { return true };
        let cell = row.cells.get(&f.prop);
        let text = cell.map(Cell::text).unwrap_or_default().to_lowercase();
        let want = f.value.trim().to_lowercase();
        let empty = cell.is_none_or(Cell::is_empty);
        match f.op {
            Op::IsEmpty => empty,
            Op::IsNotEmpty => !empty,
            Op::Contains => want.is_empty() || text.contains(&want),
            Op::Is | Op::IsNot => {
                let is = match (prop.kind, cell) {
                    (Kind::Checkbox, c) => c.is_some_and(|c| matches!(c, Cell::Bool(true))) == parse_bool(&want),
                    (_, Some(Cell::Options(o))) => o.iter().any(|x| x.to_lowercase() == want),
                    (Kind::Number | Kind::Rating, Some(Cell::Number(n))) => parse_number(&want).is_some_and(|w| (w - n).abs() < 1e-9),
                    _ => text == want,
                };
                if f.op == Op::Is { is } else { !is }
            }
            Op::Greater | Op::Less => {
                let ord = match (cell, parse_number(&want)) {
                    (Some(Cell::Number(n)), Some(w)) => n.partial_cmp(&w),
                    _ if !empty && !want.is_empty() => Some(text.as_str().cmp(want.as_str())),
                    _ => None,
                };
                match ord {
                    Some(o) => (f.op == Op::Greater && o == std::cmp::Ordering::Greater) || (f.op == Op::Less && o == std::cmp::Ordering::Less),
                    None => false,
                }
            }
        }
    }

    /// The groups of a Board (or grouped Table) by `prop`: each option, then records with none.
    /// Checkboxes group into Checked and Unchecked.
    pub fn groups(&self, ids: &[u64], prop: u64) -> Vec<(Option<String>, Vec<u64>)> {
        let Some(p) = self.prop(prop) else { return vec![(None, ids.to_vec())] };
        if p.kind == Kind::Checkbox {
            let (on, off): (Vec<u64>, Vec<u64>) = ids.iter().partition(|id| matches!(self.cell(**id, prop), Some(Cell::Bool(true))));
            return vec![(Some("Unchecked".into()), off), (Some("Checked".into()), on)];
        }
        let mut out: Vec<(Option<String>, Vec<u64>)> = p.options.iter().map(|o| (Some(o.name.clone()), Vec::new())).collect();
        let mut none = Vec::new();
        for id in ids {
            let first = match self.cell(*id, prop) {
                Some(Cell::Options(o)) => o.first().cloned(),
                _ => None,
            };
            match first.and_then(|n| out.iter_mut().find(|(g, _)| g.as_deref() == Some(n.as_str()))) {
                Some((_, v)) => v.push(*id),
                None => none.push(*id),
            }
        }
        out.insert(0, (None, none));
        out
    }

    /// Put a record in a Board group (`None` clears it).
    pub fn move_to_group(&mut self, row: u64, prop: u64, group: Option<&str>) {
        match self.prop(prop).map(|p| p.kind) {
            Some(Kind::Checkbox) => self.set(row, prop, Some(Cell::Bool(group == Some("Checked")))),
            Some(_) => match group {
                Some(g) => self.set_text(row, prop, g),
                None => self.set(row, prop, None),
            },
            None => {}
        }
    }

    /// Move record `id` to just before `before` (or to the end), for manual ordering.
    pub fn move_row(&mut self, id: u64, before: Option<u64>) {
        let Some(i) = self.rows.iter().position(|r| r.id == id) else { return };
        let row = self.rows.remove(i);
        let at = before.and_then(|b| self.rows.iter().position(|r| r.id == b)).unwrap_or(self.rows.len());
        self.rows.insert(at, row);
        self.touch();
    }

    // ---------------------------------------------------------------------------------------
    // Files.

    /// The database as JSON (its `.wcdb` file).
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    /// Read a `.wcdb` file's bytes, repairing what doesn't hold together (a missing Title, ids
    /// past the counter, too many rows).
    pub fn from_json(bytes: &[u8]) -> Result<Database, String> {
        if bytes.len() as u64 > MAX_FILE {
            return Err("the file is too large".into());
        }
        let mut db: Database = serde_json::from_slice(bytes).map_err(|e| format!("not a WordCraft database: {e}"))?;
        db.repair();
        Ok(db)
    }

    fn repair(&mut self) {
        self.props.truncate(MAX_PROPS);
        self.rows.truncate(MAX_ROWS);
        let mut titles = 0;
        for p in &mut self.props {
            if p.kind == Kind::Title {
                titles += 1;
                if titles > 1 {
                    p.kind = Kind::Text;
                }
            }
            p.options.truncate(MAX_OPTIONS);
            p.width = if p.width.is_finite() { p.width.clamp(60.0, 900.0) } else { 170.0 };
        }
        let max_id = self.props.iter().map(|p| p.id).chain(self.rows.iter().map(|r| r.id)).chain(self.views.iter().map(|v| v.id)).max().unwrap_or(0);
        self.next_id = self.next_id.max(max_id);
        if titles == 0 {
            let id = self.next();
            self.props.insert(0, Prop { id, name: "Name".into(), kind: Kind::Title, options: Vec::new(), width: 260.0 });
        }
        if self.views.is_empty() {
            let id = self.next();
            self.views.push(View { id, name: "Table".into(), ..Default::default() });
        }
        if self.name.trim().is_empty() {
            self.name = "Untitled database".into();
        }
    }

    /// Save to `path` (creating its folder).
    pub fn save_to(&mut self, path: &std::path::Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(path, self.to_json()).map_err(|e| format!("{}: {e}", path.display()))?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    /// Read a database file.
    pub fn open(path: &std::path::Path) -> Result<Database, String> {
        let bytes = read_capped(path)?;
        let mut db = Database::from_json(&bytes)?;
        db.path = Some(path.to_path_buf());
        db.dirty = false;
        Ok(db)
    }

    /// The records as CSV: a header row of property names, then one line per record.
    pub fn to_csv(&self, ids: Option<&[u64]>) -> String {
        let mut out = String::new();
        let line = |cells: Vec<String>| cells.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(",");
        out.push_str(&line(self.props.iter().map(|p| p.name.clone()).collect()));
        out.push('\n');
        let all: Vec<u64> = self.rows.iter().map(|r| r.id).collect();
        for id in ids.unwrap_or(&all) {
            out.push_str(&line(self.props.iter().map(|p| self.text(*id, p.id)).collect()));
            out.push('\n');
        }
        out
    }

    /// The records as Mail Merge recipients: field names and one row of text per record.
    pub fn merge_table(&self, ids: Option<&[u64]>) -> (Vec<String>, Vec<Vec<String>>) {
        let headers: Vec<String> = self.props.iter().map(|p| p.name.clone()).collect();
        let all: Vec<u64> = self.rows.iter().map(|r| r.id).collect();
        let rows = ids.unwrap_or(&all).iter().map(|id| self.props.iter().map(|p| self.text(*id, p.id)).collect()).collect();
        (headers, rows)
    }

    /// A database from a table of text (CSV, a spreadsheet): the first row names the properties,
    /// whose kinds are guessed from the values (numbers, dates, yes/no, a few repeated words →
    /// Select). The first column is the Title.
    pub fn from_table(name: &str, table: &[Vec<String>]) -> Database {
        let mut db = Database { name: name.into(), icon: "table".into(), ..Default::default() };
        let Some(header) = table.first() else {
            db.repair();
            return db;
        };
        let body: Vec<&Vec<String>> = table.iter().skip(1).filter(|r| r.iter().any(|c| !c.trim().is_empty())).take(MAX_ROWS).collect();
        let mut ids = Vec::new();
        for (i, h) in header.iter().take(MAX_PROPS).enumerate() {
            let values: Vec<&str> = body.iter().filter_map(|r| r.get(i)).map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            let kind = if i == 0 { Kind::Title } else { guess_kind(h, &values) };
            let name = if h.trim().is_empty() { format!("Column {}", i + 1) } else { h.clone() };
            ids.push(db.add_prop(&name, kind));
        }
        for r in body {
            let rid = db.add_row();
            for (i, pid) in ids.iter().enumerate() {
                if let Some(v) = r.get(i) {
                    db.set_text(rid, *pid, v);
                }
            }
        }
        db.add_view("Table", Layout::Table);
        if db.props.iter().any(|p| p.kind.groups() && p.kind != Kind::Checkbox) {
            db.add_view("Board", Layout::Board);
        }
        if db.props.iter().any(|p| p.kind == Kind::Date) {
            db.add_view("Calendar", Layout::Calendar);
        }
        db.repair();
        db.dirty = true;
        db
    }

    /// Add the records of a table (CSV, a spreadsheet) whose first row names the fields: columns
    /// matching a property by name fill it; the others become new Text properties. Returns how
    /// many records were added.
    pub fn append_table(&mut self, table: &[Vec<String>]) -> usize {
        let Some(header) = table.first() else { return 0 };
        let pids: Vec<u64> = header
            .iter()
            .take(MAX_PROPS)
            .map(|h| self.prop_named(h).unwrap_or_else(|| if h.trim().is_empty() { 0 } else { self.add_prop(h, Kind::Text) }))
            .collect();
        let mut n = 0;
        for r in table.iter().skip(1).filter(|r| r.iter().any(|c| !c.trim().is_empty())) {
            let rid = self.add_row();
            if rid == 0 {
                break;
            }
            for (pid, v) in pids.iter().zip(r) {
                if *pid != 0 {
                    self.set_text(rid, *pid, v);
                }
            }
            n += 1;
        }
        n
    }
}

/// Guess a column's kind from its name and values.
fn guess_kind(name: &str, values: &[&str]) -> Kind {
    let n = name.to_lowercase();
    if values.is_empty() {
        return if n.contains("date") || n.contains("due") { Kind::Date } else { Kind::Text };
    }
    let all = |f: &dyn Fn(&str) -> bool| values.iter().all(|v| f(v));
    if n.contains("email") || n.contains("e-mail") || all(&|v| v.contains('@') && !v.contains(' ')) {
        return Kind::Email;
    }
    if n.contains("url") || n.contains("link") || n.contains("doi") || all(&|v| v.starts_with("http://") || v.starts_with("https://")) {
        return Kind::Url;
    }
    if n.contains("phone") || n.contains("tel") || n.contains("mobile") {
        return Kind::Phone;
    }
    if all(&|v| parse_date(v).is_some() && v.chars().filter(|c| c.is_ascii_digit()).count() >= 6) {
        return Kind::Date;
    }
    if all(&|v| matches!(v.to_lowercase().as_str(), "yes" | "no" | "true" | "false" | "x" | "✓" | "sim" | "não" | "nao")) {
        return Kind::Checkbox;
    }
    if all(&|v| parse_number(v).is_some()) {
        return Kind::Number;
    }
    let mut distinct: Vec<&str> = values.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    if n.contains("status") || n.contains("stage") {
        return Kind::Select;
    }
    if values.len() >= 4 && distinct.len() <= 8 && distinct.len() * 2 <= values.len() && distinct.iter().all(|v| v.chars().count() <= 30) {
        return Kind::Select;
    }
    Kind::Text
}

/// A number as people type it: `1,234.5`, `12.5%`, `£30`. Not NaN or infinite.
pub fn parse_number(s: &str) -> Option<f64> {
    let t: String = s.trim().chars().filter(|c| !matches!(c, ',' | ' ' | '_' | '$' | '£' | '€' | '%')).collect();
    if t.is_empty() || t.len() > 40 {
        return None;
    }
    t.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Yes or no, as people write it.
pub fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_lowercase().as_str(), "yes" | "true" | "x" | "✓" | "1" | "checked" | "sim" | "on")
}

/// A date as `YYYY-MM-DD`, from `2026-10-12`, `2026/10/12`, `12/10/2026` (day first, as in most
/// of the world, unless the first number can't be a day), or `2026-10-12T…`.
pub fn parse_date(s: &str) -> Option<String> {
    let t = s.trim();
    let t = t.get(..t.find('T').unwrap_or(t.len())).unwrap_or(t);
    let parts: Vec<&str> = t.split(['-', '/', '.']).map(str::trim).collect();
    let [a, b, c] = parts.as_slice() else { return None };
    let num = |x: &str| -> Option<u32> { if x.is_empty() || x.len() > 4 || !x.chars().all(|c| c.is_ascii_digit()) { None } else { x.parse().ok() } };
    let (a, b, c) = (num(a)?, num(b)?, num(c)?);
    let (y, m, d) = if a > 31 {
        (a, b, c)
    } else if c > 31 || c >= 100 {
        if a > 12 {
            (c, b, a)
        } else if b > 12 {
            (c, a, b)
        } else {
            (c, b, a)
        }
    } else {
        return None;
    };
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d < 1 || d > days_in_month(y as i32, m) {
        return None;
    }
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Day of the week of a date, 0 = Monday.
pub fn weekday(y: i32, m: u32, d: u32) -> u32 {
    // Sakamoto's method.
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    let t = T.get((m.clamp(1, 12) - 1) as usize).copied().unwrap_or(0);
    let sunday0 = (y + y / 4 - y / 100 + y / 400 + t + d as i32).rem_euclid(7);
    ((sunday0 + 6) % 7) as u32
}

fn read_capped(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let len = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.len();
    if len > MAX_FILE {
        return Err(format!("{}: too large", path.display()));
    }
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

/// Parse CSV (or TSV when the first line has tabs and no commas): quoted fields, doubled quotes,
/// newlines inside quotes. Capped at [`MAX_ROWS`] + 1 rows.
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or("");
    let sep = if first.contains('\t') && !first.contains(',') {
        '\t'
    } else if first.contains(';') && !first.contains(',') {
        ';'
    } else {
        ','
    };
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else if field.len() < MAX_CELL {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            c if c == sep => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                if rows.len() > MAX_ROWS {
                    return rows;
                }
            }
            c => {
                if field.len() < MAX_CELL {
                    field.push(c);
                }
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Contacts from a vCard file (`.vcf`, versions 2.1–4.0): one row per card with First Name,
/// Last Name, Full Name, Company, Email, Phone, Address, City, Region, Postal Code, Country and
/// Birthday. Folded lines are joined and `QUOTED-PRINTABLE` and escaped values decoded.
pub fn parse_vcard(text: &str) -> Vec<Vec<String>> {
    const FIELDS: [&str; 12] =
        ["Full Name", "First Name", "Last Name", "Company", "Email", "Phone", "Address", "City", "Region", "Postal Code", "Country", "Birthday"];
    let mut out = vec![FIELDS.iter().map(|s| (*s).to_string()).collect::<Vec<_>>()];
    // Unfold: a line starting with a space or tab continues the previous one.
    let mut lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        if let Some(rest) = raw.strip_prefix([' ', '\t'])
            && let Some(last) = lines.last_mut()
        {
            last.push_str(rest);
            continue;
        }
        // Quoted-printable soft breaks end a line with `=`.
        if let Some(last) = lines.last_mut()
            && last.ends_with('=')
            && last.to_ascii_uppercase().contains("QUOTED-PRINTABLE")
        {
            last.pop();
            last.push_str(raw);
            continue;
        }
        lines.push(raw.to_string());
    }
    let mut card: Option<[String; 12]> = None;
    for line in lines {
        let Some((key, value)) = line.split_once(':') else { continue };
        let mut params = key.split(';');
        let name = params.next().unwrap_or("").rsplit('.').next().unwrap_or("").to_ascii_uppercase();
        let qp = key.to_ascii_uppercase().contains("QUOTED-PRINTABLE");
        let value = if qp { decode_qp(value) } else { value.to_string() };
        match name.as_str() {
            "BEGIN" if value.eq_ignore_ascii_case("VCARD") => card = Some(Default::default()),
            "END" if value.eq_ignore_ascii_case("VCARD") => {
                if let Some(c) = card.take() {
                    let mut c = c;
                    if c[0].is_empty() {
                        c[0] = format!("{} {}", c[1], c[2]).trim().to_string();
                    }
                    if c.iter().any(|f| !f.is_empty()) && out.len() <= MAX_ROWS {
                        out.push(c.to_vec());
                    }
                }
            }
            _ => {
                let Some(c) = card.as_mut() else { continue };
                let parts: Vec<String> = split_escaped(&value);
                let set = |slot: &mut String, v: &str| {
                    if slot.is_empty() {
                        *slot = cap(v.trim(), 4096);
                    }
                };
                match name.as_str() {
                    "FN" => set(&mut c[0], &parts.join(" ")),
                    "N" => {
                        set(&mut c[2], parts.first().map(String::as_str).unwrap_or(""));
                        set(&mut c[1], parts.get(1).map(String::as_str).unwrap_or(""));
                    }
                    "ORG" => set(&mut c[3], &parts.join(", ")),
                    "EMAIL" => set(&mut c[4], &parts.join("")),
                    "TEL" => set(&mut c[5], &parts.join("")),
                    "ADR" => {
                        // PO box; extended; street; locality; region; postal code; country.
                        let get = |i: usize| parts.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
                        let street = [get(0), get(1), get(2)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
                        set(&mut c[6], &street);
                        set(&mut c[7], &get(3));
                        set(&mut c[8], &get(4));
                        set(&mut c[9], &get(5));
                        set(&mut c[10], &get(6));
                    }
                    "BDAY" => set(&mut c[11], &parse_date(&value).unwrap_or(value.clone())),
                    _ => {}
                }
            }
        }
    }
    out
}

/// Split a vCard value on unescaped `;`, decoding `\,` `\;` `\n` `\\`.
fn split_escaped(v: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('n') | Some('N') => out.last_mut().map(|s| s.push('\n')).unwrap_or(()),
                Some(x) => out.last_mut().map(|s| s.push(x)).unwrap_or(()),
                None => {}
            },
            ';' => out.push(String::new()),
            c => out.last_mut().map(|s| s.push(c)).unwrap_or(()),
        }
    }
    out
}

/// Decode quoted-printable (`=C3=A9` → é).
fn decode_qp(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b.get(i).copied().unwrap_or(0);
        if c == b'='
            && let (Some(h), Some(l)) = (b.get(i + 1), b.get(i + 2))
            && let (Some(h), Some(l)) = ((*h as char).to_digit(16), (*l as char).to_digit(16))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(c);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A table of text from a file's bytes, by its extension: CSV/TSV/text, a spreadsheet (first
/// sheet) or a vCard.
pub fn table_from_file(name: &str, bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    if bytes.len() as u64 > MAX_FILE {
        return Err("the file is too large".into());
    }
    let ext = std::path::Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if ext == "vcf" || ext == "vcard" {
        return Ok(parse_vcard(&String::from_utf8_lossy(bytes)));
    }
    if wordcraft_formats::sheet::is_extension(&ext) || wordcraft_formats::sheet::detect(bytes).is_some() {
        let (_, rows) = wordcraft_formats::sheet::read_rows(bytes, wordcraft_formats::sheet::Pick::Index(0))?;
        return Ok(rows);
    }
    if ext == "numbers" {
        return Err("Numbers files can't be read directly: in Numbers, choose File › Export To › CSV or Excel, and import that.".into());
    }
    Ok(parse_csv(&String::from_utf8_lossy(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading_list() -> Database {
        let mut db = Database::new("Reading");
        db.add_prop("Year", Kind::Number);
        db.add_select("Status", Kind::Status, &[("To read", Tint::Gray), ("Reading", Tint::Blue), ("Read", Tint::Green)]);
        db.add_prop("Tags", Kind::MultiSelect);
        db.add_prop("Due", Kind::Date);
        db.add_row_with(&[
            ("Name", "Thinking in Systems"),
            ("Year", "2008"),
            ("Status", "Read"),
            ("Tags", "systems, ecology"),
            ("Due", "2026-10-20"),
        ]);
        db.add_row_with(&[("Name", "The Structure of Scientific Revolutions"), ("Year", "1962"), ("Status", "Reading")]);
        db.add_row_with(&[("Name", "Silent Spring"), ("Year", "1962"), ("Status", "To read"), ("Tags", "ecology")]);
        db
    }

    #[test]
    fn values_are_parsed_for_their_kind() {
        let db = reading_list();
        let year = db.prop_named("year").unwrap();
        let tags = db.prop_named("Tags").unwrap();
        let first = db.rows[0].id;
        assert_eq!(db.cell(first, year), Some(&Cell::Number(2008.0)));
        assert_eq!(db.cell(first, tags), Some(&Cell::Options(vec!["systems".into(), "ecology".into()])));
        assert_eq!(db.prop(tags).unwrap().options.len(), 2, "options are created as used");
        assert_eq!(db.title(first), "Thinking in Systems");
    }

    #[test]
    fn views_filter_sort_and_group() {
        let mut db = reading_list();
        let year = db.prop_named("Year").unwrap();
        let status = db.prop_named("Status").unwrap();
        let tags = db.prop_named("Tags").unwrap();
        let mut v = View { sorts: vec![Sort { prop: year, desc: true }], ..Default::default() };
        let ids = db.visible(&v, "");
        assert_eq!(db.title(ids[0]), "Thinking in Systems");
        v.filters.push(Filter { prop: tags, op: Op::Contains, value: "ecology".into() });
        assert_eq!(db.visible(&v, "").len(), 2);
        assert_eq!(db.visible(&v, "silent").len(), 1);
        let all = db.visible(&View::default(), "");
        let groups = db.groups(&all, status);
        let names: Vec<_> = groups.iter().map(|(g, ids)| (g.clone(), ids.len())).collect();
        assert_eq!(names, [(None, 0), (Some("To read".into()), 1), (Some("Reading".into()), 1), (Some("Read".into()), 1)]);
        let id = all[2];
        db.move_to_group(id, status, Some("Read"));
        assert_eq!(db.text(id, status), "Read");
        let f = Filter { prop: year, op: Op::Greater, value: "2000".into() };
        assert_eq!(db.visible(&View { filters: vec![f], ..Default::default() }, "").len(), 1);
    }

    #[test]
    fn csv_round_trips_and_kinds_are_guessed() {
        let db = reading_list();
        let csv = db.to_csv(None);
        let back = Database::from_table("Copy", &parse_csv(&csv));
        assert_eq!(back.rows.len(), 3);
        assert_eq!(back.props[0].kind, Kind::Title);
        assert_eq!(back.prop(back.prop_named("Year").unwrap()).unwrap().kind, Kind::Number);
        assert_eq!(back.prop(back.prop_named("Due").unwrap()).unwrap().kind, Kind::Date);
        assert_eq!(back.title(back.rows[1].id), "The Structure of Scientific Revolutions");
        let tricky = parse_csv("a,b\n\"x, \"\"y\"\"\",\"line\nbreak\"\n");
        assert_eq!(tricky[1], vec!["x, \"y\"".to_string(), "line\nbreak".to_string()]);
    }

    #[test]
    fn json_round_trips_and_repairs() {
        let db = reading_list();
        let back = Database::from_json(&db.to_json()).unwrap();
        assert_eq!(back.rows, db.rows);
        assert_eq!(back.props, db.props);
        let fixed = Database::from_json(br#"{"name":"","props":[],"rows":[{"id":5}]}"#).unwrap();
        assert_eq!(fixed.props[0].kind, Kind::Title);
        assert!(!fixed.views.is_empty());
        assert!(fixed.next_id >= 5);
        assert!(Database::from_json(b"not json").is_err());
    }

    #[test]
    fn dates_numbers_and_weekdays() {
        assert_eq!(parse_date("2026-10-12").as_deref(), Some("2026-10-12"));
        assert_eq!(parse_date("12/10/2026").as_deref(), Some("2026-10-12"));
        assert_eq!(parse_date("10/25/2026").as_deref(), Some("2026-10-25"));
        assert_eq!(parse_date("2026-02-30"), None);
        assert_eq!(parse_date("hello"), None);
        assert_eq!(parse_number("£1,234.50"), Some(1234.5));
        assert_eq!(parse_number("NaN"), None);
        assert_eq!(fmt_number(2.5), "2.5");
        assert_eq!(fmt_number(3.0), "3");
        assert_eq!(weekday(2026, 10, 12), 0, "a Monday");
        assert_eq!(days_in_month(2024, 2), 29);
    }

    #[test]
    fn vcards_become_contacts() {
        let vcf = "BEGIN:VCARD\r\nVERSION:3.0\r\nN:Lovelace;Ada;;;\r\nFN:Ada Lovelace\r\nORG:Analytical\\, Engines\r\nEMAIL;type=INTERNET:ada@example.org\r\nADR;type=HOME:;;12 St James\\'s Square;London;;SW1Y 4JH;UK\r\nEND:VCARD\r\nBEGIN:VCARD\r\nVERSION:2.1\r\nN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Sm=C3=ADth;Jo=C3=A3o\r\nTEL;CELL:+44 20 7946 0000\r\nEND:VCARD\r\n";
        let t = parse_vcard(vcf);
        assert_eq!(t.len(), 3);
        assert_eq!(t[1][0], "Ada Lovelace");
        assert_eq!(t[1][3], "Analytical, Engines");
        assert_eq!(t[1][7], "London");
        assert_eq!(t[2][0], "João Smíth");
        assert_eq!(t[2][5], "+44 20 7946 0000");
        let db = Database::from_table("Contacts", &t);
        assert_eq!(db.rows.len(), 2);
        let (headers, rows) = db.merge_table(None);
        assert_eq!(headers[1], "First Name");
        assert_eq!(rows[0][4], "ada@example.org");
    }

    #[test]
    fn hostile_input_never_panics() {
        let junk = ["", "\"", ",,,\n\n\"\"\"", "a\tb\n1", "BEGIN:VCARD\nN:;;;;;;;;;\nADR:;\nEND:VCARD", "=\n=Z", "1/2/3", "99999-99-99"];
        for j in junk {
            let _ = parse_csv(j);
            let _ = parse_vcard(j);
            let _ = parse_date(j);
            let _ = parse_number(j);
            let _ = Database::from_table("x", &parse_csv(j));
            let _ = Database::from_json(j.as_bytes());
        }
        let mut db = reading_list();
        db.set_text(999, 999, "x");
        db.delete_prop(db.title_prop().unwrap());
        assert!(db.title_prop().is_some(), "the Title can't be deleted");
        db.move_row(12345, Some(1));
        let _ = db.groups(&[1, 2, 3], 999);
        db.set_kind(db.prop_named("Year").unwrap(), Kind::Text);
        assert_eq!(db.text(db.rows[0].id, db.prop_named("Year").unwrap()), "2008");
    }
}

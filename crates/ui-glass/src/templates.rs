//! The template library: documents, page layouts and databases to start from, grouped like a
//! template chooser's categories. Every template is our own text and design, built in code; the
//! pictures in the layout templates are drawn here too (gradients, dunes, waves, soft lights),
//! so there are no image files and nothing borrowed.
//!
//! Page layout templates place text boxes, shapes and pictures freely on the page (floating,
//! positioned on the page), the way a page layout app works, rather than flowing body text.

use std::sync::Arc;

use wordcraft_doc::para::{Anchor, Float, InlineObject, ShapeKind, Wrap};
use wordcraft_doc::props::{Align, Border, BorderStyle, Borders, CharProps, LineSpacing, Rgb, TabAlign, TabLeader, TabStop, TextColor};
use wordcraft_doc::table::{Cell, Table};
use wordcraft_doc::{Block, Document, Paragraph, PartKind};

use crate::db::{Database, Kind, Layout, Tint};

/// What a template makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Made {
    Document,
    /// A document laid out with floating boxes, shapes and pictures.
    PageLayout,
    Database,
}

/// A template category, in the chooser's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Basic,
    Reports,
    Research,
    Books,
    Letters,
    Cvs,
    Flyers,
    Newsletters,
    Stationery,
    Certificates,
    Databases,
    Mine,
}

impl Category {
    pub const ALL: [Category; 12] = [
        Category::Basic,
        Category::Reports,
        Category::Research,
        Category::Books,
        Category::Letters,
        Category::Cvs,
        Category::Flyers,
        Category::Newsletters,
        Category::Stationery,
        Category::Certificates,
        Category::Databases,
        Category::Mine,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Category::Basic => "Basic",
            Category::Reports => "Reports",
            Category::Research => "Research & Study",
            Category::Books => "Books",
            Category::Letters => "Letters",
            Category::Cvs => "Curricula Vitae",
            Category::Flyers => "Flyers & Posters",
            Category::Newsletters => "Newsletters",
            Category::Stationery => "Stationery",
            Category::Certificates => "Certificates",
            Category::Databases => "Databases",
            Category::Mine => "My Templates",
        }
    }

    /// The glyph beside it in the chooser.
    pub fn icon(self) -> &'static str {
        match self {
            Category::Basic => "doc",
            Category::Reports => "table",
            Category::Research => "cap",
            Category::Books => "book",
            Category::Letters => "pen",
            Category::Cvs => "person",
            Category::Flyers => "layout",
            Category::Newsletters => "list",
            Category::Stationery => "bookmark",
            Category::Certificates => "star",
            Category::Databases => "database",
            Category::Mine => "heart",
        }
    }

    /// A line under the section title, when the category needs one.
    pub fn blurb(self) -> &'static str {
        match self {
            Category::Flyers | Category::Certificates => "Page layouts: text boxes, shapes and pictures placed freely on the page.",
            Category::Databases => "Tables of records with views (Table, Board, Gallery, List, Calendar); every record opens as a page.",
            Category::Research => "For students and researchers: essays, papers, notes, and a thesis chapter.",
            Category::Mine => "Documents and databases you saved as templates.",
            _ => "",
        }
    }
}

/// A built-in template.
#[derive(Clone, Copy, Debug)]
pub struct Template {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub made: Made,
}

const fn t(id: &'static str, name: &'static str, category: Category, made: Made) -> Template {
    Template { id, name, category, made }
}

/// Every built-in template.
pub const ALL: &[Template] = &[
    t("blank", "Blank", Category::Basic, Made::Document),
    t("blank-landscape", "Blank Landscape", Category::Basic, Made::Document),
    t("blank-layout", "Blank Layout", Category::Basic, Made::PageLayout),
    t("notes", "Notes", Category::Basic, Made::Document),
    t("sample", "Studio Handbook", Category::Basic, Made::Document),
    t("report", "Simple Report", Category::Reports, Made::Document),
    t("minimal-report", "Minimal Report", Category::Reports, Made::Document),
    t("photo-report", "Photo Report", Category::Reports, Made::Document),
    t("proposal", "Project Proposal", Category::Reports, Made::Document),
    t("essay", "Essay", Category::Research, Made::Document),
    t("research-paper", "Research Paper", Category::Research, Made::Document),
    t("lab-report", "Lab Report", Category::Research, Made::Document),
    t("lecture-notes", "Lecture Notes", Category::Research, Made::Document),
    t("thesis-chapter", "Thesis Chapter", Category::Research, Made::Document),
    t("novel", "Novel Manuscript", Category::Books, Made::Document),
    t("illustrated-novel", "Illustrated Novel", Category::Books, Made::Document),
    t("screenplay", "Screenplay", Category::Books, Made::Document),
    t("poetry", "Poetry Collection", Category::Books, Made::Document),
    t("letter", "Letter", Category::Letters, Made::Document),
    t("cover-letter", "Cover Letter", Category::Letters, Made::Document),
    t("formal-letter", "Formal Letter", Category::Letters, Made::Document),
    t("resume", "Résumé", Category::Cvs, Made::Document),
    t("modern-cv", "Modern CV", Category::Cvs, Made::Document),
    t("academic-cv", "Academic CV", Category::Cvs, Made::Document),
    t("event-flyer", "Event Flyer", Category::Flyers, Made::PageLayout),
    t("poster", "Poster", Category::Flyers, Made::PageLayout),
    t("sale-flyer", "Bold Flyer", Category::Flyers, Made::PageLayout),
    t("newsletter", "Newsletter", Category::Newsletters, Made::PageLayout),
    t("class-newsletter", "Class Newsletter", Category::Newsletters, Made::Document),
    t("letterhead", "Letterhead", Category::Stationery, Made::Document),
    t("invitation", "Invitation", Category::Stationery, Made::PageLayout),
    t("certificate", "Certificate", Category::Certificates, Made::PageLayout),
    t("award", "Award", Category::Certificates, Made::PageLayout),
    t("db-reading", "Reading List", Category::Databases, Made::Database),
    t("db-literature", "Literature Review", Category::Databases, Made::Database),
    t("db-assignments", "Assignments", Category::Databases, Made::Database),
    t("db-courses", "Course Planner", Category::Databases, Made::Database),
    t("db-research-log", "Research Log", Category::Databases, Made::Database),
    t("db-projects", "Projects", Category::Databases, Made::Database),
    t("db-contacts", "Contacts", Category::Databases, Made::Database),
    t("db-characters", "Characters", Category::Databases, Made::Database),
];

pub fn find(id: &str) -> Option<&'static Template> {
    ALL.iter().find(|t| t.id == id)
}

// ---------------------------------------------------------------------------------------------
// Building blocks.

fn rgb(hex: u32) -> Rgb {
    Rgb(((hex >> 16) & 0xFF) as u8, ((hex >> 8) & 0xFF) as u8, (hex & 0xFF) as u8)
}

fn p(text: &str) -> Paragraph {
    Paragraph::with_text(text, CharProps::default())
}

fn ps(text: &str, style: &str) -> Paragraph {
    p(text).styled(style)
}

/// Character formatting: size, bold, colour, font (any may be left out).
fn cp(size: Option<f32>, bold: bool, color: Option<u32>, font: Option<&str>) -> CharProps {
    CharProps { size, bold: bold.then_some(true), color: color.map(|c| TextColor::Rgb(rgb(c))), font: font.map(str::to_string), ..Default::default() }
}

fn pc(text: &str, props: CharProps) -> Paragraph {
    Paragraph::with_text(text, props)
}

fn centered(mut par: Paragraph) -> Paragraph {
    par.props.align = Some(Align::Center);
    par
}

fn spaced(mut par: Paragraph, before: f32, after: f32) -> Paragraph {
    par.props.space_before = Some(before);
    par.props.space_after = Some(after);
    par
}

fn page_break(mut par: Paragraph) -> Paragraph {
    par.props.page_break_before = Some(true);
    par
}

fn rule_below(mut par: Paragraph, color: u32, width: f32) -> Paragraph {
    par.props.borders =
        Some(Borders { bottom: Some(Border { style: BorderStyle::Single, width, color: Some(rgb(color)), space: 4.0 }), ..Default::default() });
    par
}

fn shaded(mut par: Paragraph, color: u32) -> Paragraph {
    par.props.shading = Some(rgb(color));
    par
}

fn right_tab(mut par: Paragraph, pos: f32) -> Paragraph {
    par.props.tabs = Some(vec![TabStop { pos, align: TabAlign::Right, leader: TabLeader::None }]);
    par
}

fn doc_of(blocks: Vec<Block>) -> Document {
    let mut d = Document::new();
    d.body = blocks.into_iter().map(Arc::new).collect();
    d
}

fn paras(list: Vec<Paragraph>) -> Vec<Block> {
    list.into_iter().map(Block::Para).collect()
}

/// A table of text with column widths; `header` rows are bold and shaded.
fn table(rows: &[&[&str]], widths: &[f32], header: Option<u32>) -> Table {
    let cols = widths.len().max(1);
    let total: f32 = widths.iter().sum();
    let mut tbl = Table::new(rows.len().max(1), cols, total);
    tbl.grid = widths.to_vec();
    for (r, row) in rows.iter().enumerate() {
        for (c, text) in row.iter().enumerate() {
            if let Some(cell) = tbl.rows.get_mut(r).and_then(|rw| rw.cells.get_mut(c)) {
                let props = if r == 0 && header.is_some() { cp(None, true, None, None) } else { CharProps::default() };
                *cell = Cell::with_text("");
                cell.blocks = vec![Arc::new(Block::Para(pc(text, props)))];
                cell.props.width = widths.get(c).copied();
                if r == 0 {
                    cell.props.shading = header.map(rgb);
                }
            }
        }
    }
    tbl
}

/// A floating object's placement on the page, at (x, y) points from the page's top-left.
fn on_page(x: f32, y: f32, wrap: Wrap) -> Float {
    Float { wrap, h_rel: Anchor::Page, v_rel: Anchor::Page, x, y, ..Default::default() }
}

/// A text box placed on the page.
fn text_box(doc: &mut Document, blocks: Vec<Paragraph>, (x, y, w, h): (f32, f32, f32, f32), fill: Option<u32>) -> InlineObject {
    let id = doc.add_part(PartKind::TextBox, blocks.into_iter().map(|p| Arc::new(Block::Para(p))).collect());
    InlineObject::Shape {
        kind: ShapeKind::TextBox,
        w,
        h,
        fill: fill.map(rgb),
        stroke: None,
        stroke_width: 0.0,
        float: on_page(x, y, Wrap::InFrontOfText),
        story: Some(id),
        freeform: None,
        effects: Default::default(),
        extra: Default::default(),
    }
}

/// A shape placed on the page (behind the text when `behind`).
fn shape(kind: ShapeKind, (x, y, w, h): (f32, f32, f32, f32), fill: Option<u32>, stroke: Option<(u32, f32)>, behind: bool) -> InlineObject {
    InlineObject::Shape {
        kind,
        w,
        h,
        fill: fill.map(rgb),
        stroke: stroke.map(|(c, _)| rgb(c)),
        stroke_width: stroke.map(|(_, w)| w).unwrap_or(0.0),
        float: on_page(x, y, if behind { Wrap::BehindText } else { Wrap::InFrontOfText }),
        story: None,
        freeform: None,
        effects: Default::default(),
        extra: Default::default(),
    }
}

/// A drawn picture placed on the page (or inline, when `at` is `None`).
fn picture(
    doc: &mut Document,
    art: Art,
    (px_w, px_h): (usize, usize),
    at: Option<(f32, f32)>,
    (w, h): (f32, f32),
    alt: &str,
) -> Option<InlineObject> {
    let png = wordcraft_ui_egui::paste_picture::png_from_rgba(px_w, px_h, draw(art, px_w, px_h)).ok()?;
    let media = doc.add_media(png, "png");
    let float = match at {
        Some((x, y)) => on_page(x, y, Wrap::BehindText),
        None => Float::default(),
    };
    Some(InlineObject::Image { media, w, h, alt: alt.into(), float, crop: [0.0; 4], ole: None })
}

/// A paragraph holding `objects` (their anchor), with `text` after them.
fn anchored(objects: Vec<InlineObject>, mut par: Paragraph) -> Paragraph {
    for (i, o) in objects.into_iter().enumerate() {
        let _ = par.insert_object(i * wordcraft_doc::para::OBJ.len_utf8(), o, &CharProps::default());
    }
    par
}

// ---------------------------------------------------------------------------------------------
// Pictures drawn in code.

/// A picture style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    /// Warm sky over layered dunes.
    Dunes,
    /// Night blue with soft circles of light.
    Lights,
    /// Layers of sea-blue waves.
    Waves,
    /// A mosaic of triangles in a warm palette.
    Mosaic,
    /// Green hills under a pale sky.
    Hills,
    /// A soft two-colour wash.
    Wash,
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn hexf(h: u32) -> [f32; 3] {
    [((h >> 16) & 0xFF) as f32, ((h >> 8) & 0xFF) as f32, (h & 0xFF) as f32]
}

/// A small deterministic random sequence (xorshift), so pictures are the same every time.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % 10_000) as f32 / 10_000.0
    }
}

/// Draw `art` as `w` × `h` RGBA pixels.
pub fn draw(art: Art, w: usize, h: usize) -> Vec<u8> {
    let (w, h) = (w.clamp(1, 4096), h.clamp(1, 4096));
    let mut px = vec![0u8; w * h * 4];
    let put = |px: &mut Vec<u8>, x: usize, y: usize, c: [f32; 3]| {
        let i = (y * w + x) * 4;
        if let Some(s) = px.get_mut(i..i + 4) {
            s.copy_from_slice(&[c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8, 255]);
        }
    };
    let (wf, hf) = (w as f32, h as f32);
    match art {
        Art::Dunes | Art::Hills | Art::Waves => {
            let (sky_top, sky_low, layers): ([f32; 3], [f32; 3], [u32; 4]) = match art {
                Art::Dunes => (hexf(0xF6C89F), hexf(0xFBE8D3), [0xE9A66B, 0xD9874E, 0xC46A38, 0xA4532A]),
                Art::Hills => (hexf(0xCFE6F2), hexf(0xF2F7EE), [0xA9CF9A, 0x82B577, 0x5F9A5A, 0x427E46]),
                _ => (hexf(0xBFE3F0), hexf(0xE8F5FA), [0x7CC3DB, 0x4FA6C6, 0x2F86AE, 0x1C6A93]),
            };
            for y in 0..h {
                for x in 0..w {
                    let (xf, yf) = (x as f32 / wf, y as f32 / hf);
                    let mut c = mix(sky_top, sky_low, yf * 1.4);
                    for (k, layer) in layers.iter().enumerate() {
                        let k = k as f32;
                        let base = 0.42 + k * 0.14;
                        let amp = if art == Art::Waves { 0.035 } else { 0.07 - k * 0.008 };
                        let freq = if art == Art::Waves { 9.0 + k * 3.0 } else { 2.2 + k * 0.9 };
                        let edge = base + amp * ((xf * freq + k * 1.7).sin() + 0.5 * (xf * freq * 2.3 + k).sin());
                        if yf > edge {
                            let shade = ((yf - edge) * 1.6).clamp(0.0, 0.35);
                            c = mix(hexf(*layer), [0.0, 0.0, 0.0], shade * 0.5);
                        }
                    }
                    put(&mut px, x, y, c);
                }
            }
        }
        Art::Lights | Art::Wash => {
            let (a, b) = if art == Art::Lights { (hexf(0x0E1A3A), hexf(0x3A1E5C)) } else { (hexf(0xF3D9E6), hexf(0xD6E4F7)) };
            for y in 0..h {
                for x in 0..w {
                    let t = (x as f32 / wf * 0.6 + y as f32 / hf * 0.4).clamp(0.0, 1.0);
                    put(&mut px, x, y, mix(a, b, t));
                }
            }
            if art == Art::Lights {
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
                let colors = [0xFFC46B, 0xFF7E9D, 0x7FD8FF, 0xFFF1C9, 0xB28DFF];
                for _ in 0..34 {
                    let cx = rng.next() * wf;
                    let cy = rng.next() * hf;
                    let r = (0.03 + rng.next() * 0.09) * wf.min(hf) * 1.6;
                    let col = hexf(colors[(rng.next() * colors.len() as f32) as usize % colors.len()]);
                    let alpha = 0.18 + rng.next() * 0.3;
                    let (x0, x1) = ((cx - r).max(0.0) as usize, ((cx + r) as usize).min(w.saturating_sub(1)));
                    let (y0, y1) = ((cy - r).max(0.0) as usize, ((cy + r) as usize).min(h.saturating_sub(1)));
                    for y in y0..=y1 {
                        for x in x0..=x1 {
                            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / r;
                            if d < 1.0 {
                                let i = (y * w + x) * 4;
                                let Some(s) = px.get(i..i + 3) else { continue };
                                let cur = [f32::from(s[0]), f32::from(s[1]), f32::from(s[2])];
                                let a = alpha * (1.0 - d.powi(4));
                                put(&mut px, x, y, mix(cur, col, a));
                            }
                        }
                    }
                }
            }
        }
        Art::Mosaic => {
            let palette = [0xF2B880, 0xE07A5F, 0x3D405B, 0x81B29A, 0xF4F1DE, 0xC9ADA7];
            let cell = (wf / 9.0).max(8.0);
            let mut rng = Rng(0x2545_F491_4F6C_DD1D);
            let cols = (wf / cell).ceil() as usize + 1;
            let rows = (hf / cell).ceil() as usize + 1;
            let mut colors = Vec::with_capacity(cols * rows * 2);
            for _ in 0..cols * rows * 2 {
                colors.push(hexf(palette[(rng.next() * palette.len() as f32) as usize % palette.len()]));
            }
            for y in 0..h {
                for x in 0..w {
                    let (gx, gy) = ((x as f32 / cell) as usize, (y as f32 / cell) as usize);
                    let (fx, fy) = (x as f32 / cell - gx as f32, y as f32 / cell - gy as f32);
                    let upper = if (gx + gy) % 2 == 0 { fx + fy < 1.0 } else { fx > fy };
                    let k = (gy * cols + gx) * 2 + usize::from(upper);
                    put(&mut px, x, y, colors.get(k).copied().unwrap_or([200.0, 200.0, 200.0]));
                }
            }
        }
    }
    px
}

// ---------------------------------------------------------------------------------------------
// Documents.

/// Build template `id` as a document (`None` for a database template or an unknown id).
pub fn document(id: &str) -> Option<Document> {
    let d = match id {
        "blank" => Document::new(),
        "blank-landscape" => {
            let mut d = Document::new();
            let s = &mut d.last_section;
            std::mem::swap(&mut s.page_w, &mut s.page_h);
            s.landscape = true;
            d
        }
        "blank-layout" => blank_layout(),
        "sample" => wordcraft_engine::sample::sample_document(),
        "letter" => wordcraft_engine::sample::letter(),
        "resume" => wordcraft_engine::sample::resume(),
        "report" => wordcraft_engine::sample::report(),
        "notes" => notes(),
        "minimal-report" => minimal_report(),
        "photo-report" => photo_report(),
        "proposal" => proposal(),
        "essay" => essay(),
        "research-paper" => research_paper(),
        "lab-report" => lab_report(),
        "lecture-notes" => lecture_notes(),
        "thesis-chapter" => thesis_chapter(),
        "novel" => novel(false),
        "illustrated-novel" => novel(true),
        "screenplay" => screenplay(),
        "poetry" => poetry(),
        "cover-letter" => cover_letter(),
        "formal-letter" => formal_letter(),
        "modern-cv" => modern_cv(),
        "academic-cv" => academic_cv(),
        "event-flyer" => event_flyer(),
        "poster" => poster(),
        "sale-flyer" => bold_flyer(),
        "newsletter" => newsletter(),
        "class-newsletter" => class_newsletter(),
        "letterhead" => letterhead(),
        "invitation" => invitation(),
        "certificate" => certificate(false),
        "award" => certificate(true),
        _ => return None,
    };
    Some(d)
}

fn blank_layout() -> Document {
    let mut d = Document::new();
    d.last_section.margin_top = 36.0;
    d.last_section.margin_bottom = 36.0;
    d.last_section.margin_left = 36.0;
    d.last_section.margin_right = 36.0;
    d
}

fn notes() -> Document {
    let today = wordcraft_engine::cmd::insert::format_date("MMMM d, yyyy");
    doc_of(paras(vec![
        ps("Notes", "Title"),
        pc(&today, cp(Some(11.0), false, Some(0x7A7A7A), None)),
        ps("Ideas", "Heading2"),
        p("Write a thought on each line. Press Tab to nest one under another."),
        ps("To do", "Heading2"),
        p("Turn a line into a to-do with the block handle or by typing /todo."),
        ps("Questions", "Heading2"),
        p(""),
    ]))
}

fn minimal_report() -> Document {
    let gray = Some(0x6B6B6B);
    doc_of(paras(vec![
        pc("ANNUAL REVIEW", cp(Some(10.0), true, Some(0x9A9A9A), None)),
        spaced(pc("Quiet Growth", cp(Some(40.0), false, Some(0x1F1F1F), Some("Georgia"))), 120.0, 6.0),
        rule_below(pc("What changed this year, and what we learned from it", cp(Some(14.0), false, gray, None)), 0x1F1F1F, 1.5),
        spaced(ps("Overview", "Heading1"), 60.0, 6.0),
        p("Open with the single most important result, in plain words. A reader who stops after this paragraph should still know what happened."),
        ps("Highlights", "Heading1"),
        p("Three or four short paragraphs work better than a long list. Give each one a number the reader can remember."),
        ps("Next year", "Heading1"),
        p("End with what you will do differently, and how you will know it worked."),
    ]))
}

fn photo_report() -> Document {
    let mut d = Document::new();
    let img = picture(&mut d, Art::Waves, (1200, 640), None, (468.0, 250.0), "Layered blue waves");
    let mut blocks = vec![
        Block::Para(pc("FIELD REPORT · COASTAL SURVEY", cp(Some(10.0), true, Some(0x2F86AE), None))),
        Block::Para(spaced(pc("Tides of the North Bay", cp(Some(30.0), true, Some(0x14324A), None)), 4.0, 12.0)),
    ];
    if let Some(i) = img {
        blocks.push(Block::Para(anchored(vec![i], p(""))));
    }
    blocks.extend(paras(vec![
        pc("Above: the bay at low water, drawn from the survey's sketches.", cp(Some(9.5), false, Some(0x7A7A7A), None)),
        ps("Summary", "Heading1"),
        p("Describe where you went, what you measured and the one thing that surprised you."),
        ps("Method", "Heading1"),
        p("Explain how the observations were made, so someone else could repeat them."),
        ps("Findings", "Heading1"),
        p("Put the evidence first and the interpretation second."),
    ]));
    let mut d2 = doc_of(blocks);
    d2.media = d.media;
    d2
}

fn proposal() -> Document {
    let mut blocks = paras(vec![
        ps("Project Proposal", "Title"),
        ps("A community tool library for the east side", "Subtitle"),
        ps("The problem", "Heading1"),
        p("State the need in one paragraph, with one number that shows its size."),
        ps("What we propose", "Heading1"),
        p("Describe the project as the people who will use it will see it."),
        ps("Plan and budget", "Heading1"),
    ]);
    blocks.push(Block::Table(table(
        &[
            &["Phase", "When", "Cost"],
            &["Find a space", "Months 1–2", "£1,200"],
            &["Collect tools", "Months 2–4", "£2,500"],
            &["Open and run", "Months 5–12", "£4,800"],
        ],
        &[230.0, 130.0, 108.0],
        Some(0xDCE8F2),
    )));
    blocks.extend(paras(vec![ps("How we will measure success", "Heading1"), p("Name two or three things you will count, and the target for each.")]));
    doc_of(blocks)
}

fn essay() -> Document {
    let mut body = vec![
        spaced(centered(pc("The Ethics of Attention", cp(Some(26.0), false, None, Some("Georgia")))), 200.0, 18.0),
        centered(p("Your Name")),
        centered(p("Course Name and Number")),
        centered(p("Instructor")),
        centered(p(&wordcraft_engine::cmd::insert::format_date("d MMMM yyyy"))),
        page_break(ps("Introduction", "Heading1")),
    ];
    let mut para = p(
        "Begin with the question your essay answers, then say in one sentence what you will argue. That sentence is your thesis; everything after it should support, test or qualify it.",
    );
    para.props.line_spacing = Some(LineSpacing::Multiple(2.0));
    para.props.indent_first = Some(36.0);
    body.push(para);
    body.push(ps("Argument", "Heading1"));
    let mut para = p(
        "Give each paragraph one job. Start it with a claim, support the claim with evidence, and end by showing how it moves the argument forward.",
    );
    para.props.line_spacing = Some(LineSpacing::Multiple(2.0));
    para.props.indent_first = Some(36.0);
    body.push(para);
    body.push(ps("Conclusion", "Heading1"));
    body.push(p("Return to the question and show what your reader now understands that they didn't at the start."));
    body.push(page_break(centered(ps("Works Cited", "Heading1"))));
    body.push(p("Author, A. (Year). Title of the work. Publisher."));
    doc_of(paras(body))
}

fn research_paper() -> Document {
    let mut blocks = paras(vec![
        centered(pc("Effects of Shade on Urban Soil Moisture", cp(Some(20.0), true, None, None))),
        centered(p("First Author¹, Second Author²")),
        centered(pc("¹ Department, University · ² Institute · corresponding@example.org", cp(Some(9.5), false, Some(0x6B6B6B), None))),
        spaced(pc("Abstract", cp(Some(11.0), true, None, None)), 18.0, 4.0),
        pc("In 150–250 words: the question, the method, the main result with its size, and why it matters.", cp(Some(10.0), false, None, None)),
        pc("Keywords: urban ecology; soil moisture; shade; field study", cp(Some(10.0), false, Some(0x6B6B6B), None)),
        ps("1. Introduction", "Heading1"),
        p("What is known, what is not, and the gap this study fills."),
        ps("2. Methods", "Heading1"),
        p("Sites, sampling, instruments and analysis, in enough detail to reproduce."),
        ps("3. Results", "Heading1"),
    ]);
    blocks.push(Block::Table(table(
        &[&["Site", "Shade (%)", "Moisture (%)"], &["A", "12", "18.4"], &["B", "47", "24.9"], &["C", "81", "31.2"]],
        &[150.0, 150.0, 150.0],
        Some(0xEDEDED),
    )));
    blocks.extend(paras(vec![
        pc("Table 1. Mean soil moisture by site.", cp(Some(9.5), false, Some(0x6B6B6B), None)),
        ps("4. Discussion", "Heading1"),
        p("Interpret the results, compare them with earlier work, and state the limits honestly."),
        ps("References", "Heading1"),
        p("Use References › Insert Citation to cite sources, then Bibliography to list them here."),
    ]));
    doc_of(blocks)
}

fn lab_report() -> Document {
    let mut blocks = paras(vec![
        ps("Lab Report: Pendulum Period", "Title"),
        p("Name · Lab partner · Section · Date"),
        ps("Aim", "Heading1"),
        p("To test whether a pendulum's period depends on its length but not its mass."),
        ps("Hypothesis", "Heading1"),
        p("The period grows with the square root of the length."),
        ps("Materials", "Heading1"),
        p("String, three masses, a stand, a stopwatch, a metre rule."),
        ps("Results", "Heading1"),
    ]);
    blocks.push(Block::Table(table(
        &[&["Length (m)", "Time for 10 swings (s)", "Period (s)"], &["0.25", "10.1", "1.01"], &["0.50", "14.2", "1.42"], &["1.00", "20.1", "2.01"]],
        &[140.0, 190.0, 138.0],
        Some(0xE4EFE7),
    )));
    blocks.extend(paras(vec![
        ps("Discussion", "Heading1"),
        p("Compare the results with the hypothesis and list the sources of error."),
        ps("Conclusion", "Heading1"),
        p(""),
    ]));
    doc_of(blocks)
}

fn lecture_notes() -> Document {
    // Cornell notes: cues on the left, notes on the right, a summary below.
    let mut tbl = table(
        &[
            &["Cues", "Notes"],
            &["Key question?", "Write notes here during the lecture."],
            &["Term", "Definition in your own words."],
            &["Example", "A worked example or diagram."],
        ],
        &[140.0, 328.0],
        Some(0xF1ECE2),
    );
    tbl.props.borders = Some(Borders {
        inside_v: Some(Border { style: BorderStyle::Single, width: 1.0, color: Some(rgb(0xC9BFAE)), space: 0.0 }),
        between: Some(Border { style: BorderStyle::Dotted, width: 0.5, color: Some(rgb(0xC9BFAE)), space: 0.0 }),
        ..Default::default()
    });
    let mut blocks = paras(vec![
        pc("Course · Lecture 4", cp(Some(10.0), true, Some(0x8C7B62), None)),
        rule_below(pc("Topic of today's lecture", cp(Some(24.0), true, None, None)), 0xC9BFAE, 1.0),
        p(""),
    ]);
    blocks.push(Block::Table(tbl));
    blocks.extend(paras(vec![
        spaced(ps("Summary", "Heading2"), 18.0, 4.0),
        p("After the lecture, sum it up in three sentences. Cover the right column and test yourself from the cues."),
    ]));
    doc_of(blocks)
}

fn thesis_chapter() -> Document {
    doc_of(paras(vec![
        pc("CHAPTER 2", cp(Some(11.0), true, Some(0x6B6B6B), None)),
        spaced(pc("Literature Review", cp(Some(28.0), false, None, Some("Georgia"))), 6.0, 24.0),
        ps("2.1 Scope of the review", "Heading2"),
        p("Explain which fields, years and kinds of sources the review covers, and why."),
        ps("2.2 Themes in the literature", "Heading2"),
        p("Organise by theme rather than by author. For each theme, say where the studies agree and where they disagree."),
        ps("2.3 Gaps and the present study", "Heading2"),
        p("End by showing the gap your research addresses."),
        ps("2.4 Chapter summary", "Heading2"),
        p(""),
    ]))
}

fn novel(illustrated: bool) -> Document {
    let mut d = Document::new();
    let mut blocks: Vec<Block> = Vec::new();
    if illustrated && let Some(img) = picture(&mut d, Art::Dunes, (900, 1200), Some((0.0, 0.0)), (612.0, 792.0), "Dunes under a warm sky") {
        let title = text_box(
            &mut d,
            vec![
                centered(pc("THE LONG SAND", cp(Some(40.0), true, Some(0xFFFFFF), Some("Georgia")))),
                centered(pc("a novel", cp(Some(16.0), false, Some(0xFFF6EA), Some("Georgia")))),
            ],
            (60.0, 120.0, 492.0, 140.0),
            None,
        );
        let author =
            text_box(&mut d, vec![centered(pc("Author Name", cp(Some(16.0), true, Some(0xFFFFFF), None)))], (60.0, 690.0, 492.0, 40.0), None);
        blocks.push(Block::Para(anchored(vec![img, title, author], p(""))));
        blocks.push(Block::Para(page_break(p(""))));
    } else {
        blocks.extend(paras(vec![
            right_tab(p("Your Name\tAbout 80,000 words"), 468.0),
            p("Address · email · phone"),
            spaced(centered(pc("THE TITLE OF YOUR NOVEL", cp(Some(18.0), true, None, None))), 220.0, 6.0),
            centered(p("by Your Name")),
        ]));
    }
    let mut ch = spaced(centered(pc("Chapter One", cp(Some(16.0), true, None, Some("Georgia")))), 140.0, 30.0).styled("Heading1");
    ch.props.page_break_before = Some(true);
    blocks.push(Block::Para(ch));
    for text in [
        "The first line should make the reader ask a question. This template sets the text the way manuscripts are usually sent: double-spaced, each paragraph indented, a new page for each chapter.",
        "Use the Binder in Writer Pro to see your chapters as a tree, and the Corkboard to move whole chapters around.",
    ] {
        let mut par = p(text);
        par.props.line_spacing = Some(LineSpacing::Multiple(2.0));
        par.props.indent_first = Some(36.0);
        blocks.push(Block::Para(par));
    }
    let mut out = doc_of(blocks);
    out.parts.append(&mut d.parts);
    out.media = d.media;
    out
}

fn screenplay() -> Document {
    let courier = |text: &str| pc(text, cp(Some(12.0), false, None, Some("Courier New")));
    let mut scene = courier("INT. LIGHTHOUSE KITCHEN - NIGHT");
    scene.props.space_before = Some(24.0);
    let mut character = courier("MARA");
    character.props.indent_left = Some(158.0);
    character.props.space_before = Some(12.0);
    character.props.space_after = Some(0.0);
    let mut dialogue = courier("If the lamp goes out tonight, we'll know before the ships do.");
    dialogue.props.indent_left = Some(72.0);
    dialogue.props.indent_right = Some(108.0);
    let mut paren = courier("(quietly)");
    paren.props.indent_left = Some(115.0);
    paren.props.space_after = Some(0.0);
    let mut transition = courier("CUT TO:");
    transition.props.align = Some(Align::Right);
    transition.props.space_before = Some(12.0);
    doc_of(paras(vec![
        spaced(centered(pc("THE KEEPER", cp(Some(14.0), true, None, Some("Courier New")))), 240.0, 12.0),
        centered(courier("Written by")),
        centered(courier("Your Name")),
        page_break(courier("FADE IN:")),
        scene,
        courier("A narrow kitchen at the foot of the tower. Rain on the window. MARA (40s) counts matches into a tin."),
        character,
        paren,
        dialogue,
        transition,
    ]))
}

fn poetry() -> Document {
    let verse = |text: &str| spaced(pc(text, cp(Some(12.0), false, None, Some("Georgia"))), 0.0, 0.0);
    doc_of(paras(vec![
        spaced(centered(pc("Small Weathers", cp(Some(30.0), false, None, Some("Georgia")))), 200.0, 6.0),
        centered(pc("poems", cp(Some(13.0), false, Some(0x7A7A7A), Some("Georgia")))),
        page_break(spaced(pc("Morning, Harbour", cp(Some(16.0), true, None, Some("Georgia"))), 80.0, 18.0)),
        verse("The boats are counting themselves again,"),
        verse("one bell for each that came home."),
        verse(""),
        verse("Write your lines here: Enter starts a new line,"),
        verse("and an empty line begins a new stanza."),
    ]))
}

fn cover_letter() -> Document {
    doc_of(paras(vec![
        pc("Your Name", cp(Some(22.0), true, None, None)),
        rule_below(pc("City · email@example.com · phone · portfolio.example.com", cp(Some(10.0), false, Some(0x6B6B6B), None)), 0x2F5D8A, 1.5),
        spaced(p(&wordcraft_engine::cmd::insert::format_date("d MMMM yyyy")), 18.0, 12.0),
        p("Hiring Manager"),
        p("Company"),
        spaced(p("Dear Hiring Manager,"), 18.0, 12.0),
        p("In the first paragraph, name the role and say in one sentence why you are a strong fit."),
        p("In the second, give one example of work you did that matches what the role needs, with a result someone could measure."),
        p("Close by saying what you would like to happen next, and thank the reader."),
        spaced(p("Kind regards,"), 12.0, 36.0),
        p("Your Name"),
    ]))
}

fn formal_letter() -> Document {
    doc_of(paras(vec![
        right_tab(pc("ORGANISATION NAME\tReference: 2026/014", cp(Some(11.0), true, Some(0x14324A), None)), 468.0),
        rule_below(pc("Street Address · City · Postcode · www.example.org", cp(Some(9.5), false, Some(0x6B6B6B), None)), 0x14324A, 0.75),
        spaced(p(&wordcraft_engine::cmd::insert::format_date("d MMMM yyyy")), 24.0, 12.0),
        p("Recipient Name"),
        p("Title"),
        p("Address"),
        spaced(pc("Subject: The matter this letter concerns", cp(None, true, None, None)), 18.0, 12.0),
        p("Dear Recipient,"),
        p("State the purpose of the letter in the first sentence. Keep each paragraph to one point."),
        p("Yours sincerely,"),
        spaced(p("Name"), 36.0, 0.0),
        p("Position"),
    ]))
}

fn modern_cv() -> Document {
    // A two-column layout: a tinted sidebar beside the main column.
    let side = 0xEAF1F7;
    let mut tbl = Table::new(1, 2, 504.0);
    tbl.grid = vec![170.0, 334.0];
    tbl.props.borders = Some(Borders::default());
    if let Some(row) = tbl.rows.get_mut(0) {
        if let Some(c) = row.cells.get_mut(0) {
            c.props.shading = Some(rgb(side));
            c.props.width = Some(170.0);
            c.props.margins = Some([14.0, 12.0, 14.0, 12.0]);
            c.blocks = paras(vec![
                pc("Contact", cp(Some(11.0), true, Some(0x2F5D8A), None)),
                p("you@example.com"),
                p("+44 0000 000000"),
                p("City, Country"),
                spaced(pc("Skills", cp(Some(11.0), true, Some(0x2F5D8A), None)), 14.0, 2.0),
                p("Python · SQL · Data pipelines · Machine learning · Visualisation"),
                spaced(pc("Languages", cp(Some(11.0), true, Some(0x2F5D8A), None)), 14.0, 2.0),
                p("English · Portuguese · Spanish"),
            ])
            .into_iter()
            .map(Arc::new)
            .collect();
        }
        if let Some(c) = row.cells.get_mut(1) {
            c.props.width = Some(334.0);
            c.props.margins = Some([14.0, 18.0, 14.0, 6.0]);
            c.blocks = paras(vec![
                pc("Alex Morgan", cp(Some(28.0), true, Some(0x14324A), None)),
                pc("Data Engineer", cp(Some(13.0), false, Some(0x2F5D8A), None)),
                spaced(rule_below(pc("Experience", cp(Some(12.0), true, None, None)), 0xB8C7D6, 0.75), 16.0, 6.0),
                right_tab(pc("Data Engineer, Harbour Analytics\t2023 – now", cp(None, true, None, None)), 300.0),
                p("Built the pipelines behind the company's daily reporting."),
                right_tab(pc("Analyst, City Library Service\t2020 – 2023", cp(None, true, None, None)), 300.0),
                p("Turned visitor data into a dashboard used by every branch."),
                spaced(rule_below(pc("Education", cp(Some(12.0), true, None, None)), 0xB8C7D6, 0.75), 16.0, 6.0),
                right_tab(p("MSc Artificial Intelligence\t2025"), 300.0),
                right_tab(p("BSc Information Technology\t2020"), 300.0),
            ])
            .into_iter()
            .map(Arc::new)
            .collect();
        }
    }
    let mut d = doc_of(vec![Block::Table(tbl)]);
    d.last_section.margin_left = 54.0;
    d.last_section.margin_right = 54.0;
    d
}

fn academic_cv() -> Document {
    let head = |s: &str| spaced(rule_below(pc(s, cp(Some(12.0), true, None, Some("Georgia"))), 0x9A9A9A, 0.5), 14.0, 6.0);
    doc_of(paras(vec![
        centered(pc("Dr Sam Okafor", cp(Some(24.0), false, None, Some("Georgia")))),
        centered(pc("Department of Ecology · University · s.okafor@example.ac.uk", cp(Some(10.0), false, Some(0x6B6B6B), None))),
        head("Research interests"),
        p("Urban ecology; soil–plant interactions; citizen science."),
        head("Education"),
        right_tab(p("PhD Ecology, University of Somewhere\t2021"), 468.0),
        right_tab(p("MSc Environmental Science\t2017"), 468.0),
        head("Publications"),
        p("Okafor, S. & Lee, J. (2025). Shade and soil moisture in city parks. Journal of Urban Ecology, 11(2), 1–14."),
        head("Teaching"),
        p("Introduction to Ecology (lecturer, 2022–now)."),
        head("Grants and awards"),
        p("Early Career Fellowship, 2024."),
    ]))
}

// ---------------------------------------------------------------------------------------------
// Page layouts.

fn layout_doc(objects: impl FnOnce(&mut Document) -> Vec<InlineObject>) -> Document {
    let mut d = blank_layout();
    let objs = objects(&mut d);
    d.body = vec![Arc::new(Block::Para(anchored(objs, p(""))))];
    d
}

fn event_flyer() -> Document {
    layout_doc(|d| {
        let mut o = Vec::new();
        o.extend(picture(d, Art::Dunes, (1200, 760), Some((0.0, 0.0)), (612.0, 388.0), "Dunes at sunset"));
        o.push(shape(ShapeKind::Rectangle, (0.0, 388.0, 612.0, 404.0), Some(0xFFF8F0), None, true));
        o.push(text_box(
            d,
            vec![
                pc("SATURDAY 14 JUNE · 6 PM", cp(Some(12.0), true, Some(0xC46A38), None)),
                spaced(pc("Night Market", cp(Some(54.0), true, Some(0x3A2216), Some("Georgia"))), 4.0, 6.0),
                pc("Food, music and makers on the old harbour wall.", cp(Some(16.0), false, Some(0x5A4030), None)),
            ],
            (48.0, 412.0, 516.0, 170.0),
            None,
        ));
        o.push(text_box(
            d,
            vec![
                pc("WHERE", cp(Some(10.0), true, Some(0xC46A38), None)),
                pc("Harbour Wall, East Quay", cp(Some(13.0), false, Some(0x3A2216), None)),
                spaced(pc("ENTRY", cp(Some(10.0), true, Some(0xC46A38), None)), 10.0, 0.0),
                pc("Free · all ages", cp(Some(13.0), false, Some(0x3A2216), None)),
            ],
            (48.0, 600.0, 240.0, 120.0),
            None,
        ));
        o.push(text_box(
            d,
            vec![pc(
                "Thirty stalls, two stages, and lanterns along the water. Bring a friend and an appetite.",
                cp(Some(12.0), false, Some(0x5A4030), None),
            )],
            (318.0, 600.0, 246.0, 120.0),
            None,
        ));
        o.push(shape(ShapeKind::Rectangle, (0.0, 752.0, 612.0, 40.0), Some(0xC46A38), None, true));
        o.push(text_box(
            d,
            vec![centered(pc("nightmarket.example.org", cp(Some(11.0), true, Some(0xFFFFFF), None)))],
            (0.0, 760.0, 612.0, 26.0),
            None,
        ));
        o
    })
}

fn poster() -> Document {
    layout_doc(|d| {
        let mut o = Vec::new();
        o.extend(picture(d, Art::Lights, (900, 1170), Some((0.0, 0.0)), (612.0, 792.0), "Soft circles of light on a night-blue background"));
        o.push(text_box(
            d,
            vec![
                pc("LIVE AT THE ROTUNDA", cp(Some(13.0), true, Some(0xFFC46B), None)),
                spaced(pc("Light", cp(Some(96.0), true, Some(0xFFFFFF), Some("Georgia"))), 0.0, 0.0),
                pc("& Sound", cp(Some(72.0), false, Some(0xFFFFFF), Some("Georgia"))),
            ],
            (48.0, 96.0, 516.0, 300.0),
            None,
        ));
        o.push(shape(ShapeKind::Line, (52.0, 470.0, 200.0, 1.0), None, Some((0xFFC46B, 2.0)), false));
        o.push(text_box(
            d,
            vec![
                pc("An evening of film, music and projected light.", cp(Some(16.0), false, Some(0xEDE7FF), None)),
                spaced(pc("FRIDAY 3 OCTOBER · DOORS 7 PM", cp(Some(13.0), true, Some(0xFFFFFF), None)), 14.0, 0.0),
            ],
            (48.0, 490.0, 420.0, 110.0),
            None,
        ));
        o.push(text_box(d, vec![pc("Tickets: rotunda.example.org", cp(Some(12.0), false, Some(0xBFB6E0), None))], (48.0, 720.0, 420.0, 30.0), None));
        o
    })
}

fn bold_flyer() -> Document {
    layout_doc(|d| {
        let mut o = Vec::new();
        o.push(shape(ShapeKind::Rectangle, (0.0, 0.0, 612.0, 792.0), Some(0xF4F1DE), None, true));
        o.extend(picture(d, Art::Mosaic, (900, 600), Some((36.0, 36.0)), (540.0, 300.0), "A mosaic of warm triangles"));
        o.push(shape(ShapeKind::Ellipse, (430.0, 270.0, 140.0, 140.0), Some(0xE07A5F), None, false));
        o.push(text_box(
            d,
            vec![centered(pc("40%", cp(Some(40.0), true, Some(0xFFFFFF), None))), centered(pc("OFF", cp(Some(16.0), true, Some(0xFFFFFF), None)))],
            (440.0, 300.0, 120.0, 80.0),
            None,
        ));
        o.push(text_box(
            d,
            vec![
                pc("Spring Clear-Out", cp(Some(48.0), true, Some(0x3D405B), None)),
                spaced(pc("Everything in the studio shop, one weekend only.", cp(Some(18.0), false, Some(0x3D405B), None)), 6.0, 18.0),
                pc("Saturday and Sunday, 10 am to 4 pm · 22 Mill Lane", cp(Some(13.0), true, Some(0xE07A5F), None)),
            ],
            (36.0, 420.0, 540.0, 220.0),
            None,
        ));
        o.push(shape(ShapeKind::Rectangle, (36.0, 700.0, 540.0, 2.0), Some(0x3D405B), None, false));
        o.push(text_box(d, vec![pc("studio-shop.example.org", cp(Some(12.0), false, Some(0x3D405B), None))], (36.0, 712.0, 540.0, 30.0), None));
        o
    })
}

fn newsletter() -> Document {
    let mut d = Document::new();
    let band = shape(ShapeKind::Rectangle, (0.0, 0.0, 612.0, 150.0), Some(0x1C6A93), None, true);
    let art = picture(&mut d, Art::Waves, (1200, 300), Some((0.0, 150.0)), (612.0, 60.0), "A strip of waves");
    let mast = text_box(
        &mut d,
        vec![
            pc("THE TIDE LINE", cp(Some(40.0), true, Some(0xFFFFFF), Some("Georgia"))),
            pc("News from the North Bay community · Issue 12 · Autumn", cp(Some(12.0), false, Some(0xD6ECF5), None)),
        ],
        (54.0, 40.0, 504.0, 100.0),
        None,
    );
    let mut objs = vec![band, mast];
    objs.extend(art);
    let mut first = anchored(objs, p(""));
    first.props.space_before = Some(150.0);
    let mut blocks = vec![Block::Para(first)];
    blocks.extend(paras(vec![
        ps("A new home for the boat club", "Heading1"),
        p("Lead with the story most readers care about. Newsletters are skimmed, so put the news in the first sentence and the detail after it."),
        p("This page is laid out in two columns. Text flows from the bottom of the first into the top of the second."),
        ps("Beach clean, 4 October", "Heading2"),
        p("Meet at the slipway at 9. Gloves and bags are provided; bring water."),
        ps("From the archive", "Heading2"),
        p("A short piece of local history, or a photo with a caption, gives readers a reason to keep the issue."),
        ps("Dates for your diary", "Heading2"),
        p("List upcoming events with day, time and place on one line each."),
    ]));
    let mut out = doc_of(blocks);
    out.parts.append(&mut d.parts);
    out.media = d.media;
    out.last_section.columns = wordcraft_doc::section::Columns { count: 2, space: 24.0, separator: true, widths: Vec::new() };
    out.last_section.margin_top = 36.0;
    out
}

fn class_newsletter() -> Document {
    doc_of(paras(vec![
        shaded(centered(pc("Year 9 Science", cp(Some(28.0), true, Some(0x14324A), None))), 0xE4EFE7),
        centered(pc("Weekly update · Week 6", cp(Some(12.0), false, Some(0x44836A), None))),
        ps("This week", "Heading1"),
        p("What the class studied, in two or three sentences parents can follow."),
        ps("Homework", "Heading1"),
        p("Due Friday: read pages 40–44 and answer questions 1 to 5."),
        ps("Coming up", "Heading1"),
        p("Field trip to the botanic garden on the 21st: permission slips by the 14th."),
    ]))
}

fn letterhead() -> Document {
    let mut d = doc_of(paras(vec![
        right_tab(pc("Harbour Studio\tharbour.example.org", cp(Some(16.0), true, Some(0x1C6A93), None)), 468.0),
        rule_below(pc("12 Quay Street · Port Ellen · +44 0000 000000", cp(Some(9.5), false, Some(0x6B6B6B), None)), 0x1C6A93, 2.0),
        spaced(p(""), 24.0, 0.0),
        p("Start your letter here."),
    ]));
    d.last_section.margin_top = 54.0;
    d
}

fn invitation() -> Document {
    layout_doc(|d| {
        let mut o = Vec::new();
        o.extend(picture(d, Art::Wash, (800, 1040), Some((0.0, 0.0)), (612.0, 792.0), "A soft pink and blue wash"));
        o.push(shape(ShapeKind::Rectangle, (60.0, 60.0, 492.0, 672.0), None, Some((0xB08CA0, 1.0)), false));
        o.push(text_box(
            d,
            vec![
                centered(pc("You are invited to", cp(Some(16.0), false, Some(0x7A5A6E), Some("Georgia")))),
                spaced(centered(pc("Ana & Rui's", cp(Some(44.0), false, Some(0x4A3442), Some("Georgia")))), 18.0, 0.0),
                centered(pc("Garden Party", cp(Some(44.0), false, Some(0x4A3442), Some("Georgia")))),
                spaced(centered(pc("SATURDAY 20 SEPTEMBER · 3 PM", cp(Some(13.0), true, Some(0x7A5A6E), None))), 30.0, 4.0),
                centered(pc("The Walled Garden, Rose Lane", cp(Some(13.0), false, Some(0x7A5A6E), None))),
                spaced(centered(pc("Kindly reply by 1 September", cp(Some(11.0), false, Some(0x9A7F8E), Some("Georgia")))), 40.0, 0.0),
            ],
            (90.0, 200.0, 432.0, 400.0),
            None,
        ));
        o
    })
}

fn certificate(award: bool) -> Document {
    let mut d = blank_layout();
    std::mem::swap(&mut d.last_section.page_w, &mut d.last_section.page_h);
    d.last_section.landscape = true;
    let (w, h) = (792.0, 612.0);
    let (ink, accent) = if award { (0x3D2B0F, 0xB8892E) } else { (0x14324A, 0x2F5D8A) };
    let mut o = vec![
        shape(ShapeKind::Rectangle, (24.0, 24.0, w - 48.0, h - 48.0), Some(0xFFFDF7), Some((accent, 3.0)), true),
        shape(ShapeKind::Rectangle, (36.0, 36.0, w - 72.0, h - 72.0), None, Some((accent, 0.75)), true),
        shape(ShapeKind::Star, (w / 2.0 - 34.0, 64.0, 68.0, 68.0), Some(accent), None, false),
    ];
    let title = if award { "Award of Excellence" } else { "Certificate of Completion" };
    let body = if award { "for outstanding work and generosity to the studio community" } else { "has completed the twelve-week course in" };
    let mut lines = vec![
        centered(pc(title, cp(Some(36.0), false, Some(ink), Some("Georgia")))),
        spaced(centered(pc("THIS IS PRESENTED TO", cp(Some(11.0), true, Some(accent), None))), 24.0, 6.0),
        centered(pc("Recipient Name", cp(Some(32.0), false, Some(ink), Some("Georgia")))),
        spaced(centered(pc(body, cp(Some(13.0), false, Some(0x5A5A5A), None))), 14.0, 2.0),
    ];
    if !award {
        lines.push(centered(pc("Introduction to Bookbinding", cp(Some(16.0), true, Some(ink), None))));
    }
    o.push(text_box(&mut d, lines, (96.0, 150.0, w - 192.0, 260.0), None));
    for (i, (label, x)) in [("Date", 140.0), ("Signature", w - 340.0)].into_iter().enumerate() {
        o.push(shape(ShapeKind::Line, (x, 470.0, 200.0, 1.0), None, Some((ink, 1.0)), false));
        o.push(text_box(&mut d, vec![centered(pc(label, cp(Some(11.0), false, Some(0x5A5A5A), None)))], (x, 476.0, 200.0, 24.0), None));
        let _ = i;
    }
    d.body = vec![Arc::new(Block::Para(anchored(o, p(""))))];
    d
}

// ---------------------------------------------------------------------------------------------
// Databases.

/// Build template `id` as a database.
pub fn database(id: &str) -> Option<Database> {
    let mut db = match id {
        "db-reading" => reading_list(),
        "db-literature" => literature_review(),
        "db-assignments" => assignments(),
        "db-courses" => courses(),
        "db-research-log" => research_log(),
        "db-projects" => projects(),
        "db-contacts" => contacts(),
        "db-characters" => characters(),
        _ => return None,
    };
    db.current = 0;
    db.dirty = true;
    Some(db)
}

fn reading_list() -> Database {
    let mut db = Database::new("Reading List");
    db.icon = "book".into();
    db.description = "Books and articles to read, with where you are in each and what you thought.".into();
    db.add_prop("Author", Kind::Text);
    db.add_select("Status", Kind::Status, &[("To read", Tint::Gray), ("Reading", Tint::Blue), ("Finished", Tint::Green)]);
    db.add_select("Type", Kind::Select, &[("Book", Tint::Brown), ("Article", Tint::Purple), ("Paper", Tint::Teal)]);
    db.add_prop("Rating", Kind::Rating);
    db.add_prop("Finished on", Kind::Date);
    db.add_prop("Tags", Kind::MultiSelect);
    for (n, a, s, ty, r, tags) in [
        ("Thinking in Systems", "Donella Meadows", "Finished", "Book", "5", "systems"),
        ("The Structure of Scientific Revolutions", "Thomas Kuhn", "Reading", "Book", "", "philosophy of science"),
        ("Attention Is All You Need", "Vaswani et al.", "Finished", "Paper", "4", "machine learning"),
        ("Silent Spring", "Rachel Carson", "To read", "Book", "", "ecology"),
    ] {
        db.add_row_with(&[("Name", n), ("Author", a), ("Status", s), ("Type", ty), ("Rating", r), ("Tags", tags)]);
    }
    db.add_view("Board", Layout::Board);
    db.add_view("Gallery", Layout::Gallery);
    db
}

fn literature_review() -> Database {
    let mut db = Database::new("Literature Review");
    db.icon = "flask".into();
    db.description = "A matrix of sources: what each one asked, how, what it found, and how it bears on your question.".into();
    db.add_prop("Authors", Kind::Text);
    db.add_prop("Year", Kind::Number);
    db.add_select(
        "Method",
        Kind::Select,
        &[("Experiment", Tint::Blue), ("Survey", Tint::Orange), ("Case study", Tint::Purple), ("Review", Tint::Gray)],
    );
    db.add_prop("Key finding", Kind::Text);
    db.add_select("Relevance", Kind::Select, &[("High", Tint::Green), ("Medium", Tint::Yellow), ("Low", Tint::Gray)]);
    db.add_prop("DOI", Kind::Url);
    db.add_prop("Themes", Kind::MultiSelect);
    db.add_prop("Read", Kind::Checkbox);
    for (n, a, y, m, f, rel, th) in [
        ("Shade and soil moisture in city parks", "Okafor & Lee", "2025", "Experiment", "Moisture rose with canopy cover", "High", "soil, shade"),
        ("Street trees and summer heat", "Ramos et al.", "2022", "Survey", "Tree-lined streets were 2–3 °C cooler", "Medium", "heat, trees"),
        ("A review of urban soil studies", "Chen", "2019", "Review", "Few long-term datasets exist", "High", "soil, methods"),
    ] {
        db.add_row_with(&[("Name", n), ("Authors", a), ("Year", y), ("Method", m), ("Key finding", f), ("Relevance", rel), ("Themes", th)]);
    }
    db.add_view("By relevance", Layout::Board);
    db
}

fn assignments() -> Database {
    let mut db = Database::new("Assignments");
    db.icon = "check".into();
    db.description = "Every assignment, when it's due and how it went.".into();
    db.add_select("Course", Kind::Select, &[("Statistics", Tint::Blue), ("Ecology", Tint::Green), ("Writing", Tint::Pink)]);
    db.add_prop("Due", Kind::Date);
    db.add_select("Status", Kind::Status, &[("Not started", Tint::Gray), ("In progress", Tint::Blue), ("Submitted", Tint::Green)]);
    db.add_prop("Weight (%)", Kind::Number);
    db.add_prop("Grade", Kind::Number);
    let base = crate::db::today();
    let soon = |days: u32| -> String {
        let y: i32 = base.get(..4).and_then(|s| s.parse().ok()).unwrap_or(2026);
        let m: u32 = base.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(1);
        let d: u32 = base.get(8..10).and_then(|s| s.parse().ok()).unwrap_or(1);
        let dim = crate::db::days_in_month(y, m).max(1);
        let total = d + days;
        if total > dim {
            format!("{:04}-{:02}-{:02}", if m == 12 { y + 1 } else { y }, if m == 12 { 1 } else { m + 1 }, total - dim)
        } else {
            format!("{y:04}-{m:02}-{total:02}")
        }
    };
    for (n, c, due, s, w) in [
        ("Problem set 3", "Statistics", soon(3), "In progress", "10"),
        ("Field notebook", "Ecology", soon(8), "Not started", "25"),
        ("Essay draft", "Writing", soon(12), "Not started", "30"),
        ("Problem set 2", "Statistics", soon(0), "Submitted", "10"),
    ] {
        db.add_row_with(&[("Name", n), ("Course", c), ("Due", &due), ("Status", s), ("Weight (%)", w)]);
    }
    db.add_view("Board", Layout::Board);
    db.add_view("Calendar", Layout::Calendar);
    db
}

fn courses() -> Database {
    let mut db = Database::new("Course Planner");
    db.icon = "cap".into();
    db.add_select("Term", Kind::Select, &[("Autumn", Tint::Orange), ("Spring", Tint::Green), ("Summer", Tint::Yellow)]);
    db.add_prop("Credits", Kind::Number);
    db.add_prop("Lecturer", Kind::Text);
    db.add_prop("Room", Kind::Text);
    db.add_select("Status", Kind::Status, &[("Planned", Tint::Gray), ("Taking", Tint::Blue), ("Passed", Tint::Green)]);
    for (n, term, cr, lec) in [
        ("Research Methods", "Autumn", "15", "Dr Patel"),
        ("Machine Learning", "Autumn", "15", "Dr Nakamura"),
        ("Dissertation", "Summer", "60", "Supervisor"),
    ] {
        db.add_row_with(&[("Name", n), ("Term", term), ("Credits", cr), ("Lecturer", lec), ("Status", "Planned")]);
    }
    db.add_view("By term", Layout::Board);
    if let Some(v) = db.views.last_mut() {
        v.group_by = db.props.iter().find(|p| p.name == "Term").map(|p| p.id);
    }
    db
}

fn research_log() -> Database {
    let mut db = Database::new("Research Log");
    db.icon = "pen".into();
    db.description = "One record per session: what you tried, what happened, what's next.".into();
    db.add_prop("Date", Kind::Date);
    db.add_select(
        "Type",
        Kind::Select,
        &[("Experiment", Tint::Blue), ("Reading", Tint::Brown), ("Analysis", Tint::Purple), ("Meeting", Tint::Orange)],
    );
    db.add_prop("What happened", Kind::Text);
    db.add_prop("Next step", Kind::Text);
    db.add_prop("Mood", Kind::Rating);
    let today = crate::db::today();
    db.add_row_with(&[
        ("Name", "Pilot soil samples"),
        ("Date", &today),
        ("Type", "Experiment"),
        ("What happened", "Readings drifted after 20 minutes"),
        ("Next step", "Recalibrate the probe"),
    ]);
    db.add_view("Calendar", Layout::Calendar);
    db.add_view("List", Layout::List);
    db
}

fn projects() -> Database {
    let mut db = Database::new("Projects");
    db.icon = "bolt".into();
    db.add_select("Status", Kind::Status, &[("Idea", Tint::Gray), ("Doing", Tint::Blue), ("Blocked", Tint::Red), ("Done", Tint::Green)]);
    db.add_select("Priority", Kind::Select, &[("High", Tint::Red), ("Medium", Tint::Yellow), ("Low", Tint::Gray)]);
    db.add_prop("Due", Kind::Date);
    db.add_prop("Owner", Kind::Text);
    for (n, s, pr) in [
        ("Portfolio website", "Doing", "High"),
        ("Data pipeline demo", "Idea", "Medium"),
        ("Conference poster", "Blocked", "High"),
        ("Reading group", "Done", "Low"),
    ] {
        db.add_row_with(&[("Name", n), ("Status", s), ("Priority", pr)]);
    }
    // The board first: projects are mostly moved between stages.
    db.add_view("Board", Layout::Board);
    db.views.swap(0, 1);
    db
}

fn contacts() -> Database {
    let mut db = Database::new("Contacts");
    db.icon = "people".into();
    db.description = "People to write to. Import contacts (.vcf) or a spreadsheet, then use them for Mail Merge.".into();
    for (n, k) in [
        ("First Name", Kind::Text),
        ("Last Name", Kind::Text),
        ("Company", Kind::Text),
        ("Email", Kind::Email),
        ("Phone", Kind::Phone),
        ("Address", Kind::Text),
        ("City", Kind::Text),
        ("Postal Code", Kind::Text),
        ("Country", Kind::Text),
    ] {
        db.add_prop(n, k);
    }
    db.add_prop("Groups", Kind::MultiSelect);
    if let Some(p) = db.props.first_mut() {
        p.name = "Full Name".into();
    }
    db.add_row_with(&[
        ("Full Name", "Ada Lovelace"),
        ("First Name", "Ada"),
        ("Last Name", "Lovelace"),
        ("Email", "ada@example.org"),
        ("City", "London"),
        ("Groups", "friends"),
    ]);
    db.add_row_with(&[
        ("Full Name", "Alan Turing"),
        ("First Name", "Alan"),
        ("Last Name", "Turing"),
        ("Email", "alan@example.org"),
        ("City", "Wilmslow"),
        ("Groups", "work"),
    ]);
    db.add_view("Gallery", Layout::Gallery);
    db
}

fn characters() -> Database {
    let mut db = Database::new("Characters");
    db.icon = "person".into();
    db.description = "Who's who in your story: roles, wants, and where they first appear.".into();
    db.add_select(
        "Role",
        Kind::Select,
        &[("Protagonist", Tint::Red), ("Antagonist", Tint::Purple), ("Supporting", Tint::Blue), ("Minor", Tint::Gray)],
    );
    db.add_prop("Wants", Kind::Text);
    db.add_prop("Fears", Kind::Text);
    db.add_prop("First appears", Kind::Text);
    db.add_select("POV", Kind::Select, &[("Yes", Tint::Green), ("No", Tint::Gray)]);
    db.add_row_with(&[
        ("Name", "Mara"),
        ("Role", "Protagonist"),
        ("Wants", "To keep the light burning"),
        ("Fears", "Being the one who let it go out"),
        ("POV", "Yes"),
    ]);
    db.add_row_with(&[("Name", "The Harbourmaster"), ("Role", "Antagonist"), ("Wants", "To automate the lighthouse")]);
    db.add_view("By role", Layout::Board);
    if let Some(v) = db.views.last_mut() {
        v.group_by = db.props.iter().find(|p| p.name == "Role").map(|p| p.id);
    }
    db.add_view("Gallery", Layout::Gallery);
    db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_and_lays_out() {
        for t in ALL {
            match t.made {
                Made::Database => {
                    let db = database(t.id).unwrap_or_else(|| panic!("{} builds", t.id));
                    assert!(!db.views.is_empty() && db.title_prop().is_some(), "{}", t.id);
                }
                _ => {
                    let doc = document(t.id).unwrap_or_else(|| panic!("{} builds", t.id));
                    let mut s = wordcraft_engine::Session::new(doc);
                    assert!(!s.layout().pages.is_empty(), "{} lays out", t.id);
                }
            }
        }
        assert!(document("nope").is_none());
        assert!(database("nope").is_none());
    }

    #[test]
    fn layout_templates_place_objects_on_the_page() {
        let d = document("event-flyer").unwrap();
        let objs: usize = d.body.iter().filter_map(|b| b.as_para()).map(|p| p.objects.len()).sum();
        assert!(objs >= 5);
        assert!(!d.media.is_empty(), "the picture is drawn and embedded");
    }

    #[test]
    fn pictures_draw_at_any_size() {
        for art in [Art::Dunes, Art::Lights, Art::Waves, Art::Mosaic, Art::Hills, Art::Wash] {
            assert_eq!(draw(art, 40, 30).len(), 40 * 30 * 4);
            assert_eq!(draw(art, 0, 0).len(), 4);
        }
    }
}

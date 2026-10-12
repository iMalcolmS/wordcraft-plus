//! The Glass ribbon: one floating glass card under the title bar, holding the selected tab's
//! groups as quiet rounded panels.
//!
//! The main tabs are curated tables (below): fewer, larger targets than the classic ribbon, each a
//! real WordCraft command id, so everything a button does is the engine's own behaviour. Tabs the
//! tables don't cover (the contextual Table, Picture, Shape and Chart tabs, Equation, Outlining,
//! Developer, Zotero) are built from the command registry itself, grouped by each command's ribbon
//! location, so no feature goes missing.

use egui::{Rect, Sense, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_engine::Session;
use wordcraft_ui_egui::theme::{medium, regular, semibold};
use wordcraft_ui_egui::{WordApp, icons, widgets as wc};

use crate::GlassApp;
use crate::tokens::{Tokens, radius};
use crate::widgets::{self as w, BIG_H, params};

/// Whether a command's button shows as pressed.
#[derive(Clone, Copy)]
pub enum On {
    No,
    /// A flag from `format.state` (bold, italic…).
    Fmt(&'static str),
    /// Something in the session (view settings, track changes).
    Session(fn(&Session) -> bool),
}

/// One command button: label, command id, params (a JSON literal), icon.
#[derive(Clone, Copy)]
pub struct Cmd {
    pub label: &'static str,
    pub id: &'static str,
    pub params: &'static str,
    pub icon: &'static str,
    pub on: On,
}

const fn c(label: &'static str, id: &'static str, icon: &'static str) -> Cmd {
    Cmd { label, id, params: "", icon, on: On::No }
}
const fn cp(label: &'static str, id: &'static str, params: &'static str, icon: &'static str) -> Cmd {
    Cmd { label, id, params, icon, on: On::No }
}
const fn con(label: &'static str, id: &'static str, icon: &'static str, on: On) -> Cmd {
    Cmd { label, id, params: "", icon, on }
}

/// A menu entry: label, command id, params.
pub type Choice = (&'static str, &'static str, &'static str);

/// Glass-only actions (the writing features), shown on the View tab.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Local {
    Typewriter,
    Snapshots,
    History,
    Goals,
    /// Writer Pro: the views and tools of the Writer tab.
    Page,
    Corkboard,
    Outliner,
    Binder,
    Scratchpad,
    CutToScratchpad,
    Pacing,
    Sprint,
    Appearance,
    /// Reader's Glow: Off → Paragraph → Sentence.
    Glow,
    Rhythm,
    Guides,
    Handles,
}

pub enum Item {
    Big(Cmd),
    Small(Cmd),
    BigMenu(Cmd, &'static [Choice]),
    SmallMenu(Cmd, &'static [Choice]),
    /// Font name and size boxes with the character formatting toggles.
    Font,
    /// The style gallery.
    Styles,
    /// A menu filled from the engine's theme list.
    Themes,
    Glass(Local, &'static str, &'static str),
    /// An icon-only button (its label is the tooltip).
    Icon(Cmd),
}

pub struct Group {
    pub name: &'static str,
    pub items: &'static [Item],
}

use self::Item::{BigMenu, Small, SmallMenu};

fn track_changes_on(s: &Session) -> bool {
    s.doc.settings.track_changes
}
fn ruler_on(s: &Session) -> bool {
    s.view.ruler
}
fn nav_on(s: &Session) -> bool {
    s.view.nav_pane
}
fn focus_on(s: &Session) -> bool {
    s.view.focus_mode
}
fn read_on(s: &Session) -> bool {
    s.view.read_mode
}
fn marks_on(s: &Session) -> bool {
    s.view.marks
}
fn print_on(s: &Session) -> bool {
    s.view.mode == wordcraft_engine::layout::ViewMode::Print && !s.view.read_mode
}
fn web_on(s: &Session) -> bool {
    s.view.mode == wordcraft_engine::layout::ViewMode::Web
}

const HOME: &[Group] = &[
    Group {
        name: "Clipboard",
        items: &[
            Item::Big(c("Paste", "edit.paste", "paste")),
            Small(c("Cut", "edit.cut", "cut")),
            Small(c("Copy", "edit.copy", "copy")),
            Small(c("Format Painter", "edit.formatPainter", "painter")),
        ],
    },
    Group { name: "Font", items: &[Item::Font] },
    Group {
        name: "Paragraph",
        items: &[
            Small(c("Bullets", "para.bullets", "bullets")),
            Small(c("Numbering", "para.numbering", "numbering")),
            SmallMenu(
                c("Line Spacing", "para.lineSpacing", "lineSpacing"),
                &[
                    ("1.0", "para.lineSpacing", r#"{"value": 1.0}"#),
                    ("1.15", "para.lineSpacing", r#"{"value": 1.15}"#),
                    ("1.5", "para.lineSpacing", r#"{"value": 1.5}"#),
                    ("2.0", "para.lineSpacing", r#"{"value": 2.0}"#),
                    ("Paragraph Settings…", "para.dialog", ""),
                ],
            ),
            Small(c("Align Left", "para.alignLeft", "alignLeft")),
            Small(c("Centre", "para.alignCenter", "alignCenter")),
            Small(c("Justify", "para.justify", "justify")),
            Small(c("Increase Indent", "para.indent", "indentRight")),
            Small(c("Decrease Indent", "para.outdent", "indentLeft")),
            Small(con("Show ¶", "view.marks", "pilcrow", On::Session(marks_on))),
        ],
    },
    Group { name: "Styles", items: &[Item::Styles, Item::Icon(c("Styles Pane", "view.stylesPane", "stylesPane"))] },
    Group {
        name: "Editing",
        items: &[
            Small(c("Find", "edit.find", "find")),
            Small(c("Replace", "edit.replace", "replace")),
            Small(c("Select All", "select.all", "select")),
        ],
    },
];

const INSERT: &[Group] = &[
    Group {
        name: "Pages",
        items: &[
            Item::Big(c("Page\nBreak", "insert.pageBreak", "pageBreak")),
            Small(c("Blank Page", "insert.blankPage", "blankPage")),
            Small(c("Cover Page", "insert.coverPage", "coverPage")),
        ],
    },
    Group { name: "Tables", items: &[Item::Big(cp("Table", "ui.dialog", r#"{"name": "insertTable"}"#, "table"))] },
    Group {
        name: "Illustrations",
        items: &[
            Item::Big(c("Pictures", "insert.picture", "picture")),
            BigMenu(
                c("Shapes", "insert.shape", "shapes"),
                &[
                    ("Rectangle", "insert.shape", r#"{"kind": "rectangle"}"#),
                    ("Rounded Rectangle", "insert.shape", r#"{"kind": "roundedRectangle"}"#),
                    ("Oval", "insert.shape", r#"{"kind": "ellipse"}"#),
                    ("Triangle", "insert.shape", r#"{"kind": "triangle"}"#),
                    ("Line", "insert.shape", r#"{"kind": "line"}"#),
                    ("Arrow", "insert.shape", r#"{"kind": "arrow"}"#),
                    ("Star", "insert.shape", r#"{"kind": "star"}"#),
                ],
            ),
            Small(cp("Icons", "ui.dialog", r#"{"name": "icons"}"#, "icons")),
            Small(c("Chart", "insert.chart", "chart")),
            Small(c("SmartArt", "insert.smartArt", "smartArt")),
        ],
    },
    Group {
        name: "Links",
        items: &[
            Small(cp("Link", "ui.dialog", r#"{"name": "link"}"#, "link")),
            Small(c("Bookmark", "insert.bookmark", "bookmark")),
            Small(c("Cross-reference", "insert.crossReference", "crossRef")),
        ],
    },
    Group { name: "Comments", items: &[Item::Big(c("Comment", "review.newComment", "newComment"))] },
    Group {
        name: "Header & Footer",
        items: &[
            SmallMenu(
                c("Header", "insert.header", "header"),
                &[
                    ("Blank", "insert.header", r#"{"preset": "blank"}"#),
                    ("Blank (Three Columns)", "insert.header", r#"{"preset": "blankThree"}"#),
                    ("Title", "insert.header", r#"{"preset": "title"}"#),
                    ("Edit Header", "insert.editHeader", ""),
                    ("Remove Header", "insert.removeHeader", ""),
                ],
            ),
            SmallMenu(
                c("Footer", "insert.footer", "footer"),
                &[
                    ("Blank", "insert.footer", r#"{"preset": "blank"}"#),
                    ("Blank (Three Columns)", "insert.footer", r#"{"preset": "blankThree"}"#),
                    ("Page Number", "insert.footer", r#"{"preset": "pageNumber"}"#),
                    ("Edit Footer", "insert.editFooter", ""),
                    ("Remove Footer", "insert.removeFooter", ""),
                ],
            ),
            SmallMenu(
                c("Page Number", "insert.pageNumber", "pageNumber"),
                &[
                    ("Top of Page", "insert.pageNumber", r#"{"position": "top", "align": "center"}"#),
                    ("Bottom of Page", "insert.pageNumber", r#"{"position": "bottom", "align": "center"}"#),
                    ("Page X of Y", "insert.pageNumber", r#"{"position": "bottom", "align": "center", "format": "x of y"}"#),
                    ("Current Position", "insert.pageNumber", r#"{"position": "current"}"#),
                ],
            ),
        ],
    },
    Group {
        name: "Text",
        items: &[
            Item::Big(c("Text Box", "insert.textBox", "textBox")),
            Small(c("Drop Cap", "insert.dropCap", "dropCap")),
            Small(c("Date & Time", "insert.dateTime", "dateTime")),
            Small(c("WordArt", "insert.wordArt", "wordArt")),
        ],
    },
    Group { name: "Symbols", items: &[Item::Big(c("Equation", "insert.equation", "equation")), Item::Big(c("Symbol", "insert.symbol", "symbol"))] },
];

const DRAW: &[Group] = &[
    Group {
        name: "Tools",
        items: &[
            Item::Big(c("Select", "draw.select", "select")),
            Item::Big(c("Lasso", "draw.lasso", "lasso")),
            Item::Big(c("Eraser", "draw.eraser", "eraser")),
        ],
    },
    Group {
        name: "Pens",
        items: &[
            Item::Big(cp("Pen", "draw.pen", r#"{"kind": "pen"}"#, "pen")),
            Item::Big(cp("Pencil", "draw.pen", r#"{"kind": "pencil"}"#, "pencil")),
            Item::Big(cp("Highlighter", "draw.pen", r#"{"kind": "highlighter"}"#, "highlight")),
        ],
    },
    Group {
        name: "Convert",
        items: &[Item::Big(c("Ink to\nShape", "draw.inkToShape", "inkToShape")), Small(c("Ink Replay", "draw.replay", "replay"))],
    },
];

const DESIGN: &[Group] = &[
    Group {
        name: "Document Formatting",
        items: &[
            Item::Themes,
            BigMenu(
                c("Style Set", "design.styleSet", "styles"),
                &[
                    ("Default", "design.styleSet", r#"{"name": "default"}"#),
                    ("Basic", "design.styleSet", r#"{"name": "basic"}"#),
                    ("Lines", "design.styleSet", r#"{"name": "lines"}"#),
                    ("Shaded", "design.styleSet", r#"{"name": "shaded"}"#),
                    ("Casual", "design.styleSet", r#"{"name": "casual"}"#),
                    ("Centred", "design.styleSet", r#"{"name": "centered"}"#),
                    ("Minimalist", "design.styleSet", r#"{"name": "minimalist"}"#),
                    ("Title", "design.styleSet", r#"{"name": "title"}"#),
                ],
            ),
            SmallMenu(
                c("Paragraph Spacing", "design.paragraphSpacing", "paraSpacing"),
                &[
                    ("Default", "design.paragraphSpacing", r#"{"value": "default"}"#),
                    ("No Paragraph Space", "design.paragraphSpacing", r#"{"value": "none"}"#),
                    ("Compact", "design.paragraphSpacing", r#"{"value": "compact"}"#),
                    ("Tight", "design.paragraphSpacing", r#"{"value": "tight"}"#),
                    ("Open", "design.paragraphSpacing", r#"{"value": "open"}"#),
                    ("Relaxed", "design.paragraphSpacing", r#"{"value": "relaxed"}"#),
                    ("Double", "design.paragraphSpacing", r#"{"value": "double"}"#),
                ],
            ),
            Small(c("Set as Default", "design.setDefault", "setDefault")),
        ],
    },
    Group {
        name: "Page Background",
        items: &[
            Item::Big(cp("Watermark", "ui.dialog", r#"{"name": "watermark"}"#, "watermark")),
            BigMenu(
                c("Page Colour", "design.pageColor", "pageColor"),
                &[
                    ("No Colour", "design.pageColor", r#"{"color": null}"#),
                    ("Paper", "design.pageColor", r#"{"color": "FBF8F1"}"#),
                    ("Mist", "design.pageColor", r#"{"color": "F1F4F8"}"#),
                    ("Sage", "design.pageColor", r#"{"color": "EEF3EC"}"#),
                ],
            ),
            Item::Big(c("Page\nBorders", "design.pageBorders", "pageBorders")),
        ],
    },
];

const LAYOUT: &[Group] = &[
    Group {
        name: "Page Setup",
        items: &[
            BigMenu(
                c("Margins", "layout.margins", "margins"),
                &[
                    ("Normal", "layout.margins", r#"{"preset": "normal"}"#),
                    ("Narrow", "layout.margins", r#"{"preset": "narrow"}"#),
                    ("Moderate", "layout.margins", r#"{"preset": "moderate"}"#),
                    ("Wide", "layout.margins", r#"{"preset": "wide"}"#),
                    ("Mirrored", "layout.margins", r#"{"preset": "mirrored"}"#),
                    ("Custom Margins…", "ui.dialog", r#"{"name": "pageSetup"}"#),
                ],
            ),
            BigMenu(
                c("Orientation", "layout.orientation", "orientation"),
                &[("Portrait", "layout.orientation", r#"{"value": "portrait"}"#), ("Landscape", "layout.orientation", r#"{"value": "landscape"}"#)],
            ),
            BigMenu(
                c("Size", "layout.size", "size"),
                &[
                    ("A4", "layout.size", r#"{"name": "A4"}"#),
                    ("A5", "layout.size", r#"{"name": "A5"}"#),
                    ("Letter", "layout.size", r#"{"name": "Letter"}"#),
                    ("Legal", "layout.size", r#"{"name": "Legal"}"#),
                ],
            ),
            BigMenu(
                c("Columns", "layout.columns", "columns"),
                &[
                    ("One", "layout.columns", r#"{"count": 1}"#),
                    ("Two", "layout.columns", r#"{"count": 2}"#),
                    ("Three", "layout.columns", r#"{"count": 3}"#),
                    ("Left", "layout.columns", r#"{"preset": "left"}"#),
                    ("Right", "layout.columns", r#"{"preset": "right"}"#),
                ],
            ),
            SmallMenu(
                c("Breaks", "layout.break", "breaks"),
                &[
                    ("Page", "layout.break", r#"{"kind": "page"}"#),
                    ("Column", "layout.break", r#"{"kind": "column"}"#),
                    ("Next Page Section", "layout.break", r#"{"kind": "nextPage"}"#),
                    ("Continuous Section", "layout.break", r#"{"kind": "continuous"}"#),
                ],
            ),
            Small(c("Line Numbers", "layout.lineNumbers", "lineNumbers")),
            Small(c("Hyphenation", "layout.hyphenation", "hyphenation")),
        ],
    },
    Group {
        name: "Paragraph",
        items: &[
            Small(c("Paragraph…", "para.dialog", "paraSpacing")),
            Small(cp("Page Setup…", "ui.dialog", r#"{"name": "pageSetup"}"#, "properties")),
            Small(cp("Tabs…", "ui.dialog", r#"{"name": "tabs"}"#, "indent")),
        ],
    },
    Group {
        name: "Arrange",
        items: &[
            Item::Big(c("Position", "arrange.position", "position")),
            BigMenu(
                c("Wrap Text", "arrange.wrap", "wrapText"),
                &[
                    ("In Line with Text", "arrange.wrap", r#"{"wrap": "inline"}"#),
                    ("Square", "arrange.wrap", r#"{"wrap": "square"}"#),
                    ("Tight", "arrange.wrap", r#"{"wrap": "tight"}"#),
                    ("Top and Bottom", "arrange.wrap", r#"{"wrap": "topAndBottom"}"#),
                    ("Behind Text", "arrange.wrap", r#"{"wrap": "behindText"}"#),
                    ("In Front of Text", "arrange.wrap", r#"{"wrap": "inFrontOfText"}"#),
                ],
            ),
            Small(c("Bring Forward", "arrange.bringForward", "bringForward")),
            Small(c("Send Backward", "arrange.sendBackward", "sendBackward")),
            Small(c("Align", "arrange.align", "align")),
            Small(c("Group", "arrange.group", "group")),
            SmallMenu(
                c("Rotate", "arrange.rotate", "rotate"),
                &[
                    ("Rotate Right 90°", "arrange.rotate", r#"{"direction": "right"}"#),
                    ("Rotate Left 90°", "arrange.rotate", r#"{"direction": "left"}"#),
                    ("Flip Vertical", "arrange.rotate", r#"{"direction": "flipVertical"}"#),
                    ("Flip Horizontal", "arrange.rotate", r#"{"direction": "flipHorizontal"}"#),
                ],
            ),
            Small(c("Selection Pane", "arrange.selectionPane", "selectionPane")),
        ],
    },
];

const REFERENCES: &[Group] = &[
    Group {
        name: "Table of Contents",
        items: &[
            BigMenu(
                c("Contents", "references.toc", "toc"),
                &[
                    ("Automatic Table (3 levels)", "references.toc", r#"{"levels": 3}"#),
                    ("Automatic Table (2 levels)", "references.toc", r#"{"levels": 2, "title": "Table of Contents"}"#),
                    ("Remove Table of Contents", "references.removeToc", ""),
                ],
            ),
            Small(c("Update Table", "references.updateToc", "updateTable")),
        ],
    },
    Group {
        name: "Footnotes",
        items: &[
            Item::Big(c("Footnote", "references.footnote", "footnote")),
            Small(c("Endnote", "references.endnote", "endnote")),
            Small(c("Next Footnote", "references.nextFootnote", "nextFootnote")),
            Small(c("Show Notes", "references.notes", "showNotes")),
        ],
    },
    Group {
        name: "Citations",
        items: &[
            Item::Big(c("Citation", "references.citation", "citation")),
            Small(c("Manage Sources", "references.sources", "sources")),
            SmallMenu(
                c("Style", "references.citationStyle", "citation"),
                &[
                    ("APA", "references.citationStyle", r#"{"style": "APA"}"#),
                    ("MLA", "references.citationStyle", r#"{"style": "MLA"}"#),
                    ("Chicago", "references.citationStyle", r#"{"style": "Chicago"}"#),
                    ("IEEE", "references.citationStyle", r#"{"style": "IEEE"}"#),
                ],
            ),
            Small(c("Bibliography", "references.bibliography", "bibliography")),
        ],
    },
    Group {
        name: "Captions",
        items: &[
            Item::Big(c("Caption", "references.caption", "caption")),
            Small(c("Table of Figures", "references.tableOfFigures", "tableOfFigures")),
            Small(c("Update Table", "references.updateFigures", "updateTable")),
            Small(c("Cross-reference", "insert.crossReference", "crossRef")),
        ],
    },
    Group {
        name: "Index",
        items: &[
            Item::Big(c("Mark\nEntry", "references.markEntry", "markEntry")),
            Small(c("Insert Index", "references.index", "index")),
            Small(c("Update Index", "references.updateIndex", "update")),
        ],
    },
];

const MAILINGS: &[Group] = &[
    Group {
        name: "Create",
        items: &[Item::Big(c("Envelopes", "mailings.envelopes", "envelope")), Item::Big(c("Labels", "mailings.labels", "labels"))],
    },
    Group {
        name: "Start Mail Merge",
        items: &[
            BigMenu(
                c("Start Mail\nMerge", "mailings.start", "mailMerge"),
                &[
                    ("Letters", "mailings.start", r#"{"kind": "letters"}"#),
                    ("Email Messages", "mailings.start", r#"{"kind": "email"}"#),
                    ("Envelopes", "mailings.start", r#"{"kind": "envelopes"}"#),
                    ("Labels", "mailings.start", r#"{"kind": "labels"}"#),
                    ("Directory", "mailings.start", r#"{"kind": "directory"}"#),
                    ("Normal Document", "mailings.start", r#"{"kind": "normal"}"#),
                ],
            ),
            Small(c("Select Recipients", "mailings.recipients", "recipients")),
            Small(c("Edit Recipients", "mailings.editRecipients", "editRecipients")),
        ],
    },
    Group {
        name: "Fields",
        items: &[
            Small(c("Address Block", "mailings.addressBlock", "addressBlock")),
            Small(c("Greeting Line", "mailings.greetingLine", "greetingLine")),
            Small(c("Merge Field", "mailings.insertField", "mergeField")),
            Small(c("Highlight Fields", "mailings.highlightFields", "highlightFields")),
            Small(c("Rules", "mailings.rules", "rules")),
            Small(c("Match Fields", "mailings.matchFields", "matchFields")),
        ],
    },
    Group {
        name: "Preview",
        items: &[
            Item::Big(c("Preview\nResults", "mailings.preview", "preview")),
            Small(c("Previous", "mailings.previous", "previous")),
            Small(c("Next", "mailings.next", "next")),
            Small(c("Check Errors", "mailings.checkErrors", "checkErrors")),
        ],
    },
    Group { name: "Finish", items: &[Item::Big(c("Finish &\nMerge", "mailings.finish", "finish"))] },
];

const REVIEW: &[Group] = &[
    Group {
        name: "Proofing",
        items: &[
            Item::Big(c("Spelling &\nGrammar", "review.spelling", "spelling")),
            Small(c("Thesaurus", "review.thesaurus", "thesaurus")),
            Small(c("Word Count", "review.wordCount", "wordCount")),
            Small(c("Language", "review.language", "language")),
        ],
    },
    Group { name: "Accessibility", items: &[Item::Big(c("Check\nAccessibility", "file.accessibility", "accessibility"))] },
    Group { name: "Speech", items: &[Item::Big(c("Read\nAloud", "review.readAloud", "readAloud"))] },
    Group {
        name: "Comments",
        items: &[
            Item::Big(c("New\nComment", "review.newComment", "newComment")),
            Small(c("Delete", "review.deleteComment", "deleteComment")),
            Small(c("Previous", "review.previousComment", "prevComment")),
            Small(c("Next", "review.nextComment", "nextComment")),
            Small(c("Show Comments", "review.comments", "showComments")),
        ],
    },
    Group {
        name: "Tracking",
        items: &[
            Item::Big(con("Track\nChanges", "review.trackChanges", "trackChanges", On::Session(track_changes_on))),
            SmallMenu(
                c("Markup", "review.markup", "markup"),
                &[("All Markup", "review.markup", r#"{"value": "all"}"#), ("No Markup", "review.markup", r#"{"value": "noMarkup"}"#)],
            ),
            Small(c("Reviewing Pane", "review.changes", "reviewingPane")),
        ],
    },
    Group {
        name: "Changes",
        items: &[
            BigMenu(
                c("Accept", "review.accept", "accept"),
                &[("Accept This Change", "review.accept", ""), ("Accept All Changes", "review.acceptAll", "")],
            ),
            BigMenu(
                c("Reject", "review.reject", "reject"),
                &[("Reject This Change", "review.reject", ""), ("Reject All Changes", "review.rejectAll", "")],
            ),
            Small(c("Previous", "review.previousChange", "prevChange")),
            Small(c("Next", "review.nextChange", "nextChange")),
        ],
    },
    Group { name: "Compare", items: &[Item::Big(c("Compare", "review.compare", "compare"))] },
    Group { name: "Protect", items: &[Item::Big(c("Restrict\nEditing", "review.restrict", "restrict"))] },
];

const VIEW: &[Group] = &[
    Group {
        name: "Views",
        items: &[
            Item::Big(con("Read\nMode", "view.readMode", "readMode", On::Session(read_on))),
            Item::Big(con("Print\nLayout", "view.printLayout", "printLayout", On::Session(print_on))),
            Item::Big(con("Web\nLayout", "view.webLayout", "webLayout", On::Session(web_on))),
            Small(c("Outline", "view.outline", "outline")),
            Small(c("Draft", "view.draft", "draft")),
        ],
    },
    Group { name: "Immersive", items: &[Item::Big(con("Focus", "view.focus", "focus", On::Session(focus_on)))] },
    Group {
        name: "Writing",
        items: &[
            Item::Glass(Local::Typewriter, "Typewriter", "focus"),
            Item::Glass(Local::Snapshots, "Snapshots", "screenshot"),
            Item::Glass(Local::History, "Version\nHistory", "replay"),
            Item::Glass(Local::Goals, "Writing\nGoals", "wordCount"),
        ],
    },
    Group {
        name: "Show",
        items: &[
            Small(con("Ruler", "view.ruler", "ruler", On::Session(ruler_on))),
            Small(c("Gridlines", "view.gridlines", "gridlines")),
            Small(con("Navigation Pane", "view.navigationPane", "navPane", On::Session(nav_on))),
        ],
    },
    Group {
        name: "Zoom",
        items: &[
            Item::Big(cp("Zoom", "ui.dialog", r#"{"name": "zoom"}"#, "zoom")),
            Small(c("100%", "view.zoom100", "zoom100")),
            Small(c("One Page", "view.onePage", "onePage")),
            Small(c("Multiple Pages", "view.multiplePages", "multiplePages")),
        ],
    },
    Group {
        name: "Appearance",
        items: &[
            SmallMenu(
                c("Interface", "ui.theme", "interfaceTheme"),
                &[
                    ("Light", "ui.theme", r#"{"value": "light"}"#),
                    ("Dark", "ui.theme", r#"{"value": "dark"}"#),
                    ("Match System", "ui.theme", r#"{"value": "system"}"#),
                ],
            ),
            Small(c("Dark Page", "view.darkMode", "darkMode")),
            Small(c("Macros", "tools.macros", "macros")),
            Item::Glass(Local::Appearance, "More\nAppearance", "interfaceTheme"),
        ],
    },
];

const WRITER: &[Group] = &[
    Group {
        name: "Views",
        items: &[
            Item::Glass(Local::Page, "Page", "printLayout"),
            Item::Glass(Local::Corkboard, "Corkboard", "multiplePages"),
            Item::Glass(Local::Outliner, "Outliner", "outline"),
        ],
    },
    Group { name: "Project", items: &[Item::Glass(Local::Binder, "Binder", "navPane")] },
    Group {
        name: "Draft",
        items: &[
            Item::Glass(Local::CutToScratchpad, "Cut to\nScratchpad", "cut"),
            Item::Glass(Local::Scratchpad, "Scratchpad", "paste"),
            Item::Glass(Local::Typewriter, "Typewriter", "focus"),
            Item::Big(con("Focus", "view.focus", "focus", On::Session(focus_on))),
        ],
    },
    Group {
        name: "Focus",
        items: &[
            Item::Glass(Local::Glow, "Reader's\nGlow", "focus"),
            Item::Glass(Local::Rhythm, "Rhythm", "wordCount"),
            Item::Glass(Local::Guides, "Guides", "gridlines"),
            Item::Glass(Local::Handles, "Block\nHandles", "more"),
        ],
    },
    Group {
        name: "Insight",
        items: &[
            Item::Glass(Local::Pacing, "Pacing", "wordCount"),
            Item::Glass(Local::Sprint, "Sprint", "replay"),
            Item::Glass(Local::Goals, "Writing\nGoals", "wordCount"),
        ],
    },
    Group {
        name: "Versions",
        items: &[Item::Glass(Local::Snapshots, "Snapshots", "screenshot"), Item::Glass(Local::History, "Version\nHistory", "replay")],
    },
];

const HELP: &[Group] = &[
    Group {
        name: "Help",
        items: &[
            Item::Big(cp("About", "ui.dialog", r#"{"name": "about"}"#, "help")),
            Item::Big(cp("Keyboard\nShortcuts", "ui.dialog", r#"{"name": "customizeKeyboard"}"#, "more")),
        ],
    },
    Group { name: "Commands", items: &[Item::Big(cp("All\nCommands", "ui.dialog", r#"{"name": "commands"}"#, "macros"))] },
];

/// The curated groups of a tab, or `None` for tabs built from the registry.
pub fn curated(tab: &str) -> Option<&'static [Group]> {
    Some(match tab {
        "Home" => HOME,
        "Insert" => INSERT,
        "Draw" => DRAW,
        "Design" => DESIGN,
        "Layout" => LAYOUT,
        "References" => REFERENCES,
        "Mailings" => MAILINGS,
        "Review" => REVIEW,
        "View" => VIEW,
        "Help" => HELP,
        "Writer" => WRITER,
        _ => return None,
    })
}

/// Every command id the curated tabs use (tests check they all exist).
pub fn curated_ids() -> Vec<&'static str> {
    let mut ids = Vec::new();
    for tab in ["Home", "Insert", "Draw", "Design", "Layout", "References", "Mailings", "Review", "View", "Help", "Writer"] {
        for g in curated(tab).unwrap_or(&[]) {
            for item in g.items {
                match item {
                    Item::Big(c) | Item::Small(c) | Item::Icon(c) => ids.push(c.id),
                    Item::BigMenu(c, choices) | Item::SmallMenu(c, choices) => {
                        ids.push(c.id);
                        ids.extend(choices.iter().map(|(_, id, _)| *id));
                    }
                    Item::Font | Item::Styles | Item::Themes | Item::Glass(..) => {}
                }
            }
        }
    }
    ids
}

/// Commands to leave out of registry-built tabs: ones that only make sense with arguments an
/// agent passes (they have no button of their own in any ribbon).
fn programmatic(id: &str) -> bool {
    id.ends_with(".state") || id.ends_with(".set") || id.ends_with(".get") || id.ends_with(".list") || id.starts_with("caret.")
}

/// A tab built from the registry: its commands grouped by the second part of their location.
fn registry_groups(app: &WordApp, tab: &str) -> Vec<(String, Vec<(&'static str, &'static str)>)> {
    let mut groups: Vec<(String, Vec<(&'static str, &'static str)>)> = Vec::new();
    let prefix = format!("{tab} ›");
    for spec in app.session.registry.all() {
        let loc = spec.location;
        if loc != tab && !loc.starts_with(&prefix) {
            continue;
        }
        if programmatic(spec.id) {
            continue;
        }
        let name = loc.split(" › ").nth(1).unwrap_or(tab).to_string();
        match groups.iter_mut().find(|(n, _)| *n == name) {
            Some((_, v)) => v.push((spec.id, spec.label)),
            None => groups.push((name, vec![(spec.id, spec.label)])),
        }
    }
    groups
}

/// The tabs shown, in order: File, the main tabs, then the contextual ones for the selection.
pub fn tabs(app: &WordApp, pro: bool) -> Vec<String> {
    let mut v: Vec<String> = vec!["File".into()];
    for e in app.session.ribbon.tabs().iter().filter(|e| !e.hidden) {
        v.push(e.name.clone());
        if pro && e.name == "Home" {
            v.push("Writer".into());
        }
    }
    for ct in wordcraft_ui_egui::ribbon::contextual_tabs(&app.session) {
        if !v.iter().any(|t| t == ct) {
            v.push(ct.to_string());
        }
    }
    v
}

/// The ribbon card for the current tab: one glass card, one row. When the window is too narrow
/// for every group, the row scrolls sideways (trackpad, wheel or drag) with soft edges showing
/// there is more, instead of wrapping groups onto a second row.
pub fn show(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, expanded: bool) {
    let tab = g.app.ui.tab.clone();
    let now = ui.input(|i| i.time);
    let changed = g.glass.shown_tab != tab;
    if changed {
        g.glass.shown_tab = tab.clone();
        g.glass.tab_changed_at = now;
    }
    // A new tab's controls fade in and slide a little (220 ms, easing out) rather than snapping.
    let p = (((now - g.glass.tab_changed_at) / 0.22) as f32).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - p).powi(3);
    if p < 1.0 {
        ui.ctx().request_repaint();
    }
    let mut open = expanded;
    egui::Panel::top("glass_ribbon")
        .show_separator_line(false)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin { left: 14, right: 14, top: 0, bottom: 10 }))
        .show_collapsible(ui, &mut open, |ui| {
            let card = ui.max_rect();
            let card = Rect::from_min_size(card.min, vec2(card.width(), BIG_H + 18.0 + 4.0 + 20.0));
            ui.painter().add(crate::tokens::glass_shape(card, radius::RIBBON, t));
            let (_, _) = ui.allocate_exact_size(card.size(), Sense::hover());
            let view = card.shrink2(vec2(14.0, 0.0));
            // A chevron click glides the row by most of its width.
            let target_id = egui::Id::new("glass_ribbon_target");
            let anim_id = egui::Id::new("glass_ribbon_glide");
            let target: Option<f32> = ui.data(|d| d.get_temp(target_id));
            // A vertical wheel over the ribbon scrolls it sideways, as a Mac toolbar does.
            if ui.rect_contains_pointer(view) {
                ui.ctx().input_mut(|i| {
                    if i.smooth_scroll_delta.x.abs() < 0.5 && i.smooth_scroll_delta.y.abs() > 0.0 && !i.modifiers.command {
                        i.smooth_scroll_delta.x = i.smooth_scroll_delta.y;
                        i.smooth_scroll_delta.y = 0.0;
                    }
                });
            }
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view).layout(egui::Layout::left_to_right(egui::Align::Min)));
            child.set_clip_rect(view.intersect(ui.clip_rect()));
            let mut area = egui::ScrollArea::horizontal()
                .id_salt("glass_ribbon_scroll")
                .auto_shrink([false, true])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
            if changed {
                area = area.horizontal_scroll_offset(0.0);
                ui.data_mut(|d| d.remove::<f32>(target_id));
            } else if let Some(goal) = target {
                let x = ui.ctx().animate_value_with_time(anim_id, goal, 0.28);
                area = area.horizontal_scroll_offset(x);
                if (x - goal).abs() < 0.5 {
                    ui.data_mut(|d| d.remove::<f32>(target_id));
                }
            }
            let out = area.show(&mut child, |ui| {
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.add_space(12.0 + 6.0 * (1.0 - ease));
                    ui.multiply_opacity(ease);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        let st = g.app.session.run("format.state", &json!({})).unwrap_or_default();
                        match curated(&tab) {
                            Some(groups) => {
                                for (k, grp) in groups.iter().enumerate() {
                                    group(ui, t, grp.name, k == 0, |ui| {
                                        items(g, ui, t, grp.items, &st);
                                    });
                                }
                            }
                            None => {
                                let groups = registry_groups(&g.app, &tab);
                                if groups.is_empty() {
                                    ui.label(egui::RichText::new("Nothing here yet for this tab.").color(t.text_dim));
                                }
                                for (k, (name, cmds)) in groups.into_iter().enumerate() {
                                    group(ui, t, &name, k == 0, |ui| {
                                        small_columns(ui, |ui, i| {
                                            let (id, label) = cmds.get(i)?;
                                            let icon = wc::command_icon(id);
                                            w::small(ui, &mut g.app, t, icon, label, id, json!({}), false, false);
                                            Some(())
                                        });
                                    });
                                }
                            }
                        }
                        ui.add_space(8.0);
                    });
                });
            });
            // Soft edges where more groups wait off-card, and a thin indicator of where you are.
            let offset = out.state.offset.x;
            let content = out.content_size.x;
            let shown = out.inner_rect.width();
            if content > shown + 1.0 {
                let left = ui.ctx().animate_bool_with_time(egui::Id::new("glass_ribbon_left"), offset > 1.0, 0.15);
                let right = ui.ctx().animate_bool_with_time(egui::Id::new("glass_ribbon_right"), offset < content - shown - 1.0, 0.15);
                let painter = ui.painter().with_clip_rect(card.shrink(1.0));
                // What the glass looks like over the window: the fade blends into it.
                let a = 1.0 - f32::from(t.glass.a()) / 255.0;
                let mix = |g: u8, c: u8| (f32::from(g) + f32::from(c) * a).round().clamp(0.0, 255.0) as u8;
                let base = egui::Color32::from_rgb(mix(t.glass.r(), t.canvas.r()), mix(t.glass.g(), t.canvas.g()), mix(t.glass.b(), t.canvas.b()));
                edge_fade(&painter, Rect::from_min_max(view.min, pos2(view.min.x + 56.0, view.max.y)), base, left, true);
                edge_fade(&painter, Rect::from_min_max(pos2(view.max.x - 56.0, view.min.y), view.max), base, right, false);
                let max = (content - shown).max(0.0);
                for (side, strength) in [(-1.0f32, left), (1.0, right)] {
                    if strength <= 0.01 {
                        continue;
                    }
                    let cx = if side < 0.0 { view.min.x + 14.0 } else { view.max.x - 14.0 };
                    let r = Rect::from_center_size(pos2(cx, card.center().y - 6.0), vec2(26.0, 26.0));
                    let resp = ui.interact(r, egui::Id::new(("glass_ribbon_chevron", side as i32)), Sense::click()).on_hover_text(if side < 0.0 {
                        "Scroll left"
                    } else {
                        "More"
                    });
                    let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
                    painter.add(crate::tokens::glass_shape(r, radius::CAPSULE, t));
                    if hover > 0.0 {
                        painter.rect_filled(r, 13.0, t.hover.gamma_multiply(hover));
                    }
                    let c = r.center();
                    let st = egui::Stroke::new(1.6, t.text.gamma_multiply(strength));
                    painter.line_segment([pos2(c.x - 2.0 * side, c.y - 5.0), pos2(c.x + 2.5 * side, c.y)], st);
                    painter.line_segment([pos2(c.x + 2.5 * side, c.y), pos2(c.x - 2.0 * side, c.y + 5.0)], st);
                    if resp.clicked() {
                        let goal = (offset + side * shown * 0.75).clamp(0.0, max);
                        ui.ctx().animate_value_with_time(anim_id, offset, 0.0);
                        ui.data_mut(|d| d.insert_temp(target_id, goal));
                        ui.ctx().request_repaint();
                    }
                }
                let track_w = (card.width() * 0.18).clamp(60.0, 160.0);
                let track = Rect::from_center_size(pos2(card.center().x, card.max.y - 6.0), vec2(track_w, 3.0));
                let hover = ui.ctx().animate_bool_with_time(egui::Id::new("glass_ribbon_bar"), ui.rect_contains_pointer(card), 0.2);
                if hover > 0.0 {
                    painter.rect_filled(track, 2.0, t.text_dim.gamma_multiply(0.15 * hover));
                    let frac = (shown / content).clamp(0.1, 1.0);
                    let pos = (offset / (content - shown).max(1.0)).clamp(0.0, 1.0);
                    let thumb_w = track_w * frac;
                    let thumb = Rect::from_min_size(pos2(track.min.x + (track_w - thumb_w) * pos, track.min.y), vec2(thumb_w, 3.0));
                    painter.rect_filled(thumb, 2.0, t.text_dim.gamma_multiply(0.55 * hover));
                }
            }
        });
}

/// A horizontal fade from the card's colour to clear, `strength` 0–1.
fn edge_fade(p: &egui::Painter, r: Rect, base: egui::Color32, strength: f32, from_left: bool) {
    if strength <= 0.0 {
        return;
    }
    let solid = base.gamma_multiply(strength);
    let clear = egui::Color32::TRANSPARENT;
    let (l, rgt) = if from_left { (solid, clear) } else { (clear, solid) };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), l);
    mesh.colored_vertex(r.right_top(), rgt);
    mesh.colored_vertex(r.right_bottom(), rgt);
    mesh.colored_vertex(r.left_bottom(), l);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
}

/// Space on each side of the hairline between two groups.
const GROUP_GAP: f32 = 12.0;

/// One group: its controls and, under them, its name. Groups sit directly on the ribbon's glass,
/// separated by a hairline, never boxed inside it.
fn group(ui: &mut Ui, t: &Tokens, name: &str, first: bool, add: impl FnOnce(&mut Ui)) {
    if !first {
        let (r, _) = ui.allocate_exact_size(vec2(2.0 * GROUP_GAP + 1.0, BIG_H + 18.0), Sense::hover());
        ui.painter().vline(r.center().x, (r.min.y + 4.0)..=(r.max.y - 4.0), egui::Stroke::new(1.0, t.edge.gamma_multiply(0.55)));
    }
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
            ui.set_min_height(BIG_H);
            add(ui);
        });
        let w = ui.min_rect().width();
        let (r, _) = ui.allocate_exact_size(vec2(w, 14.0), Sense::hover());
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, name, regular(11.0), t.text_dim);
    });
}

/// Lay out small buttons in columns of three; `f(ui, i)` adds button `i` and returns `None` past
/// the last one.
fn small_columns(ui: &mut Ui, mut f: impl FnMut(&mut Ui, usize) -> Option<()>) {
    let mut i = 0;
    loop {
        let mut done = false;
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
            for _ in 0..3 {
                if f(ui, i).is_none() {
                    done = true;
                    break;
                }
                i += 1;
            }
        });
        if done {
            break;
        }
    }
}

fn is_on(app: &WordApp, st: &Value, on: On) -> bool {
    match on {
        On::No => false,
        On::Fmt(k) => st.get(k).and_then(Value::as_bool).unwrap_or(false),
        On::Session(f) => f(&app.session),
    }
}

/// The controls of a curated group, small buttons stacked three to a column.
fn items(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, list: &'static [Item], st: &Value) {
    let mut i = 0;
    while let Some(item) = list.get(i) {
        match item {
            Item::Small(_) | Item::SmallMenu(..) => {
                // Collect this run of small buttons and stack it.
                let start = i;
                while matches!(list.get(i), Some(Item::Small(_) | Item::SmallMenu(..))) {
                    i += 1;
                }
                let run = list.get(start..i).unwrap_or(&[]);
                small_columns(ui, |ui, k| {
                    let item = run.get(k)?;
                    match item {
                        Item::Small(cmd) => {
                            let on = is_on(&g.app, st, cmd.on);
                            w::small(ui, &mut g.app, t, cmd.icon, cmd.label, cmd.id, params(cmd.params), false, on);
                        }
                        Item::SmallMenu(cmd, choices) => {
                            let resp = w::small(ui, &mut g.app, t, cmd.icon, cmd.label, cmd.id, json!({}), true, false);
                            menu(&mut g.app, &resp, choices);
                        }
                        _ => {}
                    }
                    Some(())
                });
                continue;
            }
            Item::Big(cmd) => {
                let on = is_on(&g.app, st, cmd.on);
                w::big(ui, &mut g.app, t, cmd.icon, cmd.label, cmd.id, params(cmd.params), false, on);
            }
            Item::BigMenu(cmd, choices) => {
                let resp = w::big(ui, &mut g.app, t, cmd.icon, cmd.label, cmd.id, json!({}), true, false);
                menu(&mut g.app, &resp, choices);
            }
            Item::Font => font_group(&mut g.app, ui, t, st),
            Item::Styles => styles_gallery(&mut g.app, ui, t),
            Item::Themes => themes_menu(&mut g.app, ui, t),
            Item::Glass(local, label, icon) => glass_button(g, ui, t, *local, label, icon),
            Item::Icon(cmd) => {
                let on = is_on(&g.app, st, cmd.on);
                toggle(ui, &mut g.app, t, cmd.icon, cmd.label, cmd.id, on);
            }
        }
        i += 1;
    }
}

/// A command menu opened from a ribbon button.
fn menu(app: &mut WordApp, resp: &egui::Response, choices: &[Choice]) {
    egui::Popup::menu(resp).show(|ui| {
        ui.set_min_width(190.0);
        for (label, id, p) in choices {
            let enabled = wc::enabled(app, id) || *id == "ui.dialog";
            if ui.add_enabled(enabled, egui::Button::new(*label)).clicked() {
                w::run(ui, app, id, params(p));
                ui.close();
            }
        }
    });
}

/// Design › Themes: the engine's theme list.
fn themes_menu(app: &mut WordApp, ui: &mut Ui, t: &Tokens) {
    let resp = w::big(ui, app, t, "themes", "Themes", "design.theme", json!({}), true, false);
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(190.0);
        for theme in wordcraft_engine::cmd::design::THEMES {
            if ui.button(theme.0).clicked() {
                w::run(ui, app, "design.theme", json!({"name": theme.0}));
                ui.close();
            }
        }
    });
}

/// View › Writing: the Glass features.
fn glass_button(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, local: Local, label: &str, icon: &str) {
    let on = g.is_on(local);
    let lines: Vec<&str> = label.split('\n').collect();
    let font = regular(11.0);
    let text_w = lines.iter().map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap((*l).to_string(), font.clone(), t.text).size().x)).fold(0.0, f32::max);
    let (r, resp) = ui.allocate_exact_size(vec2((text_w + 14.0).max(50.0), BIG_H), Sense::click());
    crate::tokens::paint_state(ui, r, radius::BUTTON, &resp, on, t);
    let c = if on { t.accent } else { t.text };
    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 19.0), vec2(26.0, 26.0)), icon, c, t.accent);
    let mut y = r.min.y + 37.0;
    for l in lines {
        ui.painter().text(pos2(r.center().x, y), egui::Align2::CENTER_TOP, l, font.clone(), c);
        y += 12.5;
    }
    if local == Local::Sprint {
        // A sprint length to pick, or Stop.
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(170.0);
            if g.glass.sprint.is_some() {
                if ui.button("Stop Sprint").clicked() {
                    g.sprint(None);
                    ui.close();
                }
                ui.separator();
            }
            for m in [10, 15, 25, 45, 60] {
                if ui.button(format!("{m} minutes")).clicked() {
                    g.sprint(Some(m));
                    ui.close();
                }
            }
        });
        return;
    }
    if resp.on_hover_text(label.replace('\n', " ")).clicked() {
        g.toggle(local);
    }
}

/// Home › Font: font and size boxes, then the character toggles.
fn font_group(app: &mut WordApp, ui: &mut Ui, t: &Tokens, st: &Value) {
    let flag = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(false);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
            font_picker(app, ui, t, st);
            size_picker(app, ui, t, st);
            ui.add_space(2.0);
            toggle(ui, app, t, "grow", "Increase Font Size", "format.growFont", false);
            toggle(ui, app, t, "shrink", "Decrease Font Size", "format.shrinkFont", false);
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
            toggle(ui, app, t, "bold", "Bold", "format.bold", flag("bold"));
            toggle(ui, app, t, "italic", "Italic", "format.italic", flag("italic"));
            toggle(ui, app, t, "underline", "Underline", "format.underline", flag("underline"));
            toggle(ui, app, t, "strike", "Strikethrough", "format.strikethrough", flag("strike"));
            toggle(ui, app, t, "subscript", "Subscript", "format.subscript", flag("subscript"));
            toggle(ui, app, t, "superscript", "Superscript", "format.superscript", flag("superscript"));
            ui.add_space(6.0);
            let r = toggle_menu(ui, t, "highlight", "Text Highlight Colour");
            colour_menu(
                app,
                &r,
                "format.highlight",
                &[("Yellow", "yellow"), ("Green", "green"), ("Turquoise", "cyan"), ("Pink", "magenta"), ("No Colour", "none")],
            );
            let r = toggle_menu(ui, t, "fontcolor", "Font Colour");
            colour_menu(
                app,
                &r,
                "format.color",
                &[("Automatic", "auto"), ("Ink Blue", "0A6CDB"), ("Red", "C0392B"), ("Green", "1A7A50"), ("Grey", "6B6B70")],
            );
            toggle(ui, app, t, "clear", "Clear All Formatting", "format.clear", false);
        });
    });
}

/// A pop-up button in the Mac style: one rounded field with its value and a chevron, no boxes
/// inside boxes. Returns its response; the caller attaches the menu.
fn popup_button(ui: &mut Ui, t: &Tokens, text: &str, width: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click());
    ui.painter().rect_filled(r, egui::CornerRadius::same(8), t.paper);
    ui.painter().rect_stroke(r, egui::CornerRadius::same(8), egui::Stroke::new(0.75, t.edge), egui::StrokeKind::Outside);
    crate::tokens::paint_state(ui, r, 8, &resp, false, t);
    let max_chars = ((width - 34.0) / 6.8).max(3.0) as usize;
    let shown: String =
        if text.chars().count() > max_chars { format!("{}…", text.chars().take(max_chars).collect::<String>()) } else { text.to_string() };
    ui.painter().text(pos2(r.min.x + 10.0, r.center().y), egui::Align2::LEFT_CENTER, shown, regular(13.0), t.text);
    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.max.x - 14.0, r.center().y), vec2(12.0, 12.0)), "chevronDown", t.text_dim, t.text_dim);
    resp
}

/// The font menu: a search field over the families, each shown in its own face.
fn font_picker(app: &mut WordApp, ui: &mut Ui, t: &Tokens, st: &Value) {
    let font = st.get("font").and_then(Value::as_str).unwrap_or("").to_string();
    let shown = app.previews.font_shown(&font);
    let resp = popup_button(ui, t, if shown.is_empty() { "Font" } else { &shown }, 168.0).on_hover_text("Font");
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_width(286.0);
        let qid = egui::Id::new("glass_font_query");
        let mut q: String = ui.data(|d| d.get_temp(qid)).unwrap_or_default();
        let field = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Search fonts").desired_width(270.0).margin(vec2(8.0, 6.0)));
        if field.changed() {
            ui.data_mut(|d| d.insert_temp(qid, q.clone()));
        }
        if !field.has_focus() && q.is_empty() {
            field.request_focus();
        }
        ui.add_space(4.0);
        let fams = app.previews.families();
        let prev = app.previews.font_preview_fn();
        let mut chosen: Option<String> = None;
        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            for i in wc::font_matches(&q, &fams) {
                let Some(name) = fams.get(i) else { continue };
                if prev(ui, name).clicked() {
                    chosen = Some(name.clone());
                }
            }
        });
        ui.separator();
        if ui.button("Add Font…").clicked() {
            chosen = Some(wc::ADD_FONT.to_string());
        }
        if let Some(f) = chosen {
            if f == wc::ADD_FONT {
                w::run(ui, app, "font.add", json!({}));
            } else {
                let f = app.previews.font_stored(&f);
                w::run(ui, app, "format.font", json!({"name": f}));
            }
            ui.data_mut(|d| d.remove::<String>(qid));
            ui.close();
        }
    });
}

/// The size menu: the standard sizes.
fn size_picker(app: &mut WordApp, ui: &mut Ui, t: &Tokens, st: &Value) {
    let size = st.get("size").and_then(Value::as_f64).map(|s| if s.fract() == 0.0 { format!("{s:.0}") } else { format!("{s}") }).unwrap_or_default();
    let resp = popup_button(ui, t, &size, 62.0).on_hover_text("Font Size");
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(80.0);
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for s in wordcraft_engine::cmd::format::SIZES {
                let label = if s.fract() == 0.0 { format!("{s:.0}") } else { s.to_string() };
                if ui.selectable_label(label == size, label.as_str()).clicked() {
                    w::run(ui, app, "format.size", json!({"size": s}));
                    ui.close();
                }
            }
        });
    });
}

/// A 26×24 icon toggle that runs a command.
fn toggle(ui: &mut Ui, app: &mut WordApp, t: &Tokens, icon: &str, tip: &str, id: &str, on: bool) {
    let enabled = wc::enabled(app, id);
    let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    if enabled {
        crate::tokens::paint_state(ui, r, 8, &resp, on, t);
    }
    let c = if !enabled {
        t.text_disabled
    } else if on {
        t.accent
    } else {
        t.text
    };
    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(16.0, 16.0)), icon, c, t.accent);
    let sc = wc::shortcut_text(app, id);
    let resp = resp.on_hover_text(if sc.is_empty() { tip.to_string() } else { format!("{tip} ({sc})") });
    wc::command_menu(&resp, app, id);
    if resp.clicked() && enabled {
        w::run(ui, app, id, json!({}));
    }
}

/// A 26×24 icon button that opens a menu.
fn toggle_menu(ui: &mut Ui, t: &Tokens, icon: &str, tip: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    crate::tokens::paint_state(ui, r, 8, &resp, false, t);
    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(16.0, 16.0)), icon, t.text, t.accent);
    resp.on_hover_text(tip)
}

fn colour_menu(app: &mut WordApp, resp: &egui::Response, id: &str, colours: &[(&str, &str)]) {
    egui::Popup::menu(resp).show(|ui| {
        ui.set_min_width(160.0);
        for (label, v) in colours {
            if ui.button(*label).clicked() {
                w::run(ui, app, id, json!({"color": v}));
                ui.close();
            }
        }
    });
}

/// Home › Styles: preview cards for the everyday styles.
fn styles_gallery(app: &mut WordApp, ui: &mut Ui, t: &Tokens) {
    const STYLES: [(&str, &str, &str); 5] = [
        ("Normal", "para.normal", ""),
        ("Title", "para.style", r#"{"style": "Title"}"#),
        ("Subtitle", "para.style", r#"{"style": "Subtitle"}"#),
        ("Heading 1", "para.heading1", ""),
        ("Heading 2", "para.heading2", ""),
    ];
    let current = app.session.run("para.state", &json!({})).ok().and_then(|v| v.get("style").and_then(Value::as_str).map(str::to_string));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(5.0, 0.0);
        for (label, id, p) in STYLES {
            let (r, resp) = ui.allocate_exact_size(vec2(62.0, BIG_H - 4.0), Sense::click());
            let r = r.translate(vec2(0.0, 2.0));
            let selected = current.as_deref().is_some_and(|s| s.replace(' ', "").eq_ignore_ascii_case(&label.replace(' ', "")));
            let card = egui::CornerRadius::same(10);
            ui.painter().rect_filled(r, card, t.paper);
            let stroke = if selected {
                egui::Stroke::new(2.0, t.accent)
            } else if resp.hovered() {
                egui::Stroke::new(1.0, t.text_dim)
            } else {
                egui::Stroke::new(0.75, t.edge)
            };
            ui.painter().rect_stroke(r, card, stroke, egui::StrokeKind::Outside);
            let (font, color) = match label {
                "Title" => (egui::FontId::new(19.0, egui::FontFamily::Proportional), t.text),
                "Subtitle" => (regular(14.0), t.text_dim),
                "Heading 1" => (semibold(15.0), t.text),
                "Heading 2" => (semibold(13.5), t.accent),
                _ => (regular(14.0), t.text),
            };
            ui.painter().text(pos2(r.min.x + 9.0, r.min.y + 20.0), egui::Align2::LEFT_CENTER, "Aa Bb", font, color);
            ui.painter().text(pos2(r.min.x + 9.0, r.max.y - 12.0), egui::Align2::LEFT_CENTER, label, medium(10.5), t.text_dim);
            if resp.on_hover_text(label).clicked() {
                w::run(ui, app, id, params(p));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curated_command_exists() {
        let s = Session::new(wordcraft_doc::Document::new());
        for id in curated_ids() {
            assert!(id.starts_with("ui.") || s.registry.get(id).is_some(), "unknown command {id}");
        }
    }

    #[test]
    fn every_curated_params_literal_parses() {
        for tab in ["Home", "Insert", "Draw", "Design", "Layout", "References", "Mailings", "Review", "View", "Help", "Writer"] {
            for g in curated(tab).unwrap_or(&[]) {
                for item in g.items {
                    let mut lits: Vec<&str> = Vec::new();
                    match item {
                        Item::Big(c) | Item::Small(c) => lits.push(c.params),
                        Item::BigMenu(c, ch) | Item::SmallMenu(c, ch) => {
                            lits.push(c.params);
                            lits.extend(ch.iter().map(|x| x.2));
                        }
                        _ => {}
                    }
                    for l in lits.into_iter().filter(|l| !l.is_empty()) {
                        assert!(serde_json::from_str::<Value>(l).is_ok(), "bad params {l}");
                    }
                }
            }
        }
    }
}

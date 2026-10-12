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
use crate::tokens::{Tokens, interaction_fill, paint_fill, radius, space};
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
    Group { name: "Styles", items: &[Item::Styles, Small(c("Styles Pane", "view.stylesPane", "stylesPane"))] },
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
        ],
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
        _ => return None,
    })
}

/// Every command id the curated tabs use (tests check they all exist).
pub fn curated_ids() -> Vec<&'static str> {
    let mut ids = Vec::new();
    for tab in ["Home", "Insert", "Draw", "Design", "Layout", "References", "Mailings", "Review", "View", "Help"] {
        for g in curated(tab).unwrap_or(&[]) {
            for item in g.items {
                match item {
                    Item::Big(c) | Item::Small(c) => ids.push(c.id),
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
pub fn tabs(app: &WordApp) -> Vec<String> {
    let mut v: Vec<String> = vec!["File".into()];
    for e in app.session.ribbon.tabs().iter().filter(|e| !e.hidden) {
        v.push(e.name.clone());
    }
    for ct in wordcraft_ui_egui::ribbon::contextual_tabs(&app.session) {
        if !v.iter().any(|t| t == ct) {
            v.push(ct.to_string());
        }
    }
    v
}

/// The ribbon card for the current tab.
pub fn show(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let tab = g.app.ui.tab.clone();
    egui::Panel::top("glass_ribbon")
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin { left: 10, right: 10, top: 4, bottom: 6 }))
        .show(ui, |ui| {
            let bg = ui.painter().add(egui::Shape::Noop);
            let limit = ui.max_rect();
            let inner = egui::Frame::NONE.inner_margin(space::RIBBON_PAD).show(ui, |ui| {
                ui.set_width(ui.available_width());
                // The tabs, as one segmented control.
                let names = tabs(&g.app);
                let labels: Vec<&str> = names.iter().map(String::as_str).collect();
                let sel = names.iter().position(|n| *n == tab).unwrap_or(1);
                ui.vertical_centered(|ui| {
                    if let Some(i) = w::segmented(ui, t, &labels, sel, 32.0)
                        && let Some(name) = names.get(i)
                    {
                        if name == "File" {
                            g.app.ui.backstage = true;
                        } else {
                            g.app.ui.tab = name.clone();
                        }
                    }
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    let st = g.app.session.run("format.state", &json!({})).unwrap_or_default();
                    match curated(&tab) {
                        Some(groups) => {
                            for grp in groups {
                                group(ui, t, grp.name, |ui| {
                                    items(g, ui, t, grp.items, &st);
                                });
                            }
                        }
                        None => {
                            let groups = registry_groups(&g.app, &tab);
                            if groups.is_empty() {
                                ui.label(egui::RichText::new("Nothing here yet for this tab.").color(t.text_dim));
                            }
                            for (name, cmds) in groups {
                                group(ui, t, &name, |ui| {
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
                });
            });
            // Never wider than the window: a group that doesn't fit wraps to the next row.
            let r = inner.response.rect;
            let r = Rect::from_min_max(r.min, pos2(r.max.x.min(limit.max.x), r.max.y));
            let shapes = crate::tokens::glass_shape(r, radius::RIBBON, t);
            ui.painter().set(bg, shapes);
        });
}

/// One group: a quiet rounded panel with its controls and, under them, its name.
fn group(ui: &mut Ui, t: &Tokens, name: &str, add: impl FnOnce(&mut Ui)) {
    // Groups are wrapped whole: if this one was wider last frame than the room left on the row,
    // start a new row first.
    let wid = egui::Id::new(("glass_group_width", name));
    let last: Option<f32> = ui.data(|d| d.get_temp(wid));
    if last.is_some_and(|w| w > ui.available_width() + 0.5) {
        ui.end_row();
    }
    let bg = ui.painter().add(egui::Shape::Noop);
    let resp = egui::Frame::NONE.inner_margin(egui::Margin { left: 6, right: 6, top: 6, bottom: 4 }).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                ui.set_min_height(BIG_H);
                add(ui);
            });
            let w = ui.min_rect().width();
            let (r, _) = ui.allocate_exact_size(vec2(w, 14.0), Sense::hover());
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, name, regular(10.5), t.text_dim);
        });
    });
    let r = resp.response.rect;
    ui.painter().set(bg, egui::Shape::rect_filled(r, egui::CornerRadius::same(radius::GROUP), t.fill));
    if last.is_none_or(|w| (w - r.width()).abs() > 0.5) {
        ui.data_mut(|d| d.insert_temp(wid, r.width()));
        ui.ctx().request_repaint();
    }
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
    let on = match local {
        Local::Typewriter => g.glass.prefs.typewriter,
        Local::Snapshots => g.glass.panel == Some(crate::Panel::Snapshots),
        Local::History => g.glass.panel == Some(crate::Panel::History),
        Local::Goals => g.glass.goal_open,
    };
    let lines: Vec<&str> = label.split('\n').collect();
    let font = regular(11.0);
    let text_w = lines.iter().map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap((*l).to_string(), font.clone(), t.text).size().x)).fold(0.0, f32::max);
    let (r, resp) = ui.allocate_exact_size(vec2((text_w + 14.0).max(50.0), BIG_H), Sense::click());
    if let Some(fill) = interaction_fill(&resp, on, t) {
        paint_fill(ui.painter(), r, radius::BUTTON, fill);
    }
    let c = if on { t.accent } else { t.text };
    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 19.0), vec2(26.0, 26.0)), icon, c, t.accent);
    let mut y = r.min.y + 37.0;
    for l in lines {
        ui.painter().text(pos2(r.center().x, y), egui::Align2::CENTER_TOP, l, font.clone(), c);
        y += 12.5;
    }
    if resp.on_hover_text(label.replace('\n', " ")).clicked() {
        g.toggle(local);
    }
}

/// Home › Font: font and size boxes, then the character toggles.
fn font_group(app: &mut WordApp, ui: &mut Ui, t: &Tokens, st: &Value) {
    let flag = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(false);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 6.0);
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
            let font = st.get("font").and_then(Value::as_str).unwrap_or("").to_string();
            let font = app.previews.font_shown(&font);
            let fams = app.previews.families();
            let prev = app.previews.font_preview_fn();
            if let Some(f) = wc::font_combo(ui, "glass_font", 148.0, &font, &fams, Some(&*prev)) {
                if f == wc::ADD_FONT {
                    w::run(ui, app, "font.add", json!({}));
                } else {
                    let f = app.previews.font_stored(&f);
                    w::run(ui, app, "format.font", json!({"name": f}));
                }
            }
            let size =
                st.get("size").and_then(Value::as_f64).map(|s| if s.fract() == 0.0 { format!("{s:.0}") } else { format!("{s}") }).unwrap_or_default();
            let sizes: Vec<String> =
                wordcraft_engine::cmd::format::SIZES.iter().map(|s| if s.fract() == 0.0 { format!("{s:.0}") } else { s.to_string() }).collect();
            if let Some(v) = wc::combo(ui, "glass_size", 50.0, &size, &sizes, None)
                && let Ok(x) = v.trim().parse::<f64>()
            {
                w::run(ui, app, "format.size", json!({"size": x}));
            }
            toggle(ui, app, t, "grow", "Increase Font Size", "format.growFont", false);
            toggle(ui, app, t, "shrink", "Decrease Font Size", "format.shrinkFont", false);
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(1.0, 0.0);
            toggle(ui, app, t, "bold", "Bold", "format.bold", flag("bold"));
            toggle(ui, app, t, "italic", "Italic", "format.italic", flag("italic"));
            toggle(ui, app, t, "underline", "Underline", "format.underline", flag("underline"));
            toggle(ui, app, t, "strike", "Strikethrough", "format.strikethrough", flag("strike"));
            toggle(ui, app, t, "subscript", "Subscript", "format.subscript", flag("subscript"));
            toggle(ui, app, t, "superscript", "Superscript", "format.superscript", flag("superscript"));
            ui.add_space(4.0);
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

/// A 26×24 icon toggle that runs a command.
fn toggle(ui: &mut Ui, app: &mut WordApp, t: &Tokens, icon: &str, tip: &str, id: &str, on: bool) {
    let enabled = wc::enabled(app, id);
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    if enabled && let Some(fill) = interaction_fill(&resp, on, t) {
        paint_fill(ui.painter(), r, 8, fill);
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
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    if let Some(fill) = interaction_fill(&resp, false, t) {
        paint_fill(ui.painter(), r, 8, fill);
    }
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
            let (r, resp) = ui.allocate_exact_size(vec2(66.0, BIG_H - 4.0), Sense::click());
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
        for tab in ["Home", "Insert", "Draw", "Design", "Layout", "References", "Mailings", "Review", "View", "Help"] {
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

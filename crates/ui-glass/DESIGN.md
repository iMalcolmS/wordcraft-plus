# WordCraft Glass — design spec

Glass is a second front end for the WordCraft engine. It keeps the Word workflow (File, Home,
Insert, Draw, Design, Layout, References, Mailings, Review, View, Help) and dresses it in a calm,
Apple-style Liquid Glass look: content first, controls floating above it on glass.

The code lives in `crates/ui-glass` (the UI) and `apps/wordcraft-glass` (the desktop app). The
classic UI in `crates/ui-egui` is not modified; Glass reuses its public building blocks (the page
canvas, dialogs, Backstage, keyboard handling) and draws its own chrome around them.

## Principles

1. **Content first.** The page is the only bright, opaque thing on screen. Everything else is
   quieter: glass panels, soft greys, thin icons.
2. **Floating controls.** The title bar, ribbon, panels and status bar float on the canvas with a
   10 pt gap from the window edge and each other. Nothing is a full-width band with a hard border.
3. **One accent, used sparingly.** System blue marks only what matters: the selection, the current
   page or heading, toggled buttons, primary actions (Share, Take Snapshot, Restore), progress.
4. **Concentric corners.** A shape nested inside another has the parent's radius minus the padding
   between them, so the curves stay parallel.
5. **Everything is a command.** Every button runs a WordCraft command id through `WordApp::run`,
   so shortcuts, undo, dialogs for missing input and the Quick Access Toolbar menu keep working.

## Tokens (`src/tokens.rs`)

| Token | Light | Dark |
|---|---|---|
| Canvas | `#E6E7EB` | `#121214` |
| Glass fill | white at 30–92 % (Glass setting) | `#2E2E33` at 42–92 % |
| Glass rim (highlight) | white 90 % | white 12 % |
| Glass edge (darkened) | black 13 % | black 59 % |
| Quiet fill (groups, tracks) | black 4 % | white 6 % |
| Hover / pressed | black 7 % / 12 % | white 10 % / 16 % |
| Text / secondary / disabled | `#1D1D1F` / `#5F5F65` / `#A4A4AA` | `#F2F2F4` / `#A6A6AC` / `#66666C` |
| Accent | `#0A6CDB` | `#3B8EF0` |
| Accent soft (selected rows) | accent at 13 % | accent at 19 % |

| Radius | Value |
|---|---|
| Capsules (title-bar items, tabs, status bar, buttons) | height / 2 |
| Ribbon card | 26 |
| Ribbon group (26 − 8 padding) | 18 |
| Ribbon button (18 − 6 padding) | 12 |
| Side panels, popovers | 24 |
| Rows and cards in panels | 12–14 |

Spacing: 10 pt window edge and gaps between glass, 6 pt between items, 8 pt ribbon padding,
6 pt group padding. Type: the UI font (Inter) at 12.5 pt for controls, 11 pt for ribbon labels,
10.5 pt for group names; 13 pt semibold for the document name.

### The glass material

egui has no background blur, so glass is layered: a soft shadow (0, 6, blur 24), a translucent
fill whose opacity follows the **Glass** setting (clear → tinted, like iOS 27's slider), a bright
1 pt inner rim (the specular highlight) and a 0.75 pt darker outer edge (iOS 27's darkened edge,
for contrast against busy content). **Reduce Transparency** swaps it for a solid surface with a
stronger outline.

## Layout

- **Document tabs** — the top line (beside the traffic lights on macOS), as in Notion or Safari:
  a Home tab, one tab per open document (title, unsaved dot, close button), and +. Each tab keeps
  its own document, undo, selection, scroll and versions; closing an edited tab asks first, and
  quitting asks about every edited tab. ⌘T new tab, ⌃Tab / ⌃⇧Tab move between tabs.
- **Home** — where the app starts with no document: greeting, today's goal ring, the week's bars,
  new documents from templates (paper tiles that lift on hover), recent files, the Writer Pro
  switch and Appearance.
- **Toolbar** — one line, like a Mac unified toolbar: the Quick Access capsule on the left; the tabs
  as a glass segmented control in the middle (the selected tab's pill glides to the tab you pick;
  Developer and Zotero sit behind "More"); the writing tools capsule, search and Share on the right.
- **Tabs** — File, Home, Insert, Layout, References, Review, View (and Writer with Writer Pro);
  Draw, Design, Mailings, Developer, Zotero and Help under **More**, which shows the open one's
  name. Contextual tabs (Table, Picture, Equation…) appear as needed.
- **Ribbon (Word tabs)** — WordCraft's own ribbon, complete (every group, gallery, split button
  and dialog launcher, the full Styles gallery with Clear Formatting, Alt key tips), set on the
  glass card: its tab strip is left out and its opaque backgrounds cleared. The toolbar's
  controls live on their own layer, so nothing beneath can take their clicks.
- **Ribbon (Writer tab)** — one glass card, one row. Groups sit directly on it, separated by hairlines, never
  boxed inside it; each group's name sits under its controls. When the window is too narrow the
  row scrolls sideways (trackpad, wheel or drag), with soft fades and glass chevrons at the edges
  that glide it by three quarters of its width, and a thin position bar on hover. Groups never
  wrap to a second row. Large buttons (26 pt icon over label) for main actions, small ones (16 pt icon +
  label) stacked three to a column, icon toggles 28 × 28.
- **File** — a glass menu under the File tab (New, Open, Open Recent, Save, Export, Print, Info,
  Settings), not a full-window takeover. Templates, Print, Info and Settings open the full File view.
- **Navigation** (View › Navigation Pane, or click "Page x of y" in the status capsule) — a glass
  card on the left, laid out like Word's: a row of icon tabs (Pages, Headings, Comments, Search)
  with a gliding pill and a close button. Pages shows real thumbnails; Comments lists every
  comment with its anchor (click to go there); Search finds every match as you type and shows it
  with the text around it.
- **Ribbon fields** — font and size pickers are one box each: the value and a chevron inside a
  single rounded field with a quiet hairline border that takes the accent on hover or focus, 4 pt
  apart. Thin vertical rules separate the size buttons, Change Case, the clear and effect
  buttons, as in Word's Font group. (This is a change to WordCraft's own `ui-egui` widgets, so
  the classic interface gets it too.)
- **Colour menus** — the swatches, Recent Colors, then **Custom Color ▸**: a hue/saturation wheel
  and a brightness bar open right in the menu, with a swatch, the hex value and Apply. The menu
  stays open while you use the wheel. More Colors… opens the full Colors dialog, whose Custom tab
  uses the same wheel.
- **Canvas** — the pages on the canvas colour.
- **Versions** — a glass card on the right: Snapshots | History.
- **Status capsule** — centred at the bottom: goal ring, writing time, page, words, view buttons,
  zoom, Focus.

## Motion

Everything moves, briefly and quietly; nothing snaps.

| What | Animation |
|---|---|
| Hover on any control | fill fades in/out, 120 ms |
| Toggled state | accent tint eases in, 180 ms |
| Tab change | selected pill glides (ease-out quart, restarted from where it is), 320 ms; the new tab's controls fade in and slide 18 pt from the side of the tab you came from, 220 ms ease-out |
| Navigation pane tabs | the pill glides between the icon tabs |
| Block handle and drop line | glide to the paragraph under the pointer instead of jumping |
| Ribbon overflow | chevron glides the row, 280 ms; edge fades ease in/out, 150 ms |
| Document or view switch | the new content fades in, 220 ms ease-out |
| Switches | the knob slides and the track tints, 180 ms |
| `[[` and `/` menus | fade in at the caret, 120 ms; the highlight follows ↑ ↓ at 80 ms |
| Sprint | the ring drains as the time runs |
| Focus mode | ribbon and panels slide away and back |
| Navigation and Versions panels | slide in from and out to their edge |
| File menu, goal popover | fade and rise into place, 140–160 ms |
| Typewriter mode | the page glides the caret's line towards the middle; the fade follows the caret, 140 ms, with soft edges in the paper's own colour |
| Status messages | fade in, stay, fade out over their last second |

Fonts: the UI font (Inter) at 13 pt for controls and menus, 11 pt for group names and secondary
text, 13 pt semibold for the document name; font and size pickers are single rounded pop-up
buttons with a chevron, Mac style.

## Tabs

The main tabs are curated in `src/ribbon.rs` (fewer, larger targets than the classic ribbon; each
entry is a real command id, checked by a test). Contextual tabs (Table Design, Table Layout,
Picture Format, Shape Format, Chart Design, SmartArt Design, Equation, Outlining) and Developer and
Zotero are built from the command registry, grouped by each command's ribbon location, so nothing
is missing. File opens the classic Backstage.

## Writing features

| Feature | Where | How it works |
|---|---|---|
| Writing goals | Status ring, View › Writing | Daily word target; net words written today and this session, minutes writing (edits with pauses under a minute), streak, last seven days. Saved in `glass.json`. |
| Typewriter mode | Title bar, View › Writing | Fades the page outside a band around the caret. |
| Snapshots | Title bar, View › Writing | Named copies of the document (`file.versions` save), with a preview of what changed and Restore (undoable). |
| Version History | Title bar, View › Writing | An automatic version every 10 minutes while the document changes, plus snapshots, grouped by day. Automatic versions make room for snapshots in the engine's 50-version list. |
| Focus | Status capsule, View › Immersive | `view.focus`: hides the ribbon and panes. |
| Split View | Title bar (disabled) | Planned; needs two canvases on one document. |

## Panels

Navigation, Binder, Versions and Writer Pro's side panel share one shell (`src/dock.rs`): a glass
card that slides in from its edge. Drag its inner edge to make it narrower or wider (a grip fades
in; each panel has a minimum and maximum, and the width is saved); double-click the edge to
reset. With **Floating panels** on, panels float over the page as frosted glass — the page keeps
its full width and stays visible around and faintly beneath them. (egui has no blur, so the frost
is a dense tint with a sheen, not a real blur.)

Navigation › Pages shows real page thumbnails from the engine's renderer, as many columns as fit.
Hovering a document tab shows a live thumbnail of the page that document is on, with its page
count, words and state.

## On the page (`src/overlay.rs`)

| Feature | How it works |
|---|---|
| Block handles (everyone; Appearance › Block handles) | A grip beside the paragraph under the pointer. Drag: the paragraph fades back, a frosted copy trails the pointer, an accent line glides to where it will land, the page scrolls near its edges; drop moves it (`para.moveUp/Down`, undoable). Click: selects the paragraph. Escape cancels. |
| Reader's Glow | Off / Paragraph / Sentence. Everything but the lit text fades back; the lit band glides between paragraphs and sentences (animated in page coordinates, so scrolling never lags it). |
| Pulse | A paragraph glows for a moment when its style changes (by any route) or when it lands after a drag. |
| Rhythm marks | A thin bar in the left margin per paragraph: green, amber when sentences average over 22 words (or 150+ words), red over 30 (or 220+). Hover for the figures. Words of four letters or more repeated within ten words, or three times in a paragraph, get a gradient underline (English and Portuguese stop words ignored). |
| Layout guides | The text area's margins, a centre line and a 12 pt baseline grid. |

## Writer Pro, the Scrivener way (`src/scriv.rs`)

- **Binder** — Manuscript as a tree: sections with children are folders, the rest documents,
  with disclosure triangles; open documents; Characters, Places and Research folders (+ on the
  row, click to open in the split); Scratchpad. ⌥-click or right-click a section for Quick
  Reference.
- **Editor header** — over the page: ‹ › through the sections visited, the section's name and
  status, ⌃ ⌄ previous/next section, the split and the Inspector.
- **Inspector** — the section's synopsis (white card), notes (cream card), label and status.
- **Split** — a reference pane that opens on a **document**: this document's pages, or any other
  open tab's, rendered at the pane's width (click a page of this document to go there; "Open in
  Editor" switches to another tab). The header menu also offers Manuscript (sections with
  synopses), Characters, Places and Research (items with two lines of synopsis, the selected one
  open beneath to edit).
- **Quick Reference** — a section in a small floating window to read while writing; ‹ › walk the
  sections; drag it by its title.
- **Section boxes** — on the page, each heading in a soft box tinted with its label, a dashed rule
  between sections (Writer › Section Boxes).
- **Block menu** — click a block's ⠿ handle: Turn into (Text, Heading 1–3, Quote, lists, To-do),
  **Turn into page** (a heading takes its section: it moves to a new document in its own tab and a
  page link takes its place; click the link to open the page; Undo brings it back), Quick
  Reference, Duplicate, Move up/down, Select, Delete.

## Appearance

View › More Appearance, File › Appearance… or Home: Light / Dark / Automatic, seven accent colours,
four window tones (Neutral, Warm, Cool, Paper), the glass tint and Reduce Transparency, interface
size (S/M/L), animations (System/On/Off), dark page, Typewriter mode, floating panels and Reset
Panel Sizes, Reader's Glow, rhythm marks, layout guides, block handles, and Writer Pro. Everything
applies live and is saved.

## Writer Pro

A switch (toolbar, Home, Appearance) that adds the **Writer** tab and these tools — a mix of
Scrivener, Notion and Obsidian on a Word document. Everything stays a normal .docx.

| Tool | How it works |
|---|---|
| Binder | Left panel: the manuscript's headings (status dot, click to go), open documents, Characters / Places / Research notes (+ to add, click to edit on the right), Scratchpad. |
| Corkboard | Sections as index cards (status chip, title, synopsis or first sentence, words). Drag a card to move its whole section in the document (`outline.moveUp/Down`); click the chip to cycle To Do → Draft → Revised → Done; double-click to open. Chapters or All Sections. |
| Outliner | Sections as rows: title (indented by level), status, words, editable synopsis. |
| `[[` links | Type `[[` and a menu of headings filters as you type; picking one replaces the text with an ordinary hyperlink to a hidden bookmark on that heading (`_wl_…`), so ⌘-click follows it and Word reads it. |
| `/` commands | `/` at the start of a paragraph: Text, Heading 1–3, lists, To-do, Quote, Divider, Table, Image, Equation, Page break, Footnote, Comment. Initials work (`/h2`, `/pb`). |
| Scratchpad | Cut to Scratchpad removes the selection but keeps it, dated, with where it came from; insert it back, copy or delete. |
| Pacing | Per section: sentence length and variety, filter words and passive voice per 1,000 words, dialogue share (English and Portuguese), as a heatmap, plus the document's most used filter words. |
| Sprint | 10–60 minute writing sprint; the status capsule shows time left on a draining ring and words written; a status message sums it up at the end. |

Card statuses, synopses, notes and scraps are kept per document in `glass-projects.json` next to
the preferences (they are not written into the .docx).

## The workspace: Docs, Databases, Page Layout

- **Home** leads with the three pillars (Document, Database, Page Layout cards, each with its
  templates), then featured templates and the workspace's databases.
- **Choose a Template** (`src/chooser.rs`, `src/templates.rs`): an opaque sheet; a sidebar of
  categories with glyphs; sections with 150 pt page thumbnails rendered by the engine (two per
  frame, cached); database templates drawn as a small picture of their records; click to
  select, double-click or Enter to create, Esc to close. All templates are our own text and
  design; pictures in them are drawn in code (`templates::draw`).
- **Databases** (`src/db.rs` model, `src/db_view.rs` page, `src/workspace.rs` files and tabs):
  a database opens in its own tab (a glyph before its name) and saves itself a second after the
  last change, as JSON (`.wcdb`) in `Documents/WordCraft/Databases`. Records open as `.docx`
  pages in `<database> pages/`. Hostile files are capped (rows, properties, cell size, file
  size) and repaired on open. Views keep filters, sorts, grouping and hidden properties; option
  colours are ten soft tints.
- **Page Layout** (`src/format.rs`): the Format inspector docks on the right (Style, Arrange,
  Text); with nothing selected it adds text boxes, shapes, your pictures and drawn pictures,
  each floated in front of the text on the page. Every change is a WordCraft command.
- **Colour labels** (`src/labels.rs`): per project, a name, one of the ten tints and a glyph;
  shown in the Binder, on Corkboard cards (a band and the glyph), on the page (section boxes)
  and on notes. The older fixed labels still show.
- **Split**: beside the page (the side dock) or below it (a resizable bottom panel); Swap trades
  the open document and the one in the split.
- **Dictionary & Thesaurus** (`src/dict.rs`): in the right-hand panel's place. Spelling comes
  from WordCraft's proofing (`wordcraft_proof`): English and Polish built in, and on macOS every
  language of the system spell checker through `proof::external` (installed by the app, main
  thread only). Synonyms come from MyThes thesauri (`proof::thes`) found in the dictionaries
  folders and LibreOffice's dictionary extensions; English falls back to a small built-in list.

## Research mode

- A mode like Writer Pro (`prefs.research`), with its own toolbar pill, Home card and **Research**
  ribbon tab (`ribbon::RESEARCH`, Glass's own big buttons). Its panels are `Panel::Research(RView)`
  in the side dock; the Binder and Scratchpad are shared with Writer Pro.
- The Binder in Research mode is `src/rbinder.rs` (unless Writer Pro is on and its Writer tab
  is showing): Paper (sections with word counts, drag to reorder, missing usual parts folded
  into one row), Figures & Tables, Equations, Code, Sources, Notes, Reviews, Scratchpad. It
  shares the row drawing and the manuscript tree (`scriv::manuscript`) with Writer Pro's.
- `src/refs.rs`: the reference model and readers (BibTeX, RIS, CSL-JSON, EndNote XML), the
  Library (`Documents/WordCraft/Library/library.json`, linked files, Zotero's local API).
  Citations are WordCraft's own `CITATION` fields, so Word and the bibliography styles work.
- `src/research.rs`: Library panel, citation check (cached per revision), claim marks and hover
  cards drawn by `overlay::show`.
- `src/formulas.rs`: the formula library and typing shortcuts (once per revision, after the
  app's own keys). `src/figures.rs`: SEQ captions renumbered when out of order; tables and charts
  linked to files through a `wcplus-data:` tag in their alt text.
- `src/codeblocks.rs`: a code block is a one-cell table tagged `wcplus-code:<lang>#<id>`, its
  output another tagged `wcplus-output:<id>`; runs happen on a thread with a timeout.
- `src/graphs.rs` (citation graph, argument map), `src/submit.rs` (blind copy, LaTeX zip,
  submission PDF, compile), `src/reviews.rs` (response log and letter), `src/academic.rs`
  (templates).

## Known limits

- Versions live in memory with the document and are lost when it closes.
- Version times are shown in UTC (the engine records UTC).
- Typewriter centring scrolls through the canvas's wheel input, so it works while the pointer
  rests over the page.
- Magnetic guides while dragging pictures and text boxes need a hook in the canvas (object drags
  are private to `ui-egui`); the layout guides are shown, but objects don't snap to them yet.
- Collapsing headings happens inside the canvas, so it isn't animated yet.
- Block handles move top-level paragraphs; paragraphs in table cells keep their own order.
- Not yet in Writer Pro: an AI continuity checker and institutional template checks.
- Research mode: Mendeley and EndNote come in through their export files (RIS, BibTeX, EndNote
  XML); Zotero's sync needs Zotero 7 with "Allow other applications to communicate" on. Code
  runs with the interpreters on the computer (MATLAB code runs in Octave when MATLAB isn't
  installed). Linked Excel files are read as their first sheet. Argument roles come from cue
  words and can be set by hand. The LaTeX export keeps charts as their alt text.
- Databases have no formulas, relations or rollups yet, and no sharing; a record's page is a
  separate `.docx`, not stored inside the database.
- Spelling in languages other than English and Polish needs the Mac's spell checker; on Linux
  and Windows those languages show no spelling marks yet (WordCraft's Hunspell reader doesn't
  read compounding or long-flag dictionaries). Definitions are looked up online (Wiktionary) or
  in the Mac's Dictionary app, not shown in the panel.
- The split shows the other document's pages (click to go there, Swap to edit it); a second
  live editor needs two canvases.
- On Linux desktops without a file-chooser portal or zenity, the classic app falls back to its own
  file browser; Glass can't show it yet (`file_browser::show` is crate-private in `ui-egui`).

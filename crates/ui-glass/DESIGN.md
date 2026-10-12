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

- **Title band** (macOS) — beside the traffic lights, only the document's name and its state
  (Edited / Saved / Not saved), centred, like any Mac window. Elsewhere the system title bar shows it.
- **Toolbar** — one line, like a Mac unified toolbar: the Quick Access capsule on the left; the tabs
  as a glass segmented control in the middle (the selected tab's pill glides to the tab you pick;
  Developer and Zotero sit behind "More"); the writing tools capsule, search and Share on the right.
- **Ribbon** — one glass card. Groups sit directly on it, separated by hairlines, never boxed
  inside it; each group's name sits under its controls. A group that doesn't fit wraps whole to
  the next row. Large buttons (26 pt icon over label) for main actions, small ones (16 pt icon +
  label) stacked three to a column, icon toggles 28 × 28.
- **File** — a glass menu under the File tab (New, Open, Open Recent, Save, Export, Print, Info,
  Settings), not a full-window takeover. Templates, Print, Info and Settings open the full File view.
- **Navigation** (View › Navigation Pane) — a glass card on the left: Headings | Pages.
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
| Tab change | selected pill glides, 240 ms; the new tab's controls fade in, 200 ms ease-out |
| Focus mode | ribbon and panels slide away and back |
| Navigation and Versions panels | slide in from and out to their edge |
| File menu, goal popover | fade and rise into place, 140–160 ms |
| Typewriter mode | the veil fades in and out, 250 ms |
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

## Known limits

- Versions live in memory with the document and are lost when it closes.
- Version times are shown in UTC (the engine records UTC).
- Typewriter mode fades but doesn't yet keep the caret line centred (needs a scroll hook in the
  canvas).
- Alt key tips don't draw badges on the Glass ribbon.
- On Linux desktops without a file-chooser portal or zenity, the classic app falls back to its own
  file browser; Glass can't show it yet (`file_browser::show` is crate-private in `ui-egui`).

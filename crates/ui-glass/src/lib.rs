//! WordCraft Glass: a front end for the WordCraft engine in a calm, Liquid Glass style.
//!
//! Glass wraps the egui front end's [`WordApp`] (the document session, the page canvas, dialogs,
//! keyboard handling, Backstage and file dialogs) and replaces the window chrome around it: the
//! title bar, the ribbon, the navigation pane and the status bar are drawn here, as floating
//! glass over the canvas. Nothing in `wordcraft-ui-egui` is changed; Glass only uses its public
//! building blocks, so the original UI keeps working exactly as before.
//!
//! On top of that, Glass adds writing features: daily goals and session stats, Typewriter mode,
//! Snapshots and Version History; document tabs and a Home page; and Writer Pro (Binder,
//! Corkboard, Outliner, notes, `[[links]]`, `/` commands, Scratchpad, pacing and sprints). See
//! `DESIGN.md` for the design rules.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod analysis;
pub mod docs;
pub mod goals;
pub mod inline;
pub mod project;
pub mod ribbon;
pub mod tokens;

mod appearance;
mod chrome;
mod dock;
mod home;
pub mod overlay;
mod panels;
mod pro;
mod thumbs;
mod typewriter;
mod widgets;

use serde::{Deserialize, Serialize};
use serde_json::json;
use wordcraft_ui_egui::{WordApp, i18n, now_ms};

use crate::ribbon::Local;
use crate::tokens::{Material, Tokens};

/// The label automatic history versions are saved with; anything else is a snapshot the writer
/// took.
pub const AUTO_LABEL: &str = "Auto-save";
/// How often an automatic version is kept while the document changes (seconds).
pub const AUTO_EVERY_SECS: f64 = 600.0;
/// The engine keeps at most this many versions; automatic ones make room for snapshots.
const MAX_VERSIONS: usize = 50;

/// The side panel on the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Snapshots,
    History,
    /// Writer Pro: cut text kept aside.
    Scratchpad,
    /// Writer Pro: the pacing heatmap.
    Pacing,
    /// Writer Pro: a character, place or research note.
    Note(u64),
}

impl Panel {
    /// Snapshots and History share the versions panel; the rest are Writer Pro's side panel.
    pub fn is_versions(self) -> bool {
        matches!(self, Panel::Snapshots | Panel::History)
    }
}

/// What fills the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// The start page: today's writing, templates, recent files.
    Home,
    /// The document's pages.
    #[default]
    Page,
    /// Writer Pro: the sections as index cards.
    Corkboard,
    /// Writer Pro: the sections as a table.
    Outliner,
}

/// A writing sprint: write against the clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprint {
    pub started: f64,
    pub secs: f64,
    /// Words written today when it started.
    pub words_at_start: i64,
}

/// Glass preferences, saved next to the app's own (`glass.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlassPrefs {
    pub material: Material,
    pub typewriter: bool,
    pub goals: goals::Goals,
    /// Writer Pro's tools are on.
    pub pro: bool,
    /// Writer Pro's Binder is showing.
    pub binder: bool,
    /// Side panel widths the writer chose, by panel.
    pub docks: std::collections::BTreeMap<String, f32>,
    /// Side panels float over the page as frosted glass instead of sitting beside it.
    pub floating: bool,
    /// Reader's Glow: dim everything but the paragraph or sentence being written.
    pub glow: overlay::Glow,
    /// Margin marks for dense paragraphs and underlines for repeated words.
    pub rhythm: bool,
    /// Drag handles beside each paragraph.
    pub handles: bool,
    /// Layout guides over the pages: margins, columns and a baseline grid.
    pub grid: bool,
}

impl Default for GlassPrefs {
    fn default() -> Self {
        GlassPrefs {
            material: Material::default(),
            typewriter: false,
            goals: goals::Goals::default(),
            pro: false,
            binder: true,
            docks: Default::default(),
            floating: false,
            glow: overlay::Glow::Off,
            rhythm: false,
            handles: true,
            grid: false,
        }
    }
}

/// Glass's own state.
#[derive(Default)]
pub struct GlassState {
    pub prefs: GlassPrefs,
    pub panel: Option<Panel>,
    pub goal_open: bool,
    /// The version selected in the Snapshots or History panel.
    pub selected_version: Option<usize>,
    /// The name typed for the next snapshot.
    pub snapshot_name: String,
    pub tracker: goals::Tracker,
    /// When the last automatic version was kept, and the revision it saw.
    last_auto: Option<f64>,
    auto_rev: u64,
    /// Where the goal ring and the status capsule were drawn (the popover sits above them).
    pub(crate) ring_rect: Option<egui::Rect>,
    pub(crate) status_rect: Option<egui::Rect>,
    /// The File menu, and the File tab it hangs from.
    pub file_menu: bool,
    pub file_anchor: Option<egui::Rect>,
    pub(crate) recent_pick: Option<String>,
    /// The ribbon tab last shown, and when it changed (for the cross-fade).
    pub(crate) shown_tab: String,
    pub(crate) tab_changed_at: f64,
    /// The versions panel last open, kept while it slides closed.
    pub(crate) last_panel: Option<Panel>,
    /// The Writer Pro side panel last open, kept while it slides closed.
    pub(crate) last_side: Option<Panel>,
    /// What fills the window, and the document view to come back to from Home.
    pub view: View,
    pub doc_view: View,
    pub appearance_open: bool,
    /// Writer Pro: notes, scraps and card details, per document.
    pub projects: project::Projects,
    /// The Corkboard shows every heading level rather than chapters only.
    pub board_all: bool,
    /// Pacing figures for a (document, revision).
    pub(crate) pacing_cache: ((u64, u64), Vec<(project::Section, analysis::Metrics)>),
    pub sprint: Option<Sprint>,
    /// The `[[` and `/` menus.
    pub inline: inline::Inline,
    /// Block handles, Reader's Glow, pulses and rhythm marks on the page.
    pub overlay: overlay::Overlay,
    /// Page thumbnails (Navigation › Pages, tab previews).
    pub(crate) thumbs: thumbs::Thumbs,
}

/// The Glass application: the WordCraft app plus the Glass chrome and features.
pub struct GlassApp {
    pub app: WordApp,
    pub glass: GlassState,
    /// The open documents.
    pub docs: docs::Docs,
    frames: u32,
    sent_title: String,
}

impl GlassApp {
    /// A window on `app`'s document. It starts on Home when that document is an untouched blank
    /// one, and on the page otherwise.
    pub fn new(app: WordApp) -> Self {
        let mut g = GlassApp { app, glass: GlassState::default(), docs: docs::Docs::default(), frames: 0, sent_title: String::new() };
        let s = &g.app.session;
        if s.path.is_none() && !s.dirty && s.word_count() == 0 {
            g.glass.view = View::Home;
        }
        g
    }

    /// Writer Pro's notes and scraps as JSON, to save (`glass-projects.json`).
    pub fn projects_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(&self.glass.projects).unwrap_or_default()
    }

    /// Restore saved notes and scraps (anything unreadable is ignored).
    pub fn load_projects(&mut self, bytes: &[u8]) {
        if let Ok(p) = serde_json::from_slice::<project::Projects>(bytes) {
            self.glass.projects = p;
        }
    }

    /// Turn Writer Pro on or off. Off puts away its views, panels and tab.
    pub fn set_pro(&mut self, on: bool) {
        self.glass.prefs.pro = on;
        if on {
            if self.glass.view != View::Home {
                self.app.ui.tab = "Writer".into();
            }
            self.app.status("Writer Pro is on: Binder, Corkboard, Outliner, [[links]] and / commands");
            return;
        }
        if matches!(self.glass.view, View::Corkboard | View::Outliner) {
            self.glass.view = View::Page;
        }
        if matches!(self.glass.doc_view, View::Corkboard | View::Outliner) {
            self.glass.doc_view = View::Page;
        }
        if self.glass.panel.is_some_and(|p| !p.is_versions()) {
            self.glass.panel = None;
        }
        if self.app.ui.tab == "Writer" {
            self.app.ui.tab = "Home".into();
        }
        self.glass.sprint = None;
    }

    /// Show a document view (Page, Corkboard or Outliner).
    pub fn set_view(&mut self, v: View) {
        if self.glass.view != v {
            self.docs.switched_at = now_ms() / 1000.0;
        }
        self.glass.view = v;
        if v != View::Home {
            self.glass.doc_view = v;
        }
    }

    /// Start a writing sprint of `minutes`, or stop the one running.
    pub fn sprint(&mut self, minutes: Option<u32>) {
        match minutes {
            Some(m) => {
                let today = Self::today();
                let words_at_start = self.glass.prefs.goals.today(&today).words;
                self.glass.sprint = Some(Sprint { started: now_ms() / 1000.0, secs: f64::from(m.clamp(1, 240)) * 60.0, words_at_start });
            }
            None => self.glass.sprint = None,
        }
    }

    /// Words written in the running sprint.
    pub fn sprint_words(&self) -> i64 {
        let today = Self::today();
        self.glass.sprint.map(|s| self.glass.prefs.goals.today(&today).words - s.words_at_start).unwrap_or(0)
    }

    /// Whether a Glass feature is on (for its button).
    pub fn is_on(&self, local: Local) -> bool {
        let g = &self.glass;
        match local {
            Local::Typewriter => g.prefs.typewriter,
            Local::Snapshots => g.panel == Some(Panel::Snapshots),
            Local::History => g.panel == Some(Panel::History),
            Local::Goals => g.goal_open,
            Local::Page => g.view == View::Page,
            Local::Corkboard => g.view == View::Corkboard,
            Local::Outliner => g.view == View::Outliner,
            Local::Binder => g.prefs.binder,
            Local::Scratchpad => g.panel == Some(Panel::Scratchpad),
            Local::CutToScratchpad => false,
            Local::Pacing => g.panel == Some(Panel::Pacing),
            Local::Sprint => g.sprint.is_some(),
            Local::Appearance => g.appearance_open,
            Local::Glow => g.prefs.glow != overlay::Glow::Off,
            Local::Rhythm => g.prefs.rhythm,
            Local::Guides => g.prefs.grid,
            Local::Handles => g.prefs.handles,
        }
    }

    /// The preferences as JSON, to save.
    pub fn prefs_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(&self.glass.prefs).unwrap_or_default()
    }

    /// Apply saved preferences (anything unreadable is ignored).
    pub fn load_prefs(&mut self, bytes: &[u8]) {
        if let Ok(p) = serde_json::from_slice::<GlassPrefs>(bytes) {
            self.glass.prefs = p;
        }
    }

    /// Today's date (`YYYY-MM-DD`, UTC).
    pub fn today() -> String {
        goals::date_of((now_ms() / 1000.0) as i64)
    }

    /// Turn a Glass feature on or off.
    pub fn toggle(&mut self, local: Local) {
        let flip = |cur: Option<Panel>, p: Panel| if cur == Some(p) { None } else { Some(p) };
        match local {
            Local::Typewriter => self.glass.prefs.typewriter = !self.glass.prefs.typewriter,
            Local::Snapshots => self.glass.panel = flip(self.glass.panel, Panel::Snapshots),
            Local::History => self.glass.panel = flip(self.glass.panel, Panel::History),
            Local::Goals => self.glass.goal_open = !self.glass.goal_open,
            Local::Page => self.set_view(View::Page),
            Local::Corkboard => self.set_view(View::Corkboard),
            Local::Outliner => self.set_view(View::Outliner),
            Local::Binder => self.glass.prefs.binder = !self.glass.prefs.binder,
            Local::Scratchpad => self.glass.panel = flip(self.glass.panel, Panel::Scratchpad),
            Local::CutToScratchpad => pro::cut_to_scratchpad(self),
            Local::Pacing => self.glass.panel = flip(self.glass.panel, Panel::Pacing),
            Local::Sprint => self.sprint(if self.glass.sprint.is_some() { None } else { Some(25) }),
            Local::Appearance => self.glass.appearance_open = !self.glass.appearance_open,
            Local::Glow => {
                use overlay::Glow;
                self.glass.prefs.glow = match self.glass.prefs.glow {
                    Glow::Off => Glow::Paragraph,
                    Glow::Paragraph => Glow::Sentence,
                    Glow::Sentence => Glow::Off,
                };
                let name = match self.glass.prefs.glow {
                    Glow::Off => "off",
                    Glow::Paragraph => "on the paragraph",
                    Glow::Sentence => "on the sentence",
                };
                self.app.status(format!("Reader's Glow {name}"));
            }
            Local::Rhythm => self.glass.prefs.rhythm = !self.glass.prefs.rhythm,
            Local::Guides => self.glass.prefs.grid = !self.glass.prefs.grid,
            Local::Handles => self.glass.prefs.handles = !self.glass.prefs.handles,
        }
    }

    /// Per-frame work before layout: the app's own, then goals and automatic versions.
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.app.logic(ctx);
        let now = now_ms() / 1000.0;
        let words = self.app.cached_word_count();
        let (doc, rev) = (self.app.session.document_id(), self.app.session.rev());
        let today = Self::today();
        self.glass.tracker.tick(&mut self.glass.prefs.goals, &today, doc, words, rev, now);
        self.auto_version(now);
        if let Some(sp) = self.glass.sprint
            && now - sp.started >= sp.secs
        {
            let words = self.sprint_words();
            self.glass.sprint = None;
            self.app.status(format!("Sprint done: {} words in {} minutes", words.max(0), (sp.secs / 60.0).round()));
        }
        // Softer, rounder menus and windows to match the glass.
        ctx.all_styles_mut(|s| {
            s.visuals.window_corner_radius = egui::CornerRadius::same(16);
            s.visuals.menu_corner_radius = egui::CornerRadius::same(12);
            s.visuals.window_shadow = egui::epaint::Shadow { offset: [0, 10], blur: 30, spread: 0, color: egui::Color32::from_black_alpha(40) };
            s.visuals.popup_shadow = egui::epaint::Shadow { offset: [0, 6], blur: 20, spread: 0, color: egui::Color32::from_black_alpha(32) };
            for w in [&mut s.visuals.widgets.inactive, &mut s.visuals.widgets.hovered, &mut s.visuals.widgets.active, &mut s.visuals.widgets.open] {
                w.corner_radius = egui::CornerRadius::same(8);
            }
        });
    }

    /// Keep an automatic version every [`AUTO_EVERY_SECS`] while the document changes, dropping
    /// the oldest automatic one when the engine's list is nearly full, so snapshots are kept.
    fn auto_version(&mut self, now: f64) {
        let last = *self.glass.last_auto.get_or_insert(now);
        let rev = self.app.session.rev();
        if now - last < AUTO_EVERY_SECS || rev == self.glass.auto_rev {
            return;
        }
        self.glass.last_auto = Some(now);
        self.glass.auto_rev = rev;
        let versions = &mut self.app.session.versions;
        if versions.len() + 1 >= MAX_VERSIONS
            && let Some(i) = versions.iter().position(|(l, _, _)| l == AUTO_LABEL)
        {
            versions.remove(i);
            self.glass.selected_version = None;
        }
        if let Err(e) = self.app.session.run("file.versions", &json!({"save": AUTO_LABEL})) {
            log::warn!("automatic version: {e}");
        }
    }

    pub fn raw_input_hook(&mut self, raw: &mut egui::RawInput) {
        self.app.raw_input_hook(raw);
        inline::take_keys(self, raw);
    }

    /// ⌘T opens a new tab; ⌃Tab and ⌃⇧Tab move between tabs.
    fn tab_shortcuts(&mut self, ctx: &egui::Context) {
        let (new_tab, next, prev) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::T),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::Tab),
                i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Tab),
            )
        });
        if new_tab {
            self.new_tab(wordcraft_engine::Session::new(wordcraft_engine::doc::Document::new()));
        }
        let n = self.docs.tabs.len();
        if n > 1 && (next || prev) {
            let i = if next { (self.docs.active + 1) % n } else { (self.docs.active + n - 1) % n };
            self.switch_to(i);
        }
    }

    /// A document: toolbar, ribbon, panels, and the page (or the Corkboard or Outliner).
    fn document(&mut self, ui: &mut egui::Ui, t: &Tokens, focus: bool) {
        use wordcraft_ui_egui::{canvas, panes};
        let ctx = ui.ctx().clone();
        chrome::toolbar(self, ui, t);
        // The ribbon, panels and panes slide away in Focus mode and back when it ends.
        ribbon::show(self, ui, t, !focus);
        chrome::status_bar(self, ui, t);
        match self.glass.panel {
            Some(p) if p.is_versions() => self.glass.last_panel = Some(p),
            Some(p) => self.glass.last_side = Some(p),
            None => {}
        }
        let versions = self.glass.panel.is_some_and(Panel::is_versions);
        let side = self.glass.panel.is_some_and(|p| !p.is_versions()) && self.glass.prefs.pro;
        let nav = self.app.session.view.nav_pane;
        let pro = self.glass.prefs.pro;
        if !focus {
            // The original side panes (Styles, Comments, Clipboard…), except Navigation,
            // which Glass draws itself.
            self.app.session.view.nav_pane = false;
            panes::show(&mut self.app, ui);
            self.app.session.view.nav_pane = nav;
        }
        // Where floating panels go: what's left under the toolbar and ribbon.
        let area = ui.available_rect_before_wrap();
        dock::show(self, ui, t, &dock::VERSIONS, versions && !focus, area, |g, ui| panels::versions(g, ui, t));
        dock::show(self, ui, t, &dock::SIDE, side && !focus, area, |g, ui| pro::side(g, ui, t));
        dock::show(self, ui, t, &dock::BINDER, pro && self.glass.prefs.binder && !focus, area, |g, ui| pro::binder(g, ui, t));
        dock::show(self, ui, t, &dock::NAVIGATION, nav && !focus && !(pro && self.glass.prefs.binder), area, |g, ui| panels::navigation(g, ui, t));
        let view = self.glass.view;
        let typewriter = ctx.animate_bool_with_time(egui::Id::new("glass_typewriter_fade"), self.glass.prefs.typewriter && view == View::Page, 0.25);
        // A document or view switch fades the new content in.
        let since = now_ms() / 1000.0 - self.docs.switched_at;
        let switch = ((since / 0.22) as f32).clamp(0.0, 1.0);
        if switch < 1.0 {
            ctx.request_repaint();
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin { left: 0, right: 0, top: 6, bottom: 0 }))
            .show(ui, |ui| {
                ui.multiply_opacity(1.0 - (1.0 - switch).powi(3));
                match view {
                    View::Corkboard if self.glass.prefs.pro => pro::corkboard(self, ui, t),
                    View::Outliner if self.glass.prefs.pro => pro::outliner(self, ui, t),
                    _ => {
                        overlay::before_canvas(self, ui);
                        canvas::show(&mut self.app, ui);
                        overlay::show(self, ui, t);
                        if typewriter > 0.0 {
                            typewriter::overlay(&mut self.app, ui, typewriter);
                        }
                    }
                }
            });
        if view == View::Page {
            inline::show(self, &ctx, t);
        } else {
            self.glass.inline.open = false;
        }
    }

    /// Lay out the window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        use wordcraft_ui_egui::{backstage, colors, content_controls, dialogs, keys, keytips, read_aloud, scroll, zotero};
        let ctx = ui.ctx().clone();
        i18n::set_current(i18n::Lang::from_pref(&self.app.ui.language));
        if !self.app.animations_on() {
            let opts = ctx.options(|o| o.input_options);
            ctx.input_mut(|i| i.smooth_scroll_delta = scroll::unsmoothed_scroll(i, &opts));
        }
        // Fonts installed by `logic` take effect on the next frame.
        if self.frames < 2 {
            self.frames += 1;
            ctx.request_repaint();
            return;
        }
        let t = Tokens::new(self.app.ui_is_dark(), self.glass.prefs.material);
        keytips::logic(&mut self.app, &ctx);
        self.tab_shortcuts(&ctx);
        self.glass.thumbs.budget = thumbs::PER_FRAME;
        if self.app.ui.backstage {
            backstage::show(&mut self.app, ui);
        } else {
            let focus = self.app.session.view.focus_mode;
            let view = self.glass.view;
            docs::strip(self, ui, &t);
            if view == View::Home {
                egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.canvas)).show(ui, |ui| {
                    let since = now_ms() / 1000.0 - self.docs.switched_at;
                    let p = ((since / 0.22) as f32).clamp(0.0, 1.0);
                    if p < 1.0 {
                        ui.ctx().request_repaint();
                    }
                    ui.multiply_opacity(1.0 - (1.0 - p).powi(3));
                    home::show(self, ui, &t);
                });
            } else {
                self.document(ui, &t, focus);
            }
            appearance::show(self, &ctx, &t);
            chrome::file_menu(self, &ctx, &t);
            if view != View::Home {
                chrome::goal_popover(self, &ctx, &t);
            }
            docs::confirm_close(self, &ctx, &t);
        }
        content_controls::popup(&mut self.app, &ctx);
        dialogs::show(&mut self.app, &ctx);
        colors::show(&mut self.app, &ctx);
        zotero::show_alert(&mut self.app, &ctx);
        read_aloud::show(&mut self.app, &ctx);
        keytips::show(&mut self.app, &ctx, ui);
        keys::global_shortcuts(&mut self.app, &ctx);
        if let Some(url) = self.app.canvas.open_url.take() {
            ctx.open_url(egui::OpenUrl::new_tab(url));
        }
        let title = format!("{}{} - WordCraft Glass", self.app.title_stem(), if self.app.session.dirty { " •" } else { "" });
        if title != self.sent_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.sent_title = title;
        }
    }
}

//! WordCraft Glass: a front end for the WordCraft engine in a calm, Liquid Glass style.
//!
//! Glass wraps the egui front end's [`WordApp`] (the document session, the page canvas, dialogs,
//! keyboard handling, Backstage and file dialogs) and replaces the window chrome around it: the
//! title bar, the ribbon, the navigation pane and the status bar are drawn here, as floating
//! glass over the canvas. Nothing in `wordcraft-ui-egui` is changed; Glass only uses its public
//! building blocks, so the original UI keeps working exactly as before.
//!
//! On top of that, Glass adds writing features: daily goals and session stats, Typewriter mode,
//! Snapshots and Version History. See `DESIGN.md` for the design rules.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod goals;
pub mod ribbon;
pub mod tokens;

mod chrome;
mod panels;
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
}

/// Glass preferences, saved next to the app's own (`glass.json`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlassPrefs {
    pub material: Material,
    pub typewriter: bool,
    pub goals: goals::Goals,
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
    /// The side panel last open, kept while it slides closed.
    pub(crate) last_panel: Option<Panel>,
}

/// The Glass application: the WordCraft app plus the Glass chrome and features.
pub struct GlassApp {
    pub app: WordApp,
    pub glass: GlassState,
    frames: u32,
    sent_title: String,
}

impl GlassApp {
    pub fn new(app: WordApp) -> Self {
        GlassApp { app, glass: GlassState::default(), frames: 0, sent_title: String::new() }
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
        match local {
            Local::Typewriter => self.glass.prefs.typewriter = !self.glass.prefs.typewriter,
            Local::Snapshots => self.glass.panel = if self.glass.panel == Some(Panel::Snapshots) { None } else { Some(Panel::Snapshots) },
            Local::History => self.glass.panel = if self.glass.panel == Some(Panel::History) { None } else { Some(Panel::History) },
            Local::Goals => self.glass.goal_open = !self.glass.goal_open,
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
    }

    /// Lay out the window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        use wordcraft_ui_egui::{backstage, canvas, colors, content_controls, dialogs, keys, keytips, panes, read_aloud, scroll, zotero};
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
        if self.app.ui.backstage {
            backstage::show(&mut self.app, ui);
        } else {
            let focus = self.app.session.view.focus_mode;
            chrome::title_band(self, ui, &t);
            chrome::toolbar(self, ui, &t);
            // The ribbon, panels and panes slide away in Focus mode and back when it ends.
            ribbon::show(self, ui, &t, !focus);
            chrome::status_bar(self, ui, &t);
            if let Some(p) = self.glass.panel {
                self.glass.last_panel = Some(p);
            }
            panels::versions(self, ui, &t, self.glass.panel.is_some());
            let nav = self.app.session.view.nav_pane;
            panels::navigation(self, ui, &t, nav && !focus);
            if !focus {
                // The original side panes (Styles, Comments, Clipboard…), except Navigation,
                // which Glass draws itself.
                self.app.session.view.nav_pane = false;
                panes::show(&mut self.app, ui);
                self.app.session.view.nav_pane = nav;
            }
            let typewriter = ctx.animate_bool_with_time(egui::Id::new("glass_typewriter_fade"), self.glass.prefs.typewriter, 0.25);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin { left: 0, right: 0, top: 6, bottom: 0 }))
                .show(ui, |ui| {
                    canvas::show(&mut self.app, ui);
                    if typewriter > 0.0 {
                        typewriter::overlay(&mut self.app, ui, typewriter);
                    }
                });
            chrome::file_menu(self, &ctx, &t);
            chrome::goal_popover(self, &ctx, &t);
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

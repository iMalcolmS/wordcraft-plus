//! The Glass ribbon: one floating glass card under the toolbar.
//!
//! The Word tabs (Home, Insert, Layout, References, Review, View, and Draw, Design, Mailings,
//! Developer, Zotero and Help under More, plus the contextual tabs) are WordCraft's own ribbon,
//! complete, set on the glass. Writer Pro's tab is Glass's own: one row of large buttons that
//! scrolls sideways when the window is narrow.

use egui::{Rect, Sense, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_ui_egui::theme::regular;
use wordcraft_ui_egui::{WordApp, icons};

use crate::GlassApp;
use crate::tokens::{Tokens, radius};
use crate::widgets::{self as w, BIG_H};

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
    Inspector,
    Split,
    /// Scrivenings: headings boxed, sections ruled off.
    Sections,
}

pub enum Item {
    /// A Glass feature.
    Glass(Local, &'static str, &'static str),
    /// View › Focus.
    Focus,
    /// Research mode: a tool (action, label, icon).
    Research(RAct, &'static str, &'static str),
    /// An engine command, run as the classic ribbon runs it (dialogs included): icon, label, id.
    Cmd(&'static str, &'static str, &'static str),
}

/// Research mode's tools.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RAct {
    Library,
    Check,
    HoverCards,
    Sections,
    Scratchpad,
    Formulas,
    Code,
    Data,
    Graph,
    Argument,
    Submit,
    BlindCopy,
    ImportReview,
    CommentsToLog,
    ResponseLetter,
}

const RESEARCH: &[Group] = &[
    Group {
        name: "Citations",
        items: &[
            Item::Research(RAct::Library, "Library", "sources"),
            Item::Cmd("toc", "Bibliography", "references.bibliography"),
            Item::Research(RAct::Check, "Citation\nCheck", "checkErrors"),
            Item::Research(RAct::HoverCards, "Hover\nCards", "researcher"),
        ],
    },
    Group {
        name: "Writing",
        items: &[
            Item::Research(RAct::Sections, "Sections", "navPane"),
            Item::Research(RAct::Scratchpad, "Scratchpad", "paste"),
            Item::Research(RAct::Formulas, "Formulas", "equation"),
            Item::Research(RAct::Code, "Code", "macros"),
        ],
    },
    Group {
        name: "Figures",
        items: &[
            Item::Cmd("caption", "Caption", "references.caption"),
            Item::Cmd("crossRef", "Cross-\nreference", "insert.crossReference"),
            Item::Research(RAct::Data, "Linked\nData", "chart"),
            Item::Cmd("tableOfFigures", "Table of\nFigures", "references.tableOfFigures"),
        ],
    },
    Group {
        name: "Insight",
        items: &[Item::Research(RAct::Graph, "Citation\nGraph", "share"), Item::Research(RAct::Argument, "Argument\nMap", "outline")],
    },
    Group {
        name: "Submit",
        items: &[Item::Research(RAct::BlindCopy, "Blind\nCopy", "protect"), Item::Research(RAct::Submit, "Export &\nCompile", "save")],
    },
    Group {
        name: "Reviews",
        items: &[
            Item::Research(RAct::ImportReview, "Import\nReview", "open"),
            Item::Research(RAct::CommentsToLog, "Comments\nto Log", "comment"),
            Item::Research(RAct::ResponseLetter, "Response\nLetter", "mailMerge"),
        ],
    },
];

/// The Research tab's groups.
pub fn research_groups() -> &'static [Group] {
    RESEARCH
}

pub struct Group {
    pub name: &'static str,
    pub items: &'static [Item],
}

const WRITER: &[Group] = &[
    Group {
        name: "Views",
        items: &[
            Item::Glass(Local::Page, "Page", "printLayout"),
            Item::Glass(Local::Corkboard, "Corkboard", "multiplePages"),
            Item::Glass(Local::Outliner, "Outliner", "outline"),
        ],
    },
    Group {
        name: "Project",
        items: &[
            Item::Glass(Local::Binder, "Binder", "navPane"),
            Item::Glass(Local::Inspector, "Inspector", "help"),
            Item::Glass(Local::Split, "Split", "split"),
            Item::Glass(Local::Sections, "Section\nBoxes", "outline"),
        ],
    },
    Group {
        name: "Draft",
        items: &[
            Item::Glass(Local::CutToScratchpad, "Cut to\nScratchpad", "cut"),
            Item::Glass(Local::Scratchpad, "Scratchpad", "paste"),
            Item::Glass(Local::Typewriter, "Typewriter", "focus"),
            Item::Focus,
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

/// The Writer tab's groups.
pub fn writer_groups() -> &'static [Group] {
    WRITER
}

/// The tabs shown, in order: File, the main tabs, then the contextual ones for the selection.
pub fn tabs(app: &WordApp, pro: bool, research: bool) -> Vec<String> {
    let mut v: Vec<String> = vec!["File".into()];
    for e in app.session.ribbon.tabs().iter().filter(|e| !e.hidden) {
        v.push(e.name.clone());
        if e.name == "Home" {
            if pro {
                v.push("Writer".into());
            }
            if research {
                v.push("Research".into());
            }
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
    // Every Word tab is WordCraft's own ribbon, complete — every group, gallery, menu and
    // split button — set on the glass. Only Writer Pro's tab is drawn here.
    let research = g.app.ui.tab == "Research" && g.glass.prefs.research;
    if g.app.ui.tab != "Writer" && !research {
        classic(g, ui, t, expanded);
        return;
    }
    let groups = if research { RESEARCH } else { WRITER };
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
                        for (k, grp) in groups.iter().enumerate() {
                            group(ui, t, grp.name, k == 0, |ui| {
                                items(g, ui, t, grp.items);
                            });
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

/// WordCraft's ribbon for the current tab, on the glass card: its own tab strip is left out (Glass
/// has its own in the toolbar) and its opaque backgrounds are cleared, so its groups sit on the
/// glass like Glass's own. Every button, menu, gallery and split button works exactly as in the
/// classic app, Alt key tips included.
fn classic(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, expanded: bool) {
    use wordcraft_ui_egui::widgets::{CONTENT_H, LABEL_H};
    let ribbon_h = CONTENT_H + LABEL_H + 10.0;
    // The classic tab strip's height: laid out above the card, where it is clipped away.
    let strip = 30.0;
    let now = ui.input(|i| i.time);
    if g.glass.shown_tab != g.app.ui.tab {
        // The new tab's groups slide in from the side the tab is on.
        let order = tabs(&g.app, g.glass.prefs.pro, g.glass.prefs.research);
        let from = order.iter().position(|n| *n == g.glass.shown_tab);
        let to = order.iter().position(|n| *n == g.app.ui.tab);
        g.glass.tab_dir = match (from, to) {
            (Some(a), Some(b)) if b < a => -1.0,
            _ => 1.0,
        };
        g.glass.shown_tab = g.app.ui.tab.clone();
        g.glass.tab_changed_at = now;
    }
    let p = (((now - g.glass.tab_changed_at) / 0.22) as f32).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - p).powi(3);
    if p < 1.0 {
        ui.ctx().request_repaint();
    }
    let mut open = expanded && !g.app.ui.ribbon_collapsed;
    egui::Panel::top("glass_ribbon")
        .show_separator_line(false)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.canvas).inner_margin(egui::Margin { left: 14, right: 14, top: 0, bottom: 10 }))
        .show_collapsible(ui, &mut open, |ui| {
            let full = ui.max_rect();
            let card = Rect::from_min_size(full.min, vec2(full.width(), ribbon_h + 12.0));
            ui.painter().add(crate::tokens::glass_shape(card, radius::RIBBON, t));
            let _ = ui.allocate_exact_size(card.size(), Sense::hover());
            let inner = card.shrink2(vec2(16.0, 5.0));
            if ui.rect_contains_pointer(inner) {
                ui.ctx().input_mut(|i| {
                    if i.smooth_scroll_delta.x.abs() < 0.5 && i.smooth_scroll_delta.y.abs() > 0.0 && !i.modifiers.command {
                        i.smooth_scroll_delta.x = i.smooth_scroll_delta.y;
                        i.smooth_scroll_delta.y = 0.0;
                    }
                });
            }
            let slide = g.glass.tab_dir * 18.0 * (1.0 - ease);
            let host = Rect::from_min_max(pos2(inner.min.x + slide, inner.min.y - strip - 3.0), pos2(inner.max.x + slide, inner.max.y));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(host).layout(egui::Layout::top_down(egui::Align::Min)));
            child.set_clip_rect(inner.intersect(ui.clip_rect()));
            child.multiply_opacity(ease);
            let layer = child.layer_id();
            let start = ui.ctx().graphics_mut(|gr| gr.entry(layer).next_idx().0);
            wordcraft_ui_egui::ribbon::show(&mut g.app, &mut child);
            let tk = wordcraft_ui_egui::theme::Tokens::get(ui.ctx());
            ui.ctx().graphics_mut(|gr| {
                let list = gr.entry(layer);
                let end = list.next_idx().0;
                for i in start..end {
                    list.mutate_shape(egui::layers::ShapeIdx(i), |cs| {
                        clear_background(&mut cs.shape, &tk);
                        // The classic tab strip sits above the card: never shown.
                        cs.clip_rect = cs.clip_rect.intersect(inner);
                    });
                }
            });
        });
}

/// Clear the classic ribbon's opaque fills and borders, so it shows the glass beneath.
fn clear_background(shape: &mut egui::Shape, tk: &wordcraft_ui_egui::theme::Tokens) {
    match shape {
        egui::Shape::Rect(r) if r.fill == tk.ribbon || r.fill == tk.tab_strip => {
            r.fill = egui::Color32::TRANSPARENT;
            r.stroke = egui::Stroke::NONE;
        }
        egui::Shape::Vec(v) => {
            for s in v {
                clear_background(s, tk);
            }
        }
        _ => {}
    }
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

/// The Writer tab's buttons.
fn items(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, list: &'static [Item]) {
    for item in list {
        match item {
            Item::Glass(local, label, icon) => glass_button(g, ui, t, *local, label, icon),
            Item::Focus => {
                let on = g.app.session.view.focus_mode;
                w::big(ui, &mut g.app, t, "focus", "Focus", "view.focus", json!({}), false, on);
            }
            Item::Cmd(icon, label, id) => {
                w::big(ui, &mut g.app, t, icon, label, id, json!({}), false, false);
            }
            Item::Research(act, label, icon) => research_button(g, ui, t, *act, label, icon),
        }
    }
}

impl GlassApp {
    /// Whether a Research tool is showing.
    pub fn research_on(&self, act: RAct) -> bool {
        use crate::research::RView;
        let panel = |v: RView| self.glass.panel == Some(crate::Panel::Research(v));
        match act {
            RAct::Library => panel(RView::Library),
            RAct::Check => panel(RView::Check),
            RAct::HoverCards => self.glass.prefs.hover_cards,
            RAct::Sections => self.glass.prefs.binder,
            RAct::Scratchpad => self.glass.panel == Some(crate::Panel::Scratchpad),
            RAct::Formulas => panel(RView::Formulas),
            RAct::Code => panel(RView::Code),
            RAct::Data => panel(RView::Data),
            RAct::Graph => panel(RView::Graph),
            RAct::Argument => panel(RView::Argument),
            RAct::Submit => panel(RView::Submit),
            RAct::BlindCopy | RAct::ImportReview | RAct::CommentsToLog | RAct::ResponseLetter => false,
        }
    }

    /// Use a Research tool.
    pub fn research_act(&mut self, act: RAct) {
        use crate::research::RView;
        match act {
            RAct::Library => self.research_panel(RView::Library),
            RAct::Check => self.research_panel(RView::Check),
            RAct::HoverCards => {
                self.glass.prefs.hover_cards = !self.glass.prefs.hover_cards;
                self.app.status(if self.glass.prefs.hover_cards { "Hover cards on: rest the pointer on a citation" } else { "Hover cards off" });
            }
            RAct::Sections => self.glass.prefs.binder = !self.glass.prefs.binder,
            RAct::Scratchpad => self.toggle(Local::Scratchpad),
            RAct::Formulas => self.research_panel(RView::Formulas),
            RAct::Code => self.research_panel(RView::Code),
            RAct::Data => self.research_panel(RView::Data),
            RAct::Graph => self.research_panel(RView::Graph),
            RAct::Argument => self.research_panel(RView::Argument),
            RAct::Submit => self.research_panel(RView::Submit),
            RAct::BlindCopy => self.blind_copy(),
            RAct::ImportReview => {
                self.ask_file(wordcraft_ui_egui::FileDialogRequest::Open { purpose: "reviews".into() }, crate::workspace::PickFor::ImportReviews)
            }
            RAct::CommentsToLog => self.comments_to_log(),
            RAct::ResponseLetter => self.response_letter(),
        }
    }
}

fn research_button(g: &mut GlassApp, ui: &mut Ui, t: &Tokens, act: RAct, label: &str, icon: &str) {
    let on = g.research_on(act);
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
    if resp.on_hover_text(label.replace('\n', " ")).clicked() {
        g.research_act(act);
    }
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

//! Dictionary & Thesaurus: a panel for the word at the caret (or one typed in), in the text's
//! language or one picked. It shows whether the word is spelled right (with suggestions), its
//! synonyms by meaning from an installed thesaurus (any language: Portuguese, Spanish, French,
//! German…), and links to look it up: Wiktionary in that language, and on a Mac the Dictionary
//! app. Click a suggestion or synonym to put it in place of the word.

use egui::{Align2, Sense, Ui, vec2};
use serde_json::json;
use wordcraft_engine::Selection;
use wordcraft_engine::proof::{self, ProofLang};
use wordcraft_ui_egui::theme::{medium, regular, semibold};

use crate::GlassApp;
use crate::tokens::{Tokens, paint_state, radius};

/// The panel's state.
#[derive(Default)]
pub struct Dict {
    /// The word typed in the search field (empty: the word at the caret).
    pub query: String,
    /// A language picked (`None`: the text's).
    pub lang: Option<String>,
    /// The last lookup: (word, language) and its synonyms by meaning.
    cache: Option<((String, String), Vec<(String, Vec<String>)>)>,
}

/// Language names for the picker (tag, name): the ones people most often write in.
const NAMES: [(&str, &str); 24] = [
    ("en-US", "English (US)"),
    ("en-GB", "English (UK)"),
    ("pt-PT", "Português (Portugal)"),
    ("pt-BR", "Português (Brasil)"),
    ("es-ES", "Español"),
    ("fr-FR", "Français"),
    ("de-DE", "Deutsch"),
    ("it-IT", "Italiano"),
    ("nl-NL", "Nederlands"),
    ("ca-ES", "Català"),
    ("gl-ES", "Galego"),
    ("sv-SE", "Svenska"),
    ("da-DK", "Dansk"),
    ("nb-NO", "Norsk bokmål"),
    ("fi-FI", "Suomi"),
    ("pl-PL", "Polski"),
    ("cs-CZ", "Čeština"),
    ("ro-RO", "Română"),
    ("hu-HU", "Magyar"),
    ("tr-TR", "Türkçe"),
    ("ru-RU", "Русский"),
    ("uk-UA", "Українська"),
    ("el-GR", "Ελληνικά"),
    ("ko-KR", "한국어"),
];

pub fn lang_name(tag: &str) -> String {
    NAMES.iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map(|(_, n)| (*n).to_string()).unwrap_or_else(|| tag.to_string())
}

/// The word at the caret, its byte range in its paragraph, and its language tag.
pub fn word_at_caret(g: &GlassApp) -> Option<(String, usize, usize, String)> {
    let s = &g.app.session;
    let f = &s.sel.focus;
    let p = s.doc.para_at(f)?;
    let (a, b) = p.word_at(f.off);
    // Word's "word" takes the space after it too: keep only the letters.
    let raw = p.text.get(a..b)?;
    let lead = raw.len() - raw.trim_start().len();
    let w = raw.trim().to_string();
    let (a, b) = (a + lead, a + lead + w.len());
    let lang = p.props_at(a).lang.clone().or_else(|| s.doc.styles.default_chr.lang.clone()).unwrap_or_else(|| "en-US".into());
    Some((w, a, b, lang))
}

/// Put `text` in place of the word at the caret (keeping a capital first letter).
pub fn replace_word(g: &mut GlassApp, text: &str) {
    let Some((w, a, b, _)) = word_at_caret(g) else { return };
    if a == b {
        return;
    }
    let text = if w.chars().next().is_some_and(char::is_uppercase) {
        let mut c = text.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
    } else {
        text.to_string()
    };
    let focus = g.app.session.sel.focus.clone();
    let start = wordcraft_doc::Pos { off: a, ..focus.clone() };
    let end = wordcraft_doc::Pos { off: b, ..focus };
    g.app.session.sel = Selection { anchor: start, focus: end };
    if let Err(e) = g.app.run("text.insert", json!({"text": text})) {
        g.app.status(e);
    }
}

/// Wiktionary in the word's language.
pub fn wiktionary_url(word: &str, tag: &str) -> String {
    let primary = tag.split(['-', '_']).next().unwrap_or("en").to_ascii_lowercase();
    let site = if primary.len() == 2 && primary.chars().all(|c| c.is_ascii_lowercase()) { primary } else { "en".into() };
    let enc: String = word
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' { char::from(b).to_string() } else { format!("%{b:02X}") })
        .collect();
    format!("https://{site}.wiktionary.org/wiki/{enc}")
}

fn section(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add_space(14.0);
    ui.label(egui::RichText::new(text).font(semibold(12.0)).color(t.text_dim));
    ui.add_space(4.0);
}

/// A word chip; true when clicked.
fn chip(ui: &mut Ui, t: &Tokens, text: &str) -> egui::Response {
    let w = ui.painter().layout_no_wrap(text.to_string(), regular(12.5), t.text).size().x + 18.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    paint_state(ui, r, radius::CAPSULE, &resp, false, t);
    ui.painter().rect_stroke(r, 13.0, egui::Stroke::new(0.75, t.edge), egui::StrokeKind::Inside);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, text, regular(12.5), t.text);
    resp
}

pub fn panel(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Dictionary").font(semibold(15.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::widgets::icon_capsule(ui, t, "close", "Close", 26.0, false, true).clicked() {
                g.glass.panel = None;
            }
        });
    });
    ui.add_space(8.0);
    let caret = word_at_caret(g);
    let mut query = g.glass.dict.query.clone();
    let hint = caret.as_ref().map(|c| c.0.clone()).filter(|w| !w.is_empty()).unwrap_or_else(|| "Look up a word".into());
    let resp = ui.add(egui::TextEdit::singleline(&mut query).hint_text(hint).desired_width(ui.available_width()).margin(vec2(10.0, 7.0)));
    if resp.changed() {
        g.glass.dict.query = query.clone();
    }
    let (word, from_caret) =
        if query.trim().is_empty() { (caret.as_ref().map(|c| c.0.clone()).unwrap_or_default(), true) } else { (query.trim().to_string(), false) };
    let text_lang = caret.as_ref().map(|c| c.3.clone()).unwrap_or_else(|| "en-US".into());
    let lang = g.glass.dict.lang.clone().unwrap_or_else(|| text_lang.clone());
    // Language picker.
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Language").font(regular(12.0)).color(t.text_dim));
        let shown = if g.glass.dict.lang.is_none() { format!("{} (the text's)", lang_name(&lang)) } else { lang_name(&lang) };
        egui::ComboBox::from_id_salt("glass_dict_lang").selected_text(shown).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
            if ui.selectable_label(g.glass.dict.lang.is_none(), format!("The text's language ({})", lang_name(&text_lang))).clicked() {
                g.glass.dict.lang = None;
            }
            ui.separator();
            for (tag, name) in NAMES {
                if ui.selectable_label(g.glass.dict.lang.as_deref() == Some(tag), name).clicked() {
                    g.glass.dict.lang = Some(tag.to_string());
                }
            }
        });
    });
    if word.trim().is_empty() || !word.chars().any(char::is_alphabetic) {
        ui.add_space(16.0);
        ui.label(egui::RichText::new("Put the caret in a word, or type one above.").font(regular(12.5)).color(t.text_dim));
        return;
    }
    egui::ScrollArea::vertical().id_salt("glass_dict_scroll").auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(10.0);
        ui.label(egui::RichText::new(&word).font(semibold(22.0)).color(t.text));
        // Spelling.
        let pl = ProofLang::from_tag(Some(&lang));
        section(ui, t, "SPELLING");
        if !pl.is_proofed() {
            ui.label(egui::RichText::new(no_speller(&lang)).font(regular(12.0)).color(t.text_dim));
        } else if proof::is_correct_in(&word, pl) {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                crate::glyphs::paint(ui.painter(), r, "check", egui::Color32::from_rgb(0x44, 0x83, 0x61));
                ui.label(egui::RichText::new("Spelled correctly").font(regular(12.5)).color(t.text));
            });
        } else {
            ui.label(egui::RichText::new("Not in the dictionary. Did you mean:").font(regular(12.5)).color(t.text));
            ui.add_space(4.0);
            let sugg = proof::suggest_in(&word, 8, pl);
            ui.horizontal_wrapped(|ui| {
                for s in &sugg {
                    if chip(ui, t, s).on_hover_text(if from_caret { "Replace the word" } else { "Look it up" }).clicked() {
                        if from_caret {
                            replace_word(g, s);
                        } else {
                            g.glass.dict.query = s.clone();
                        }
                    }
                }
            });
            if sugg.is_empty() {
                ui.label(egui::RichText::new("No suggestions.").font(regular(12.0)).color(t.text_dim));
            }
        }
        // Synonyms.
        section(ui, t, "SYNONYMS");
        let key = (word.to_lowercase(), lang.clone());
        if g.glass.dict.cache.as_ref().is_none_or(|(k, _)| *k != key) {
            let r = g.app.session.run("review.thesaurus", &json!({"word": word, "lang": lang})).unwrap_or_default();
            let meanings: Vec<(String, Vec<String>)> = match r.get("meanings").and_then(|m| m.as_array()) {
                Some(ms) => ms
                    .iter()
                    .map(|m| {
                        let pos = m.get("pos").and_then(|p| p.as_str()).unwrap_or("").to_string();
                        let words = m.get("synonyms").and_then(|s| s.as_array()).map(|a| a.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()).unwrap_or_default();
                        (pos, words)
                    })
                    .collect(),
                None => {
                    let words: Vec<String> = r.get("synonyms").and_then(|s| s.as_array()).map(|a| a.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()).unwrap_or_default();
                    if words.is_empty() { Vec::new() } else { vec![(String::new(), words)] }
                }
            };
            g.glass.dict.cache = Some((key, meanings));
        }
        let meanings = g.glass.dict.cache.as_ref().map(|(_, m)| m.clone()).unwrap_or_default();
        if meanings.is_empty() {
            let thes = proof::thes::has(&lang);
            let msg = if thes {
                "No synonyms found for this word.".to_string()
            } else {
                format!(
                    "No thesaurus for {} is installed. Thesauri come with LibreOffice's dictionaries, or put th_{}_v2.idx and .dat files in your dictionaries folder ({}).",
                    lang_name(&lang),
                    lang.replace('-', "_"),
                    dictionaries_folder()
                )
            };
            ui.add(egui::Label::new(egui::RichText::new(msg).font(regular(12.0)).color(t.text_dim)).wrap());
        }
        for (pos, words) in &meanings {
            if !pos.is_empty() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(pos).font(medium(11.5)).color(t.accent));
            }
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for w in words {
                    let resp = chip(ui, t, w).on_hover_text("Click to use it; right-click to look it up");
                    if resp.clicked() && from_caret {
                        replace_word(g, w);
                    }
                    if resp.secondary_clicked() || (resp.clicked() && !from_caret) {
                        g.glass.dict.query = w.clone();
                    }
                }
            });
        }
        // Look up.
        section(ui, t, "LOOK UP");
        ui.horizontal_wrapped(|ui| {
            if crate::widgets::quiet_button(ui, t, "Wiktionary", None, 30.0).on_hover_text("Definitions, pronunciation and translations").clicked() {
                let url = wiktionary_url(&word, &lang);
                g.open_link(ui.ctx(), &url);
            }
            if cfg!(target_os = "macos") && crate::widgets::quiet_button(ui, t, "Dictionary app", None, 30.0).on_hover_text("Open the word in the Mac's Dictionary").clicked() {
                let url = format!("dict://{}", wiktionary_url(&word, "en").rsplit('/').next().unwrap_or(""));
                g.open_link(ui.ctx(), &url);
            }
        });
        ui.add_space(14.0);
    });
}

/// Where dictionaries and thesauri can be added on this system.
pub fn dictionaries_folder() -> &'static str {
    if cfg!(target_os = "macos") {
        "~/Library/Spelling"
    } else if cfg!(windows) {
        "%LOCALAPPDATA%\\WordCraft\\dictionaries"
    } else {
        "~/.local/share/hunspell"
    }
}

fn no_speller(lang: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("The Mac has no spelling dictionary for {}: add one in System Settings › Keyboard › Text Input.", lang_name(lang))
    } else {
        format!("No spelling dictionary for {} here. English and Polish are checked; on a Mac every language the system spells is.", lang_name(lang))
    }
}

impl GlassApp {
    /// Open a link: web pages in the browser, other links (the Dictionary app's) with the system.
    pub fn open_link(&self, ctx: &egui::Context, url: &str) {
        match (&self.open_external, url.starts_with("http")) {
            (Some(open), false) => open(url),
            _ => ctx.open_url(egui::OpenUrl::new_tab(url)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_engine::Session;
    use wordcraft_ui_egui::{Services, WordApp};

    #[test]
    fn links_are_encoded_per_language() {
        assert_eq!(wiktionary_url("olá", "pt-PT"), "https://pt.wiktionary.org/wiki/ol%C3%A1");
        assert_eq!(wiktionary_url("x y", "zzz"), "https://en.wiktionary.org/wiki/x%20y");
    }

    #[test]
    fn a_synonym_replaces_the_word_at_the_caret_keeping_its_capital() {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default()));
        g.app.run("text.insert", json!({"text": "Happy days"})).unwrap();
        let start = wordcraft_doc::Pos { off: 2, ..g.app.session.sel.focus.clone() };
        g.app.session.sel = Selection::caret(start);
        assert_eq!(word_at_caret(&g).map(|w| w.0), Some("Happy".into()));
        replace_word(&mut g, "glad");
        assert_eq!(g.app.session.doc.body.first().and_then(|b| b.as_para()).map(|p| p.text.clone()), Some("Glad days".into()));
    }
}

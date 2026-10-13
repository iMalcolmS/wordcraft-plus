//! Formulas and typing shortcuts for Research mode.
//!
//! - **Formula library**: equations you use again and again, saved by name with their LaTeX.
//!   Insert one from the Formulas panel or by typing `/eq-name` at the start of a paragraph.
//!   The selected equation can be saved into it.
//! - **LaTeX as you type**: `$\frac{a}{b}$` becomes a real equation as soon as the closing `$`
//!   is typed (prices like "$5 and $10" are left alone).
//! - **Markdown as you type**: `**bold**`, `*italic*`, `` `code` ``, and `#`, `##`, `###` or `>`
//!   then a space at the start of a paragraph for headings and quotes.
//!
//! Every change runs WordCraft commands, so each is one Undo away.

use egui::{Sense, Ui, vec2};
use serde::{Deserialize, Serialize};
use serde_json::json;
use wordcraft_doc::{Pos, StoryRef};
use wordcraft_engine::Selection;
use wordcraft_ui_egui::theme::{medium, regular};

use crate::GlassApp;
use crate::tokens::{Tokens, paint_state};

/// A saved formula.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Formula {
    pub name: String,
    pub latex: String,
}

/// The formulas a new library starts with.
pub const DEFAULTS: [(&str, &str); 14] = [
    ("matriz", "A = \\begin{pmatrix} a_{11} & a_{12} \\\\ a_{21} & a_{22} \\end{pmatrix}"),
    ("matrix", "A = \\begin{bmatrix} 1 & 0 \\\\ 0 & 1 \\end{bmatrix}"),
    ("quadratic", "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}"),
    ("mean", "\\bar{x} = \\frac{1}{n} \\sum_{i=1}^{n} x_i"),
    ("variance", "s^2 = \\frac{1}{n-1} \\sum_{i=1}^{n} (x_i - \\bar{x})^2"),
    ("normal", "f(x) = \\frac{1}{\\sigma \\sqrt{2\\pi}} e^{-\\frac{(x-\\mu)^2}{2\\sigma^2}}"),
    ("bayes", "P(A|B) = \\frac{P(B|A) P(A)}{P(B)}"),
    ("regression", "y = \\beta_0 + \\beta_1 x + \\varepsilon"),
    ("softmax", "\\sigma(z)_i = \\frac{e^{z_i}}{\\sum_{j} e^{z_j}}"),
    ("crossentropy", "L = -\\sum_{i} y_i \\log \\hat{y}_i"),
    ("derivative", "f'(x) = \\lim_{h \\to 0} \\frac{f(x+h) - f(x)}{h}"),
    ("integral", "\\int_{a}^{b} f(x) \\, dx"),
    ("euler", "e^{i\\pi} + 1 = 0"),
    ("ttest", "t = \\frac{\\bar{x} - \\mu_0}{s / \\sqrt{n}}"),
];

/// Most formulas kept.
const MAX_FORMULAS: usize = 500;

/// A formula name as typed after `/eq-`: letters, digits and `-`.
pub fn clean_name(n: &str) -> String {
    n.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(40)
        .collect()
}

impl GlassApp {
    /// The formula library (seeded with [`DEFAULTS`] the first time).
    pub fn formulas(&mut self) -> &mut Vec<Formula> {
        let p = &mut self.glass.prefs;
        if !p.formulas_seeded {
            p.formulas_seeded = true;
            if p.formulas.is_empty() {
                p.formulas = DEFAULTS.iter().map(|(n, l)| Formula { name: (*n).into(), latex: (*l).into() }).collect();
            }
        }
        &mut self.glass.prefs.formulas
    }

    /// Insert LaTeX as an equation at the caret.
    pub fn insert_latex(&mut self, latex: &str) {
        if let Err(e) = self.app.run("insert.equation", json!({"latex": latex, "edit": false})) {
            self.app.status(format!("That LaTeX couldn't be read: {e}"));
        }
    }
}

pub fn panel(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    ui.add(
        egui::Label::new(
            egui::RichText::new("Click to insert. In the text, type /eq- and the name at the start of a paragraph, or $…$ around LaTeX.")
                .font(regular(12.0))
                .color(t.text_dim),
        )
        .wrap(),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let mut on = g.glass.prefs.latex_shortcuts;
        if ui.checkbox(&mut on, "$…$ → equation").changed() {
            g.glass.prefs.latex_shortcuts = on;
        }
        let mut md = g.glass.prefs.markdown_shortcuts;
        if ui.checkbox(&mut md, "Markdown").changed() {
            g.glass.prefs.markdown_shortcuts = md;
        }
    });
    ui.add_space(6.0);
    // Save the selected equation.
    if let Ok(v) = g.app.session.run("equation.get", &json!({}))
        && let Some(latex) = v.get("latex").and_then(|l| l.as_str()).map(str::to_string)
        && crate::widgets::quiet_button(ui, t, "Save the Selected Equation", None, 28.0).clicked()
    {
        let n = g.formulas().len() + 1;
        g.formulas().push(Formula { name: format!("formula-{n}"), latex });
    }
    // Add one by hand.
    let id = egui::Id::new("glass_formula_new");
    let (mut name, mut latex): (String, String) = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut name).hint_text("name").desired_width(90.0));
        ui.add(egui::TextEdit::singleline(&mut latex).hint_text("LaTeX, e.g. \\frac{a}{b}").desired_width(ui.available_width() - 50.0));
        if ui.button("Add").clicked() && !clean_name(&name).is_empty() && !latex.trim().is_empty() && g.formulas().len() < MAX_FORMULAS {
            let f = Formula { name: clean_name(&name), latex: latex.trim().to_string() };
            g.formulas().push(f);
            name.clear();
            latex.clear();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, (name, latex)));
    ui.add_space(8.0);
    let list = g.formulas().clone();
    let mut insert: Option<String> = None;
    let mut remove: Option<usize> = None;
    egui::ScrollArea::vertical().id_salt("glass_formulas").auto_shrink([false, false]).show(ui, |ui| {
        for (i, f) in list.iter().enumerate() {
            let w = ui.available_width();
            let galley = ui.painter().layout(f.latex.clone(), egui::FontId::monospace(11.0), t.text_dim, w - 16.0);
            let (r, resp) = ui.allocate_exact_size(vec2(w, galley.size().y + 26.0), Sense::click());
            paint_state(ui, r, 8, &resp, false, t);
            ui.painter().text(r.min + vec2(8.0, 6.0), egui::Align2::LEFT_TOP, format!("/eq-{}", f.name), medium(12.0), t.text);
            ui.painter().galley(r.min + vec2(8.0, 22.0), galley, t.text_dim);
            if resp.clicked() {
                insert = Some(f.latex.clone());
            }
            resp.context_menu(|ui| {
                if ui.button("Copy LaTeX").clicked() {
                    ui.ctx().copy_text(f.latex.clone());
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    remove = Some(i);
                    ui.close();
                }
            });
        }
    });
    if let Some(l) = insert {
        g.insert_latex(&l);
    }
    if let Some(i) = remove
        && i < g.formulas().len()
    {
        g.formulas().remove(i);
    }
}

// ---------------------------------------------------------------------------------------------
// Typing shortcuts.

/// LaTeX between `$`s just closed before `off` in `text`: (start of the opening `$`, the LaTeX).
pub fn closed_math(text: &str, off: usize) -> Option<(usize, String)> {
    let before = text.get(..off)?;
    let inner_end = before.strip_suffix('$')?;
    let open = inner_end.rfind('$')?;
    let inner = inner_end.get(open + 1..)?;
    // `$$`, spaces at the edges, or a price ("$5 and $10") aren't math.
    if inner.is_empty() || inner.starts_with(' ') || inner.ends_with(' ') || inner.chars().count() > 400 {
        return None;
    }
    if inner.contains(wordcraft_doc::para::OBJ) || inner.contains('\n') {
        return None;
    }
    let mathy = inner.chars().any(|c| matches!(c, '\\' | '^' | '_' | '=' | '{' | '+' | '<' | '>'))
        || inner.chars().all(|c| c.is_alphabetic()) && inner.chars().count() <= 3;
    if !mathy || inner.chars().next().is_some_and(|c| c.is_ascii_digit()) && !inner.contains('\\') && !inner.contains('^') {
        return None;
    }
    // The opening `$` follows a space, a bracket or the start, never a letter or digit.
    if open > 0 && before.get(..open).and_then(|b| b.chars().last()).is_some_and(|c| c.is_alphanumeric()) {
        return None;
    }
    Some((open, inner.to_string()))
}

/// A Markdown span just closed before `off`: (start, end of the inner text, marker length, kind).
pub fn closed_markdown(text: &str, off: usize) -> Option<(usize, usize, usize, &'static str)> {
    let before = text.get(..off)?;
    for (mark, kind) in [("**", "bold"), ("__", "bold"), ("`", "code"), ("*", "italic"), ("_", "italic")] {
        let Some(inner_end) = before.strip_suffix(mark) else { continue };
        if mark.len() == 1 && inner_end.ends_with(mark) {
            continue; // the first half of `**`
        }
        let Some(open) = inner_end.rfind(mark) else { continue };
        let inner = inner_end.get(open + mark.len()..)?;
        if inner.is_empty()
            || inner.starts_with(' ')
            || inner.ends_with(' ')
            || inner.contains(wordcraft_doc::para::OBJ)
            || inner.chars().count() > 300
        {
            continue;
        }
        if mark.len() == 1 && (inner.starts_with(mark) || before.get(..open).is_some_and(|b| b.ends_with(mark))) {
            continue;
        }
        // `_` inside a word (snake_case) isn't emphasis.
        if mark.starts_with('_') && before.get(..open).and_then(|b| b.chars().last()).is_some_and(|c| c.is_alphanumeric()) {
            continue;
        }
        return Some((open, open + mark.len() + inner.len(), mark.len(), kind));
    }
    None
}

/// Apply LaTeX and Markdown shortcuts at the caret, once per change.
pub fn typing_shortcuts(g: &mut GlassApp) {
    let s = &g.app.session;
    let key = (s.document_id(), s.rev());
    if g.glass.shortcuts_seen == key {
        return;
    }
    g.glass.shortcuts_seen = key;
    let focus = s.sel.focus.clone();
    if !s.sel.is_collapsed() || focus.story != StoryRef::Body || g.glass.inline.open {
        return;
    }
    let Some(p) = s.doc.para(StoryRef::Body, &focus.path) else { return };
    let text = p.text.clone();
    let at = |off: usize| Pos::new(StoryRef::Body, focus.path.clone(), off);
    // Headings and quotes: `# ` … at the start.
    if g.glass.prefs.markdown_shortcuts {
        for (marker, cmd, params) in [
            ("### ", "para.heading3", json!({})),
            ("## ", "para.heading2", json!({})),
            ("# ", "para.heading1", json!({})),
            ("> ", "para.style", json!({"style": "Quote"})),
        ] {
            if focus.off == marker.len() && text.starts_with(marker) {
                g.app.session.sel = Selection { anchor: at(0), focus: at(marker.len()) };
                let _ = g.app.run("text.delete", json!({}));
                let _ = g.app.run(cmd, params);
                return;
            }
        }
    }
    if g.glass.prefs.latex_shortcuts
        && let Some((open, latex)) = closed_math(&text, focus.off)
    {
        g.app.session.sel = Selection { anchor: at(open), focus: at(focus.off) };
        let _ = g.app.run("text.delete", json!({}));
        g.insert_latex(&latex);
        return;
    }
    if g.glass.prefs.markdown_shortcuts
        && let Some((open, inner_end, mlen, kind)) = closed_markdown(&text, focus.off)
    {
        // Remove the closing marker, then the opening one, then format what's between.
        g.app.session.sel = Selection { anchor: at(inner_end), focus: at(inner_end + mlen) };
        let _ = g.app.run("text.delete", json!({}));
        g.app.session.sel = Selection { anchor: at(open), focus: at(open + mlen) };
        let _ = g.app.run("text.delete", json!({}));
        g.app.session.sel = Selection { anchor: at(open), focus: at(inner_end - mlen) };
        let _ = match kind {
            "bold" => g.app.run("format.bold", json!({"value": true})),
            "italic" => g.app.run("format.italic", json!({"value": true})),
            _ => g.app.run("format.font", json!({"name": "Courier New"})),
        };
        g.app.session.sel = Selection::caret(at(inner_end - mlen));
        // Type on in plain text: with a caret, formatting applies to what is typed next.
        let _ = match kind {
            "bold" => g.app.run("format.bold", json!({"value": false})),
            "italic" => g.app.run("format.italic", json!({"value": false})),
            _ => g.app.run("format.font", json!({"name": wordcraft_doc::styles::BODY_FONT})),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_engine::Session;
    use wordcraft_ui_egui::{Services, WordApp};

    #[test]
    fn math_is_found_but_prices_are_not() {
        let t = "so $\\frac{a}{b}$";
        assert_eq!(closed_math(t, t.len()), Some((3, "\\frac{a}{b}".into())));
        let t = "costs $5 and $";
        assert_eq!(closed_math(t, t.len()), None);
        let t = "from $5 to $10$";
        assert_eq!(closed_math(t, t.len()), None);
        let t = "let $x$";
        assert_eq!(closed_math(t, t.len()), Some((4, "x".into())));
        assert_eq!(closed_math("$$", 2), None);
        assert_eq!(closed_math("é$", 3), None);
    }

    #[test]
    fn markdown_spans_are_found() {
        let t = "a **bold**";
        assert_eq!(closed_markdown(t, t.len()), Some((2, 8, 2, "bold")));
        let t = "an *idea*";
        assert_eq!(closed_markdown(t, t.len()), Some((3, 8, 1, "italic")));
        let t = "run `ls`";
        assert_eq!(closed_markdown(t, t.len()), Some((4, 7, 1, "code")));
        let t = "my_var_name";
        assert_eq!(closed_markdown(t, t.len()), None);
        let t = "a **";
        assert_eq!(closed_markdown(t, t.len()), None);
    }

    #[test]
    fn shortcuts_turn_typing_into_equations_and_formatting() {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default()));
        g.glass.prefs.research = true;
        g.app.run("text.insert", json!({"text": "Area $\\pi r^2$"})).unwrap();
        typing_shortcuts(&mut g);
        let p = g.app.session.doc.body.first().and_then(|b| b.as_para()).unwrap();
        assert!(!p.text.contains('$'), "{:?}", p.text);
        assert_eq!(p.objects.len(), 1, "an equation took its place");
        g.app.run("text.insert", json!({"text": " is **big**"})).unwrap();
        typing_shortcuts(&mut g);
        let p = g.app.session.doc.body.first().and_then(|b| b.as_para()).unwrap();
        assert!(p.text.ends_with(" is big"), "{:?}", p.text);
        g.app.run("text.newParagraph", json!({})).unwrap();
        g.app.run("text.insert", json!({"text": "## "})).unwrap();
        typing_shortcuts(&mut g);
        assert!(g.formulas().len() >= DEFAULTS.len());
    }
}

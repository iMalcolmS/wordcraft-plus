//! A spelling service the host provides, for the languages WordCraft has no dictionary of its
//! own for: on macOS, the system spell checker (every language the Mac spells: Portuguese,
//! Spanish, French, German, Italian, Dutch, the Nordic languages, Russian, Turkish…).
//!
//! The host installs it once at start-up ([`install`]); [`ProofLang::from_tag`] then resolves a
//! run's language to [`ProofLang::System`] when the service spells it, and the `*_in` functions
//! ask the service. A service that can't answer (`None`, say from a background thread) leaves
//! the word unmarked, as a language without proofing tools does.
//!
//! [`ProofLang::from_tag`]: crate::ProofLang::from_tag
//! [`ProofLang::System`]: crate::ProofLang::System

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// What the host's spell checker can do.
pub trait SpellService: Send + Sync {
    /// The languages it spells, as tags (`en`, `pt_PT`, `pt_BR`, `es`, `de`…).
    fn languages(&self) -> Vec<String>;
    /// Is `word` spelled correctly in `lang` (one of [`Self::languages`])? `None`: it can't say.
    fn check(&self, word: &str, lang: &str) -> Option<bool>;
    /// Suggestions for a misspelled `word`, best first.
    fn suggest(&self, word: &str, lang: &str, max: usize) -> Vec<String>;
}

struct State {
    service: Arc<dyn SpellService>,
    /// The service's languages, normalised (`pt-PT`), and the code each resolves to.
    langs: Vec<String>,
    /// Tags already resolved (a tag → the service's language, or none).
    resolved: HashMap<String, Option<LangCode>>,
}

static STATE: RwLock<Option<State>> = RwLock::new(None);

/// A language code the service understands, kept small and `Copy` (at most 15 ASCII bytes,
/// such as `pt_BR` or `zh_Hant_TW`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LangCode {
    len: u8,
    bytes: [u8; 15],
}

impl LangCode {
    /// `None` when `s` is longer than 15 bytes or not ASCII.
    pub fn new(s: &str) -> Option<LangCode> {
        if s.is_empty() || s.len() > 15 || !s.is_ascii() {
            return None;
        }
        let mut bytes = [0u8; 15];
        bytes.get_mut(..s.len())?.copy_from_slice(s.as_bytes());
        Some(LangCode { len: s.len() as u8, bytes })
    }

    pub fn as_str(&self) -> &str {
        self.bytes.get(..usize::from(self.len)).and_then(|b| std::str::from_utf8(b).ok()).unwrap_or("")
    }

    /// The primary language (`pt` of `pt_BR`).
    pub fn primary(&self) -> &str {
        self.as_str().split(['_', '-']).next().unwrap_or("")
    }
}

/// Use `service` for languages without a built-in dictionary (replacing any earlier one).
pub fn install(service: Arc<dyn SpellService>) {
    let langs = service.languages().into_iter().take(500).collect();
    *STATE.write().unwrap_or_else(|e| e.into_inner()) = Some(State { service, langs, resolved: HashMap::new() });
    crate::bump_generation();
}

/// Stop using the host's spell checker.
pub fn uninstall() {
    *STATE.write().unwrap_or_else(|e| e.into_inner()) = None;
    crate::bump_generation();
}

/// The languages the host's spell checker offers (empty without one).
pub fn languages() -> Vec<String> {
    STATE.read().unwrap_or_else(|e| e.into_inner()).as_ref().map(|s| s.langs.clone()).unwrap_or_default()
}

/// The service's language for a BCP 47 tag: the exact region first (`pt-BR` → `pt_BR`), then
/// the bare language (`pt`), then any region of it.
pub fn resolve(tag: &str) -> Option<LangCode> {
    let norm = tag.trim().replace('-', "_");
    if norm.is_empty() {
        return None;
    }
    // `None` here: no service installed.
    let hit = STATE.read().unwrap_or_else(|e| e.into_inner()).as_ref().map(|s| s.resolved.get(&norm).copied())?;
    if let Some(r) = hit {
        return r;
    }
    let mut guard = STATE.write().unwrap_or_else(|e| e.into_inner());
    let state = guard.as_mut()?;
    let primary = norm.split('_').next().unwrap_or("").to_ascii_lowercase();
    let pick = state
        .langs
        .iter()
        .find(|l| l.replace('-', "_").eq_ignore_ascii_case(&norm))
        .or_else(|| state.langs.iter().find(|l| l.eq_ignore_ascii_case(&primary)))
        .or_else(|| state.langs.iter().find(|l| l.replace('-', "_").split('_').next().is_some_and(|p| p.eq_ignore_ascii_case(&primary))))
        .and_then(|l| LangCode::new(l));
    if state.resolved.len() > 1000 {
        state.resolved.clear();
    }
    state.resolved.insert(norm, pick);
    pick
}

fn service() -> Option<Arc<dyn SpellService>> {
    STATE.read().unwrap_or_else(|e| e.into_inner()).as_ref().map(|s| s.service.clone())
}

/// Is `word` correct in `lang`? Unknown (no service, or it can't say) counts as correct.
pub fn is_correct(word: &str, lang: LangCode) -> bool {
    if word.chars().count() <= 1 || word.chars().any(|c| c.is_ascii_digit()) || word.len() > 200 {
        return true;
    }
    service().and_then(|s| s.check(word, lang.as_str())).unwrap_or(true)
}

/// Suggestions for `word` in `lang`.
pub fn suggest(word: &str, max: usize, lang: LangCode) -> Vec<String> {
    if word.len() > 200 {
        return Vec::new();
    }
    service().map(|s| s.suggest(word, lang.as_str(), max.min(50))).unwrap_or_default().into_iter().take(max).collect()
}

/// "Possible spelling mistake" in the text's language, for the languages people most often
/// write in; English otherwise.
pub fn message(lang: LangCode) -> &'static str {
    match lang.primary() {
        "pt" => "Possível erro ortográfico",
        "es" | "gl" => "Posible error ortográfico",
        "fr" => "Faute d'orthographe possible",
        "de" => "Möglicher Rechtschreibfehler",
        "it" => "Possibile errore di ortografia",
        "nl" => "Mogelijke spelfout",
        "ca" => "Possible error ortogràfic",
        "sv" => "Möjligt stavfel",
        "da" | "nb" | "no" => "Mulig stavefeil",
        _ => "Possible spelling mistake",
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tiny service: knows a few Portuguese and Spanish words.
    pub struct Fake;

    impl SpellService for Fake {
        fn languages(&self) -> Vec<String> {
            vec!["en".into(), "pt_PT".into(), "pt_BR".into(), "es".into()]
        }
        fn check(&self, word: &str, lang: &str) -> Option<bool> {
            let known: &[&str] = match lang {
                "pt_PT" | "pt_BR" => &["olá", "mundo", "escrever", "Olá"],
                "es" => &["hola", "mundo"],
                _ => return None,
            };
            Some(known.contains(&word))
        }
        fn suggest(&self, word: &str, _lang: &str, _max: usize) -> Vec<String> {
            if word == "mumdo" { vec!["mundo".into()] } else { Vec::new() }
        }
    }

    #[test]
    fn a_host_spell_checker_proofs_its_languages() {
        use crate::ProofLang;
        install(Arc::new(Fake));
        let pt = ProofLang::from_tag(Some("pt-PT"));
        assert!(matches!(pt, ProofLang::System(c) if c.as_str() == "pt_PT"));
        assert!(
            matches!(ProofLang::from_tag(Some("pt-AO")), ProofLang::System(c) if c.primary() == "pt"),
            "another region falls back to the language"
        );
        assert!(matches!(ProofLang::from_tag(Some("es-MX")), ProofLang::System(c) if c.as_str() == "es"));
        assert_eq!(ProofLang::from_tag(Some("en-GB")), ProofLang::En, "English keeps its own dictionary");
        assert!(crate::is_correct_in("mundo", pt));
        assert!(!crate::is_correct_in("mumdo", pt));
        assert_eq!(crate::suggest_in("mumdo", 5, pt), ["mundo"]);
        let issues = crate::check_spelling_in("Olá mumdo", pt);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].message, "Possível erro ortográfico");
        assert!(crate::check_grammar_in("Olá mumdo", pt).is_empty());
        uninstall();
        assert_eq!(ProofLang::from_tag(Some("pt-PT")), ProofLang::Other);
    }

    #[test]
    fn codes_are_small_and_safe() {
        assert_eq!(LangCode::new("pt_BR").map(|c| c.primary().to_string()), Some("pt".into()));
        assert!(LangCode::new("").is_none());
        assert!(LangCode::new("a-very-long-language-tag").is_none());
        assert!(LangCode::new("ç").is_none());
    }
}

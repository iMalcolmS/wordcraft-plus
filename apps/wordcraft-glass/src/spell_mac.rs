//! macOS: the system spell checker (`NSSpellChecker`) as WordCraft's spelling service for the
//! languages it has no dictionary of its own for, so Portuguese, Spanish, French, German and
//! every other language the Mac spells get spelling marks and suggestions.
//!
//! The spell checker is used from the main thread only (where the layout runs); a call from
//! another thread answers "can't say", which leaves the word unmarked.

use std::sync::Mutex;

use objc2::MainThreadMarker;
use objc2_app_kit::NSSpellChecker;
use objc2_foundation::{NSNotFound, NSRange, NSString};
use wordcraft_engine::proof::external::SpellService;

/// The spell checker's language last set (it checks in one language at a time).
struct MacSpell {
    current: Mutex<String>,
}

impl MacSpell {
    /// Point the spell checker at `lang` (if it isn't already); false when it can't be used.
    fn set_language(&self, checker: &NSSpellChecker, lang: &str) -> bool {
        let mut cur = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if *cur != lang {
            if !checker.setLanguage(&NSString::from_str(lang)) {
                return false;
            }
            *cur = lang.to_string();
        }
        true
    }
}

impl SpellService for MacSpell {
    fn languages(&self) -> Vec<String> {
        if MainThreadMarker::new().is_none() {
            return Vec::new();
        }
        NSSpellChecker::sharedSpellChecker().availableLanguages().iter().map(|l| l.to_string()).collect()
    }

    fn check(&self, word: &str, lang: &str) -> Option<bool> {
        MainThreadMarker::new()?;
        let checker = NSSpellChecker::sharedSpellChecker();
        if !self.set_language(&checker, lang) {
            return None;
        }
        let r = checker.checkSpellingOfString_startingAt(&NSString::from_str(word), 0);
        Some(r.length == 0 || r.location as isize == NSNotFound)
    }

    fn suggest(&self, word: &str, lang: &str, max: usize) -> Vec<String> {
        if MainThreadMarker::new().is_none() {
            return Vec::new();
        }
        let checker = NSSpellChecker::sharedSpellChecker();
        let s = NSString::from_str(word);
        let range = NSRange { location: 0, length: s.length() };
        checker
            .guessesForWordRange_inString_language_inSpellDocumentWithTag(range, &s, Some(&NSString::from_str(lang)), 0)
            .map(|a| a.iter().take(max).map(|g| g.to_string()).collect())
            .unwrap_or_default()
    }
}

/// Use the Mac's spell checker for every language it spells (call once, on the main thread).
pub fn install() {
    let service = MacSpell { current: Mutex::new(String::new()) };
    if service.languages().is_empty() {
        return;
    }
    wordcraft_engine::proof::external::install(std::sync::Arc::new(service));
}

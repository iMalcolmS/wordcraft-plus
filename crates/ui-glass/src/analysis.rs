//! Pacing and prose rhythm: per-section measures a writer can act on, in English and Portuguese.
//!
//! - **Sentence length** (average words) and **variety** (how much it changes: low variety reads
//!   flat, high reads lively).
//! - **Filter words** per 1,000 words ("she noticed", "he felt", "sentiu", "percebeu"): words that
//!   put a character's senses between the reader and the scene.
//! - **Passive voice** per 1,000 words (an estimate: a form of "to be" followed by a participle).
//! - **Dialogue** as a share of the text (quoted speech, or lines opened with a dash).

/// Filter words, English and Portuguese, lowercase.
const FILTER: &[&str] = &[
    "noticed", "felt", "saw", "heard", "realised", "realized", "seemed", "wondered", "thought", "watched", "knew", "decided", "looked", "sentiu",
    "percebeu", "notou", "viu", "ouviu", "pensou", "parecia", "decidiu", "sabia", "olhou", "reparou", "achou",
];
const TO_BE: &[&str] = &["was", "were", "been", "being", "is", "are", "foi", "foram", "era", "eram", "sido", "estava", "estavam", "é", "são"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metrics {
    pub words: usize,
    pub sentences: usize,
    pub avg_sentence: f32,
    pub variety: f32,
    pub filter_per_k: f32,
    pub passive_per_k: f32,
    pub dialogue: f32,
    /// The filter words found, most used first.
    pub filters: Vec<(String, usize)>,
}

fn words_of(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '’' && c != '-').filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect()
}

fn participle(w: &str) -> bool {
    ["ed", "ado", "ido", "ada", "ida", "ados", "idos", "adas", "idas"].iter().any(|e| w.len() > e.len() + 2 && w.ends_with(e))
}

pub fn analyze(text: &str) -> Metrics {
    let mut m = Metrics::default();
    let mut lengths: Vec<f32> = Vec::new();
    for s in text.split(['.', '!', '?', '…', '\n']) {
        let n = words_of(s).len();
        if n > 0 {
            lengths.push(n as f32);
        }
    }
    let words = words_of(text);
    m.words = words.len();
    m.sentences = lengths.len();
    if !lengths.is_empty() {
        let mean = lengths.iter().sum::<f32>() / lengths.len() as f32;
        let var = lengths.iter().map(|l| (l - mean).powi(2)).sum::<f32>() / lengths.len() as f32;
        m.avg_sentence = mean;
        m.variety = var.sqrt();
    }
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut passive = 0usize;
    for (i, w) in words.iter().enumerate() {
        if FILTER.contains(&w.as_str()) {
            *counts.entry(w.clone()).or_default() += 1;
        }
        if TO_BE.contains(&w.as_str()) && words.get(i + 1).is_some_and(|n| participle(n)) {
            passive += 1;
        }
    }
    let k = (m.words.max(1) as f32) / 1000.0;
    m.filter_per_k = counts.values().sum::<usize>() as f32 / k;
    m.passive_per_k = passive as f32 / k;
    let mut filters: Vec<(String, usize)> = counts.into_iter().collect();
    filters.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    m.filters = filters;
    // Dialogue: text inside quotes, plus lines opened with a dash (Portuguese and Spanish style).
    let total = text.chars().filter(|c| !c.is_whitespace()).count().max(1);
    let mut inside = false;
    let mut spoken = 0usize;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('—') || trimmed.starts_with('–') {
            spoken += trimmed.chars().filter(|c| !c.is_whitespace()).count();
            continue;
        }
        for c in line.chars() {
            match c {
                '“' | '«' => inside = true,
                '”' | '»' => inside = false,
                '"' => inside = !inside,
                _ if inside && !c.is_whitespace() => spoken += 1,
                _ => {}
            }
        }
        inside = false;
    }
    m.dialogue = (spoken as f32 / total as f32).clamp(0.0, 1.0);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_sentences_filters_and_passive() {
        let m = analyze("She noticed the door. It was opened by someone she felt she knew. Short.");
        assert_eq!(m.sentences, 3);
        assert!(m.filter_per_k > 0.0);
        assert!(m.passive_per_k > 0.0);
        assert!(m.variety > 0.0);
        assert_eq!(m.filters.first().map(|f| f.0.as_str()), Some("felt"));
    }

    #[test]
    fn counts_dialogue_in_quotes_and_dashes() {
        let m = analyze("“Hello,” she said.\n— Olá, tudo bem?\nNarration only here.");
        assert!(m.dialogue > 0.2 && m.dialogue < 0.9, "{}", m.dialogue);
        assert_eq!(analyze("").words, 0);
    }
}

//! Thesauri: synonyms in any language that has a MyThes thesaurus installed, the format the
//! free office suites use (`th_pt_PT_v2.idx` / `.dat`, `th_es_ES_v2`, `th_de_DE_v2`…). Like the
//! spelling dictionaries they aren't bundled: they come with LibreOffice, with the system's
//! `mythes-*` packages, or are dropped into the dictionaries folder.
//!
//! The index (`.idx`, word → byte offset) is read once per language; each lookup reads one entry
//! from the data file (`.dat`): `word|n`, then `n` lines of `(part of speech)|synonym|…`.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Largest index read.
const MAX_INDEX: u64 = 64 << 20;
/// Most bytes read for one entry.
const MAX_ENTRY: usize = 64 * 1024;
/// Most meanings and synonyms kept for one word.
const MAX_MEANINGS: usize = 60;
const MAX_WORDS: usize = 60;

/// One sense of a word: its part of speech (`noun`, `verb`… as the thesaurus writes it) and
/// its synonyms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meaning {
    pub pos: String,
    pub words: Vec<String>,
}

struct Thesaurus {
    dat: PathBuf,
    latin1: bool,
    index: HashMap<String, u64>,
}

/// Loaded thesauri by language tag (`None`: none installed).
static CACHE: RwLock<Option<HashMap<String, Option<Arc<Thesaurus>>>>> = RwLock::new(None);

/// Folders that may hold dictionaries and thesauri: the spelling dictionary folders, the
/// systems' thesaurus folders, and every dictionary extension of an installed LibreOffice.
pub fn dirs() -> Vec<PathBuf> {
    let mut out = crate::hunspell::search_dirs();
    if cfg!(target_arch = "wasm32") {
        return out;
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Applications/LibreOffice.app/Contents/Resources/extensions"));
        if let Some(h) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(h).join("Applications/LibreOffice.app/Contents/Resources/extensions"));
        }
    } else if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(b) = std::env::var_os(var) {
                roots.push(PathBuf::from(b).join("LibreOffice").join("share").join("extensions"));
            }
        }
    } else {
        out.push(PathBuf::from("/usr/share/mythes"));
        out.push(PathBuf::from("/usr/local/share/mythes"));
        roots.push(PathBuf::from("/usr/lib/libreoffice/share/extensions"));
        roots.push(PathBuf::from("/usr/lib64/libreoffice/share/extensions"));
        roots.push(PathBuf::from("/opt/libreoffice/share/extensions"));
    }
    for root in roots {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for e in entries.flatten().take(200) {
                let p = e.path();
                if p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("dict-")) {
                    out.push(p.clone());
                    out.push(p.join("dictionaries"));
                }
            }
        }
    }
    out
}

/// The thesauri installed, as (language, index, data): `th_pt_PT_v2.idx` → `pt_PT`.
pub fn installed() -> Vec<(String, PathBuf, PathBuf)> {
    let mut out: Vec<(String, PathBuf, PathBuf)> = Vec::new();
    for dir in dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten().take(2000) {
            let p = e.path();
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let Some(stem) = name.strip_suffix(".idx").and_then(|s| s.strip_prefix("th_")) else { continue };
            let dat = p.with_extension("dat");
            if !dat.is_file() {
                continue;
            }
            let lang = stem.trim_end_matches("_v2").to_string();
            if !out.iter().any(|(l, _, _)| *l == lang) {
                out.push((lang, p, dat));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The languages with a thesaurus installed (`pt_PT`, `es_ES`, `en_US`…).
pub fn languages() -> Vec<String> {
    installed().into_iter().map(|(l, _, _)| l).collect()
}

/// The best installed thesaurus for a tag: the same region first, then the same language.
fn pick(tag: &str) -> Option<(PathBuf, PathBuf)> {
    let norm = tag.trim().replace('-', "_");
    let primary = norm.split('_').next().unwrap_or("").to_ascii_lowercase();
    let all = installed();
    all.iter()
        .find(|(l, _, _)| l.eq_ignore_ascii_case(&norm))
        .or_else(|| all.iter().find(|(l, _, _)| l.split('_').next().is_some_and(|p| p.eq_ignore_ascii_case(&primary))))
        .map(|(_, i, d)| (i.clone(), d.clone()))
}

fn decode(bytes: &[u8], latin1: bool) -> String {
    if latin1 { bytes.iter().map(|&b| char::from(b)).collect() } else { String::from_utf8_lossy(bytes).into_owned() }
}

fn load(idx: &Path, dat: &Path) -> Result<Thesaurus, String> {
    let len = std::fs::metadata(idx).map_err(|e| e.to_string())?.len();
    if len > MAX_INDEX {
        return Err(format!("{}: too large", idx.display()));
    }
    let bytes = std::fs::read(idx).map_err(|e| e.to_string())?;
    let first_line = bytes.split(|b| *b == b'\n').next().unwrap_or_default();
    let enc = String::from_utf8_lossy(first_line).trim().to_ascii_uppercase();
    let latin1 = enc.starts_with("ISO8859") || enc.starts_with("ISO-8859") || enc == "LATIN1";
    let text = decode(&bytes, latin1);
    let mut index = HashMap::new();
    for line in text.lines().skip(2) {
        if let Some((w, off)) = line.rsplit_once('|')
            && let Ok(off) = off.trim().parse::<u64>()
        {
            index.insert(w.to_lowercase(), off);
        }
    }
    Ok(Thesaurus { dat: dat.to_path_buf(), latin1, index })
}

fn thesaurus(tag: &str) -> Option<Arc<Thesaurus>> {
    let key = tag.trim().replace('-', "_").to_ascii_lowercase();
    if let Some(hit) = CACHE.read().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let loaded = pick(tag).and_then(|(i, d)| load(&i, &d).map_err(|e| log::warn!("thesaurus: {e}")).ok()).map(Arc::new);
    let mut guard = CACHE.write().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    if map.len() > 64 {
        map.clear();
    }
    map.insert(key, loaded.clone());
    loaded
}

/// Forget loaded thesauri (one was installed or removed).
pub fn reload() {
    *CACHE.write().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Parse one `.dat` entry: `word|n` then `n` meaning lines.
pub fn parse_entry(text: &str) -> Vec<Meaning> {
    let mut lines = text.lines();
    let n = lines.next().and_then(|l| l.rsplit_once('|')).and_then(|(_, n)| n.trim().parse::<usize>().ok()).unwrap_or(0);
    lines
        .take(n.min(MAX_MEANINGS))
        .map(|l| {
            let mut parts = l.split('|');
            let pos = parts.next().unwrap_or("").trim().trim_matches(['(', ')']).to_string();
            let words = parts.map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).take(MAX_WORDS).collect();
            Meaning { pos, words }
        })
        .filter(|m| !m.words.is_empty())
        .collect()
}

/// The meanings of `word` in the thesaurus for `tag` (empty when there is none, or the word
/// isn't in it).
pub fn lookup(word: &str, tag: &str) -> Vec<Meaning> {
    let w = word.trim().to_lowercase();
    if w.is_empty() || w.len() > 200 {
        return Vec::new();
    }
    let Some(t) = thesaurus(tag) else { return Vec::new() };
    let Some(&off) = t.index.get(&w) else { return Vec::new() };
    let read = || -> std::io::Result<Vec<u8>> {
        let mut f = std::fs::File::open(&t.dat)?;
        f.seek(SeekFrom::Start(off))?;
        let mut buf = vec![0u8; MAX_ENTRY];
        let n = f.read(&mut buf)?;
        buf.truncate(n);
        Ok(buf)
    };
    match read() {
        Ok(bytes) => parse_entry(&decode(&bytes, t.latin1)),
        Err(e) => {
            log::warn!("thesaurus {}: {e}", t.dat.display());
            Vec::new()
        }
    }
}

/// Whether a thesaurus is installed for `tag`.
pub fn has(tag: &str) -> bool {
    thesaurus(tag).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_parse_and_junk_is_harmless() {
        let m = parse_entry("feliz|2\n(adj)|contente|alegre|satisfeito\n(adj)|afortunado\nnext|1\n");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].pos, "adj");
        assert_eq!(m[0].words, ["contente", "alegre", "satisfeito"]);
        assert!(parse_entry("").is_empty());
        assert!(parse_entry("x|99999999999999999999\n|||").is_empty());
        assert!(parse_entry("x|3\n(n)|\n").is_empty());
    }

    #[test]
    fn a_thesaurus_in_the_dictionaries_folder_is_found_and_read() {
        let dir = std::env::temp_dir().join(format!("wordcraft-thes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dat = "UTF-8\nfeliz|1\n(adj)|contente|alegre\nrápido|1\n(adj)|veloz|ligeiro\n";
        let off_feliz = "UTF-8\n".len();
        let off_rapido = "UTF-8\nfeliz|1\n(adj)|contente|alegre\n".len();
        let idx = format!("UTF-8\n2\nfeliz|{off_feliz}\nrápido|{off_rapido}\n");
        std::fs::write(dir.join("th_xx_YY_v2.dat"), dat).unwrap();
        std::fs::write(dir.join("th_xx_YY_v2.idx"), idx).unwrap();
        crate::hunspell::set_dictionary_dir(Some(dir.clone()));
        reload();
        assert!(languages().contains(&"xx_YY".to_string()));
        assert_eq!(lookup("Rápido", "xx-YY")[0].words, ["veloz", "ligeiro"]);
        assert_eq!(lookup("feliz", "xx")[0].words, ["contente", "alegre"], "the language alone finds it");
        assert!(lookup("triste", "xx-YY").is_empty());
        assert!(lookup("feliz", "zz").is_empty());
        crate::hunspell::set_dictionary_dir(None);
        reload();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

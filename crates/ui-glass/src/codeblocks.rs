//! Research mode: notebook-style code blocks. A code block is a one-cell table, shaded, in a
//! monospaced font with its syntax coloured; its alt text (`wcplus-code:python#3`) says what
//! language it is in. Run it and its output (text, and the pictures it draws) appears in an
//! output block right below, replaced on every run.
//!
//! Code runs with the interpreters installed on this computer: `python3`, `Rscript`, `julia`,
//! and `octave` (or `matlab`) for MATLAB code. Each run gets a folder in `WC_OUT_DIR`; pictures
//! saved there are shown. Matplotlib figures, R plots and Octave figures are saved there
//! automatically.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Ui, vec2};
use serde_json::json;
use wordcraft_doc::props::{CharProps, Rgb, TextColor};
use wordcraft_doc::{Block, Paragraph, Pos, Run, StoryRef, Table};
use wordcraft_engine::Selection;
use wordcraft_ui_egui::theme::{regular, semibold};

use crate::GlassApp;
use crate::tokens::Tokens;

/// The alt text that marks a code block (`wcplus-code:python#3`).
pub const CODE_TAG: &str = "wcplus-code:";
/// The alt text that marks a code block's output.
pub const OUT_TAG: &str = "wcplus-output:";
/// The font code is set in.
pub const CODE_FONT: &str = "Courier New";
/// A run is stopped after this long.
const TIMEOUT_SECS: u64 = 120;
/// Output kept (characters), and pictures shown.
const MAX_OUTPUT: usize = 20_000;
const MAX_PICTURES: usize = 8;
/// Lines of code a block can hold.
const MAX_LINES: usize = 2_000;

/// A language code blocks can be written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Python,
    R,
    Matlab,
    Julia,
}

impl Lang {
    pub const ALL: [Lang; 4] = [Lang::Python, Lang::R, Lang::Matlab, Lang::Julia];

    pub fn name(self) -> &'static str {
        match self {
            Lang::Python => "Python",
            Lang::R => "R",
            Lang::Matlab => "MATLAB",
            Lang::Julia => "Julia",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Lang::Python => "python",
            Lang::R => "r",
            Lang::Matlab => "matlab",
            Lang::Julia => "julia",
        }
    }

    pub fn from_id(s: &str) -> Option<Lang> {
        match s.trim().to_ascii_lowercase().as_str() {
            "python" | "py" | "python3" => Some(Lang::Python),
            "r" => Some(Lang::R),
            "matlab" | "octave" | "m" => Some(Lang::Matlab),
            "julia" | "jl" => Some(Lang::Julia),
            _ => None,
        }
    }

    fn ext(self) -> &'static str {
        match self {
            Lang::Python => "py",
            Lang::R => "R",
            Lang::Matlab => "m",
            Lang::Julia => "jl",
        }
    }

    fn comment(self) -> &'static str {
        match self {
            Lang::Matlab => "%",
            _ => "#",
        }
    }

    fn keywords(self) -> &'static [&'static str] {
        match self {
            Lang::Python => &[
                "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else",
                "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
                "return", "try", "while", "with", "yield", "print", "range", "len",
            ],
            Lang::R => &[
                "if", "else", "repeat", "while", "function", "for", "in", "next", "break", "TRUE", "FALSE", "NULL", "Inf", "NaN", "NA", "library",
                "return",
            ],
            Lang::Matlab => &[
                "break",
                "case",
                "catch",
                "continue",
                "else",
                "elseif",
                "end",
                "for",
                "function",
                "global",
                "if",
                "otherwise",
                "persistent",
                "return",
                "switch",
                "try",
                "while",
                "disp",
                "plot",
                "figure",
                "hold",
                "on",
                "off",
            ],
            Lang::Julia => &[
                "begin", "break", "catch", "const", "continue", "do", "else", "elseif", "end", "export", "false", "finally", "for", "function",
                "global", "if", "import", "let", "local", "macro", "module", "quote", "return", "struct", "true", "try", "using", "while", "println",
                "in",
            ],
        }
    }

    /// The programs that run it, in order of preference, with their arguments before the file.
    fn programs(self) -> &'static [(&'static str, &'static [&'static str])] {
        match self {
            Lang::Python => &[("python3", &[]), ("python", &[])],
            Lang::R => &[("Rscript", &["--vanilla"])],
            Lang::Matlab => &[("octave", &["--no-gui", "--quiet", "--no-window-system"]), ("matlab", &["-batch"])],
            Lang::Julia => &[("julia", &[])],
        }
    }

    /// Lines run before the code (where plots go).
    fn prelude(self) -> &'static str {
        match self {
            Lang::Python => "import os as _wc_os\n_wc_os.environ.setdefault('MPLBACKEND', 'Agg')\n",
            Lang::R => "png(file.path(Sys.getenv('WC_OUT_DIR'), 'plot%03d.png'), width = 1200, height = 800, res = 160)\n",
            Lang::Matlab => "set(0, 'DefaultFigureVisible', 'off');\n",
            Lang::Julia => "",
        }
    }

    /// Lines run after the code (save the plots it drew).
    fn postlude(self) -> &'static str {
        match self {
            Lang::Python => {
                "\nimport sys as _wc_sys\nif 'matplotlib.pyplot' in _wc_sys.modules:\n    import matplotlib.pyplot as _wc_plt\n    for _wc_i in _wc_plt.get_fignums():\n        _wc_plt.figure(_wc_i).savefig(_wc_os.path.join(_wc_os.environ['WC_OUT_DIR'], 'figure%03d.png' % _wc_i), dpi=160, bbox_inches='tight')\n"
            }
            Lang::R => "\ninvisible(dev.off())\n",
            Lang::Matlab => {
                "\n_wc_figs = get(0, 'children');\nfor _wc_i = 1:numel(_wc_figs)\n  print(_wc_figs(_wc_i), fullfile(getenv('WC_OUT_DIR'), sprintf('figure%03d.png', _wc_i)), '-dpng', '-r160');\nend\n"
            }
            Lang::Julia => "",
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Syntax colouring.

/// What a stretch of code is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Plain,
    Keyword,
    Str,
    Comment,
    Number,
}

fn tok_color(k: Tok) -> Option<Rgb> {
    match k {
        Tok::Plain => None,
        Tok::Keyword => Some(Rgb(0x8E, 0x3B, 0xB0)),
        Tok::Str => Some(Rgb(0x2E, 0x7D, 0x32)),
        Tok::Comment => Some(Rgb(0x80, 0x80, 0x7A)),
        Tok::Number => Some(Rgb(0x1F, 0x5F, 0xBF)),
    }
}

/// One line of code as (byte length, kind) pieces.
pub fn highlight(lang: Lang, line: &str) -> Vec<(usize, Tok)> {
    let mut out: Vec<(usize, Tok)> = Vec::new();
    let mut push = |len: usize, k: Tok| {
        if len == 0 {
            return;
        }
        match out.last_mut() {
            Some((l, last)) if *last == k => *l += len,
            _ => out.push((len, k)),
        }
    };
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let end_of = |k: usize| chars.get(k).map(|(i, _)| *i).unwrap_or(line.len());
    let mut k = 0;
    while let Some(&(i, c)) = chars.get(k) {
        let rest = line.get(i..).unwrap_or("");
        if rest.starts_with(lang.comment()) {
            push(line.len() - i, Tok::Comment);
            break;
        }
        // Strings ('…' or "…"); in MATLAB a ' after a name is the transpose.
        let transpose =
            lang == Lang::Matlab && c == '\'' && k > 0 && chars.get(k - 1).is_some_and(|(_, p)| p.is_alphanumeric() || matches!(p, ')' | ']' | '\''));
        if (c == '"' || c == '\'') && !transpose {
            let mut j = k + 1;
            while let Some(&(_, d)) = chars.get(j) {
                if d == '\\' {
                    j += 2;
                    continue;
                }
                j += 1;
                if d == c {
                    break;
                }
            }
            let j = j.min(chars.len());
            push(end_of(j) - i, Tok::Str);
            k = j;
            continue;
        }
        if c.is_ascii_digit() && (k == 0 || chars.get(k - 1).is_some_and(|(_, p)| !p.is_alphanumeric() && *p != '_')) {
            let mut j = k;
            while chars.get(j).is_some_and(|(_, d)| d.is_ascii_alphanumeric() || *d == '.' || *d == '_') {
                j += 1;
            }
            push(end_of(j) - i, Tok::Number);
            k = j;
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut j = k;
            while chars.get(j).is_some_and(|(_, d)| d.is_alphanumeric() || *d == '_' || (lang == Lang::R && *d == '.')) {
                j += 1;
            }
            let word = line.get(i..end_of(j)).unwrap_or("");
            push(word.len(), if lang.keywords().contains(&word) { Tok::Keyword } else { Tok::Plain });
            k = j;
            continue;
        }
        push(c.len_utf8(), Tok::Plain);
        k += 1;
    }
    out
}

fn code_props(color: Option<Rgb>) -> CharProps {
    CharProps { font: Some(CODE_FONT.into()), size: Some(9.5), color: color.map(TextColor::Rgb), no_proof: Some(true), ..Default::default() }
}

/// A paragraph of code with its colours.
fn code_para(lang: Option<Lang>, line: &str) -> Paragraph {
    let mut p = Paragraph::with_text(line, code_props(None));
    if let Some(lang) = lang {
        p.runs = highlight(lang, &p.text).into_iter().map(|(len, k)| Run { len, props: code_props(tok_color(k)) }).collect();
    }
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = None;
    p
}

/// A one-cell table holding `lines`, shaded, marked with `tag`.
fn block_table(lang: Option<Lang>, lines: &[&str], tag: &str, title: &str, shade: Rgb) -> Table {
    let mut t = Table::new(1, 1, 468.0);
    t.props.description = Some(tag.to_string());
    t.props.caption = Some(title.to_string());
    let lines: Vec<&str> = if lines.is_empty() { vec![""] } else { lines.iter().take(MAX_LINES).copied().collect() };
    if let Some(cell) = t.rows.get_mut(0).and_then(|r| r.cells.get_mut(0)) {
        cell.props.shading = Some(shade);
        cell.props.margins = Some([6.0, 8.0, 6.0, 8.0]);
        cell.blocks = lines.iter().map(|l| wordcraft_doc::para_block(code_para(lang, l))).collect();
    }
    t
}

/// The table that holds a code block.
pub fn code_table(lang: Lang, code: &str, id: u64) -> Table {
    let lines: Vec<&str> = code.lines().collect();
    block_table(Some(lang), &lines, &format!("{CODE_TAG}{}#{id}", lang.id()), &format!("{} code", lang.name()), Rgb(0xF4, 0xF4, 0xF1))
}

/// The table that holds a code block's output (text; pictures are added after).
pub fn output_table(text: &str, ok: bool, id: u64) -> Table {
    let lines: Vec<&str> = text.lines().collect();
    let shade = if ok { Rgb(0xFA, 0xFA, 0xF8) } else { Rgb(0xFC, 0xEE, 0xEC) };
    block_table(None, &lines, &format!("{OUT_TAG}{id}"), "Output", shade)
}

// ---------------------------------------------------------------------------------------------
// Code blocks in the document.

/// A code block found in the document.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeBlock {
    /// Its index in the body.
    pub block: usize,
    pub lang: Lang,
    pub id: u64,
    pub code: String,
    /// Whether an output block follows it.
    pub has_output: bool,
}

fn cell_text(t: &Table) -> String {
    let Some(cell) = t.rows.first().and_then(|r| r.cells.first()) else { return String::new() };
    cell.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect::<Vec<_>>().join("\n")
}

fn parse_tag(desc: &str) -> Option<(Lang, u64)> {
    let rest = desc.strip_prefix(CODE_TAG)?;
    let (lang, id) = rest.split_once('#').unwrap_or((rest, "0"));
    Some((Lang::from_id(lang)?, id.trim().parse().unwrap_or(0)))
}

/// Every code block in the body.
pub fn code_blocks(doc: &wordcraft_doc::Document) -> Vec<CodeBlock> {
    let mut out = Vec::new();
    for (i, b) in doc.body.iter().enumerate() {
        let Block::Table(t) = b.as_ref() else { continue };
        let Some((lang, id)) = t.props.description.as_deref().and_then(parse_tag) else { continue };
        let has_output = matches!(doc.body.get(i + 1).map(|b| b.as_ref()), Some(Block::Table(o)) if o.props.description.as_deref().is_some_and(|d| d.starts_with(OUT_TAG)));
        out.push(CodeBlock { block: i, lang, id, code: cell_text(t), has_output });
        if out.len() >= 500 {
            break;
        }
    }
    out
}

/// What a finished run produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunOutput {
    pub text: String,
    pub ok: bool,
    pub pictures: Vec<PathBuf>,
}

/// Code running in the background, and the panel's draft.
#[derive(Default)]
pub struct Code {
    pub lang: Option<usize>,
    pub draft: String,
    running: Vec<(u64, Arc<std::sync::atomic::AtomicBool>, Receiver<RunOutput>)>,
    /// Interpreters found on this computer (looked up once).
    found: Option<Vec<(Lang, Option<String>)>>,
}

/// Look for a program on the PATH.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".bat", ".cmd", ""] } else { &[""] };
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    // GUI apps on macOS start with a short PATH: look where installers put these too.
    for extra in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/Library/Frameworks/R.framework/Resources/bin",
        "/Applications/Julia-1.11.app/Contents/Resources/julia/bin",
    ] {
        dirs.push(PathBuf::from(extra));
    }
    dirs.into_iter().flat_map(|d| exts.iter().map(move |e| d.join(format!("{name}{e}")))).find(|p| p.is_file())
}

/// The program (and its arguments) that runs `lang` here.
pub fn interpreter(lang: Lang) -> Option<(PathBuf, Vec<String>)> {
    lang.programs().iter().find_map(|(name, args)| which(name).map(|p| (p, args.iter().map(|a| a.to_string()).collect())))
}

/// Run `code` and collect what it printed and drew (blocking; call it on a thread).
pub fn run_code(lang: Lang, code: &str, stop: &std::sync::atomic::AtomicBool) -> RunOutput {
    let Some((prog, mut args)) = interpreter(lang) else {
        let need = lang.programs().iter().map(|(n, _)| *n).collect::<Vec<_>>().join(" or ");
        return RunOutput { text: format!("{} isn't installed on this computer (looked for {need}).", lang.name()), ok: false, pictures: Vec::new() };
    };
    let dir = std::env::temp_dir().join(format!("wordcraft-plus-run-{}-{}", std::process::id(), wordcraft_ui_egui::now_ms() as u64));
    let out_dir = dir.join("out");
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        return RunOutput { text: format!("Couldn't make a folder to run in: {e}"), ok: false, pictures: Vec::new() };
    }
    let script = dir.join(format!("block.{}", lang.ext()));
    let source = format!("{}{}\n{}", lang.prelude(), code, lang.postlude());
    if let Err(e) = std::fs::write(&script, source) {
        return RunOutput { text: format!("Couldn't write the code to run: {e}"), ok: false, pictures: Vec::new() };
    }
    let is_matlab = prog.file_stem().is_some_and(|s| s == "matlab");
    if is_matlab {
        args.push(format!("run('{}')", script.to_string_lossy().replace('\'', "''")));
    } else {
        args.push(script.to_string_lossy().into_owned());
    }
    let child = std::process::Command::new(&prog)
        .args(&args)
        .current_dir(&dir)
        .env("WC_OUT_DIR", &out_dir)
        .env("MPLBACKEND", "Agg")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return RunOutput { text: format!("Couldn't start {}: {e}", prog.display()), ok: false, pictures: Vec::new() },
    };
    // Read both pipes on threads so a chatty program can't fill one and stall.
    let mut readers = Vec::new();
    for pipe in [child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>), child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>)]
        .into_iter()
        .flatten()
    {
        readers.push(std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.take(4 << 20).read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        }));
    }
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {}
            Err(_) => break None,
        }
        if stop.load(std::sync::atomic::Ordering::Relaxed) || started.elapsed().as_secs() > TIMEOUT_SECS {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let texts: Vec<String> = readers.into_iter().map(|r| r.join().unwrap_or_default()).collect();
    let mut text = texts.iter().filter(|s| !s.trim().is_empty()).cloned().collect::<Vec<_>>().join("\n");
    let ok = status.is_some_and(|s| s.success());
    if status.is_none() {
        text.push_str(if stop.load(std::sync::atomic::Ordering::Relaxed) { "\n(stopped)" } else { "\n(stopped: it ran too long)" });
    }
    if text.chars().count() > MAX_OUTPUT {
        text = format!("{}\n…", text.chars().take(MAX_OUTPUT).collect::<String>());
    }
    let mut pictures: Vec<PathBuf> = std::fs::read_dir(&out_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "svg"))
                })
                .collect()
        })
        .unwrap_or_default();
    pictures.sort();
    pictures.truncate(MAX_PICTURES);
    RunOutput { text: text.trim_end().to_string(), ok, pictures }
}

impl GlassApp {
    /// Put `blocks` at the caret (through the clipboard, which is kept as it was).
    pub(crate) fn paste_blocks(&mut self, blocks: Vec<Block>) -> Result<(), String> {
        let s = &mut self.app.session;
        let saved = s.clipboard.take();
        s.clipboard = Some(wordcraft_doc::edit::Fragment { blocks, parts: Default::default() });
        let r = self.app.run("edit.paste", json!({}));
        self.app.session.clipboard = saved;
        r.map(|_| ())
    }

    /// A new code block id.
    fn next_code_id(&self) -> u64 {
        code_blocks(&self.app.session.doc).iter().map(|c| c.id).max().unwrap_or(0) + 1
    }

    /// Insert a code block at the caret.
    pub fn insert_code_block(&mut self, lang: Lang, code: &str) {
        let id = self.next_code_id();
        // On its own line: a table pasted mid-paragraph would split it.
        let s = &self.app.session;
        if s.doc.para_at(&s.sel.focus).is_some_and(|p| !p.text.is_empty()) {
            let _ = self.app.run("text.newParagraph", json!({}));
        }
        let code = if code.trim().is_empty() { "" } else { code };
        if let Err(e) = self.paste_blocks(vec![Block::Table(code_table(lang, code, id))]) {
            self.app.status(format!("Couldn't insert the code block: {e}"));
            return;
        }
        // Into the block, ready to type.
        if let Some(cb) = code_blocks(&self.app.session.doc).into_iter().find(|c| c.id == id)
            && let Some(cell) = self.app.session.doc.para_paths(StoryRef::Body).into_iter().find(|p| crate::top_of(p) == cb.block)
        {
            self.app.session.sel = Selection::caret(Pos::new(StoryRef::Body, cell, 0));
        }
        self.app.status(format!("{} code block inserted: write code, then Run it from Research › Code", lang.name()));
    }

    /// Run the code block `id` (its output replaces the last one when it finishes).
    pub fn run_code_block(&mut self, id: u64) {
        let Some(cb) = code_blocks(&self.app.session.doc).into_iter().find(|c| c.id == id) else { return };
        if self.glass.code.running.iter().any(|(r, _, _)| *r == id) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop2 = stop.clone();
        std::thread::spawn(move || {
            let _ = tx.send(run_code(cb.lang, &cb.code, &stop2));
        });
        self.glass.code.running.push((id, stop, rx));
        self.app.status(format!("Running the {} block…", cb.lang.name()));
    }

    /// Run every code block, top to bottom.
    pub fn run_all_code(&mut self) {
        for cb in code_blocks(&self.app.session.doc) {
            self.run_code_block(cb.id);
        }
    }

    /// Whether block `id` is running.
    pub fn code_running(&self, id: u64) -> bool {
        self.glass.code.running.iter().any(|(r, _, _)| *r == id)
    }

    /// Stop every run.
    pub fn stop_code(&mut self) {
        for (_, stop, _) in &self.glass.code.running {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Recolour a block's code (after editing it).
    pub fn recolor_code_block(&mut self, id: u64) {
        let Some(cb) = code_blocks(&self.app.session.doc).into_iter().find(|c| c.id == id) else { return };
        let saved = self.app.session.sel.clone();
        if self.replace_block(cb.block, vec![Block::Table(code_table(cb.lang, &cb.code, id))]).is_ok() {
            self.app.session.sel = saved;
            if self.app.session.doc.para_at(&self.app.session.sel.focus).is_none() {
                let _ = self.app.run("caret.docStart", json!({}));
            }
        }
    }

    /// Replace body block `i` (a table) with `blocks`.
    fn replace_block(&mut self, i: usize, blocks: Vec<Block>) -> Result<(), String> {
        let Some(cell) = self.app.session.doc.para_paths(StoryRef::Body).into_iter().find(|p| crate::top_of(p) == i) else {
            return Err("the block is gone".into());
        };
        self.app.session.sel = Selection::caret(Pos::new(StoryRef::Body, cell, 0));
        self.app.run("table.deleteTable", json!({}))?;
        // The caret is now at the start of what followed the table.
        let at = self.app.session.doc.para_paths(StoryRef::Body).into_iter().find(|p| crate::top_of(p) >= i);
        match at {
            Some(p) => self.app.session.sel = Selection::caret(Pos::new(StoryRef::Body, p, 0)),
            None => {
                let _ = self.app.run("caret.docEnd", json!({}));
                let _ = self.app.run("text.newParagraph", json!({}));
            }
        }
        self.paste_blocks(blocks)
    }

    /// Pick up finished runs and put their output in the document.
    pub(crate) fn code_tick(&mut self) {
        let mut done: Vec<(u64, RunOutput)> = Vec::new();
        self.glass.code.running.retain(|(id, _, rx)| match rx.try_recv() {
            Ok(out) => {
                done.push((*id, out));
                false
            }
            Err(TryRecvError::Empty) => true,
            Err(TryRecvError::Disconnected) => false,
        });
        for (id, out) in done {
            self.show_output(id, &out);
        }
    }

    fn show_output(&mut self, id: u64, out: &RunOutput) {
        let Some(cb) = code_blocks(&self.app.session.doc).into_iter().find(|c| c.id == id) else { return };
        let saved = self.app.session.sel.clone();
        let text = if out.text.is_empty() && out.pictures.is_empty() { "(no output)".to_string() } else { out.text.clone() };
        // Recolour the code, and replace (or add) the output.
        let mut blocks = vec![Block::Table(code_table(cb.lang, &cb.code, id)), Block::Table(output_table(&text, out.ok, id))];
        if cb.has_output && self.replace_block(cb.block + 1, Vec::new()).is_err() {
            blocks.clear();
        }
        let r = if blocks.is_empty() { Err("the output couldn't be replaced".to_string()) } else { self.replace_block(cb.block, blocks) };
        if let Err(e) = r {
            self.app.status(format!("Couldn't show the output: {e}"));
            self.app.session.sel = saved;
            return;
        }
        // Pictures go in the output block, after its text.
        if !out.pictures.is_empty()
            && let Some(cb) = code_blocks(&self.app.session.doc).into_iter().find(|c| c.id == id)
            && let Some(last) = self.app.session.doc.para_paths(StoryRef::Body).into_iter().rfind(|p| crate::top_of(p) == cb.block + 1)
        {
            let len = self.app.session.doc.para(StoryRef::Body, &last).map(|p| p.text.len()).unwrap_or(0);
            self.app.session.sel = Selection::caret(Pos::new(StoryRef::Body, last, len));
            for pic in &out.pictures {
                if self.app.session.doc.para_at(&self.app.session.sel.focus).is_some_and(|p| !p.text.is_empty()) {
                    let _ = self.app.run("text.newParagraph", json!({}));
                }
                if let Err(e) = self.app.run(
                    "insert.picture",
                    json!({"path": pic.to_string_lossy(), "width": 400, "alt": format!("Output of the {} block", cb.lang.name())}),
                ) {
                    self.app.status(format!("Couldn't show a picture: {e}"));
                }
            }
        }
        self.app.session.sel = saved;
        if self.app.session.doc.para_at(&self.app.session.sel.focus).is_none() {
            let _ = self.app.run("caret.docStart", json!({}));
        }
        self.app.status(if out.ok {
            "Code ran: its output is below the block"
        } else {
            "The code stopped with an error: see the output below the block"
        });
    }
}

/// Where the code block under the caret is.
fn block_at_caret(g: &GlassApp) -> Option<u64> {
    let top = crate::top_of(&g.app.session.sel.focus.path);
    code_blocks(&g.app.session.doc).into_iter().find(|c| c.block == top).map(|c| c.id)
}

/// The Code panel: write and insert a block, run blocks, see which languages can run.
pub fn panel(g: &mut GlassApp, ui: &mut Ui, t: &Tokens) {
    let found = g
        .glass
        .code
        .found
        .get_or_insert_with(|| Lang::ALL.iter().map(|l| (*l, interpreter(*l).map(|(p, _)| p.to_string_lossy().into_owned()))).collect())
        .clone();
    let li = g.glass.code.lang.unwrap_or(0).min(Lang::ALL.len() - 1);
    let lang = Lang::ALL.get(li).copied().unwrap_or(Lang::Python);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (k, l) in Lang::ALL.iter().enumerate() {
            if ui.selectable_label(k == li, l.name()).clicked() {
                g.glass.code.lang = Some(k);
            }
        }
    });
    ui.add_space(6.0);
    ui.add(
        egui::TextEdit::multiline(&mut g.glass.code.draft)
            .font(egui::FontId::monospace(12.0))
            .hint_text(match lang {
                Lang::Python => "import matplotlib.pyplot as plt\nplt.plot([1, 4, 9])",
                Lang::R => "x <- rnorm(100)\nhist(x)",
                Lang::Matlab => "x = linspace(0, 2*pi);\nplot(x, sin(x))",
                Lang::Julia => "println(sum(1:10))",
            })
            .desired_rows(5)
            .desired_width(ui.available_width()),
    );
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        if crate::widgets::quiet_button(ui, t, "Insert Code Block", None, 28.0).clicked() {
            let code = std::mem::take(&mut g.glass.code.draft);
            g.insert_code_block(lang, &code);
        }
        let here = block_at_caret(g);
        if crate::widgets::quiet_button(ui, t, "Run Block at Cursor", None, 28.0).clicked() {
            match here {
                Some(id) => g.run_code_block(id),
                None => g.app.status("Put the cursor in a code block first"),
            }
        }
        if crate::widgets::quiet_button(ui, t, "Run All", None, 28.0).clicked() {
            g.run_all_code();
        }
        if !g.glass.code.running.is_empty() && crate::widgets::quiet_button(ui, t, "Stop", None, 28.0).clicked() {
            g.stop_code();
        }
    });
    ui.add_space(10.0);
    ui.label(egui::RichText::new("IN THIS DOCUMENT").font(semibold(10.5)).color(t.text_dim));
    let blocks = code_blocks(&g.app.session.doc);
    if blocks.is_empty() {
        ui.add(
            egui::Label::new(
                egui::RichText::new("No code blocks yet. Insert one here, or type /code in the document.").font(regular(12.0)).color(t.text_dim),
            )
            .wrap(),
        );
    }
    let mut run: Option<u64> = None;
    let mut recolor: Option<u64> = None;
    let mut go: Option<usize> = None;
    egui::ScrollArea::vertical().id_salt("glass_code_blocks").max_height(260.0).show(ui, |ui| {
        for cb in &blocks {
            let first = cb.code.lines().find(|l| !l.trim().is_empty()).unwrap_or("(empty)");
            let first: String = first.chars().take(40).collect();
            ui.horizontal(|ui| {
                let busy = g.code_running(cb.id);
                if ui.add(egui::Button::new(if busy { "…" } else { "▶" }).min_size(vec2(26.0, 22.0))).on_hover_text("Run").clicked() {
                    run = Some(cb.id);
                }
                if ui.link(egui::RichText::new(format!("{} · {first}", cb.lang.name())).font(egui::FontId::monospace(11.5))).clicked() {
                    go = Some(cb.block);
                }
                if ui.small_button("Recolour").on_hover_text("Colour the code again after editing it").clicked() {
                    recolor = Some(cb.id);
                }
            });
        }
    });
    if let Some(id) = run {
        g.run_code_block(id);
    }
    if let Some(id) = recolor {
        g.recolor_code_block(id);
    }
    if let Some(b) = go
        && let Some(p) = g.app.session.doc.para_paths(StoryRef::Body).into_iter().find(|p| crate::top_of(p) == b)
    {
        g.app.session.sel = Selection::caret(Pos::new(StoryRef::Body, p, 0));
        g.app.canvas.scroll_to_caret = true;
    }
    ui.add_space(10.0);
    ui.label(egui::RichText::new("ON THIS COMPUTER").font(semibold(10.5)).color(t.text_dim));
    for (l, at) in &found {
        let (txt, c) = match at {
            Some(p) => (format!("{}: {}", l.name(), Path::new(p).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()), t.text),
            None => (format!("{}: not installed", l.name()), t.text_dim),
        };
        ui.label(egui::RichText::new(txt).font(regular(11.5)).color(c));
    }
    if ui.small_button("Look Again").clicked() {
        g.glass.code.found = None;
    }
    if !g.glass.code.running.is_empty() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_engine::Session;
    use wordcraft_ui_egui::{Services, WordApp};

    #[test]
    fn code_is_coloured() {
        let toks = highlight(Lang::Python, "for x in range(3):  # loop 'a'");
        assert_eq!(toks.first(), Some(&(3, Tok::Keyword)));
        assert_eq!(toks.last().map(|t| t.1), Some(Tok::Comment));
        let toks = highlight(Lang::Matlab, "y = x' + 'str'; % c");
        assert!(toks.contains(&(5, Tok::Str)), "{toks:?}");
        assert_eq!(highlight(Lang::R, "x <- 1.5e3").last(), Some(&(5, Tok::Number)));
        // The pieces always add up to the line.
        for line in ["print(\"unterminated", "é = 'ç'", "", "a\\'b"] {
            assert_eq!(highlight(Lang::Python, line).iter().map(|p| p.0).sum::<usize>(), line.len());
        }
    }

    #[test]
    fn a_code_block_is_inserted_found_and_given_its_output() {
        let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default()));
        g.app.run("text.insert", json!({"text": "Before"})).unwrap();
        g.insert_code_block(Lang::Python, "print(1 + 1)\nx = 'two'");
        let blocks = code_blocks(&g.app.session.doc);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].code, "print(1 + 1)\nx = 'two'");
        assert!(!blocks[0].has_output);
        let id = blocks[0].id;
        g.show_output(id, &RunOutput { text: "2".into(), ok: true, pictures: Vec::new() });
        let blocks = code_blocks(&g.app.session.doc);
        assert_eq!(blocks.len(), 1, "the block is kept");
        assert!(blocks[0].has_output);
        // A second run replaces the output rather than adding another.
        g.show_output(id, &RunOutput { text: "3".into(), ok: true, pictures: Vec::new() });
        let outs = g
            .app
            .session
            .doc
            .body
            .iter()
            .filter(|b| matches!(b.as_ref(), Block::Table(t) if t.props.description.as_deref().is_some_and(|d| d.starts_with(OUT_TAG))))
            .count();
        assert_eq!(outs, 1);
        assert!(g.app.session.doc.plain_text(StoryRef::Body).contains('3'));
        assert!(g.app.session.doc.plain_text(StoryRef::Body).contains("Before"));
    }

    #[test]
    fn python_runs_when_installed() {
        if interpreter(Lang::Python).is_none() {
            return;
        }
        let out = run_code(Lang::Python, "print(6 * 7)", &std::sync::atomic::AtomicBool::new(false));
        assert!(out.ok, "{}", out.text);
        assert_eq!(out.text, "42");
        let bad = run_code(Lang::Python, "raise ValueError('nope')", &std::sync::atomic::AtomicBool::new(false));
        assert!(!bad.ok);
        assert!(bad.text.contains("nope"));
        // A matplotlib figure comes back as a picture, shown in the output block.
        let plot = run_code(
            Lang::Python,
            "import matplotlib.pyplot as plt\nplt.plot([1, 4, 9])\nprint('drawn')",
            &std::sync::atomic::AtomicBool::new(false),
        );
        if plot.ok {
            assert_eq!(plot.pictures.len(), 1, "{plot:?}");
            let mut g = GlassApp::new(WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default()));
            g.insert_code_block(Lang::Python, "plt.plot([1, 4, 9])");
            let id = code_blocks(&g.app.session.doc)[0].id;
            g.show_output(id, &plot);
            assert_eq!(g.app.session.doc.media.len(), 1);
        }
    }
}

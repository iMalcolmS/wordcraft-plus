//! Headless screenshots of the Glass UI: renders the whole window offscreen (wgpu).
//!
//! `cargo run -p wordcraft-ui-glass --example glass_shot -- script.jsonl`
//!
//! The script is JSON lines, as for `wordcraft-ui-egui`'s `ui_shot`: control-channel requests
//! (`{"method": …, "params": …}`), `{"shot": "/abs/out.png"}`, `{"steps": n}`, plus Glass's own:
//! `{"glass": "snapshots" | "history" | "goals" | "typewriter"}` toggles a feature,
//! `{"tab": "Insert"}` picks a ribbon tab, `{"words": n}` sets today's words written and
//! `{"snapshot": "name"}` takes a snapshot and `{"select": i}` selects version `i`;
//! `{"chooser": "Databases"}` opens Choose a Template (on a category; `"select"` selects one),
//! `{"dbview": i}` shows a database's view `i` and `{"peek": i}` opens its record `i`;
//! `{"pro": true}` turns Writer Pro on, `{"home": true}` shows Home, `{"template": "letter"}`
//! opens a template in a tab and `{"run": "text.insert", "params": {…}}` runs a command;
//! `{"prefs": {"glow": "Sentence"}}` merges Glass preferences, and `{"pointer": [x, y]}` moves
//! the pointer (with `"button": "down" | "up"` it presses or releases there). Research mode:
//! `{"research": "library" | "check" | "formulas" | "graph" | "argument" | "data" | "code" |
//! "submit" | "on"}` turns it on and shows a panel, `{"bib": "@article{…}"}` adds references to
//! the Library, `{"cite": "key"}` cites one, `{"codeblock": "python", "code": "…"}` inserts a
//! code block and `{"runcode": true}` runs them all and waits. The window is 1440×900 pt at 2× and opens the sample
//! document unless the first line is `{"empty": true}`.

use wordcraft_engine::Session;
use wordcraft_ui_egui::{ControlRequest, Services, WordApp};
use wordcraft_ui_glass::GlassApp;
use wordcraft_ui_glass::ribbon::Local;

static READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn main() {
    let path = std::env::args().nth(1).expect("usage: glass_shot script.jsonl");
    let script = std::fs::read_to_string(&path).expect("read script");
    let (tx, rx) = std::sync::mpsc::channel::<ControlRequest>();
    let lines: Vec<serde_json::Value> =
        script.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).expect("json line")).collect();
    let doc = if lines.first().is_some_and(|l| l.get("empty").is_some()) {
        wordcraft_doc::Document::new()
    } else {
        wordcraft_engine::sample::sample_document()
    };
    let mut app = WordApp::new(Session::new(doc), Services::default()).with_control(rx);
    // `{"mac": true}`: lay out for macOS's integrated title bar (the traffic lights aren't drawn).
    app.integrated_titlebar = lines.iter().any(|l| l.get("mac").is_some());
    let w = lines.iter().find_map(|l| l.get("width").and_then(|v| v.as_f64())).unwrap_or(1440.0) as f32;
    let h = lines.iter().find_map(|l| l.get("height").and_then(|v| v.as_f64())).unwrap_or(900.0) as f32;
    let mut harness =
        egui_kittest::Harness::builder().with_size(egui::vec2(w, h)).with_pixels_per_point(2.0).with_max_steps(1_000_000).wgpu().build_ui_state(
            |ui, g: &mut GlassApp| {
                if !READY.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let ctx = ui.ctx().clone();
                g.logic(&ctx);
                g.ui(ui);
            },
            GlassApp::new(app),
        );
    harness.input_mut().max_texture_side = Some(8192);
    READY.store(true, std::sync::atomic::Ordering::Relaxed);
    step_n(&mut harness, 6);
    for l in lines {
        if let Some(p) = l.get("shot").and_then(|v| v.as_str()) {
            // Let time-based fades (tab and view switches) finish.
            std::thread::sleep(std::time::Duration::from_millis(300));
            step_n(&mut harness, 4);
            match harness.render() {
                Ok(img) => {
                    img.save(p).expect("save png");
                    println!("{p}");
                }
                Err(e) => eprintln!("render failed: {e}"),
            }
        } else if let Some(n) = l.get("steps").and_then(|v| v.as_u64()) {
            step_n(&mut harness, n as usize);
        } else if let Some(f) = l.get("glass").and_then(|v| v.as_str()) {
            let local = match f {
                "snapshots" => Local::Snapshots,
                "history" => Local::History,
                "goals" => Local::Goals,
                "corkboard" => Local::Corkboard,
                "outliner" => Local::Outliner,
                "page" => Local::Page,
                "binder" => Local::Binder,
                "scratchpad" => Local::Scratchpad,
                "cut" => Local::CutToScratchpad,
                "pacing" => Local::Pacing,
                "sprint" => Local::Sprint,
                "appearance" => Local::Appearance,
                "inspector" => Local::Inspector,
                "split" => Local::Split,
                _ => Local::Typewriter,
            };
            harness.state_mut().toggle(local);
            step_n(&mut harness, 2);
        } else if let Some(patch) = l.get("prefs").and_then(|v| v.as_object()) {
            // Merge keys into Glass's preferences.
            let g = harness.state_mut();
            let mut v = serde_json::to_value(&g.glass.prefs).expect("prefs");
            if let Some(o) = v.as_object_mut() {
                for (k, x) in patch {
                    o.insert(k.clone(), x.clone());
                }
            }
            g.glass.prefs = serde_json::from_value(v).expect("prefs");
            step_n(&mut harness, 2);
        } else if let Some(p) = l.get("pointer").and_then(|v| v.as_array()) {
            let pos = egui::pos2(p.first().and_then(|x| x.as_f64()).unwrap_or(0.0) as f32, p.get(1).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32);
            harness.input_mut().events.push(egui::Event::PointerMoved(pos));
            // A press and its release must land within egui's click time: one frame each.
            step_n(&mut harness, if l.get("button").is_some() { 1 } else { 3 });
            if let Some(btn) = l.get("button").and_then(|v| v.as_str()) {
                harness.input_mut().events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: btn == "down",
                    modifiers: egui::Modifiers::NONE,
                });
                step_n(&mut harness, 1);
            }
        } else if let Some(id) = l.get("print").and_then(|v| v.as_str()) {
            let r = harness.state_mut().app.session.run(id, &serde_json::json!({}));
            println!("{id}: {}", r.map(|v| v.to_string()).unwrap_or_else(|e| e.to_string()).chars().take(200).collect::<String>());
        } else if let Some(b) = l.get("quick").and_then(|v| v.as_u64()) {
            harness.state_mut().glass.scriv.quick = Some(wordcraft_engine::doc::Path::top(b as usize));
            step_n(&mut harness, 4);
        } else if let Some(b) = l.get("blockmenu").and_then(|v| v.as_u64()) {
            harness.state_mut().glass.overlay.menu = Some((b as usize, egui::pos2(380.0, 360.0)));
            step_n(&mut harness, 10);
        } else if let Some(b) = l.get("topage").and_then(|v| v.as_u64()) {
            let r = wordcraft_ui_glass::overlay::turn_into_page(harness.state_mut(), b as usize);
            println!("turn into page: {r:?}");
            step_n(&mut harness, 4);
        } else if let Some(tab) = l.get("navtab").and_then(|v| v.as_str()) {
            harness.state_mut().app.ui.nav_tab = tab.to_string();
            step_n(&mut harness, 2);
        } else if let Some(v) = l.get("research").and_then(|v| v.as_str()) {
            use wordcraft_ui_glass::research::RView;
            let g = harness.state_mut();
            g.set_research(true);
            let view = match v {
                "library" => Some(RView::Library),
                "check" => Some(RView::Check),
                "formulas" => Some(RView::Formulas),
                "graph" => Some(RView::Graph),
                "argument" => Some(RView::Argument),
                "data" => Some(RView::Data),
                "code" => Some(RView::Code),
                "submit" => Some(RView::Submit),
                _ => None,
            };
            g.glass.panel = view.map(wordcraft_ui_glass::Panel::Research);
            step_n(&mut harness, 40);
        } else if let Some(b) = l.get("bib").and_then(|v| v.as_str()) {
            let refs = wordcraft_ui_glass::refs::parse_bibtex(b);
            harness.state_mut().library_mut().merge(refs);
            step_n(&mut harness, 2);
        } else if let Some(k) = l.get("cite").and_then(|v| v.as_str()) {
            let g = harness.state_mut();
            if let Some(r) = g.library_mut().find(k).cloned() {
                g.cite(&r);
            }
            step_n(&mut harness, 2);
        } else if let Some(lang) = l.get("codeblock").and_then(|v| v.as_str()) {
            let code = l.get("code").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(lang) = wordcraft_ui_glass::codeblocks::Lang::from_id(lang) {
                harness.state_mut().insert_code_block(lang, code);
            }
            step_n(&mut harness, 2);
        } else if l.get("runcode").is_some() {
            harness.state_mut().run_all_code();
            for _ in 0..300 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                step(&mut harness);
                let g = harness.state_mut();
                let ids: Vec<u64> = wordcraft_ui_glass::codeblocks::code_blocks(&g.app.session.doc).iter().map(|c| c.id).collect();
                if !ids.iter().any(|id| g.code_running(*id)) {
                    break;
                }
            }
            step_n(&mut harness, 4);
        } else if let Some(on) = l.get("pro").and_then(|v| v.as_bool()) {
            harness.state_mut().set_pro(on);
            step_n(&mut harness, 2);
        } else if l.get("home").is_some() {
            harness.state_mut().set_view(wordcraft_ui_glass::View::Home);
            step_n(&mut harness, 30);
        } else if let Some(kind) = l.get("template").and_then(|v| v.as_str()) {
            match wordcraft_ui_glass::templates::find(kind) {
                Some(t) => harness.state_mut().create_from(&wordcraft_ui_glass::chooser::Pick::Builtin(t.id)),
                None => harness.state_mut().new_from_template(kind),
            }
            step_n(&mut harness, 4);
        } else if let Some(c) = l.get("chooser") {
            let cat = c.as_str().and_then(|n| wordcraft_ui_glass::templates::Category::ALL.into_iter().find(|x| x.name() == n));
            harness.state_mut().choose_template(cat);
            if let Some(sel) = l.get("select").and_then(|v| v.as_str()).and_then(wordcraft_ui_glass::templates::find) {
                harness.state_mut().glass.chooser.selected = Some(wordcraft_ui_glass::chooser::Pick::Builtin(sel.id));
            }
            // Thumbnails render a couple per frame.
            step_n(&mut harness, 60);
        } else if let Some(v) = l.get("dbview").and_then(|v| v.as_u64()) {
            if let Some(d) = harness.state_mut().active_db_mut() {
                d.current = v as usize;
            }
            step_n(&mut harness, 6);
        } else if let Some(i) = l.get("peek").and_then(|v| v.as_u64()) {
            let id = harness.state_mut().active_db().and_then(|d| d.rows.get(i as usize)).map(|r| r.id);
            harness.state_mut().glass.dbui.peek = id;
            step_n(&mut harness, 6);
        } else if let Some(p) = l.get("panel").and_then(|v| v.as_str()) {
            let g = harness.state_mut();
            match p {
                "dictionary" => g.glass.panel = Some(wordcraft_ui_glass::Panel::Dictionary),
                "labels" => g.glass.labels_open = true,
                _ => {}
            }
            step_n(&mut harness, 8);
        } else if l.get("labelcards").is_some() {
            // Give the first sections labels, to show colour coding.
            let g = harness.state_mut();
            let key = wordcraft_ui_glass::project::key(&g.app.session);
            let secs = wordcraft_ui_glass::project::sections(&g.app.session);
            let p = g.glass.projects.by_doc.entry(key).or_default();
            let tags: Vec<u64> = p.tags().iter().map(|t| t.id).collect();
            for (i, s) in secs.iter().enumerate() {
                p.card_mut(&s.title).tag = tags.get(i % 4).copied();
            }
            step_n(&mut harness, 4);
        } else if let Some(i) = l.get("splitdoc").and_then(|v| v.as_u64()) {
            let g = harness.state_mut();
            let uid = g.docs.tabs.get(i as usize).map(|t| t.uid);
            g.glass.panel = Some(wordcraft_ui_glass::Panel::Reference(wordcraft_ui_glass::scriv::RefView::Document(uid)));
            step_n(&mut harness, 30);
        } else if l.get("split_below").is_some() {
            harness.state_mut().glass.prefs.split_below = true;
            step_n(&mut harness, 4);
        } else if l.get("format").is_some() {
            harness.state_mut().glass.format_open = true;
            step_n(&mut harness, 4);
        } else if let Some(id) = l.get("run").and_then(|v| v.as_str()) {
            let p = l.get("params").cloned().unwrap_or_else(|| serde_json::json!({}));
            if let Err(e) = harness.state_mut().app.run(id, p) {
                eprintln!("{id}: {e}");
            }
            step_n(&mut harness, 2);
        } else if let Some(tab) = l.get("tab").and_then(|v| v.as_str()) {
            harness.state_mut().app.ui.tab = tab.to_string();
            step_n(&mut harness, 2);
        } else if let Some(n) = l.get("words").and_then(|v| v.as_i64()) {
            let today = GlassApp::today();
            harness.state_mut().glass.prefs.goals.today_mut(&today).words = n;
            harness.state_mut().glass.prefs.goals.today_mut(&today).secs = 42.0 * 60.0;
            step_n(&mut harness, 2);
        } else if l.get("file").is_some() {
            // Open the File menu under where the File tab sits.
            let g = harness.state_mut();
            g.glass.file_menu = true;
            g.glass.file_anchor = Some(egui::Rect::from_min_size(egui::pos2(240.0, 40.0), egui::vec2(52.0, 28.0)));
            step_n(&mut harness, 20);
        } else if let Some(i) = l.get("select").and_then(|v| v.as_u64()) {
            harness.state_mut().glass.selected_version = Some(i as usize);
            step_n(&mut harness, 2);
        } else if let Some(name) = l.get("snapshot").and_then(|v| v.as_str()) {
            let _ = harness.state_mut().app.session.run("file.versions", &serde_json::json!({"save": name}));
            step_n(&mut harness, 2);
        } else if let Some(m) = l.get("method").and_then(|v| v.as_str()) {
            let (req, reply) = ControlRequest::new(m, l.get("params").cloned().unwrap_or_default());
            tx.send(req).expect("send");
            for _ in 0..60 {
                step(&mut harness);
                if let Ok(r) = reply.try_recv() {
                    let s = r.to_string();
                    println!("{m}: {}", s.get(..s.len().min(300)).unwrap_or(&s));
                    break;
                }
            }
            step_n(&mut harness, 2);
        }
    }
}

fn step(harness: &mut egui_kittest::Harness<'_, GlassApp>) {
    let mut raw = std::mem::take(harness.input_mut());
    harness.state_mut().raw_input_hook(&mut raw);
    *harness.input_mut() = raw;
    harness.step();
}

fn step_n(harness: &mut egui_kittest::Harness<'_, GlassApp>, n: usize) {
    for _ in 0..n {
        step(harness);
    }
}

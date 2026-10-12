//! Headless screenshots of the Glass UI: renders the whole window offscreen (wgpu).
//!
//! `cargo run -p wordcraft-ui-glass --example glass_shot -- script.jsonl`
//!
//! The script is JSON lines, as for `wordcraft-ui-egui`'s `ui_shot`: control-channel requests
//! (`{"method": …, "params": …}`), `{"shot": "/abs/out.png"}`, `{"steps": n}`, plus Glass's own:
//! `{"glass": "snapshots" | "history" | "goals" | "typewriter"}` toggles a feature,
//! `{"tab": "Insert"}` picks a ribbon tab, `{"words": n}` sets today's words written and
//! `{"snapshot": "name"}` takes a snapshot and `{"select": i}` selects version `i`. The window is 1440×900 pt at 2× and opens the sample
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
                _ => Local::Typewriter,
            };
            harness.state_mut().toggle(local);
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

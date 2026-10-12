//! WordCraft Glass desktop app: the WordCraft engine and editor with the Glass front end.
//!
//! Usage: `wordcraft-glass [--control <port>] [--sample] [files…]`
//!
//! The platform glue (native file dialogs, printing, the system's animation setting, logging, the
//! control server and, on macOS, Finder's open-document events) is shared with `apps/wordcraft`:
//! the modules below are that app's own files, compiled into this one unchanged, so both apps
//! behave the same on every platform and fixes there reach Glass too.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

/// The classic app's platform modules, compiled here unchanged.
#[path = "../../wordcraft/src"]
#[allow(dead_code)]
mod host {
    pub mod animations;
    #[cfg(target_os = "macos")]
    pub mod apple_events;
    pub mod control_server;
    pub mod file_dialogs;
    #[cfg(target_os = "windows")]
    pub mod graphics;
    pub mod logging;
    pub mod print;
}

#[cfg(target_os = "macos")]
use host::apple_events;
#[cfg(target_os = "windows")]
use host::graphics;
use host::{animations, control_server, file_dialogs, logging, print};

use wordcraft_engine::Session;
use wordcraft_ui_egui::paste_picture::ClipboardPicture;
use wordcraft_ui_egui::window_geometry::{WindowGeometry, take_rescue};
use wordcraft_ui_egui::{Services, UiState, WordApp};
use wordcraft_ui_glass::GlassApp;

/// The app, the restored window geometry until the first frame has checked it, the control
/// server's key file (removed on exit), the file dialogs the app asked for and, on macOS, the
/// documents opened from Finder.
struct App(
    GlassApp,
    Option<WindowGeometry>,
    Option<control_server::KeyFile>,
    file_dialogs::Launcher,
    #[cfg(target_os = "macos")] fmv_macos_events::Inbox,
);

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        apple_events::poll(&self.4, &mut self.0.app, ctx);
        self.0.logic(ctx);
        self.3.show(frame);
        let app = &mut self.0.app;
        let prev = app.ui.window;
        app.ui.window = ctx.input(|i| WindowGeometry::track(prev, i.viewport(), i.viewport_rect().size()));
        if let Some(pos) = ctx.input(|i| take_rescue(&mut self.1, i.viewport())) {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }
        // The window's close button: with unsaved changes, ask first.
        let closing = ctx.input(|i| i.viewport().close_requested());
        if closing && !self.0.close_requested() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else if closing || self.0.app.quit_requested {
            #[cfg(target_os = "macos")]
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.0.app.quit_requested = false;
                schedule_macos_terminate(&self.0, self.2.as_ref());
            }
            #[cfg(not(target_os = "macos"))]
            if self.0.app.quit_requested {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.0.raw_input_hook(raw);
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.0.ui(ui);
        self.3.show(frame);
    }
    fn on_exit(&mut self) {
        save_prefs(&self.0);
        if let Some(key_file) = &self.2 {
            key_file.remove();
        }
    }
}

/// Quit the way Command-Q does, on the next turn of the main queue (see apps/wordcraft).
#[cfg(target_os = "macos")]
fn schedule_macos_terminate(app: &GlassApp, key_file: Option<&control_server::KeyFile>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static QUIT: AtomicBool = AtomicBool::new(false);
    if QUIT.swap(true, Ordering::Relaxed) {
        return;
    }
    save_prefs(app);
    if let Some(key_file) = key_file {
        key_file.remove();
    }
    dispatch2::DispatchQueue::main().exec_async(|| {
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            objc2_app_kit::NSApplication::sharedApplication(mtm).terminate(None);
        }
        std::process::exit(0);
    });
}

/// Glass keeps its own preference files, so it never disturbs the classic app's: the editor's
/// (`glass-ui.json`) and Glass's (`glass.json`), in the same settings folder.
fn settings_file(name: &str) -> Option<std::path::PathBuf> {
    wordcraft_control_key::settings_dir().map(|b| b.join(name))
}

fn log_dir() -> Option<std::path::PathBuf> {
    Some(settings_file("glass-ui.json")?.parent()?.join("logs"))
}

/// Runs without preferences (`WORDCRAFT_NO_PREFS`) neither read nor write them.
fn prefs_enabled() -> bool {
    std::env::var_os("WORDCRAFT_NO_PREFS").is_none()
}

fn load_prefs(g: &mut GlassApp) {
    if !prefs_enabled() {
        return;
    }
    if let Some(p) = settings_file("glass-ui.json")
        && let Ok(bytes) = std::fs::read(&p)
        && let Some(ui) = UiState::from_json(&bytes)
    {
        g.app.apply_prefs(ui);
    }
    if let Some(p) = settings_file("glass.json")
        && let Ok(bytes) = std::fs::read(&p)
    {
        g.load_prefs(&bytes);
    }
    if let Some(p) = settings_file("glass-projects.json")
        && let Ok(bytes) = std::fs::read(&p)
    {
        g.load_projects(&bytes);
    }
}

fn load_user_fonts() {
    if !prefs_enabled() {
        return;
    }
    let Some(dir) = wordcraft_control_key::settings_dir().map(|d| d.join("fonts")) else { return };
    let (loaded, skipped) = wordcraft_fonts::FontDb::global().load_user_fonts(&dir);
    if !loaded.is_empty() || !skipped.is_empty() {
        log::info!("user fonts: {} loaded, {} skipped from {}", loaded.len(), skipped.len(), dir.display());
    }
}

fn saved_window() -> Option<WindowGeometry> {
    if !prefs_enabled() {
        return None;
    }
    let bytes = std::fs::read(settings_file("glass-ui.json")?).ok()?;
    UiState::from_json(&bytes)?.window?.sanitized()
}

fn save_prefs(g: &GlassApp) {
    if !prefs_enabled() {
        return;
    }
    let (Some(ui_path), Some(glass_path)) = (settings_file("glass-ui.json"), settings_file("glass.json")) else { return };
    if let Some(dir) = ui_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&g.app.prefs()) {
        let _ = std::fs::write(&ui_path, bytes);
    }
    let _ = std::fs::write(&glass_path, g.prefs_json());
    if let Some(p) = settings_file("glass-projects.json") {
        let _ = std::fs::write(&p, g.projects_json());
    }
}

fn services(file_dialog: file_dialogs::Hook, ctx: &egui::Context) -> Services {
    Services {
        file_dialog: Some(file_dialog),
        print: Some(print::hook()),
        clipboard_picture: Some(Box::new(clipboard_picture)),
        animations_enabled: Some(animations::hook(ctx)),
        clipboard_text: Some(Box::new(|| arboard::Clipboard::new().ok()?.get_text().ok())),
        ..Default::default()
    }
}

fn clipboard_picture() -> Option<ClipboardPicture> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| log::warn!("clipboard: {e}")).ok()?;
    if let Ok(files) = clipboard.get().file_list()
        && !files.is_empty()
    {
        return Some(ClipboardPicture::Files(files));
    }
    let image = clipboard.get_image().ok()?;
    Some(ClipboardPicture::Pixels { width: image.width, height: image.height, rgba: image.bytes.into_owned() })
}

fn app_icon() -> Option<egui::IconData> {
    #[cfg(target_os = "macos")]
    let png: &[u8] = include_bytes!("../../../assets/app-icon/wordcraft-macos-512.png");
    #[cfg(not(target_os = "macos"))]
    let png: &[u8] = include_bytes!("../../../assets/app-icon/hicolor/256x256/apps/ai.storyteller.wordcraft.png");
    eframe::icon_data::from_png_bytes(png).map_err(|e| log::warn!("app icon: {e}")).ok()
}

fn main() -> eframe::Result {
    let logger = logging::install();
    logging::install_panic_hook();
    let mut control_port: Option<u16> = std::env::var("WORDCRAFT_CONTROL_PORT").ok().and_then(|p| p.parse().ok());
    let mut files = Vec::new();
    let mut sample = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--control" => control_port = args.next().and_then(|p| p.parse().ok()),
            "--sample" => sample = true,
            "--version" => {
                println!("wordcraft-glass {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => files.push(a),
        }
    }
    if let Some(logger) = logger {
        match log_dir().filter(|_| prefs_enabled()) {
            Some(dir) => match logger.attach_dir(&dir) {
                Ok(path) => log::info!("WordCraft Glass {}, log file {}", env!("CARGO_PKG_VERSION"), path.display()),
                Err(e) => log::warn!("no log file: {e}"),
            },
            None => logger.no_file(),
        }
    }
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("WordCraft Glass")
            .with_app_id("ai.storyteller.wordcraft.glass")
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([900.0, 560.0])
            .with_drag_and_drop(true)
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false),
        ..Default::default()
    };
    if let Some(icon) = app_icon() {
        options.viewport = options.viewport.with_icon(icon);
    }
    #[cfg(target_os = "windows")]
    graphics::configure(&mut options, eframe::wgpu::Backends::from_env());
    load_user_fonts();
    let restored = saved_window();
    if let Some(window) = restored {
        options.viewport = window.apply(options.viewport);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if std::env::var_os("DISPLAY").is_some() && std::env::var_os("WORDCRAFT_WAYLAND").is_none() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            b.with_x11();
        }));
    }
    #[cfg(target_os = "macos")]
    let apple_events = apple_events::AppleEvents::install();
    #[cfg(target_os = "macos")]
    let apple_events = &apple_events;
    eframe::run_native(
        "WordCraft Glass",
        options,
        Box::new(move |cc| {
            let doc = if sample { wordcraft_engine::sample::sample_document() } else { wordcraft_doc::Document::new() };
            let (file_dialog, dialogs) = file_dialogs::hook(&cc.egui_ctx);
            let mut app = WordApp::new(Session::new(doc), services(file_dialog, &cc.egui_ctx));
            app.watch_desktop_theme(&cc.egui_ctx);
            app.integrated_titlebar = cfg!(target_os = "macos");
            let mut key_file = None;
            if let Some(port) = control_port
                && let Some((rx, file)) = control_server::start(port, cc.egui_ctx.clone(), wordcraft_control_key::settings_dir().as_deref())
            {
                app = app.with_control(rx);
                key_file = file;
            }
            let mut glass = GlassApp::new(app);
            load_prefs(&mut glass);
            glass.app.ui.window = restored;
            // Each file opens in its own tab.
            for f in files {
                glass.open_path(&f);
            }
            Ok(Box::new(App(
                glass,
                restored,
                key_file,
                dialogs,
                #[cfg(target_os = "macos")]
                apple_events.connect(&cc.egui_ctx),
            )))
        }),
    )
}

//! Live page thumbnails, rendered by the engine's own renderer (the same one that draws the
//! page), for the Navigation pane's Pages and the document tabs' hover previews.
//!
//! Thumbnails are cached per document and page and refreshed as the document changes, at most
//! every second and two per frame, so typing never waits on them.

use std::collections::HashMap;

use egui::TextureHandle;
use wordcraft_engine::Session;

use crate::GlassApp;

struct Entry {
    rev: u64,
    at: f64,
    width: u32,
    tex: TextureHandle,
}

#[derive(Default)]
pub struct Thumbs {
    map: HashMap<(u64, usize), Entry>,
    /// Renders left this frame.
    pub budget: u32,
}

/// Renders allowed per frame.
pub const PER_FRAME: u32 = 2;
/// A changed document's thumbnails refresh at most this often (seconds).
const REFRESH_SECS: f64 = 1.0;

impl Thumbs {
    /// Page `page` of `s`, `width` pixels wide, if it is ready (else it is rendered when the
    /// frame's budget allows, and the caller should ask again next frame).
    pub fn get(&mut self, ctx: &egui::Context, s: &mut Session, page: usize, width: u32) -> Option<TextureHandle> {
        let key = (s.document_id(), page);
        let rev = s.rev();
        let now = wordcraft_ui_egui::now_ms() / 1000.0;
        let width = width.clamp(32, 1200);
        if let Some(e) = self.map.get(&key) {
            let fresh = e.rev == rev && e.width >= width;
            if fresh || now - e.at < REFRESH_SECS || self.budget == 0 {
                if !fresh {
                    ctx.request_repaint_after(std::time::Duration::from_millis(300));
                }
                return Some(e.tex.clone());
            }
        }
        if self.budget == 0 {
            ctx.request_repaint();
            return None;
        }
        self.budget -= 1;
        let layout = s.layout();
        let pg = layout.pages.get(page)?;
        let scale = width as f32 / pg.w.max(1.0);
        let img = wordcraft_engine::render::render_page(&s.doc, pg, scale, &wordcraft_engine::render::RenderOptions::default());
        let px = img.to_straight();
        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &px);
        let tex = match self.map.get_mut(&key) {
            Some(e) => {
                e.tex.set(ci, egui::TextureOptions::LINEAR);
                e.rev = rev;
                e.at = now;
                e.width = width;
                e.tex.clone()
            }
            None => {
                let tex = ctx.load_texture(format!("glass_thumb_{}_{page}", key.0), ci, egui::TextureOptions::LINEAR);
                self.map.insert(key, Entry { rev, at: now, width, tex: tex.clone() });
                tex
            }
        };
        // Forget documents no longer open now and then.
        if self.map.len() > 400 {
            self.map.retain(|(d, _), _| *d == key.0);
        }
        Some(tex)
    }
}

impl GlassApp {
    /// A thumbnail of page `page` of the document in tab `tab` (`None` = the open document).
    pub fn thumbnail(&mut self, ctx: &egui::Context, tab: Option<usize>, page: usize, width: u32) -> Option<TextureHandle> {
        let active = self.docs.active;
        let GlassApp { app, glass, docs, .. } = self;
        let session = match tab {
            Some(i) if i != active => docs.stash_mut(i)?,
            _ => &mut app.session,
        };
        glass.thumbs.get(ctx, session, page, width)
    }
}

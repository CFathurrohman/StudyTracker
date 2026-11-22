//! Development screenshots: STUDYTRACKER_SHOT=<file.png> renders the main window from the app's own frames,
//! then quits. With STUDYTRACKER_SHOT_FLOAT=1 the floating widget is drawn inside the main window instead,
//! so it can be captured the same way. Works even when other windows cover the app.

use eframe::egui::{self, ColorImage, ViewportCommand, ViewportId};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub struct Shot {
    out: PathBuf,
    pub float: bool,
    started: Instant,
    requested: bool,
    done: Arc<AtomicBool>,
}

impl Shot {
    pub fn from_env() -> Option<Self> {
        let out = PathBuf::from(std::env::var_os("STUDYTRACKER_SHOT")?);
        let float = std::env::var_os("STUDYTRACKER_SHOT_FLOAT").is_some();
        Some(Self { out, float, started: Instant::now(), requested: false, done: Arc::new(AtomicBool::new(false)) })
    }

    /// Call every frame of the root viewport.
    pub fn step(&mut self, ctx: &egui::Context) {
        ctx.request_repaint();
        if !self.requested && self.started.elapsed().as_millis() > 1800 {
            self.requested = true;
            let (done, path) = (self.done.clone(), self.out.clone());
            ctx.send_viewport_cmd_to(
                ViewportId::ROOT,
                ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                    let _ = save_png(&path, &img);
                    done.store(true, Ordering::SeqCst);
                })),
            );
        }
        if self.done.load(Ordering::SeqCst) || self.started.elapsed().as_secs() > 12 {
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        }
    }
}

fn save_png(path: &PathBuf, img: &ColorImage) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.size[0] as u32, img.size[1] as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(std::io::Error::other)?;
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    w.write_image_data(&bytes).map_err(std::io::Error::other)?;
    Ok(())
}

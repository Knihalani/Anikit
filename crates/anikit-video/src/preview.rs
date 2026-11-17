//! In-process previews for agents (the `preview` command and the MCP
//! `render_still` / `render_contact_sheet` tools): the real pipeline's
//! `render_rgba`, plus a contact sheet that shows each scene at its settled
//! moment (70% through), which is what a reviewer needs to see.

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use crate::eval::{Evaluator, Options};
use crate::pipeline;

/// A rendered frame: straight-alpha RGBA8.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A compiled spec ready to render frames.
pub struct Preview {
    pub ev: Evaluator,
}

impl Preview {
    /// Compile a spec value; asset paths resolve against `base_dir`.
    pub fn new(spec: Value, base_dir: &Path) -> Result<Preview, String> {
        // An empty base (a template with no params file) means the working directory.
        let base_dir = if base_dir.as_os_str().is_empty() { Path::new(".") } else { base_dir };
        let parsed = crate::spec::parse::from_value(spec).map_err(|d| d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n"))?;
        let fonts = crate::text::FontStore::new();
        let opts = Options { base_dir: base_dir.to_path_buf(), fonts: Some(Arc::new(fonts)), load_assets: true, assets: None };
        let ev = Evaluator::new(&parsed.spec, opts).map_err(|e| e.to_string())?;
        Ok(Preview { ev })
    }

    /// One settled frame per scene (70% through it).
    pub fn scene_times(&self) -> Vec<f32> {
        self.ev.scenes().iter().map(|s| s.start + (s.end - s.start) * 0.7).collect()
    }

    /// Render the frame at `t`, scaled down to at most `max_width` wide.
    pub fn frame(&self, t: f32, max_width: Option<u32>) -> Image {
        let scale = max_width.map_or(1.0, |m| (m as f32 / self.ev.width as f32).min(1.0));
        let (width, height, rgba) = pipeline::render_rgba(&self.ev, t.clamp(0.0, (self.ev.duration - 1e-3).max(0.0)), scale);
        Image { width, height, rgba }
    }

    /// A grid of frames (default: one per scene), each `cell_width` wide.
    pub fn sheet(&self, times: Option<Vec<f32>>, cell_width: u32) -> Image {
        let times = times.unwrap_or_else(|| self.scene_times());
        let frames: Vec<Image> = times.iter().map(|&t| self.frame(t, Some(cell_width))).collect();
        let cols = (frames.len() as f32).sqrt().ceil().max(1.0) as usize;
        let cols = cols.max(if self.ev.width >= self.ev.height { 2 } else { 3 }).min(frames.len().max(1));
        let rows = frames.len().div_ceil(cols).max(1);
        let (cw, ch) = frames.first().map(|f| (f.width as usize, f.height as usize)).unwrap_or((1, 1));
        let gap = 8;
        let (sw, sh) = (cols * cw + (cols + 1) * gap, rows * ch + (rows + 1) * gap);
        let mut rgba = vec![0u8; sw * sh * 4];
        for p in rgba.chunks_mut(4) {
            p.copy_from_slice(&[24, 24, 28, 255]);
        }
        for (k, f) in frames.iter().enumerate() {
            let (ox, oy) = (gap + (k % cols) * (cw + gap), gap + (k / cols) * (ch + gap));
            let w = (f.width as usize).min(cw);
            for y in 0..(f.height as usize).min(ch) {
                let s = y * f.width as usize * 4;
                let d = ((oy + y) * sw + ox) * 4;
                rgba[d..d + w * 4].copy_from_slice(&f.rgba[s..s + w * 4]);
            }
        }
        Image { width: sw as u32, height: sh as u32, rgba }
    }
}

/// Encode as PNG.
pub fn png(img: &Image) -> Vec<u8> {
    use image::ImageEncoder;
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(&img.rgba, img.width, img.height, image::ExtendedColorType::Rgba8)
        .expect("png encode");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_text_still_and_sheet() {
        let spec = json!({ "size": [320, 180], "scenes": [
            { "duration": 1, "layers": [ { "text": "Hi", "size": 60, "color": "#ffffff" } ] },
            { "duration": 1, "background": "#ff0000", "layers": [] }
        ] });
        let p = Preview::new(spec, Path::new(".")).unwrap();
        let f = p.frame(0.5, None);
        assert_eq!((f.width, f.height), (320, 180));
        // Some bright pixels from the glyphs near the centre.
        let bright = f.rgba.chunks(4).filter(|px| px[0] > 200 && px[1] > 200).count();
        assert!(bright > 50, "text did not render ({bright} bright px)");
        let s = p.sheet(None, 160);
        assert!(s.width > 160 && png(&s).starts_with(b"\x89PNG"));
    }
}

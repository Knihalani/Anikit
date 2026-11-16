//! In-process previews (stills and contact sheets) for agents: spec → eval
//! (with the real text engine) → render → PNG. Used by `anikit-video preview`
//! and as the MCP fallback while the pipeline's `still`/`sheet` commands are
//! unavailable. Not the production pipeline: single-threaded and RGBA only.

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use crate::dl::{FontId, ImageId};
use crate::eval::text::{PGlyph, TextBlock, TextEngine, TextReq};
use crate::eval::{Evaluator, Options};
use crate::render::{Assets, Renderer};
use crate::text::layout::{Align, TextSpec, TextStyle};
use crate::text::FontStore;

/// Adapts the text system to the evaluator's [`TextEngine`].
pub struct Text(pub Arc<FontStore>);

impl TextEngine for Text {
    fn layout(&self, r: &TextReq) -> TextBlock {
        let style = TextStyle {
            font: r.family.to_string(),
            weight: r.weight,
            italic: r.italic,
            size: r.size,
            // TextReq is in px, TextStyle in em.
            letter_spacing: if r.size > 0.0 { r.letter_spacing / r.size } else { 0.0 },
            line_height: r.line_height,
            ..TextStyle::default()
        };
        let spec = TextSpec {
            text: r.text.to_string(),
            style,
            width: r.max_width,
            align: match r.align {
                0 => Align::Left,
                2 => Align::Right,
                _ => Align::Center,
            },
            max_lines: r.max_lines,
            ..TextSpec::default()
        };
        let l = self.0.layout(&spec);
        let glyphs = l
            .glyphs
            .iter()
            .map(|g| {
                let run = &l.runs[g.run as usize];
                let gl = run.glyphs[g.index as usize];
                PGlyph { font: run.font, id: gl.id, x: gl.x, y: gl.y, advance: self.0.advance(run.font, gl.id, run.size), char_index: g.char, word: g.word, line: g.line }
            })
            .collect();
        // Aligned text is positioned inside its wrap width, so that is the block's width.
        TextBlock { size: [r.max_width.unwrap_or(l.width).max(l.width), l.height], glyphs, chars: l.chars.len() as u32, words: l.words.len() as u32, lines: l.lines.len() as u32, font_size: l.font_size }
    }
}

/// A rendered frame: straight-alpha RGBA8.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A compiled spec ready to render frames.
pub struct Preview {
    pub ev: Evaluator,
    fonts: Arc<FontStore>,
}

impl Preview {
    /// Compile a spec value; asset paths resolve against `base_dir`.
    pub fn new(spec: Value, base_dir: &Path) -> Result<Preview, String> {
        let parsed = crate::spec::parse::from_value(spec).map_err(|d| d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n"))?;
        let fonts = Arc::new(FontStore::new());
        let opts = Options { base_dir: base_dir.to_path_buf(), text: Some(Arc::new(Text(fonts.clone()))), load_assets: true, ..Options::default() };
        let ev = Evaluator::new(&parsed.spec, opts).map_err(|e| e.to_string())?;
        Ok(Preview { ev, fonts })
    }

    /// One settled frame per scene (70% through it).
    pub fn scene_times(&self) -> Vec<f32> {
        self.ev.scenes().iter().map(|s| s.start + (s.end - s.start) * 0.7).collect()
    }

    /// Render the frame at `t`, downscaled by an integer factor so it is at most `max_width` wide.
    pub fn frame(&self, t: f32, max_width: Option<u32>) -> Image {
        let list = self.ev.eval(t.clamp(0.0, (self.ev.duration - 1e-3).max(0.0)));
        // Fonts are interned during layout, so build the asset table after eval.
        let mut assets = Assets::default();
        for (i, img) in self.ev.images.iter().enumerate() {
            let _ = assets.images.insert_rgba8(ImageId(i as u32), img.width, img.height, img.rgba.to_vec());
        }
        for i in 0..self.fonts.face_count() as u32 {
            assets.fonts.insert(FontId(i), self.fonts.face(FontId(i)).data.clone());
        }
        let mut r = Renderer::new(Arc::new(assets));
        let px = r.render(&list);
        let (w, h) = (self.ev.width, self.ev.height);
        let mut rgba = px.data_as_u8_slice().to_vec();
        for p in rgba.chunks_mut(4) {
            let a = p[3] as u32;
            if a > 0 && a < 255 {
                for c in &mut p[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        let img = Image { width: w, height: h, rgba };
        match max_width {
            Some(mw) if mw < w => downscale(&img, w.div_ceil(mw)),
            _ => img,
        }
    }

    /// A grid of frames (default: one per scene), each downscaled to `cell_width`.
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
            for y in 0..f.height as usize {
                let s = y * f.width as usize * 4;
                let d = ((oy + y) * sw + ox) * 4;
                rgba[d..d + f.width as usize * 4].copy_from_slice(&f.rgba[s..s + f.width as usize * 4]);
            }
        }
        Image { width: sw as u32, height: sh as u32, rgba }
    }
}

/// Box-filter downscale by an integer factor.
fn downscale(img: &Image, k: u32) -> Image {
    let (w, h) = (img.width / k, img.height / k);
    let mut out = vec![0u8; (w * h * 4) as usize];
    let n = k * k;
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            for dy in 0..k {
                for dx in 0..k {
                    let s = (((y * k + dy) * img.width + x * k + dx) * 4) as usize;
                    for (c, a) in acc.iter_mut().enumerate() {
                        *a += img.rgba[s + c] as u32;
                    }
                }
            }
            let d = ((y * w + x) * 4) as usize;
            for c in 0..4 {
                out[d + c] = (acc[c] / n) as u8;
            }
        }
    }
    Image { width: w, height: h, rgba: out }
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

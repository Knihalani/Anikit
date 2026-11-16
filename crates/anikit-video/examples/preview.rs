//! Template QA preview: expand a template (or load a spec) and write PNG
//! stills plus a contact sheet, with real text. Stand-in until the CLI's
//! `still`/`sheet` land.
//!
//! `cargo run --release -p anikit-video --example preview -- <template|spec.json> [16:9|9:16|1:1] [out_dir] [--brand brand.json] [--times 0.5,1.2]`

use std::sync::Arc;

use anikit_video::eval::text::{PGlyph, TextBlock, TextEngine, TextReq};
use anikit_video::eval::{Evaluator, Options};
use anikit_video::render::{Assets, Renderer};
use anikit_video::templates::{brand::Brand, Format, Template};
use anikit_video::text::layout::{Align, TextSpec, TextStyle};
use anikit_video::text::FontStore;

struct Engine(Arc<FontStore>);

impl TextEngine for Engine {
    fn layout(&self, r: &TextReq) -> TextBlock {
        let style = TextStyle { font: r.family.to_string(), weight: r.weight, italic: r.italic, size: r.size, letter_spacing: if r.size > 0.0 { r.letter_spacing / r.size } else { 0.0 }, line_height: r.line_height, ..TextStyle::default() };
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
        TextBlock { size: [r.max_width.unwrap_or(l.width).max(l.width), l.height], glyphs, chars: l.chars.len() as u32, words: l.words.len() as u32, lines: l.lines.len() as u32, font_size: l.font_size }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let flag = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let pos: Vec<&String> = a.iter().enumerate().filter(|(i, x)| !x.starts_with("--") && !(*i > 0 && a[i - 1].starts_with("--"))).map(|(_, x)| x).collect();
    let src = pos.first().expect("template name or spec.json");
    let fmt = pos.get(1).map(|s| s.as_str()).unwrap_or("16:9");
    let out_dir = std::path::PathBuf::from(pos.get(2).map(|s| s.as_str()).unwrap_or("target/preview"));
    let brand = flag("--brand").map(|p| Brand::from_json(&std::fs::read_to_string(p).unwrap()).unwrap()).unwrap_or_default();
    let (spec, base) = if src.ends_with(".json") {
        (serde_json::from_str(&std::fs::read_to_string(src.as_str()).unwrap()).unwrap(), std::path::Path::new(src.as_str()).parent().unwrap().to_path_buf())
    } else {
        let t = Template::builtin(src).unwrap();
        let params = flag("--params").map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).unwrap_or_else(|| t.example_params());
        (t.expand(&params, &brand, Format::parse(fmt).unwrap()).unwrap(), std::path::PathBuf::from("examples/video/assets"))
    };
    let parsed = anikit_video::spec::parse::from_value(spec).unwrap_or_else(|d| panic!("{d:?}"));
    let fonts = Arc::new(FontStore::new());
    let opts = Options { base_dir: base, text: Some(Arc::new(Engine(fonts.clone()))), load_assets: true, ..Options::default() };
    let (ev, diags) = Evaluator::build(&parsed.spec, &opts);
    for d in &diags {
        eprintln!("{d}");
    }
    let ev = ev.expect("spec did not compile");
    // Times: each scene's settled moment (70% through), plus any requested.
    let mut times: Vec<f32> = ev.scenes().iter().map(|s| s.start + (s.end - s.start) * 0.7).collect();
    if let Some(t) = flag("--times") {
        times = t.split(',').map(|x| x.parse().unwrap()).collect();
    }
    let list0 = ev.eval(0.0);
    let mut assets = Assets::default();
    for (i, img) in ev.images.iter().enumerate() {
        let _ = assets.images.insert_rgba8(anikit_video::dl::ImageId(i as u32), img.width, img.height, img.rgba.to_vec());
    }
    for i in 0..fonts.face_count() as u32 {
        let face = fonts.face(anikit_video::dl::FontId(i));
        assets.fonts.insert(anikit_video::dl::FontId(i), face.data.clone());
    }
    let _ = list0;
    drop(assets);
    let mut r;
    let (w, h) = (ev.width, ev.height);
    let name = std::path::Path::new(src.as_str()).file_stem().unwrap().to_string_lossy().replace('.', "_");
    let tag = fmt.replace(':', "x");
    let cols = 3usize.min(times.len()).max(1);
    let rows = times.len().div_ceil(cols);
    let (tw, th) = (w as usize / 3, h as usize / 3);
    let mut sheet = vec![0u8; cols * tw * rows * th * 4];
    std::fs::create_dir_all(&out_dir).unwrap();
    for (k, &t) in times.iter().enumerate() {
        // Fonts are interned lazily during layout; refresh the renderer's copy.
        let list = ev.eval(t);
        let mut assets = Assets::default();
        for (i, img) in ev.images.iter().enumerate() {
            let _ = assets.images.insert_rgba8(anikit_video::dl::ImageId(i as u32), img.width, img.height, img.rgba.to_vec());
        }
        for i in 0..fonts.face_count() as u32 {
            assets.fonts.insert(anikit_video::dl::FontId(i), fonts.face(anikit_video::dl::FontId(i)).data.clone());
        }
        r = Renderer::new(Arc::new(assets));
        let px = r.render(&list);
        let data: Vec<u8> = px.data_as_u8_slice().to_vec();
        // Un-premultiply.
        let mut rgba = data.clone();
        for p in rgba.chunks_mut(4) {
            let al = p[3] as f32 / 255.0;
            if al > 0.0 {
                for c in &mut p[..3] {
                    *c = ((*c as f32 / al).min(255.0)) as u8;
                }
            }
        }
        let path = out_dir.join(format!("{name}-{tag}-{k}.png"));
        save_png(&path, w, h, &rgba);
        let (cx, cy) = (k % cols, k / cols);
        for y in 0..th {
            for x in 0..tw {
                let s = ((y * 3) * w as usize + x * 3) * 4;
                let d = ((cy * th + y) * cols * tw + cx * tw + x) * 4;
                sheet[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
            }
        }
    }
    let sp = out_dir.join(format!("{name}-{tag}-sheet.png"));
    save_png(&sp, (cols * tw) as u32, (rows * th) as u32, &sheet);
    println!("{} ({} frames, {:.1} s)", sp.display(), times.len(), ev.duration);
}

fn save_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) {
    let f = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

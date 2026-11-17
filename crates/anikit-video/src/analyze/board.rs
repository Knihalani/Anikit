//! `anikit-video board`: a mood board from reference images (paintings,
//! posters, photographs, packaging, architecture… ideally from outside tech).
//! Measures each image's palette, value structure, texture, composition and
//! line character, then merges them into theme tokens, a `post` finish and a
//! look summary that an agent can apply to a spec.

use super::image_ops::*;
use super::sheet::Canvas;
use super::{grade_of, layout_of, r2, r3, Grade, Layout, Swatch};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Longest side images are reduced to before measuring.
const MAX_SIDE: u32 = 640;

#[derive(Debug, Clone, Serialize)]
pub struct Board {
    pub images: Vec<ImageLook>,
    /// Palette over all images (each image weighted equally), by share.
    pub palette: Vec<Swatch>,
    /// The chromatic colours (OKLab chroma > 0.06), clustered on their own so
    /// small but vivid accents (a neon line, city lights, a red stamp) aren't
    /// swamped by large neutral areas. `share` is their share of chromatic pixels.
    pub accents: Vec<Swatch>,
    /// Theme tokens for a spec: `bg`, `surface`, `text`, `muted`, `primary`, `accent`.
    pub tokens: Value,
    /// A `{ "theme": …, "post": … }` patch to merge into a spec.
    pub spec_patch: Value,
    /// One paragraph: palette, value, texture, composition and line character.
    pub summary: String,
    pub notes: Vec<&'static str>,
    #[serde(skip)]
    thumbs: Vec<(u32, u32, Vec<u8>)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageLook {
    pub file: String,
    pub width: u32,
    pub height: u32,
    pub palette: Vec<Swatch>,
    /// Vivid colours only (see `Board::accents`), and their share of the whole image.
    pub accents: Vec<Swatch>,
    pub chromatic_share: f32,
    /// Mean OKLab lightness, its std (contrast) and `low-key` / `mid-key` / `high-key`.
    pub luma: f32,
    pub contrast: f32,
    pub key: String,
    pub saturation: f32,
    pub grade: Grade,
    /// High-pass residual in the flattest tiles: 0.004 clean … 0.03 heavy grain.
    pub grain: f32,
    /// Mean |Laplacian|: how much fine detail there is (≈ 0.01 smooth … 0.08 busy).
    pub detail: f32,
    /// Hard edges as a share of all edges: ≥ 0.08 graphic/flat, ≤ 0.05 painterly/photographic.
    pub hard_edges: f32,
    pub layout: Layout,
    /// Share of 16 px tiles that are quiet (no texture, no edges): breathing room.
    pub negative_space: f32,
    /// Edge orientation shares: horizontal, vertical, diagonal.
    pub lines: Lines,
    pub vignette: f32,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Lines {
    pub horizontal: f32,
    pub vertical: f32,
    pub diagonal: f32,
    /// 0 = one direction dominates … 1 = every direction equally (organic).
    pub spread: f32,
    /// e.g. `rectilinear (verticals)`, `diagonal / dynamic`, `organic, curving`.
    pub character: String,
}

fn load(path: &Path) -> Result<(u32, u32, u32, u32, Vec<u8>), String> {
    let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (w0, h0) = (img.width(), img.height());
    let k = (MAX_SIDE as f32 / w0.max(h0) as f32).min(1.0);
    let (w, h) = (((w0 as f32 * k).round() as u32).max(8), ((h0 as f32 * k).round() as u32).max(8));
    let rgb = img.resize_exact(w, h, image::imageops::FilterType::Triangle).to_rgb8();
    Ok((w0, h0, w, h, rgb.into_raw()))
}

/// Image files under `inputs` (files, or folders of png/jpg/jpeg/webp), sorted.
pub fn collect(inputs: &[String]) -> Result<Vec<PathBuf>, String> {
    let ok = |p: &Path| p.extension().map(|e| e.to_string_lossy().to_lowercase()).is_some_and(|e| ["png", "jpg", "jpeg", "webp"].contains(&e.as_str()));
    let mut out = Vec::new();
    for i in inputs {
        let p = PathBuf::from(i);
        if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(&p).map_err(|e| format!("{i}: {e}"))?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| ok(p)).collect();
            v.sort();
            out.extend(v);
        } else if p.exists() {
            out.push(p);
        } else {
            return Err(format!("{i}: no such file"));
        }
    }
    if out.is_empty() {
        return Err("no images (png, jpg, webp) found".into());
    }
    Ok(out)
}

fn lines(l: &[f32], w: usize, h: usize) -> Lines {
    // Orientation of strong gradients, in 4 bins: the edge runs perpendicular to the gradient.
    let mut bins = [0f32; 4]; // edge horizontal, diag /, vertical, diag \
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let gx = l[i + 1] - l[i - 1];
            let gy = l[i + w] - l[i - w];
            let m = gx.hypot(gy);
            if m < 0.08 {
                continue;
            }
            let a = gy.atan2(gx).rem_euclid(std::f32::consts::PI); // gradient direction 0..π
            let b = ((a / std::f32::consts::FRAC_PI_4) + 0.5).floor() as usize % 4;
            // gradient vertical (b == 2) → edge horizontal.
            let edge = [2, 3, 0, 1][b];
            bins[edge] += m;
        }
    }
    let t: f32 = bins.iter().sum::<f32>().max(1e-6);
    let p: Vec<f32> = bins.iter().map(|b| b / t).collect();
    let (hz, vt, dg) = (p[0], p[2], p[1] + p[3]);
    let ent = -p.iter().filter(|v| **v > 0.0).map(|v| v * v.ln()).sum::<f32>() / 4f32.ln();
    let character = if t < 1.0 {
        "soft, almost edgeless".to_string()
    } else if hz + vt > 0.72 {
        format!("rectilinear ({})", if hz > vt * 1.4 { "horizontals" } else if vt > hz * 1.4 { "verticals" } else { "grid" })
    } else if dg > 0.5 {
        "diagonal / dynamic".into()
    } else if ent > 0.95 {
        "organic, curving".into()
    } else {
        "mixed".into()
    };
    Lines { horizontal: r2(hz), vertical: r2(vt), diagonal: r2(dg), spread: r2(ent), character }
}

fn negative_space(l: &[f32], w: usize, h: usize) -> f32 {
    let t = 16;
    let (mut quiet, mut n) = (0, 0);
    for ty in 0..h / t {
        for tx in 0..w / t {
            let (mut s, mut s2, mut c) = (0.0f32, 0.0f32, 0.0f32);
            for y in ty * t..ty * t + t {
                for x in tx * t..tx * t + t {
                    let v = l[y * w + x];
                    s += v;
                    s2 += v * v;
                    c += 1.0;
                }
            }
            let sd = ((s2 / c) - (s / c).powi(2)).max(0.0).sqrt();
            quiet += (sd < 0.025) as usize;
            n += 1;
        }
    }
    if n == 0 { 0.0 } else { quiet as f32 / n as f32 }
}

fn look(file: &Path, w0: u32, h0: u32, w: usize, h: usize, rgb: &[u8]) -> (ImageLook, Vec<[f32; 3]>) {
    let l = luma_plane(rgb, w, h);
    let mut lab = Vec::with_capacity(w * h / 4);
    for i in (0..w * h).step_by(4) {
        lab.push(srgb_to_oklab([rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]]));
    }
    let n = lab.len().max(1) as f32;
    let luma = lab.iter().map(|c| c[0]).sum::<f32>() / n;
    let contrast = (lab.iter().map(|c| (c[0] - luma).powi(2)).sum::<f32>() / n).sqrt();
    let saturation = lab.iter().map(|c| c[1].hypot(c[2])).sum::<f32>() / n;
    let (_, hard) = flatness(&l, w, h);
    let key = if luma < 0.38 { "low-key" } else if luma > 0.72 { "high-key" } else { "mid-key" };
    let img = ImageLook {
        file: file.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(),
        width: w0,
        height: h0,
        palette: kmeans_palette(&lab, 6),
        accents: kmeans_palette(&vivid(&lab), 4),
        chromatic_share: r3(vivid(&lab).len() as f32 / n),
        luma: r2(luma),
        contrast: r2(contrast),
        key: key.into(),
        saturation: r3(saturation),
        grade: grade_of(&lab, saturation),
        grain: r3(grain(&l, w, h)),
        detail: r3(mean_abs_laplacian(&l, w, h)),
        hard_edges: r2(hard),
        layout: layout_of(&l, w, h),
        negative_space: r2(negative_space(&l, w, h)),
        lines: lines(&l, w, h),
        vignette: r2(vignette(&l, w, h)),
    };
    (img, lab)
}

fn vivid(lab: &[[f32; 3]]) -> Vec<[f32; 3]> {
    lab.iter().filter(|c| c[1].hypot(c[2]) > 0.06 && c[0] > 0.25).copied().collect()
}

fn rel_lum(h: &str) -> f32 {
    let c = hex_to_rgb(h);
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

fn contrast_ratio(a: &str, b: &str) -> f32 {
    let (x, y) = (rel_lum(a), rel_lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Tokens from the merged palette: the ground follows the board's key; text
/// is the board colour with the most contrast against it (≥ 4.5:1, else
/// near-black/white); primary and accent are the most chromatic colours with
/// a real share.
fn tokens(p: &[Swatch], acc: &[Swatch], mean_l: f32) -> Value {
    let dark = mean_l < 0.55;
    let big: Vec<&Swatch> = p.iter().filter(|s| s.share >= 0.05).collect();
    let pick = |v: &[&Swatch], f: &dyn Fn(&Swatch) -> f32| v.iter().copied().max_by(|a, b| f(a).total_cmp(&f(b))).map(|s| s.hex.clone());
    let bg = pick(&big, &|s| if dark { -s.l + s.share * 0.3 } else { s.l + s.share * 0.3 }).unwrap_or_else(|| if dark { "#101010".into() } else { "#F2F0EA".into() });
    let all: Vec<&Swatch> = p.iter().collect();
    let mut text = pick(&all, &|s| contrast_ratio(&s.hex, &bg)).unwrap_or_default();
    if contrast_ratio(&text, &bg) < 4.5 {
        text = if dark { "#F4F2EC".into() } else { "#121212".into() };
    }
    // Accents come from the vivid clusters when there are any, else from the main palette.
    let src: Vec<&Swatch> = if acc.is_empty() { p.iter().filter(|s| s.c > 0.035 && s.share >= 0.02).collect() } else { acc.iter().filter(|s| s.share >= 0.04).collect() };
    let mut chroma: Vec<&Swatch> = src.into_iter().filter(|s| s.hex != bg).collect();
    // Vivid, present, and visible on the ground (contrast up to 4.5:1 counts).
    let score = |s: &Swatch| s.c * s.share.sqrt() * (contrast_ratio(&s.hex, &bg).min(4.5) / 4.5);
    chroma.sort_by(|a, b| score(b).total_cmp(&score(a)));
    let primary = chroma.first().map(|s| s.hex.clone()).unwrap_or_else(|| text.clone());
    let accent = chroma
        .iter()
        .find(|s| {
            let (a, b) = (hex_to_oklab(&s.hex), hex_to_oklab(&primary));
            ((a[1] - b[1]).hypot(a[2] - b[2])) > 0.06
        })
        .map(|s| s.hex.clone())
        .unwrap_or_else(|| primary.clone());
    // Surface: the ground nudged toward the text; muted: halfway between.
    let mix = |a: &str, b: &str, t: f32| {
        let (x, y) = (hex_to_oklab(a), hex_to_oklab(b));
        hex(oklab_to_srgb([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t]))
    };
    json!({ "bg": bg, "surface": mix(&bg, &text, 0.08), "text": text, "muted": mix(&bg, &text, 0.55), "primary": primary, "accent": accent })
}

/// One image's measures, OKLab samples and thumbnail (w, h, rgb).
type Measured = (ImageLook, Vec<[f32; 3]>, (u32, u32, Vec<u8>));

/// Measure every image and merge them into a board.
pub fn board(files: &[PathBuf]) -> Result<Board, String> {
    use rayon::prelude::*;
    let res: Vec<Result<Measured, String>> = files
        .par_iter()
        .map(|f| {
            let (w0, h0, w, h, rgb) = load(f)?;
            let (il, lab) = look(f, w0, h0, w as usize, h as usize, &rgb);
            Ok((il, lab, (w, h, rgb)))
        })
        .collect();
    let mut images = Vec::new();
    let mut thumbs = Vec::new();
    let mut samples = Vec::new();
    let mut vivid_all = Vec::new();
    for r in res {
        let (il, lab, th) = r?;
        // Equal weight per image: 6000 samples each (vivid: 2000 each).
        let step = (lab.len() / 6000).max(1);
        samples.extend(lab.iter().step_by(step).copied());
        let v = vivid(&lab);
        let vs = (v.len() / 2000).max(1);
        vivid_all.extend(v.iter().step_by(vs).copied());
        images.push(il);
        thumbs.push(th);
    }
    let palette = kmeans_palette(&samples, 8);
    let accents = kmeans_palette(&vivid_all, 5);
    let n = images.len() as f32;
    let avg = |f: &dyn Fn(&ImageLook) -> f32| images.iter().map(f).sum::<f32>() / n;
    let mean_l = avg(&|i| i.luma);
    let tok = tokens(&palette, &accents, mean_l);
    let (contrast, sat, grain_v, detail, hard, neg, vig) =
        (avg(&|i| i.contrast), avg(&|i| i.saturation), avg(&|i| i.grain), avg(&|i| i.detail), avg(&|i| i.hard_edges), avg(&|i| i.negative_space), avg(&|i| i.vignette));
    let warmth = avg(&|i| i.grade.warmth);
    let black = avg(&|i| i.grade.black);
    let white = avg(&|i| i.grade.white);
    let mut post = serde_json::Map::new();
    if grain_v > 0.008 {
        post.insert("grain".into(), json!((((grain_v as f64 - 0.004) * 2.0).clamp(0.01, 0.06) * 100.0).round() / 100.0));
    }
    if vig > 0.25 {
        post.insert("vignette".into(), json!(((vig as f64).min(0.7) * 100.0).round() / 100.0));
    }
    if black > 0.15 {
        post.insert("lift".into(), json!(((black as f64 * 0.5).min(0.15) * 100.0).round() / 100.0));
    }
    // Lines and composition in words.
    let mut chars: Vec<(String, usize)> = Vec::new();
    for i in &images {
        match chars.iter_mut().find(|c| c.0 == i.lines.character) {
            Some(c) => c.1 += 1,
            None => chars.push((i.lines.character.clone(), 1)),
        }
    }
    chars.sort_by_key(|c| std::cmp::Reverse(c.1));
    let centred = images.iter().filter(|i| i.layout.label == "centred").count();
    let summary = format!(
        "{} image{}. Palette: {}; vivid accents {} (ground {}, text {}, primary {}, accent {}). Value: {} on average (mean L {:.2}), {} contrast (black {:.2}, white {:.2}), {}, {}. Texture: {}, {} detail, {} edges. Composition: {}/{} centred, {:.0}% negative space on average. Line character: {}.",
        images.len(),
        if images.len() == 1 { "" } else { "s" },
        palette.iter().take(5).map(|s| format!("{} {:.0}%", s.hex, s.share * 100.0)).collect::<Vec<_>>().join(", "),
        if accents.is_empty() { "none".to_string() } else { accents.iter().take(4).map(|s| s.hex.clone()).collect::<Vec<_>>().join(", ") },
        tok["bg"].as_str().unwrap_or(""),
        tok["text"].as_str().unwrap_or(""),
        tok["primary"].as_str().unwrap_or(""),
        tok["accent"].as_str().unwrap_or(""),
        if mean_l < 0.38 { "low-key" } else if mean_l > 0.72 { "high-key" } else { "mid-key" },
        mean_l,
        if contrast < 0.12 { "low" } else if contrast > 0.24 { "high" } else { "medium" },
        black,
        white,
        if warmth > 0.015 { "warm" } else if warmth < -0.015 { "cool" } else { "neutral" },
        if sat < 0.03 { "near monochrome" } else if sat < 0.07 { "muted colour" } else if sat > 0.13 { "saturated colour" } else { "natural colour" },
        if grain_v > 0.012 { "visible grain" } else if grain_v > 0.007 { "fine grain" } else { "clean" },
        if detail > 0.05 { "dense" } else if detail < 0.02 { "sparse" } else { "moderate" },
        if hard >= 0.08 { "hard graphic" } else { "soft painterly/photographic" },
        centred,
        images.len(),
        neg * 100.0,
        chars.iter().map(|(c, k)| format!("{c} ×{k}")).collect::<Vec<_>>().join(", ")
    );
    let spec_patch = json!({ "theme": { "colors": tok.clone() }, "post": Value::Object(post) });
    Ok(Board {
        images,
        palette,
        accents,
        tokens: tok,
        spec_patch,
        summary,
        notes: vec![
            "tokens are a starting point: check text on bg (≥ 4.5:1 is enforced) and pick the accent by eye.",
            "Use the board for colour, value, texture, composition and line character; the subject matter of the references is not yours to copy.",
            "grain/detail/hard_edges are relative measures: compare images against each other and your render's stills against the board.",
        ],
        thumbs,
    })
}

/// The board sheet: each image with its palette and measures, then the merged palette and tokens.
pub fn board_sheet(b: &Board) -> (u32, u32, Vec<u8>) {
    let cols = b.thumbs.len().clamp(1, 4);
    let cw = 300usize;
    let rows = b.thumbs.len().div_ceil(cols);
    let cell_h = |t: &(u32, u32, Vec<u8>)| (cw as f32 * t.1 as f32 / t.0 as f32).round() as usize;
    let max_h = b.thumbs.iter().map(cell_h).max().unwrap_or(cw).min(420);
    let pad = 10;
    let row_h = max_h + 26 + 30 + pad;
    let w = cols * (cw + pad) + pad;
    let foot = 150;
    let h = rows * row_h + pad + foot;
    let mut c = Canvas { w, h, px: vec![22; w * h * 3] };
    for (i, (t, il)) in b.thumbs.iter().zip(&b.images).enumerate() {
        let (gx, gy) = (pad + (i % cols) * (cw + pad), pad + (i / cols) * row_h);
        let th = cell_h(t).min(max_h);
        for y in 0..th {
            let sy = (y * t.1 as usize / cell_h(t)).min(t.1 as usize - 1);
            for x in 0..cw {
                let sx = (x * t.0 as usize / cw).min(t.0 as usize - 1);
                let si = (sy * t.0 as usize + sx) * 3;
                let di = ((gy + y) * w + gx + x) * 3;
                c.px[di..di + 3].copy_from_slice(&t.2[si..si + 3]);
            }
        }
        let mut x = gx;
        for (j, p) in il.palette.iter().enumerate() {
            let pw = if j + 1 == il.palette.len() { gx + cw - x } else { (p.share * cw as f32).round() as usize };
            c.rect(x as i32, (gy + max_h) as i32, pw as i32, 18, hex_to_rgb(&p.hex));
            x += pw;
        }
        let mut x = gx;
        for (j, p) in il.accents.iter().enumerate() {
            let pw = if j + 1 == il.accents.len() { gx + cw - x } else { (p.share * cw as f32).round() as usize };
            c.rect(x as i32, (gy + max_h + 18) as i32, pw as i32, 8, hex_to_rgb(&p.hex));
            x += pw;
        }
        let l1 = format!("{} C{:.2} NEG{:.0}", il.key, il.contrast, il.negative_space * 100.0);
        let l2 = il.lines.character.replace(['(', ')', ','], "");
        c.text(gx as i32 + 2, (gy + max_h + 30) as i32, &l1, 1, [230, 230, 230]);
        c.text(gx as i32 + 2, (gy + max_h + 39) as i32, &l2, 1, [180, 210, 255]);
    }
    // Merged palette (by share) and the tokens.
    let fy = h - foot;
    let mut x = pad;
    let tw = w - 2 * pad;
    for (j, p) in b.palette.iter().enumerate() {
        let pw = if j + 1 == b.palette.len() { pad + tw - x } else { (p.share * tw as f32).round() as usize };
        c.rect(x as i32, fy as i32 + 6, pw as i32, 50, hex_to_rgb(&p.hex));
        x += pw;
    }
    let mut x = pad;
    for (j, p) in b.accents.iter().enumerate() {
        let pw = if j + 1 == b.accents.len() { pad + tw - x } else { (p.share * tw as f32).round() as usize };
        c.rect(x as i32, fy as i32 + 44, pw as i32, 12, hex_to_rgb(&p.hex));
        x += pw;
    }
    let names = ["bg", "surface", "text", "muted", "primary", "accent"];
    let sw = tw / names.len();
    for (k, n) in names.iter().enumerate() {
        let hx = b.tokens[n].as_str().unwrap_or("#000000");
        let x0 = (pad + k * sw) as i32;
        c.rect(x0, fy as i32 + 66, sw as i32 - 6, 50, hex_to_rgb(hx));
        c.text(x0 + 2, fy as i32 + 122, &format!("{n} {hx}"), 2, [235, 235, 235]);
    }
    (w as u32, h as u32, c.px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_palette_lines_and_space() {
        let dir = std::env::temp_dir().join(format!("anikit-board-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Navy ground with orange horizontal bands in the lower half; and a quiet cream card with one dark bar.
        let a = image::RgbImage::from_fn(320, 240, |_, y| if y > 120 && (y / 12) % 2 == 0 { image::Rgb([235, 120, 30]) } else { image::Rgb([18, 28, 60]) });
        let b = image::RgbImage::from_fn(320, 240, |x, y| if (150..170).contains(&x) && (40..200).contains(&y) { image::Rgb([20, 20, 20]) } else { image::Rgb([240, 232, 214]) });
        a.save(dir.join("a.png")).unwrap();
        b.save(dir.join("b.png")).unwrap();
        let files = collect(&[dir.to_string_lossy().to_string()]).unwrap();
        let bd = board(&files).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(bd.images.len(), 2);
        let ia = &bd.images[0];
        assert!(ia.lines.character.contains("horizontals"), "{:?}", ia.lines);
        assert!(ia.palette.iter().any(|s| hex_to_rgb(&s.hex)[0] > 200 && hex_to_rgb(&s.hex)[2] < 80), "{:?}", ia.palette);
        let ib = &bd.images[1];
        assert!(ib.lines.character.contains("verticals"), "{:?}", ib.lines);
        assert!(ib.negative_space > 0.8, "{}", ib.negative_space);
        assert_eq!(ib.key, "high-key");
        // Tokens: readable text on the ground; the orange is the primary.
        let t = &bd.tokens;
        assert!(contrast_ratio(t["text"].as_str().unwrap(), t["bg"].as_str().unwrap()) >= 4.5, "{t}");
        assert!(hex_to_rgb(t["primary"].as_str().unwrap())[0] > 200, "{t}");
        // The patch is valid spec JSON.
        let spec = json!({ "theme": bd.spec_patch["theme"], "post": bd.spec_patch["post"], "scenes": [{ "duration": 1, "layers": [{ "text": "x", "color": "$primary" }] }] });
        let p = crate::spec::parse::from_value(spec).unwrap();
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
        let (w, h, px) = board_sheet(&bd);
        assert_eq!(px.len(), (w * h * 3) as usize);
    }
}

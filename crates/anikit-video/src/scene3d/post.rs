//! The `scene3d` post stack, all in linear HDR until the very end:
//! floor-reflection blur → depth of field → motion blur → bloom → ACES tone
//! map → vignette → sRGB with dithering.

use super::math::smoothstep;
use super::trace::Px;

pub struct Post {
    /// Focus distance (view depth) and blur radius in pixels for points at infinity.
    pub focus: f32,
    pub aperture: f32,
    /// Shutter as a fraction of the frame interval (0.5 = 180°).
    pub shutter: f32,
    pub bloom: f32,
    pub exposure: f32,
    pub vignette: f32,
    /// `aces` (filmic, contrasty) or `neutral` (Khronos PBR Neutral: whites stay white).
    pub tonemap: Tonemap,
    /// Denoiser passes (0 = off).
    pub denoise: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tonemap {
    Aces,
    Neutral,
}

/// Edge-aware à-trous filter over (AO, key visibility): passes at steps 1,
/// 2, 4, … with a 3×3 tent kernel, stopped by depth, normal and id.
fn denoise_vis(px: &[Px], w: usize, h: usize, passes: u32) -> Vec<[f32; 2]> {
    let mut cur: Vec<[f32; 2]> = px.iter().map(|q| [q.ao, q.vk]).collect();
    // Only pixels with diffuse albedo matter.
    let lit: Vec<bool> = px.iter().map(|q| q.alb[0] + q.alb[1] + q.alb[2] + q.spec[0] + q.spec[1] + q.spec[2] > 1e-5).collect();
    let k = [0.25f32, 0.5, 0.25];
    for pass in 0..passes {
        let step = 1i32 << pass;
        let src = cur.clone();
        let body = |(y, row): (usize, &mut [[f32; 2]])| {
            for x in 0..w {
                let i = y * w + x;
                if !lit[i] {
                    continue;
                }
                let c = &px[i];
                let mut acc = [0.0f32; 2];
                let mut wsum = 0.0;
                for (jy, ky) in k.iter().enumerate() {
                    let sy = y as i32 + (jy as i32 - 1) * step;
                    if sy < 0 || sy >= h as i32 {
                        continue;
                    }
                    for (jx, kx) in k.iter().enumerate() {
                        let sx = x as i32 + (jx as i32 - 1) * step;
                        if sx < 0 || sx >= w as i32 {
                            continue;
                        }
                        let j = sy as usize * w + sx as usize;
                        if !lit[j] {
                            continue;
                        }
                        let t = &px[j];
                        if t.id != c.id && t.id != u32::MAX && c.id != u32::MAX {
                            // Different objects only blend at the floor (id MAX).
                            continue;
                        }
                        let dz = (t.depth - c.depth).abs() / (c.depth * 0.02 + 1e-3);
                        let mut nd = (c.nrm[0] * t.nrm[0] + c.nrm[1] * t.nrm[1] + c.nrm[2] * t.nrm[2]).max(0.0);
                        nd *= nd;
                        nd *= nd;
                        nd *= nd;
                        let wt = ky * kx * nd * nd / (1.0 + dz * dz);
                        let v = src[j];
                        acc[0] += v[0] * wt;
                        acc[1] += v[1] * wt;
                        wsum += wt;
                    }
                }
                if wsum > 1e-6 {
                    row[x] = [acc[0] / wsum, acc[1] / wsum];
                }
            }
        };
        use rayon::prelude::*;
        if rayon::current_thread_index().is_some() {
            cur.chunks_mut(w).enumerate().for_each(body);
        } else {
            cur.par_chunks_mut(w).enumerate().for_each(body);
        }
    }
    cur
}

type Img = Vec<[f32; 3]>;

#[allow(dead_code)]
/// Vogel (golden-angle) disc: `n` points with radius ≤ 1.
fn vogel(n: usize) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let r = ((i as f32 + 0.5) / n as f32).sqrt();
            let a = i as f32 * 2.399_963;
            (r * a.cos(), r * a.sin())
        })
        .collect()
}

fn par_rows<F: Fn(usize, &mut [[f32; 3]]) + Sync + Send>(out: &mut Img, w: usize, f: F) {
    use rayon::prelude::*;
    if rayon::current_thread_index().is_some() {
        out.chunks_mut(w).enumerate().for_each(|(y, r)| f(y, r));
    } else {
        out.par_chunks_mut(w).enumerate().for_each(|(y, r)| f(y, r));
    }
}

#[inline]
fn fetch(img: &[[f32; 3]], w: usize, h: usize, x: f32, y: f32) -> [f32; 3] {
    // Bilinear, clamped.
    let x = x.clamp(0.0, w as f32 - 1.001);
    let y = y.clamp(0.0, h as f32 - 1.001);
    let (ix, iy) = (x as usize, y as usize);
    let (tx, ty) = (x - ix as f32, y - iy as f32);
    let a = img[iy * w + ix];
    let b = img[iy * w + ix + 1];
    let c = img[(iy + 1) * w + ix];
    let d = img[(iy + 1) * w + ix + 1];
    std::array::from_fn(|k| (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty)
}

/// Circle-of-confusion radius (pixels). The backdrop (no geometry) blurs
/// like something about 8× the focus distance away: it is smooth anyway, and
/// a huge radius there starves the gather at object edges.
#[inline]
fn coc_of(p: &Post, s: f32, depth: f32) -> f32 {
    let d = depth.min(p.focus * 8.0).max(1e-3);
    (p.aperture * s * (1.0 - p.focus / d).abs()).min(28.0 * s)
}

fn tick(stages: &mut Vec<(&'static str, f64)>, clock: &mut std::time::Instant, name: &'static str) {
    stages.push((name, clock.elapsed().as_secs_f64() * 1e3));
    *clock = std::time::Instant::now();
}

/// `ANIKIT_SCENE3D_DEBUG=refl|col|diffuse` shows one buffer (development aid).
fn debug() -> u8 {
    static D: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *D.get_or_init(|| match std::env::var("ANIKIT_SCENE3D_DEBUG").as_deref() {
        Ok("refl") => 1,
        Ok("col") => 2,
        Ok("diffuse") => 3,
        _ => 0,
    })
}

/// [`box_blur`] for 4-channel pixels.
fn box_blur4(src: &[[f32; 4]], w: usize, h: usize, r: usize, rows: bool) -> Vec<[f32; 4]> {
    let mut out = vec![[0.0f32; 4]; src.len()];
    let (n, m) = if rows { (h, w) } else { (w, h) };
    let one = |a: usize, out: &mut [[f32; 4]]| {
        let at = |b: usize| if rows { a * w + b } else { b * w + a };
        let mut sum = [0.0f32; 4];
        let mut cnt = 0.0f32;
        for b in 0..=r.min(m - 1) {
            for c in 0..4 {
                sum[c] += src[at(b)][c];
            }
            cnt += 1.0;
        }
        for b in 0..m {
            let inv = 1.0 / cnt;
            out[at(b)] = sum.map(|v| v * inv);
            if b + r + 1 < m {
                for c in 0..4 {
                    sum[c] += src[at(b + r + 1)][c];
                }
                cnt += 1.0;
            }
            if b >= r {
                for c in 0..4 {
                    sum[c] -= src[at(b - r)][c];
                }
                cnt -= 1.0;
            }
        }
    };
    if rows {
        use rayon::prelude::*;
        if rayon::current_thread_index().is_some() {
            for a in 0..n {
                one(a, &mut out);
            }
        } else {
            let rows_out: Vec<Vec<[f32; 4]>> = (0..n)
                .into_par_iter()
                .map(|a| {
                    let mut row = vec![[0.0f32; 4]; w];
                    let mut sum = [0.0f32; 4];
                    let mut cnt = 0.0f32;
                    for b in 0..=r.min(m - 1) {
                        for c in 0..4 {
                            sum[c] += src[a * w + b][c];
                        }
                        cnt += 1.0;
                    }
                    for b in 0..m {
                        let inv = 1.0 / cnt;
                        row[b] = sum.map(|v| v * inv);
                        if b + r + 1 < m {
                            for c in 0..4 {
                                sum[c] += src[a * w + b + r + 1][c];
                            }
                            cnt += 1.0;
                        }
                        if b >= r {
                            for c in 0..4 {
                                sum[c] -= src[a * w + b - r][c];
                            }
                            cnt -= 1.0;
                        }
                    }
                    row
                })
                .collect();
            for (a, row) in rows_out.into_iter().enumerate() {
                out[a * w..a * w + w].copy_from_slice(&row);
            }
        }
    } else {
        // Columns: sweep rows keeping one running sum per column (cache friendly).
        let _ = (&one, n);
        let mut sum = vec![[0.0f32; 4]; w];
        let mut cnt = 0.0f32;
        for b in 0..=r.min(h - 1) {
            for x in 0..w {
                for c in 0..4 {
                    sum[x][c] += src[b * w + x][c];
                }
            }
            cnt += 1.0;
        }
        for y in 0..h {
            let inv = 1.0 / cnt;
            for x in 0..w {
                out[y * w + x] = sum[x].map(|v| v * inv);
            }
            if y + r + 1 < h {
                let row = &src[(y + r + 1) * w..(y + r + 2) * w];
                for x in 0..w {
                    for c in 0..4 {
                        sum[x][c] += row[x][c];
                    }
                }
                cnt += 1.0;
            }
            if y >= r {
                let row = &src[(y - r) * w..(y - r + 1) * w];
                for x in 0..w {
                    for c in 0..4 {
                        sum[x][c] -= row[x][c];
                    }
                }
                cnt -= 1.0;
            }
        }
    }
    out
}

/// Separable running-sum box blur (radius `r`) along rows or columns.
fn box_blur(src: &[f32], w: usize, h: usize, r: usize, rows: bool) -> Vec<f32> {
    let mut out = vec![0.0f32; src.len()];
    let (n, m) = if rows { (h, w) } else { (w, h) };
    for a in 0..n {
        let at = |b: usize| if rows { a * w + b } else { b * w + a };
        let mut sum = 0.0f32;
        let mut cnt = 0.0f32;
        for b in 0..=r.min(m - 1) {
            sum += src[at(b)];
            cnt += 1.0;
        }
        for b in 0..m {
            out[at(b)] = sum / cnt;
            if b + r + 1 < m {
                sum += src[at(b + r + 1)];
                cnt += 1.0;
            }
            if b >= r {
                sum -= src[at(b - r)];
                cnt -= 1.0;
            }
        }
    }
    out
}

pub fn process(px: &[Px], w: usize, h: usize, p: &Post, parts: Option<&super::particles::Layer>) -> Vec<u8> {
    let s = h as f32 / 1080.0;
    let mut clock = std::time::Instant::now();
    let mut stages: Vec<(&str, f64)> = Vec::new();
    // 1. Glossy floor reflection: a separable Gaussian whose width follows
    // each pixel's blur radius (contact-sharp, softer with distance), wider
    // vertically like real glossy reflections, normalised by floor coverage
    // so it never bleeds onto objects.
    let rv0: Vec<[f32; 4]> = px.iter().map(|q| [q.refl[0], q.refl[1], q.refl[2], q.cov]).collect();
    // The radius field comes from one ray per pixel and can alternate
    // between rows (hit / miss); a 9×9 box average keeps the blur coherent.
    let rad0: Vec<f32> = px.iter().map(|q| if q.cov > 0.0 { q.rad * s } else { 0.0 }).collect();
    let rad = box_blur(&box_blur(&rad0, w, h, 4, true), w, h, 4, false);
    // A pyramid of blurred levels: level k is 2^k smaller, blurred a little
    // more vertically than horizontally (glossy reflections streak), so its
    // effective σ is about 1.1·2^k px. Every pixel interpolates between the
    // two levels around its own σ (bilinear lookups). Fixed kernels can't
    // ripple the way a per-pixel variable kernel does, and the whole stack
    // costs about a third of one full-resolution pass.
    let maxr = rad.iter().cloned().fold(0.0f32, f32::max) * 0.5 * 1.35;
    struct Lvl {
        sig: f32,
        w: usize,
        h: usize,
        f: f32,
        px: Vec<[f32; 4]>,
    }
    let mut levels: Vec<Lvl> = vec![Lvl { sig: 0.0, w, h, f: 1.0, px: rv0.clone() }];
    let noblur = std::env::var_os("ANIKIT_SCENE3D_NOBLUR").is_some();
    while !noblur && levels.len() < 7 && levels.last().unwrap().sig < maxr {
        let prev = levels.last().unwrap();
        let (nw, nh) = (prev.w.div_ceil(2).max(1), prev.h.div_ceil(2).max(1));
        let mut d = vec![[0.0f32; 4]; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(prev.w - 1), (2 * y + dy).min(prev.h - 1));
                    let t = prev.px[sy * prev.w + sx];
                    for c in 0..4 {
                        acc[c] += t[c] * 0.25;
                    }
                }
                d[y * nw + x] = acc;
            }
        }
        let mut b = box_blur4(&d, nw, nh, 1, false);
        b = box_blur4(&b, nw, nh, 1, false);
        b = box_blur4(&b, nw, nh, 1, true);
        let f = prev.f * 2.0;
        levels.push(Lvl { sig: 1.1 * f, w: nw, h: nh, f, px: b });
    }
    let sample = |l: &Lvl, x: usize, y: usize| -> [f32; 4] {
        if l.f == 1.0 {
            return l.px[y * l.w + x];
        }
        let fx = ((x as f32 + 0.5) / l.f - 0.5).clamp(0.0, l.w as f32 - 1.0);
        let fy = ((y as f32 + 0.5) / l.f - 0.5).clamp(0.0, l.h as f32 - 1.0);
        let (ix, iy) = (fx as usize, fy as usize);
        let (tx, ty) = (fx - ix as f32, fy - iy as f32);
        let (ix1, iy1) = ((ix + 1).min(l.w - 1), (iy + 1).min(l.h - 1));
        let (a, b, c, d) = (l.px[iy * l.w + ix], l.px[iy * l.w + ix1], l.px[iy1 * l.w + ix], l.px[iy1 * l.w + ix1]);
        std::array::from_fn(|k| (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty)
    };
    let rv: Vec<[f32; 4]> = (0..w * h)
        .map(|i| {
            let c = rv0[i];
            if c[3] <= 0.0 {
                return c;
            }
            let sg = rad[i] * 0.5 * 1.35;
            let (x, y) = (i % w, i / w);
            let mut k = 0;
            while k + 1 < levels.len() && levels[k + 1].sig < sg {
                k += 1;
            }
            let a = sample(&levels[k], x, y);
            let (m, _) = if k + 1 < levels.len() {
                let b = sample(&levels[k + 1], x, y);
                let t = ((sg - levels[k].sig) / (levels[k + 1].sig - levels[k].sig)).clamp(0.0, 1.0);
                (std::array::from_fn::<f32, 4, _>(|j| a[j] + (b[j] - a[j]) * t), ())
            } else {
                (a, ())
            };
            // Colour per unit coverage, times this pixel's own coverage.
            if m[3] > 1e-6 {
                let f = c[3] / m[3];
                let r = px[i].rf;
                [m[0] * f * r, m[1] * f * r, m[2] * f * r, c[3]]
            } else {
                let r = px[i].rf;
                [c[0] * r, c[1] * r, c[2] * r, c[3]]
            }
        })
        .collect();
    tick(&mut stages, &mut clock, "refl");
    // 1b. Denoise AO and key visibility (few rays per pixel) with an
    // edge-aware à-trous filter (depth, normal and object id stop it), then
    // light the diffuse albedo with them.
    let vis = denoise_vis(px, w, h, p.denoise);
    let mut hdr: Img = px
        .iter()
        .zip(&rv)
        .zip(&vis)
        .map(|((q, r), v)| {
            let (c, rr, d) = match debug() {
                1 => (0.0, 1.0, 0.0),
                2 => (1.0, 0.0, 0.0),
                3 => (0.0, 0.0, 1.0),
                _ => (1.0, 1.0, 1.0),
            };
            std::array::from_fn(|k| q.col[k] * c + r[k] * rr + (q.alb[k] * (q.amb[k] * v[0] + q.key[k] * v[1]) + q.spec[k] * v[0]) * d)
        })
        .collect();

    tick(&mut stages, &mut clock, "denoise");
    // 2. Depth of field, gathered at half resolution (blur is low-frequency;
    // the 2×2 prefilter also turns sparse highlights into smooth discs) and
    // blended over the sharp image by how defocused each pixel is.
    if p.aperture > 0.05 {
        let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
        let mut hc: Img = vec![[0.0; 3]; hw * hh];
        let mut hd = vec![0.0f32; hw * hh];
        for y in 0..hh {
            for x in 0..hw {
                let mut acc = [0.0f32; 3];
                let mut d = f32::MAX;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(w - 1), (2 * y + dy).min(h - 1));
                    let c = hdr[sy * w + sx];
                    for k in 0..3 {
                        acc[k] += c[k] * 0.25;
                    }
                    d = d.min(px[sy * w + sx].depth);
                }
                hc[y * hw + x] = acc;
                hd[y * hw + x] = d;
            }
        }
        // CoC in half-res pixels.
        let coc: Vec<f32> = hd.iter().map(|&d| coc_of(p, s, d) * 0.5).collect();
        // Max CoC per 8×8 tile, dilated by two tiles, bounds the gather radius.
        let (tw, th) = (hw.div_ceil(8), hh.div_ceil(8));
        let mut tile = vec![0.0f32; tw * th];
        let mut geo = vec![false; tw * th];
        for y in 0..hh {
            for x in 0..hw {
                let ti = (y / 8) * tw + x / 8;
                tile[ti] = tile[ti].max(coc[y * hw + x]);
                geo[ti] |= hd[y * hw + x] < 1e3;
            }
        }
        let mut dil = tile.clone();
        for ty in 0..th {
            for tx in 0..tw {
                let mut m = 0.0f32;
                let mut g = false;
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        let (a, b) = (tx as i32 + dx, ty as i32 + dy);
                        if a >= 0 && b >= 0 && (a as usize) < tw && (b as usize) < th {
                            m = m.max(tile[b as usize * tw + a as usize]);
                            g |= geo[b as usize * tw + a as usize];
                        }
                    }
                }
                dil[ty * tw + tx] = if g { m } else { 0.0 };
            }
        }
        let sets: Vec<Vec<(f32, f32)>> = [24usize, 48, 96, 160].iter().map(|&n| vogel(n)).collect();
        // Half-res result: colour plus how much of it is blur (alpha).
        let mut out4: Vec<[f32; 4]> = vec![[0.0; 4]; hw * hh];
        {
            let body = |(y, row): (usize, &mut [[f32; 4]])| {
                for x in 0..hw {
                    let i0 = y * hw + x;
                    let rmax = dil[(y / 8) * tw + x / 8];
                    let c0 = hc[i0];
                    if rmax < 0.5 {
                        row[x] = [c0[0], c0[1], c0[2], 0.0];
                        continue;
                    }
                    let (dc, cc) = (hd[i0], coc[i0]);
                    let set = &sets[if rmax < 2.5 { 0 } else if rmax < 5.0 { 1 } else if rmax < 9.0 { 2 } else { 3 }];
                    let ang = super::math::u01(super::math::hash2(x as u32, y as u32 ^ 0xdead)) * std::f32::consts::TAU;
                    let (ca, sa) = (ang.cos(), ang.sin());
                    let wt0 = 1.0 / (cc * cc).max(1.0);
                    let mut acc = [c0[0] * wt0, c0[1] * wt0, c0[2] * wt0, smoothstep(0.4, 1.2, cc) * wt0];
                    let mut wsum = wt0;
                    for &(dx, dy) in set {
                        let (fx, fy) = ((dx * ca - dy * sa) * rmax, (dx * sa + dy * ca) * rmax);
                        let sx = (x as f32 + fx).round().clamp(0.0, hw as f32 - 1.0) as usize;
                        let sy = (y as f32 + fy).round().clamp(0.0, hh as f32 - 1.0) as usize;
                        let i = sy * hw + sx;
                        let dist = (fx * fx + fy * fy).sqrt();
                        // A sample behind the centre can't blur over it more than the centre is blurred.
                        let ct = if hd[i] > dc { coc[i].min(cc) } else { coc[i] };
                        let cover = ((ct - dist) * 0.5 + 0.5).clamp(0.0, 1.0);
                        if cover <= 0.0 {
                            continue;
                        }
                        let wt = cover / (ct * ct).max(1.0);
                        let t = hc[i];
                        acc[0] += t[0] * wt;
                        acc[1] += t[1] * wt;
                        acc[2] += t[2] * wt;
                        acc[3] += smoothstep(0.4, 1.2, ct) * wt;
                        wsum += wt;
                    }
                    row[x] = acc.map(|v| v / wsum);
                }
            };
            use rayon::prelude::*;
            if rayon::current_thread_index().is_some() {
                out4.chunks_mut(hw).enumerate().for_each(body);
            } else {
                out4.par_chunks_mut(hw).enumerate().for_each(body);
            }
        }
        // A half-res tent smooths the gather's grain before upsampling.
        let o2 = out4.clone();
        for y in 0..hh {
            for x in 0..hw {
                let mut acc = [0.0f32; 4];
                for (dy, wy) in [(-1i32, 0.25f32), (0, 0.5), (1, 0.25)] {
                    for (dx, wx) in [(-1i32, 0.25f32), (0, 0.5), (1, 0.25)] {
                        let sx = (x as i32 + dx).clamp(0, hw as i32 - 1) as usize;
                        let sy = (y as i32 + dy).clamp(0, hh as i32 - 1) as usize;
                        let t = o2[sy * hw + sx];
                        for k in 0..4 {
                            acc[k] += t[k] * wx * wy;
                        }
                    }
                }
                // Keep in-focus half-res pixels exact.
                let a = smoothstep(0.0, 0.5, o2[y * hw + x][3]);
                out4[y * hw + x] = std::array::from_fn(|k| o2[y * hw + x][k] * (1.0 - a) + acc[k] * a);
            }
        }
        // Upsample and blend.
        par_rows(&mut hdr, w, |y, row| {
            let fy = ((y as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, hh as f32 - 1.0);
            let (iy, ty) = (fy as usize, fy - fy.floor());
            let iy1 = (iy + 1).min(hh - 1);
            for x in 0..w {
                let fx = ((x as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, hw as f32 - 1.0);
                let (ix, tx) = (fx as usize, fx - fx.floor());
                let ix1 = (ix + 1).min(hw - 1);
                let (a, b, c, d) = (out4[iy * hw + ix], out4[iy * hw + ix1], out4[iy1 * hw + ix], out4[iy1 * hw + ix1]);
                let up: [f32; 4] = std::array::from_fn(|k| (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty);
                let own = smoothstep(0.8, 2.2, coc_of(p, s, px[y * w + x].depth));
                let al = up[3].max(own).clamp(0.0, 1.0);
                if al > 0.0 {
                    for k in 0..3 {
                        row[x][k] = row[x][k] * (1.0 - al) + up[k] * al;
                    }
                }
            }
        });
    }

    tick(&mut stages, &mut clock, "dof");
    // 3. Motion blur along each pixel's motion vector.
    if p.shutter > 0.0 {
        let src = hdr.clone();
        par_rows(&mut hdr, w, |y, row| {
            for x in 0..w {
                let v = px[y * w + x].vel;
                let (vx, vy) = (v[0] * p.shutter, v[1] * p.shutter);
                let l = (vx * vx + vy * vy).sqrt();
                if l < 0.35 {
                    continue;
                }
                let n = (l * 1.5).ceil().clamp(3.0, 24.0) as usize;
                let mut acc = [0.0f32; 3];
                for i in 0..n {
                    let t = (i as f32 + 0.5) / n as f32 - 0.5;
                    let c = fetch(&src, w, h, x as f32 - vx * t, y as f32 - vy * t);
                    for k in 0..3 {
                        acc[k] += c[k];
                    }
                }
                row[x] = acc.map(|a| a / n as f32);
            }
        });
    }

    tick(&mut stages, &mut clock, "mblur");
    // 3b. Particles (already blurred and streaked by their splatter) go in
    // before bloom, so bright ones glow.
    if let Some(l) = parts {
        par_rows(&mut hdr, w, |y, row| {
            for (x, c) in row.iter_mut().enumerate() {
                *c = l.apply(y * w + x, *c);
            }
        });
    }
    // 4. Bloom: a pyramid of blurred levels, mostly from highlights.
    if p.bloom > 0.0 {
        let mut levels: Vec<(Img, usize, usize)> = Vec::new();
        // Soft threshold: only what's above ~0.8 contributes strongly.
        let mut cur: Img = hdr
            .iter()
            .map(|c| {
                let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
                let k = ((l - 0.6) / (l + 1e-4)).clamp(0.0, 1.0) * 0.85 + 0.15;
                c.map(|v| v * k)
            })
            .collect();
        let (mut cw, mut ch) = (w, h);
        for _ in 0..7 {
            if cw < 8 || ch < 8 {
                break;
            }
            let (nw, nh) = (cw / 2, ch / 2);
            let mut next: Img = vec![[0.0; 3]; nw * nh];
            let curr = &cur;
            par_rows(&mut next, nw, |y, row| {
                for x in 0..nw {
                    // 13-tap-ish: 4×4 tent over the 2×2 block and its ring.
                    let mut acc = [0.0f32; 3];
                    let mut wsum = 0.0;
                    for dy in -1i32..=2 {
                        for dx in -1i32..=2 {
                            let sx = (2 * x as i32 + dx).clamp(0, cw as i32 - 1) as usize;
                            let sy = (2 * y as i32 + dy).clamp(0, ch as i32 - 1) as usize;
                            let wt = if (0..=1).contains(&dx) { 3.0 } else { 1.0 } * if (0..=1).contains(&dy) { 3.0 } else { 1.0 };
                            let c = curr[sy * cw + sx];
                            for k in 0..3 {
                                acc[k] += c[k] * wt;
                            }
                            wsum += wt;
                        }
                    }
                    row[x] = acc.map(|a| a / wsum);
                }
            });
            levels.push((next.clone(), nw, nh));
            cur = next;
            cw = nw;
            ch = nh;
        }
        // Sum levels back up (bilinear upsampling), wider levels weighted more gently.
        let weights = [0.2, 0.22, 0.2, 0.16, 0.12, 0.06, 0.04];
        let total: f32 = weights.iter().take(levels.len()).sum();
        par_rows(&mut hdr, w, |y, row| {
            for x in 0..w {
                let mut b = [0.0f32; 3];
                for (li, (img, lw, lh)) in levels.iter().enumerate() {
                    let f = (1 << (li + 1)) as f32;
                    let c = fetch(img, *lw, *lh, (x as f32 + 0.5) / f - 0.5, (y as f32 + 0.5) / f - 0.5);
                    for k in 0..3 {
                        b[k] += c[k] * weights[li];
                    }
                }
                for k in 0..3 {
                    row[x][k] = row[x][k] * (1.0 - p.bloom) + b[k] / total * p.bloom * 1.6;
                }
            }
        });
    }

    tick(&mut stages, &mut clock, "bloom");
    // 5. Tone map, vignette, sRGB, dither.
    let mut out = vec![0u8; w * h * 4];
    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
    let inv_d = 1.0 / (cx * cx + cy * cy).sqrt();
    let body = |(y, row): (usize, &mut [u8])| {
        for x in 0..w {
            let c = hdr[y * w + x];
            let (dx, dy) = ((x as f32 - cx) * inv_d, (y as f32 - cy) * inv_d);
            let vig = 1.0 - p.vignette * (dx * dx + dy * dy).powf(1.3) * 1.6;
            let hs = super::math::hash2(x as u32 ^ 0x51ed, y as u32 ^ 0x27d4);
            let dith = (super::math::u01(hs) + super::math::u01(super::math::hash2(hs, 77)) - 1.0) / 255.0;
            let e = [c[0] * p.exposure * vig, c[1] * p.exposure * vig, c[2] * p.exposure * vig];
            let mapped = if p.tonemap == Tonemap::Neutral { neutral(e) } else { aces(e) };
            for k in 0..3 {
                row[x * 4 + k] = ((srgb(mapped[k]) + dith) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            }
            row[x * 4 + 3] = 255;
        }
    };
    use rayon::prelude::*;
    if rayon::current_thread_index().is_some() {
        out.chunks_mut(w * 4).enumerate().for_each(body);
    } else {
        out.par_chunks_mut(w * 4).enumerate().for_each(body);
    }
    tick(&mut stages, &mut clock, "tonemap");
    if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() {
        eprintln!("  post: {}", stages.iter().map(|(n, ms)| format!("{n} {ms:.0}")).collect::<Vec<_>>().join(", "));
    }
    out
}

/// ACES filmic (Stephen Hill's RRT + ODT fit), sRGB primaries in and out.
fn aces(c: [f32; 3]) -> [f32; 3] {
    let i = [[0.59719, 0.35458, 0.04823], [0.07600, 0.90834, 0.01566], [0.02840, 0.13383, 0.83777]];
    let o = [[1.60475, -0.53108, -0.07367], [-0.10208, 1.10813, -0.00605], [-0.00327, -0.07276, 1.07602]];
    let a: [f32; 3] = std::array::from_fn(|r| i[r][0] * c[0] + i[r][1] * c[1] + i[r][2] * c[2]);
    let b = a.map(|v| {
        let n = v * (v + 0.0245786) - 0.000090537;
        let d = v * (0.983729 * v + 0.432951) + 0.238081;
        n / d
    });
    std::array::from_fn(|r| (o[r][0] * b[0] + o[r][1] * b[1] + o[r][2] * b[2]).clamp(0.0, 1.0))
}

/// Khronos PBR Neutral tone mapping: colours stay true, white stays white.
fn neutral(c: [f32; 3]) -> [f32; 3] {
    const START: f32 = 0.8 - 0.04;
    const DESAT: f32 = 0.15;
    let x = c[0].min(c[1]).min(c[2]);
    let off = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
    let mut c = c.map(|v| v - off);
    let peak = c[0].max(c[1]).max(c[2]);
    if peak < START {
        return c.map(|v| v.clamp(0.0, 1.0));
    }
    let d = 1.0 - START;
    let np = 1.0 - d * d / (peak + d - START);
    c = c.map(|v| v * np / peak);
    let g = 1.0 - 1.0 / (DESAT * (peak - np) + 1.0);
    c.map(|v| (v * (1.0 - g) + np * g).clamp(0.0, 1.0))
}

fn srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

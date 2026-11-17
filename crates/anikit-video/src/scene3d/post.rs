//! The `scene3d` post stack, all in linear HDR until the very end:
//! floor-reflection blur → depth of field → motion blur → bloom → ACES tone
//! map → vignette → sRGB with dithering.

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

pub fn process(px: &[Px], w: usize, h: usize, p: &Post) -> Vec<u8> {
    let s = h as f32 / 1080.0;
    // 1. Glossy floor reflection: a separable Gaussian whose width follows
    // each pixel's blur radius (contact-sharp, softer with distance), wider
    // vertically like real glossy reflections, normalised by floor coverage
    // so it never bleeds onto objects.
    let mut rv: Vec<[f32; 4]> = px.iter().map(|q| [q.refl[0], q.refl[1], q.refl[2], q.cov]).collect();
    let rad: Vec<f32> = px.iter().map(|q| q.rad * s).collect();
    for (pass, stretch) in [(0usize, 0.75f32), (1, 1.35)] {
        let src = rv.clone();
        let line = |y: usize, row: &mut [[f32; 4]]| {
            for x in 0..w {
                let i = y * w + x;
                if px[i].cov <= 0.0 {
                    continue;
                }
                let sig = rad[i] * stretch * 0.5;
                if sig < 0.4 {
                    continue;
                }
                let reach = (sig * 2.5).min(120.0);
                let step = (reach / 16.0).max(1.0);
                let mut acc = [0.0f32; 4];
                let mut k = -reach;
                while k <= reach + 1e-3 {
                    let wt = (-(k * k) / (2.0 * sig * sig)).exp();
                    let (sx, sy) = if pass == 0 { ((x as f32 + k).round().clamp(0.0, w as f32 - 1.0) as usize, y) } else { (x, (y as f32 + k).round().clamp(0.0, h as f32 - 1.0) as usize) };
                    let t = src[sy * w + sx];
                    for c in 0..4 {
                        acc[c] += t[c] * wt;
                    }
                    k += step;
                }
                if acc[3] > 1e-6 {
                    // Keep this pixel's own coverage; take the colour per unit coverage.
                    let f = src[i][3] / acc[3];
                    row[x] = [acc[0] * f, acc[1] * f, acc[2] * f, src[i][3]];
                }
            }
        };
        use rayon::prelude::*;
        let rvp = &mut rv;
        if rayon::current_thread_index().is_some() {
            rvp.chunks_mut(w).enumerate().for_each(|(y, r)| line(y, r));
        } else {
            rvp.par_chunks_mut(w).enumerate().for_each(|(y, r)| line(y, r));
        }
    }
    let mut hdr: Img = px.iter().zip(&rv).map(|(q, r)| [q.col[0] + r[0], q.col[1] + r[1], q.col[2] + r[2]]).collect();

    // 2. Depth of field: scatter-as-gather with per-pixel circles of confusion.
    if p.aperture > 0.05 {
        let coc: Vec<f32> = px.iter().map(|q| coc_of(p, s, q.depth)).collect();
        // Max CoC per 16×16 tile, dilated by one tile, bounds the gather radius.
        let (tw, th) = (w.div_ceil(16), h.div_ceil(16));
        let mut tile = vec![0.0f32; tw * th];
        // Tiles that see geometry; pure backdrop (a smooth gradient) needs no gather.
        let mut geo = vec![false; tw * th];
        for y in 0..h {
            for x in 0..w {
                let ti = (y / 16) * tw + x / 16;
                tile[ti] = tile[ti].max(coc[y * w + x]);
                geo[ti] |= px[y * w + x].depth < 1e3;
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
        let sets: Vec<Vec<(f32, f32)>> = [32usize, 64, 128, 224].iter().map(|&n| vogel(n)).collect();
        let src = hdr.clone();
        // Taps read a tent-prefiltered copy: each tap then stands for its
        // neighbourhood, so sparse highlights blur into smooth discs instead
        // of salt-and-pepper grain.
        let mut pre = hdr.clone();
        par_rows(&mut pre, w, |y, row| {
            let y0 = y.saturating_sub(1);
            let y1 = (y + 1).min(h - 1);
            for x in 0..w {
                let x0 = x.saturating_sub(1);
                let x1 = (x + 1).min(w - 1);
                let mut acc = [0.0f32; 3];
                for (yy, wy) in [(y0, 1.0), (y, 2.0), (y1, 1.0)] {
                    for (xx, wx) in [(x0, 1.0), (x, 2.0), (x1, 1.0)] {
                        let t = src[yy * w + xx];
                        for k in 0..3 {
                            acc[k] += t[k] * wx * wy / 16.0;
                        }
                    }
                }
                row[x] = acc;
            }
        });
        par_rows(&mut hdr, w, |y, row| {
            for x in 0..w {
                let rmax = dil[(y / 16) * tw + x / 16];
                if rmax < 0.6 {
                    row[x] = src[y * w + x];
                    continue;
                }
                let i0 = y * w + x;
                let (dc, cc) = (px[i0].depth, coc[i0]);
                // Enough taps for the disc area, rotated per pixel: fine grain
                // instead of the ghost copies a fixed pattern leaves.
                let set = &sets[if rmax < 4.0 { 0 } else if rmax < 9.0 { 1 } else if rmax < 16.0 { 2 } else { 3 }];
                let ang = super::math::u01(super::math::hash2(x as u32, y as u32 ^ 0xdead)) * std::f32::consts::TAU;
                let (ca, sa) = (ang.cos(), ang.sin());
                let mut acc = [0.0f32; 3];
                let mut wsum = 0.0;
                // Centre sample.
                {
                    let wt = 1.0 / (cc * cc).max(1.0);
                    for k in 0..3 {
                        acc[k] += src[i0][k] * wt;
                    }
                    wsum += wt;
                }
                for &(dx, dy) in set {
                    let (fx, fy) = ((dx * ca - dy * sa) * rmax, (dx * sa + dy * ca) * rmax);
                    let sx = (x as f32 + fx).round().clamp(0.0, w as f32 - 1.0) as usize;
                    let sy = (y as f32 + fy).round().clamp(0.0, h as f32 - 1.0) as usize;
                    let i = sy * w + sx;
                    let dist = (fx * fx + fy * fy).sqrt();
                    // A sample behind the centre can't blur over it more than the centre is blurred.
                    let ct = if px[i].depth > dc { coc[i].min(cc) } else { coc[i] };
                    let cover = ((ct - dist) * 0.5 + 0.5).clamp(0.0, 1.0);
                    if cover <= 0.0 {
                        continue;
                    }
                    let wt = cover / (ct * ct).max(1.0);
                    let t = if rmax >= 4.0 { pre[i] } else { src[i] };
                    for k in 0..3 {
                        acc[k] += t[k] * wt;
                    }
                    wsum += wt;
                }
                row[x] = acc.map(|v| v / wsum);
            }
        });
    }

    // 2b. A 3×3 tent over defocused pixels removes the gather's fine grain.
    if p.aperture > 0.05 {
        let src = hdr.clone();
        par_rows(&mut hdr, w, |y, row| {
            for x in 1..w.saturating_sub(1) {
                if y == 0 || y + 1 >= h {
                    continue;
                }
                let c = coc_of(p, s, px[y * w + x].depth);
                let a = ((c - 1.0) / 3.0).clamp(0.0, 1.0);
                if a <= 0.0 {
                    continue;
                }
                let mut acc = [0.0f32; 3];
                for dy in 0..3 {
                    for dx in 0..3 {
                        let wt = [1.0, 2.0, 1.0][dx] * [1.0, 2.0, 1.0][dy] / 16.0;
                        let t = src[(y + dy - 1) * w + x + dx - 1];
                        for k in 0..3 {
                            acc[k] += t[k] * wt;
                        }
                    }
                }
                for k in 0..3 {
                    row[x][k] = row[x][k] * (1.0 - a) + acc[k] * a;
                }
            }
        });
    }

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
            let mapped = aces([c[0] * p.exposure * vig, c[1] * p.exposure * vig, c[2] * p.exposure * vig]);
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

fn srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

//! Pixel measures for `analyze`: colour (OKLab k-means), sharpness, grain,
//! vignette, glow, points and text boxes.

use super::{r2, Depth, Swatch, TextRegion};

fn lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn unlin(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

#[allow(clippy::excessive_precision)]
pub fn srgb_to_oklab(c: [u8; 3]) -> [f32; 3] {
    let (r, g, b) = (lin(c[0]), lin(c[1]), lin(c[2]));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

#[allow(clippy::excessive_precision)]
pub fn oklab_to_srgb(c: [f32; 3]) -> [u8; 3] {
    let l = (c[0] + 0.3963377774 * c[1] + 0.2158037573 * c[2]).powi(3);
    let m = (c[0] - 0.1055613458 * c[1] - 0.0638541728 * c[2]).powi(3);
    let s = (c[0] - 0.0894841775 * c[1] - 1.2914855480 * c[2]).powi(3);
    [
        unlin(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
        unlin(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
        unlin(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s),
    ]
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

pub fn hex_to_rgb(h: &str) -> [u8; 3] {
    let h = h.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    [p(0), p(2), p(4)]
}

pub fn hex_to_oklab(h: &str) -> [f32; 3] {
    srgb_to_oklab(hex_to_rgb(h))
}

/// k-means in OKLab (deterministic farthest-point start), clusters by share.
pub fn kmeans_palette(samples: &[[f32; 3]], k: usize) -> Vec<Swatch> {
    if samples.is_empty() {
        return Vec::new();
    }
    let d2 = |a: &[f32; 3], b: &[f32; 3]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
    let n = samples.len() as f32;
    let mean = samples.iter().fold([0.0; 3], |a, s| [a[0] + s[0] / n, a[1] + s[1] / n, a[2] + s[2] / n]);
    let mut cents: Vec<[f32; 3]> = Vec::new();
    // Start from the sample nearest the mean, then farthest points.
    let first = samples.iter().min_by(|a, b| d2(a, &mean).total_cmp(&d2(b, &mean))).copied().unwrap_or(mean);
    cents.push(first);
    let stride = (samples.len() / 4000).max(1);
    while cents.len() < k {
        let far = samples.iter().step_by(stride).max_by(|a, b| {
            let da = cents.iter().map(|c| d2(a, c)).fold(f32::MAX, f32::min);
            let db = cents.iter().map(|c| d2(b, c)).fold(f32::MAX, f32::min);
            da.total_cmp(&db)
        });
        match far {
            Some(f) if cents.iter().all(|c| d2(f, c) > 1e-4) => cents.push(*f),
            _ => break,
        }
    }
    let mut assign = vec![0usize; samples.len()];
    for _ in 0..12 {
        for (i, s) in samples.iter().enumerate() {
            assign[i] = (0..cents.len()).min_by(|&a, &b| d2(s, &cents[a]).total_cmp(&d2(s, &cents[b]))).unwrap_or(0);
        }
        let mut acc = vec![[0f64; 4]; cents.len()];
        for (i, s) in samples.iter().enumerate() {
            let a = &mut acc[assign[i]];
            a[0] += s[0] as f64;
            a[1] += s[1] as f64;
            a[2] += s[2] as f64;
            a[3] += 1.0;
        }
        for (c, a) in cents.iter_mut().zip(&acc) {
            if a[3] > 0.0 {
                *c = [(a[0] / a[3]) as f32, (a[1] / a[3]) as f32, (a[2] / a[3]) as f32];
            }
        }
    }
    let mut counts = vec![0usize; cents.len()];
    for a in &assign {
        counts[*a] += 1;
    }
    // Merge near-identical clusters (ΔE_ok < 0.05), so a two-colour frame reads as two colours.
    let mut i = 0;
    while i < cents.len() {
        let mut j = i + 1;
        while j < cents.len() {
            if d2(&cents[i], &cents[j]) < 0.05 * 0.05 && counts[i] + counts[j] > 0 {
                let (a, b) = (counts[i] as f32, counts[j] as f32);
                let t = a + b;
                cents[i] = [0, 1, 2].map(|c| (cents[i][c] * a + cents[j][c] * b) / t);
                counts[i] += counts[j];
                cents.remove(j);
                counts.remove(j);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
    let mut out: Vec<Swatch> = cents
        .iter()
        .zip(&counts)
        .filter(|(_, n)| **n > 0)
        .map(|(c, cnt)| Swatch { hex: hex(oklab_to_srgb(*c)), share: r2(*cnt as f32 / n), l: r2(c[0]), c: (c[1].hypot(c[2]) * 1000.0).round() / 1000.0 })
        .collect();
    out.sort_by(|a, b| b.share.total_cmp(&a.share));
    out
}

pub fn luma_plane(rgb: &[u8], w: usize, h: usize) -> Vec<f32> {
    (0..w * h).map(|i| (0.2126 * rgb[i * 3] as f32 + 0.7152 * rgb[i * 3 + 1] as f32 + 0.0722 * rgb[i * 3 + 2] as f32) / 255.0).collect()
}

pub fn mean_abs_laplacian(l: &[f32], w: usize, h: usize) -> f32 {
    let mut s = 0.0;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            s += (4.0 * l[i] - l[i - 1] - l[i + 1] - l[i - w] - l[i + w]).abs();
        }
    }
    s / ((w - 2) * (h - 2)).max(1) as f32
}

/// 1 − (corner luma / centre luma), clamped to 0…1.
pub fn vignette(l: &[f32], w: usize, h: usize) -> f32 {
    let (cw, ch) = (w / 8, h / 8);
    let mut corner = 0.0;
    let mut cn = 0.0;
    for &(x0, y0) in &[(0, 0), (w - cw, 0), (0, h - ch), (w - cw, h - ch)] {
        for y in y0..y0 + ch {
            for x in x0..x0 + cw {
                corner += l[y * w + x];
                cn += 1.0;
            }
        }
    }
    let mut centre = 0.0;
    let mut n = 0.0;
    for y in h * 3 / 10..h * 7 / 10 {
        for x in w * 3 / 10..w * 7 / 10 {
            centre += l[y * w + x];
            n += 1.0;
        }
    }
    let (c, m) = (corner / cn, centre / n);
    if m < 0.05 {
        return 0.0;
    }
    (1.0 - c / m).clamp(0.0, 1.0)
}

fn box3(l: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut o = l.to_vec();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let mut s = 0.0;
            for dy in 0..3 {
                for dx in 0..3 {
                    s += l[(y + dy - 1) * w + x + dx - 1];
                }
            }
            o[y * w + x] = s / 9.0;
        }
    }
    o
}

/// Std of the high-pass residual in the flattest fifth of 16×16 tiles.
pub fn grain(l: &[f32], w: usize, h: usize) -> f32 {
    let b = box3(l, w, h);
    let t = 16;
    let mut tiles = Vec::new();
    for ty in 0..h / t {
        for tx in 0..w / t {
            let (mut s, mut s2, mut gm, mut n) = (0.0, 0.0, 0.0, 0.0);
            for y in ty * t + 1..ty * t + t - 1 {
                for x in tx * t + 1..tx * t + t - 1 {
                    let i = y * w + x;
                    let r = l[i] - b[i];
                    s += r;
                    s2 += r * r;
                    gm += (b[i + 1] - b[i - 1]).abs() + (b[i + w] - b[i - w]).abs();
                    n += 1.0;
                }
            }
            let mean_l = b[(ty * t + t / 2) * w + tx * t + t / 2];
            // Skip clipped tiles (no noise survives there).
            if mean_l > 0.03 && mean_l < 0.97 {
                tiles.push((gm / n, ((s2 / n) - (s / n).powi(2)).max(0.0).sqrt()));
            }
        }
    }
    if tiles.is_empty() {
        return 0.0;
    }
    tiles.sort_by(|a, b| a.0.total_cmp(&b.0));
    let k = (tiles.len() / 5).max(1);
    let mut v: Vec<f32> = tiles[..k].iter().map(|t| t.1).collect();
    v.sort_by(f32::total_cmp);
    v[v.len() / 2]
}

/// Brightness of a ring around clipped highlights, above the frame's typical
/// level, relative to the headroom: 0 (hard edges) … 1 (strong bloom).
pub fn glow(l: &[f32], w: usize, h: usize) -> f32 {
    let bright: Vec<bool> = l.iter().map(|v| *v > 0.92).collect();
    let nb = bright.iter().filter(|b| **b).count();
    if nb < 20 || nb > l.len() / 4 {
        return 0.0;
    }
    // Pixels 3…8 px from a bright pixel (Chebyshev), not themselves bright.
    let mut near = vec![u8::MAX; l.len()];
    for y in 0..h {
        for x in 0..w {
            if bright[y * w + x] {
                near[y * w + x] = 0;
            }
        }
    }
    // Two-pass chamfer distance (Chebyshev).
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut d = near[i];
            if x > 0 {
                d = d.min(near[i - 1].saturating_add(1));
            }
            if y > 0 {
                d = d.min(near[i - w].saturating_add(1));
                if x > 0 {
                    d = d.min(near[i - w - 1].saturating_add(1));
                }
                if x + 1 < w {
                    d = d.min(near[i - w + 1].saturating_add(1));
                }
            }
            near[i] = d;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut d = near[i];
            if x + 1 < w {
                d = d.min(near[i + 1].saturating_add(1));
            }
            if y + 1 < h {
                d = d.min(near[i + w].saturating_add(1));
                if x + 1 < w {
                    d = d.min(near[i + w + 1].saturating_add(1));
                }
                if x > 0 {
                    d = d.min(near[i + w - 1].saturating_add(1));
                }
            }
            near[i] = d;
        }
    }
    let (mut ring, mut rn, mut far, mut fnn) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..l.len() {
        match near[i] {
            3..=8 => {
                ring += l[i];
                rn += 1.0;
            }
            d if d > 24 => {
                far += l[i];
                fnn += 1.0;
            }
            _ => {}
        }
    }
    if rn < 10.0 || fnn < 10.0 {
        return 0.0;
    }
    let (ring, far) = (ring / rn, far / fnn);
    ((ring - far) / (1.0 - far).max(0.05)).clamp(0.0, 1.0)
}

/// Small isolated bright points (≤ 12 px blobs well above their surroundings).
pub fn count_points(l: &[f32], w: usize, h: usize) -> usize {
    let mut n = 0;
    let r = 4;
    let mut y = r;
    while y < h - r {
        let mut x = r;
        while x < w - r {
            let v = l[y * w + x];
            if v > 0.45 {
                // A local maximum…
                let mut is_max = true;
                'm: for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if (dx, dy) != (0, 0) && l[((y as i32 + dy) as usize) * w + (x as i32 + dx) as usize] > v {
                            is_max = false;
                            break 'm;
                        }
                    }
                }
                // …that is much brighter than a ring at radius 4.
                if is_max {
                    let ring = [l[(y - r) * w + x], l[(y + r) * w + x], l[y * w + x - r], l[y * w + x + r], l[(y - r) * w + x - r], l[(y + r) * w + x + r], l[(y - r) * w + x + r], l[(y + r) * w + x - r]];
                    let mx = ring.iter().copied().fold(0.0, f32::max);
                    if v - mx > 0.22 {
                        n += 1;
                    }
                }
            }
            x += 1;
        }
        y += 1;
    }
    n
}

/// Share of pixels with any gradient, and the share of those that are hard
/// edges. Vector motion graphics have hard edges between flat fills (≥ 0.08
/// measured on the gallery stills); renders and photos shade smoothly (≤ 0.05).
pub fn flatness(l: &[f32], w: usize, h: usize) -> (f32, f32) {
    let (mut nonflat, mut hard, mut n) = (0.0, 0.0, 0.0);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let g = (l[i + 1] - l[i - 1]).abs() + (l[i + w] - l[i - w]).abs();
            if g >= 0.006 {
                nonflat += 1.0;
                if g > 0.15 {
                    hard += 1.0;
                }
            }
            n += 1.0;
        }
    }
    (nonflat / n, if nonflat > 0.0 { hard / nonflat } else { 0.0 })
}

/// Sharpness per tile on a 6×4 grid: the sharpest tile, and how many
/// textured tiles are much softer (defocus).
pub fn depth_cues(l: &[f32], w: usize, h: usize) -> Depth {
    let (gx, gy) = (6, 4);
    let (tw, th) = (w / gx, h / gy);
    let mut tiles = Vec::new();
    for ty in 0..gy {
        for tx in 0..gx {
            let (mut lap, mut s, mut s2, mut n) = (0.0, 0.0, 0.0, 0.0);
            for y in (ty * th).max(1)..((ty + 1) * th).min(h - 1) {
                for x in (tx * tw).max(1)..((tx + 1) * tw).min(w - 1) {
                    let i = y * w + x;
                    lap += (4.0 * l[i] - l[i - 1] - l[i + 1] - l[i - w] - l[i + w]).abs();
                    s += l[i];
                    s2 += l[i] * l[i];
                    n += 1.0;
                }
            }
            let std = ((s2 / n) - (s / n).powi(2)).max(0.0f32).sqrt();
            tiles.push((lap / n, std));
        }
    }
    let sharp = tiles.iter().map(|t| t.0).fold(0.0, f32::max);
    let textured: Vec<_> = tiles.iter().filter(|t| t.1 > 0.04).collect();
    let soft = textured.iter().filter(|t| t.0 < sharp * 0.3).count();
    let share = if textured.is_empty() { 0.0 } else { soft as f32 / textured.len() as f32 };
    let dof = if sharp < 0.012 && !textured.is_empty() {
        "soft"
    } else if share > 0.3 {
        "shallow"
    } else {
        "deep"
    };
    Depth { sharpness: (sharp * 1000.0).round() / 1000.0, defocus_share: r2(share), dof: dof.into() }
}

/// Boxes of dense, high-contrast vertical strokes (glyph edges), merged into lines.
pub fn text_regions(l: &[f32], w: usize, h: usize) -> Vec<TextRegion> {
    // Strong horizontal-gradient pixels.
    let mut e = vec![false; w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            e[i] = (l[i + 1] - l[i - 1]).abs() > 0.2;
        }
    }
    // Close horizontally (join letters) and a little vertically.
    let gap = (w / 80).max(4);
    let mut m = vec![false; w * h];
    for y in 0..h {
        let mut last: Option<usize> = None;
        for x in 0..w {
            if e[y * w + x] {
                if let Some(lx) = last {
                    if x - lx <= gap {
                        for k in lx..=x {
                            m[y * w + k] = true;
                        }
                    }
                }
                m[y * w + x] = true;
                last = Some(x);
            }
        }
    }
    let mut m2 = m.clone();
    for y in 1..h - 1 {
        for x in 0..w {
            if m[(y - 1) * w + x] && m[(y + 1) * w + x] {
                m2[y * w + x] = true;
            }
        }
    }
    // Connected components (4-connected).
    let mut label = vec![u32::MAX; w * h];
    let mut boxes: Vec<(usize, usize, usize, usize, usize)> = Vec::new(); // x0 y0 x1 y1 count
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !m2[start] || label[start] != u32::MAX {
            continue;
        }
        let id = boxes.len() as u32;
        let (mut x0, mut y0, mut x1, mut y1, mut cnt) = (w, h, 0, 0, 0);
        stack.push(start);
        label[start] = id;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
            cnt += 1;
            let mut push = |j: usize| {
                if m2[j] && label[j] == u32::MAX {
                    label[j] = id;
                    stack.push(j);
                }
            };
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < w {
                push(i + 1);
            }
            if y > 0 {
                push(i - w);
            }
            if y + 1 < h {
                push(i + w);
            }
        }
        boxes.push((x0, y0, x1, y1, cnt));
    }
    // Join words into lines: boxes of similar height that overlap vertically
    // and sit within about a letter height of each other.
    let mut boxes: Vec<(usize, usize, usize, usize, usize)> = boxes.into_iter().filter(|b| (b.3 - b.1 + 1) as f32 >= h as f32 * 0.015 && (b.3 - b.1) < h / 3).collect();
    boxes.sort_by_key(|b| b.0);
    let mut merged = true;
    while merged {
        merged = false;
        'outer: for i in 0..boxes.len() {
            for j in i + 1..boxes.len() {
                let (a, b) = (boxes[i], boxes[j]);
                let (ha, hb) = ((a.3 - a.1 + 1) as f32, (b.3 - b.1 + 1) as f32);
                let ov = (a.3.min(b.3) as f32 - a.1.max(b.1) as f32 + 1.0).max(0.0);
                let gap = if b.0 > a.2 { (b.0 - a.2) as f32 } else if a.0 > b.2 { (a.0 - b.2) as f32 } else { 0.0 };
                if ov >= 0.6 * ha.min(hb) && ha.max(hb) < 1.6 * ha.min(hb) && gap <= 0.9 * ha.max(hb) {
                    boxes[i] = (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3), a.4 + b.4);
                    boxes.remove(j);
                    merged = true;
                    break 'outer;
                }
            }
        }
    }
    let mut out: Vec<TextRegion> = Vec::new();
    for (x0, y0, x1, y1, cnt) in boxes {
        let (bw, bh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        let hf = bh / h as f32;
        if !(0.025..=0.3).contains(&hf) || bw < bh * 1.8 || bw > w as f32 * 0.98 {
            continue;
        }
        let fill = cnt as f32 / (bw * bh);
        // Strokes: edge pixels per box area, and transitions along the middle row.
        let mut edges = 0;
        for y in y0..=y1 {
            for x in x0..=x1 {
                edges += e[y * w + x] as usize;
            }
        }
        let density = edges as f32 / (bw * bh);
        let midy = (y0 + y1) / 2;
        let strokes = (x0 + 1..=x1).filter(|&x| e[midy * w + x] && !e[midy * w + x - 1]).count() as f32;
        if fill < 0.3 || !(0.05..=0.6).contains(&density) || strokes < (bw / bh * 0.8).max(4.0) {
            continue;
        }
        // Text sits on a quieter ground: texture (foliage, fur) is busy all round.
        let m = (bh * 0.6) as usize;
        let (rx0, ry0, rx1, ry1) = (x0.saturating_sub(m), y0.saturating_sub(m), (x1 + m).min(w - 1), (y1 + m).min(h - 1));
        let mut ring = 0usize;
        let mut rn = 0usize;
        for y in ry0..=ry1 {
            for x in rx0..=rx1 {
                if y < y0 || y > y1 || x < x0 || x > x1 {
                    ring += e[y * w + x] as usize;
                    rn += 1;
                }
            }
        }
        if rn > 0 && ring as f32 / rn as f32 > density * 0.4 {
            continue;
        }
        let bx = [x0 as f32 / w as f32, y0 as f32 / h as f32, bw / w as f32, bh / h as f32];
        let cx = bx[0] + bx[2] / 2.0;
        let align = if (cx - 0.5).abs() < 0.08 {
            "center"
        } else if cx < 0.5 {
            "left"
        } else {
            "right"
        };
        let cy = bx[1] + bx[3] / 2.0;
        let position = if cy < 0.36 { "top" } else if cy > 0.64 { "bottom" } else { "middle" };
        // Ink: the minority side of the box's lightness split.
        let (mut sum, mut cnt) = (0.0f32, 0.0f32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                sum += l[y * w + x];
                cnt += 1.0;
            }
        }
        let mid = sum / cnt;
        let bright = (y0..=y1).flat_map(|y| (x0..=x1).map(move |x| (x, y))).filter(|&(x, y)| l[y * w + x] > mid).count() as f32 / cnt;
        let (ink, ink_share) = if bright < 0.5 { ("light-on-dark", bright) } else { ("dark-on-light", 1.0 - bright) };
        let weight = if ink_share < 0.16 { "thin" } else if ink_share < 0.28 { "regular" } else { "bold" };
        out.push(TextRegion {
            bbox: bx.map(|v| (v * 1000.0).round() / 1000.0),
            height: (hf * 1000.0).round() / 1000.0,
            align: align.into(),
            position: position.into(),
            confidence: r2(density),
            ink: ink.into(),
            ink_share: r2(ink_share),
            weight: weight.into(),
        });
    }
    out.sort_by(|a, b| (b.bbox[2] * b.bbox[3]).total_cmp(&(a.bbox[2] * a.bbox[3])));
    out.truncate(6);
    out
}

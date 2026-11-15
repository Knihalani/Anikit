//! Fast animated GIF encoder (for screenshots / recordings of animations).
//!
//! * Per-frame median-cut palette (256 colours) built from a pixel sample.
//! * Nearest-colour lookups cached in a 32 768-entry RGB555 table.
//! * Only the rectangle that changed since the previous frame is encoded;
//!   unchanged frames just extend the previous frame's delay.
//! * LZW with an open-addressing hash dictionary, no allocations per code.

use alloc::vec::Vec;

pub struct GifEncoder {
    w: u16,
    h: u16,
    out: Vec<u8>,
    prev: Vec<u8>,
    has_prev: bool,
    /// Byte offset of the last frame's delay field (to extend it).
    last_delay_at: Option<usize>,
    /// Frames written.
    pub frames: u32,
}


impl GifEncoder {
    /// `loop_count`: 0 = forever.
    pub fn new(width: u16, height: u16, loop_count: u16) -> GifEncoder {
        let mut out = Vec::with_capacity(1 << 16);
        out.extend_from_slice(b"GIF89a");
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&[0x70, 0, 0]); // no global table, 8-bit colour resolution
        // NETSCAPE2.0 looping extension.
        out.extend_from_slice(&[0x21, 0xFF, 0x0B]);
        out.extend_from_slice(b"NETSCAPE2.0");
        out.extend_from_slice(&[0x03, 0x01]);
        out.extend_from_slice(&loop_count.to_le_bytes());
        out.push(0);
        GifEncoder { w: width, h: height, out, prev: Vec::new(), has_prev: false, last_delay_at: None, frames: 0 }
    }

    /// Add an RGBA frame (`width * height * 4` bytes) shown for `delay_cs`
    /// hundredths of a second.
    pub fn add_frame(&mut self, rgba: &[u8], delay_cs: u16) {
        let (w, h) = (self.w as usize, self.h as usize);
        if rgba.len() < w * h * 4 {
            return;
        }
        // Changed rectangle vs the previous frame.
        let (x0, y0, x1, y1) = if self.has_prev {
            let mut bx = (w, h, 0usize, 0usize);
            for y in 0..h {
                let row = y * w * 4;
                let a = &rgba[row..row + w * 4];
                let b = &self.prev[row..row + w * 4];
                if a == b {
                    continue;
                }
                let mut l = 0;
                while a[l * 4..l * 4 + 3] == b[l * 4..l * 4 + 3] {
                    l += 1;
                }
                let mut r = w - 1;
                while a[r * 4..r * 4 + 3] == b[r * 4..r * 4 + 3] {
                    r -= 1;
                }
                bx.0 = bx.0.min(l);
                bx.2 = bx.2.max(r + 1);
                bx.1 = bx.1.min(y);
                bx.3 = bx.3.max(y + 1);
            }
            if bx.0 >= bx.2 {
                // Identical frame: extend the previous delay.
                if let Some(at) = self.last_delay_at {
                    let d = u16::from_le_bytes([self.out[at], self.out[at + 1]]).saturating_add(delay_cs);
                    self.out[at..at + 2].copy_from_slice(&d.to_le_bytes());
                }
                return;
            }
            bx
        } else {
            (0, 0, w, h)
        };
        let (fw, fh) = (x1 - x0, y1 - y0);
        // Delta frames: unchanged pixels become transparent (index 255) so the
        // previous frame shows through exactly.
        let delta = self.has_prev;
        let changed = |p: usize| -> bool { !delta || rgba[p..p + 3] != self.prev[p..p + 3] };

        // Palette from a sample of the (changed pixels of the) rectangle.
        let total = fw * fh;
        let step = (total / 16384).max(1);
        let mut sample: Vec<[u8; 3]> = Vec::with_capacity(total / step + 1);
        let mut k = 0;
        while k < total {
            let (x, y) = (x0 + k % fw, y0 + k / fw);
            let p = (y * w + x) * 4;
            if changed(p) {
                sample.push([rgba[p], rgba[p + 1], rgba[p + 2]]);
            }
            k += step;
        }
        if sample.is_empty() {
            let p = (y0 * w + x0) * 4;
            sample.push([rgba[p], rgba[p + 1], rgba[p + 2]]);
        }
        let palette = build_palette(&mut sample, if delta { 255 } else { 256 });

        // Map pixels to palette indices, caching exact 24-bit colours in an
        // open-addressed table (a lossy RGB555 key would merge near colours).
        const CAP: usize = 1 << 16;
        let mut keys = alloc::vec![u32::MAX; CAP];
        let mut vals = alloc::vec![0u8; CAP];
        let mut used = 0usize;
        let mut idx = Vec::with_capacity(total);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = (y * w + x) * 4;
                if !changed(p) {
                    idx.push(255);
                    continue;
                }
                let (r, g, b) = (rgba[p], rgba[p + 1], rgba[p + 2]);
                let key = ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
                let mut h = (key.wrapping_mul(2654435761) >> 16) as usize & (CAP - 1);
                let c = loop {
                    if keys[h] == key {
                        break vals[h];
                    }
                    if keys[h] == u32::MAX {
                        let c = nearest(&palette, r, g, b) as u8;
                        if used < CAP / 2 {
                            keys[h] = key;
                            vals[h] = c;
                            used += 1;
                        }
                        break c;
                    }
                    h = (h + 1) & (CAP - 1);
                };
                idx.push(c);
            }
        }

        // Graphic control extension: delay, disposal "do not dispose",
        // transparency (index 255) for delta frames.
        self.out.extend_from_slice(&[0x21, 0xF9, 0x04, if delta { 0x05 } else { 0x04 }]);
        self.last_delay_at = Some(self.out.len());
        self.out.extend_from_slice(&delay_cs.to_le_bytes());
        self.out.extend_from_slice(&[if delta { 255 } else { 0 }, 0]);
        // Image descriptor with a local 256-colour table.
        self.out.push(0x2C);
        for v in [x0 as u16, y0 as u16, fw as u16, fh as u16] {
            self.out.extend_from_slice(&v.to_le_bytes());
        }
        self.out.push(0x87);
        for i in 0..256 {
            let c = palette.get(i).copied().unwrap_or([0, 0, 0]);
            self.out.extend_from_slice(&c);
        }
        lzw(&idx, &mut self.out);

        self.prev.clear();
        self.prev.extend_from_slice(&rgba[..w * h * 4]);
        self.has_prev = true;
        self.frames += 1;
    }

    /// Finish and return the file bytes.
    pub fn finish(mut self) -> Vec<u8> {
        self.out.push(0x3B);
        self.out
    }

    /// Bytes written so far (without the trailer).
    pub fn bytes(&self) -> &[u8] {
        &self.out
    }
}

fn nearest(pal: &[[u8; 3]], r: u8, g: u8, b: u8) -> usize {
    let mut best = 0;
    let mut bd = i32::MAX;
    for (i, c) in pal.iter().enumerate() {
        let (dr, dg, db) = (c[0] as i32 - r as i32, c[1] as i32 - g as i32, c[2] as i32 - b as i32);
        // Perceptual-ish weights.
        let d = 2 * dr * dr + 4 * dg * dg + 3 * db * db;
        if d < bd {
            bd = d;
            best = i;
            if d == 0 {
                break;
            }
        }
    }
    best
}

/// Exact slots for frequent colours (flat UI areas reproduce exactly), then
/// median-cut for the rest.
fn build_palette(sample: &mut [[u8; 3]], n: usize) -> Vec<[u8; 3]> {
    let total = sample.len();
    sample.sort_unstable();
    let mut runs: Vec<(usize, [u8; 3])> = Vec::new();
    let mut k = 0;
    while k < total {
        let c = sample[k];
        let mut e = k + 1;
        while e < total && sample[e] == c {
            e += 1;
        }
        runs.push((e - k, c));
        k = e;
    }
    if runs.len() <= n {
        return runs.into_iter().map(|(_, c)| c).collect();
    }
    runs.sort_unstable_by_key(|r| core::cmp::Reverse(r.0));
    let min = (total / 400).max(2);
    let fixed: Vec<[u8; 3]> = runs.iter().take(n / 4).filter(|r| r.0 >= min).map(|r| r.1).collect();
    let mut rest: Vec<[u8; 3]> = sample.iter().filter(|c| !fixed.contains(c)).copied().collect();
    let mut pal = fixed;
    if !rest.is_empty() {
        let room = n - pal.len();
        pal.extend(median_cut(&mut rest, room));
    }
    pal
}

/// Median-cut quantisation into at most `n` colours.
fn median_cut(px: &mut [[u8; 3]], n: usize) -> Vec<[u8; 3]> {
    if px.is_empty() {
        return alloc::vec![[0, 0, 0]];
    }
    let mut boxes: Vec<(usize, usize)> = alloc::vec![(0, px.len())];
    while boxes.len() < n {
        // Split the box with the largest channel range (weighted by size).
        let mut best = None;
        let mut best_score = 0u64;
        for (bi, &(s, e)) in boxes.iter().enumerate() {
            if e - s < 2 {
                continue;
            }
            let (mut lo, mut hi) = ([255u8; 3], [0u8; 3]);
            for p in &px[s..e] {
                for c in 0..3 {
                    lo[c] = lo[c].min(p[c]);
                    hi[c] = hi[c].max(p[c]);
                }
            }
            let (ch, range) = (0..3).map(|c| (c, hi[c] - lo[c])).max_by_key(|&(_, r)| r).unwrap_or((0, 0));
            if range == 0 {
                continue;
            }
            let score = range as u64 * (e - s) as u64;
            if score > best_score {
                best_score = score;
                best = Some((bi, ch));
            }
        }
        let Some((bi, ch)) = best else { break };
        let (s, e) = boxes[bi];
        px[s..e].sort_unstable_by_key(|p| p[ch]);
        let m = s + (e - s) / 2;
        boxes[bi] = (s, m);
        boxes.push((m, e));
    }
    boxes
        .iter()
        .map(|&(s, e)| {
            // Dominant exact colour (flat UI areas must reproduce exactly so
            // delta frames line up); otherwise the mean.
            let slice = &mut px[s..e];
            slice.sort_unstable();
            let (mut best, mut best_n, mut run) = (slice[0], 0usize, 0usize);
            for k in 0..slice.len() {
                run = if k > 0 && slice[k] == slice[k - 1] { run + 1 } else { 1 };
                if run > best_n {
                    best_n = run;
                    best = slice[k];
                }
            }
            if best_n * 4 >= slice.len() {
                return best;
            }
            let mut acc = [0u64; 3];
            for p in slice.iter() {
                for c in 0..3 {
                    acc[c] += p[c] as u64;
                }
            }
            let n = (e - s).max(1) as u64;
            [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8]
        })
        .collect()
}

/// GIF LZW (min code size 8) written as data sub-blocks.
fn lzw(data: &[u8], out: &mut Vec<u8>) {
    const MIN: u32 = 8;
    const CLEAR: u32 = 1 << MIN;
    const EOI: u32 = CLEAR + 1;
    const TABLE: usize = 8209; // prime > 4096 * 2
    out.push(MIN as u8);
    let mut keys = alloc::vec![u32::MAX; TABLE];
    let mut vals = alloc::vec![0u16; TABLE];
    let mut bytes: Vec<u8> = Vec::with_capacity(data.len() / 2 + 16);
    let (mut acc, mut nbits) = (0u32, 0u32);
    let mut width = MIN + 1;
    let mut next = EOI + 1;
    let emit = |code: u32, width: u32, bytes: &mut Vec<u8>, acc: &mut u32, nbits: &mut u32| {
        *acc |= code << *nbits;
        *nbits += width;
        while *nbits >= 8 {
            bytes.push(*acc as u8);
            *acc >>= 8;
            *nbits -= 8;
        }
    };
    emit(CLEAR, width, &mut bytes, &mut acc, &mut nbits);
    if data.is_empty() {
        emit(EOI, width, &mut bytes, &mut acc, &mut nbits);
    } else {
        let mut prefix = data[0] as u32;
        for &b in &data[1..] {
            let key = (prefix << 8) | b as u32;
            let mut h = (key.wrapping_mul(2654435761) as usize) % TABLE;
            let mut found = None;
            while keys[h] != u32::MAX {
                if keys[h] == key {
                    found = Some(vals[h] as u32);
                    break;
                }
                h = (h + 1) % TABLE;
            }
            if let Some(code) = found {
                prefix = code;
                continue;
            }
            emit(prefix, width, &mut bytes, &mut acc, &mut nbits);
            if next < 4096 {
                keys[h] = key;
                vals[h] = next as u16;
                if next == (1 << width) && width < 12 {
                    width += 1;
                }
                next += 1;
            } else {
                emit(CLEAR, width, &mut bytes, &mut acc, &mut nbits);
                keys.iter_mut().for_each(|k| *k = u32::MAX);
                width = MIN + 1;
                next = EOI + 1;
            }
            prefix = b as u32;
        }
        emit(prefix, width, &mut bytes, &mut acc, &mut nbits);
        // A decoder bumps its width one code later than the encoder.
        if next == (1 << width) && width < 12 {
            width += 1;
        }
        emit(EOI, width, &mut bytes, &mut acc, &mut nbits);
    }
    if nbits > 0 {
        bytes.push(acc as u8);
    }
    for chunk in bytes.chunks(255) {
        out.push(chunk.len() as u8);
        out.extend_from_slice(chunk);
    }
    out.push(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal GIF LZW decoder for round-trip tests.
    fn decode_lzw(sub: &[u8], min: u32) -> Vec<u8> {
        let clear = 1u32 << min;
        let eoi = clear + 1;
        let (mut acc, mut nbits, mut width) = (0u32, 0u32, min + 1);
        let mut dict: Vec<Vec<u8>> = (0..clear).map(|i| alloc::vec![i as u8]).collect();
        dict.push(Vec::new());
        dict.push(Vec::new());
        let mut out = Vec::new();
        let mut prev: Option<Vec<u8>> = None;
        let mut it = sub.iter();
        loop {
            while nbits < width {
                match it.next() {
                    Some(&b) => {
                        acc |= (b as u32) << nbits;
                        nbits += 8;
                    }
                    None => return out,
                }
            }
            let code = acc & ((1 << width) - 1);
            acc >>= width;
            nbits -= width;
            if code == clear {
                dict.truncate(eoi as usize + 1);
                width = min + 1;
                prev = None;
                continue;
            }
            if code == eoi {
                return out;
            }
            let entry = if (code as usize) < dict.len() {
                dict[code as usize].clone()
            } else {
                let p = prev.clone().unwrap();
                let mut e = p.clone();
                e.push(p[0]);
                e
            };
            out.extend_from_slice(&entry);
            if let Some(p) = prev {
                let mut n = p.clone();
                n.push(entry[0]);
                dict.push(n);
                if dict.len() == (1 << width) as usize && width < 12 {
                    width += 1;
                }
            }
            prev = Some(entry);
        }
    }

    #[test]
    fn lzw_roundtrip() {
        for data in [
            alloc::vec![0u8; 0],
            alloc::vec![7u8; 1],
            (0..20000u32).map(|i| (i * 7 % 13) as u8).collect::<Vec<u8>>(),
            (0..60000u32).map(|i| (i.wrapping_mul(2654435761u32) >> 24) as u8).collect::<Vec<u8>>(),
        ] {
            let mut out = Vec::new();
            lzw(&data, &mut out);
            assert_eq!(out[0], 8);
            // Unpack sub-blocks.
            let mut sub = Vec::new();
            let mut i = 1;
            while out[i] != 0 {
                let n = out[i] as usize;
                sub.extend_from_slice(&out[i + 1..i + 1 + n]);
                i += n + 1;
            }
            assert_eq!(decode_lzw(&sub, 8), data);
        }
    }

    #[test]
    fn encodes_frames_and_dedupes() {
        let (w, h) = (32u16, 16u16);
        let mut e = GifEncoder::new(w, h, 0);
        let mut f = alloc::vec![0u8; 32 * 16 * 4];
        for (i, px) in f.chunks_mut(4).enumerate() {
            px.copy_from_slice(&[(i * 8) as u8, 100, 200, 255]);
        }
        e.add_frame(&f, 5);
        e.add_frame(&f, 5); // identical → extends delay
        f[4 * 40] = 255;
        e.add_frame(&f, 5);
        assert_eq!(e.frames, 2);
        let bytes = e.finish();
        assert_eq!(&bytes[..6], b"GIF89a");
        assert_eq!(*bytes.last().unwrap(), 0x3B);
        // First frame's delay was extended to 10cs.
        let gce = bytes.windows(3).position(|w| w == [0x21, 0xF9, 0x04]).unwrap();
        assert_eq!(u16::from_le_bytes([bytes[gce + 4], bytes[gce + 5]]), 10);
        // Second frame only covers the changed pixel.
        let second = bytes[gce + 1..].windows(3).position(|w| w == [0x21, 0xF9, 0x04]).unwrap() + gce + 1;
        assert_eq!(bytes[second + 3], 0x05); // transparency on delta frames
        let desc = second + 8;
        assert_eq!(bytes[desc], 0x2C);
        assert_eq!(u16::from_le_bytes([bytes[desc + 5], bytes[desc + 6]]), 1); // width 1
    }

    #[test]
    fn palette_is_exact_for_few_colours() {
        let mut px: Vec<[u8; 3]> = (0..1000).map(|i| if i % 2 == 0 { [255, 0, 0] } else { [0, 0, 255] }).collect();
        let p = median_cut(&mut px, 256);
        assert!(p.contains(&[255, 0, 0]) && p.contains(&[0, 0, 255]));
        // Frequent colours are kept exactly even among many others.
        let mut many: Vec<[u8; 3]> = (0..20000u32)
            .map(|i| if i % 3 == 0 { [22, 26, 34] } else { [(i % 251) as u8, (i % 241) as u8, (i % 239) as u8] })
            .collect();
        let pal = build_palette(&mut many, 256);
        assert!(pal.contains(&[22, 26, 34]));
    }
}

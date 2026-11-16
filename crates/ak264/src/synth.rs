//! Synthetic motion-graphics content for tests and benchmarks.

use crate::frame::YuvBuffer;

/// A deterministic test scene: a gradient background, rounded "cards" that slide,
/// and rows of glyph-like strokes (text) that hold still. `t` is the frame index.
pub struct Scene {
    pub width: u32,
    pub height: u32,
    /// Number of moving cards.
    pub cards: usize,
    /// Pixels per frame of card motion.
    pub speed: i32,
    /// Moving background gradient (full-frame change).
    pub moving_bg: bool,
    /// A panel (fraction of the frame width) whose brightness pulses every frame.
    pub fade: f32,
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

impl Scene {
    pub fn new(width: u32, height: u32) -> Self {
        Scene { width, height, cards: 3, speed: 4, moving_bg: false, fade: 0.0 }
    }

    /// Benchmark presets: `hold` (a small moving element over held text, ~3% of
    /// macroblocks change), `typical` (cards sliding, a pulsing panel; ~15–25%
    /// change), `full` (moving background: every macroblock changes).
    pub fn preset(name: &str, width: u32, height: u32) -> Self {
        let mut s = Scene::new(width, height);
        match name {
            "hold" => {
                s.cards = 1;
                s.speed = 2;
            }
            "full" => {
                s.moving_bg = true;
                s.cards = 6;
                s.speed = 6;
            }
            _ => {
                s.cards = 6;
                s.speed = 5;
                s.fade = 0.3;
            }
        }
        s
    }

    /// Card rectangles at frame t: (x, y, w, h, luma, u, v), rounded outward to pixels.
    pub fn cards_at(&self, t: u32) -> Vec<(i32, i32, i32, i32, u8, u8, u8)> {
        self.cards_f(t)
            .into_iter()
            .map(|(x, y, w, h, l, u, v)| {
                let x0 = x.floor() as i32;
                (x0, y as i32, (x + w).ceil() as i32 - x0, h as i32, l, u, v)
            })
            .collect()
    }

    fn cards_f(&self, t: u32) -> Vec<(f32, f32, f32, f32, u8, u8, u8)> {
        let (w, h) = (self.width as f32, self.height as f32);
        (0..self.cards)
            .map(|i| {
                let cw = ((w / 5.0).max(8.0) as i32 & !1) as f32;
                let ch = ((h / 6.0).max(8.0) as i32 & !1) as f32;
                let span = w + cw;
                // sub-pixel speeds: real renders move by fractional amounts
                let v = self.speed as f32 * (1.0 + (i % 2) as f32) * 0.913;
                let x = ((i as f32 * w / 3.0 + v * t as f32) % span) - cw;
                let y = ((h / 8.0 + i as f32 * h / 4.0) as i32 % ((h - ch) as i32).max(1)) as f32;
                let r = hash(i as u32 + 7);
                (x, y, cw, ch, 60 + (r % 150) as u8, 64 + (r >> 8) as u8 % 128, 64 + (r >> 16) as u8 % 128)
            })
            .collect()
    }

    pub fn frame(&self, t: u32) -> YuvBuffer {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut f = YuvBuffer::new(self.width, self.height);
        let shift = if self.moving_bg { t as usize * 3 } else { 0 };
        for y in 0..h {
            for x in 0..w {
                f.y[y * w + x] = (32 + ((x + shift) * 120 / w.max(1)) + y * 60 / h.max(1)) as u8;
            }
        }
        let (cw, chh) = (w / 2, h / 2);
        for y in 0..chh {
            for x in 0..cw {
                f.u[y * cw + x] = (100 + (x + shift / 2) * 40 / cw.max(1)) as u8;
                f.v[y * cw + x] = (140 - y * 30 / chh.max(1)) as u8;
            }
        }
        if self.fade > 0.0 {
            // a panel in the upper right whose brightness pulses
            let pw = (w as f32 * self.fade) as usize;
            let (px, py, ph) = (w - pw - w / 20, h / 12, h / 3);
            let k = (t as f32 * 0.21).sin() * 0.5 + 0.5;
            for y in py..(py + ph).min(h) {
                for x in px..px + pw {
                    let base = 90.0 + 60.0 * (x - px) as f32 / pw as f32;
                    f.y[y * w + x] = (base + 70.0 * k) as u8;
                }
            }
        }
        // "text": rows of glyph strokes in the lower third.
        let ty0 = h * 2 / 3;
        let gh = (h / 24).max(6);
        let mut x = w / 12;
        let mut line = 0;
        let mut k = 0u32;
        while line < 3 {
            let gw = 3 + (hash(k) % ((gh as u32 * 2 / 3).max(2))) as usize;
            let y0 = ty0 + line * gh * 3 / 2;
            if x + gw >= w * 11 / 12 {
                x = w / 12;
                line += 1;
                continue;
            }
            if hash(k * 3 + 1) % 7 != 0 && y0 + gh < h {
                let pat = hash(k * 5 + 2);
                for yy in 0..gh {
                    for xx in 0..gw {
                        let stem = xx < 2 || (pat & 1 != 0 && xx + 2 >= gw);
                        let bar = (yy < 2 && pat & 2 != 0) || (yy + 2 >= gh && pat & 4 != 0) || (yy == gh / 2 && pat & 8 != 0);
                        if stem || bar {
                            f.y[(y0 + yy) * w + x + xx] = 235;
                        } else if xx < 3 || yy < 3 || xx + 3 >= gw || yy + 3 >= gh {
                            let o = (y0 + yy) * w + x + xx;
                            f.y[o] = ((f.y[o] as u32 * 2 + 235) / 3) as u8;
                        }
                    }
                }
            }
            x += gw + 2;
            k += 1;
        }
        for (cx, cy, cw_, ch_, l, u, v) in self.cards_f(t) {
            let (x0, x1) = (cx.floor() as i32, (cx + cw_).ceil() as i32);
            let (y0, y1) = (cy as i32, (cy + ch_) as i32);
            for y in y0.max(0)..y1.min(h as i32) {
                for x in x0.max(0)..x1.min(w as i32) {
                    let xf = x as f32;
                    let cov = ((xf + 1.0 - cx).min(1.0) * (cx + cw_ - xf).min(1.0)).clamp(0.0, 1.0);
                    let dy = (y - y0).min(y1 - 1 - y);
                    let dx = (xf - cx).min(cx + cw_ - 1.0 - xf);
                    if dx + (dy as f32) < 2.0 {
                        continue;
                    }
                    let i = y as usize * w + x as usize;
                    let fill = l.saturating_add(((y - y0) / 3) as u8 % 50) as f32;
                    f.y[i] = (f.y[i] as f32 * (1.0 - cov) + fill * cov + 0.5) as u8;
                    if x % 2 == 0 && y % 2 == 0 && cov > 0.5 {
                        let ci = (y as usize / 2) * cw + x as usize / 2;
                        if ci < f.u.len() {
                            f.u[ci] = u;
                            f.v[ci] = v;
                        }
                    }
                }
            }
        }
        f
    }
}

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
        Scene { width, height, cards: 3, speed: 4, moving_bg: false }
    }

    /// Card rectangles at frame t: (x, y, w, h, luma, u, v).
    pub fn cards_at(&self, t: u32) -> Vec<(i32, i32, i32, i32, u8, u8, u8)> {
        let (w, h) = (self.width as i32, self.height as i32);
        (0..self.cards)
            .map(|i| {
                let cw = (w / 5).max(8) & !1;
                let ch = (h / 6).max(8) & !1;
                let span = (w + cw).max(1);
                let x = ((i as i32 * w / 3 + self.speed * t as i32 * (1 + i as i32 % 2)) % span) - cw;
                let y = (h / 8 + i as i32 * h / 4) % (h - ch).max(1);
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
                        }
                    }
                }
            }
            x += gw + 2;
            k += 1;
        }
        for (cx, cy, cw_, ch_, l, u, v) in self.cards_at(t) {
            for y in cy.max(0)..(cy + ch_).min(h as i32) {
                for x in cx.max(0)..(cx + cw_).min(w as i32) {
                    // rounded-ish corners
                    let dx = (x - cx).min(cx + cw_ - 1 - x);
                    let dy = (y - cy).min(cy + ch_ - 1 - y);
                    if dx + dy < 3 {
                        continue;
                    }
                    f.y[y as usize * w + x as usize] = l.saturating_add(((y - cy) / 2) as u8 % 40);
                    if x % 2 == 0 && y % 2 == 0 {
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

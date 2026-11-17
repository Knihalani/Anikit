//! `analyze --sheet`: one key frame per shot with its number, time, length,
//! incoming transition and palette, plus a timeline strip of shots and beats.

use super::image_ops::hex_to_rgb;
use super::Analysis;

/// 3×5 glyphs, rows top to bottom, 3 bits each (MSB = left).
fn glyph(c: char) -> [u8; 5] {
    match c.to_ascii_uppercase() {
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [7, 1, 7, 4, 7],
        '3' => [7, 1, 7, 1, 7],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 7, 1, 7],
        '6' => [7, 4, 7, 5, 7],
        '7' => [7, 1, 1, 1, 1],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 7],
        'A' => [2, 5, 7, 5, 5],
        'B' => [6, 5, 6, 5, 6],
        'C' => [7, 4, 4, 4, 7],
        'D' => [6, 5, 5, 5, 6],
        'E' => [7, 4, 6, 4, 7],
        'F' => [7, 4, 6, 4, 4],
        'G' => [7, 4, 5, 5, 7],
        'H' => [5, 5, 7, 5, 5],
        'I' => [7, 2, 2, 2, 7],
        'J' => [1, 1, 1, 5, 7],
        'K' => [5, 5, 6, 5, 5],
        'L' => [4, 4, 4, 4, 7],
        'M' => [5, 7, 7, 5, 5],
        'N' => [6, 5, 5, 5, 5],
        'O' => [7, 5, 5, 5, 7],
        'P' => [7, 5, 7, 4, 4],
        'Q' => [7, 5, 5, 7, 1],
        'R' => [6, 5, 6, 5, 5],
        'S' => [7, 4, 7, 1, 7],
        'T' => [7, 2, 2, 2, 2],
        'U' => [5, 5, 5, 5, 7],
        'V' => [5, 5, 5, 5, 2],
        'W' => [5, 5, 7, 7, 5],
        'X' => [5, 5, 2, 5, 5],
        'Y' => [5, 5, 2, 2, 2],
        'Z' => [7, 1, 2, 4, 7],
        '.' => [0, 0, 0, 0, 2],
        ':' => [0, 2, 0, 2, 0],
        '-' => [0, 0, 7, 0, 0],
        '+' => [0, 2, 7, 2, 0],
        '/' => [1, 1, 2, 4, 4],
        _ => [0, 0, 0, 0, 0],
    }
}

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Canvas {
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
        for yy in y.max(0)..(y + h).min(self.h as i32) {
            for xx in x.max(0)..(x + w).min(self.w as i32) {
                let i = (yy as usize * self.w + xx as usize) * 3;
                self.px[i..i + 3].copy_from_slice(&c);
            }
        }
    }
    fn text(&mut self, x: i32, y: i32, s: &str, scale: i32, c: [u8; 3]) {
        let tw = s.chars().count() as i32 * 4 * scale;
        self.rect(x - scale, y - scale, tw + scale, 7 * scale, [0, 0, 0]);
        for (k, ch) in s.chars().enumerate() {
            let g = glyph(ch);
            for (r, bits) in g.iter().enumerate() {
                for b in 0..3 {
                    if bits >> (2 - b) & 1 == 1 {
                        self.rect(x + (k as i32 * 4 + b) * scale, y + r as i32 * scale, scale, scale, c);
                    }
                }
            }
        }
    }
}

fn transition_colour(kind: &str) -> [u8; 3] {
    match kind {
        "crossfade" => [80, 160, 255],
        "fadeBlack" => [120, 120, 120],
        "fadeWhite" => [235, 235, 235],
        "flash" => [255, 220, 40],
        "whip" => [255, 70, 70],
        "zoom" => [190, 110, 255],
        "wipe" => [60, 220, 120],
        "start" => [200, 200, 200],
        _ => [255, 255, 255],
    }
}

/// Render the contact sheet (RGB) for `a`.
pub fn contact_sheet(a: &Analysis) -> (u32, u32, Vec<u8>) {
    let n = a.shots.len().max(1);
    let cols = if n <= 4 { n } else if a.global.width < a.global.height { 6 } else { 4 };
    let cw = if a.global.width < a.global.height { 220 } else { 360 };
    let ch = (cw as f64 * a.global.height as f64 / a.global.width as f64).round() as usize;
    let sw = 18;
    let pad = 6;
    let rows = n.div_ceil(cols);
    let cell_h = ch + sw + pad;
    let tl_h = 56;
    let w = cols * (cw + pad) + pad;
    let h = rows * cell_h + pad + tl_h + pad;
    let mut c = Canvas { w, h, px: vec![18; w * h * 3] };
    for (i, s) in a.shots.iter().enumerate() {
        let (gx, gy) = (pad + (i % cols) * (cw + pad), pad + (i / cols) * cell_h);
        // Border in the colour of the incoming transition.
        c.rect(gx as i32 - 3, gy as i32 - 3, cw as i32 + 6, (ch + sw) as i32 + 6, transition_colour(&s.transition_in));
        if let Some(k) = a.keyframes.get(i) {
            for y in 0..ch {
                let sy = y * k.h / ch;
                for x in 0..cw {
                    let sx = x * k.w / cw;
                    let si = (sy * k.w + sx) * 3;
                    let di = ((gy + y) * w + gx + x) * 3;
                    c.px[di..di + 3].copy_from_slice(&k.rgb[si..si + 3]);
                }
            }
        }
        // Palette swatches, width by share.
        let mut x = gx;
        for (j, p) in s.palette.iter().enumerate() {
            let pw = if j + 1 == s.palette.len() { gx + cw - x } else { (p.share * cw as f32).round() as usize };
            c.rect(x as i32, (gy + ch) as i32, pw as i32, sw as i32, hex_to_rgb(&p.hex));
            x += pw;
        }
        // Text-region boxes.
        for t in &s.text {
            let (bx, by, bw, bh) = ((gx as f32 + t.bbox[0] * cw as f32) as i32, (gy as f32 + t.bbox[1] * ch as f32) as i32, (t.bbox[2] * cw as f32) as i32, (t.bbox[3] * ch as f32) as i32);
            let col = [0, 255, 200];
            c.rect(bx, by, bw, 1, col);
            c.rect(bx, by + bh, bw, 1, col);
            c.rect(bx, by, 1, bh, col);
            c.rect(bx + bw, by, 1, bh + 1, col);
        }
        let label = format!("{} {:.2}S +{:.2}", i + 1, s.start, s.duration);
        c.text(gx as i32 + 5, gy as i32 + 5, &label, 2, [255, 255, 255]);
        let tin = if s.transition_in == "start" { String::new() } else { s.transition_in.to_uppercase() };
        let info = format!("{} {}", tin, s.camera.label.replace("handheld/shake", "SHAKE"));
        c.text(gx as i32 + 5, (gy + ch) as i32 - 17, info.trim(), 2, transition_colour(&s.transition_in));
        c.text(gx as i32 + cw as i32 - 4 * 2 * s.technique[0].len() as i32 - 6, gy as i32 + 5, &s.technique[0], 2, [255, 210, 120]);
    }
    // Timeline: shots as their main colour, beats as ticks.
    let ty = h - tl_h - pad + 18;
    let tw = (w - 2 * pad) as f64;
    let dur = a.global.duration.max(1e-6);
    let xat = |t: f64| (pad as f64 + t / dur * tw) as i32;
    for s in &a.shots {
        let col = s.palette.first().map(|p| hex_to_rgb(&p.hex)).unwrap_or([60, 60, 60]);
        c.rect(xat(s.start), ty as i32, (xat(s.end) - xat(s.start)).max(1), 30, col);
        c.rect(xat(s.start), ty as i32 - 4, 2, 38, transition_colour(&s.transition_in));
    }
    if let Some(au) = &a.audio {
        for b in &au.beats {
            c.rect(xat(*b), ty as i32 - 10, 1, 6, [200, 200, 200]);
        }
        for b in &au.downbeats {
            c.rect(xat(*b), ty as i32 - 16, 2, 12, [255, 255, 255]);
        }
    }
    let head = match &a.audio {
        Some(au) if au.bpm > 0.0 => format!("{:.1}S {} SHOTS MEDIAN {:.2}S {} {:.0} BPM", dur, a.shots.len(), a.global.median_shot, a.global.pace, au.bpm),
        _ => format!("{:.1}S {} SHOTS MEDIAN {:.2}S {}", dur, a.shots.len(), a.global.median_shot, a.global.pace),
    };
    c.text(pad as i32 + 2, (h - tl_h - pad) as i32 - 2, &head, 2, [255, 255, 255]);
    (w as u32, h as u32, c.px)
}

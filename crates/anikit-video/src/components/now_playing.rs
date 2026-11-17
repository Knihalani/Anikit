//! `now-playing`: a music player's "Now Playing" screen, laid out like a
//! real phone app on a 390 pt wide grid (status bar, Dynamic Island, cover
//! art, serif track title, live scrubber with real timecodes, transport
//! controls, volume and the bottom action row).
//!
//! The cover is drawn, not a placeholder: a printed two-colour sleeve (a
//! low sun behind blind slats, with paper grain). Meant as the screen of a
//! `product-shot` (render it in a composition and pass the composition name
//! as the shot's `screen`), but it works flat in any box.

use super::style::*;
use super::text::{draw, Face};
use super::uikit::{self as ui, ts};
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{self, Op, Rgba, IDENTITY};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Track length, seconds.
    pub length: f32,
    /// Playhead at local time 0, seconds.
    pub position: f32,
    /// Status-bar clock.
    pub clock: String,
    /// Cover field colour and sun colour.
    pub cover: Option<String>,
    pub sun: Option<String>,
    #[serde(skip)]
    cov: Rgba,
    #[serde(skip)]
    sunc: Rgba,
}

impl Default for NowPlaying {
    fn default() -> Self {
        NowPlaying {
            title: "Salt & Glass".into(),
            artist: "Maren Holt".into(),
            album: "Low Tide Hours".into(),
            length: 243.0,
            position: 106.0,
            clock: "17:42".into(),
            cover: None,
            sun: None,
            cov: Rgba([0.0; 4]),
            sunc: Rgba([0.0; 4]),
        }
    }
}

impl Prepare for NowPlaying {
    fn prepare(&mut self) -> Result<(), Error> {
        let col = |s: &Option<String>, d: &str, k: &str| -> Result<Rgba, Error> {
            parse_color(s.as_deref().unwrap_or(d)).ok_or_else(|| Error(format!("`{k}`: not a colour")))
        };
        self.cov = col(&self.cover, "#1d3ec9", "cover")?;
        self.sunc = col(&self.sun, "#ff5b2e", "sun")?;
        if self.length <= 0.0 {
            return Err(Error("`length` must be > 0 seconds".into()));
        }
        Ok(())
    }
}

fn hx(s: &str) -> Rgba {
    parse_color(s).unwrap_or(Rgba([1.0, 0.0, 1.0, 1.0]))
}

fn mmss(s: f32) -> String {
    let s = s.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

impl Component for NowPlaying {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let u = w / 390.0;
        let a = ctx.assets;
        let mut o = Vec::new();
        let ink = hx("#0b0d12");
        let paper = hx("#f3eee4");
        let mute = hx("#8d93a1");
        let faint = Rgba([1.0, 1.0, 1.0, 0.16]);
        // Background: ink, warmed at the top by the cover.
        o.push(rect(0.0, 0.0, w, h, 0.0, solid(ink)));
        o.push(rect(0.0, 0.0, w, h * 0.62, 0.0, vgrad(0.0, h * 0.62, mix(ink, self.cov, 0.34), ink)));

        // Status bar + Dynamic Island.
        let sb = ts(Face::Sans, 17.0 * u, 600, paper);
        draw(&mut o, a, &self.clock, &sb, 51.0 * u, 37.0 * u, 0.5);
        o.push(rect(w * 0.5 - 63.0 * u, 11.0 * u, 126.0 * u, 37.0 * u, 18.5 * u, solid(Rgba([0.0, 0.0, 0.0, 1.0]))));
        let (bx, by) = (w - 52.0 * u, 26.0 * u);
        for i in 0..4 {
            let bh = (4.0 + 2.6 * i as f32) * u;
            o.push(rect(bx - 44.0 * u + i as f32 * 5.0 * u, by + 10.5 * u - bh, 3.2 * u, bh, 0.8 * u, solid(paper)));
        }
        o.push(Op::Stroke { path: rrect_path(bx, by - 1.0 * u, 25.0 * u, 12.0 * u, 3.6 * u), paint: solid(Rgba([1.0, 1.0, 1.0, 0.4])), stroke: line(1.0 * u, [0.0, 1.0]), transform: IDENTITY });
        o.push(rect(bx + 2.0 * u, by + 1.0 * u, 17.0 * u, 8.0 * u, 2.0 * u, solid(paper)));
        o.push(rect(bx + 26.5 * u, by + 3.0 * u, 1.6 * u, 4.0 * u, 0.8 * u, solid(Rgba([1.0, 1.0, 1.0, 0.4]))));

        // Header: collapse, "Playing from album", more.
        ui::icon(&mut o, "chevron-down", 22.0 * u, 66.0 * u, 26.0 * u, paper, 2.0);
        let cap = ts(Face::Sans, 11.0 * u, 600, mute).tracking(0.08);
        draw(&mut o, a, "PLAYING FROM ALBUM", &cap, w * 0.5, 75.0 * u, 0.5);
        draw(&mut o, a, &self.album, &ts(Face::Sans, 14.0 * u, 600, paper), w * 0.5, 94.0 * u, 0.5);
        ui::icon(&mut o, "more", w - 48.0 * u, 66.0 * u, 26.0 * u, paper, 2.0);

        // Cover art.
        let (ax, ay, aw) = (24.0 * u, 122.0 * u, 342.0 * u);
        shadow(&mut o, ax, ay, aw, aw, 8.0 * u, 18.0 * u, 36.0 * u, 0.6);
        self.cover_art(&mut o, ctx, t, ax, ay, aw);

        // Title, artist, like.
        let ty = ay + aw + 52.0 * u;
        let tst = ts(Face::Serif, 34.0 * u, 400, paper);
        ui::fit(&mut o, a, &self.title, &tst, 24.0 * u, ty, 290.0 * u, 0.0);
        draw(&mut o, a, &self.artist, &ts(Face::Sans, 17.0 * u, 500, mute), 24.0 * u, ty + 27.0 * u, 0.0);
        ui::icon_filled(&mut o, "heart", w - 52.0 * u, ty - 22.0 * u, 26.0 * u, self.sunc);

        // Scrubber (real timecodes, moves with the clock).
        let pos = (self.position + t).min(self.length);
        let k = pos / self.length;
        let (sx, sw, sy) = (24.0 * u, 342.0 * u, ty + 62.0 * u);
        o.push(rect(sx, sy, sw, 4.0 * u, 2.0 * u, solid(faint)));
        o.push(rect(sx, sy, sw * k, 4.0 * u, 2.0 * u, solid(paper)));
        o.push(circle(sx + sw * k, sy + 2.0 * u, 6.0 * u, solid(paper)));
        let tc = ts(Face::Sans, 12.0 * u, 600, mute).tabular();
        draw(&mut o, a, &mmss(pos), &tc, sx, sy + 24.0 * u, 0.0);
        draw(&mut o, a, &format!("-{}", mmss(self.length - pos)), &tc, sx + sw, sy + 24.0 * u, 1.0);

        // Transport.
        let cy = sy + 82.0 * u;
        ui::icon(&mut o, "shuffle", 30.0 * u, cy - 11.0 * u, 22.0 * u, mute, 2.0);
        ui::icon_filled(&mut o, "skip-back", w * 0.5 - 104.0 * u, cy - 17.0 * u, 34.0 * u, paper);
        ui::icon_filled(&mut o, "pause", w * 0.5 - 26.0 * u, cy - 26.0 * u, 52.0 * u, paper);
        ui::icon_filled(&mut o, "skip-forward", w * 0.5 + 70.0 * u, cy - 17.0 * u, 34.0 * u, paper);
        ui::icon(&mut o, "repeat", w - 52.0 * u, cy - 11.0 * u, 22.0 * u, self.sunc, 2.0);

        // Volume.
        let vy = cy + 70.0 * u;
        ui::icon(&mut o, "volume", 24.0 * u, vy - 9.0 * u, 18.0 * u, mute, 2.0);
        ui::icon(&mut o, "volume-2", w - 42.0 * u, vy - 9.0 * u, 18.0 * u, mute, 2.0);
        let (vx, vw) = (54.0 * u, w - 108.0 * u);
        o.push(rect(vx, vy - 2.0 * u, vw, 4.0 * u, 2.0 * u, solid(faint)));
        o.push(rect(vx, vy - 2.0 * u, vw * 0.62, 4.0 * u, 2.0 * u, solid(Rgba([1.0, 1.0, 1.0, 0.72]))));

        // Bottom actions + home indicator.
        let by2 = h - 82.0 * u;
        ui::icon(&mut o, "quote", w * 0.5 - 112.0 * u, by2, 22.0 * u, mute, 2.0);
        ui::icon(&mut o, "airplay", w * 0.5 - 11.0 * u, by2, 22.0 * u, mute, 2.0);
        ui::icon(&mut o, "list", w * 0.5 + 90.0 * u, by2, 22.0 * u, mute, 2.0);
        o.push(rect(w * 0.5 - 67.0 * u, h - 13.0 * u, 134.0 * u, 5.0 * u, 2.5 * u, solid(paper)));
        o
    }
}

impl NowPlaying {
    /// A printed sleeve: cobalt field, a low sun cut by blind slats, a title
    /// set in the serif, paper grain on top.
    fn cover_art(&self, o: &mut Vec<Op>, ctx: &Ctx, t: f32, x: f32, y: f32, s: f32) {
        let a = ctx.assets;
        let clip = rrect_path(x, y, s, s, 8.0 * s / 342.0);
        o.push(Op::PushLayer { opacity: 1.0, blend: dl::Blend::Normal, filters: Vec::new(), clip: Some((clip, IDENTITY)), cache_key: None });
        let field = self.cov;
        o.push(rect(x, y, s, s, 0.0, vgrad(y, y + s, shade(field, -0.06), shade(field, 0.05))));
        // Sun: slow drift down the sleeve (motion artwork).
        let (cx, cy, r) = (x + s * 0.58, y + s * (0.5 + 0.012 * (t * 0.5).sin()), s * 0.3);
        o.push(circle(cx, cy, r, radial(cx, cy - r * 0.3, r * 1.2, vec![(0.0, mix(self.sunc, hx("#ffc59a"), 0.45)), (0.7, self.sunc), (1.0, shade(self.sunc, -0.08))])));
        // Slats across the sun, thickening downward.
        for i in 0..11 {
            let fy = y + s * (0.24 + i as f32 * 0.058);
            let th = s * (0.006 + 0.0035 * i as f32);
            o.push(rect(x, fy, s, th, 0.0, solid(field)));
        }
        // Horizon: the lower third is sea, a deeper cobalt.
        o.push(rect(x, y + s * 0.74, s, s * 0.26, 0.0, solid(shade(field, -0.2))));
        o.push(rect(x, y + s * 0.74, s, s * 0.004, 0.0, solid(Rgba([1.0, 0.86, 0.75, 0.55]))));
        // Typography.
        let cream = hx("#f3eee4");
        draw(o, a, &self.artist.to_uppercase(), &ts(Face::Sans, s * 0.034, 700, cream).tracking(0.22), x + s * 0.065, y + s * 0.1, 0.0);
        draw(o, a, &self.album, &ts(Face::Serif, s * 0.115, 400, cream), x + s * 0.06, y + s * 0.915, 0.0);
        draw(o, a, "LP · 2026", &ts(Face::Sans, s * 0.028, 600, Rgba([0.95, 0.93, 0.89, 0.7])).tracking(0.18), x + s * 0.935, y + s * 0.905, 1.0);
        // Paper grain (static, made once).
        let g = 512u32;
        let id = a.image(fnv(&[b"np-grain-v1"]), g, g, &mut || {
            let mut v = vec![0u8; (g * g * 4) as usize];
            for (i, px) in v.chunks_exact_mut(4).enumerate() {
                let n = hash01((i as u32 * 2654435761) ^ 0x9e37);
                let l = if n > 0.5 { 255 } else { 0 };
                px.copy_from_slice(&[l, l, l, (((n - 0.5).abs() * 2.0).powf(3.0) * 46.0) as u8]);
            }
            v
        });
        let k = s / 512.0;
        o.push(Op::Fill { path: rrect_path(x, y, s, s, 0.0), paint: dl::Paint::Image { image: id, transform: [1.0 / k, 0.0, 0.0, 1.0 / k, -x / k, -y / k], opacity: 1.0 }, rule: dl::FillRule::NonZero, transform: IDENTITY });
        o.push(Op::PopLayer);
    }
}

//! Ken Burns: a photo with a slow zoom and pan, optional parallax layers
//! (cut-outs that move at different rates), a vignette and a gentle film
//! grade. Without a `src` it pans over a generated landscape placeholder.

use super::fxkit;
use super::fxpix;
use super::style::*;
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Op, Paint, Path, Rgba};
use serde::Deserialize;

/// A parallax layer: an image (usually a cut-out with alpha) and how much
/// more than the photo it moves (`0` = with the photo, `1` = twice as much).
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub src: String,
    #[serde(default = "half")]
    pub depth: f32,
}
fn half() -> f32 {
    0.5
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KenBurns {
    pub src: Option<String>,
    /// Preset: `in`, `out`, `left`, `right`, `up` or `down`.
    pub motion: String,
    /// Start/end view as `[x, y, w, h]` fractions of the image (override `motion`).
    pub from: Option<[f32; 4]>,
    pub to: Option<[f32; 4]>,
    /// Zoom amount for presets (1.2 = 20 % closer).
    pub zoom: f32,
    /// Seconds for the whole move.
    pub duration: f32,
    pub delay: f32,
    pub layers: Vec<Layer>,
    /// Corner darkening 0..1.
    pub vignette: f32,
    /// Warm/teal grade and lifted blacks 0..1.
    pub film: f32,
    /// Placeholder colours (sky top → horizon) when there is no `src`.
    pub colors: Vec<Color>,
    #[serde(skip)]
    cam: [(f32, f32, f32); 2],
}

impl Default for KenBurns {
    fn default() -> Self {
        KenBurns {
            src: None,
            motion: "in".into(),
            from: None,
            to: None,
            zoom: 1.2,
            duration: 8.0,
            delay: 0.0,
            layers: Vec::new(),
            vignette: 0.4,
            film: 0.3,
            colors: Vec::new(),
            cam: [(1.0, 0.5, 0.5); 2],
        }
    }
}

const MOTIONS: [&str; 6] = ["in", "out", "left", "right", "up", "down"];

impl Prepare for KenBurns {
    fn prepare(&mut self) -> Result<(), Error> {
        if !(self.zoom.is_finite() && self.zoom > 0.0 && self.duration.is_finite() && self.delay.is_finite()) {
            return Err(Error("`zoom` must be > 0 and `duration`/`delay` finite".into()));
        }
        let z = self.zoom.max(1.0);
        // Pans run along the slack a zoom of `z` leaves, with a little zoom-in for life.
        let cam = match self.motion.as_str() {
            "in" => [(1.0, 0.5, 0.5), (z, 0.5, 0.47)],
            "out" => [(z, 0.5, 0.47), (1.0, 0.5, 0.5)],
            "left" => [(z, 1.0, 0.5), (z * 1.04, 0.0, 0.5)],
            "right" => [(z, 0.0, 0.5), (z * 1.04, 1.0, 0.5)],
            "up" => [(z, 0.5, 1.0), (z * 1.04, 0.5, 0.0)],
            "down" => [(z, 0.5, 0.0), (z * 1.04, 0.5, 1.0)],
            m => {
                let hint = closest(m, &MOTIONS).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                return Err(Error(format!("motion must be one of {}, not {m:?}{hint}", MOTIONS.join(", "))));
            }
        };
        self.cam = cam;
        // Explicit rects: the camera that just contains the rect (at ≥ 1× cover).
        for (i, r) in [self.from, self.to].into_iter().enumerate() {
            if let Some([x, y, w, h]) = r {
                if !(w > 0.0 && h > 0.0) || [x, y, w, h].iter().any(|v| !v.is_finite()) {
                    return Err(Error("`from`/`to` are [x, y, w, h] fractions with w, h > 0".into()));
                }
                // Scale relative to cover: 1/max(w, h) is exact for a box matching the image aspect.
                self.cam[i] = ((1.0 / w.max(h)).max(1.0), x + w * 0.5, y + h * 0.5);
            }
        }
        Ok(())
    }
}

impl KenBurns {
    /// Camera at time `t`: (scale over cover, centre x, centre y as fractions).
    fn camera(&self, t: f32) -> (f32, f32, f32) {
        let p = seg(t, self.delay, self.duration);
        // Mostly linear (Ken Burns never stops), softened at both ends.
        let e = 0.55 * p + 0.45 * (0.5 - 0.5 * (std::f32::consts::PI * p).cos());
        let ([s0, x0, y0], [s1, x1, y1]) = ([self.cam[0].0, self.cam[0].1, self.cam[0].2], [self.cam[1].0, self.cam[1].1, self.cam[1].2]);
        (s0 * (s1 / s0).powf(e), lerp(x0, x1, e), lerp(y0, y1, e))
    }
}

/// Destination rect of an `iw × ih` image in a `w × h` box for camera
/// `(s, cx, cy)` seen from parallax depth `d`.
fn place(iw: f32, ih: f32, w: f32, h: f32, (s, cx, cy): (f32, f32, f32), d: f32) -> [f32; 4] {
    let k0 = (w / iw).max(h / ih);
    // Deeper layers zoom and pan more; a little overscan hides their edges.
    let sd = (1.0 + (s - 1.0) * (1.0 + d)) * (1.0 + 0.08 * d);
    let k = k0 * sd;
    let (vw, vh) = (w / k, h / k);
    let (mut px, mut py) = (cx * iw, cy * ih);
    if d != 0.0 {
        px = iw * 0.5 + (px - iw * 0.5) * (1.0 + d);
        py = ih * 0.5 + (py - ih * 0.5) * (1.0 + d);
    }
    let px = px.clamp(vw * 0.5, (iw - vw * 0.5).max(vw * 0.5));
    let py = py.clamp(vh * 0.5, (ih - vh * 0.5).max(vh * 0.5));
    [w * 0.5 - px * k, h * 0.5 - py * k, iw * k, ih * k]
}

impl Component for KenBurns {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let cam = self.camera(t);
        let mut o = Vec::with_capacity(16);
        // Images fill the box through an image paint (no clip layer needed).
        let img = |id, iw: u32, ih: u32, d: f32| {
            let [x, y, dw, _] = place(iw as f32, ih as f32, w, h, cam, d);
            let k = dw / iw as f32;
            rect(0.0, 0.0, w, h, 0.0, Paint::Image { image: id, transform: [1.0 / k, 0.0, 0.0, 1.0 / k, -x / k, -y / k], opacity: 1.0 })
        };
        match self.src.as_deref().and_then(|s| ctx.assets.media(s, t)) {
            Some((id, iw, ih)) => o.push(img(id, iw, ih, 0.0)),
            None => {
                push_clip(&mut o, 0.0, 0.0, w, h, 0.0);
                self.placeholder(&mut o, cam, ctx);
                o.push(Op::PopLayer);
            }
        }
        for l in &self.layers {
            if let Some((id, iw, ih)) = ctx.assets.media(&l.src, t) {
                o.push(img(id, iw, ih, l.depth.clamp(0.0, 3.0)));
            }
        }
        // Film look in one cheap overlay: warm highlights top-left, lifted
        // blacks through the middle, teal shadows bottom-right; then the vignette.
        let (film, vig) = self.overlays(w, h);
        if fxkit::exact() {
            o.extend(film.map(|p| rect(0.0, 0.0, w, h, 0.0, p)));
            o.extend(vig.map(|p| rect(0.0, 0.0, w, h, 0.0, p)));
        } else if film.is_some() || vig.is_some() {
            // Shortcut: both are static, so they are baked once into one
            // image (vignette over film, as drawn) and blitted.
            o.push(overlay_image(ctx, film, vig));
        }
        o
    }

    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.duration)
    }
}

impl KenBurns {
    /// The film grade and vignette paints over a `w × h` box.
    fn overlays(&self, w: f32, h: f32) -> (Option<Paint>, Option<Paint>) {
        let f = clamp01(self.film);
        let film = (f > 0.0).then(|| Paint::Linear { p0: [0.0, 0.0], p1: [w * 0.45, h], stops: vec![(0.0, Rgba([1.0, 0.7, 0.42, 0.16 * f])), (0.5, Rgba([0.16, 0.13, 0.17, 0.1 * f])), (1.0, Rgba([0.04, 0.3, 0.36, 0.2 * f]))] });
        let v = clamp01(self.vignette);
        let r = (w * w + h * h).sqrt() * 0.5;
        let vig = (v > 0.0).then(|| radial(w * 0.5, h * 0.5, r, vec![(0.45, Rgba([0.0, 0.0, 0.0, 0.0])), (0.8, Rgba([0.0, 0.0, 0.0, 0.45 * v])), (1.0, Rgba([0.0, 0.0, 0.0, v]))]));
        (film, vig)
    }

    /// A generated dusk landscape, drawn as if it were a `w × h` image.
    fn placeholder(&self, o: &mut Vec<Op>, cam: (f32, f32, f32), ctx: &Ctx) {
        let (w, h) = (ctx.width, ctx.height);
        let th = ctx.theme;
        let [x0, y0, dw, _] = place(w, h, w, h, cam, 0.0);
        let k = dw / w;
        let m = [k, 0.0, 0.0, k, x0, y0];
        let (top, horizon) = match self.colors.as_slice() {
            [a, b, ..] => (a.0, b.0),
            [a] => (a.0, shade(a.0, 0.5)),
            [] => (shade(th.accent, -0.72), mix(th.accent, Rgba([1.0, 0.85, 0.7, 1.0]), 0.55)),
        };
        let mut s = Vec::with_capacity(8);
        s.push(rect(0.0, 0.0, w, h, 0.0, Paint::Linear { p0: [0.0, 0.0], p1: [0.0, h * 0.68], stops: vec![(0.0, top), (0.6, mix(top, horizon, 0.45)), (1.0, horizon)] }));
        // Sun and its haze.
        let (sx, sy) = (w * 0.62, h * 0.56);
        s.push(circle(sx, sy, h * 0.5, radial(sx, sy, h * 0.5, vec![(0.0, Rgba([1.0, 0.9, 0.75, 0.55])), (1.0, Rgba([1.0, 0.8, 0.6, 0.0]))])));
        s.push(circle(sx, sy, h * 0.075, solid(Rgba([1.0, 0.95, 0.86, 0.95]))));
        // Three ridges, receding into haze.
        for (i, (base, amp, dark)) in [(0.6, 0.08, 0.35), (0.7, 0.07, 0.6), (0.82, 0.06, 0.82)].into_iter().enumerate() {
            let n = 14;
            let mut p = Path::new();
            p.move_to((0.0, h as f64));
            let pts: Vec<(f32, f32)> = (0..=n)
                .map(|j| {
                    let u = j as f32 / n as f32;
                    let y = h * base - h * amp * ((u * (5.0 + i as f32 * 2.3) + i as f32 * 1.7).sin() * 0.6 + (u * 13.0 + i as f32).sin() * 0.25 + 0.5);
                    (u * w, y)
                })
                .collect();
            let ridge = smooth_path(&pts, 1.0);
            for el in ridge.elements() {
                match el {
                    kurbo::PathEl::MoveTo(a) => p.line_to(*a),
                    e => p.push(*e),
                }
            }
            p.line_to((w as f64, h as f64));
            p.close_path();
            let c = mix(horizon, shade(top, -0.6), dark);
            s.push(fill(p, Paint::Linear { p0: [0.0, h * (base - amp)], p1: [0.0, h], stops: vec![(0.0, c), (1.0, shade(c, -0.4))] }));
        }
        super::transform_ops(&mut s, &m);
        o.extend(s);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::{build, preview, Theme};
    use serde_json::json;

    /// A test photo: diagonal colour field with a grid, so motion is visible.
    pub fn photo(w: u32, h: u32) -> Vec<u8> {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
                let line = (x % 80 < 3 || y % 80 < 3) as u8 as f32;
                let r = 40.0 + 180.0 * u + 30.0 * line;
                let g = 60.0 + 120.0 * (1.0 - v) + 30.0 * line;
                let b = 110.0 + 120.0 * v * (1.0 - u) + 30.0 * line;
                px.extend_from_slice(&[r.min(255.0) as u8, g.min(255.0) as u8, b.min(255.0) as u8, 255]);
            }
        }
        px
    }

    #[test]
    fn gallery() {
        let th = Theme::default();
        let a = preview::host();
        a.add_media("kb-photo.png", 1200, 800, photo(1200, 800));
        // A cut-out circle for parallax.
        let (fw, fh) = (1200u32, 800u32);
        let mut px = Vec::new();
        for y in 0..fh {
            for x in 0..fw {
                let (dx, dy) = (x as f32 - 850.0, y as f32 - 640.0);
                let a = (1.0 - ((dx * dx + dy * dy).sqrt() - 200.0)).clamp(0.0, 1.0);
                px.extend_from_slice(&[255, 220, 120, (a * 255.0) as u8]);
            }
        }
        a.add_media("kb-fg.png", fw, fh, px);
        for (name, p, t) in [
            ("ken-burns-placeholder", json!({}), 4.0),
            ("ken-burns-photo", json!({"src": "kb-photo.png", "motion": "left", "layers": [{"src": "kb-fg.png", "depth": 1}]}), 6.0),
            ("ken-burns-rects", json!({"src": "kb-photo.png", "from": [0, 0, 1, 1], "to": [0.5, 0.4, 0.3, 0.3]}), 8.0),
        ] {
            let c = build("ken-burns", &p).unwrap();
            for tt in [0.0, t * 0.5, t, 20.0] {
                preview::validate(&c.eval(tt, 960.0, 540.0, 30.0, &th, a)).unwrap();
            }
            preview::snapshot(name, 960, 540, th.background, c.eval(t, 960.0, 540.0, 30.0, &th, a), a);
        }
        assert!(build("ken-burns", &json!({"motion": "lft"})).err().unwrap().0.contains("did you mean `left`"));
    }

    #[test]
    fn camera_stays_inside_the_image() {
        for m in super::MOTIONS {
            assert!(build("ken-burns", &json!({"motion": m})).is_ok());
        }
        // Cover-fit plus clamping: the image always covers the box.
        for (s, cx, cy) in [(1.0, 0.0, 0.0), (1.3, 1.0, 1.0), (1.1, 0.5, 0.2)] {
            let [x, y, w, h] = super::place(1200.0, 800.0, 960.0, 540.0, (s, cx, cy), 0.0);
            assert!(x <= 0.01 && y <= 0.01 && x + w >= 959.99 && y + h >= 539.99, "{x} {y} {w} {h}");
        }
    }
}

/// The film and vignette overlays evaluated per pixel (gradients in
/// premultiplied sRGB at pixel centres, as the renderer draws them),
/// composited and cached as one box-sized image.
fn overlay_image(ctx: &Ctx, film: Option<Paint>, vig: Option<Paint>) -> Op {
    let (iw, ih) = (ctx.width.ceil().max(1.0) as usize, ctx.height.ceil().max(1.0) as usize);
    let mut hs = fxpix::Hash::new();
    hs.bytes(b"ken-burns-overlay");
    hs.f32s(&[ctx.width, ctx.height]);
    for p in [&film, &vig] {
        let (geo, stops): (Vec<f32>, &[(f32, Rgba)]) = match p {
            Some(Paint::Linear { p0, p1, stops }) => (vec![1.0, p0[0], p0[1], p1[0], p1[1]], stops),
            Some(Paint::Radial { c, r, stops }) => (vec![2.0, c[0], c[1], *r], stops),
            _ => (vec![0.0], &[]),
        };
        hs.f32s(&geo);
        for (o, c) in stops {
            hs.f32s(&[*o]);
            hs.f32s(&c.0);
        }
    }
    let at = |p: &Option<Paint>, x: f32, y: f32| -> [f32; 4] {
        match p {
            Some(Paint::Linear { p0, p1, stops }) => {
                let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
                let t = ((x - p0[0]) * dx + (y - p0[1]) * dy) / (dx * dx + dy * dy).max(1e-6);
                fxpix::stops_at(stops, t)
            }
            Some(Paint::Radial { c, r, stops }) => {
                let t = ((x - c[0]) * (x - c[0]) + (y - c[1]) * (y - c[1])).sqrt() / r.max(1e-3);
                fxpix::stops_at(stops, t)
            }
            _ => [0.0; 4],
        }
    };
    let id = ctx.assets.image(hs.finish(), iw as u32, ih as u32, &mut || {
        let mut px = vec![0u8; iw * ih * 4];
        for (j, row) in px.chunks_exact_mut(iw * 4).enumerate() {
            let y = j as f32 + 0.5;
            for (i, o) in row.chunks_exact_mut(4).enumerate() {
                let x = i as f32 + 0.5;
                let c = fxpix::over(at(&vig, x, y), at(&film, x, y));
                let a = c[3].clamp(0.0, 1.0);
                let k = if a > 0.0 { 255.0 / a } else { 0.0 };
                o.copy_from_slice(&[fxpix::byte(c[0] * k), fxpix::byte(c[1] * k), fxpix::byte(c[2] * k), fxpix::byte(a * 255.0)]);
            }
        }
        px
    });
    Op::Image { image: id, dst: [0.0, 0.0, iw as f32, ih as f32], opacity: 1.0, transform: crate::dl::IDENTITY }
}

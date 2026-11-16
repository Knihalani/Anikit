//! `card-stack`: a 2.5D stack of cards (images or titled cards). Cards
//! further back are smaller, higher and darker; the whole stack can be
//! skewed for an isometric feel. Every `interval` seconds the front card
//! swipes away (left, right, up, or alternating) and the stack advances,
//! with the swiped card returning to the back.

use super::style::*;
use super::text::{draw, wrap, Face, TextStyle};
use super::{transform_ops, with_opacity, Component, Ctx, Error, Prepare};
use crate::dl::{self, Op, Paint, Rgba, IDENTITY};
use serde::Deserialize;

/// A card: a string is a title; an object can add `body`, an image `src`,
/// a `color` and an `eyebrow` (default: its number).
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum Card {
    Title(String),
    Full(CardSpec),
}

#[derive(Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct CardSpec {
    pub title: String,
    pub body: String,
    pub src: Option<String>,
    pub color: Option<Color>,
    pub eyebrow: Option<String>,
}

impl Card {
    fn spec(&self) -> CardSpec {
        match self {
            Card::Title(t) => CardSpec { title: t.clone(), ..Default::default() },
            Card::Full(s) => s.clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CardStack {
    pub cards: Vec<Card>,
    /// Seconds per card.
    pub interval: f32,
    /// `left`, `right`, `up` or `alternate`.
    pub swipe: String,
    /// Cards visible behind the front one, plus one.
    pub visible: u32,
    /// Card width / height.
    pub aspect: f32,
    /// Card width as a fraction of the box width (capped by the height).
    pub size: f32,
    /// Vertical step between depths, as a fraction of the card height.
    pub depth: f32,
    /// Darkening per depth step (0..1).
    pub darken: f32,
    /// Skew of the whole stack in degrees (isometric feel).
    pub skew: f32,
    pub radius: Option<f32>,
    /// Keep cycling after the last card (otherwise stop on it).
    #[serde(rename = "loop")]
    pub looping: bool,
    pub delay: f32,
    #[serde(skip)]
    specs: Vec<CardSpec>,
}

impl Default for CardStack {
    fn default() -> Self {
        CardStack {
            cards: Vec::new(),
            interval: 2.2,
            swipe: "left".into(),
            visible: 3,
            aspect: 1.5,
            size: 0.62,
            depth: 0.09,
            darken: 0.22,
            skew: -4.0,
            radius: None,
            looping: true,
            delay: 0.0,
            specs: Vec::new(),
        }
    }
}

impl Prepare for CardStack {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.cards.is_empty() {
            return Err(Error("needs `cards`: titles (\"Design\") or {title, body, src, color}".into()));
        }
        let ok = ["left", "right", "up", "alternate"];
        if !ok.contains(&self.swipe.as_str()) {
            let hint = closest(&self.swipe, &ok).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
            return Err(Error(format!("swipe must be left, right, up or alternate, not {:?}{hint}", self.swipe)));
        }
        if !(self.interval > 0.1 && self.aspect > 0.1 && self.size > 0.0) {
            return Err(Error("`interval` must be > 0.1 s, `aspect` and `size` > 0".into()));
        }
        self.visible = self.visible.clamp(1, 8);
        self.specs = self.cards.iter().map(Card::spec).collect();
        Ok(())
    }
}

/// Ease out with a soft overshoot.
fn settle(x: f32) -> f32 {
    let x = clamp01(x) - 1.0;
    1.0 + 2.2 * x * x * x + 1.2 * x * x
}

impl Component for CardStack {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let at = t - self.delay;
        if at < 0.0 {
            return Vec::new();
        }
        let (bw, bh) = (ctx.width, ctx.height);
        let n = self.specs.len();
        let vis = self.visible as f32;
        // Card geometry, leaving room above for the cards behind.
        let mut cw = bw * self.size;
        let mut ch = cw / self.aspect;
        let room = bh * 0.86 / (1.0 + self.depth * (vis - 1.0));
        if ch > room {
            ch = room;
            cw = ch * self.aspect;
        }
        let step = ch * self.depth;
        let (cx, cy) = (bw * 0.5, bh * 0.5 + step * (vis - 1.0) * 0.5);
        let r = self.radius.unwrap_or(ctx.theme.radius * 1.4).min(ch * 0.2);

        // Which card is in front and how far the current swipe has got.
        let sw = (self.interval * 0.45).min(0.75);
        let mut k = (at / self.interval).floor() as usize;
        let f = at - k as f32 * self.interval;
        let mut p = seg(f, self.interval - sw, sw);
        let last = if self.looping { usize::MAX } else { n.saturating_sub(1) };
        if k >= last {
            k = last;
            p = 0.0;
        }
        let dir = match self.swipe.as_str() {
            "right" => 1.0,
            "alternate" if k % 2 == 1 => 1.0,
            "up" => 0.0,
            _ => -1.0,
        };
        let adv = settle(p * 1.15);

        let entrance = |d: f32| spring(at - (vis - 1.0 - d).max(0.0) * 0.09, 0.9);
        let skew = self.skew.to_radians().tan();
        let stack_m: dl::Affine = [1.0, skew, 0.0, 1.0, 0.0, -skew * cx];

        // (depth for sorting, ops)
        let mut layers: Vec<(f32, Vec<Op>)> = Vec::with_capacity(n.min(self.visible as usize + 2));
        for c in 0..n {
            let d = ((c + n - k % n) % n) as f32;
            let mut ops = Vec::new();
            let (e, m, op, rot) = if d == 0.0 && p > 0.0 {
                // Swiping away.
                let q = ease_in_out(p);
                let (dx, dy, rot) = if dir == 0.0 { (0.0, -(bh * 0.5 + ch) * q, 0.0) } else { (dir * (bw * 0.5 + cw * 0.8) * q, -ch * 0.08 * q, dir * 0.3 * q) };
                let m = dl::mul(&translate(dx, dy), &rotate_about(rot, cx, cy + ch * 0.3));
                (-1.0, m, 1.0 - seg(p, 0.55, 0.45), rot)
            } else {
                // Moving one step forward during the swipe.
                let e = if p > 0.0 { d - adv } else { d };
                if e > vis {
                    continue;
                }
                let s = 1.0 - 0.07 * e;
                let m = dl::mul(&translate(0.0, -e * step), &scale_about(s, cx, cy - ch * 0.5));
                // The back card fades in as it joins the visible stack.
                let fade = clamp01(vis - e);
                // And a card just recycled to the back fades in too.
                let back_in = if d == (n - 1) as f32 && k > 0 { seg(f, 0.0, 0.35) } else { 1.0 };
                let ent = entrance(d);
                let m = dl::mul(&translate(0.0, (1.0 - ent) * ch * 0.35), &m);
                (e, m, fade * back_in * clamp01(ent * 2.0), 0.0)
            };
            if op <= 0.003 {
                continue;
            }
            let (x0, y0) = (cx - cw * 0.5, cy - ch * 0.5);
            let full = dl::mul(&stack_m, &m);
            // The shadow stays axis-aligned (the renderer's cached-sprite fast
            // path), inset so the silhouette never peeks out of a skewed card.
            {
                let s = (full[0] * full[3] - full[1] * full[2]).abs().sqrt();
                let (sx, sy) = (full[0] * cx + full[2] * cy + full[4], full[1] * cx + full[3] * cy + full[5]);
                let (sr, cr) = (rot.sin().abs(), rot.cos().abs());
                let w2 = (cw * cr - ch * sr).max(cw * 0.3) * s;
                let h2 = (ch * cr - cw * sr - skew.abs() * cw).max(ch * 0.3) * s;
                let th = ctx.theme;
                shadow(&mut ops, sx - w2 * 0.5, sy - h2 * 0.5, w2, h2, r * s, ch * 0.06 * s, ch * 0.12 * s, if th.dark { 0.6 } else { 0.24 });
            }
            let mut card = Vec::new();
            self.card(&mut card, ctx, c, x0, y0, cw, ch, r, at);
            let dark = (e.max(0.0) * self.darken).min(0.85);
            if dark > 0.003 {
                card.push(rect(x0, y0, cw, ch, r, solid(Rgba([0.0, 0.0, 0.0, dark]))));
            }
            transform_ops(&mut card, &full);
            ops.extend(card);
            layers.push((e, with_opacity(ops, op)));
        }
        // Back to front; the swiping card (depth −1) is drawn last, on top.
        layers.sort_by(|a, b| {
            let ka = if a.0 < 0.0 { f32::NEG_INFINITY } else { a.0 };
            let kb = if b.0 < 0.0 { f32::NEG_INFINITY } else { b.0 };
            kb.total_cmp(&ka)
        });
        layers.into_iter().flat_map(|(_, o)| o).collect()
    }
}

impl CardStack {
    fn card(&self, o: &mut Vec<Op>, ctx: &Ctx, i: usize, x: f32, y: f32, w: f32, h: f32, r: f32, t: f32) {
        let th = ctx.theme;
        let s = &self.specs[i];
        let base = s.color.map(|c| c.0).unwrap_or(th.pal(i));
        let pad = w * 0.07;
        let white = Rgba([1.0, 1.0, 1.0, 1.0]);
        let media = s.src.as_deref().and_then(|src| ctx.assets.media(src, t));
        if let Some((id, iw, ih)) = media {
            push_clip(o, x, y, w, h, r);
            let k = (w / iw as f32).max(h / ih as f32);
            let (dw, dh) = (iw as f32 * k, ih as f32 * k);
            o.push(Op::Image { image: id, dst: [x + (w - dw) * 0.5, y + (h - dh) * 0.5, dw, dh], opacity: 1.0, transform: IDENTITY });
            if !s.title.is_empty() {
                o.push(rect(x, y + h * 0.45, w, h * 0.55, 0.0, vgrad(y + h * 0.45, y + h, Rgba([0.0, 0.0, 0.0, 0.0]), Rgba([0.0, 0.0, 0.0, 0.7]))));
            }
            o.push(Op::PopLayer);
        } else {
            // Rich gradient face with a soft orb of light.
            o.push(rect(x, y, w, h, r, Paint::Linear { p0: [x, y], p1: [x + w, y + h], stops: vec![(0.0, shade(base, -0.05)), (1.0, shade(mix(base, th.pal(i + 1), 0.35), -0.55))] }));
            push_clip(o, x, y, w, h, r);
            o.push(circle(x + w * 0.85, y + h * 0.1, h * 0.75, radial(x + w * 0.85, y + h * 0.1, h * 0.75, vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.32])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))])));
            // Fine diagonal lines for texture.
            for j in 0..6 {
                let off = w * (0.45 + j as f32 * 0.09);
                o.push(Op::Stroke { path: { let mut p = crate::dl::Path::new(); p.move_to(((x + off) as f64, (y + h) as f64)); p.line_to(((x + off + h * 0.6) as f64, (y + h * 0.4) as f64)); p }, paint: solid(Rgba([1.0, 1.0, 1.0, 0.05])), stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
            }
            o.push(Op::PopLayer);
            let eb = s.eyebrow.clone().unwrap_or_else(|| format!("{:02}", i + 1));
            let es = TextStyle::new(Face::Mono, (h * 0.065).max(10.0), 600, Rgba([1.0, 1.0, 1.0, 0.75])).tracking(0.08);
            draw(o, ctx.assets, &eb, &es, x + pad, y + pad + es.size * 0.8, 0.0);
        }
        // Title and body, bottom-left.
        let ts = TextStyle::new(Face::Display, (h * 0.13).max(14.0), 700, white).tracking(-0.02);
        let bs = TextStyle::new(Face::Sans, (h * 0.055).max(10.0), 450, Rgba([1.0, 1.0, 1.0, 0.78]));
        let body = if s.body.is_empty() { Vec::new() } else { wrap(ctx.assets, &s.body, &bs, w - pad * 2.0).into_iter().take(3).collect() };
        let mut by = y + h - pad - (body.len() as f32 - 1.0).max(0.0) * bs.size * 1.35;
        if !body.is_empty() {
            for (j, l) in body.iter().enumerate() {
                draw(o, ctx.assets, l, &bs, x + pad, by + j as f32 * bs.size * 1.35, 0.0);
            }
            by -= bs.size * 1.6;
        }
        if !s.title.is_empty() {
            draw(o, ctx.assets, &s.title, &ts, x + pad, by, 0.0);
        }
        border(o, x, y, w, h, r, Rgba([1.0, 1.0, 1.0, 0.14]));
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use serde_json::json;

    #[test]
    fn gallery() {
        let th = Theme::default();
        let a = preview::host();
        a.add_media("stack-photo.png", 1200, 800, super::super::ken_burns::tests::photo(1200, 800));
        let p = json!({"cards": [
            {"title": "Design", "body": "Pixel-perfect layouts from a JSON spec."},
            "Build",
            {"title": "Ship", "src": "stack-photo.png"},
            "Repeat"
        ]});
        let c = build("card-stack", &p).unwrap();
        for t in [0.0, 1.0, 2.0, 2.4, 5.0, 30.0] {
            preview::validate(&c.eval(t, 960.0, 540.0, 30.0, &th, a)).unwrap();
        }
        preview::snapshot("card-stack-rest", 960, 540, th.background, c.eval(1.4, 960.0, 540.0, 30.0, &th, a), a);
        preview::snapshot("card-stack-swipe", 960, 540, th.background, c.eval(1.8, 960.0, 540.0, 30.0, &th, a), a);
        preview::snapshot("card-stack-photo", 960, 540, th.background, c.eval(5.6, 960.0, 540.0, 30.0, &th, a), a);
    }

    #[test]
    fn stops_without_loop_and_errors() {
        let th = Theme::default();
        let a = preview::host();
        let c = build("card-stack", &json!({"cards": ["A", "B"], "loop": false})).unwrap();
        assert_eq!(c.eval(50.0, 600.0, 400.0, 30.0, &th, a), c.eval(60.0, 600.0, 400.0, 30.0, &th, a));
        assert!(build("card-stack", &json!({})).is_err());
        assert!(build("card-stack", &json!({"cards": ["A"], "swipe": "lef"})).err().unwrap().0.contains("did you mean `left`"));
    }
}

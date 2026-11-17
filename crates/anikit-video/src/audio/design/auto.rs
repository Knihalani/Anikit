//! Cues from the picture: every layer's on-screen box is sampled each frame
//! (from the display list, so presets, springs, expressions, physics and
//! scripts all count); motion segments become whooshes peaking at the
//! fastest frame, hard stops become landings on the contact frame, pops
//! become pops, text changes become ticks (counters become one ticker), and
//! scene transitions get their own pass. A density budget keeps it sparse:
//! the most salient events win, the rest stay silent.

use super::bus::Cue;
use super::spec::{LayerSfx, SoundPlan};
use super::DesignParams;
use crate::dl::Op;
use crate::eval::{Evaluator, TransKind};
use kurbo::Affine;
use std::collections::HashMap;

/// What happened on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A move (the hit is its fastest frame).
    Move,
    /// A hard stop / landing (the hit is the contact frame).
    Land,
    /// Scale-up from nothing (a pop-in).
    Pop,
    /// Text content changed once.
    Text,
    /// Text changed many times (a counter): `count` changes over `span` s.
    Count,
    /// A scene transition.
    Transition,
    /// A layer appears in place (wipe, mask, fade).
    Reveal,
}

/// One candidate event.
#[derive(Debug, Clone)]
pub(crate) struct Event {
    pub kind: Kind,
    /// Hit time (s).
    pub t: f64,
    /// Importance (bigger = more deserving of a sound).
    pub salience: f32,
    /// 0..1 frame-relative x of the event (for pan).
    pub x: f32,
    /// −1..1 horizontal travel.
    pub travel: f32,
    /// 0..1 normalised speed.
    pub speed: f32,
    /// Fraction of the frame diagonal the object spans.
    pub size: f32,
    /// Move length (s) / ticker span.
    pub span: f32,
    pub count: u32,
    pub layer: u32,
    pub path: String,
    pub trans: Option<TransKind>,
    /// The move ends with the layer leaving the frame / disappearing.
    pub exits: bool,
}

#[derive(Clone, Copy, Default)]
struct Sample {
    cx: f32,
    cy: f32,
    size: f32,
    alpha: f32,
    text: u64,
}

/// Layer id used for camera moves.
const CAMERA: u32 = u32::MAX - 1;

/// Seconds per analysis step (30 per second, or the video's rate if lower).
fn step(ev: &Evaluator) -> f32 {
    1.0 / ev.fps.clamp(1.0, 30.0)
}

/// Sample every eligible layer's box over the whole video.
fn track(ev: &Evaluator, plan: &SoundPlan) -> (HashMap<u32, Vec<Option<Sample>>>, f32) {
    let dt = step(ev);
    let n = (ev.duration / dt).ceil() as usize + 1;
    let (w, h) = (ev.width as f32, ev.height as f32);
    let diag = (w * w + h * h).sqrt();
    let overrides: HashMap<u32, &LayerSfx> = plan.layers.iter().map(|(t, l)| (*t, l)).collect();
    let mut tracks: HashMap<u32, Vec<Option<Sample>>> = HashMap::new();
    for k in 0..n {
        let t = (k as f32 * dt).min(ev.duration - 1e-3);
        let (dl, tags) = ev.eval_tagged(t);
        let mut frame: HashMap<u32, (kurbo::Rect, f32, u64)> = HashMap::new();
        for tag in &tags {
            if tag.layer == u32::MAX {
                continue;
            }
            let o = overrides.get(&tag.layer);
            // Shader inputs (`channels`) are textures, not things on screen.
            if o.is_none() && ev.layer_info(tag.layer).0.contains(".channels[") {
                continue;
            }
            if o.is_some_and(|o| o.off) || (!plan.auto && !o.is_some_and(|o| o.auto)) {
                continue;
            }
            let Some(ops) = dl.ops.get(tag.ops.clone()) else {
                continue;
            };
            let Some(r) = crate::render::ops_bounds(ops, Affine::IDENTITY) else {
                continue;
            };
            let alpha = match ops.first() {
                Some(Op::PushLayer { opacity, .. }) => *opacity,
                _ => 1.0,
            };
            let kind = ev.layer_info(tag.layer).1;
            let text = if kind == "text" {
                ops.iter()
                    .fold(0xcbf2_9ce4_8422_2325u64, |hsh, op| match op {
                        Op::Glyphs { glyphs, .. } => glyphs
                            .iter()
                            .fold(hsh, |a, g| (a ^ g.id as u64).wrapping_mul(0x100_0000_01b3)),
                        _ => hsh,
                    })
            } else {
                0
            };
            frame
                .entry(tag.layer)
                .and_modify(|e| e.0 = e.0.union(r))
                .or_insert((r, alpha, text));
        }
        for (layer, (r, alpha, text)) in frame {
            // Skip full-frame things (backgrounds, camera plates) unless the layer asked.
            let area = (r.width() * r.height()) as f32 / (w * h);
            if area > 0.75 && !overrides.contains_key(&layer) {
                continue;
            }
            let tr = tracks.entry(layer).or_insert_with(|| vec![None; n]);
            tr[k] = Some(Sample {
                cx: r.center().x as f32 / w,
                cy: r.center().y as f32 / h,
                size: ((r.width() * r.height()).sqrt() as f32 / diag).min(1.0),
                alpha: alpha.clamp(0.0, 1.0),
                text,
            });
        }
    }
    (tracks, dt)
}

/// Find events in the picture.
pub(crate) fn events(ev: &Evaluator, plan: &SoundPlan) -> Vec<Event> {
    let (tracks, dt) = track(ev, plan);
    let aspect = ev.width as f32 / ev.height.max(1) as f32;
    let mut out = Vec::new();
    let mut layers: Vec<_> = tracks.into_iter().collect();
    layers.sort_by_key(|(l, _)| *l);
    let n = layers.first().map(|(_, t)| t.len()).unwrap_or(0);
    let norm = (1.0 + aspect * aspect).sqrt();
    // Per-layer velocity (frame-diagonals / s, x scaled by aspect) and growth (log size / s).
    let vel = |tr: &[Option<Sample>], k: usize| -> Option<(f32, f32, f32)> {
        match (
            tr.get(k.wrapping_sub(1)).copied().flatten(),
            tr.get(k).copied().flatten(),
        ) {
            (Some(a), Some(b)) if k > 0 => {
                let g = if a.size > 1e-4 && b.size > 1e-4 {
                    (b.size / a.size).ln()
                } else {
                    0.0
                };
                let (dx, dy) = ((b.cx - a.cx) * aspect / norm, (b.cy - a.cy) / norm);
                // A jump of a fifth of the frame in one step is a teleport (a wrap, a cut), not motion.
                if (dx * dx + dy * dy).sqrt() > 0.2 {
                    return None;
                }
                Some((dx / dt, dy / dt, g / dt))
            }
            _ => None,
        }
    };
    // Camera: when most visible layers move together, that shared motion is the
    // camera's (one event), and each layer only sounds its own motion on top.
    let mut cam = vec![(0.0f32, 0.0f32, 0.0f32); n];
    for (k, c) in cam.iter_mut().enumerate() {
        let vs: Vec<(f32, f32, f32)> = layers.iter().filter_map(|(_, tr)| vel(tr, k)).collect();
        if vs.len() < 3 {
            continue;
        }
        let med = |f: fn(&(f32, f32, f32)) -> f32| {
            let mut x: Vec<f32> = vs.iter().map(f).collect();
            x.sort_by(|a, b| a.total_cmp(b));
            x[x.len() / 2]
        };
        let m = (med(|v| v.0), med(|v| v.1), med(|v| v.2));
        let mag = (m.0 * m.0 + m.1 * m.1).sqrt() + m.2.abs() * 0.25;
        if mag < 0.04 {
            continue;
        }
        let agree = vs
            .iter()
            .filter(|v| {
                ((v.0 - m.0).powi(2) + (v.1 - m.1).powi(2)).sqrt() + (v.2 - m.2).abs() * 0.25
                    < 0.35 * mag + 0.02
            })
            .count();
        if agree * 10 >= vs.len() * 6 {
            *c = m;
        }
    }
    let cam_track: Vec<Option<Sample>> = (0..n)
        .map(|_| {
            Some(Sample {
                cx: 0.5,
                cy: 0.5,
                size: 1.0,
                alpha: 1.0,
                text: 0,
            })
        })
        .collect();
    let mut sources: Vec<(u32, String, &[Option<Sample>], bool)> = layers
        .iter()
        .map(|(l, tr)| (*l, ev.layer_info(*l).0.to_string(), tr.as_slice(), false))
        .collect();
    if cam.iter().any(|c| c.0 != 0.0 || c.1 != 0.0 || c.2 != 0.0) {
        sources.push((CAMERA, "camera".to_string(), cam_track.as_slice(), true));
    }
    for (layer, path, tr, is_cam) in sources {
        // Velocity relative to the camera (or the camera's own).
        let v: Vec<(f32, f32, f32)> = (0..n)
            .map(|k| {
                if is_cam {
                    cam[k]
                } else {
                    vel(tr, k)
                        .map(|v| (v.0 - cam[k].0, v.1 - cam[k].1, v.2 - cam[k].2))
                        .unwrap_or_default()
                }
            })
            .collect();
        let sp: Vec<f32> = v
            .iter()
            .map(|v| (v.0 * v.0 + v.1 * v.1).sqrt() + v.2.abs() * 0.25)
            .collect();
        let (v_on, v_off) = (0.22f32, 0.04f32);
        let mut k = 1;
        while k < n {
            if sp[k] <= v_on {
                k += 1;
                continue;
            }
            let mut a = k;
            while a > 1 && sp[a - 1] > v_off {
                a -= 1;
            }
            let mut b = k;
            while b + 1 < n && sp[b + 1] > v_off && tr[b + 1].is_some() {
                b += 1;
            }
            let (pk, vp) =
                (a..=b)
                    .map(|i| (i, sp[i]))
                    .fold((a, 0.0f32), |m, x| if x.1 > m.1 { x } else { m });
            let s_pk = tr[pk].unwrap_or_default();
            let s_a = tr[a.saturating_sub(1)].or(tr[a]).unwrap_or_default();
            let s_b = tr[b].unwrap_or_default();
            let size = s_pk.size.max(s_b.size);
            let alpha = s_pk.alpha.max(s_b.alpha);
            let len = (b + 1 - a) as f32 * dt;
            let dx: f32 = v[a..=b].iter().map(|v| v.0).sum::<f32>() * dt;
            let travel = (dx * 2.0).clamp(-1.0, 1.0);
            let speed = (vp / 2.5).clamp(0.0, 1.0);
            let sal = vp.min(4.0)
                * (0.25 + size.sqrt())
                * (0.3 + 0.7 * alpha)
                * if is_cam { 0.6 } else { 1.0 };
            let grew = !is_cam
                && s_a.size < s_b.size * 0.25
                && (s_b.cx - s_a.cx).abs() + (s_b.cy - s_a.cy).abs() < 0.03;
            let exits = !is_cam && (b + 1 >= n || tr[b + 1].is_none());
            let base = Event {
                kind: Kind::Move,
                t: pk as f64 * dt as f64,
                salience: sal,
                x: s_pk.cx,
                travel,
                speed,
                size,
                span: len,
                count: 1,
                layer,
                path: path.clone(),
                trans: None,
                exits,
            };
            if len > 2.5 {
                // Steady, ongoing motion (drifts, loops, conveyor belts) is ambience, not an event.
                k = b + 1;
                continue;
            }
            if grew {
                out.push(Event {
                    kind: Kind::Pop,
                    t: ((a + (b - a) / 2) as f64) * dt as f64,
                    ..base.clone()
                });
            } else if len >= 0.08 {
                out.push(base.clone());
            }
            if !grew && !is_cam {
                // Contact: a hard stop (still fast in the last moving frame), or the
                // first reversal of a spring's overshoot.
                let hard = b + 1 < n && tr[b + 1].is_some() && sp[b] > 0.3 * vp && vp > 0.5;
                let rev = (pk + 1..=b).find(|&i| {
                    let (p, q) = (v[i - 1], v[i]);
                    p.0 * q.0 + p.1 * q.1 + p.2 * q.2 * 0.06 < 0.0 && sp[i - 1] > 0.25 * vp
                });
                let contact = match (rev, hard) {
                    (Some(i), _) => Some(i - 1),
                    (None, true) => Some(b),
                    _ => None,
                };
                if let (Some(c), true) = (contact, vp > 0.5) {
                    let s_c = tr[c].unwrap_or(s_b);
                    out.push(Event {
                        kind: Kind::Land,
                        t: c as f64 * dt as f64,
                        salience: sal * 1.2,
                        x: s_c.cx,
                        ..base
                    });
                }
            }
            k = b + 1;
        }
    }
    for (layer, tr) in &layers {
        let (layer, path) = (*layer, ev.layer_info(*layer).0.to_string());
        let sp: Vec<f32> = (0..n)
            .map(|k| {
                vel(tr, k)
                    .map(|v| ((v.0 - cam[k].0).powi(2) + (v.1 - cam[k].1).powi(2)).sqrt())
                    .unwrap_or(0.0)
            })
            .collect();
        let v_on = 0.22f32;
        // Static reveals (wipes, masks, fades, cuts in): the first frame a layer is
        // visible after being absent, when it isn't flying in.
        for k in 1..n {
            if tr[k - 1].is_none() && tr[k].is_some() && sp.get(k + 1).is_none_or(|v| *v < v_on) {
                let s = tr[(k + 3).min(n - 1)].or(tr[k]).unwrap_or_default();
                if s.size > 0.02 {
                    out.push(Event {
                        kind: Kind::Reveal,
                        t: (k as f64 + 4.0) * dt as f64,
                        salience: 0.15 + 0.9 * s.size.sqrt() * (0.3 + 0.7 * s.alpha),
                        x: s.cx,
                        travel: 0.0,
                        speed: 0.4,
                        size: s.size,
                        span: 0.25,
                        count: 1,
                        layer,
                        path: path.clone(),
                        trans: None,
                        exits: false,
                    });
                }
            }
        }
        // Text changes while visible (not on the first frame it appears).
        let mut changes: Vec<f64> = Vec::new();
        for k in 1..n {
            if let (Some(a), Some(b)) = (tr[k - 1], tr[k]) {
                if a.text != b.text && b.text != 0 && a.text != 0 && sp[k] < v_on {
                    changes.push(k as f64 * dt as f64);
                }
            }
        }
        let mut i = 0;
        while i < changes.len() {
            let mut j = i;
            while j + 1 < changes.len() && changes[j + 1] - changes[j] < 0.2 {
                j += 1;
            }
            let s = tr[(changes[i] / dt as f64).round() as usize].unwrap_or_default();
            let count = (j + 1 - i) as u32;
            let base = Event {
                kind: Kind::Text,
                t: changes[i],
                salience: 0.25 + 0.5 * s.size,
                x: s.cx,
                travel: 0.0,
                speed: 0.0,
                size: s.size,
                span: 0.0,
                count: 1,
                layer,
                path: path.clone(),
                trans: None,
                exits: false,
            };
            if count >= 3 {
                out.push(Event {
                    kind: Kind::Count,
                    span: (changes[j] - changes[i]) as f32 + dt,
                    count,
                    salience: 0.5 + 0.5 * s.size,
                    ..base
                });
            } else {
                for c in &changes[i..=j] {
                    out.push(Event {
                        t: *c,
                        ..base.clone()
                    });
                }
            }
            i = j + 1;
        }
    }
    if plan.spec.transitions {
        for s in ev.scenes.iter().skip(1) {
            let k = s.trans.kind;
            if matches!(k, TransKind::Cut) {
                continue;
            }
            let d = s.trans.dur.max(0.05) as f64;
            let sal = match k {
                TransKind::Crossfade | TransKind::Dip | TransKind::Blur => 0.3,
                _ => 1.5,
            };
            out.push(Event {
                kind: Kind::Transition,
                t: s.start as f64 + d * 0.5,
                salience: sal,
                x: 0.5,
                travel: -s.trans.dir[0].clamp(-1.0, 1.0),
                speed: (0.6 / d as f32).clamp(0.2, 1.0),
                size: 1.0,
                span: d as f32,
                count: 1,
                layer: u32::MAX,
                path: format!("scenes[{}].transition", s.id),
                trans: Some(k),
                exits: false,
            });
        }
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t).then(b.salience.total_cmp(&a.salience)));
    out
}

/// Keep the events that deserve a sound: a budget of onsets per second set by
/// `density`, no two onsets within 60 ms (a group and its child moving
/// together sound once), whooshes ≥ 250 ms apart, explicit layer sounds always.
fn placeholder() -> Event {
    Event {
        kind: Kind::Move,
        t: 0.0,
        salience: 0.0,
        x: 0.5,
        travel: 0.0,
        speed: 0.0,
        size: 0.0,
        span: 0.0,
        count: 0,
        layer: 0,
        path: String::new(),
        trans: None,
        exits: false,
    }
}

pub(crate) fn select(mut evs: Vec<Event>, plan: &SoundPlan, duration: f32) -> Vec<Event> {
    let density = plan.spec.density.clamp(0.0, 1.0);
    let forced: std::collections::HashSet<u32> = plan
        .layers
        .iter()
        .filter(|(_, l)| l.enter.is_some() || l.land.is_some() || l.exit.is_some())
        .map(|(t, _)| *t)
        .collect();
    // Research: ≤ 2–3 onsets/s at the busiest, one hero every 4–8 s, 30–50 % of motion silent.
    let budget = ((duration * (0.25 + 1.1 * density)).ceil() as usize).max(1);
    let top = evs.iter().map(|e| e.salience).fold(0.0f32, f32::max);
    let floor = (0.12 * (1.0 - density)).max(top * 0.3 * (1.0 - density));
    let mut order: Vec<usize> = (0..evs.len()).collect();
    order.sort_by(|&a, &b| {
        let fa = forced.contains(&evs[a].layer) as u8;
        let fb = forced.contains(&evs[b].layer) as u8;
        fb.cmp(&fa)
            .then(evs[b].salience.total_cmp(&evs[a].salience))
            .then(evs[a].t.total_cmp(&evs[b].t))
    });
    let mut keep: Vec<usize> = Vec::new();
    for i in order {
        let e = &evs[i];
        let f = forced.contains(&e.layer);
        if !f && (keep.len() >= budget || e.salience < floor) {
            continue;
        }
        let clash = keep.iter().any(|&j| {
            let o = &evs[j];
            let gap = (o.t - e.t).abs();
            gap < 0.06 || (gap < 0.3 && o.kind == e.kind)
        });
        if !clash {
            keep.push(i);
        }
    }
    keep.sort();
    keep.into_iter()
        .map(|i| std::mem::replace(&mut evs[i], placeholder()))
        .collect()
}

/// Choose the sound for an event (palette, size, speed, layer overrides).
fn sound_for(e: &Event, plan: &SoundPlan, hero: bool) -> Option<&'static str> {
    let pal = plan.palette.as_str();
    let big = e.size > 0.3;
    if e.layer == CAMERA {
        return Some(if pal == "cinematic" {
            "whoosh-deep"
        } else {
            "whoosh-air"
        });
    }
    Some(match e.kind {
        Kind::Move => match pal {
            "cinematic" => {
                if big {
                    "whoosh-deep"
                } else if e.speed > 0.5 {
                    "whoosh"
                } else {
                    "air"
                }
            }
            "luxury" => "air",
            "digital" => {
                if e.speed > 0.6 {
                    "swish"
                } else {
                    "air"
                }
            }
            "organic" => {
                if big {
                    "passby"
                } else if e.speed > 0.6 {
                    "swish"
                } else {
                    "air"
                }
            }
            "playful" => "swish",
            _ => {
                if big && e.speed > 0.5 {
                    "passby"
                } else if e.speed > 0.6 {
                    "swish"
                } else {
                    "air"
                }
            }
        },
        Kind::Land => match pal {
            "cinematic" => {
                if hero {
                    "impact"
                } else {
                    "thump"
                }
            }
            "organic" => {
                if big {
                    "thump"
                } else {
                    "wood"
                }
            }
            "digital" => {
                if big {
                    "thump"
                } else {
                    "blip"
                }
            }
            "luxury" => {
                if big {
                    "thump"
                } else {
                    "glass"
                }
            }
            "playful" => {
                if big {
                    "thump"
                } else {
                    "plastic"
                }
            }
            _ => "thump",
        },
        Kind::Pop => match pal {
            "digital" => "blip",
            "organic" => "wood",
            "luxury" => "droplet",
            _ => "pop",
        },
        Kind::Text => "ticker",
        Kind::Count => "ticker",
        Kind::Reveal => match pal {
            "digital" => "blip",
            "playful" => "pop",
            "cinematic" => "whoosh-air",
            "clean" => "swish",
            _ => "air",
        },
        Kind::Transition => match e.trans? {
            TransKind::Glitch => "glitch",
            TransKind::Zoom => {
                if pal == "cinematic" {
                    "whoosh-deep"
                } else {
                    "whoosh"
                }
            }
            TransKind::Crossfade | TransKind::Dip | TransKind::Blur => "air",
            TransKind::Slide | TransKind::Push | TransKind::Wipe if pal == "cinematic" => "whoosh",
            _ => "swish",
        },
    })
}

/// Turn selected events into cues.
pub(crate) fn cues(evs: &[Event], plan: &SoundPlan) -> Vec<Cue> {
    let overrides: HashMap<u32, &LayerSfx> = plan.layers.iter().map(|(t, l)| (*t, l)).collect();
    let hero_sal = evs
        .iter()
        .filter(|e| e.kind == Kind::Land)
        .map(|e| e.salience)
        .fold(0.0f32, f32::max);
    let mut out = Vec::new();
    for (i, e) in evs.iter().enumerate() {
        let hero = e.kind == Kind::Land && e.salience >= hero_sal * 0.999 && e.size > 0.15;
        let o = overrides.get(&e.layer);
        let fixed = o.and_then(|o| match e.kind {
            Kind::Land => o.land.clone(),
            Kind::Move if e.exits => o.exit.clone().or_else(|| o.enter.clone()),
            Kind::Move | Kind::Pop => o.enter.clone(),
            _ => None,
        });
        let name = match fixed {
            Some(n) => n,
            None => match sound_for(e, plan, hero) {
                Some(n) => n.to_string(),
                None => continue,
            },
        };
        // Round-robin variation: ±1 semitone, ±1.5 dB, a fresh seed per cue.
        let seed = plan
            .spec
            .seed
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(i as u64 * 7919 + e.layer as u64);
        let jitter = |k: u64| ((seed.wrapping_mul(k).rotate_left(17) % 1000) as f32 / 500.0) - 1.0;
        let pan = if plan.spec.pan {
            ((e.x - 0.5) * 2.0 * 0.6).clamp(-0.6, 0.6)
        } else {
            0.0
        };
        let intensity = (0.35 + 0.4 * e.salience.min(1.5)).clamp(0.2, 1.0);
        let p = DesignParams {
            duration: match e.kind {
                Kind::Move => Some((e.span * 1.3).clamp(0.2, 1.4)),
                Kind::Transition => Some((e.span * 1.6).clamp(0.3, 1.4)),
                Kind::Count => Some(e.span.max(0.1)),
                Kind::Text => Some(0.05),
                Kind::Reveal => Some(0.35),
                _ => None,
            },
            pitch: jitter(31) * 1.0
                + if e.size < 0.08 && matches!(e.kind, Kind::Land | Kind::Pop) {
                    3.0
                } else {
                    0.0
                },
            intensity: if hero { 0.9 } else { intensity },
            size: 0.3,
            pan,
            motion: if plan.spec.pan { e.travel } else { 0.0 },
            speed: e.speed,
            tone: 0.5,
            tune: plan.tune,
            count: match e.kind {
                Kind::Count => Some(e.count.min(40)),
                Kind::Text => Some(1),
                _ => None,
            },
            seed,
        };
        let duck_db = match e.kind {
            Kind::Land if hero => 5.0,
            Kind::Land => 2.0,
            Kind::Transition if e.salience > 1.0 => 2.0,
            Kind::Move if e.size > 0.3 => 1.5,
            _ => 0.0,
        };
        let gain_db = jitter(57) * 1.5
            + o.map(|o| o.gain).unwrap_or(0.0)
            + match e.kind {
                // Smaller things sound smaller.
                Kind::Move | Kind::Land => 6.0 * (e.size.sqrt() - 0.5).clamp(-0.6, 0.4),
                Kind::Text => -4.0,
                Kind::Reveal => -5.0 + 4.0 * e.size.sqrt(),
                _ => 0.0,
            };
        let send = match plan.palette.as_str() {
            "cinematic" | "luxury" => 0.35,
            "digital" => 0.12,
            _ => 0.22,
        };
        out.push(Cue {
            name,
            at: e.t,
            params: p,
            gain_db,
            send,
            duck_db,
            own_size: false,
            why: format!(
                "{}: {}",
                e.path,
                match e.kind {
                    Kind::Move => "moves",
                    Kind::Land => "lands",
                    Kind::Pop => "pops in",
                    Kind::Text => "text changes",
                    Kind::Count => "counts",
                    Kind::Transition => "transition",
                    Kind::Reveal => "appears",
                }
            ),
        });
    }
    out
}

/// Explicit cues from `sound.cues`.
pub fn explicit(plan: &SoundPlan) -> Vec<Cue> {
    plan.cues
        .iter()
        .enumerate()
        .map(|(i, (c, at))| {
            let mut p = DesignParams {
                tune: plan.tune,
                seed: plan.spec.seed.wrapping_add(1000 + i as u64),
                ..Default::default()
            };
            c.apply(&mut p);
            if let Some(x) = c.x {
                p.pan = ((x - 0.5) * 2.0 * 0.6).clamp(-0.6, 0.6);
            }
            Cue {
                name: c.sfx.clone(),
                at: *at,
                params: p,
                gain_db: c.gain.unwrap_or(0.0),
                send: c.send.unwrap_or(0.22),
                duck_db: c.duck.unwrap_or(0.0),
                own_size: c.size.is_some(),
                why: format!("sound.cues[{i}]"),
            }
        })
        .collect()
}

/// All cues for a video: picture-derived (when enabled) plus explicit.
pub fn plan_cues(ev: &Evaluator, plan: &SoundPlan) -> Vec<Cue> {
    let mut all = Vec::new();
    if plan.auto || plan.layers.iter().any(|(_, l)| l.auto) {
        let evs = select(events(ev, plan), plan, ev.duration);
        all.extend(cues(&evs, plan));
    }
    all.extend(explicit(plan));
    all.sort_by(|a, b| a.at.total_cmp(&b.at));
    all
}

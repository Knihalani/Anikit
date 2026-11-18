//! `physarum`: agents that deposit into a 2D field and read it back.
//!
//! The slime-mould model (Jones 2010): each agent senses the trail field at
//! three points ahead (left, front, right), turns towards the strongest,
//! steps forward and deposits; the field diffuses and decays. Networks of
//! veins form, merge and re-route. Text can seed the agents, attract them
//! (it keeps depositing), or both.
//!
//! Deterministic at any thread count: agents move from the previous field
//! only, and deposits are fixed-point integer additions (order-free).

use super::fluid::{word_mask, Word};
use super::style::{fnv, Color};
use super::{Component, Ctx, Error, Prepare};
use crate::dl::Op;
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Fixed-point scale of deposits (integer sums are order-free).
const FIX: f32 = 4096.0;

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Physarum {
    /// Number of agents.
    pub agents: u32,
    /// Field cells along the long side.
    pub resolution: u32,
    /// Sensor angle (degrees), sensor distance (cells), turn per step (degrees), step length (cells).
    pub sensor_angle: f32,
    pub sensor_distance: f32,
    pub turn: f32,
    pub step: f32,
    /// Trail deposited per agent per step; fraction of the field kept per step; diffusion (0–1 of a 3×3 blur per step).
    pub deposit: f32,
    pub decay: f32,
    pub diffuse: f32,
    /// Random wobble added to every turn (degrees).
    pub jitter: f32,
    /// Where agents start: `disc`, `ring`, `random`, `point`, `text`.
    pub spawn: String,
    /// Spawn disc/ring radius (fraction of the box height) and centre (fractions of the box).
    pub radius: f32,
    pub center: [f32; 2],
    /// Agents start facing `out`, `in` or `random`.
    pub facing: String,
    /// A word: agents spawn in it (`spawn: text`) and/or it attracts them (`attract`: trail it adds per step).
    pub text: Option<Word>,
    pub attract: f32,
    /// Edges wrap around (a torus); otherwise agents turn back.
    pub wrap: bool,
    /// Look: background, gradient from faint to dense trail, exposure (v = 1 − exp(−trail · exposure)).
    pub background: Option<Color>,
    pub colors: Vec<Color>,
    pub exposure: f32,
    /// Simulation steps per second.
    pub speed: f32,
    pub seed: u32,
    #[serde(skip)]
    sim: OnceLock<Sim>,
}

impl Default for Physarum {
    fn default() -> Self {
        Physarum {
            agents: 200_000,
            resolution: 640,
            sensor_angle: 30.0,
            sensor_distance: 9.0,
            turn: 30.0,
            step: 1.0,
            deposit: 1.0,
            decay: 0.9,
            diffuse: 1.0,
            jitter: 0.0,
            spawn: "disc".into(),
            radius: 0.35,
            center: [0.5, 0.5],
            facing: "out".into(),
            text: None,
            attract: 0.0,
            wrap: true,
            background: None,
            colors: Vec::new(),
            exposure: 0.15,
            speed: 60.0,
            seed: 1,
            sim: OnceLock::new(),
        }
    }
}

impl Prepare for Physarum {
    fn prepare(&mut self) -> Result<(), Error> {
        if !["disc", "ring", "random", "point", "text"].contains(&self.spawn.as_str()) {
            return Err(Error(format!("physarum: unknown spawn `{}` (disc, ring, random, point, text)", self.spawn)));
        }
        if !["out", "in", "random"].contains(&self.facing.as_str()) {
            return Err(Error(format!("physarum: unknown facing `{}` (out, in, random)", self.facing)));
        }
        if self.spawn == "text" && self.text.is_none() {
            return Err(Error("physarum: `spawn: text` needs a `text` ({ \"text\": \"...\" })".into()));
        }
        self.agents = self.agents.clamp(1, 8_000_000);
        self.resolution = self.resolution.clamp(32, 4096);
        self.decay = self.decay.clamp(0.0, 1.0);
        self.diffuse = self.diffuse.clamp(0.0, 1.0);
        self.speed = self.speed.clamp(1.0, 600.0);
        Ok(())
    }
}

#[derive(Clone)]
struct State {
    step: u64,
    /// x, y (cells), heading (radians).
    agents: Vec<[f32; 3]>,
    field: Vec<f32>,
}

struct SimState {
    lives: Vec<State>,
    frames: HashMap<u64, Arc<Vec<f32>>>,
    checkpoints: Vec<State>,
}

struct Sim {
    w: usize,
    h: usize,
    mask: Option<Vec<f32>>,
    state: Mutex<SimState>,
}

#[inline]
fn hash(a: u32, b: u32, c: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

fn lin(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

impl Physarum {
    fn sim(&self, aspect: f32) -> &Sim {
        self.sim.get_or_init(|| {
            let long = self.resolution as f32;
            let (w, h) = if aspect >= 1.0 { (long as usize, (long / aspect).round().max(8.0) as usize) } else { ((long * aspect).round().max(8.0) as usize, long as usize) };
            let mask = self.text.as_ref().map(|t| word_mask(t, w, h, aspect));
            Sim { w, h, mask, state: Mutex::new(SimState { lives: Vec::new(), frames: HashMap::new(), checkpoints: Vec::new() }) }
        })
    }

    fn init(&self, s: &Sim) -> State {
        let (w, h) = (s.w as f32, s.h as f32);
        let n = self.agents as usize;
        let (cx, cy) = (self.center[0] * w, self.center[1] * h);
        let r = self.radius * h;
        // Text spawn: agents placed in the mask's cells, chosen by hash.
        let cells: Vec<u32> = match (&s.mask, self.spawn.as_str()) {
            (Some(m), "text") => m.iter().enumerate().filter(|(_, v)| **v > 0.5).map(|(i, _)| i as u32).collect(),
            _ => Vec::new(),
        };
        let agents = (0..n)
            .into_par_iter()
            .map(|i| {
                let i = i as u32;
                let (u1, u2, u3) = (unit(hash(self.seed, i, 1)), unit(hash(self.seed, i, 2)), unit(hash(self.seed, i, 3)));
                let (x, y) = match self.spawn.as_str() {
                    "random" => (u1 * w, u2 * h),
                    "point" => (cx, cy),
                    "ring" => {
                        let a = u1 * std::f32::consts::TAU;
                        (cx + a.cos() * r, cy + a.sin() * r)
                    }
                    "text" if !cells.is_empty() => {
                        let c = cells[(hash(self.seed, i, 4) as usize) % cells.len()] as usize;
                        ((c % s.w) as f32 + u1, (c / s.w) as f32 + u2)
                    }
                    _ => {
                        let (a, rr) = (u1 * std::f32::consts::TAU, u2.sqrt() * r);
                        (cx + a.cos() * rr, cy + a.sin() * rr)
                    }
                };
                let out = (y - cy).atan2(x - cx);
                let a = match self.facing.as_str() {
                    "out" if (x - cx).abs() + (y - cy).abs() > 1e-3 => out,
                    "in" if (x - cx).abs() + (y - cy).abs() > 1e-3 => out + std::f32::consts::PI,
                    _ => u3 * std::f32::consts::TAU,
                };
                [x.rem_euclid(w), y.rem_euclid(h), a]
            })
            .collect();
        State { step: 0, agents, field: vec![0.0; s.w * s.h] }
    }

    /// One step: sense, turn, move and deposit (from the previous field), then diffuse and decay.
    fn advance(&self, s: &Sim, st: &mut State) {
        let (w, h) = (s.w, s.h);
        let (wf, hf) = (w as f32, h as f32);
        let field = &st.field;
        let sa = self.sensor_angle.to_radians();
        let sd = self.sensor_distance;
        let turn = self.turn.to_radians();
        let jit = self.jitter.to_radians();
        let step_no = st.step as u32;
        let wrap = self.wrap;
        let at = |x: f32, y: f32| -> f32 {
            let (xi, yi) = if wrap { ((x.rem_euclid(wf)) as usize % w, (y.rem_euclid(hf)) as usize % h) } else { (x.clamp(0.0, wf - 1.0) as usize, y.clamp(0.0, hf - 1.0) as usize) };
            field[yi * w + xi]
        };
        let dep: Vec<AtomicU32> = (0..w * h).map(|_| AtomicU32::new(0)).collect();
        let q = (self.deposit.max(0.0) * FIX).round() as u32;
        let seed = self.seed;
        st.agents.par_chunks_mut(4096).enumerate().for_each(|(c, chunk)| {
            for (k, ag) in chunk.iter_mut().enumerate() {
                let i = (c * 4096 + k) as u32;
                let [x, y, mut a] = *ag;
                let f = at(x + a.cos() * sd, y + a.sin() * sd);
                let l = at(x + (a - sa).cos() * sd, y + (a - sa).sin() * sd);
                let r = at(x + (a + sa).cos() * sd, y + (a + sa).sin() * sd);
                let rnd = hash(seed, i, step_no.wrapping_mul(2).wrapping_add(17));
                if f >= l && f >= r {
                } else if f < l && f < r {
                    a += if rnd & 1 == 0 { turn } else { -turn };
                } else if l > r {
                    a -= turn;
                } else {
                    a += turn;
                }
                if jit > 0.0 {
                    a += (unit(hash(seed, i, step_no.wrapping_mul(2).wrapping_add(18))) - 0.5) * 2.0 * jit;
                }
                let (mut nx, mut ny) = (x + a.cos() * self.step, y + a.sin() * self.step);
                if wrap {
                    nx = nx.rem_euclid(wf);
                    ny = ny.rem_euclid(hf);
                } else if nx < 0.0 || ny < 0.0 || nx >= wf || ny >= hf {
                    // Turn back at the edge, at a random angle.
                    nx = x;
                    ny = y;
                    a = unit(rnd >> 1) * std::f32::consts::TAU;
                }
                let ci = ((ny as usize).min(h - 1)) * w + (nx as usize).min(w - 1);
                dep[ci].fetch_add(q, Ordering::Relaxed);
                *ag = [nx, ny, a];
            }
        });
        // Field: add deposits (and the word's attraction), blur, decay.
        let attract = self.attract;
        let mask = s.mask.as_deref();
        let src: Vec<f32> = (0..w * h)
            .into_par_iter()
            .with_min_len(4096)
            .map(|i| field[i] + dep[i].load(Ordering::Relaxed) as f32 / FIX + mask.map(|m| m[i] * attract).unwrap_or(0.0))
            .collect();
        let (dif, keep) = (self.diffuse, self.decay);
        let mut out = vec![0.0f32; w * h];
        out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            let (yu, yd) = if wrap { ((y + h - 1) % h, (y + 1) % h) } else { (y.saturating_sub(1), (y + 1).min(h - 1)) };
            for x in 0..w {
                let (xl, xr) = if wrap { ((x + w - 1) % w, (x + 1) % w) } else { (x.saturating_sub(1), (x + 1).min(w - 1)) };
                let mut sum = 0.0;
                for yy in [yu, y, yd] {
                    sum += src[yy * w + xl] + src[yy * w + x] + src[yy * w + xr];
                }
                let c = src[y * w + x];
                row[x] = (c + (sum / 9.0 - c) * dif) * keep;
            }
        });
        st.field = out;
        st.step += 1;
    }

    fn frame(&self, s: &Sim, frame: u64, fps: f32) -> Arc<Vec<f32>> {
        let target = ((frame as f32 / fps) * self.speed).round() as u64;
        let mut st = s.state.lock().unwrap();
        if let Some(fr) = st.frames.get(&target) {
            return fr.clone();
        }
        let best_live = st.lives.iter().enumerate().filter(|(_, l)| l.step <= target).max_by_key(|(_, l)| l.step).map(|(k, l)| (k, l.step));
        let best_cp = st.checkpoints.iter().filter(|c| c.step <= target).map(|c| c.step).max();
        let start = match (best_live, best_cp) {
            (Some((k, ls)), cp) if cp.is_none_or(|c| ls >= c) => st.lives.swap_remove(k),
            (_, Some(c)) => st.checkpoints.iter().find(|x| x.step == c).cloned().unwrap(),
            _ => self.init(s),
        };
        let bytes = start.agents.len() * 12 + start.field.len() * 4;
        let max_cp = ((1usize << 29) / bytes.max(1)).clamp(1, 64);
        let st_ref = &mut *st;
        let out = crate::scene3d::sim_blocking(|| {
            let mut f = start;
            while f.step < target {
                self.advance(s, &mut f);
                if f.step % 60 == 0 && !st_ref.checkpoints.iter().any(|c| c.step == f.step) && st_ref.checkpoints.len() < max_cp {
                    st_ref.checkpoints.push(f.clone());
                }
            }
            f
        });
        let fr = Arc::new(out.field.clone());
        st.frames.insert(target, fr.clone());
        if st.frames.len() > 48 {
            let mut keys: Vec<u64> = st.frames.keys().copied().collect();
            keys.sort_unstable();
            for k in keys.iter().take(keys.len() - 48) {
                st.frames.remove(k);
            }
        }
        st.lives.push(out);
        let keep = ((1usize << 29) / bytes.max(1)).clamp(1, 6);
        while st.lives.len() > keep {
            let k = st.lives.iter().enumerate().min_by_key(|(_, l)| l.step).map(|(k, _)| k).unwrap();
            st.lives.swap_remove(k);
        }
        fr
    }

    fn render(&self, s: &Sim, field: &[f32], w: usize, h: usize) -> Vec<u8> {
        let lc = |c: &Color| [lin(c.0 .0[0]), lin(c.0 .0[1]), lin(c.0 .0[2])];
        let bg = self.background.as_ref().map(lc).unwrap_or([0.004, 0.0045, 0.006]);
        let grad: Vec<[f32; 3]> = if self.colors.is_empty() { vec![bg, [0.9, 0.92, 0.95]] } else { std::iter::once(bg).chain(self.colors.iter().map(lc)).collect() };
        let (fw, fh) = (s.w, s.h);
        let (kx, ky) = (fw as f32 / w as f32, fh as f32 / h as f32);
        let exposure = self.exposure;
        let mut out = vec![0u8; w * h * 4];
        out.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                // Bilinear from the field.
                let (gx, gy) = (((x as f32 + 0.5) * kx - 0.5).clamp(0.0, (fw - 1) as f32), ((y as f32 + 0.5) * ky - 0.5).clamp(0.0, (fh - 1) as f32));
                let (x0, y0) = (gx as usize, gy as usize);
                let (x1, y1) = ((x0 + 1).min(fw - 1), (y0 + 1).min(fh - 1));
                let (tx, ty) = (gx - x0 as f32, gy - y0 as f32);
                let a = field[y0 * fw + x0] + (field[y0 * fw + x1] - field[y0 * fw + x0]) * tx;
                let b = field[y1 * fw + x0] + (field[y1 * fw + x1] - field[y1 * fw + x0]) * tx;
                let v = 1.0 - (-(a + (b - a) * ty).max(0.0) * exposure).exp();
                let f = v * (grad.len() - 1) as f32;
                let k = (f as usize).min(grad.len() - 2);
                let t = f - k as f32;
                let hs = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
                let dith = ((hs >> 8) & 0xff) as f32 / 255.0 - 0.5;
                for c in 0..3 {
                    let lv = grad[k][c] + (grad[k + 1][c] - grad[k][c]) * t;
                    row[x * 4 + c] = (srgb(lv) * 255.0 + 0.5 + dith).clamp(0.0, 255.0) as u8;
                }
                row[x * 4 + 3] = 255;
            }
        });
        out
    }

    fn key(&self) -> u64 {
        let word = self.text.as_ref().map(|w| format!("{}{}{}{:?}{}", w.text, w.font, w.weight, w.pos, w.size)).unwrap_or_default();
        let s = format!(
            "{} {} {} {} {} {} {} {} {} {} {} {} {:?} {} {} {} {} {:?} {:?} {} {} {}",
            self.agents,
            self.resolution,
            self.sensor_angle,
            self.sensor_distance,
            self.turn,
            self.step,
            self.deposit,
            self.decay,
            self.diffuse,
            self.jitter,
            self.spawn,
            self.radius,
            self.center,
            self.facing,
            word,
            self.attract,
            self.wrap,
            self.background.map(|c| c.0 .0),
            self.colors.iter().map(|c| c.0 .0).collect::<Vec<_>>(),
            self.exposure,
            self.speed,
            self.seed
        );
        fnv(&[b"physarum", s.as_bytes()])
    }
}

impl Component for Physarum {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let ps = crate::eval::shader::preview_scale();
        let (w, h) = ((ctx.width * ps).round().max(2.0) as usize, (ctx.height * ps).round().max(2.0) as usize);
        let aspect = ctx.width / ctx.height.max(1.0);
        let fps = ctx.fps.max(1.0);
        let frame = (t * fps).round().max(0.0) as u64;
        let s = self.sim(aspect);
        let key = fnv(&[&self.key().to_le_bytes(), &frame.to_le_bytes(), &fps.to_bits().to_le_bytes(), &(w as u32).to_le_bytes(), &(h as u32).to_le_bytes()]);
        let image = ctx.assets.image(key, w as u32, h as u32, &mut || {
            let fr = self.frame(s, frame, fps);
            self.render(s, &fr, w, h)
        });
        vec![Op::Image { image, dst: [0.0, 0.0, ctx.width, ctx.height], opacity: 1.0, transform: crate::dl::IDENTITY }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slime(v: serde_json::Value) -> Physarum {
        let mut p: Physarum = serde_json::from_value(v).unwrap();
        p.prepare().unwrap();
        p
    }

    #[test]
    fn identical_at_any_thread_count() {
        let spec = serde_json::json!({ "agents": 20000, "resolution": 128 });
        let run = |threads: usize| {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                let p = slime(spec.clone());
                let s = p.sim(16.0 / 9.0);
                p.frame(s, 30, 30.0).as_ref().clone()
            })
        };
        let (a, b) = (run(1), run(4));
        assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
    }

    #[test]
    fn frames_in_any_order_match() {
        let spec = serde_json::json!({ "agents": 5000, "resolution": 96, "spawn": "random" });
        let (a, b) = (slime(spec.clone()), slime(spec));
        let (sa, sb) = (a.sim(1.0), b.sim(1.0));
        let x = a.frame(sa, 20, 30.0);
        let _ = b.frame(sb, 50, 30.0);
        let y = b.frame(sb, 20, 30.0);
        assert_eq!(x, y);
    }

    #[test]
    fn agents_form_a_network() {
        // From uniform noise, trails concentrate into veins: the field's
        // contrast (coefficient of variation) grows well above the start's.
        let p = slime(serde_json::json!({ "agents": 30000, "resolution": 128, "spawn": "random", "facing": "random" }));
        let s = p.sim(1.0);
        let cv = |f: &[f32]| {
            let m = f.iter().sum::<f32>() / f.len() as f32;
            (f.iter().map(|x| (x - m) * (x - m)).sum::<f32>() / f.len() as f32).sqrt() / m.max(1e-9)
        };
        let early = cv(&p.frame(s, 1, 30.0));
        let late = cv(&p.frame(s, 150, 30.0));
        assert!(late > early * 1.5 && late > 1.0, "contrast {early} → {late}");
    }
}

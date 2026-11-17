//! The sequencer: a [`Song`] of [`Part`]s (an instrument + notes in beats)
//! rendered sample-accurately into dry stereo stems.
//!
//! Rendering is parallel and deterministic: notes are cut into fixed groups
//! (by start time, independent of the thread count), each group renders its
//! voices sequentially into its own buffer on any thread, and the groups are
//! summed in a fixed order. Voice interactions (mono release, choke groups,
//! round robin) are resolved in a sequential pre-pass.

use crate::notes::Note;
use crate::voice::{Instrument, NoteOn, Rng, Voice, BLOCK};
use rayon::prelude::*;
use std::sync::Arc;

/// Tempo, meter and swing: the beat grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub bpm: f64,
    /// Beats (quarter notes) per bar: 4 for 4/4, 3 for 3/4, 3 for 6/8.
    pub beats_per_bar: f64,
    /// 0 = straight, 1 = hard swing (75 %); 0.33 ≈ triplet feel.
    pub swing: f64,
    /// Swing subdivision in beats: 0.25 (16ths) or 0.5 (8ths).
    pub swing_unit: f64,
    /// Seconds before beat 1 (pickup / offset on the video timeline).
    pub offset: f64,
}

impl Default for Grid {
    fn default() -> Self {
        Grid { bpm: 120.0, beats_per_bar: 4.0, swing: 0.0, swing_unit: 0.25, offset: 0.0 }
    }
}

impl Grid {
    /// Seconds of beat position `b` (straight, no swing).
    #[inline]
    pub fn secs(&self, b: f64) -> f64 {
        self.offset + b * 60.0 / self.bpm
    }
    /// Seconds of beat `b` with swing applied to off-beat subdivisions.
    pub fn swung(&self, b: f64) -> f64 {
        if self.swing <= 0.0 {
            return self.secs(b);
        }
        let u = self.swing_unit;
        let pair = 2.0 * u;
        let k = (b / pair).floor();
        let f = b - k * pair; // position inside the pair, 0..2u
        // Piecewise-linear warp: the midpoint u moves to u·(1 + swing/2).
        let mid = u * (1.0 + 0.5 * self.swing.clamp(0.0, 1.0));
        let w = if f < u { f / u * mid } else { mid + (f - u) / u * (pair - mid) };
        self.secs(k * pair + w)
    }
    /// Seconds per beat.
    pub fn beat(&self) -> f64 {
        60.0 / self.bpm
    }
    /// Seconds per bar.
    pub fn bar(&self) -> f64 {
        self.beats_per_bar * 60.0 / self.bpm
    }
}

/// One instrument part.
#[derive(Clone)]
pub struct Part {
    pub name: String,
    pub inst: Arc<dyn Instrument>,
    pub notes: Vec<Note>,
    /// Humanisation 0..1: timing (≈ ±12 ms at 1) and velocity (± 0.1 at 1).
    pub humanize: f32,
    /// Seed for humanisation and voices.
    pub seed: u64,
    /// Ignore swing (e.g. straight hats against a swung groove).
    pub straight: bool,
}

/// A timed note, in seconds and frames, ready to render.
#[derive(Debug, Clone, Copy)]
pub struct Event {
    pub start: usize,
    pub release: usize,
    pub on: NoteOn,
}

/// Turn a part's notes into frame-accurate events.
pub fn events(part: &Part, grid: &Grid, rate: u32) -> Vec<Event> {
    let mut notes = part.notes.clone();
    notes.sort_by(|a, b| a.t.total_cmp(&b.t).then(a.key.total_cmp(&b.key)));
    let r = rate as f64;
    let mut rng = Rng::new(part.seed ^ 0x5eed);
    let mut rr = [0u32; 128];
    let mut out = Vec::with_capacity(notes.len());
    for (i, n) in notes.iter().enumerate() {
        let (mut t0, mut t1) = if part.straight {
            (grid.secs(n.t), grid.secs(n.t + n.len))
        } else {
            (grid.swung(n.t), grid.swung(n.t + n.len))
        };
        let mut vel = n.vel;
        if part.humanize > 0.0 {
            let h = part.humanize as f64;
            // Notes on the beat drift less (the groove anchors on it).
            let on_beat = (n.t - n.t.round()).abs() < 1e-6;
            let dt = rng.gauss() as f64 * 0.006 * h * if on_beat { 0.5 } else { 1.0 };
            t0 = (t0 + dt).max(0.0);
            t1 += dt;
            vel = (vel + rng.gauss() * 0.05 * part.humanize).clamp(0.02, 1.0);
        }
        let start = (t0 * r).round().max(0.0) as usize;
        let release = ((t1 * r).round() as usize).max(start + 1);
        let k = (n.key.round() as i64).clamp(0, 127) as usize;
        out.push(Event {
            start,
            release,
            on: NoteOn {
                key: n.key,
                vel,
                rate: rate as f32,
                seed: part.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (i as u64).wrapping_mul(0xD1B5_4A32_D192_ED03),
                rr: rr[k],
                len: (t1 - t0) as f32,
            },
        });
        rr[k] += 1;
    }
    out
}

/// Longest a voice may ring after its release (safety cap), seconds.
const MAX_TAIL: f64 = 20.0;
/// Notes per parallel render group.
const GROUP: usize = 48;
/// Voice-seconds per parallel render group.
const GROUP_SECS: f64 = 6.0;

struct Job {
    start: usize,
    release: usize,
    kill: usize,
    voice: Box<dyn Voice>,
}

/// Render one part to dry planar stereo of exactly `frames` frames.
pub fn render_part(part: &Part, grid: &Grid, rate: u32, frames: usize) -> (Vec<f32>, Vec<f32>) {
    let evs = events(part, grid, rate);
    let mono = part.inst.mono();
    // Create voices in parallel (first use of a sample decodes it), then
    // resolve mono and choke groups in a sequential pass, in note order.
    let live: Vec<&Event> = evs.iter().take_while(|e| e.start < frames).collect();
    let voices: Vec<Box<dyn Voice>> = live.par_iter().map(|e| part.inst.voice(&e.on)).collect();
    let mut jobs: Vec<Job> = Vec::with_capacity(live.len());
    let mut last_in_class: std::collections::HashMap<u32, usize> = Default::default();
    for (e, voice) in live.into_iter().zip(voices) {
        let class = voice.exclusive();
        if mono {
            if let Some(prev) = jobs.last_mut() {
                // Legato: the previous note releases where this one starts.
                if prev.release > e.start {
                    prev.release = e.start.max(prev.start + 1);
                }
            }
        }
        if class != 0 {
            if let Some(&j) = last_in_class.get(&class) {
                let p = &mut jobs[j];
                p.kill = p.kill.min(e.start.max(p.start + 1));
            }
            last_in_class.insert(class, jobs.len());
        }
        jobs.push(Job { start: e.start, release: e.release, kill: usize::MAX, voice });
    }
    let tail = (MAX_TAIL * rate as f64) as usize;
    // Groups of consecutive notes render in parallel; each gets its own buffer.
    // A group closes after GROUP notes or ~GROUP_SECS of voice time, so
    // sparse parts with long notes (pads) still spread over the cores. The
    // split depends only on the notes, never on the thread count.
    let budget = (GROUP_SECS * rate as f64) as usize;
    let groups: Vec<Vec<Job>> = {
        let mut g: Vec<Vec<Job>> = Vec::new();
        let mut cur: Vec<Job> = Vec::new();
        let mut cost = 0usize;
        for j in jobs {
            cost += (j.release.min(j.kill) - j.start) + rate as usize;
            cur.push(j);
            if cur.len() >= GROUP || cost >= budget {
                g.push(std::mem::take(&mut cur));
                cost = 0;
            }
        }
        if !cur.is_empty() {
            g.push(cur);
        }
        g
    };
    let rendered: Vec<(usize, Vec<f32>, Vec<f32>)> = groups
        .into_par_iter()
        .map(|mut g| {
            let g0 = g.iter().map(|j| j.start).min().unwrap_or(0);
            let mut l: Vec<f32> = Vec::new();
            let mut r: Vec<f32> = Vec::new();
            for j in g.iter_mut() {
                let end = frames.min(j.release.saturating_add(tail)).min(j.kill.saturating_add(rate as usize / 10));
                render_voice(j, end, g0, &mut l, &mut r);
            }
            (g0, l, r)
        })
        .collect();
    let mut l = vec![0.0f32; frames];
    let mut r = vec![0.0f32; frames];
    for (g0, gl, gr) in rendered {
        let n = gl.len().min(frames.saturating_sub(g0));
        for (o, s) in l[g0..g0 + n].iter_mut().zip(&gl[..n]) {
            *o += s;
        }
        for (o, s) in r[g0..g0 + n].iter_mut().zip(&gr[..n]) {
            *o += s;
        }
    }
    (l, r)
}

fn render_voice(j: &mut Job, end: usize, g0: usize, l: &mut Vec<f32>, r: &mut Vec<f32>) {
    let mut pos = j.start;
    let mut released = false;
    let mut killed = false;
    while pos < end {
        if !released && pos >= j.release {
            j.voice.release();
            released = true;
        }
        if !killed && pos >= j.kill {
            j.voice.kill();
            killed = true;
        }
        let mut n = BLOCK.min(end - pos);
        if !released && j.release > pos {
            n = n.min(j.release - pos);
        }
        if !killed && j.kill > pos {
            n = n.min(j.kill - pos);
        }
        let a = pos - g0;
        if l.len() < a + n {
            // Grow in big steps to keep reallocations rare.
            let want = (a + n).max(l.len() * 3 / 2).max(1 << 16);
            l.resize(want, 0.0);
            r.resize(want, 0.0);
        }
        let alive = j.voice.render(&mut l[a..a + n], &mut r[a..a + n]);
        pos += n;
        if !alive {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test instrument: a sine with a linear release.
    struct Sine;
    struct SineVoice {
        ph: f32,
        inc: f32,
        amp: f32,
        rel: bool,
        excl: u32,
    }
    impl Voice for SineVoice {
        fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
            for (a, b) in l.iter_mut().zip(r.iter_mut()) {
                let s = (self.ph * std::f32::consts::TAU).sin() * self.amp;
                self.ph = (self.ph + self.inc).fract();
                *a += s;
                *b += s;
                if self.rel {
                    self.amp = (self.amp - 0.001).max(0.0);
                }
            }
            self.amp > 0.0
        }
        fn release(&mut self) {
            self.rel = true;
        }
        fn exclusive(&self) -> u32 {
            self.excl
        }
    }
    impl Instrument for Sine {
        fn voice(&self, n: &NoteOn) -> Box<dyn Voice> {
            Box::new(SineVoice { ph: 0.0, inc: n.freq() / n.rate, amp: n.vel * 0.5, rel: false, excl: if n.key as i32 == 42 || n.key as i32 == 46 { 1 } else { 0 } })
        }
    }

    #[test]
    fn swing_warps_offbeats_only() {
        let g = Grid { bpm: 120.0, swing: 1.0, ..Default::default() };
        assert_eq!(g.swung(1.0), 0.5);
        assert_eq!(g.swung(0.5), 0.25);
        assert!((g.swung(0.25) - 0.1875).abs() < 1e-12); // 75 % of the 8th
    }

    #[test]
    fn renders_sample_accurately_and_deterministically() {
        let notes: Vec<Note> = (0..200).map(|i| Note { t: i as f64 * 0.25, len: 0.2, key: 60.0 + (i % 12) as f32, vel: 0.8 }).collect();
        let part = Part { name: "t".into(), inst: Arc::new(Sine), notes, humanize: 0.0, seed: 1, straight: false };
        let grid = Grid { bpm: 120.0, ..Default::default() };
        let (l, _) = render_part(&part, &grid, 48_000, 48_000 * 30);
        // The first note starts exactly at frame 0, the second at 6000 (0.125 s).
        assert_eq!(l[0], 0.0);
        assert!(l[1] != 0.0);
        let (l2, _) = render_part(&part, &grid, 48_000, 48_000 * 30);
        assert_eq!(l, l2);
        let ch = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        let (l3, _) = ch.install(|| render_part(&part, &grid, 48_000, 48_000 * 30));
        assert_eq!(l, l3, "independent of thread count");
    }

    #[test]
    fn choke_groups_cut_open_hats() {
        let notes = vec![Note { t: 0.0, len: 4.0, key: 46.0, vel: 1.0 }, Note { t: 1.0, len: 0.1, key: 42.0, vel: 0.0 }];
        let part = Part { name: "h".into(), inst: Arc::new(Sine), notes, humanize: 0.0, seed: 1, straight: false };
        let grid = Grid::default();
        let (l, _) = render_part(&part, &grid, 48_000, 48_000 * 3);
        // After the choke (0.5 s) plus the kill window, the open hat is silent.
        assert!(l[30_000..].iter().all(|s| s.abs() < 1e-6));
        assert!(l[20_000].abs() > 0.0 || l[20_001].abs() > 0.0);
    }
}

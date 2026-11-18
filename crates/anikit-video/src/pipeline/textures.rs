//! Rasterised sub-composition textures for 3D (and any layer that samples a
//! composition as pixels).
//!
//! `CompTextures::get(comp, t, lod)` returns the composition's frame at
//! composition time `t` as a premultiplied RGBA8 [`Pixmap`], downscaled by
//! `2^lod`. Designed for thousands of instances that show the same composition
//! at different time offsets:
//!
//! - **Deterministic:** `t` snaps to the composition's frame grid (`floor(t·fps)`),
//!   so every thread, run and frame order produces the same pixels for the same
//!   request.
//! - **Shared:** identical frames (static spans, held last frames, loops)
//!   share one texture: entries are keyed by display-list hash, not by time.
//! - **Thread-safe, no duplicate work:** concurrent requests for the same
//!   texture wait for one render (`OnceLock`); different textures render in
//!   parallel, each on a pooled renderer.
//! - **Bounded:** least-recently-used textures are dropped beyond `cap_bytes`
//!   (an `Arc` already handed out stays valid).
//!
//! Keep one cache per root [`Evaluator`] (for example next to it for a whole
//! render): entries are keyed by the composition evaluator's address.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use vello_cpu::Pixmap;

use crate::dl::DisplayList;
use crate::eval::Evaluator;
use crate::render::Renderer;

type Cell = Arc<OnceLock<Arc<Pixmap>>>;

struct Entry {
    cell: Cell,
    bytes: usize,
    used: u64,
}

#[derive(Default)]
struct State {
    /// (composition, frame, lod) → display-list hash (cheap to keep: no pixels).
    keys: HashMap<(usize, u32, u8), u64>,
    /// (display-list hash, lod) → texture.
    tex: HashMap<(u64, u8), Entry>,
    clock: u64,
    bytes: usize,
    renders: u64,
}

/// A thread-safe, memory-capped cache of composition frames as textures.
pub struct CompTextures {
    cap: usize,
    state: Mutex<State>,
    pool: Mutex<Vec<(u64, Renderer)>>,
}

/// Counters for profiling and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextureStats {
    pub textures: usize,
    pub bytes: usize,
    /// Rasterisations so far (a hit costs one evaluation and a hash at most).
    pub renders: u64,
}

impl CompTextures {
    /// A cache holding at most about `cap_bytes` of pixels.
    pub fn new(cap_bytes: usize) -> Self {
        CompTextures { cap: cap_bytes, state: Mutex::new(State::default()), pool: Mutex::new(Vec::new()) }
    }

    /// The frame index `t` snaps to (the composition's own fps, clamped to its length).
    pub fn frame_index(comp: &Evaluator, t: f32) -> u32 {
        let n = comp.frames().max(1);
        ((t.max(0.0) * comp.fps + 1e-3).floor() as u32).min(n - 1)
    }

    /// Texture size at `lod` (each level halves, at least 1 px).
    pub fn size(comp: &Evaluator, lod: u8) -> (u32, u32) {
        let s = 0.5f32.powi(lod as i32);
        (((comp.width as f32 * s).round() as u32).max(1), ((comp.height as f32 * s).round() as u32).max(1))
    }

    /// `comp`'s frame at composition time `t` (map a layer's clock with
    /// [`crate::eval::CompTime::map`]), downscaled by `2^lod`; premultiplied RGBA8.
    pub fn get(&self, comp: &Evaluator, t: f32, lod: u8) -> Arc<Pixmap> {
        let k = Self::frame_index(comp, t);
        let id = comp as *const Evaluator as usize;
        let known = self.state.lock().unwrap().keys.get(&(id, k, lod)).copied();
        // Evaluate outside the lock; the list is needed to hash and maybe to render.
        let mut dl = None;
        let hash = known.unwrap_or_else(|| {
            let mut l = crate::eval::outside_scale_pass(|| comp.eval(k as f32 / comp.fps));
            super::scale_list(&mut l, 0.5f32.powi(lod as i32));
            let h = super::hash_list(&l);
            dl = Some(l);
            h
        });
        let cell = {
            let mut st = self.state.lock().unwrap();
            st.keys.insert((id, k, lod), hash);
            st.clock += 1;
            let now = st.clock;
            let e = st.tex.entry((hash, lod)).or_insert_with(|| Entry { cell: Cell::default(), bytes: 0, used: now });
            e.used = now;
            e.cell.clone()
        };
        let pix = cell
            .get_or_init(|| {
                let l = dl.take().unwrap_or_else(|| {
                    let mut l = crate::eval::outside_scale_pass(|| comp.eval(k as f32 / comp.fps));
                    super::scale_list(&mut l, 0.5f32.powi(lod as i32));
                    l
                });
                // Isolated: while this cell initialises, waiting on rayon must not
                // run another frame on this thread (it could ask for this cell).
                Arc::new(crate::scene3d::kernel::par::isolated(|| self.render(comp, &l)))
            })
            .clone();
        // Account for it and evict the least recently used beyond the cap.
        let mut st = self.state.lock().unwrap();
        let bytes = pix.width() as usize * pix.height() as usize * 4;
        if let Some(e) = st.tex.get_mut(&(hash, lod)) {
            if e.bytes == 0 {
                e.bytes = bytes;
                st.bytes += bytes;
                st.renders += 1;
            }
        }
        while st.bytes > self.cap && st.tex.len() > 1 {
            let Some((&victim, _)) = st.tex.iter().filter(|(k, e)| e.bytes > 0 && **k != (hash, lod)).min_by_key(|(_, e)| e.used) else { break };
            let e = st.tex.remove(&victim).unwrap();
            st.bytes -= e.bytes;
        }
        pix
    }

    pub fn stats(&self) -> TextureStats {
        let st = self.state.lock().unwrap();
        TextureStats { textures: st.tex.values().filter(|e| e.bytes > 0).count(), bytes: st.bytes, renders: st.renders }
    }

    fn render(&self, comp: &Evaluator, dl: &DisplayList) -> Pixmap {
        let version = (comp.assets.font_count() as u64) ^ ((Arc::as_ptr(&comp.assets) as u64) << 16);
        let pooled = {
            let mut p = self.pool.lock().unwrap();
            p.iter().position(|(v, _)| *v == version).map(|i| p.swap_remove(i).1)
        };
        let mut r = pooled.unwrap_or_else(|| {
            let mut r = Renderer::new(comp.assets.snapshot_fonts());
            let a = comp.assets.clone();
            r.set_image_source(Arc::new(move |id| a.pixmap(id)));
            r
        });
        let pix = r.render(dl).clone();
        self.pool.lock().unwrap().push((version, r));
        pix
    }
}

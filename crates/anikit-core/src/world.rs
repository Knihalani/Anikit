//! A fast 2D physics world: circles, static boxes, distance constraints
//! (ropes, cloth, soft bodies), attractors/repulsors, bounds.
//!
//! Design for speed:
//! * Struct-of-arrays `f32` storage — tight, cache-friendly loops that
//!   auto-vectorise; no per-body objects.
//! * Position-based dynamics with sub-stepping: unconditionally stable,
//!   handles stacking, constraints and collisions in one simple solver.
//! * Broadphase: a hashed uniform grid rebuilt every sub-step with a counting
//!   sort (O(n), zero allocations after warm-up) — no trees to rebalance.
//! * Restitution and friction applied as a velocity pass over the contact
//!   list recorded during the position solve.

use alloc::vec::Vec;

#[inline(always)]
fn sqrtf(x: f32) -> f32 {
    crate::math::sqrtf(x)
}
#[inline(always)]
fn floorf(x: f32) -> f32 {
    crate::math::floorf(x)
}

/// A distance constraint between two bodies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constraint {
    pub a: u32,
    pub b: u32,
    pub length: f32,
    /// 0–1, fraction of the error corrected per iteration.
    pub stiffness: f32,
    /// Break when stretched beyond `length * tear` (0 = never).
    pub tear: f32,
    pub alive: bool,
}

#[derive(Clone, Copy, Debug)]
struct Contact {
    i: u32,
    j: u32,
    nx: f32,
    ny: f32,
    /// Relative normal velocity before the solve (negative = approaching).
    vn: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attractor {
    pub x: f32,
    pub y: f32,
    /// Positive attracts, negative repels (units/s² at the centre).
    pub strength: f32,
    pub radius: f32,
}

/// Simulation settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldConfig {
    pub gravity: [f32; 2],
    /// Linear drag per second (0 = none).
    pub damping: f32,
    pub substeps: u32,
    /// Constraint solver iterations per sub-step.
    pub iterations: u32,
    /// Bounciness of body–body contacts (0–1).
    pub restitution: f32,
    /// Bounciness against bounds and boxes (0–1).
    pub wall_restitution: f32,
    /// Tangential friction on contact (0–1).
    pub friction: f32,
    /// `[min_x, min_y, max_x, max_y]`; infinite when disabled.
    pub bounds: [f32; 4],
}

impl Default for WorldConfig {
    fn default() -> Self {
        WorldConfig {
            gravity: [0.0, 980.0],
            damping: 0.05,
            substeps: 4,
            iterations: 2,
            restitution: 0.2,
            wall_restitution: 0.3,
            friction: 0.05,
            bounds: [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY],
        }
    }
}

pub struct World {
    pub config: WorldConfig,
    // Bodies (SoA).
    x: Vec<f32>,
    y: Vec<f32>,
    px: Vec<f32>,
    py: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
    r: Vec<f32>,
    inv_m: Vec<f32>,
    angle: Vec<f32>,
    active: Vec<u8>,
    /// Mass to restore when unpinned.
    mass: Vec<f32>,
    boxes: Vec<[f32; 4]>,
    constraints: Vec<Constraint>,
    attractors: Vec<Attractor>,
    // Broadphase.
    cell: f32,
    max_r: f32,
    table: Vec<u32>,
    items: Vec<u32>,
    hashes: Vec<u32>,
    contacts: Vec<Contact>,
    cursor: Vec<u32>,
    // Dense-grid (bounded worlds): bodies gathered in cell order.
    sx: Vec<f32>,
    sy: Vec<f32>,
    sr: Vec<f32>,
    sw: Vec<f32>,
    svx: Vec<f32>,
    svy: Vec<f32>,
    /// Render output: `[x, y, angle] * n` (f32).
    out: Vec<f32>,
    /// Pair tests performed in the last step (diagnostics).
    pub pair_tests: u64,
}

impl Default for World {
    fn default() -> Self {
        World::new(WorldConfig::default())
    }
}

#[inline(always)]
fn hash_cell(cx: i32, cy: i32, mask: u32) -> u32 {
    ((cx as u32).wrapping_mul(0x9E37_79B1) ^ (cy as u32).wrapping_mul(0x85EB_CA77)) & mask
}

impl World {
    pub fn new(config: WorldConfig) -> World {
        World {
            config,
            x: Vec::new(),
            y: Vec::new(),
            px: Vec::new(),
            py: Vec::new(),
            vx: Vec::new(),
            vy: Vec::new(),
            r: Vec::new(),
            inv_m: Vec::new(),
            angle: Vec::new(),
            active: Vec::new(),
            mass: Vec::new(),
            boxes: Vec::new(),
            constraints: Vec::new(),
            attractors: Vec::new(),
            cell: 1.0,
            max_r: 0.0,
            table: Vec::new(),
            items: Vec::new(),
            hashes: Vec::new(),
            contacts: Vec::new(),
            cursor: Vec::new(),
            sx: Vec::new(),
            sy: Vec::new(),
            sr: Vec::new(),
            sw: Vec::new(),
            svx: Vec::new(),
            svy: Vec::new(),
            out: Vec::new(),
            pair_tests: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.x.len()
    }
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// Add a circle; `mass <= 0` makes it static. Returns its index.
    pub fn add_body(&mut self, x: f32, y: f32, radius: f32, mass: f32) -> u32 {
        let r = radius.max(0.0);
        self.x.push(x);
        self.y.push(y);
        self.px.push(x);
        self.py.push(y);
        self.vx.push(0.0);
        self.vy.push(0.0);
        self.r.push(r);
        self.mass.push(mass);
        self.inv_m.push(if mass > 0.0 { 1.0 / mass } else { 0.0 });
        self.angle.push(0.0);
        self.active.push(1);
        self.out.extend_from_slice(&[x, y, 0.0]);
        if r > self.max_r {
            self.max_r = r;
        }
        (self.x.len() - 1) as u32
    }

    /// Add many bodies at once from `[x, y, r, mass] * n`.
    pub fn add_bodies(&mut self, data: &[f32]) -> u32 {
        let first = self.x.len() as u32;
        for c in data.chunks_exact(4) {
            self.add_body(c[0], c[1], c[2], c[3]);
        }
        first
    }

    pub fn set_position(&mut self, i: usize, x: f32, y: f32, keep_velocity: bool) {
        if i >= self.len() {
            return;
        }
        self.x[i] = x;
        self.y[i] = y;
        self.px[i] = x;
        self.py[i] = y;
        if let Some(o) = self.out.get_mut(i * 3..i * 3 + 2) {
            o[0] = x;
            o[1] = y;
        }
        if !keep_velocity {
            self.vx[i] = 0.0;
            self.vy[i] = 0.0;
        }
    }
    pub fn set_velocity(&mut self, i: usize, vx: f32, vy: f32) {
        if i < self.len() {
            self.vx[i] = vx;
            self.vy[i] = vy;
        }
    }
    pub fn velocity(&self, i: usize) -> [f32; 2] {
        if i < self.len() {
            [self.vx[i], self.vy[i]]
        } else {
            [0.0; 2]
        }
    }
    pub fn position(&self, i: usize) -> [f32; 2] {
        if i < self.len() {
            [self.x[i], self.y[i]]
        } else {
            [0.0; 2]
        }
    }
    /// Pin (static) or unpin a body.
    pub fn set_pinned(&mut self, i: usize, pinned: bool) {
        if i < self.len() {
            let m = self.mass[i];
            self.inv_m[i] = if pinned || m <= 0.0 { 0.0 } else { 1.0 / m };
        }
    }
    pub fn set_radius(&mut self, i: usize, r: f32) {
        if i < self.len() {
            self.r[i] = r.max(0.0);
            if r > self.max_r {
                self.max_r = r;
            }
        }
    }
    /// Remove a body from the simulation (its slot stays; index stays valid).
    pub fn set_active(&mut self, i: usize, on: bool) {
        if i < self.len() {
            self.active[i] = on as u8;
        }
    }
    /// Apply an instantaneous velocity change to every body near a point.
    pub fn impulse(&mut self, x: f32, y: f32, radius: f32, strength: f32) {
        let r2 = radius * radius;
        for i in 0..self.len() {
            if self.inv_m[i] == 0.0 || self.active[i] == 0 {
                continue;
            }
            let (dx, dy) = (self.x[i] - x, self.y[i] - y);
            let d2 = dx * dx + dy * dy;
            if d2 < r2 && d2 > 1e-6 {
                let d = sqrtf(d2);
                let f = strength * (1.0 - d / radius);
                self.vx[i] += dx / d * f;
                self.vy[i] += dy / d * f;
            }
        }
    }

    pub fn add_box(&mut self, min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> u32 {
        self.boxes.push([min_x.min(max_x), min_y.min(max_y), min_x.max(max_x), min_y.max(max_y)]);
        (self.boxes.len() - 1) as u32
    }
    pub fn clear_boxes(&mut self) {
        self.boxes.clear();
    }

    /// Distance constraint; `length < 0` uses the current distance.
    pub fn add_constraint(&mut self, a: u32, b: u32, length: f32, stiffness: f32, tear: f32) -> u32 {
        let len = if length < 0.0 {
            let (ai, bi) = (a as usize, b as usize);
            if ai >= self.len() || bi >= self.len() {
                0.0
            } else {
                let (dx, dy) = (self.x[bi] - self.x[ai], self.y[bi] - self.y[ai]);
                sqrtf(dx * dx + dy * dy)
            }
        } else {
            length
        };
        self.constraints.push(Constraint { a, b, length: len, stiffness: stiffness.clamp(0.0, 1.0), tear, alive: true });
        (self.constraints.len() - 1) as u32
    }
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    pub fn set_attractors(&mut self, a: &[Attractor]) {
        self.attractors.clear();
        self.attractors.extend_from_slice(a);
    }

    /// Index of the body under a point (topmost = last added), or `u32::MAX`.
    pub fn pick(&self, x: f32, y: f32) -> u32 {
        for i in (0..self.len()).rev() {
            if self.active[i] == 0 {
                continue;
            }
            let (dx, dy) = (self.x[i] - x, self.y[i] - y);
            if dx * dx + dy * dy <= self.r[i] * self.r[i] {
                return i as u32;
            }
        }
        u32::MAX
    }

    /// `[x, y, angle] * n` as `f32` (stable address between steps unless bodies are added).
    pub fn output(&self) -> &[f32] {
        &self.out
    }

    /// Rebuild the hashed grid (counting sort).
    fn build_grid(&mut self) {
        let n = self.len();
        let m = (n * 2).next_power_of_two().max(16);
        let mask = (m - 1) as u32;
        self.table.clear();
        self.table.resize(m + 1, 0);
        self.hashes.resize(n, 0);
        self.items.resize(n, 0);
        let inv = 1.0 / self.cell;
        for i in 0..n {
            if self.active[i] == 0 {
                self.hashes[i] = u32::MAX;
                continue;
            }
            let h = hash_cell(floorf(self.x[i] * inv) as i32, floorf(self.y[i] * inv) as i32, mask);
            self.hashes[i] = h;
            self.table[h as usize + 1] += 1;
        }
        for k in 0..m {
            self.table[k + 1] += self.table[k];
        }
        self.cursor.clear();
        self.cursor.extend_from_slice(&self.table[..m]);
        for i in 0..n {
            let h = self.hashes[i];
            if h == u32::MAX {
                continue;
            }
            let c = &mut self.cursor[h as usize];
            self.items[*c as usize] = i as u32;
            *c += 1;
        }
    }

    /// Dense grid size for bounded worlds, if reasonable.
    fn dense_dims(&self) -> Option<(usize, usize)> {
        let [x0, y0, x1, y1] = self.config.bounds;
        if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
            return None;
        }
        let gw = ((x1 - x0) / self.cell) as usize + 1;
        let gh = ((y1 - y0) / self.cell) as usize + 1;
        let cells = gw.checked_mul(gh)?;
        if cells > (self.len() * 8).max(4096) {
            return None;
        }
        Some((gw, gh))
    }

    /// Bounded worlds: counting-sort bodies by dense cell id, gather into
    /// sorted arrays, solve with a half stencil (each cell pair once), scatter.
    fn solve_collisions_dense(&mut self, gw: usize, gh: usize, record: bool) {
        let n = self.len();
        let [x0, y0, _, _] = self.config.bounds;
        let inv = 1.0 / self.cell;
        let cells = gw * gh;
        self.table.clear();
        self.table.resize(cells + 1, 0);
        self.hashes.resize(n, 0);
        self.items.resize(n, 0);
        for i in 0..n {
            if self.active[i] == 0 {
                self.hashes[i] = u32::MAX;
                continue;
            }
            let cx = (((self.x[i] - x0) * inv) as isize).clamp(0, gw as isize - 1) as usize;
            let cy = (((self.y[i] - y0) * inv) as isize).clamp(0, gh as isize - 1) as usize;
            let c = (cy * gw + cx) as u32;
            self.hashes[i] = c;
            self.table[c as usize + 1] += 1;
        }
        for k in 0..cells {
            self.table[k + 1] += self.table[k];
        }
        let total = self.table[cells] as usize;
        self.cursor.clear();
        self.cursor.extend_from_slice(&self.table[..cells]);
        for i in 0..n {
            let c = self.hashes[i];
            if c == u32::MAX {
                continue;
            }
            let slot = &mut self.cursor[c as usize];
            self.items[*slot as usize] = i as u32;
            *slot += 1;
        }
        // Gather.
        for v in [&mut self.sx, &mut self.sy, &mut self.sr, &mut self.sw, &mut self.svx, &mut self.svy] {
            v.resize(total, 0.0);
        }
        for k in 0..total {
            let i = self.items[k] as usize;
            self.sx[k] = self.x[i];
            self.sy[k] = self.y[i];
            self.sr[k] = self.r[i];
            self.sw[k] = self.inv_m[i];
            self.svx[k] = self.vx[i];
            self.svy[k] = self.vy[i];
        }
        let mut tests = 0u64;
        let (sx, sy, sr, sw) = (&mut self.sx, &mut self.sy, &self.sr, &self.sw);
        let (svx, svy) = (&self.svx, &self.svy);
        let contacts = &mut self.contacts;
        let items = &self.items;
        let mut pair = |a: usize, b: usize| {
            let dx = sx[b] - sx[a];
            let dy = sy[b] - sy[a];
            let rs = sr[a] + sr[b];
            let d2 = dx * dx + dy * dy;
            if d2 >= rs * rs || d2 < 1e-12 {
                return;
            }
            let (wa, wb) = (sw[a], sw[b]);
            let w = wa + wb;
            if w == 0.0 {
                return;
            }
            let d = sqrtf(d2);
            let (nx, ny) = (dx / d, dy / d);
            let o = (rs - d) / w;
            sx[a] -= nx * o * wa;
            sy[a] -= ny * o * wa;
            sx[b] += nx * o * wb;
            sy[b] += ny * o * wb;
            if record {
                let vn = (svx[b] - svx[a]) * nx + (svy[b] - svy[a]) * ny;
                contacts.push(Contact { i: items[a], j: items[b], nx, ny, vn });
            }
        };
        let t = &self.table;
        for cy in 0..gh {
            for cx in 0..gw {
                let c = cy * gw + cx;
                let (s, e) = (t[c] as usize, t[c + 1] as usize);
                if s == e {
                    continue;
                }
                for a in s..e {
                    for b in a + 1..e {
                        tests += 1;
                        pair(a, b);
                    }
                }
                // Half stencil: E, SW, S, SE.
                let mut nb = |nc: usize| {
                    let (ns, ne) = (t[nc] as usize, t[nc + 1] as usize);
                    for a in s..e {
                        for b in ns..ne {
                            tests += 1;
                            pair(a, b);
                        }
                    }
                };
                if cx + 1 < gw {
                    nb(c + 1);
                }
                if cy + 1 < gh {
                    if cx > 0 {
                        nb(c + gw - 1);
                    }
                    nb(c + gw);
                    if cx + 1 < gw {
                        nb(c + gw + 1);
                    }
                }
            }
        }
        // Scatter.
        for k in 0..total {
            let i = self.items[k] as usize;
            self.x[i] = self.sx[k];
            self.y[i] = self.sy[k];
        }
        self.pair_tests += tests;
    }

    fn solve_collisions(&mut self, record: bool) {
        let n = self.len();
        if n < 2 {
            return;
        }
        let m = self.table.len() - 1;
        let mask = (m - 1) as u32;
        let inv = 1.0 / self.cell;
        let mut tests = 0u64;
        for i in 0..n {
            if self.active[i] == 0 {
                continue;
            }
            let cx = floorf(self.x[i] * inv) as i32;
            let cy = floorf(self.y[i] * inv) as i32;
            let mut seen = [u32::MAX; 9];
            let mut ns = 0;
            for oy in -1..=1 {
                for ox in -1..=1 {
                    let h = hash_cell(cx + ox, cy + oy, mask);
                    if seen[..ns].contains(&h) {
                        continue;
                    }
                    seen[ns] = h;
                    ns += 1;
                    let (s, e) = (self.table[h as usize] as usize, self.table[h as usize + 1] as usize);
                    for k in s..e {
                        let j = self.items[k] as usize;
                        if j <= i {
                            continue;
                        }
                        tests += 1;
                        let dx = self.x[j] - self.x[i];
                        let dy = self.y[j] - self.y[i];
                        let rs = self.r[i] + self.r[j];
                        let d2 = dx * dx + dy * dy;
                        if d2 >= rs * rs || d2 < 1e-12 {
                            continue;
                        }
                        let (wi, wj) = (self.inv_m[i], self.inv_m[j]);
                        let w = wi + wj;
                        if w == 0.0 {
                            continue;
                        }
                        let d = sqrtf(d2);
                        let (nx, ny) = (dx / d, dy / d);
                        let overlap = (rs - d) / w;
                        self.x[i] -= nx * overlap * wi;
                        self.y[i] -= ny * overlap * wi;
                        self.x[j] += nx * overlap * wj;
                        self.y[j] += ny * overlap * wj;
                        if record {
                            let vn = (self.vx[j] - self.vx[i]) * nx + (self.vy[j] - self.vy[i]) * ny;
                            self.contacts.push(Contact { i: i as u32, j: j as u32, nx, ny, vn });
                        }
                    }
                }
            }
        }
        self.pair_tests += tests;
    }

    fn solve_constraints(&mut self) {
        for c in self.constraints.iter_mut() {
            if !c.alive {
                continue;
            }
            let (a, b) = (c.a as usize, c.b as usize);
            if a >= self.x.len() || b >= self.x.len() {
                continue;
            }
            let (wa, wb) = (self.inv_m[a], self.inv_m[b]);
            let w = wa + wb;
            if w == 0.0 {
                continue;
            }
            let dx = self.x[b] - self.x[a];
            let dy = self.y[b] - self.y[a];
            let d = sqrtf(dx * dx + dy * dy);
            if d < 1e-9 {
                continue;
            }
            if c.tear > 0.0 && d > c.length * c.tear {
                c.alive = false;
                continue;
            }
            let k = (d - c.length) / (d * w) * c.stiffness;
            self.x[a] += dx * k * wa;
            self.y[a] += dy * k * wa;
            self.x[b] -= dx * k * wb;
            self.y[b] -= dy * k * wb;
        }
    }

    /// Push bodies out of bounds and boxes; returns per-body wall normal via callback.
    fn solve_walls(&mut self, h: f32) {
        let [bx0, by0, bx1, by1] = self.config.bounds;
        let e_cfg = self.config.wall_restitution;
        // Slow impacts don't bounce (avoids perpetual micro-bounces at rest).
        let rest = 40.0 * h;
        let fr = 1.0 - self.config.friction;
        for i in 0..self.len() {
            if self.inv_m[i] == 0.0 || self.active[i] == 0 {
                continue;
            }
            let r = self.r[i];
            // Bounds: clamp position, reflect the implied velocity via `prev`.
            if self.x[i] - r < bx0 {
                let v = self.x[i] - self.px[i];
                let e = if v.abs() > rest { e_cfg } else { 0.0 };
                self.x[i] = bx0 + r;
                self.px[i] = self.x[i] + v * e;
                self.py[i] = self.y[i] - (self.y[i] - self.py[i]) * fr;
            } else if self.x[i] + r > bx1 {
                let v = self.x[i] - self.px[i];
                let e = if v.abs() > rest { e_cfg } else { 0.0 };
                self.x[i] = bx1 - r;
                self.px[i] = self.x[i] + v * e;
                self.py[i] = self.y[i] - (self.y[i] - self.py[i]) * fr;
            }
            if self.y[i] - r < by0 {
                let v = self.y[i] - self.py[i];
                let e = if v.abs() > rest { e_cfg } else { 0.0 };
                self.y[i] = by0 + r;
                self.py[i] = self.y[i] + v * e;
                self.px[i] = self.x[i] - (self.x[i] - self.px[i]) * fr;
            } else if self.y[i] + r > by1 {
                let v = self.y[i] - self.py[i];
                let e = if v.abs() > rest { e_cfg } else { 0.0 };
                self.y[i] = by1 - r;
                self.py[i] = self.y[i] + v * e;
                self.px[i] = self.x[i] - (self.x[i] - self.px[i]) * fr;
            }
            // Static boxes: push out along the closest-point normal.
            for bx in &self.boxes {
                let cx = self.x[i].clamp(bx[0], bx[2]);
                let cy = self.y[i].clamp(bx[1], bx[3]);
                let (dx, dy) = (self.x[i] - cx, self.y[i] - cy);
                let d2 = dx * dx + dy * dy;
                if d2 >= r * r {
                    continue;
                }
                let (nx, ny, pen) = if d2 > 1e-12 {
                    let d = sqrtf(d2);
                    (dx / d, dy / d, r - d)
                } else {
                    // Centre inside the box: exit through the nearest face.
                    let faces = [
                        (self.x[i] - bx[0], -1.0, 0.0),
                        (bx[2] - self.x[i], 1.0, 0.0),
                        (self.y[i] - bx[1], 0.0, -1.0),
                        (bx[3] - self.y[i], 0.0, 1.0),
                    ];
                    let mut best = faces[0];
                    for f in faces {
                        if f.0 < best.0 {
                            best = f;
                        }
                    }
                    (best.1, best.2, best.0 + r)
                };
                let (vx, vy) = (self.x[i] - self.px[i], self.y[i] - self.py[i]);
                self.x[i] += nx * pen;
                self.y[i] += ny * pen;
                let vn = vx * nx + vy * ny;
                let e = if vn.abs() > rest { e_cfg } else { 0.0 };
                if vn < 0.0 {
                    let (tx, ty) = (vx - vn * nx, vy - vn * ny);
                    let (nvx, nvy) = (tx * fr - vn * e * nx, ty * fr - vn * e * ny);
                    self.px[i] = self.x[i] - nvx;
                    self.py[i] = self.y[i] - nvy;
                }
            }
        }
    }

    /// Advance by `dt` seconds (clamped to 1/20 s to stay stable across tab switches).
    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        if dt == 0.0 || self.is_empty() {
            self.write_out();
            return;
        }
        let subs = self.config.substeps.max(1);
        let h = dt / subs as f32;
        let [gx, gy] = self.config.gravity;
        let drag = libm::expf(-self.config.damping * h);
        self.cell = (self.max_r * 2.0).max(1e-3);
        let record = self.config.restitution > 0.0 || self.config.friction > 0.0;
        let n = self.len();
        self.pair_tests = 0;
        for _ in 0..subs {
            // Integrate (vectorisable).
            for i in 0..n {
                if self.inv_m[i] == 0.0 || self.active[i] == 0 {
                    self.px[i] = self.x[i];
                    self.py[i] = self.y[i];
                    continue;
                }
                let mut ax = gx;
                let mut ay = gy;
                for a in &self.attractors {
                    let (dx, dy) = (a.x - self.x[i], a.y - self.y[i]);
                    let d2 = dx * dx + dy * dy;
                    if d2 < a.radius * a.radius && d2 > 1e-6 {
                        let d = sqrtf(d2);
                        let f = a.strength * (1.0 - d / a.radius);
                        ax += dx / d * f;
                        ay += dy / d * f;
                    }
                }
                self.vx[i] = (self.vx[i] + ax * h) * drag;
                self.vy[i] = (self.vy[i] + ay * h) * drag;
                self.px[i] = self.x[i];
                self.py[i] = self.y[i];
                self.x[i] += self.vx[i] * h;
                self.y[i] += self.vy[i] * h;
            }
            self.contacts.clear();
            if let Some((gw, gh)) = self.dense_dims() {
                self.solve_collisions_dense(gw, gh, record);
            } else {
                self.build_grid();
                self.solve_collisions(record);
            }
            for _ in 0..self.config.iterations.max(1) {
                self.solve_constraints();
            }
            self.solve_walls(h);
            // Velocities from positions.
            let inv_h = 1.0 / h;
            for i in 0..n {
                if self.inv_m[i] == 0.0 || self.active[i] == 0 {
                    continue;
                }
                self.vx[i] = (self.x[i] - self.px[i]) * inv_h;
                self.vy[i] = (self.y[i] - self.py[i]) * inv_h;
            }
            // Restitution + friction on recorded contacts.
            if record {
                let e = self.config.restitution;
                let fr = self.config.friction;
                for c in &self.contacts {
                    let (i, j) = (c.i as usize, c.j as usize);
                    let (wi, wj) = (self.inv_m[i], self.inv_m[j]);
                    let w = wi + wj;
                    if w == 0.0 {
                        continue;
                    }
                    let (rvx, rvy) = (self.vx[j] - self.vx[i], self.vy[j] - self.vy[i]);
                    let vn = rvx * c.nx + rvy * c.ny;
                    let target = if c.vn < -1.0 { -e * c.vn } else { 0.0 };
                    let dvn = (target - vn).max(0.0);
                    let (tx, ty) = (rvx - vn * c.nx, rvy - vn * c.ny);
                    let (ix, iy) = (dvn * c.nx - tx * fr, dvn * c.ny - ty * fr);
                    self.vx[i] -= ix * wi / w;
                    self.vy[i] -= iy * wi / w;
                    self.vx[j] += ix * wj / w;
                    self.vy[j] += iy * wj / w;
                }
            }
        }
        // Rolling angle from horizontal travel.
        for i in 0..n {
            if self.r[i] > 0.0 {
                self.angle[i] += self.vx[i] * dt / self.r[i];
            }
        }
        self.write_out();
    }

    fn write_out(&mut self) {
        let n = self.len();
        self.out.resize(n * 3, 0.0);
        for i in 0..n {
            self.out[i * 3] = self.x[i];
            self.out[i * 3 + 1] = self.y[i];
            self.out[i * 3 + 2] = self.angle[i];
        }
    }

    /// Total kinetic energy (diagnostics / rest detection).
    pub fn kinetic_energy(&self) -> f32 {
        let mut e = 0.0;
        for i in 0..self.len() {
            if self.inv_m[i] > 0.0 && self.active[i] != 0 {
                e += 0.5 / self.inv_m[i] * (self.vx[i] * self.vx[i] + self.vy[i] * self.vy[i]);
            }
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FNV-1a over a world's `[x, y, angle]` output and velocities.
    fn fold(h: &mut u32, w: &World) {
        let vel = (0..w.len()).flat_map(|i| w.velocity(i));
        for v in w.output().iter().copied().chain(vel) {
            *h = (*h ^ v.to_bits()).wrapping_mul(16_777_619);
        }
    }

    /// A fixed scenario covering the dense grid (bounded world: stacking,
    /// boxes, a tearing rope, a pinned body, an attractor, an impulse,
    /// restitution + friction) and the hashed grid (unbounded world). The
    /// hash was recorded on the current solver, so a future optimisation
    /// must reproduce its arithmetic exactly; `test/world-3d.test.ts` runs
    /// the same scenario through the wasm build.
    /// All inputs are exact in `f32` so the JS side passes identical values.
    fn golden_scenario() -> u32 {
        let dt = (1.0f64 / 60.0) as f32;
        let mut h = 0x811c_9dc5u32;
        let cfg = WorldConfig {
            gravity: [0.0, 980.0],
            damping: 0.0625,
            substeps: 4,
            iterations: 2,
            restitution: 0.25,
            wall_restitution: 0.375,
            friction: 0.125,
            bounds: [0.0, 0.0, 800.0, 600.0],
        };
        let mut w = World::new(cfg);
        for i in 0..600 {
            w.add_body((20 + (i % 30) * 25) as f32, (20 + (i / 30) * 25) as f32, (6 + (i * 7) % 5) as f32, (1 + i % 3) as f32);
        }
        w.add_box(300.0, 400.0, 500.0, 450.0);
        w.set_pinned(0, true);
        let first = w.add_bodies(&(0..20).flat_map(|k| [(100 + k * 10) as f32, 50.0, 3.0, 0.5]).collect::<Vec<f32>>());
        for k in 0..19 {
            w.add_constraint(first + k, first + k + 1, -1.0, 1.0, 3.0);
        }
        w.set_pinned(first as usize, true);
        w.set_attractors(&[Attractor { x: 400.0, y: 300.0, strength: 2000.0, radius: 150.0 }]);
        for step in 0..90 {
            if step == 30 {
                w.impulse(400.0, 300.0, 200.0, 500.0);
            }
            if step == 45 {
                w.set_velocity(10, 300.0, -200.0);
            }
            w.step(dt);
        }
        fold(&mut h, &w);
        let mut u = World::new(WorldConfig { gravity: [0.0, 200.0], damping: 0.0625, substeps: 2, iterations: 2, restitution: 0.25, wall_restitution: 0.375, friction: 0.125, ..Default::default() });
        for i in 0..400 {
            u.add_body(((i % 20) * 9) as f32, ((i / 20) * 9) as f32, 5.0, 1.0);
        }
        for _ in 0..60 {
            u.step(dt);
        }
        fold(&mut h, &u);
        h
    }

    #[test]
    fn solver_matches_recorded_golden_hash() {
        assert_eq!(golden_scenario(), WORLD_GOLDEN);
    }
    const WORLD_GOLDEN: u32 = 108_870_945;

    fn boxed_world() -> World {
        World::new(WorldConfig { bounds: [0.0, 0.0, 400.0, 400.0], ..Default::default() })
    }

    #[test]
    fn falls_and_rests_on_floor() {
        let mut w = boxed_world();
        let b = w.add_body(200.0, 50.0, 10.0, 1.0) as usize;
        for _ in 0..600 {
            w.step(1.0 / 60.0);
        }
        let [_, y] = w.position(b);
        assert!((y - 390.0).abs() < 0.5, "{y}");
        assert!(w.velocity(b)[1].abs() < 5.0);
    }

    #[test]
    fn comes_fully_to_rest() {
        let mut w = boxed_world();
        let b = w.add_body(100.0, 100.0, 10.0, 1.0) as usize;
        for _ in 0..900 {
            w.step(1.0 / 60.0);
        }
        assert!(w.velocity(b)[1].abs() < 1e-3, "{:?}", w.velocity(b));
        w.set_position(b, 50.0, 60.0, false);
        assert_eq!(&w.output()[..2], &[50.0, 60.0]);
    }

    #[test]
    fn no_overlap_in_a_pile() {
        let mut w = boxed_world();
        for i in 0..400 {
            w.add_body(20.0 + (i % 20) as f32 * 18.0, 20.0 + (i / 20) as f32 * 9.0, 4.0 + (i % 3) as f32, 1.0);
        }
        for _ in 0..600 {
            w.step(1.0 / 60.0);
        }
        // Allow small residual penetration from the iterative solver.
        let mut worst: f32 = 0.0;
        for i in 0..w.len() {
            for j in i + 1..w.len() {
                let [xi, yi] = w.position(i);
                let [xj, yj] = w.position(j);
                let d = ((xi - xj).powi(2) + (yi - yj).powi(2)).sqrt();
                worst = worst.max(w.r[i] + w.r[j] - d);
            }
            let [x, y] = w.position(i);
            assert!(x >= -0.01 && x <= 400.01 && y >= -0.01 && y <= 400.01);
        }
        assert!(worst < 2.0, "max penetration {worst}");
    }

    #[test]
    fn rope_hangs_from_a_pin() {
        let mut w = World::new(WorldConfig { iterations: 8, damping: 2.0, ..Default::default() });
        let mut prev = w.add_body(0.0, 0.0, 2.0, 0.0);
        for k in 1..20 {
            let b = w.add_body(k as f32 * 10.0, 0.0, 2.0, 1.0);
            w.add_constraint(prev, b, 10.0, 1.0, 0.0);
            prev = b;
        }
        for _ in 0..900 {
            w.step(1.0 / 60.0);
        }
        let [x, y] = w.position(19);
        assert!(x.abs() < 10.0 && y > 150.0, "{x} {y}");
        // Lengths are (nearly) preserved.
        for c in w.constraints() {
            let [ax, ay] = w.position(c.a as usize);
            let [bx, by] = w.position(c.b as usize);
            let d = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
            assert!((d - 10.0).abs() < 1.5, "{d}");
        }
    }

    #[test]
    fn restitution_bounces_and_boxes_block() {
        let mut w = World::new(WorldConfig { gravity: [0.0, 0.0], damping: 0.0, wall_restitution: 1.0, friction: 0.0, bounds: [0.0, 0.0, 100.0, 100.0], ..Default::default() });
        let b = w.add_body(50.0, 50.0, 5.0, 1.0) as usize;
        w.set_velocity(b, 300.0, 0.0);
        for _ in 0..12 {
            w.step(1.0 / 60.0);
        }
        assert!(w.velocity(b)[0] < -250.0, "{:?}", w.velocity(b));
        let mut g = boxed_world();
        g.add_box(150.0, 200.0, 250.0, 220.0);
        let c = g.add_body(200.0, 100.0, 10.0, 1.0) as usize;
        for _ in 0..300 {
            g.step(1.0 / 60.0);
        }
        assert!((g.position(c)[1] - 190.0).abs() < 1.0, "{:?}", g.position(c));
    }

    #[test]
    fn attractor_pulls_and_pick_finds() {
        let mut w = World::new(WorldConfig { gravity: [0.0, 0.0], ..Default::default() });
        let b = w.add_body(100.0, 0.0, 5.0, 1.0);
        w.set_attractors(&[Attractor { x: 0.0, y: 0.0, strength: 5000.0, radius: 500.0 }]);
        w.step(1.0 / 60.0);
        assert!(w.velocity(b as usize)[0] < 0.0);
        assert_eq!(w.pick(w.position(0)[0], 0.0), 0);
        assert_eq!(w.pick(1000.0, 1000.0), u32::MAX);
    }
}

//! A small, fast software 3D renderer: z-buffered, lit, instanced triangles.
//!
//! Meshes (positions, normals, triangle indices) are drawn many times per
//! call with per-instance affine transforms and colours. Shading is
//! per-vertex Blinn–Phong with ambient, a key light, a fill light, a
//! fresnel rim and depth fog, interpolated across each triangle and depth
//! tested per pixel. Output is RGBA words over a clear colour, ready to
//! composite with particles ([`crate::splat`]) and type.

use alloc::vec;
use alloc::vec::Vec;

/// Column-major 4×4 matrix (same layout as [`crate::space::Mat4`], in f32).
pub type M4 = [f32; 16];

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub ambient: f32,
    pub diffuse: f32,
    pub specular: f32,
    pub shininess: f32,
    /// Fresnel-style edge light.
    pub rim: f32,
    /// Self-illumination: base colour × emissive is added unlit (glows under bloom).
    pub emissive: f32,
    /// Mirror reflection of the procedural environment (0 = matte, 1 = chrome).
    pub metal: f32,
    /// Thin-film rainbow sheen that shifts with viewing angle, and its hue offset.
    pub iridescence: f32,
    pub hue: f32,
}

impl Default for Material {
    fn default() -> Self {
        Self { ambient: 0.22, diffuse: 0.85, specular: 0.5, shininess: 40.0, rim: 0.35, emissive: 0.0, metal: 0.0, iridescence: 0.0, hue: 0.0 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Lighting {
    /// Direction *towards* the key light (normalised internally) and its colour.
    pub key: [f32; 3],
    pub key_color: [f32; 3],
    /// Fill light direction and colour (cheap second light).
    pub fill: [f32; 3],
    pub fill_color: [f32; 3],
    pub rim_color: [f32; 3],
    /// Fog: start and end distance from the camera, and colour (0…1).
    pub fog: [f32; 2],
    pub fog_color: [f32; 3],
    /// Environment for reflections: sky (up), horizon band, ground (down).
    pub sky: [f32; 3],
    pub horizon: [f32; 3],
    pub ground: [f32; 3],
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            key: [0.4, 0.8, 0.6],
            key_color: [1.0, 1.0, 1.0],
            fill: [-0.6, -0.2, 0.4],
            fill_color: [0.25, 0.28, 0.35],
            rim_color: [1.0, 1.0, 1.0],
            fog: [0.0, f32::INFINITY],
            fog_color: [0.0; 3],
            sky: [0.25, 0.3, 0.55],
            horizon: [1.0, 0.85, 0.7],
            ground: [0.05, 0.05, 0.07],
        }
    }
}

/// One mesh: `pos`/`nrm` are xyz per vertex, `idx` three per triangle.
#[derive(Debug, Default, Clone)]
pub struct Mesh {
    pub pos: Vec<f32>,
    pub nrm: Vec<f32>,
    pub idx: Vec<u32>,
}

/// Per-instance data: a 3×4 affine (column-major, `[r00 r10 r20 r01 r11 r21 r02 r12 r22 tx ty tz]`) and RGB.
pub const INSTANCE_STRIDE: usize = 15;

#[derive(Debug, Default)]
pub struct Raster {
    pub width: usize,
    pub height: usize,
    /// RGBA words (little endian), `width * height`.
    pub color: Vec<u32>,
    depth: Vec<f32>,
    // Scratch per mesh draw.
    sx: Vec<f32>,
    sy: Vec<f32>,
    sz: Vec<f32>,
    sw: Vec<f32>,
    col: Vec<[f32; 3]>,
    order: Vec<u32>,
    keys: Vec<f32>,
    /// Triangles drawn by the last call (diagnostics).
    pub drawn: usize,
}

#[inline(always)]
fn norm3(v: [f32; 3]) -> [f32; 3] {
    let l = crate::math::sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}
#[inline(always)]
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline(always)]
fn pack(c: [f32; 3]) -> u32 {
    // Float→int casts saturate in Rust, so only the upper bound needs a min.
    let q = |v: f32| ((v * 255.0 + 0.5) as u32).min(255);
    q(c[0]) | (q(c[1]) << 8) | (q(c[2]) << 16) | 0xff00_0000
}

impl Raster {
    pub fn new(w: usize, h: usize) -> Self {
        let mut r = Self::default();
        r.resize(w, h);
        r
    }
    pub fn resize(&mut self, w: usize, h: usize) {
        self.width = w.max(1);
        self.height = h.max(1);
        self.color = vec![0; self.width * self.height];
        self.depth = vec![f32::INFINITY; self.width * self.height];
    }
    /// Clear colour (0xRRGGBB, or alpha in the top byte for a transparent clear) and depth.
    pub fn clear(&mut self, rgb: u32, alpha: u8) {
        let c = ((rgb >> 16) & 255) | (rgb & 0xff00) | ((rgb & 255) << 16) | ((alpha as u32) << 24);
        self.color.iter_mut().for_each(|p| *p = c);
        self.depth.iter_mut().for_each(|d| *d = f32::INFINITY);
    }

    /// Draw `mesh` once per instance. `vp` is view × projection, `eye` the
    /// camera position (for specular, rim and fog).
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::needless_range_loop)]
    pub fn draw(&mut self, mesh: &Mesh, instances: &[f32], vp: &M4, eye: [f32; 3], mat: &Material, light: &Lighting, cull_back: bool) {
        let nv = mesh.pos.len() / 3;
        let (w, h) = (self.width as f32, self.height as f32);
        let (hw, hh) = (w * 0.5, h * 0.5);
        let key = norm3(light.key);
        let fill = norm3(light.fill);
        let fog_span = (light.fog[1] - light.fog[0]).max(1e-3);
        let fog_on = light.fog[1].is_finite();
        self.sx.resize(nv, 0.0);
        self.sy.resize(nv, 0.0);
        self.sz.resize(nv, 0.0);
        self.sw.resize(nv, 0.0);
        self.col.resize(nv, [0.0; 3]);
        self.drawn = 0;
        // Front-to-back: nearer instances fill the depth buffer first, so the
        // depth test rejects hidden pixels before any shading or stores.
        let count = instances.len() / INSTANCE_STRIDE;
        self.order.clear();
        self.order.extend(0..count as u32);
        if count > 1 {
            let d2 = |i: u32| {
                let b = i as usize * INSTANCE_STRIDE;
                let (dx, dy, dz) = (instances[b + 9] - eye[0], instances[b + 10] - eye[1], instances[b + 11] - eye[2]);
                dx * dx + dy * dy + dz * dz
            };
            self.keys.clear();
            self.keys.extend(self.order.iter().map(|&i| d2(i)));
            let keys = &self.keys;
            self.order.sort_unstable_by(|a, b| keys[*a as usize].partial_cmp(&keys[*b as usize]).unwrap_or(core::cmp::Ordering::Equal));
        }
        for oi in 0..count {
            let ii = self.order[oi] as usize;
            let inst = &instances[ii * INSTANCE_STRIDE..(ii + 1) * INSTANCE_STRIDE];
            let m = &inst[..12];
            let base = [inst[12], inst[13], inst[14]];
            // Transform + light + project every vertex of this instance.
            for v in 0..nv {
                let (px, py, pz) = (mesh.pos[v * 3], mesh.pos[v * 3 + 1], mesh.pos[v * 3 + 2]);
                let wx = m[0] * px + m[3] * py + m[6] * pz + m[9];
                let wy = m[1] * px + m[4] * py + m[7] * pz + m[10];
                let wz = m[2] * px + m[5] * py + m[8] * pz + m[11];
                let (nx, ny, nz) = (mesh.nrm[v * 3], mesh.nrm[v * 3 + 1], mesh.nrm[v * 3 + 2]);
                let n = norm3([m[0] * nx + m[3] * ny + m[6] * nz, m[1] * nx + m[4] * ny + m[7] * nz, m[2] * nx + m[5] * ny + m[8] * nz]);
                let view = [eye[0] - wx, eye[1] - wy, eye[2] - wz];
                let dist = crate::math::sqrtf(dot(view, view)).max(1e-6);
                let vd = [view[0] / dist, view[1] / dist, view[2] / dist];
                let dk = dot(n, key).max(0.0);
                let df = dot(n, fill).max(0.0);
                let hv = norm3([key[0] + vd[0], key[1] + vd[1], key[2] + vd[2]]);
                let sp = if dk > 0.0 {
                    // Schlick's cheap power: x / (s − s·x + x) ≈ x^s.
                    let x = dot(n, hv).max(0.0);
                    x / (mat.shininess - mat.shininess * x + x) * mat.specular
                } else {
                    0.0
                };
                let fr = 1.0 - dot(n, vd).abs();
                let rim = fr * fr * fr * mat.rim;
                let mut c = [0.0f32; 3];
                for k in 0..3 {
                    c[k] = base[k] * (mat.ambient + mat.diffuse * (dk * light.key_color[k] + df * light.fill_color[k] + mat.emissive)) + sp * light.key_color[k] + rim * light.rim_color[k];
                }
                if mat.metal > 0.0 {
                    // Reflect the view ray off the surface and look up a sky/horizon/ground gradient.
                    let ndv = dot(n, vd);
                    let ry = 2.0 * ndv * n[1] - vd[1];
                    let up = ry.clamp(-1.0, 1.0);
                    let band = libm::expf(-up * up * 18.0);
                    let env: [f32; 3] = core::array::from_fn(|k| {
                        let sg = if up > 0.0 { light.sky[k] } else { light.ground[k] };
                        sg + (light.horizon[k] - sg) * band
                    });
                    for k in 0..3 {
                        c[k] += (env[k] * (0.35 + 0.65 * base[k]) - c[k]) * mat.metal;
                    }
                }
                if mat.iridescence > 0.0 {
                    // Thin-film sheen: a cosine palette indexed by the viewing angle.
                    let x = (1.0 - dot(n, vd).abs()) * 1.6 + mat.hue;
                    let tau = core::f32::consts::TAU;
                    for (k, off) in [0.0f32, 0.33, 0.67].iter().enumerate() {
                        c[k] += (0.5 + 0.5 * libm::cosf(tau * (x + off))) * mat.iridescence;
                    }
                }
                if fog_on {
                    let f = ((dist - light.fog[0]) / fog_span).clamp(0.0, 1.0);
                    for k in 0..3 {
                        c[k] += (light.fog_color[k] - c[k]) * f;
                    }
                }
                self.col[v] = c;
                let cx = vp[0] * wx + vp[4] * wy + vp[8] * wz + vp[12];
                let cy = vp[1] * wx + vp[5] * wy + vp[9] * wz + vp[13];
                let cz = vp[2] * wx + vp[6] * wy + vp[10] * wz + vp[14];
                let cw = vp[3] * wx + vp[7] * wy + vp[11] * wz + vp[15];
                self.sw[v] = cw;
                if cw > 1e-4 {
                    let iw = 1.0 / cw;
                    self.sx[v] = hw + cx * iw * hw;
                    self.sy[v] = hh - cy * iw * hh;
                    self.sz[v] = cz * iw;
                }
            }
            for t in mesh.idx.chunks_exact(3) {
                let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
                // Reject triangles that touch the near plane (cheap; the scenes keep geometry in front).
                if self.sw[a] <= 1e-4 || self.sw[b] <= 1e-4 || self.sw[c] <= 1e-4 {
                    continue;
                }
                if self.sz[a] < -1.0 || self.sz[b] < -1.0 || self.sz[c] < -1.0 {
                    continue;
                }
                self.tri(a, b, c, cull_back);
            }
        }
    }

    #[inline]
    fn tri(&mut self, a: usize, b: usize, c: usize, cull_back: bool) {
        let (x0, y0, x1, y1, x2, y2) = (self.sx[a], self.sy[a], self.sx[b], self.sy[b], self.sx[c], self.sy[c]);
        let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
        if area.abs() < 1e-6 || (cull_back && area > 0.0) {
            return;
        }
        let (w, h) = (self.width as isize, self.height as isize);
        let minx = (crate::math::floorf(x0.min(x1).min(x2)) as isize).max(0);
        let maxx = (crate::math::ceilf(x0.max(x1).max(x2)) as isize).min(w - 1);
        let miny = (crate::math::floorf(y0.min(y1).min(y2)) as isize).max(0);
        let maxy = (crate::math::ceilf(y0.max(y1).max(y2)) as isize).min(h - 1);
        if minx > maxx || miny > maxy {
            return;
        }
        // Huge triangles (projected far off-screen) are skipped rather than walked.
        if (maxx - minx) as usize * (maxy - miny) as usize > (self.width * self.height) * 2 {
            return;
        }
        self.drawn += 1;
        let inv = 1.0 / area;
        let (z0, z1, z2) = (self.sz[a], self.sz[b], self.sz[c]);
        let (c0, c1, c2) = (self.col[a], self.col[b], self.col[c]);
        // Edge functions; per row, solve for the covered span directly, then
        // walk it with incrementally interpolated depth and colour.
        let e = |ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32| (bx - ax) * (py - ay) - (by - ay) * (px - ax);
        let sgn = if area > 0.0 { 1.0 } else { -1.0 };
        let d = [-(y2 - y1) * sgn, -(y0 - y2) * sgn, -(y1 - y0) * sgn];
        let dz = (d[0] * z0 + d[1] * z1 + d[2] * z2) * inv * sgn;
        let dc: [f32; 3] = core::array::from_fn(|k| (d[0] * c0[k] + d[1] * c1[k] + d[2] * c2[k]) * inv * sgn);
        let span = (maxx - minx) as f32;
        for py in miny..=maxy {
            let fy = py as f32 + 0.5;
            let fx = minx as f32 + 0.5;
            let w = [e(x1, y1, x2, y2, fx, fy) * sgn, e(x2, y2, x0, y0, fx, fy) * sgn, e(x0, y0, x1, y1, fx, fy) * sgn];
            let (mut lo, mut hi) = (0.0f32, span);
            let mut empty = false;
            for k in 0..3 {
                if d[k] == 0.0 {
                    if w[k] < 0.0 {
                        empty = true;
                    }
                } else if d[k] > 0.0 {
                    lo = lo.max(crate::math::ceilf(-w[k] / d[k]));
                } else {
                    hi = hi.min(crate::math::floorf(-w[k] / d[k]));
                }
            }
            if empty || lo > hi {
                continue;
            }
            let k0 = lo;
            let (b0, b1, b2) = ((w[0] + d[0] * k0) * sgn * inv, (w[1] + d[1] * k0) * sgn * inv, (w[2] + d[2] * k0) * sgn * inv);
            let mut z = b0 * z0 + b1 * z1 + b2 * z2;
            let mut c = [b0 * c0[0] + b1 * c1[0] + b2 * c2[0], b0 * c0[1] + b1 * c1[1] + b2 * c2[1], b0 * c0[2] + b1 * c1[2] + b2 * c2[2]];
            let row = py as usize * self.width + minx as usize;
            let (a0, a1) = (row + lo as usize, row + hi as usize);
            for i in a0..=a1 {
                if z < self.depth[i] {
                    self.depth[i] = z;
                    self.color[i] = pack(c);
                }
                z += dz;
                c[0] += dc[0];
                c[1] += dc[1];
                c[2] += dc[2];
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Mesh builders
// ---------------------------------------------------------------------------

/// Unit cube (−0.5…0.5) with flat per-face normals.
#[allow(clippy::needless_range_loop)]
pub fn cube() -> Mesh {
    let mut m = Mesh::default();
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];
    for (n, u, v) in faces {
        let b = (m.pos.len() / 3) as u32;
        for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            for k in 0..3 {
                m.pos.push(n[k] * 0.5 + u[k] * 0.5 * su + v[k] * 0.5 * sv);
            }
            m.nrm.extend_from_slice(&n);
        }
        m.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    m
}

/// A tube of radius `r` swept along a closed curve given by `f(t)`, t ∈ [0, 1).
#[allow(clippy::needless_range_loop)]
pub fn tube(f: impl Fn(f32) -> [f32; 3], r: f32, segments: usize, sides: usize) -> Mesh {
    let mut m = Mesh::default();
    let tau = core::f32::consts::TAU;
    let pts: Vec<[f32; 3]> = (0..segments).map(|i| f(i as f32 / segments as f32)).collect();
    // Parallel-transport frames, then spread the closing twist evenly along
    // the curve so the last ring lines up with the first (no seam).
    let tangent = |i: usize| {
        let q = pts[(i + 1) % segments];
        let o = pts[(i + segments - 1) % segments];
        norm3([q[0] - o[0], q[1] - o[1], q[2] - o[2]])
    };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let transport = |n: [f32; 3], t: [f32; 3]| {
        let d = dot(n, t);
        let v = [n[0] - d * t[0], n[1] - d * t[1], n[2] - d * t[2]];
        if dot(v, v) < 1e-8 { norm3([t[1], -t[0], 0.0]) } else { norm3(v) }
    };
    let t0 = tangent(0);
    let seed_n = if t0[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let mut normals = Vec::with_capacity(segments);
    let mut n = transport(seed_n, t0);
    for i in 0..segments {
        if i > 0 {
            n = transport(n, tangent(i));
        }
        normals.push(n);
    }
    // Angle between the frame carried once around the loop and the first frame.
    let carried = transport(normals[segments - 1], t0);
    let b0 = cross(t0, normals[0]);
    let twist = libm::atan2f(dot(carried, b0), dot(carried, normals[0]));
    for i in 0..segments {
        let t = tangent(i);
        let n0 = normals[i];
        let b = cross(t, n0);
        let a = -twist * i as f32 / segments as f32;
        let (ca, sa) = (libm::cosf(a), libm::sinf(a));
        let n = [n0[0] * ca + b[0] * sa, n0[1] * ca + b[1] * sa, n0[2] * ca + b[2] * sa];
        let bn = cross(t, n);
        let p = pts[i];
        for j in 0..sides {
            let ang = j as f32 / sides as f32 * tau;
            let (cj, sj) = (libm::cosf(ang), libm::sinf(ang));
            let dir = [n[0] * cj + bn[0] * sj, n[1] * cj + bn[1] * sj, n[2] * cj + bn[2] * sj];
            for k in 0..3 {
                m.pos.push(p[k] + dir[k] * r);
            }
            m.nrm.extend_from_slice(&dir);
        }
    }
    for i in 0..segments {
        let i2 = (i + 1) % segments;
        for j in 0..sides {
            let j2 = (j + 1) % sides;
            let (a, b, c, d) = ((i * sides + j) as u32, (i2 * sides + j) as u32, (i2 * sides + j2) as u32, (i * sides + j2) as u32);
            m.idx.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    m
}

/// A (p, q) torus knot as a tube.
pub fn torus_knot(p: f32, q: f32, big_r: f32, r: f32, tube_r: f32, segments: usize, sides: usize) -> Mesh {
    tube(
        move |t| {
            let a = t * core::f32::consts::TAU;
            let k = big_r + r * libm::cosf(q * a);
            [k * libm::cosf(p * a), k * libm::sinf(p * a), r * 1.6 * libm::sinf(q * a)]
        },
        tube_r,
        segments,
        sides,
    )
}

/// A torus in the XY plane.
pub fn torus(big_r: f32, r: f32, segments: usize, sides: usize) -> Mesh {
    tube(move |t| { let a = t * core::f32::consts::TAU; [big_r * libm::cosf(a), big_r * libm::sinf(a), 0.0] }, r, segments, sides)
}

/// UV sphere of radius 1.
pub fn sphere(rings: usize, segments: usize) -> Mesh {
    let mut m = Mesh::default();
    let pi = core::f32::consts::PI;
    for i in 0..=rings {
        let th = i as f32 / rings as f32 * pi;
        for j in 0..=segments {
            let ph = j as f32 / segments as f32 * 2.0 * pi;
            let n = [libm::sinf(th) * libm::cosf(ph), libm::cosf(th), libm::sinf(th) * libm::sinf(ph)];
            m.pos.extend_from_slice(&n);
            m.nrm.extend_from_slice(&n);
        }
    }
    let s = segments + 1;
    for i in 0..rings {
        for j in 0..segments {
            let (a, b) = ((i * s + j) as u32, ((i + 1) * s + j) as u32);
            m.idx.extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{look_at, mul, perspective};

    fn vp() -> M4 {
        let m = mul(&perspective(45.0, 1.0, 1.0, 100.0), &look_at([0.0, 0.0, 5.0], [0.0; 3], [0.0, 1.0, 0.0]));
        core::array::from_fn(|i| m[i] as f32)
    }
    const ID: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

    #[test]
    fn draws_a_lit_cube_with_depth() {
        let mut r = Raster::new(64, 64);
        r.clear(0x000000, 255);
        let mut inst = ID.to_vec();
        inst.extend_from_slice(&[1.0, 0.0, 0.0]);
        r.draw(&cube(), &inst, &vp(), [0.0, 0.0, 5.0], &Material::default(), &Lighting::default(), true);
        let centre = r.color[32 * 64 + 32];
        assert!(centre & 0xff > 40, "lit red face {:x}", centre);
        assert_eq!(r.color[0] & 0xffffff, 0, "background stays clear");
        assert!(r.drawn >= 2 && r.drawn <= 6, "back faces culled: {}", r.drawn);
        // A second cube behind the first is hidden by the depth test.
        let before = r.color[32 * 64 + 32];
        let mut back = ID.to_vec();
        back[11] = -2.0;
        back.extend_from_slice(&[0.0, 1.0, 0.0]);
        r.draw(&cube(), &back, &vp(), [0.0, 0.0, 5.0], &Material::default(), &Lighting::default(), true);
        assert_eq!(r.color[32 * 64 + 32], before);
    }

    #[test]
    fn mesh_builders_are_consistent() {
        for m in [cube(), sphere(8, 12), tube(|t| [libm::cosf(t * 6.283), libm::sinf(t * 6.283), 0.0], 0.2, 32, 8)] {
            assert_eq!(m.pos.len(), m.nrm.len());
            assert_eq!(m.idx.len() % 3, 0);
            assert!(m.idx.iter().all(|&i| (i as usize) < m.pos.len() / 3));
        }
    }
}

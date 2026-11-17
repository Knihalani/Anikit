//! The `scene3d` renderer: a ray tracer with physically based shading.
//!
//! * **Visibility:** primary rays per pixel, adaptively supersampled (4
//!   samples, 16 where the 4 disagree), resolved in a tone-mapped weighted
//!   space so bright highlights on edges don't alias.
//! * **Shading:** GGX/Schlick split-sum specular from the analytic,
//!   pre-filtered studio environment ([`super::env`]); mirror and glossy
//!   reflections of the scene itself are traced (up to `max_depth` bounces),
//!   so chrome reflects the other letters and the floor.
//! * **Shadows:** the key softbox is an area light sampled with shadow rays
//!   (soft, contact-hardening shadows); ambient occlusion is traced too.
//!   On the floor both are baked once per static layout into a smooth texture
//!   ([`FloorVis`]), so the floor never shows sampling noise.
//! * **Floor:** a glossy dark plane whose reflection is kept in its own buffer
//!   and blurred by the distance to what it reflects (contact-sharp, softer
//!   further away), fading into the backdrop with distance.
//!
//! The image then goes through [`super::post`]: depth of field, motion blur,
//! bloom, ACES tone mapping and dithering.

use super::bvh::Bvh;
use super::env::Env;
use super::math::*;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Material {
    /// Linear base colour.
    pub base: V3,
    pub metallic: f32,
    pub roughness: f32,
    /// Dielectric reflectance scale (1 = F0 0.04).
    pub specular: f32,
    pub emissive: V3,
    /// Clear lacquer on top (0..1).
    pub clearcoat: f32,
}

impl Material {
    pub fn chrome() -> Material {
        Material { base: v3(0.94, 0.94, 0.96), metallic: 1.0, roughness: 0.06, specular: 1.0, emissive: v3(0.0, 0.0, 0.0), clearcoat: 0.0 }
    }
}

pub struct Object {
    pub bvh: Arc<Bvh>,
    pub xf: Xf,
    /// Transform one frame earlier (for motion vectors).
    pub prev: Xf,
    pub mat: Material,
}

/// Baked floor lighting: key-light visibility and ambient occlusion over a
/// rectangle of the floor.
pub struct FloorVis {
    pub x0: f32,
    pub z0: f32,
    pub x1: f32,
    pub z1: f32,
    pub nx: usize,
    pub nz: usize,
    /// (key visibility, ambient occlusion) per texel.
    pub data: Vec<[f32; 2]>,
}

impl FloorVis {
    #[inline]
    pub fn sample(&self, x: f32, z: f32) -> [f32; 2] {
        let fx = (x - self.x0) / (self.x1 - self.x0) * self.nx as f32 - 0.5;
        let fz = (z - self.z0) / (self.z1 - self.z0) * self.nz as f32 - 0.5;
        if fx < -1.0 || fz < -1.0 || fx > self.nx as f32 || fz > self.nz as f32 {
            return [1.0, 1.0];
        }
        let (ix, iz) = (fx.floor(), fz.floor());
        let (tx, tz) = (fx - ix, fz - iz);
        let at = |i: i32, k: i32| -> [f32; 2] {
            if i < 0 || k < 0 || i >= self.nx as i32 || k >= self.nz as i32 {
                [1.0, 1.0]
            } else {
                self.data[k as usize * self.nx + i as usize]
            }
        };
        let (i, k) = (ix as i32, iz as i32);
        let (a, b, c, d) = (at(i, k), at(i + 1, k), at(i, k + 1), at(i + 1, k + 1));
        let l = |j: usize| (a[j] * (1.0 - tx) + b[j] * tx) * (1.0 - tz) + (c[j] * (1.0 - tx) + d[j] * tx) * tz;
        [l(0), l(1)]
    }
}

pub struct Floor {
    pub y: f32,
    pub color: V3,
    pub roughness: f32,
    /// Extra mirror reflectance on top of Fresnel (0 = plain lacquer).
    pub mirror: f32,
    /// Light pool: the key's light on the floor falls off around this centre.
    pub pool: (f32, f32, f32),
    /// The floor fades into the backdrop between these distances from the pool centre.
    pub fade: (f32, f32),
    pub vis: Option<Arc<FloorVis>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: V3,
    pub target: V3,
    pub fov: f32,
    /// Focus distance (world units) and aperture (blur-circle radius in
    /// pixels at 1080p for points at infinity).
    pub focus: f32,
    pub aperture: f32,
    pub roll: f32,
}

impl Camera {
    pub fn basis(&self) -> (V3, V3, V3) {
        let f = norm(sub(self.target, self.pos));
        let r0 = norm(cross(f, v3(0.0, 1.0, 0.0)));
        let u0 = cross(r0, f);
        let a = self.roll.to_radians();
        let r = add(scale(r0, a.cos()), scale(u0, a.sin()));
        let u = add(scale(u0, a.cos()), scale(r0, -a.sin()));
        (f, r, u)
    }
}

pub struct Scene {
    pub objects: Vec<Object>,
    pub env: Env,
    pub floor: Option<Floor>,
    /// Backdrop colours (linear): at the horizon and at the top of frame.
    pub backdrop: (V3, V3),
    /// A soft glow on the backdrop: direction, colour, angular width (1 - cos).
    pub glow: (V3, V3, f32),
    pub exposure: f32,
}

#[derive(Clone, Copy)]
pub struct Quality {
    pub min_spp: u32,
    pub max_spp: u32,
    pub ao_rays: u32,
    pub shadow_rays: u32,
    pub max_depth: u32,
}

impl Default for Quality {
    fn default() -> Self {
        Quality { min_spp: 4, max_spp: 16, ao_rays: 6, shadow_rays: 4, max_depth: 3 }
    }
}

/// What a ray hit.
#[derive(Clone, Copy)]
struct Surf {
    t: f32,
    p: V3,
    ng: V3,
    ns: V3,
    /// Object index, or `usize::MAX` for the floor.
    obj: usize,
}

const FLOOR: usize = usize::MAX;
const EPS: f32 = 1e-3;

/// Per-pixel output of the primary pass.
#[derive(Clone, Copy, Default)]
pub struct Px {
    pub col: [f32; 3],
    pub refl: [f32; 3],
    /// Weighted floor coverage and reflection blur radius (pixels).
    pub cov: f32,
    pub rad: f32,
    /// View depth (distance along the camera axis) and motion (pixels per frame).
    pub depth: f32,
    pub vel: [f32; 2],
}

struct Sample {
    col: V3,
    refl: V3,
    floor: bool,
    rad: f32,
    depth: f32,
    vel: (f32, f32),
    id: usize,
}

impl Scene {
    fn trace(&self, o: V3, d: V3, tmax: f32) -> Option<Surf> {
        let mut best: Option<Surf> = None;
        let mut tm = tmax;
        if let Some(f) = &self.floor {
            if d.y < -1e-6 {
                let t = (f.y - o.y) / d.y;
                if t > EPS && t < tm {
                    tm = t;
                    let up = v3(0.0, 1.0, 0.0);
                    best = Some(Surf { t, p: madd(o, d, t), ng: up, ns: up, obj: FLOOR });
                }
            }
        }
        for (i, ob) in self.objects.iter().enumerate() {
            let lo = ob.xf.point_inv(o);
            let ld = scale(ob.xf.rot_inv(d), 1.0 / ob.xf.s);
            if let Some(h) = ob.bvh.intersect(lo, ld, EPS, tm) {
                tm = h.t;
                let tr = &ob.bvh.tris[h.tri as usize];
                let ng = norm(ob.xf.rot(cross(tr.e1, tr.e2)));
                let w = 1.0 - h.u - h.v;
                let ns = norm(ob.xf.rot(add(add(scale(tr.n[0], w), scale(tr.n[1], h.u)), scale(tr.n[2], h.v))));
                best = Some(Surf { t: h.t, p: madd(o, d, h.t), ng, ns, obj: i });
            }
        }
        best
    }

    fn occluded(&self, o: V3, d: V3, tmax: f32, floor: bool) -> bool {
        if floor {
            if let Some(f) = &self.floor {
                if d.y < -1e-6 && (f.y - o.y) / d.y < tmax && (f.y - o.y) / d.y > EPS {
                    return true;
                }
            }
        }
        for ob in &self.objects {
            let lo = ob.xf.point_inv(o);
            let ld = scale(ob.xf.rot_inv(d), 1.0 / ob.xf.s);
            if ob.bvh.occluded(lo, ld, EPS, tmax) {
                return true;
            }
        }
        false
    }

    /// The backdrop seen by camera rays that miss everything.
    #[inline]
    pub fn backdrop(&self, d: V3) -> V3 {
        let base = lerp3(self.backdrop.0, self.backdrop.1, smoothstep(-0.05, 0.6, d.y));
        let (g, c, w) = self.glow;
        madd(base, c, (-(1.0 - dot(d, g)) / w.max(1e-4)).exp())
    }

    /// Key softbox visibility (0..1) and ambient occlusion (0..1) at `p`.
    fn visibility(&self, p: V3, n: V3, rays_k: u32, rays_ao: u32, rot: (f32, f32), ao_dist: f32) -> (f32, f32) {
        let o = madd(p, n, 2e-3);
        let mut vk = 1.0;
        if let Some(k) = self.env.key_box() {
            if rays_k > 0 && dot(n, k.c) > -0.5 {
                let mut lit = 0;
                for i in 0..rays_k {
                    let (a, b) = r2(i, rot);
                    let d = k.dir_at(a * 2.0 - 1.0, b * 2.0 - 1.0);
                    if dot(d, n) <= 0.0 || !self.occluded(o, d, 1e6, false) {
                        lit += 1;
                    }
                }
                vk = lit as f32 / rays_k as f32;
            }
        }
        let mut ao = 1.0;
        if rays_ao > 0 {
            let mut sum = 0.0;
            let rot2 = (rot.1, rot.0);
            for i in 0..rays_ao {
                let (a, b) = r2(i, rot2);
                let d = cosine_dir(n, a, b);
                if self.occluded(o, d, ao_dist, true) {
                    sum += 1.0;
                }
            }
            ao = 1.0 - sum / rays_ao as f32;
        }
        (vk, ao)
    }

    /// Diffuse light from the key softbox for normal `n` (radiance for albedo 1, unshadowed).
    #[inline]
    fn key_diffuse(&self, n: V3) -> V3 {
        match self.env.key_box() {
            Some(k) => {
                let c = dot(n, k.c).max(0.0);
                scale(k.radiance, self.env.key_solid_angle * c * (1.0 - k.falloff / 3.0) / std::f32::consts::PI)
            }
            None => v3(0.0, 0.0, 0.0),
        }
    }

    /// Radiance arriving along a secondary ray.
    fn radiance(&self, o: V3, d: V3, depth: u32, q: &Quality, rough: f32, rot: (f32, f32)) -> V3 {
        match self.trace(o, d, 1e6) {
            Some(s) => {
                if s.obj == FLOOR {
                    let (c, r, f, _) = self.shade_floor(&s, d, depth, q, rot);
                    let b = self.backdrop(d);
                    lerp3(add(c, r), b, f)
                } else {
                    self.shade_obj(&s, d, depth, q, rot)
                }
            }
            None => self.env.radiance(d, rough),
        }
    }

    /// Floor: (diffuse colour, reflection, fade to backdrop, reflected hit distance).
    fn shade_floor(&self, s: &Surf, d: V3, depth: u32, q: &Quality, rot: (f32, f32)) -> (V3, V3, f32, f32) {
        let f = self.floor.as_ref().unwrap();
        let p = s.p;
        let (vk, ao) = match &f.vis {
            Some(v) => {
                let a = v.sample(p.x, p.z);
                (a[0], a[1])
            }
            None => (1.0, 1.0),
        };
        let dx = p.x - f.pool.0;
        let dz = p.z - f.pool.1;
        let r2d = dx * dx + dz * dz;
        let pool = (-r2d / (f.pool.2 * f.pool.2)).exp();
        let up = v3(0.0, 1.0, 0.0);
        let light = add(scale(self.env.diffuse(up), ao), scale(self.key_diffuse(up), vk * pool * ao.sqrt()));
        let diffuse = mul(f.color, light);
        let fade = smoothstep(f.fade.0, f.fade.1, r2d.sqrt());
        // Reflection: Fresnel lacquer plus an optional mirror boost.
        let nv = (-d.y).clamp(1e-4, 1.0);
        let fr = 0.04 + 0.96 * (1.0 - nv).powi(5);
        let k = (fr + f.mirror * (1.0 - fr)) * (1.0 - fade);
        let mut refl = v3(0.0, 0.0, 0.0);
        let mut dist = 0.0;
        if k > 1e-3 {
            let r = v3(d.x, -d.y, d.z);
            if depth < q.max_depth {
                match self.trace(madd(p, up, 1e-3), r, 1e6) {
                    Some(h) if h.obj != FLOOR => {
                        dist = h.t;
                        refl = self.shade_obj(&h, r, depth + 1, q, rot);
                    }
                    _ => {
                        // Past the objects the floor mirrors the dark backdrop
                        // (plus a trace of the studio lights).
                        dist = 1e3;
                        refl = madd(self.backdrop(r), self.env.radiance(r, f.roughness.max(0.3)), 0.05);
                    }
                }
            } else {
                refl = madd(self.backdrop(r), self.env.radiance(r, f.roughness.max(0.3)), 0.05);
                dist = 1e3;
            }
            refl = scale(refl, k);
        }
        (diffuse, refl, fade, dist)
    }

    fn shade_obj(&self, s: &Surf, d: V3, depth: u32, q: &Quality, rot: (f32, f32)) -> V3 {
        let ob = &self.objects[s.obj];
        let m = &ob.mat;
        let (mut ng, mut n) = (s.ng, s.ns);
        if dot(ng, d) > 0.0 {
            ng = scale(ng, -1.0);
        }
        if dot(n, ng) < 0.0 {
            n = scale(n, -1.0);
        }
        let v = scale(d, -1.0);
        // Keep the shading normal facing the viewer.
        let mut nv = dot(n, v);
        if nv < 0.02 {
            n = norm(madd(n, v, 0.02 - nv));
            nv = dot(n, v).max(1e-4);
        }
        let rough = m.roughness.clamp(0.02, 1.0);
        let f0 = lerp3(v3(0.04 * m.specular, 0.04 * m.specular, 0.04 * m.specular), m.base, m.metallic);
        let ab = env_brdf(rough, nv);
        let spec_w = madd(scale(f0, ab.0), v3(1.0, 1.0, 1.0), ab.1);
        let diff_col = scale(m.base, 1.0 - m.metallic);
        let needs_diffuse = luma(diff_col) > 0.01;
        let rotp = (rot.1 * 0.37 + s.p.x * 13.7 % 1.0, rot.0 * 0.61 + s.p.z * 7.3 % 1.0);
        let (vk, ao) = if needs_diffuse || rough > 0.3 {
            let (nk, na) = if depth == 0 && needs_diffuse { (q.shadow_rays * 3, q.ao_rays * 3) } else if depth == 0 { (q.shadow_rays, q.ao_rays) } else { (q.shadow_rays.min(2), q.ao_rays.min(2)) };
            self.visibility(s.p, ng, nk, na, rotp, 0.8)
        } else {
            (1.0, 1.0)
        };
        // Specular: trace the reflection (sharp and glossy), fall back to the
        // pre-filtered environment for rough surfaces.
        let mut r = reflect(d, n);
        let rg = dot(r, ng);
        if rg < 0.01 {
            r = norm(madd(r, ng, 0.01 - rg));
        }
        let rough_mix = smoothstep(0.18, 0.5, rough);
        let env_l = scale(self.env.radiance(r, rough), ao.powf(0.5 + rough));
        let traced = if depth < q.max_depth && rough_mix < 1.0 {
            let l = self.radiance(madd(s.p, ng, 1e-3), r, depth + 1, q, rough, rot);
            lerp3(l, env_l, rough_mix)
        } else {
            env_l
        };
        let mut c = mul(traced, spec_w);
        if needs_diffuse {
            let light = add(scale(self.env.diffuse(n), ao), scale(self.key_diffuse(n), vk));
            let kd = 1.0 - luma(spec_w).min(1.0);
            c = madd(c, mul(diff_col, light), kd);
        }
        if m.clearcoat > 0.0 {
            let fc = (0.04 + 0.96 * (1.0 - nv).powi(5)) * m.clearcoat;
            c = madd(scale(c, 1.0 - fc), self.env.radiance(r, 0.03), fc);
        }
        add(c, m.emissive)
    }

    fn primary(&self, cam: &Camera, prev: &Camera, w: usize, h: usize, sx: f32, sy: f32, q: &Quality, rot: (f32, f32)) -> Sample {
        let (f, r, u) = cam.basis();
        let th = (cam.fov * 0.5).to_radians().tan();
        let aspect = w as f32 / h as f32;
        let nx = (sx / w as f32 * 2.0 - 1.0) * th * aspect;
        let ny = (1.0 - sy / h as f32 * 2.0) * th;
        let d = norm(add(f, add(scale(r, nx), scale(u, ny))));
        let project_prev = |p: V3| -> (f32, f32) {
            let (pf, pr, pu) = prev.basis();
            let pth = (prev.fov * 0.5).to_radians().tan();
            let q = sub(p, prev.pos);
            let z = dot(q, pf).max(1e-4);
            let px = (dot(q, pr) / z / (pth * aspect) + 1.0) * 0.5 * w as f32;
            let py = (1.0 - dot(q, pu) / z / pth) * 0.5 * h as f32;
            (px, py)
        };
        match self.trace(cam.pos, d, 1e6) {
            None => Sample { col: self.backdrop(d), refl: v3(0.0, 0.0, 0.0), floor: false, rad: 0.0, depth: 1e4, vel: (0.0, 0.0), id: usize::MAX - 1 },
            Some(s) => {
                let depth = dot(sub(s.p, cam.pos), f);
                let prev_p = if s.obj == FLOOR {
                    s.p
                } else {
                    let ob = &self.objects[s.obj];
                    ob.prev.point(ob.xf.point_inv(s.p))
                };
                let (ppx, ppy) = project_prev(prev_p);
                let vel = (sx - ppx, sy - ppy);
                if s.obj == FLOOR {
                    let (c, refl, fade, dist) = self.shade_floor(&s, d, 0, q, rot);
                    let col = lerp3(c, self.backdrop(d), fade);
                    let fl = self.floor.as_ref().unwrap();
                    let focal = h as f32 * 0.5 / th;
                    let rad = (fl.roughness * dist.min(50.0) / depth.max(1e-3) * focal * 0.5).min(48.0);
                    Sample { col, refl, floor: true, rad, depth, vel, id: FLOOR }
                } else {
                    let col = self.shade_obj(&s, d, 0, q, rot);
                    Sample { col, refl: v3(0.0, 0.0, 0.0), floor: false, rad: 0.0, depth, vel, id: s.obj }
                }
            }
        }
    }

    /// Render the primary pass for rows `y0..y1` into `out` (row-major, `w` wide).
    pub fn render_rows(&self, cam: &Camera, prev: &Camera, w: usize, h: usize, y0: usize, y1: usize, q: &Quality, out: &mut [Px]) {
        let mut samples: Vec<Sample> = Vec::with_capacity(q.max_spp as usize);
        for y in y0..y1 {
            for x in 0..w {
                let hs = hash2(x as u32, y as u32);
                let rot = (u01(hs), u01(hash2(hs, 0x9e37)));
                samples.clear();
                let take = |i: u32, samples: &mut Vec<Sample>| {
                    let (a, b) = r2(i, rot);
                    let srot = (u01(hash2(hs, i * 2 + 1)), u01(hash2(hs, i * 2 + 2)));
                    samples.push(self.primary(cam, prev, w, h, x as f32 + a, y as f32 + b, q, srot));
                };
                for i in 0..q.min_spp {
                    take(i, &mut samples);
                }
                // Refine where the first samples disagree.
                let tm = |s: &Sample| {
                    let l = luma(add(s.col, s.refl));
                    l / (1.0 + l)
                };
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                let id0 = samples[0].id;
                let mut mixed = false;
                for s in &samples {
                    let v = tm(s);
                    lo = lo.min(v);
                    hi = hi.max(v);
                    mixed |= s.id != id0;
                }
                if mixed || hi - lo > 0.04 {
                    for i in q.min_spp..q.max_spp {
                        take(i, &mut samples);
                    }
                }
                // Resolve with tone-mapped weights (bright highlights don't alias).
                let mut px = Px::default();
                let mut wsum = 0.0;
                let mut best_d = f32::MAX;
                let mut fl_w = 0.0;
                for s in &samples {
                    let wt = 1.0 / (1.0 + luma(add(s.col, s.refl)));
                    wsum += wt;
                    for k in 0..3 {
                        px.col[k] += get(s.col, k) * wt;
                        px.refl[k] += get(s.refl, k) * wt;
                    }
                    if s.floor {
                        px.cov += wt;
                        px.rad += s.rad * wt;
                        fl_w += wt;
                    }
                    if s.depth < best_d {
                        best_d = s.depth;
                        px.vel = [s.vel.0, s.vel.1];
                    }
                }
                for k in 0..3 {
                    px.col[k] /= wsum;
                    px.refl[k] /= wsum;
                }
                if fl_w > 0.0 {
                    px.rad /= fl_w;
                }
                px.cov /= wsum;
                px.depth = best_d;
                out[(y - y0) * w + x] = px;
            }
        }
    }

    /// Bake key visibility and AO over the floor around the objects.
    pub fn bake_floor(&mut self, res: usize, rays: u32) {
        let Some(f) = &self.floor else { return };
        if self.objects.is_empty() {
            return;
        }
        let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        for o in &self.objects {
            for k in 0..8 {
                let c = v3(if k & 1 == 0 { o.bvh.lo.x } else { o.bvh.hi.x }, if k & 2 == 0 { o.bvh.lo.y } else { o.bvh.hi.y }, if k & 4 == 0 { o.bvh.lo.z } else { o.bvh.hi.z });
                let p = o.xf.point(c);
                lo = vmin(lo, p);
                hi = vmax(hi, p);
            }
        }
        // Margin: shadows reach out about the objects' height.
        let m = (hi.y - f.y).max(0.5) * 1.6;
        let (x0, x1, z0, z1) = (lo.x - m, hi.x + m, lo.z - m, hi.z + m);
        let aspect = (x1 - x0) / (z1 - z0);
        let (nx, nz) = if aspect >= 1.0 { (res, ((res as f32 / aspect) as usize).max(8)) } else { (((res as f32 * aspect) as usize).max(8), res) };
        let y = f.y;
        let me: &Scene = self;
        let up = v3(0.0, 1.0, 0.0);
        let row = |k: usize| -> Vec<[f32; 2]> {
            (0..nx)
                .map(|i| {
                    let p = v3(x0 + (i as f32 + 0.5) / nx as f32 * (x1 - x0), y, z0 + (k as f32 + 0.5) / nz as f32 * (z1 - z0));
                    let hs = hash2(i as u32 + 7, k as u32 + 13);
                    let (a, b) = me.visibility(p, up, rays, rays, (u01(hs), u01(hash2(hs, 5))), (hi.y - y).max(0.3) * 1.2);
                    [a, b]
                })
                .collect()
        };
        use rayon::prelude::*;
        let rows: Vec<Vec<[f32; 2]>> = if rayon::current_thread_index().is_some() { (0..nz).map(row).collect() } else { (0..nz).into_par_iter().map(row).collect() };
        let raw: Vec<[f32; 2]> = rows.into_iter().flatten().collect();
        // A small blur removes the remaining sampling noise.
        let mut data = raw.clone();
        for k in 0..nz {
            for i in 0..nx {
                let mut s = [0.0f32; 2];
                let mut wsum = 0.0;
                for dk in -2i32..=2 {
                    for di in -2i32..=2 {
                        let (a, b) = (i as i32 + di, k as i32 + dk);
                        if a < 0 || b < 0 || a >= nx as i32 || b >= nz as i32 {
                            continue;
                        }
                        let wt = (-((di * di + dk * dk) as f32) / 2.5).exp();
                        let v = raw[b as usize * nx + a as usize];
                        s[0] += v[0] * wt;
                        s[1] += v[1] * wt;
                        wsum += wt;
                    }
                }
                data[k * nx + i] = [s[0] / wsum, s[1] / wsum];
            }
        }
        let vis = FloorVis { x0, z0, x1, z1, nx, nz, data };
        self.floor.as_mut().unwrap().vis = Some(Arc::new(vis));
    }
}

/// Split-sum environment BRDF (scale, bias) for F0 (Karis' analytic fit).
#[inline]
fn env_brdf(rough: f32, nv: f32) -> (f32, f32) {
    let c0 = [-1.0, -0.0275, -0.572, 0.022];
    let c1 = [1.0, 0.0425, 1.04, -0.04];
    let r = [rough * c0[0] + c1[0], rough * c0[1] + c1[1], rough * c0[2] + c1[2], rough * c0[3] + c1[3]];
    let a004 = (r[0] * r[0]).min((-9.28 * nv).exp2()) * r[0] + r[1];
    (a004 * -1.04 + r[2], a004 * 1.04 + r[3])
}

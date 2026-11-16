//! Surface nets: mesh a sampled signed distance field (negative inside).
//!
//! The host samples the field on a regular grid (it owns the SDF function);
//! this module extracts one vertex per surface cell (the mean of the edge
//! crossings), quads across every sign-changing grid edge, and normals from
//! the grid gradient. Output is a [`crate::raster::Mesh`].

use crate::math::sqrtf;
use crate::raster::Mesh;
use alloc::vec;
use alloc::vec::Vec;

const EDGES: [(usize, usize); 12] = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];

/// Mesh `d` (`nx × ny × nz` samples, x fastest) with grid spacing `s`, the
/// first sample at `origin`. Front faces are counter-clockwise from outside.
pub fn surface_nets(d: &[f32], nx: usize, ny: usize, nz: usize, origin: [f32; 3], s: f32) -> Mesh {
    let mut m = Mesh { pos: Vec::new(), nrm: Vec::new(), uv: Vec::new(), idx: Vec::new() };
    if nx < 2 || ny < 2 || nz < 2 || d.len() < nx * ny * nz {
        return m;
    }
    let (rx, ry, rz) = (nx - 1, ny - 1, nz - 1);
    let at = |i: usize, j: usize, k: usize| d[i + nx * (j + ny * k)];
    let mut cell = vec![u32::MAX; rx * ry * rz];
    let mut c = [0f32; 8];
    for k in 0..rz {
        for j in 0..ry {
            for i in 0..rx {
                let mut mask = 0u32;
                for (q, cq) in c.iter_mut().enumerate() {
                    *cq = at(i + (q & 1), j + ((q >> 1) & 1), k + ((q >> 2) & 1));
                    if *cq < 0.0 {
                        mask |= 1 << q;
                    }
                }
                if mask == 0 || mask == 255 {
                    continue;
                }
                let (mut x, mut y, mut z, mut n) = (0f32, 0f32, 0f32, 0f32);
                for &(a, b) in EDGES.iter() {
                    if (c[a] < 0.0) == (c[b] < 0.0) {
                        continue;
                    }
                    let t = c[a] / (c[a] - c[b]);
                    let (ax, ay, az) = ((a & 1) as f32, ((a >> 1) & 1) as f32, ((a >> 2) & 1) as f32);
                    let (bx, by, bz) = ((b & 1) as f32, ((b >> 1) & 1) as f32, ((b >> 2) & 1) as f32);
                    x += ax + t * (bx - ax);
                    y += ay + t * (by - ay);
                    z += az + t * (bz - az);
                    n += 1.0;
                }
                // Gradient: mean of the four parallel cell edges per axis.
                let gx = (c[1] - c[0]) + (c[3] - c[2]) + (c[5] - c[4]) + (c[7] - c[6]);
                let gy = (c[2] - c[0]) + (c[3] - c[1]) + (c[6] - c[4]) + (c[7] - c[5]);
                let gz = (c[4] - c[0]) + (c[5] - c[1]) + (c[6] - c[2]) + (c[7] - c[3]);
                let l = sqrtf(gx * gx + gy * gy + gz * gz);
                let l = if l > 0.0 { 1.0 / l } else { 0.0 };
                cell[i + rx * (j + ry * k)] = (m.pos.len() / 3) as u32;
                m.pos.extend_from_slice(&[origin[0] + (i as f32 + x / n) * s, origin[1] + (j as f32 + y / n) * s, origin[2] + (k as f32 + z / n) * s]);
                m.nrm.extend_from_slice(&[gx * l, gy * l, gz * l]);
                m.uv.extend_from_slice(&[0.0, 0.0]);
            }
        }
    }
    let cid = |i: usize, j: usize, k: usize| cell[i + rx * (j + ry * k)];
    let mut quad = |q: [u32; 4], inside_first: bool| {
        if q.contains(&u32::MAX) {
            return;
        }
        let [a, b, cc, dd] = q;
        if inside_first {
            m.idx.extend_from_slice(&[a, cc, b, b, cc, dd]);
        } else {
            m.idx.extend_from_slice(&[a, b, cc, b, dd, cc]);
        }
    };
    for k in 1..rz {
        for j in 1..ry {
            for i in 0..rx {
                let (a, b) = (at(i, j, k), at(i + 1, j, k));
                if (a < 0.0) != (b < 0.0) {
                    quad([cid(i, j - 1, k - 1), cid(i, j, k - 1), cid(i, j - 1, k), cid(i, j, k)], a < 0.0);
                }
            }
        }
    }
    for k in 1..rz {
        for j in 0..ry {
            for i in 1..rx {
                let (a, b) = (at(i, j, k), at(i, j + 1, k));
                if (a < 0.0) != (b < 0.0) {
                    quad([cid(i - 1, j, k - 1), cid(i - 1, j, k), cid(i, j, k - 1), cid(i, j, k)], a < 0.0);
                }
            }
        }
    }
    for k in 0..rz {
        for j in 1..ry {
            for i in 1..rx {
                let (a, b) = (at(i, j, k), at(i, j, k + 1));
                if (a < 0.0) != (b < 0.0) {
                    quad([cid(i - 1, j - 1, k), cid(i, j - 1, k), cid(i - 1, j, k), cid(i, j, k)], a < 0.0);
                }
            }
        }
    }
    fix_winding(&mut m);
    m
}

/// Flip triangles whose geometric normal disagrees with their vertex normals.
fn fix_winding(m: &mut Mesh) {
    let (p, n) = (&m.pos, &m.nrm);
    for t in m.idx.chunks_exact_mut(3) {
        let (a, b, c) = (t[0] as usize * 3, t[1] as usize * 3, t[2] as usize * 3);
        let u = [p[b] - p[a], p[b + 1] - p[a + 1], p[b + 2] - p[a + 2]];
        let v = [p[c] - p[a], p[c + 1] - p[a + 1], p[c + 2] - p[a + 2]];
        let f = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let s = [n[a] + n[b] + n[c], n[a + 1] + n[b + 1] + n[c + 1], n[a + 2] + n[b + 2] + n[c + 2]];
        if f[0] * s[0] + f[1] * s[1] + f[2] * s[2] < 0.0 {
            t.swap(1, 2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_vertices_lie_on_the_surface_with_outward_normals() {
        let n = 33;
        let s = 2.4 / (n - 1) as f32;
        let o = -1.2f32;
        let mut d = vec![0f32; n * n * n];
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    let (x, y, z) = (o + i as f32 * s, o + j as f32 * s, o + k as f32 * s);
                    d[i + n * (j + n * k)] = sqrtf(x * x + y * y + z * z) - 0.8;
                }
            }
        }
        let m = surface_nets(&d, n, n, n, [o, o, o], s);
        assert!(m.idx.len() / 3 > 1000);
        assert_eq!(m.idx.len() % 3, 0);
        for v in 0..m.pos.len() / 3 {
            let (x, y, z) = (m.pos[v * 3], m.pos[v * 3 + 1], m.pos[v * 3 + 2]);
            let r = sqrtf(x * x + y * y + z * z);
            assert!((r - 0.8).abs() < 0.03, "radius {r}");
            let dot = (x * m.nrm[v * 3] + y * m.nrm[v * 3 + 1] + z * m.nrm[v * 3 + 2]) / r;
            assert!(dot > 0.95, "normal {dot}");
        }
        for &i in &m.idx {
            assert!((i as usize) < m.pos.len() / 3);
        }
    }

    #[test]
    fn empty_and_degenerate_grids() {
        assert!(surface_nets(&[1.0; 8], 2, 2, 2, [0.0; 3], 1.0).idx.is_empty());
        assert!(surface_nets(&[1.0; 4], 1, 2, 2, [0.0; 3], 1.0).pos.is_empty());
    }
}

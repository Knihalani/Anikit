//! 3D Gaussian splats: file parsing, GPU packing and depth sorting.
//!
//! Splats are kept in a canonical float layout of [`STRIDE`] values each:
//! `x y z  sx sy sz  qw qx qy qz  r g b a` (linear scale, a normalised
//! rotation quaternion, colour and opacity in `0..=1`). Parsers produce it,
//! [`pack`] turns it into 8 `u32` words per splat for a WebGL2 `RGBA32UI`
//! texture (two texels: centre + colour, then the 3D covariance as six
//! halves), and [`Sorter`] orders splats back to front with a 16-bit
//! counting sort (one pass over 65 536 buckets), which is what keeps a
//! million splats under a few milliseconds per sort.

#![allow(clippy::needless_range_loop)]

use alloc::vec;
use alloc::vec::Vec;

/// Floats per splat in the canonical layout.
pub const STRIDE: usize = 14;
/// `u32` words per splat in the packed GPU layout.
pub const PACKED: usize = 8;

const SH_C0: f32 = 0.282_094_8;

fn exp(x: f32) -> f32 {
    libm::expf(x)
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + exp(-x))
}

fn rd_f32(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Parse antimatter15-style `.splat` data: 32-byte records of
/// `f32 position[3], f32 scale[3], u8 rgba[4], u8 rotation[4]` (w x y z,
/// encoded as `q * 128 + 128`). Trailing partial records are ignored.
pub fn parse_splat(bytes: &[u8]) -> Vec<f32> {
    let n = bytes.len() / 32;
    let mut out = vec![0.0f32; n * STRIDE];
    for i in 0..n {
        let b = &bytes[i * 32..i * 32 + 32];
        let o = &mut out[i * STRIDE..(i + 1) * STRIDE];
        for k in 0..6 {
            o[k] = rd_f32(b, k * 4);
        }
        let mut q = [0.0f32; 4];
        for k in 0..4 {
            q[k] = (b[28 + k] as f32 - 128.0) / 128.0;
        }
        let q = normalize_q(q);
        o[6..10].copy_from_slice(&q);
        for k in 0..4 {
            o[10 + k] = b[24 + k] as f32 / 255.0;
        }
    }
    out
}

fn normalize_q(q: [f32; 4]) -> [f32; 4] {
    let l = crate::math::sqrtf(q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]);
    if l < 1e-12 {
        [1.0, 0.0, 0.0, 0.0]
    } else {
        [q[0] / l, q[1] / l, q[2] / l, q[3] / l]
    }
}

/// Why a `.ply` file was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlyError {
    /// No `ply` magic or no `end_header`.
    NotPly,
    /// ASCII or big-endian bodies are not supported.
    Format,
    /// The vertex element has no `x`, `y` or `z`.
    MissingPosition,
    /// The body is shorter than the header announces.
    Truncated,
}

#[derive(Clone, Copy)]
enum Ty {
    F32,
    F64,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
}

impl Ty {
    fn parse(s: &[u8]) -> Option<Ty> {
        Some(match s {
            b"float" | b"float32" => Ty::F32,
            b"double" | b"float64" => Ty::F64,
            b"uchar" | b"uint8" => Ty::U8,
            b"char" | b"int8" => Ty::I8,
            b"ushort" | b"uint16" => Ty::U16,
            b"short" | b"int16" => Ty::I16,
            b"uint" | b"uint32" => Ty::U32,
            b"int" | b"int32" => Ty::I32,
            _ => return None,
        })
    }
    fn size(self) -> usize {
        match self {
            Ty::U8 | Ty::I8 => 1,
            Ty::U16 | Ty::I16 => 2,
            Ty::F32 | Ty::U32 | Ty::I32 => 4,
            Ty::F64 => 8,
        }
    }
    fn read(self, b: &[u8], o: usize) -> f32 {
        match self {
            Ty::F32 => rd_f32(b, o),
            Ty::F64 => {
                let mut a = [0u8; 8];
                a.copy_from_slice(&b[o..o + 8]);
                f64::from_le_bytes(a) as f32
            }
            Ty::U8 => b[o] as f32,
            Ty::I8 => b[o] as i8 as f32,
            Ty::U16 => u16::from_le_bytes([b[o], b[o + 1]]) as f32,
            Ty::I16 => i16::from_le_bytes([b[o], b[o + 1]]) as f32,
            Ty::U32 => u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as f32,
            Ty::I32 => i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as f32,
        }
    }
}

// Property slots the parser understands.
const P_X: usize = 0; // x y z = 0 1 2
const P_SCALE: usize = 3; // scale_0..2 = 3 4 5
const P_ROT: usize = 6; // rot_0..3 = 6..9
const P_DC: usize = 10; // f_dc_0..2 = 10..12
const P_OPACITY: usize = 13;
const P_RGB: usize = 14; // red green blue = 14..16
const P_ALPHA: usize = 17;
const SLOTS: usize = 18;

fn slot(name: &[u8]) -> Option<usize> {
    Some(match name {
        b"x" => P_X,
        b"y" => P_X + 1,
        b"z" => P_X + 2,
        b"scale_0" => P_SCALE,
        b"scale_1" => P_SCALE + 1,
        b"scale_2" => P_SCALE + 2,
        b"rot_0" => P_ROT,
        b"rot_1" => P_ROT + 1,
        b"rot_2" => P_ROT + 2,
        b"rot_3" => P_ROT + 3,
        b"f_dc_0" => P_DC,
        b"f_dc_1" => P_DC + 1,
        b"f_dc_2" => P_DC + 2,
        b"opacity" => P_OPACITY,
        b"red" | b"r" => P_RGB,
        b"green" | b"g" => P_RGB + 1,
        b"blue" | b"b" => P_RGB + 2,
        b"alpha" | b"a" => P_ALPHA,
        _ => return None,
    })
}

/// Parse a binary little-endian `.ply`: the 3DGS training output
/// (`x y z`, `scale_*` in log space, `rot_*` as w x y z, `opacity` as a
/// logit, `f_dc_*` spherical-harmonic DC terms) or a plain point cloud with
/// `red green blue [alpha]` bytes (given a small isotropic scale).
/// Higher-order SH bands (`f_rest_*`) are skipped: colour is view-independent.
pub fn parse_ply(bytes: &[u8]) -> Result<Vec<f32>, PlyError> {
    if !bytes.starts_with(b"ply") {
        return Err(PlyError::NotPly);
    }
    let end = find(bytes, b"end_header").ok_or(PlyError::NotPly)?;
    let mut body = end + b"end_header".len();
    while body < bytes.len() && (bytes[body] == b'\r' || bytes[body] == b' ') {
        body += 1;
    }
    if body < bytes.len() && bytes[body] == b'\n' {
        body += 1;
    }
    let header = &bytes[..end];
    let mut count = 0usize;
    let mut stride = 0usize;
    let mut offset_before = 0usize; // bytes of elements declared before `vertex`
    let mut in_vertex = false;
    let mut seen_vertex = false;
    let mut other_count = 0usize;
    let mut other_stride = 0usize;
    let mut props: [Option<(Ty, usize)>; SLOTS] = [None; SLOTS];
    let mut binary_le = false;
    for line in header.split(|&c| c == b'\n') {
        let line = trim(line);
        let mut w = line.split(|&c| c == b' ').filter(|s| !s.is_empty());
        match w.next() {
            Some(b"format") => binary_le = w.next() == Some(b"binary_little_endian"),
            Some(b"element") => {
                if !seen_vertex && other_count > 0 {
                    offset_before += other_count * other_stride;
                }
                let name = w.next().unwrap_or(b"");
                let n = parse_usize(w.next().unwrap_or(b"0"));
                in_vertex = name == b"vertex";
                if in_vertex {
                    seen_vertex = true;
                    count = n;
                } else {
                    other_count = n;
                    other_stride = 0;
                }
            }
            Some(b"property") => {
                let t = w.next().unwrap_or(b"");
                if t == b"list" {
                    // Lists (faces) have variable size: only allowed after the vertices.
                    if in_vertex || !seen_vertex {
                        return Err(PlyError::Format);
                    }
                    continue;
                }
                let ty = Ty::parse(t).ok_or(PlyError::Format)?;
                if in_vertex {
                    if let Some(s) = slot(w.next().unwrap_or(b"")) {
                        props[s] = Some((ty, stride));
                    }
                    stride += ty.size();
                } else {
                    other_stride += ty.size();
                }
            }
            _ => {}
        }
    }
    if !binary_le {
        return Err(PlyError::Format);
    }
    if props[P_X].is_none() || props[P_X + 1].is_none() || props[P_X + 2].is_none() {
        return Err(PlyError::MissingPosition);
    }
    let start = body + offset_before;
    if bytes.len() < start + count * stride {
        return Err(PlyError::Truncated);
    }
    let gs = props[P_SCALE].is_some();
    let mut out = vec![0.0f32; count * STRIDE];
    let get = |rec: &[u8], s: usize, d: f32| props[s].map_or(d, |(t, o)| t.read(rec, o));
    for i in 0..count {
        let rec = &bytes[start + i * stride..start + (i + 1) * stride];
        let o = &mut out[i * STRIDE..(i + 1) * STRIDE];
        for k in 0..3 {
            o[k] = get(rec, P_X + k, 0.0);
        }
        for k in 0..3 {
            o[3 + k] = if gs { exp(get(rec, P_SCALE + k, -4.6)) } else { 0.01 };
        }
        let q = normalize_q([get(rec, P_ROT, 1.0), get(rec, P_ROT + 1, 0.0), get(rec, P_ROT + 2, 0.0), get(rec, P_ROT + 3, 0.0)]);
        o[6..10].copy_from_slice(&q);
        if props[P_DC].is_some() {
            for k in 0..3 {
                o[10 + k] = (0.5 + SH_C0 * get(rec, P_DC + k, 0.0)).clamp(0.0, 1.0);
            }
        } else {
            let scale = |s: usize| match props[s] {
                Some((Ty::U8, _)) => 1.0 / 255.0,
                Some((Ty::U16, _)) => 1.0 / 65535.0,
                _ => 1.0,
            };
            for k in 0..3 {
                o[10 + k] = (get(rec, P_RGB + k, 255.0) * scale(P_RGB + k)).clamp(0.0, 1.0);
            }
        }
        o[13] = if props[P_OPACITY].is_some() {
            sigmoid(get(rec, P_OPACITY, 0.0))
        } else if props[P_ALPHA].is_some() {
            let a = get(rec, P_ALPHA, 255.0);
            (if matches!(props[P_ALPHA], Some((Ty::U8, _))) { a / 255.0 } else { a }).clamp(0.0, 1.0)
        } else {
            1.0
        };
    }
    Ok(out)
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

fn trim(s: &[u8]) -> &[u8] {
    let mut a = 0;
    let mut b = s.len();
    while a < b && (s[a] == b' ' || s[a] == b'\r' || s[a] == b'\t') {
        a += 1;
    }
    while b > a && (s[b - 1] == b' ' || s[b - 1] == b'\r' || s[b - 1] == b'\t') {
        b -= 1;
    }
    &s[a..b]
}

fn parse_usize(s: &[u8]) -> usize {
    s.iter().take_while(|c| c.is_ascii_digit()).fold(0usize, |a, &c| a.saturating_mul(10).saturating_add((c - b'0') as usize))
}

/// 3D covariance `R S Sᵀ Rᵀ` as its upper triangle `[xx, xy, xz, yy, yz, zz]`.
pub fn covariance(scale: [f32; 3], q: [f32; 4]) -> [f32; 6] {
    let [w, x, y, z] = q;
    let r = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y)],
        [2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x)],
        [2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y)],
    ];
    // M = R S, Σ = M Mᵀ
    let mut m = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = r[i][j] * scale[j];
        }
    }
    let d = |a: usize, b: usize| m[a][0] * m[b][0] + m[a][1] * m[b][1] + m[a][2] * m[b][2];
    [d(0, 0), d(0, 1), d(0, 2), d(1, 1), d(1, 2), d(2, 2)]
}

/// IEEE half-precision bits of `v` (round to nearest, flush tiny values).
pub fn f16_bits(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let e = ((b >> 23) & 0xff) as i32;
    let m = b & 0x7f_ffff;
    if e == 0xff {
        return sign | 0x7c00 | if m != 0 { 0x200 } else { 0 };
    }
    let e2 = e - 127 + 15;
    if e2 >= 0x1f {
        return sign | 0x7c00;
    }
    if e2 <= 0 {
        if e2 < -10 {
            return sign;
        }
        let m = m | 0x80_0000;
        let shift = (14 - e2) as u32;
        let half = (m >> shift) + ((m >> (shift - 1)) & 1);
        return sign | half as u16;
    }
    let mut h = ((e2 as u32) << 10) | (m >> 13);
    if m & 0x1000 != 0 {
        h += 1; // may carry into the exponent, which is still correct
    }
    sign | h as u16
}

/// Pack canonical splats into 8 `u32` words each: `x y z` (f32 bits),
/// `rgba8`, then the covariance as three `half2` words and a zero.
pub fn pack(splats: &[f32], out: &mut [u32]) {
    let n = (splats.len() / STRIDE).min(out.len() / PACKED);
    for i in 0..n {
        let s = &splats[i * STRIDE..(i + 1) * STRIDE];
        let o = &mut out[i * PACKED..(i + 1) * PACKED];
        o[0] = s[0].to_bits();
        o[1] = s[1].to_bits();
        o[2] = s[2].to_bits();
        let c = |v: f32| ((v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32) & 0xff;
        o[3] = c(s[10]) | (c(s[11]) << 8) | (c(s[12]) << 16) | (c(s[13]) << 24);
        let cov = covariance([s[3], s[4], s[5]], [s[6], s[7], s[8], s[9]]);
        let h = |a: f32, b: f32| f16_bits(a) as u32 | ((f16_bits(b) as u32) << 16);
        o[4] = h(cov[0], cov[1]);
        o[5] = h(cov[2], cov[3]);
        o[6] = h(cov[4], cov[5]);
        o[7] = 0;
    }
}

/// Back-to-front depth sorter with reusable buffers (no allocation per sort
/// once sized). Centres are `x y z` triples, optionally blended with a
/// second set (`morph`) so a morphing cloud stays correctly ordered.
#[derive(Default)]
pub struct Sorter {
    /// Centres of set A (`x y z` per splat).
    pub a: Vec<f32>,
    /// Centres of set B (same length as `a`, or empty).
    pub b: Vec<f32>,
    keys: Vec<u16>,
    depth: Vec<f32>,
    counts: Vec<u32>,
    /// Splat indices, furthest first, after [`Sorter::sort`].
    pub order: Vec<u32>,
}

impl Sorter {
    /// Resize for `n` splats.
    pub fn resize(&mut self, n: usize) {
        self.a.resize(n * 3, 0.0);
        self.keys.resize(n, 0);
        self.depth.resize(n, 0.0);
        self.order.resize(n, 0);
        if self.counts.len() != 65536 {
            self.counts = vec![0; 65536];
        }
    }

    /// Splat count.
    pub fn len(&self) -> usize {
        self.a.len() / 3
    }

    /// True when there are no splats.
    pub fn is_empty(&self) -> bool {
        self.a.is_empty()
    }

    /// Sort by view depth `d = vx·x + vy·y + vz·z + vw`, where larger `d` is
    /// nearer the camera (the third row of a view·model matrix in a
    /// right-handed, −z-forward camera). `mix` blends set A toward set B.
    /// Writes `order` furthest first; returns the number of splats.
    pub fn sort(&mut self, v: [f32; 4], mix: f32) -> usize {
        let n = self.len();
        let blend = mix > 0.0 && self.b.len() == self.a.len();
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for i in 0..n {
            let (mut x, mut y, mut z) = (self.a[i * 3], self.a[i * 3 + 1], self.a[i * 3 + 2]);
            if blend {
                x += (self.b[i * 3] - x) * mix;
                y += (self.b[i * 3 + 1] - y) * mix;
                z += (self.b[i * 3 + 2] - z) * mix;
            }
            let d = v[0] * x + v[1] * y + v[2] * z + v[3];
            self.depth[i] = d;
            lo = lo.min(d);
            hi = hi.max(d);
        }
        if n == 0 {
            return 0;
        }
        let k = if hi > lo { 65535.0 / (hi - lo) } else { 0.0 };
        self.counts.iter_mut().for_each(|c| *c = 0);
        for i in 0..n {
            // Ascending d = furthest first.
            let key = ((self.depth[i] - lo) * k) as u32;
            let key = key.min(65535) as u16;
            self.keys[i] = key;
            self.counts[key as usize] += 1;
        }
        let mut sum = 0u32;
        for c in self.counts.iter_mut() {
            let t = *c;
            *c = sum;
            sum += t;
        }
        for i in 0..n {
            let key = self.keys[i] as usize;
            self.order[self.counts[key] as usize] = i as u32;
            self.counts[key] += 1;
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn splat_record(p: [f32; 3], s: [f32; 3], rgba: [u8; 4], rot: [u8; 4]) -> Vec<u8> {
        let mut v = Vec::new();
        for f in p.iter().chain(s.iter()) {
            v.extend_from_slice(&f.to_le_bytes());
        }
        v.extend_from_slice(&rgba);
        v.extend_from_slice(&rot);
        v
    }

    #[test]
    fn parses_splat_records() {
        let mut b = splat_record([1.0, 2.0, 3.0], [0.1, 0.2, 0.3], [255, 0, 128, 255], [255, 128, 128, 128]);
        b.extend(splat_record([-1.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0, 0, 0, 0], [128, 128, 128, 128]));
        b.push(7); // partial record ignored
        let s = parse_splat(&b);
        assert_eq!(s.len(), 2 * STRIDE);
        assert_eq!(&s[0..6], &[1.0, 2.0, 3.0, 0.1, 0.2, 0.3]);
        assert!((s[6] - 1.0).abs() < 1e-6 && s[7].abs() < 1e-6);
        assert!((s[10] - 1.0).abs() < 1e-6 && s[11] == 0.0 && (s[13] - 1.0).abs() < 1e-6);
        // A zero quaternion becomes identity.
        assert_eq!(&s[STRIDE + 6..STRIDE + 10], &[1.0, 0.0, 0.0, 0.0]);
    }

    fn ply(header: &str, body: &[u8]) -> Vec<u8> {
        let mut v = header.as_bytes().to_vec();
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn parses_3dgs_ply() {
        let h = "ply\nformat binary_little_endian 1.0\nelement vertex 2\nproperty float x\nproperty float y\nproperty float z\nproperty float nx\nproperty float ny\nproperty float nz\nproperty float f_dc_0\nproperty float f_dc_1\nproperty float f_dc_2\nproperty float f_rest_0\nproperty float opacity\nproperty float scale_0\nproperty float scale_1\nproperty float scale_2\nproperty float rot_0\nproperty float rot_1\nproperty float rot_2\nproperty float rot_3\nend_header\n";
        let mut body = Vec::new();
        for i in 0..2 {
            let vals = [i as f32, 2.0, 3.0, 0.0, 0.0, 1.0, 0.0, 1.0, -1.0, 9.0, 0.0, 0.0, -1.0, 1.0, 2.0, 0.0, 0.0, 0.0];
            for v in vals {
                body.extend_from_slice(&f32::to_le_bytes(v));
            }
        }
        let s = parse_ply(&ply(h, &body)).unwrap();
        assert_eq!(s.len(), 2 * STRIDE);
        assert_eq!(&s[STRIDE..STRIDE + 3], &[1.0, 2.0, 3.0]);
        assert!((s[3] - 1.0).abs() < 1e-6 && (s[4] - exp(-1.0)).abs() < 1e-6);
        assert_eq!(&s[6..10], &[1.0, 0.0, 0.0, 0.0]); // (2,0,0,0) normalised
        assert!((s[10] - 0.5).abs() < 1e-6 && (s[11] - (0.5 + SH_C0)).abs() < 1e-6 && (s[12] - (0.5 - SH_C0)).abs() < 1e-6);
        assert!((s[13] - 0.5).abs() < 1e-6); // sigmoid(0)
    }

    #[test]
    fn parses_point_cloud_ply_and_rejects_bad_files() {
        let h = "ply\r\nformat binary_little_endian 1.0\r\ncomment x\r\nelement vertex 1\r\nproperty double x\r\nproperty float y\r\nproperty float z\r\nproperty uchar red\r\nproperty uchar green\r\nproperty uchar blue\r\nelement face 0\r\nproperty list uchar int vertex_indices\r\nend_header\r\n";
        let mut body = Vec::new();
        body.extend_from_slice(&5.0f64.to_le_bytes());
        body.extend_from_slice(&6.0f32.to_le_bytes());
        body.extend_from_slice(&7.0f32.to_le_bytes());
        body.extend_from_slice(&[255, 0, 51]);
        let s = parse_ply(&ply(h, &body)).unwrap();
        assert_eq!(&s[0..3], &[5.0, 6.0, 7.0]);
        assert!((s[12] - 0.2).abs() < 1e-6 && s[13] == 1.0 && s[3] == 0.01);
        assert_eq!(parse_ply(b"hello"), Err(PlyError::NotPly));
        assert_eq!(parse_ply(b"ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nend_header\n"), Err(PlyError::Format));
        assert_eq!(
            parse_ply(&ply("ply\nformat binary_little_endian 1.0\nelement vertex 1\nproperty float x\nend_header\n", &[0; 4])),
            Err(PlyError::MissingPosition)
        );
        assert_eq!(parse_ply(&ply(h, &body[..10])), Err(PlyError::Truncated));
    }

    #[test]
    fn covariance_of_rotated_scale() {
        let c = covariance([2.0, 1.0, 1.0], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(c, [4.0, 0.0, 0.0, 1.0, 0.0, 1.0]);
        // 90° about z swaps x and y.
        let h = core::f32::consts::FRAC_1_SQRT_2;
        let c = covariance([2.0, 1.0, 1.0], [h, 0.0, 0.0, h]);
        assert!((c[0] - 1.0).abs() < 1e-5 && (c[3] - 4.0).abs() < 1e-5 && c[1].abs() < 1e-5);
    }

    #[test]
    fn half_floats() {
        assert_eq!(f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits(-2.0), 0xc000);
        assert_eq!(f16_bits(0.0), 0);
        assert_eq!(f16_bits(65504.0), 0x7bff);
        assert_eq!(f16_bits(1e6), 0x7c00);
        assert_eq!(f16_bits(0.000_061_035_156), 0x0400); // smallest normal
        assert_eq!(f16_bits(0.000_030_517_578), 0x0200); // subnormal
        assert_eq!(f16_bits(1e-9), 0);
    }

    #[test]
    fn packs_eight_words() {
        let s = [1.0, -2.0, 3.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.5];
        let mut out = [0u32; 8];
        pack(&s, &mut out);
        assert_eq!(f32::from_bits(out[1]), -2.0);
        assert_eq!(out[3], 0xff | (128 << 24));
        assert_eq!(out[4], 0x3c00);
        assert_eq!(out[5], 0x3c00 << 16);
        assert_eq!(out[6], 0x3c00 << 16);
    }

    #[test]
    fn sorts_back_to_front() {
        let mut s = Sorter::default();
        let zs = [5.0, -10.0, 0.0, 3.0, -2.0];
        s.resize(zs.len());
        for (i, z) in zs.iter().enumerate() {
            s.a[i * 3 + 2] = *z;
        }
        // Camera at +z looking down −z: view depth row (0,0,1,-20) (larger = nearer).
        assert_eq!(s.sort([0.0, 0.0, 1.0, -20.0], 0.0), 5);
        assert_eq!(s.order, [1, 4, 2, 3, 0]);
        // Looking the other way reverses it.
        s.sort([0.0, 0.0, -1.0, 0.0], 0.0);
        assert_eq!(s.order, [0, 3, 2, 4, 1]);
        // Blending toward a second set re-orders by the blended centres.
        s.b = vec![0.0; 15];
        s.b[2] = -100.0;
        s.sort([0.0, 0.0, 1.0, 0.0], 1.0);
        assert_eq!(s.order[0], 0);
        // Big random set: order is a permutation and depth is monotonic.
        let n = 10_000;
        s.b.clear();
        s.resize(n);
        let mut seed = 7u32;
        for v in s.a.iter_mut() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *v = (seed >> 8) as f32 / 65536.0 - 128.0;
        }
        s.sort([0.3, -0.5, 0.8, 0.0], 0.0);
        let d = |i: u32| 0.3 * s.a[i as usize * 3] - 0.5 * s.a[i as usize * 3 + 1] + 0.8 * s.a[i as usize * 3 + 2];
        let mut seen = vec![false; n];
        let tol = 2.0 * 256.0 * 1.6 / 65535.0;
        for w in s.order.windows(2) {
            assert!(d(w[0]) <= d(w[1]) + tol);
        }
        for &i in &s.order {
            assert!(!seen[i as usize]);
            seen[i as usize] = true;
        }
    }
}

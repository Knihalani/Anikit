//! 3D: vectors, quaternions, 4×4 matrices (CSS `matrix3d` column-major
//! order), CSS transform-list parsing, matrix decomposition/recomposition for
//! browser-exact 3D transform interpolation, and a bulk perspective projector
//! with radix depth sorting for large point clouds.

use alloc::vec::Vec;

use crate::math::{abs, cos, sin, sqrt, tan, RAD};
use crate::util::{split_call, Cursor};

pub type Mat4 = [f64; 16];
pub type Quat = [f64; 4];

pub const IDENTITY: Mat4 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// `a × b` (column-major: `m[col * 4 + row]`).
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut o = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            o[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
        }
    }
    o
}

pub fn transform_point(m: &Mat4, p: [f64; 3]) -> [f64; 4] {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
        m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15],
    ]
}

pub fn translate(x: f64, y: f64, z: f64) -> Mat4 {
    let mut m = IDENTITY;
    m[12] = x;
    m[13] = y;
    m[14] = z;
    m
}
pub fn scale(x: f64, y: f64, z: f64) -> Mat4 {
    let mut m = IDENTITY;
    m[0] = x;
    m[5] = y;
    m[10] = z;
    m
}
/// Rotation of `deg` degrees about an arbitrary axis (CSS `rotate3d`).
pub fn rotate_axis(x: f64, y: f64, z: f64, deg: f64) -> Mat4 {
    to_matrix(&quat_axis_angle(x, y, z, deg))
}
pub fn rotate_x(deg: f64) -> Mat4 {
    let (s, c) = (sin(deg * RAD), cos(deg * RAD));
    [1.0, 0.0, 0.0, 0.0, 0.0, c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0]
}
pub fn rotate_y(deg: f64) -> Mat4 {
    let (s, c) = (sin(deg * RAD), cos(deg * RAD));
    [c, 0.0, -s, 0.0, 0.0, 1.0, 0.0, 0.0, s, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0]
}
pub fn rotate_z(deg: f64) -> Mat4 {
    let (s, c) = (sin(deg * RAD), cos(deg * RAD));
    [c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
}
pub fn skew(ax_deg: f64, ay_deg: f64) -> Mat4 {
    let mut m = IDENTITY;
    m[4] = tan(ax_deg * RAD);
    m[1] = tan(ay_deg * RAD);
    m
}
pub fn perspective_css(d: f64) -> Mat4 {
    let mut m = IDENTITY;
    if d != 0.0 {
        m[11] = -1.0 / d;
    }
    m
}

pub fn determinant(m: &Mat4) -> f64 {
    let inv = cofactors(m);
    m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12]
}

fn cofactors(m: &Mat4) -> Mat4 {
    let mut inv = [0.0; 16];
    inv[0] = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15] + m[9] * m[7] * m[14] + m[13] * m[6] * m[11] - m[13] * m[7] * m[10];
    inv[4] = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15] - m[8] * m[7] * m[14] - m[12] * m[6] * m[11] + m[12] * m[7] * m[10];
    inv[8] = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15] + m[8] * m[7] * m[13] + m[12] * m[5] * m[11] - m[12] * m[7] * m[9];
    inv[12] = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14] - m[8] * m[6] * m[13] - m[12] * m[5] * m[10] + m[12] * m[6] * m[9];
    inv[1] = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15] - m[9] * m[3] * m[14] - m[13] * m[2] * m[11] + m[13] * m[3] * m[10];
    inv[5] = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15] + m[8] * m[3] * m[14] + m[12] * m[2] * m[11] - m[12] * m[3] * m[10];
    inv[9] = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15] - m[8] * m[3] * m[13] - m[12] * m[1] * m[11] + m[12] * m[3] * m[9];
    inv[13] = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14] + m[8] * m[2] * m[13] + m[12] * m[1] * m[10] - m[12] * m[2] * m[9];
    inv[2] = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15] + m[5] * m[3] * m[14] + m[13] * m[2] * m[7] - m[13] * m[3] * m[6];
    inv[6] = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15] - m[4] * m[3] * m[14] - m[12] * m[2] * m[7] + m[12] * m[3] * m[6];
    inv[10] = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15] + m[4] * m[3] * m[13] + m[12] * m[1] * m[7] - m[12] * m[3] * m[5];
    inv[14] = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14] - m[4] * m[2] * m[13] - m[12] * m[1] * m[6] + m[12] * m[2] * m[5];
    inv[3] = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11] - m[5] * m[3] * m[10] - m[9] * m[2] * m[7] + m[9] * m[3] * m[6];
    inv[7] = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11] + m[4] * m[3] * m[10] + m[8] * m[2] * m[7] - m[8] * m[3] * m[6];
    inv[11] = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11] - m[4] * m[3] * m[9] - m[8] * m[1] * m[7] + m[8] * m[3] * m[5];
    inv[15] = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10] + m[4] * m[2] * m[9] + m[8] * m[1] * m[6] - m[8] * m[2] * m[5];
    inv
}

pub fn invert(m: &Mat4) -> Option<Mat4> {
    let inv = cofactors(m);
    let det = m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12];
    if abs(det) < 1e-12 {
        return None;
    }
    let d = 1.0 / det;
    let mut o = [0.0; 16];
    for i in 0..16 {
        o[i] = inv[i] * d;
    }
    Some(o)
}

pub fn transpose(m: &Mat4) -> Mat4 {
    let mut o = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            o[c * 4 + r] = m[r * 4 + c];
        }
    }
    o
}

// ---------------------------------------------------------------------------
// Quaternions `[x, y, z, w]`
// ---------------------------------------------------------------------------

pub fn quat_axis_angle(x: f64, y: f64, z: f64, deg: f64) -> Quat {
    let l = sqrt(x * x + y * y + z * z);
    if l == 0.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let h = deg * RAD / 2.0;
    let s = sin(h) / l;
    [x * s, y * s, z * s, cos(h)]
}

/// Euler angles (degrees), applied as CSS `rotateX() rotateY() rotateZ()`.
pub fn quat_euler(x: f64, y: f64, z: f64) -> Quat {
    quat_mul(&quat_mul(&quat_axis_angle(1.0, 0.0, 0.0, x), &quat_axis_angle(0.0, 1.0, 0.0, y)), &quat_axis_angle(0.0, 0.0, 1.0, z))
}

pub fn quat_mul(a: &Quat, b: &Quat) -> Quat {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// Spherical interpolation along the shortest arc.
pub fn slerp(a: &Quat, b: &Quat, t: f64) -> Quat {
    let mut d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let mut b = *b;
    if d < 0.0 {
        d = -d;
        for v in b.iter_mut() {
            *v = -*v;
        }
    }
    let (k0, k1) = if d > 0.9995 {
        (1.0 - t, t)
    } else {
        let th = libm::acos(d.min(1.0));
        let s = sin(th);
        (sin((1.0 - t) * th) / s, sin(t * th) / s)
    };
    let mut q = [a[0] * k0 + b[0] * k1, a[1] * k0 + b[1] * k1, a[2] * k0 + b[2] * k1, a[3] * k0 + b[3] * k1];
    let l = sqrt(q.iter().map(|v| v * v).sum());
    if l > 0.0 {
        for v in q.iter_mut() {
            *v /= l;
        }
    }
    q
}

/// Rotation matrix of a unit quaternion.
pub fn to_matrix(q: &Quat) -> Mat4 {
    let [x, y, z, w] = *q;
    [
        1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w), 0.0,
        2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w), 0.0,
        2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y), 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

// ---------------------------------------------------------------------------
// CSS transform lists
// ---------------------------------------------------------------------------

fn angle(c: &mut Cursor) -> Option<f64> {
    c.skip_sep();
    let v = c.number()?;
    let unit = c.ident();
    Some(match unit {
        "rad" => v / RAD,
        "turn" => v * 360.0,
        "grad" => v * 0.9,
        _ => v,
    })
}
fn length(c: &mut Cursor) -> Option<f64> {
    c.skip_sep();
    let v = c.number()?;
    let _ = c.ident();
    c.eat(b'%');
    Some(v)
}

/// Parse a CSS `transform` list (lengths in px; `%` treated as px) into a matrix.
pub fn parse_transform(s: &str) -> Option<Mat4> {
    let s = s.trim();
    if s.is_empty() || s == "none" {
        return Some(IDENTITY);
    }
    let mut m = IDENTITY;
    let mut rest = s;
    while !rest.trim().is_empty() {
        let open = rest.find('(')?;
        let close = rest[open..].find(')')? + open;
        let (name, args) = split_call(&rest[..=close]);
        let args = args.unwrap_or("");
        let mut c = Cursor::new(args);
        let name = name.trim().to_ascii_lowercase();
        let f = match name.as_str() {
            "translate" => {
                let x = length(&mut c)?;
                let y = length(&mut c).unwrap_or(0.0);
                translate(x, y, 0.0)
            }
            "translatex" => translate(length(&mut c)?, 0.0, 0.0),
            "translatey" => translate(0.0, length(&mut c)?, 0.0),
            "translatez" => translate(0.0, 0.0, length(&mut c)?),
            "translate3d" => translate(length(&mut c)?, length(&mut c)?, length(&mut c)?),
            "scale" => {
                let x = { c.skip_sep(); c.number()? };
                c.skip_sep();
                let y = c.number().unwrap_or(x);
                scale(x, y, 1.0)
            }
            "scalex" => scale({ c.skip_sep(); c.number()? }, 1.0, 1.0),
            "scaley" => scale(1.0, { c.skip_sep(); c.number()? }, 1.0),
            "scalez" => scale(1.0, 1.0, { c.skip_sep(); c.number()? }),
            "scale3d" => {
                let v: Vec<f64> = crate::util::parse_numbers(args);
                scale(*v.first()?, *v.get(1)?, *v.get(2)?)
            }
            "rotate" | "rotatez" => rotate_z(angle(&mut c)?),
            "rotatex" => rotate_x(angle(&mut c)?),
            "rotatey" => rotate_y(angle(&mut c)?),
            "rotate3d" => {
                c.skip_sep();
                let x = c.number()?;
                c.skip_sep();
                let y = c.number()?;
                c.skip_sep();
                let z = c.number()?;
                rotate_axis(x, y, z, angle(&mut c)?)
            }
            "skew" => {
                let ax = angle(&mut c)?;
                let ay = angle(&mut c).unwrap_or(0.0);
                skew(ax, ay)
            }
            "skewx" => skew(angle(&mut c)?, 0.0),
            "skewy" => skew(0.0, angle(&mut c)?),
            "perspective" => perspective_css(length(&mut c)?),
            "matrix" => {
                let v = crate::util::parse_numbers(args);
                if v.len() != 6 {
                    return None;
                }
                [v[0], v[1], 0.0, 0.0, v[2], v[3], 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, v[4], v[5], 0.0, 1.0]
            }
            "matrix3d" => {
                let v = crate::util::parse_numbers(args);
                if v.len() != 16 {
                    return None;
                }
                let mut o = [0.0; 16];
                o.copy_from_slice(&v);
                o
            }
            _ => return None,
        };
        m = mul(&m, &f);
        rest = &rest[close + 1..];
    }
    Some(m)
}

// ---------------------------------------------------------------------------
// Decomposition (CSS Transforms 2) and recomposition
// ---------------------------------------------------------------------------

/// `[tx, ty, tz, sx, sy, sz, skew_xy, skew_xz, skew_yz, p0, p1, p2, p3, qx, qy, qz, qw]`
pub type Decomposed = [f64; 17];

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn comb(a: [f64; 3], b: [f64; 3], s1: f64, s2: f64) -> [f64; 3] {
    [a[0] * s1 + b[0] * s2, a[1] * s1 + b[1] * s2, a[2] * s1 + b[2] * s2]
}
fn len3(a: [f64; 3]) -> f64 {
    sqrt(dot3(a, a))
}
fn norm3(a: [f64; 3]) -> [f64; 3] {
    let l = len3(a);
    if l == 0.0 {
        a
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

pub fn decompose(m: &Mat4) -> Option<Decomposed> {
    if m[15] == 0.0 {
        return None;
    }
    let mut m = *m;
    let w = m[15];
    for v in m.iter_mut() {
        *v /= w;
    }
    let mut pm = m;
    pm[3] = 0.0;
    pm[7] = 0.0;
    pm[11] = 0.0;
    pm[15] = 1.0;
    if abs(determinant(&pm)) < 1e-12 {
        return None;
    }
    let mut out = [0.0; 17];
    if m[3] != 0.0 || m[7] != 0.0 || m[11] != 0.0 {
        let rhs = [m[3], m[7], m[11], m[15]];
        let t = transpose(&invert(&pm)?);
        // perspective = transposedInverse × rhs
        for r in 0..4 {
            out[9 + r] = t[r] * rhs[0] + t[4 + r] * rhs[1] + t[8 + r] * rhs[2] + t[12 + r] * rhs[3];
        }
    } else {
        out[9..13].copy_from_slice(&[0.0, 0.0, 0.0, 1.0]);
    }
    out[0] = m[12];
    out[1] = m[13];
    out[2] = m[14];
    let mut row = [[m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]];
    let mut sc = [0.0; 3];
    let mut sk = [0.0; 3];
    sc[0] = len3(row[0]);
    row[0] = norm3(row[0]);
    sk[0] = dot3(row[0], row[1]);
    row[1] = comb(row[1], row[0], 1.0, -sk[0]);
    sc[1] = len3(row[1]);
    row[1] = norm3(row[1]);
    sk[0] /= sc[1];
    sk[1] = dot3(row[0], row[2]);
    row[2] = comb(row[2], row[0], 1.0, -sk[1]);
    sk[2] = dot3(row[1], row[2]);
    row[2] = comb(row[2], row[1], 1.0, -sk[2]);
    sc[2] = len3(row[2]);
    row[2] = norm3(row[2]);
    sk[1] /= sc[2];
    sk[2] /= sc[2];
    let c = [
        row[1][1] * row[2][2] - row[1][2] * row[2][1],
        row[1][2] * row[2][0] - row[1][0] * row[2][2],
        row[1][0] * row[2][1] - row[1][1] * row[2][0],
    ];
    if dot3(row[0], c) < 0.0 {
        for i in 0..3 {
            sc[i] = -sc[i];
            row[i] = [-row[i][0], -row[i][1], -row[i][2]];
        }
    }
    let mut q = [
        0.5 * sqrt((1.0 + row[0][0] - row[1][1] - row[2][2]).max(0.0)),
        0.5 * sqrt((1.0 - row[0][0] + row[1][1] - row[2][2]).max(0.0)),
        0.5 * sqrt((1.0 - row[0][0] - row[1][1] + row[2][2]).max(0.0)),
        0.5 * sqrt((1.0 + row[0][0] + row[1][1] + row[2][2]).max(0.0)),
    ];
    if row[2][1] > row[1][2] {
        q[0] = -q[0];
    }
    if row[0][2] > row[2][0] {
        q[1] = -q[1];
    }
    if row[1][0] > row[0][1] {
        q[2] = -q[2];
    }
    out[3..6].copy_from_slice(&sc);
    out[6..9].copy_from_slice(&sk);
    out[13..17].copy_from_slice(&q);
    Some(out)
}

pub fn recompose(d: &Decomposed) -> Mat4 {
    let mut m = IDENTITY;
    for i in 0..4 {
        m[i * 4 + 3] = d[9 + i];
    }
    for i in 0..4 {
        for j in 0..3 {
            m[12 + i] += d[j] * m[j * 4 + i];
        }
    }
    // Rotation: the decomposition yields a standard unit quaternion.
    let rot = to_matrix(&[d[13], d[14], d[15], d[16]]);
    m = mul(&m, &rot);
    if d[8] != 0.0 {
        let mut t = IDENTITY;
        t[9] = d[8];
        m = mul(&m, &t);
    }
    if d[7] != 0.0 {
        let mut t = IDENTITY;
        t[8] = d[7];
        m = mul(&m, &t);
    }
    if d[6] != 0.0 {
        let mut t = IDENTITY;
        t[4] = d[6];
        m = mul(&m, &t);
    }
    for i in 0..3 {
        for j in 0..4 {
            m[i * 4 + j] *= d[3 + i];
        }
    }
    m
}

/// Interpolate two decomposed transforms (lerp + quaternion slerp).
pub fn mix_decomposed(a: &Decomposed, b: &Decomposed, t: f64) -> Decomposed {
    let mut o = [0.0; 17];
    for i in 0..13 {
        o[i] = a[i] + (b[i] - a[i]) * t;
    }
    let q = slerp(&[a[13], a[14], a[15], a[16]], &[b[13], b[14], b[15], b[16]], t);
    o[13..17].copy_from_slice(&q);
    o
}

/// Interpolate two matrices the way browsers interpolate `transform`.
pub fn mix_matrix(a: &Mat4, b: &Mat4, t: f64) -> Mat4 {
    match (decompose(a), decompose(b)) {
        (Some(da), Some(db)) => recompose(&mix_decomposed(&da, &db, t)),
        _ => {
            if t < 0.5 {
                *a
            } else {
                *b
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Cameras and bulk projection
// ---------------------------------------------------------------------------

/// Right-handed look-at view matrix.
pub fn look_at(eye: [f64; 3], target: [f64; 3], up: [f64; 3]) -> Mat4 {
    let f = norm3([target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]]);
    let s = norm3([f[1] * up[2] - f[2] * up[1], f[2] * up[0] - f[0] * up[2], f[0] * up[1] - f[1] * up[0]]);
    let u = [s[1] * f[2] - s[2] * f[1], s[2] * f[0] - s[0] * f[2], s[0] * f[1] - s[1] * f[0]];
    [
        s[0], u[0], -f[0], 0.0,
        s[1], u[1], -f[1], 0.0,
        s[2], u[2], -f[2], 0.0,
        -dot3(s, eye), -dot3(u, eye), dot3(f, eye), 1.0,
    ]
}

/// OpenGL-style perspective projection.
pub fn perspective(fov_y_deg: f64, aspect: f64, near: f64, far: f64) -> Mat4 {
    let f = 1.0 / tan(fov_y_deg * RAD / 2.0);
    let nf = 1.0 / (near - far);
    [f / aspect, 0.0, 0.0, 0.0, 0.0, f, 0.0, 0.0, 0.0, 0.0, (far + near) * nf, -1.0, 0.0, 0.0, 2.0 * far * near * nf, 0.0]
}

/// Projects large point sets to screen space and depth-sorts them.
#[derive(Default, Debug)]
pub struct Projector {
    /// `[screen_x, screen_y, depth, size_scale] * n`; culled points get scale 0.
    pub out: Vec<f32>,
    /// Point indices ordered far → near (painter's order).
    pub order: Vec<u32>,
    keys: Vec<u32>,
    tmp_keys: Vec<u32>,
    tmp_idx: Vec<u32>,
}

impl Projector {
    /// Project `n` points read from `src` at `stride` with x/y/z channel
    /// offsets, transformed by `model` then `view_proj`, into a `w × h`
    /// viewport. `sort` enables the radix depth sort.
    #[allow(clippy::too_many_arguments)]
    pub fn project(
        &mut self,
        src: &[f32],
        stride: usize,
        offs: [usize; 3],
        n: usize,
        model: &Mat4,
        view_proj: &Mat4,
        w: f64,
        h: f64,
        sort: bool,
    ) {
        let m = mul(view_proj, model);
        let mf: [f32; 16] = core::array::from_fn(|i| m[i] as f32);
        let (hw, hh) = ((w * 0.5) as f32, (h * 0.5) as f32);
        // Size factor: projected size of a unit at clip w = 1.
        let fy = view_proj[5] as f32 * hh;
        self.out.resize(n * 4, 0.0);
        let n = n.min(src.len() / stride.max(1));
        for i in 0..n {
            let b = i * stride;
            let (x, y, z) = (src[b + offs[0]], src[b + offs[1]], src[b + offs[2]]);
            let cx = mf[0] * x + mf[4] * y + mf[8] * z + mf[12];
            let cy = mf[1] * x + mf[5] * y + mf[9] * z + mf[13];
            let cw = mf[3] * x + mf[7] * y + mf[11] * z + mf[15];
            let cz = mf[2] * x + mf[6] * y + mf[10] * z + mf[14];
            let o = &mut self.out[i * 4..i * 4 + 4];
            // Behind the eye, or in front of the near plane (clip z < −w).
            if cw <= 1e-4 || cz < -cw {
                o[0] = 0.0;
                o[1] = 0.0;
                o[2] = f32::INFINITY;
                o[3] = 0.0;
                continue;
            }
            let iw = 1.0 / cw;
            o[0] = hw + cx * iw * hw;
            o[1] = hh - cy * iw * hh;
            o[2] = cw;
            o[3] = fy * iw;
        }
        if sort {
            self.sort_far_to_near(n);
        } else {
            self.order.clear();
            self.order.extend(0..n as u32);
        }
    }

    /// LSD radix sort (3 × 11-bit passes) on depth, descending.
    fn sort_far_to_near(&mut self, n: usize) {
        self.keys.resize(n, 0);
        self.order.resize(n, 0);
        self.tmp_keys.resize(n, 0);
        self.tmp_idx.resize(n, 0);
        for i in 0..n {
            let d = self.out[i * 4 + 2];
            // Positive floats sort like their bits; invert for descending order.
            let bits = if d.is_finite() { d.max(0.0).to_bits() } else { u32::MAX };
            self.keys[i] = !bits;
            self.order[i] = i as u32;
        }
        let mut hist = [0u32; 2048];
        for pass in 0..3 {
            let shift = pass * 11;
            hist.iter_mut().for_each(|h| *h = 0);
            for &k in &self.keys {
                hist[((k >> shift) & 2047) as usize] += 1;
            }
            let mut sum = 0;
            for h in hist.iter_mut() {
                let c = *h;
                *h = sum;
                sum += c;
            }
            for i in 0..n {
                let k = self.keys[i];
                let slot = &mut hist[((k >> shift) & 2047) as usize];
                self.tmp_keys[*slot as usize] = k;
                self.tmp_idx[*slot as usize] = self.order[i];
                *slot += 1;
            }
            core::mem::swap(&mut self.keys, &mut self.tmp_keys);
            core::mem::swap(&mut self.order, &mut self.tmp_idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Mat4, b: &Mat4, eps: f64) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < eps)
    }

    #[test]
    fn parse_and_compose() {
        let m = parse_transform("translate(10px, 20px) rotate(90deg)").unwrap();
        let p = transform_point(&m, [1.0, 0.0, 0.0]);
        assert!((p[0] - 10.0).abs() < 1e-9 && (p[1] - 21.0).abs() < 1e-9);
        let q = parse_transform("rotateY(90deg)").unwrap();
        let p = transform_point(&q, [1.0, 0.0, 0.0]);
        assert!((p[2] + 1.0).abs() < 1e-9, "{p:?}");
        assert!(close(&parse_transform("none").unwrap(), &IDENTITY, 1e-12));
        let mm = parse_transform("matrix(1, 0, 0, 1, 5, 6)").unwrap();
        assert_eq!((mm[12], mm[13]), (5.0, 6.0));
        let r3 = parse_transform("rotate3d(0, 0, 1, 0.25turn)").unwrap();
        assert!(close(&r3, &rotate_z(90.0), 1e-9));
    }

    #[test]
    fn decompose_roundtrip() {
        for s in [
            "translate3d(10px, -20px, 30px) rotateX(30deg) rotateY(-45deg) rotateZ(60deg) scale3d(2, 0.5, 1.5)",
            "rotateY(170deg)",
            "perspective(500px) rotateX(20deg)",
            "skewX(20deg) scale(1.2)",
            "matrix(0.5, 0.3, -0.2, 1.1, 4, 5)",
        ] {
            let m = parse_transform(s).unwrap();
            let d = decompose(&m).unwrap();
            let r = recompose(&d);
            assert!(close(&m, &r, 1e-6), "{s}\n{m:?}\n{r:?}");
        }
    }

    #[test]
    fn interpolation_matches_rotation() {
        let a = parse_transform("rotateY(0deg)").unwrap();
        let b = parse_transform("rotateY(90deg)").unwrap();
        let mid = mix_matrix(&a, &b, 0.5);
        assert!(close(&mid, &rotate_y(45.0), 1e-9));
        let c = parse_transform("translateX(100px) scale(2)").unwrap();
        let mid = mix_matrix(&IDENTITY, &c, 0.5);
        assert!((mid[12] - 50.0).abs() < 1e-9 && (mid[0] - 1.5).abs() < 1e-9);
        // Quaternion helpers agree with matrices.
        assert!(close(&to_matrix(&quat_euler(0.0, 0.0, 30.0)), &rotate_z(30.0), 1e-9));
    }

    #[test]
    fn culls_points_in_front_of_the_near_plane() {
        let vp = mul(&perspective(40.0, 1.0, 10.0, 1000.0), &look_at([0.0, 0.0, 100.0], [0.0; 3], [0.0, 1.0, 0.0]));
        let mut p = Projector::default();
        // 95 is 5 units from the eye (inside the near plane of 10); 0 is fine.
        p.project(&[0.3, 0.2, 95.0, 0.0, 0.0, 0.0], 3, [0, 1, 2], 2, &IDENTITY, &vp, 100.0, 100.0, false);
        assert_eq!(p.out[3], 0.0);
        assert!(p.out[7] > 0.0);
    }

    #[test]
    fn projection_and_sorting() {
        let view = look_at([0.0, 0.0, 10.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let proj = perspective(60.0, 1.0, 0.1, 100.0);
        let vp = mul(&proj, &view);
        // Three points at different depths + one behind the camera.
        let pts: [f32; 12] = [0.0, 0.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0, -5.0, 0.0, 0.0, 20.0];
        let mut p = Projector::default();
        p.project(&pts, 3, [0, 1, 2], 4, &IDENTITY, &vp, 200.0, 200.0, true);
        assert!((p.out[0] - 100.0).abs() < 1e-4 && (p.out[1] - 100.0).abs() < 1e-4);
        assert_eq!(p.out[3 * 4 + 3], 0.0); // culled
        assert_eq!(&p.order[..], &[3, 2, 0, 1]); // far → near (culled first)
        assert!(p.out[1 * 4 + 3] > p.out[0 * 4 + 3]); // nearer → bigger
        // Sorting large random sets is correct.
        let n = 5000;
        let pts: Vec<f32> = (0..n * 3).map(|i| ((i * 7919) % 1000) as f32 / 100.0 - 5.0).collect();
        p.project(&pts, 3, [0, 1, 2], n, &IDENTITY, &vp, 200.0, 200.0, true);
        for w in p.order.windows(2) {
            assert!(p.out[w[0] as usize * 4 + 2] >= p.out[w[1] as usize * 4 + 2]);
        }
    }
}

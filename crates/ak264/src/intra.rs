//! Intra prediction (8.3): 16×16 luma, 4×4 luma, 8×8 chroma.

/// Neighbouring samples of a block.
pub struct Edges<const N: usize> {
    pub top: [u8; N],
    pub left: [u8; N],
    pub tl: u8,
    pub has_top: bool,
    pub has_left: bool,
    pub has_tl: bool,
}

#[inline(always)]
fn clip(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// I16 modes: 0 V, 1 H, 2 DC, 3 Plane.
pub fn pred16(e: &Edges<16>, mode: u8, out: &mut [u8; 256]) {
    match mode {
        0 => {
            for y in 0..16 {
                out[y * 16..y * 16 + 16].copy_from_slice(&e.top);
            }
        }
        1 => {
            for y in 0..16 {
                out[y * 16..y * 16 + 16].fill(e.left[y]);
            }
        }
        2 => {
            let st: u32 = e.top.iter().map(|&v| v as u32).sum();
            let sl: u32 = e.left.iter().map(|&v| v as u32).sum();
            let dc = match (e.has_top, e.has_left) {
                (true, true) => (st + sl + 16) >> 5,
                (true, false) => (st + 8) >> 4,
                (false, true) => (sl + 8) >> 4,
                _ => 128,
            };
            out.fill(dc as u8);
        }
        _ => {
            let (a, b, c) = plane_params(e);
            for y in 0..16 {
                let base = a + c * (y as i32 - 7) - 7 * b + 16;
                let row: &mut [u8; 16] = (&mut out[y * 16..y * 16 + 16]).try_into().unwrap();
                *row = crate::simd::plane_row16(base, b);
            }
        }
    }
}

/// Plane prediction parameters (a, b, c) of a 16×16 block (8.3.3.4).
#[inline]
fn plane_params(e: &Edges<16>) -> (i32, i32, i32) {
    {
        {
            let p = |x: i32, y: i32| -> i32 {
                if y < 0 {
                    if x < 0 {
                        e.tl as i32
                    } else {
                        e.top[x as usize] as i32
                    }
                } else {
                    e.left[y as usize] as i32
                }
            };
            let mut h = 0;
            let mut v = 0;
            for i in 0..8 {
                h += (i + 1) * (p(8 + i, -1) - p(6 - i, -1));
                v += (i + 1) * (p(-1, 8 + i) - p(-1, 6 - i));
            }
            let a = 16 * (p(-1, 15) + p(15, -1));
            let b = (5 * h + 32) >> 6;
            let c = (5 * v + 32) >> 6;
            (a, b, c)
        }
    }
}

/// SAD of `src` against the 16×16 prediction of `mode` without building it
/// (modes 0 V, 1 H, 2 DC; plane builds the prediction).
pub fn sad16_mode(e: &Edges<16>, mode: u8, src: &[u8; 256]) -> u32 {
    #[cfg(target_arch = "x86_64")]
    if mode == 3 {
        let (a, b, c) = plane_params(e);
        // SAFETY: SSE2 is baseline on x86_64; loads are inside `src`.
        unsafe {
            use std::arch::x86_64::*;
            // row y, column x: clip((a + b·(x − 7) + c·(y − 7) + 16) >> 5)
            let x0 = _mm_setr_epi16(0, 1, 2, 3, 4, 5, 6, 7);
            let bb = _mm_set1_epi16(b as i16);
            let start = (a - 7 * b - 7 * c + 16) as i16;
            let mut lo = _mm_add_epi16(_mm_set1_epi16(start), _mm_mullo_epi16(x0, bb));
            let mut hi = _mm_add_epi16(lo, _mm_slli_epi16::<3>(bb));
            let cc = _mm_set1_epi16(c as i16);
            let mut acc = _mm_setzero_si128();
            for y in 0..16 {
                let p = _mm_packus_epi16(_mm_srai_epi16::<5>(lo), _mm_srai_epi16::<5>(hi));
                let r = _mm_loadu_si128(src.as_ptr().add(y * 16) as *const __m128i);
                acc = _mm_add_epi64(acc, _mm_sad_epu8(r, p));
                lo = _mm_add_epi16(lo, cc);
                hi = _mm_add_epi16(hi, cc);
            }
            return (_mm_cvtsi128_si64(acc) + _mm_cvtsi128_si64(_mm_unpackhi_epi64(acc, acc))) as u32;
        }
    }
    #[cfg(target_arch = "x86_64")]
    if mode < 3 {
        // SAFETY: SSE2 is baseline on x86_64; every load is inside `src`/`e.top`.
        unsafe {
            use std::arch::x86_64::*;
            let row = |y: usize| _mm_loadu_si128(src.as_ptr().add(y * 16) as *const __m128i);
            let mut acc = _mm_setzero_si128();
            match mode {
                0 => {
                    let t = _mm_loadu_si128(e.top.as_ptr() as *const __m128i);
                    for y in 0..16 {
                        acc = _mm_add_epi64(acc, _mm_sad_epu8(row(y), t));
                    }
                }
                1 => {
                    for y in 0..16 {
                        acc = _mm_add_epi64(acc, _mm_sad_epu8(row(y), _mm_set1_epi8(e.left[y] as i8)));
                    }
                }
                _ => {
                    let z = _mm_setzero_si128();
                    let hsum = |p: *const u8| {
                        let v = _mm_sad_epu8(_mm_loadu_si128(p as *const __m128i), z);
                        (_mm_cvtsi128_si64(v) + _mm_cvtsi128_si64(_mm_unpackhi_epi64(v, v))) as u32
                    };
                    let (st, sl) = (hsum(e.top.as_ptr()), hsum(e.left.as_ptr()));
                    let dc = match (e.has_top, e.has_left) {
                        (true, true) => (st + sl + 16) >> 5,
                        (true, false) => (st + 8) >> 4,
                        (false, true) => (sl + 8) >> 4,
                        _ => 128,
                    };
                    let d = _mm_set1_epi8(dc as u8 as i8);
                    for y in 0..16 {
                        acc = _mm_add_epi64(acc, _mm_sad_epu8(row(y), d));
                    }
                }
            }
            return (_mm_cvtsi128_si64(acc) + _mm_cvtsi128_si64(_mm_unpackhi_epi64(acc, acc))) as u32;
        }
    }
    let mut p = [0u8; 256];
    pred16(e, mode, &mut p);
    crate::simd::sad16(src, 16, &p, 16)
}

/// Chroma 8×8 modes: 0 DC, 1 H, 2 V, 3 Plane.
pub fn pred_chroma(e: &Edges<8>, mode: u8, out: &mut [u8; 64]) {
    match mode {
        0 => {
            for by in 0..2 {
                for bx in 0..2 {
                    let st: u32 = e.top[bx * 4..bx * 4 + 4].iter().map(|&v| v as u32).sum();
                    let sl: u32 = e.left[by * 4..by * 4 + 4].iter().map(|&v| v as u32).sum();
                    let (t, l) = (e.has_top, e.has_left);
                    let dc = if (bx == 0 && by == 0) || (bx == 1 && by == 1) {
                        match (t, l) {
                            (true, true) => (st + sl + 4) >> 3,
                            (false, true) => (sl + 2) >> 2,
                            (true, false) => (st + 2) >> 2,
                            _ => 128,
                        }
                    } else if bx == 1 {
                        if t {
                            (st + 2) >> 2
                        } else if l {
                            (sl + 2) >> 2
                        } else {
                            128
                        }
                    } else if l {
                        (sl + 2) >> 2
                    } else if t {
                        (st + 2) >> 2
                    } else {
                        128
                    };
                    for y in 0..4 {
                        let o = (by * 4 + y) * 8 + bx * 4;
                        out[o..o + 4].fill(dc as u8);
                    }
                }
            }
        }
        1 => {
            for y in 0..8 {
                out[y * 8..y * 8 + 8].fill(e.left[y]);
            }
        }
        2 => {
            for y in 0..8 {
                out[y * 8..y * 8 + 8].copy_from_slice(&e.top);
            }
        }
        _ => {
            let p = |x: i32, y: i32| -> i32 {
                if y < 0 {
                    if x < 0 {
                        e.tl as i32
                    } else {
                        e.top[x as usize] as i32
                    }
                } else {
                    e.left[y as usize] as i32
                }
            };
            let mut h = 0;
            let mut v = 0;
            for i in 0..4 {
                h += (i + 1) * (p(4 + i, -1) - p(2 - i, -1));
                v += (i + 1) * (p(-1, 4 + i) - p(-1, 2 - i));
            }
            let a = 16 * (p(-1, 7) + p(7, -1));
            let b = (34 * h + 32) >> 6;
            let c = (34 * v + 32) >> 6;
            for y in 0..8 {
                for x in 0..8 {
                    out[y * 8 + x] = clip((a + b * (x as i32 - 3) + c * (y as i32 - 3) + 16) >> 5);
                }
            }
        }
    }
}

/// 4×4 edges: top has 8 samples (4 top + 4 top-right, already substituted when
/// top-right is unavailable), left 4.
pub struct Edges4 {
    pub top: [u8; 8],
    pub left: [u8; 4],
    pub tl: u8,
    pub has_top: bool,
    pub has_left: bool,
    pub has_tl: bool,
}

/// Which I4x4 modes are usable with the given neighbours.
pub fn mode4_ok(e: &Edges4, m: u8) -> bool {
    match m {
        0 | 3 | 7 => e.has_top,
        1 | 8 => e.has_left,
        2 => true,
        _ => e.has_top && e.has_left && e.has_tl,
    }
}

/// I4x4 prediction (9 modes). Output raster 4×4.
pub fn pred4(e: &Edges4, mode: u8, out: &mut [u8; 16]) {
    // edge layout: [l3 l2 l1 l0 tl t0 .. t7]
    let mut g = [0i32; 13];
    for y in 0..4 {
        g[3 - y] = e.left[y] as i32;
    }
    g[4] = e.tl as i32;
    for x in 0..8 {
        g[5 + x] = e.top[x] as i32;
    }
    let f2 = |i: usize| ((g[i - 1] + 2 * g[i] + g[i + 1] + 2) >> 2) as u8;
    let f1 = |i: usize| ((g[i] + g[i + 1] + 1) >> 1) as u8;
    match mode {
        0 => {
            for y in 0..4 {
                out[y * 4..y * 4 + 4].copy_from_slice(&e.top[..4]);
            }
        }
        1 => {
            for y in 0..4 {
                out[y * 4..y * 4 + 4].fill(e.left[y]);
            }
        }
        2 => {
            let st: u32 = e.top[..4].iter().map(|&v| v as u32).sum();
            let sl: u32 = e.left.iter().map(|&v| v as u32).sum();
            let dc = match (e.has_top, e.has_left) {
                (true, true) => (st + sl + 4) >> 3,
                (false, true) => (sl + 2) >> 2,
                (true, false) => (st + 2) >> 2,
                _ => 128,
            };
            out.fill(dc as u8);
        }
        3 => {
            // diagonal down-left: t index x+y+1 filtered; last uses (t6 + 3 t7 + 2) >> 2
            for y in 0..4 {
                for x in 0..4 {
                    out[y * 4 + x] = if x + y == 6 {
                        ((g[11] + 3 * g[12] + 2) >> 2) as u8
                    } else {
                        f2(5 + x + y + 1)
                    };
                }
            }
        }
        4 => {
            // diagonal down-right: centre at tl (index 4) shifted by x - y
            for y in 0..4 {
                for x in 0..4 {
                    out[y * 4 + x] = f2((4 + x as isize - y as isize) as usize);
                }
            }
        }
        5 => {
            for y in 0..4 {
                for x in 0..4 {
                    let z = 2 * x as i32 - y as i32;
                    let t = 4 + x as i32 - (y as i32 >> 1); // index of p[x-(y>>1)-1, -1]
                    out[y * 4 + x] = match z {
                        0 | 2 | 4 | 6 => f1(t as usize),
                        1 | 3 | 5 => f2(t as usize),
                        -1 => f2(4),
                        _ => f2((4 - y as i32 + 1) as usize),
                    };
                }
            }
        }
        6 => {
            for y in 0..4 {
                for x in 0..4 {
                    let z = 2 * y as i32 - x as i32;
                    let li = 4 - y as i32 + (x as i32 >> 1); // index of p[-1, y-(x>>1)-1]
                    out[y * 4 + x] = match z {
                        0 | 2 | 4 | 6 => f1((li - 1) as usize),
                        1 | 3 | 5 => f2(li as usize),
                        -1 => f2(4),
                        _ => f2((4 + x as i32 - 1) as usize),
                    };
                }
            }
        }
        7 => {
            for y in 0..4 {
                for x in 0..4 {
                    let i = 5 + x + (y >> 1);
                    out[y * 4 + x] = if y % 2 == 0 { f1(i) } else { f2(i + 1) };
                }
            }
        }
        _ => {
            for y in 0..4 {
                for x in 0..4 {
                    let z = x + 2 * y;
                    let li = 3 - (y + (x >> 1)) as isize; // index of p[-1, y+(x>>1)]
                    out[y * 4 + x] = match z {
                        0 | 2 | 4 => f1((li - 1) as usize),
                        1 | 3 => f2((li - 1) as usize),
                        5 => ((g[1] + 3 * g[0] + 2) >> 2) as u8,
                        _ => g[0] as u8,
                    };
                }
            }
        }
    }
}

/// Reference I4x4 prediction straight from the spec equations (tests only).
#[cfg(test)]
pub fn pred4_ref(e: &Edges4, mode: u8, out: &mut [u8; 16]) {
    // p(x,-1) for x in -1..8, p(-1,y) for y in -1..4
    let t = |x: i32| -> i32 {
        if x < 0 {
            e.tl as i32
        } else {
            e.top[x as usize] as i32
        }
    };
    let l = |y: i32| -> i32 {
        if y < 0 {
            e.tl as i32
        } else {
            e.left[y as usize] as i32
        }
    };
    for y in 0..4i32 {
        for x in 0..4i32 {
            let v = match mode {
                0 => t(x),
                1 => l(y),
                2 => {
                    let st: i32 = (0..4).map(t).sum();
                    let sl: i32 = (0..4).map(l).sum();
                    match (e.has_top, e.has_left) {
                        (true, true) => (st + sl + 4) >> 3,
                        (false, true) => (sl + 2) >> 2,
                        (true, false) => (st + 2) >> 2,
                        _ => 128,
                    }
                }
                3 => {
                    if x == 3 && y == 3 {
                        (t(6) + 3 * t(7) + 2) >> 2
                    } else {
                        (t(x + y) + 2 * t(x + y + 1) + t(x + y + 2) + 2) >> 2
                    }
                }
                4 => {
                    if x > y {
                        (t(x - y - 2) + 2 * t(x - y - 1) + t(x - y) + 2) >> 2
                    } else if x < y {
                        (l(y - x - 2) + 2 * l(y - x - 1) + l(y - x) + 2) >> 2
                    } else {
                        (t(0) + 2 * t(-1) + l(0) + 2) >> 2
                    }
                }
                5 => {
                    let z = 2 * x - y;
                    if z >= 0 && z % 2 == 0 {
                        (t(x - (y >> 1) - 1) + t(x - (y >> 1)) + 1) >> 1
                    } else if z >= 0 {
                        (t(x - (y >> 1) - 2) + 2 * t(x - (y >> 1) - 1) + t(x - (y >> 1)) + 2) >> 2
                    } else if z == -1 {
                        (l(0) + 2 * l(-1) + t(0) + 2) >> 2
                    } else {
                        (l(y - 1) + 2 * l(y - 2) + l(y - 3) + 2) >> 2
                    }
                }
                6 => {
                    let z = 2 * y - x;
                    if z >= 0 && z % 2 == 0 {
                        (l(y - (x >> 1) - 1) + l(y - (x >> 1)) + 1) >> 1
                    } else if z >= 0 {
                        (l(y - (x >> 1) - 2) + 2 * l(y - (x >> 1) - 1) + l(y - (x >> 1)) + 2) >> 2
                    } else if z == -1 {
                        (l(0) + 2 * l(-1) + t(0) + 2) >> 2
                    } else {
                        (t(x - 1) + 2 * t(x - 2) + t(x - 3) + 2) >> 2
                    }
                }
                7 => {
                    if y % 2 == 0 {
                        (t(x + (y >> 1)) + t(x + (y >> 1) + 1) + 1) >> 1
                    } else {
                        (t(x + (y >> 1)) + 2 * t(x + (y >> 1) + 1) + t(x + (y >> 1) + 2) + 2) >> 2
                    }
                }
                _ => {
                    let z = x + 2 * y;
                    if z < 5 && z % 2 == 0 {
                        (l(y + (x >> 1)) + l(y + (x >> 1) + 1) + 1) >> 1
                    } else if z < 5 {
                        (l(y + (x >> 1)) + 2 * l(y + (x >> 1) + 1) + l(y + (x >> 1) + 2) + 2) >> 2
                    } else if z == 5 {
                        (l(2) + 3 * l(3) + 2) >> 2
                    } else {
                        l(3)
                    }
                }
            };
            out[(y * 4 + x) as usize] = v as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sad16_mode_matches() {
        let src: [u8; 256] = core::array::from_fn(|i| (i * 37 % 251) as u8);
        let tops: [[u8; 16]; 3] = [core::array::from_fn(|i| (i * 13 + 7) as u8), core::array::from_fn(|i| if i < 8 { 0 } else { 255 }), core::array::from_fn(|i| if i < 8 { 255 } else { 0 })];
        for (k, (ht, hl)) in [(true, true), (true, false), (false, true), (false, false), (true, true), (true, true)].into_iter().enumerate() {
            let t = tops[k.saturating_sub(3).min(2)];
            let e = Edges { top: t, left: core::array::from_fn(|i| if k == 5 { if i < 8 { 0 } else { 255 } } else { (200 - i * 5) as u8 }), tl: if k == 4 { 255 } else { 90 }, has_top: ht, has_left: hl, has_tl: ht && hl };
            for m in 0..4 {
                let mut p = [0u8; 256];
                pred16(&e, m, &mut p);
                assert_eq!(sad16_mode(&e, m, &src), crate::simd::sad16(&src, 16, &p, 16));
            }
        }
    }

    #[test]
    fn pred4_matches_spec() {
        let mut seed = 1u32;
        for _ in 0..200 {
            let mut r = || {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 24) as u8
            };
            let e = Edges4 {
                top: core::array::from_fn(|_| r()),
                left: core::array::from_fn(|_| r()),
                tl: r(),
                has_top: true,
                has_left: true,
                has_tl: true,
            };
            for m in 0..9 {
                let (mut a, mut b) = ([0u8; 16], [0u8; 16]);
                pred4(&e, m, &mut a);
                pred4_ref(&e, m, &mut b);
                assert_eq!(a, b, "mode {m}");
            }
        }
    }
}

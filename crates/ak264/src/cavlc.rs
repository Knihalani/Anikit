//! CAVLC residual block coding (9.2).

use crate::bits::BitWriter;
use crate::vlc::*;

#[inline(always)]
fn nc_table(nc: i32) -> usize {
    match nc {
        -1 => 4,
        0 | 1 => 0,
        2 | 3 => 1,
        4..=7 => 2,
        _ => 3,
    }
}

/// Write one residual block. `coeffs` are levels in scan order (len = maxNumCoeff:
/// 16, 15 or 4). `nc` is the predicted count (-1 for chroma DC). Returns TotalCoeff.
pub fn write_block(w: &mut BitWriter, coeffs: &[i32], nc: i32) -> u32 {
    let max = coeffs.len();
    // Gather non-zero levels from highest frequency down.
    let mut levels = [0i32; 16];
    let mut runs = [0u32; 16];
    let mut n = 0usize;
    let mut last = None;
    for i in (0..max).rev() {
        if coeffs[i] != 0 {
            last = Some(i);
            break;
        }
    }
    let Some(last) = last else {
        let (c, l) = COEFF_TOKEN[nc_table(nc)][0][0];
        w.put(l as u32, c as u32);
        return 0;
    };
    let mut i = last as isize;
    while i >= 0 {
        let v = coeffs[i as usize];
        // count zeros below this coefficient (run before)
        let mut run = 0;
        let mut j = i - 1;
        while j >= 0 && coeffs[j as usize] == 0 {
            run += 1;
            j -= 1;
        }
        levels[n] = v;
        runs[n] = run;
        n += 1;
        i = j;
    }
    // total_zeros = zeros before the last (highest) coefficient = (last+1) - n
    let total_zeros = (last as u32 + 1) - n as u32;
    let total = n as u32;
    let mut t1 = 0u32;
    while (t1 as usize) < n && t1 < 3 && levels[t1 as usize].abs() == 1 {
        t1 += 1;
    }
    let (c, l) = COEFF_TOKEN[nc_table(nc)][total as usize][t1 as usize];
    w.put(l as u32, c as u32);
    for k in 0..t1 as usize {
        w.put(1, (levels[k] < 0) as u32);
    }
    let mut suffix_len: u32 = if total > 10 && t1 < 3 { 1 } else { 0 };
    for (k, &lv) in levels.iter().enumerate().take(n).skip(t1 as usize) {
        let mut code: i32 = if lv > 0 { 2 * lv - 2 } else { -2 * lv - 1 };
        if k == t1 as usize && t1 < 3 {
            code -= 2;
        }
        let code = code as u32;
        if suffix_len == 0 {
            if code < 14 {
                w.put(code + 1, 1);
            } else if code < 30 {
                w.put(15, 1); // prefix 14
                w.put(4, code - 14);
            } else {
                w.put(16, 1); // prefix 15
                w.put(12, (code - 30).min(4095));
            }
        } else if code < (15 << suffix_len) {
            w.put((code >> suffix_len) + 1, 1);
            w.put(suffix_len, code & ((1 << suffix_len) - 1));
        } else {
            w.put(16, 1);
            w.put(12, (code - (15 << suffix_len)).min(4095));
        }
        if suffix_len == 0 {
            suffix_len = 1;
        }
        if lv.unsigned_abs() > (3 << (suffix_len - 1)) && suffix_len < 6 {
            suffix_len += 1;
        }
    }
    if (total as usize) < max {
        let (c, l) = if nc == -1 {
            TOTAL_ZEROS_CDC[total as usize][total_zeros as usize]
        } else {
            TOTAL_ZEROS[total as usize][total_zeros as usize]
        };
        w.put(l as u32, c as u32);
    }
    let mut zl = total_zeros;
    for &run in runs.iter().take(n - 1) {
        if zl == 0 {
            break;
        }
        let (c, l) = RUN_BEFORE[zl.min(7) as usize][run as usize];
        w.put(l as u32, c as u32);
        zl -= run;
    }
    total
}

/// Largest level magnitude the escape code can carry at suffixLength 0 (Baseline).
pub const MAX_LEVEL: i32 = 2063;

//! Parsing and formatting helpers that avoid `core::fmt` for floats (which is
//! large in WebAssembly builds).

use alloc::string::String;
use alloc::vec::Vec;

/// A tiny forward-only byte cursor used by all parsers in the crate.
pub struct Cursor<'a> {
    pub s: &'a [u8],
    pub i: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(s: &'a str) -> Self {
        Cursor { s: s.as_bytes(), i: 0 }
    }
    #[inline]
    pub fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    #[inline]
    pub fn done(&self) -> bool {
        self.i >= self.s.len()
    }
    pub fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' || c == 0x0c {
                self.i += 1;
            } else {
                break;
            }
        }
    }
    /// Skip whitespace and at most one comma.
    #[inline(never)]
    pub fn skip_sep(&mut self) {
        self.skip_ws();
        if self.peek() == Some(b',') {
            self.i += 1;
            self.skip_ws();
        }
    }
    pub fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    /// Parse a floating point number (SVG/CSS grammar: sign, digits, `.`, exponent).
    #[inline(never)]
    pub fn number(&mut self) -> Option<f64> {
        let start = self.i;
        let s = self.s;
        let mut i = self.i;
        let mut neg = false;
        if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
            neg = s[i] == b'-';
            i += 1;
        }
        let mut mant: u64 = 0;
        let mut exp10: i32 = 0;
        let mut digits = 0;
        let mut sig = 0;
        while i < s.len() && s[i].is_ascii_digit() {
            if sig < 18 {
                mant = mant * 10 + (s[i] - b'0') as u64;
                if mant != 0 {
                    sig += 1;
                }
            } else {
                exp10 += 1;
            }
            i += 1;
            digits += 1;
        }
        if i < s.len() && s[i] == b'.' {
            i += 1;
            while i < s.len() && s[i].is_ascii_digit() {
                if sig < 18 {
                    mant = mant * 10 + (s[i] - b'0') as u64;
                    exp10 -= 1;
                    if mant != 0 {
                        sig += 1;
                    }
                }
                i += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            self.i = start;
            return None;
        }
        if i < s.len() && (s[i] == b'e' || s[i] == b'E') {
            let mut j = i + 1;
            let mut eneg = false;
            if j < s.len() && (s[j] == b'+' || s[j] == b'-') {
                eneg = s[j] == b'-';
                j += 1;
            }
            if j < s.len() && s[j].is_ascii_digit() {
                let mut e: i32 = 0;
                while j < s.len() && s[j].is_ascii_digit() {
                    e = (e * 10 + (s[j] - b'0') as i32).min(10_000);
                    j += 1;
                }
                exp10 += if eneg { -e } else { e };
                i = j;
            }
        }
        self.i = i;
        let v = mant as f64 * pow10(exp10);
        Some(if neg { -v } else { v })
    }
    /// Parse an ASCII identifier made of letters, digits, `-` and `_`.
    pub fn ident(&mut self) -> &'a str {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'-' || c == b'_' {
                self.i += 1;
            } else {
                break;
            }
        }
        core::str::from_utf8(&self.s[start..self.i]).unwrap_or("")
    }
}

fn pow10(e: i32) -> f64 {
    const P: [f64; 23] = [
        1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18,
        1e19, 1e20, 1e21, 1e22,
    ];
    if (0..=22).contains(&e) {
        P[e as usize]
    } else if (-22..0).contains(&e) {
        1.0 / P[(-e) as usize]
    } else {
        libm::pow(10.0, e as f64)
    }
}

/// Parse a full string as a number. Returns `None` on trailing garbage.
pub fn parse_f64(s: &str) -> Option<f64> {
    let mut c = Cursor::new(s.trim());
    let v = c.number()?;
    if c.done() {
        Some(v)
    } else {
        None
    }
}

/// Append `x` to `out` with at most `decimals` fractional digits, trailing
/// zeros stripped. Produces the shortest readable form used for CSS/SVG output.
pub fn push_num(out: &mut String, x: f64, decimals: u32) {
    if !x.is_finite() {
        out.push('0');
        return;
    }
    let scale = pow10(decimals as i32);
    let mut r = libm::round(x * scale);
    if r == 0.0 {
        out.push('0');
        return;
    }
    if r < 0.0 {
        out.push('-');
        r = -r;
    }
    if r >= 9.0e15 {
        // Out of exact-integer range: fall back to integer rendering of x.
        push_uint(out, libm::round(if x < 0.0 { -x } else { x }) as u64);
        return;
    }
    let r = r as u64;
    let div = scale as u64;
    let int = r / div;
    let mut frac = r % div;
    push_uint(out, int);
    if frac != 0 {
        out.push('.');
        let mut digits = decimals;
        while frac.is_multiple_of(10) {
            frac /= 10;
            digits -= 1;
        }
        let mut buf = [b'0'; 20];
        let mut n = digits as usize;
        let mut f = frac;
        while n > 0 {
            n -= 1;
            buf[n] = b'0' + (f % 10) as u8;
            f /= 10;
        }
        out.push_str(core::str::from_utf8(&buf[..digits as usize]).unwrap_or(""));
    }
}

pub fn push_uint(out: &mut String, mut v: u64) {
    let mut buf = [0u8; 20];
    let mut n = buf.len();
    if v == 0 {
        out.push('0');
        return;
    }
    while v > 0 {
        n -= 1;
        buf[n] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    out.push_str(core::str::from_utf8(&buf[n..]).unwrap_or(""));
}

pub fn fmt_num(x: f64, decimals: u32) -> String {
    let mut s = String::new();
    push_num(&mut s, x, decimals);
    s
}

/// Split the argument list of a CSS-like function call: `name(a, b c, d)`.
/// Returns `(name, args)` where args is `None` when there are no parentheses.
pub fn split_call(s: &str) -> (&str, Option<&str>) {
    let s = s.trim();
    if let Some(open) = s.find('(') {
        let close = s.rfind(')').unwrap_or(s.len());
        let close = if close < open { s.len() } else { close };
        (s[..open].trim(), Some(&s[open + 1..close]))
    } else {
        (s, None)
    }
}

/// Parse a comma/space separated list of numbers.
pub fn parse_numbers(s: &str) -> Vec<f64> {
    let mut c = Cursor::new(s);
    let mut out = Vec::new();
    loop {
        c.skip_sep();
        match c.number() {
            Some(v) => out.push(v),
            None => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numbers() {
        assert_eq!(parse_f64("1.5"), Some(1.5));
        assert_eq!(parse_f64("-.5e2"), Some(-50.0));
        assert_eq!(parse_f64("abc"), None);
        assert_eq!(parse_numbers("1,2 3-4.5.5"), [1.0, 2.0, 3.0, -4.5, 0.5]);
    }
    #[test]
    fn format() {
        assert_eq!(fmt_num(1.0, 3), "1");
        assert_eq!(fmt_num(-0.0001, 3), "0");
        assert_eq!(fmt_num(1.23456, 3), "1.235");
        assert_eq!(fmt_num(-12.5, 2), "-12.5");
        assert_eq!(fmt_num(0.05, 3), "0.05");
    }
}

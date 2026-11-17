//! Bit writer (RBSP) and NAL unit packing with emulation prevention.

/// MSB-first bit writer for RBSP payloads.
#[derive(Default, Clone)]
pub struct BitWriter {
    pub buf: Vec<u8>,
    acc: u64,
    n: u32, // bits in acc
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        self.buf.clear();
        self.acc = 0;
        self.n = 0;
    }
    /// Write the low `len` bits of `v` (len <= 32).
    #[inline(always)]
    pub fn put(&mut self, len: u32, v: u32) {
        debug_assert!(len <= 32);
        if len == 0 {
            return;
        }
        let v = (v as u64) & ((1u64 << len) - 1);
        self.acc = (self.acc << len) | v;
        self.n += len;
        if self.n >= 32 {
            self.n -= 32;
            let w = (self.acc >> self.n) as u32;
            self.buf.extend_from_slice(&w.to_be_bytes());
        }
    }
    #[inline(always)]
    pub fn bit(&mut self, b: bool) {
        self.put(1, b as u32);
    }
    /// Unsigned Exp-Golomb.
    #[inline(always)]
    pub fn ue(&mut self, v: u32) {
        let x = v as u64 + 1;
        let len = 64 - x.leading_zeros();
        // (len-1) zeros, then x in len bits
        let total = 2 * len - 1;
        if total <= 32 {
            self.put(total, x as u32);
        } else {
            self.put(len - 1, 0);
            self.put(len, x as u32);
        }
    }
    /// Signed Exp-Golomb.
    #[inline(always)]
    pub fn se(&mut self, v: i32) {
        let k = if v > 0 { 2 * v as u32 - 1 } else { (-2 * v) as u32 };
        self.ue(k);
    }
    pub fn bits_written(&self) -> usize {
        self.buf.len() * 8 + self.n as usize
    }
    pub fn byte_aligned(&self) -> bool {
        self.n.is_multiple_of(8)
    }
    /// rbsp_trailing_bits: a 1 then zeros to the byte boundary; flushes.
    pub fn trailing(&mut self) {
        self.put(1, 1);
        let pad = (8 - self.n % 8) % 8;
        self.put(pad, 0);
        self.flush();
    }
    /// Append whole bytes (the writer must be byte aligned).
    pub fn append_bytes(&mut self, b: &[u8]) {
        self.flush();
        assert_eq!(self.n, 0);
        self.buf.extend_from_slice(b);
    }
    fn flush(&mut self) {
        while self.n >= 8 {
            self.n -= 8;
            self.buf.push((self.acc >> self.n) as u8);
        }
    }
}

/// Bits used by ue(v).
#[inline(always)]
pub fn ue_len(v: u32) -> u32 {
    let x = v + 1;
    2 * (32 - x.leading_zeros()) - 1
}
#[inline(always)]
pub fn se_len(v: i32) -> u32 {
    ue_len(if v > 0 { 2 * v as u32 - 1 } else { (-2 * v) as u32 })
}

/// Append a NAL unit (4-byte start code, header, escaped payload) to `out`.
pub fn write_nal(out: &mut Vec<u8>, ref_idc: u8, nal_type: u8, rbsp: &[u8]) {
    out.reserve(rbsp.len() + rbsp.len() / 64 + 8);
    out.extend_from_slice(&[0, 0, 0, 1, (ref_idc << 5) | nal_type]);
    let mut zeros = 0u32;
    for &b in rbsp {
        if zeros >= 2 && b <= 3 {
            out.push(3);
            zeros = 0;
        }
        out.push(b);
        if b == 0 {
            zeros += 1
        } else {
            zeros = 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exp_golomb() {
        let mut w = BitWriter::new();
        for v in [0u32, 1, 2, 3, 7, 254] {
            w.ue(v);
        }
        w.trailing();
        // 1 010 011 00100 0001000 000000011111111 + stop
        assert_eq!(ue_len(254), 15);
        assert_eq!(w.buf[0], 0b1010_0110);
        assert_eq!(se_len(-3), ue_len(6));
    }
    #[test]
    fn emulation_prevention() {
        let mut o = vec![];
        write_nal(&mut o, 3, 7, &[0, 0, 1, 0, 0, 0, 0, 0, 3]);
        assert_eq!(&o[5..], &[0, 0, 3, 1, 0, 0, 3, 0, 0, 3, 0, 3]);
    }
}

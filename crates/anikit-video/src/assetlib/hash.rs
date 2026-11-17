//! MD5 (Poly Haven publishes it) and SHA-256 (what we pin for everything else),
//! small and dependency-free. Both stream: feed bytes with `update`.

pub struct Md5 {
    s: [u32; 4],
    buf: [u8; 64],
    n: usize,
    len: u64,
}

const MD5_K: [u32; 64] = {
    // floor(abs(sin(i + 1)) * 2^32), precomputed.
    [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1, 0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ]
};
const MD5_R: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21,
];

impl Default for Md5 {
    fn default() -> Self {
        Md5 { s: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476], buf: [0; 64], n: 0, len: 0 }
    }
}

impl Md5 {
    fn block(&mut self, b: &[u8; 64]) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            *w = u32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
        }
        let [mut a, mut bb, mut c, mut d] = self.s;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((bb & c) | (!bb & d), i),
                1 => ((d & bb) | (!d & c), (5 * i + 1) % 16),
                2 => (bb ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (bb | !d), (7 * i) % 16),
            };
            let t = d;
            d = c;
            c = bb;
            bb = bb.wrapping_add(a.wrapping_add(f).wrapping_add(MD5_K[i]).wrapping_add(m[g]).rotate_left(MD5_R[i]));
            a = t;
        }
        self.s = [self.s[0].wrapping_add(a), self.s[1].wrapping_add(bb), self.s[2].wrapping_add(c), self.s[3].wrapping_add(d)];
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.len += data.len() as u64;
        while !data.is_empty() {
            let k = (64 - self.n).min(data.len());
            self.buf[self.n..self.n + k].copy_from_slice(&data[..k]);
            self.n += k;
            data = &data[k..];
            if self.n == 64 {
                let b = self.buf;
                self.block(&b);
                self.n = 0;
            }
        }
    }

    pub fn hex(mut self) -> String {
        let bits = self.len.wrapping_mul(8);
        self.update(&[0x80]);
        while self.n != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_le_bytes());
        self.s.iter().flat_map(|w| w.to_le_bytes()).map(|b| format!("{b:02x}")).collect()
    }
}

pub struct Sha256 {
    s: [u32; 8],
    buf: [u8; 64],
    n: usize,
    len: u64,
}

const SHA_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Default for Sha256 {
    fn default() -> Self {
        Sha256 { s: [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19], buf: [0; 64], n: 0, len: 0 }
    }
}

impl Sha256 {
    fn block(&mut self, b: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut h = self.s;
        for i in 0..64 {
            let s1 = h[4].rotate_right(6) ^ h[4].rotate_right(11) ^ h[4].rotate_right(25);
            let ch = (h[4] & h[5]) ^ (!h[4] & h[6]);
            let t1 = h[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(SHA_K[i]).wrapping_add(w[i]);
            let s0 = h[0].rotate_right(2) ^ h[0].rotate_right(13) ^ h[0].rotate_right(22);
            let maj = (h[0] & h[1]) ^ (h[0] & h[2]) ^ (h[1] & h[2]);
            let t2 = s0.wrapping_add(maj);
            h = [t1.wrapping_add(t2), h[0], h[1], h[2], h[3].wrapping_add(t1), h[4], h[5], h[6]];
        }
        for (s, x) in self.s.iter_mut().zip(h) {
            *s = s.wrapping_add(x);
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.len += data.len() as u64;
        while !data.is_empty() {
            let k = (64 - self.n).min(data.len());
            self.buf[self.n..self.n + k].copy_from_slice(&data[..k]);
            self.n += k;
            data = &data[k..];
            if self.n == 64 {
                let b = self.buf;
                self.block(&b);
                self.n = 0;
            }
        }
    }

    pub fn hex(mut self) -> String {
        let bits = self.len.wrapping_mul(8);
        self.update(&[0x80]);
        while self.n != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        self.s.iter().flat_map(|w| w.to_be_bytes()).map(|b| format!("{b:02x}")).collect()
    }
}

/// Hash a file with either algorithm (`"md5"` or `"sha256"`).
pub fn file_hex(path: &std::path::Path, algo: &str) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 1 << 16];
    let (mut m, mut s) = (Md5::default(), Sha256::default());
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if algo == "md5" {
            m.update(&buf[..n]);
        } else {
            s.update(&buf[..n]);
        }
    }
    Ok(if algo == "md5" { m.hex() } else { s.hex() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        let md5 = |s: &[u8]| {
            let mut h = Md5::default();
            h.update(s);
            h.hex()
        };
        let sha = |s: &[u8]| {
            let mut h = Sha256::default();
            h.update(s);
            h.hex()
        };
        assert_eq!(md5(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5(b"The quick brown fox jumps over the lazy dog"), "9e107d9d372bb6826bd81d3542a419d6");
        assert_eq!(sha(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let long = vec![b'a'; 1000];
        assert_eq!(md5(&long), "cabe45dcc9ae5b66ba86600cca6b8ba8");
        assert_eq!(sha(&long), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
    }
}

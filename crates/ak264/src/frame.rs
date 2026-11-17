//! Frames and padded planes.

/// A borrowed planar YUV 4:2:0 frame (8-bit). Chroma planes are
/// `ceil(width/2) × ceil(height/2)`.
#[derive(Clone, Copy)]
pub struct YuvFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub y: &'a [u8],
    pub u: &'a [u8],
    pub v: &'a [u8],
    pub y_stride: usize,
    pub c_stride: usize,
}

impl<'a> YuvFrame<'a> {
    /// Tightly packed planes.
    pub fn new(width: u32, height: u32, y: &'a [u8], u: &'a [u8], v: &'a [u8]) -> Self {
        let cw = (width as usize).div_ceil(2);
        YuvFrame { width, height, y, u, v, y_stride: width as usize, c_stride: cw }
    }
}

/// An owned, tightly packed YUV 4:2:0 frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YuvBuffer {
    pub width: u32,
    pub height: u32,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

impl YuvBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let (w, h) = (width as usize, height as usize);
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        YuvBuffer { width, height, y: vec![16; w * h], u: vec![128; cw * ch], v: vec![128; cw * ch] }
    }
    pub fn frame(&self) -> YuvFrame<'_> {
        YuvFrame::new(self.width, self.height, &self.y, &self.u, &self.v)
    }
    /// Copy a frame of the same size (any strides) into this buffer.
    pub fn copy_from(&mut self, f: &YuvFrame) {
        let (w, h) = (self.width as usize, self.height as usize);
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        for y in 0..h {
            self.y[y * w..y * w + w].copy_from_slice(&f.y[y * f.y_stride..y * f.y_stride + w]);
        }
        for y in 0..ch {
            self.u[y * cw..y * cw + cw].copy_from_slice(&f.u[y * f.c_stride..y * f.c_stride + cw]);
            self.v[y * cw..y * cw + cw].copy_from_slice(&f.v[y * f.c_stride..y * f.c_stride + cw]);
        }
    }
    /// Concatenated planes (the layout of ffmpeg's `-pix_fmt yuv420p` rawvideo).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(self.y.len() * 3 / 2);
        o.extend_from_slice(&self.y);
        o.extend_from_slice(&self.u);
        o.extend_from_slice(&self.v);
        o
    }
}

/// A plane with a replicated border of `pad` samples on each side.
#[derive(Clone)]
pub(crate) struct Plane {
    pub data: Vec<u8>,
    pub stride: usize,
    pub pad: usize,
    pub w: usize,
    pub h: usize,
}

impl Plane {
    pub fn new(w: usize, h: usize, pad: usize) -> Self {
        let stride = (w + 2 * pad + 31) & !31;
        Plane { data: vec![0; stride * (h + 2 * pad)], stride, pad, w, h }
    }
    #[inline(always)]
    pub fn off(&self, x: isize, y: isize) -> usize {
        ((y + self.pad as isize) as usize) * self.stride + (x + self.pad as isize) as usize
    }
    /// Replicate edges into the border.
    pub fn extend(&mut self) {
        let (p, s, w, h) = (self.pad, self.stride, self.w, self.h);
        if p == 0 {
            return;
        }
        for y in 0..h {
            let r = (y + p) * s;
            let (l, rr) = (self.data[r + p], self.data[r + p + w - 1]);
            self.data[r..r + p].fill(l);
            self.data[r + p + w..r + 2 * p + w].fill(rr);
        }
        let row_len = w + 2 * p;
        for y in 0..p {
            let (src, dst) = (p * s, y * s);
            self.data.copy_within(src..src + row_len, dst);
            let (src, dst) = ((p + h - 1) * s, (p + h + y) * s);
            self.data.copy_within(src..src + row_len, dst);
        }
    }
    /// Copy a w×h region from a strided source and replicate it to fill the plane (no border).
    pub fn load(&mut self, src: &[u8], stride: usize, w: usize, h: usize) {
        let p = self.pad;
        for y in 0..self.h {
            let sy = y.min(h - 1);
            let d = (y + p) * self.stride + p;
            let row = &src[sy * stride..sy * stride + w];
            self.data[d..d + w].copy_from_slice(row);
            if self.w > w {
                let last = row[w - 1];
                self.data[d + w..d + self.w].fill(last);
            }
        }
    }
}

/// Three planes of a picture at macroblock-aligned size.
#[derive(Clone)]
pub(crate) struct Pic {
    pub y: Plane,
    pub u: Plane,
    pub v: Plane,
}

impl Pic {
    pub fn new(mbw: usize, mbh: usize, pad: usize) -> Self {
        Pic {
            y: Plane::new(mbw * 16, mbh * 16, pad),
            u: Plane::new(mbw * 8, mbh * 8, pad / 2),
            v: Plane::new(mbw * 8, mbh * 8, pad / 2),
        }
    }
    pub fn extend(&mut self) {
        self.y.extend();
        self.u.extend();
        self.v.extend();
    }
}

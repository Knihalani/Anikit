//! In-loop deblocking filter (8.7).

use crate::encoder::MbInfo;
use crate::frame::Pic;
use crate::transform::Quant;

pub(crate) fn deblock(_p: &mut Pic, _info: &[MbInfo], _mbw: usize, _mbh: usize, _q: &Quant, _slices: Vec<(usize, usize)>) {
    unimplemented!("deblocking")
}

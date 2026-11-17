//! The mixer (provisional: plain sum; replaced by the full chain).

use crate::render::{RenderOpts, Stem, Timings};
use crate::spec::Arrangement;

pub struct MixOut {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    pub stems: Vec<Stem>,
    pub report: serde_json::Value,
}

pub fn mix(a: &Arrangement, dry: Vec<(Vec<f32>, Vec<f32>)>, _rate: u32, frames: usize, _opts: &RenderOpts, _tm: &mut Timings) -> Result<MixOut, String> {
    let mut l = vec![0.0f32; frames];
    let mut r = vec![0.0f32; frames];
    for ((dl, dr), t) in dry.iter().zip(&a.tracks) {
        let g = 10f32.powf(t.mix.gain as f32 / 20.0) * 0.3;
        for i in 0..frames {
            l[i] += dl[i] * g;
            r[i] += dr[i] * g;
        }
    }
    Ok(MixOut { l, r, stems: Vec::new(), report: serde_json::json!({}) })
}

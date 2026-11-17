//! Mid/side stereo width with optional mono bass (side high-passed with a
//! 4th-order Linkwitz-Riley slope below `monoBelow`).

use serde::Deserialize;

use super::biquad::{Biquad, FilterKind};
use super::Processor;

/// Widener parameters.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct WidenerParams {
    /// Side gain: 0 = mono, 1 = unchanged, 2 = twice as wide.
    pub width: f32,
    /// Keep everything below this frequency (Hz) mono.
    pub mono_below: Option<f32>,
}

impl Default for WidenerParams {
    fn default() -> Self {
        WidenerParams { width: 1.0, mono_below: None }
    }
}

/// Mid/side width processor.
#[derive(Debug, Clone)]
pub struct Widener {
    width: f32,
    hp: Option<[Biquad; 2]>,
}

impl Widener {
    pub fn new(rate: f32, p: &WidenerParams) -> Widener {
        Widener {
            width: p.width.max(0.0),
            hp: p.mono_below.filter(|f| *f > 0.0).map(|f| {
                let b = Biquad::new(FilterKind::HighPass, rate, f, std::f32::consts::FRAC_1_SQRT_2, 0.0);
                [b.clone(), b]
            }),
        }
    }
}

impl Processor for Widener {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let w = self.width;
        let mut side = [0.0f32; 256];
        for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            let n = cl.len();
            for ((s, a), b) in side[..n].iter_mut().zip(cl.iter()).zip(cr.iter()) {
                *s = 0.5 * (a - b) * w;
            }
            if let Some(hp) = self.hp.as_mut() {
                hp[0].process_mono(&mut side[..n]);
                hp[1].process_mono(&mut side[..n]);
            }
            for ((s, a), b) in side[..n].iter().zip(cl.iter_mut()).zip(cr.iter_mut()) {
                let m = 0.5 * (*a + *b);
                *a = m + s;
                *b = m - s;
            }
        }
    }
    fn reset(&mut self) {
        if let Some(h) = self.hp.as_mut() {
            h[0].reset();
            h[1].reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_and_mono_bass() {
        let mut w = Widener::new(48_000.0, &WidenerParams { width: 0.0, mono_below: None });
        let mut l = vec![1.0f32, 0.5, -0.2];
        let mut r = vec![0.0f32, 0.5, 0.2];
        w.process(&mut l, &mut r);
        assert_eq!(l, r);
        let mut w = Widener::new(48_000.0, &WidenerParams { width: 1.0, mono_below: None });
        let mut l = vec![1.0f32, 0.5, -0.2];
        let mut r = vec![0.0f32, 0.5, 0.2];
        w.process(&mut l, &mut r);
        assert_eq!(l, vec![1.0, 0.5, -0.2]);
        // A 40 Hz side-only signal is removed with monoBelow 150.
        let mut w = Widener::new(48_000.0, &WidenerParams { width: 1.5, mono_below: Some(150.0) });
        let mut l: Vec<f32> = (0..48_000).map(|i| (2.0 * std::f32::consts::PI * 40.0 * i as f32 / 48_000.0).sin()).collect();
        let mut r: Vec<f32> = l.iter().map(|v| -v).collect();
        w.process(&mut l, &mut r);
        let pk = l[24_000..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(pk < 0.06, "{pk}");
    }
}

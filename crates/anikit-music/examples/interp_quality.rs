//! Interpolation quality: SINAD of pitched sine samples and aliasing
//! rejection, for the cubic Hermite and windowed-sinc kernels.
//!
//! ```text
//! cargo run --release -p anikit-music --example interp_quality
//! ```

use anikit_music::sf2::engine::*;
use anikit_music::sf2::interp::Interp;
use std::sync::Arc;

const OUT: f64 = 48_000.0;

fn render(interp: Interp, f0: f64, rate: u32, semis: f64) -> Vec<f32> {
    let n = rate as usize * 4;
    let data: Vec<f32> = (0..n).map(|i| (2.0 * std::f64::consts::PI * f0 * i as f64 / rate as f64).sin() as f32 * 0.5).collect();
    let p = OscParams {
        sample: Arc::new(SampleBuf::new(vec![data], rate)),
        start: 0,
        end: n as u32,
        loop_start: 0,
        loop_end: 0,
        loop_mode: LoopMode::None,
        ratio: 2f64.powf(semis / 12.0) * rate as f64 / OUT,
        rate: OUT as f32,
        atten: 0.0,
        gain: 1.0,
        pan: -1.0,
        vol_env: Env { delay: 0.0, attack: 0.0, hold: 100.0, decay: 1.0, sustain: 0.0, release: 1.0 },
        mod_env: Env::GATE,
        mod_env_pitch: 0.0,
        mod_env_fc: 0.0,
        mod_lfo: Lfo::default(),
        mod_lfo_pitch: 0.0,
        mod_lfo_fc: 0.0,
        mod_lfo_vol: 0.0,
        vib_lfo: Lfo::default(),
        vib_lfo_pitch: 0.0,
        filter: FilterKind::Off,
        fc: 13500.0,
        q: 0.707,
        exclusive: 0,
        interp,
    };
    let mut o = Osc::new(p);
    let frames = 48_000;
    let (mut l, mut r) = (vec![0f32; frames], vec![0f32; frames]);
    for k in 0..frames / 128 {
        o.render(&mut l[k * 128..k * 128 + 128], &mut r[k * 128..k * 128 + 128]);
    }
    l[4_800..].to_vec()
}

/// Least-squares fit of a sine at `f`: (fitted power, residual power).
fn fit(y: &[f32], f: f64) -> (f64, f64) {
    let w = 2.0 * std::f64::consts::PI * f / OUT;
    let (mut cc, mut ss, mut cs, mut yc, mut ys) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (i, &v) in y.iter().enumerate() {
        let (s, c) = (w * i as f64).sin_cos();
        cc += c * c;
        ss += s * s;
        cs += c * s;
        yc += v as f64 * c;
        ys += v as f64 * s;
    }
    let det = cc * ss - cs * cs;
    let a = (yc * ss - ys * cs) / det;
    let b = (ys * cc - yc * cs) / det;
    let mut sig = 0.0;
    let mut res = 0.0;
    for (i, &v) in y.iter().enumerate() {
        let (s, c) = (w * i as f64).sin_cos();
        let m = a * c + b * s;
        sig += m * m;
        res += (v as f64 - m).powi(2);
    }
    (sig, res)
}

fn main() {
    println!("SINAD (dB) of a 44.1 kHz sine sample resampled to 48 kHz (higher is better)");
    println!("{:<34} {:>10} {:>10}", "case", "cubic", "sinc");
    for (f0, semis) in [(440.0, 0.0), (1000.0, 7.0), (3000.0, 7.0), (5000.0, 7.0), (10_000.0, 7.0), (14_000.0, 7.0), (5000.0, -12.0), (16_000.0, 0.0)] {
        let fout = f0 * 2f64.powf(semis / 12.0);
        let mut row = format!("{:>6} Hz {:+3} st → {:>7.0} Hz", f0, semis, fout);
        for interp in [Interp::Cubic, Interp::Sinc] {
            let y = render(interp, f0, 44_100, semis);
            let (s, r) = fit(&y, fout);
            row += &format!(" {:>10.1}", 10.0 * (s / r).log10());
        }
        println!("{row}");
    }
    println!("\naliasing: tone transposed above the 24 kHz output Nyquist; level of what folds back, re the tone (lower is better)");
    for (f0, semis) in [(18_000.0, 5.0), (15_000.0, 12.0), (20_000.0, 7.0)] {
        let mut row = format!("{:>6} Hz {:+3} st → {:>7.0} Hz", f0, semis, f0 * 2f64.powf(semis / 12.0));
        for interp in [Interp::Cubic, Interp::Sinc] {
            let y = render(interp, f0, 44_100, semis);
            let p: f64 = y.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / y.len() as f64;
            row += &format!(" {:>10.1}", 10.0 * (p / 0.125).log10());
        }
        println!("{row}");
    }
}

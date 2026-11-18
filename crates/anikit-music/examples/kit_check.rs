//! Load an SFZ drum kit and report the peak, stereo correlation and onset delay of each GM drum key
//! at two velocities: `kit_check kit.sfz`.
fn main() {
use anikit_music::Instrument;
    let p = std::env::args().nth(1).unwrap();
    let t = std::time::Instant::now();
    let opt = anikit_music::sf2::SfzOptions { skip_missing: true, ..Default::default() };
    let inst = anikit_music::sf2::SfzInstrument::load_with(&p, opt).unwrap_or_else(|e| panic!("{e}"));
    println!("loaded in {:?}", t.elapsed());
    for key in [35, 36, 37, 38, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 53, 55, 57] {
        let mut line = format!("key {key}:");
        for vel in [0.3f32, 0.9] {
            let mut v = inst.voice(&anikit_music::NoteOn { key: key as f32, vel, rate: 48_000.0, seed: 1, rr: 0, len: 0.5 });
            let (mut l, mut r) = (vec![0.0f32; 48_000], vec![0.0f32; 48_000]);
            for (a, b) in l.chunks_mut(128).zip(r.chunks_mut(128)) {
                v.render(a, b);
            }
            let pk = l.iter().chain(&r).fold(0.0f32, |m, x| m.max(x.abs()));
            let corr = {
                let (mut ll, mut rr, mut lr) = (0f64, 0f64, 0f64);
                for (x, y) in l.iter().zip(&r) {
                    ll += (*x as f64).powi(2);
                    rr += (*y as f64).powi(2);
                    lr += *x as f64 * *y as f64;
                }
                lr / (ll * rr).sqrt().max(1e-12)
            };
            let onset = l.iter().zip(&r).position(|(a, b)| a.abs().max(b.abs()) > pk * 0.1).unwrap_or(0) as f32 / 48.0;
            line += &format!("  vel {vel}: peak {:6.1} dB corr {corr:.2} onset {onset:4.1} ms", 20.0 * pk.max(1e-9).log10());
        }
        println!("{line}");
    }
}

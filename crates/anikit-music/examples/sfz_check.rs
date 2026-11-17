//! Load an SFZ, play A4 and middle C at a few velocities, report pitch and level.
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let t = std::time::Instant::now();
    let inst = anikit_music::sf2::load_sfz(&p).unwrap_or_else(|e| panic!("{e}"));
    println!("loaded in {:?}", t.elapsed());
    for (key, vel) in [(69.0f32, 0.3f32), (69.0, 0.9), (60.0, 0.6), (48.0, 0.7), (40.0, 0.7), (33.0, 0.7), (76.0, 0.7)] {
        let mut v = inst.voice(&anikit_music::NoteOn { key, vel, rate: 48_000.0, seed: 1, rr: 0, len: 1.0 });
        let (mut l, mut r) = (vec![0.0f32; 48_000], vec![0.0f32; 48_000]);
        for (a, b) in l.chunks_mut(128).zip(r.chunks_mut(128)) {
            v.render(a, b);
        }
        // Zero-crossing pitch over 0.2–0.8 s.
        let s = &l[9600..38400];
        // Autocorrelation pitch (robust to harmonics): best lag in 20 Hz..2.5 kHz.
        let n = 4096.min(s.len() / 2);
        let ac = |lag: usize| -> f32 { (0..n).map(|i| s[i] * s[i + lag]).sum() };
        let e0 = ac(0).max(1e-12);
        let mut best = (0usize, 0f32);
        for lag in 19..2400 {
            let c = ac(lag) / e0;
            if c > best.1 * 1.02 || (best.0 == 0 && c > 0.0) {
                best = (lag, c);
            }
        }
        let f = if best.0 > 0 && best.1 > 0.3 { 48_000.0 / best.0 as f32 } else { 0.0 };
        let pk = l.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let want = 440.0 * ((key - 69.0) / 12.0).exp2();
        println!("key {key} vel {vel}: ~{f:.1} Hz (want {want:.1}), peak {:.1} dBFS", 20.0 * pk.max(1e-9).log10());
    }
}

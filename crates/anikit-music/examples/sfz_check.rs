//! Load an SFZ, play A4 and middle C at a few velocities, report pitch and level.
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let t = std::time::Instant::now();
    let inst = anikit_music::sf2::load_sfz(&p).unwrap_or_else(|e| panic!("{e}"));
    println!("loaded in {:?}", t.elapsed());
    for (key, vel) in [(69.0f32, 0.3f32), (69.0, 0.9), (60.0, 0.6), (36.0, 0.7), (96.0, 0.7)] {
        let mut v = inst.voice(&anikit_music::NoteOn { key, vel, rate: 48_000.0, seed: 1, rr: 0, len: 1.0 });
        let (mut l, mut r) = (vec![0.0f32; 48_000], vec![0.0f32; 48_000]);
        for (a, b) in l.chunks_mut(128).zip(r.chunks_mut(128)) {
            v.render(a, b);
        }
        // Zero-crossing pitch over 0.2–0.8 s.
        let s = &l[9600..38400];
        let zc = s.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        let f = zc as f32 / 0.6;
        let pk = l.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let want = 440.0 * ((key - 69.0) / 12.0).exp2();
        println!("key {key} vel {vel}: ~{f:.1} Hz (want {want:.1}), peak {:.1} dBFS", 20.0 * pk.max(1e-9).log10());
    }
}

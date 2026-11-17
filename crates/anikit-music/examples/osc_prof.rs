use anikit_music::sf2::bank;
use anikit_music::{NoteOn, Voice};
fn main() {
    let g = bank::gm();
    let prog: u16 = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(0);
    if std::env::var("CUBIC").is_ok() {
        g.set_interp(anikit_music::sf2::interp::Interp::Cubic);
    }
    let inst = g.instrument(0, prog).unwrap();
    g.preload_preset(0, prog);
    let mut best = f64::MAX;
    for _ in 0..7 {
        let mut voices: Vec<Box<dyn Voice>> = (0..16)
            .map(|i| inst.voice(&NoteOn { key: 48.0 + i as f32 * 2.0, vel: 0.8, rate: 48_000.0, seed: i, rr: 0, len: 9.0 }))
            .collect();
        let (mut l, mut r) = ([0f32; 128], [0f32; 128]);
        let t = std::time::Instant::now();
        let blocks = 48_000 * 3 / 128;
        let mut s = 0.0;
        for _ in 0..blocks {
            l.fill(0.0);
            r.fill(0.0);
            for v in voices.iter_mut() {
                v.render(&mut l, &mut r);
            }
            s += l[5];
        }
        std::hint::black_box(s);
        best = best.min(t.elapsed().as_secs_f64() / (blocks as f64 * 128.0 / 48_000.0));
    }
    println!("{:.2} ns/voice-frame, {:.0}x RT for 16 voices", best * 1e9 / (16.0 * 48_000.0), 1.0 / best);
}

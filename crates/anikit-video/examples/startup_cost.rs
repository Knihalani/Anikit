//! Time-to-first-frame pieces for a spec with music: decode and beat analysis.
//! `cargo run --release -p anikit-video --example startup_cost [audio file]`
use std::time::Instant;
fn main() {
    let p = std::env::args().nth(1).unwrap_or("examples/video/pulse/pulse.flac".into());
    for _ in 0..2 {
        let t = Instant::now();
        let pcm = anikit_video::audio::decode_file(std::path::Path::new(&p)).unwrap();
        let td = t.elapsed();
        let t = Instant::now();
        let b = anikit_video::audio::analyze_pcm(&pcm);
        let ta = t.elapsed();
        let t = Instant::now();
        let m = pcm.mono();
        let tm = t.elapsed();
        eprintln!("{p}: {:.1} s of audio at {} Hz; decode {td:?}, analyse {ta:?} ({} beats), mono {tm:?} ({} samples)", pcm.duration(), pcm.rate, b.beats.len(), m.len());
    }
}

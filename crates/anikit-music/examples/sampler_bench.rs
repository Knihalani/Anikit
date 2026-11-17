//! Sampler benchmark: bank load/decode times and render throughput.
//!
//! ```text
//! cargo run --release -p anikit-music --example sampler_bench
//! ```

use anikit_music::sf2::bank;
use anikit_music::sf2::interp::Interp;
use anikit_music::{Instrument, NoteOn, Voice, BLOCK};
use std::sync::Arc;
use std::time::Instant;

const RATE: f32 = 48_000.0;

/// Render `secs` of a part that keeps `poly` voices sounding: a new chord of
/// `chord` notes every `every` seconds, each note held `hold` seconds; the
/// oldest voices are killed when the count exceeds `poly`. Returns
/// (wall seconds, voice-seconds rendered).
fn part(inst: &Arc<dyn Instrument>, poly: usize, chord: usize, every: f32, hold: f32, secs: f32) -> (f64, f64) {
    let total = (secs * RATE) as usize;
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut voices: Vec<(Box<dyn Voice>, usize, bool)> = Vec::new(); // (voice, release frame, released)
    let mut next = 0usize;
    let mut n = 0u64;
    let mut voice_frames = 0u64;
    let keys = [48.0, 52.0, 55.0, 59.0, 60.0, 64.0, 67.0, 71.0, 62.0, 65.0, 69.0, 72.0, 74.0, 76.0, 79.0, 83.0];
    let t0 = Instant::now();
    let mut off = 0;
    let mut sink = 0f32;
    while off < total {
        if off >= next {
            for c in 0..chord {
                let key = keys[(c + (n as usize * 3)) % keys.len()] - 12.0 * ((n % 3) as f32 - 1.0).abs();
                let v = inst.voice(&NoteOn {
                    key,
                    vel: 0.55 + 0.4 * ((n as usize + c) % 5) as f32 / 4.0,
                    rate: RATE,
                    seed: n * 31 + c as u64,
                    rr: 0,
                    len: hold,
                });
                voices.push((v, off + (hold * RATE) as usize, false));
            }
            n += 1;
            next += (every * RATE) as usize;
            while voices.len() > poly {
                voices.remove(0);
            }
        }
        l.fill(0.0);
        r.fill(0.0);
        for (v, rel, released) in voices.iter_mut() {
            if !*released && off >= *rel {
                v.release();
                *released = true;
            }
        }
        voices.retain_mut(|(v, _, _)| v.render(&mut l, &mut r));
        voice_frames += (voices.len() * BLOCK) as u64;
        sink += l[0] + r[BLOCK - 1];
        off += BLOCK;
    }
    std::hint::black_box(sink);
    (t0.elapsed().as_secs_f64(), voice_frames as f64 / RATE as f64)
}

fn main() {
    let t = Instant::now();
    let g = bank::gm();
    let parse = t.elapsed();
    let t = Instant::now();
    let piano = g.instrument(0, 0).unwrap();
    g.preload_preset(0, 0);
    let piano_decode = t.elapsed();
    let t = Instant::now();
    g.preload();
    let all_par = t.elapsed();
    println!("bundled bank: {} presets", g.presets().len());
    println!("  parse (first gm() call): {:.1} ms", parse.as_secs_f64() * 1e3);
    println!("  decode piano samples (parallel): {:.1} ms", piano_decode.as_secs_f64() * 1e3);
    println!("  decode remaining samples (parallel, {} threads): {:.1} ms", rayon::current_num_threads(), all_par.as_secs_f64() * 1e3);
    // single-threaded full decode on a fresh copy
    let fresh = anikit_music::sf2::sampler::SoundFont::from_bytes(std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/GeneralUser-GS.sf3")).unwrap()).unwrap();
    let t = Instant::now();
    let mut i = 0;
    while fresh.sample(i).is_ok() {
        i += 1;
    }
    println!("  decode all {i} samples single-threaded: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);

    let strings = g.instrument(0, 48).unwrap();
    for interp in [Interp::Sinc, Interp::Cubic] {
        g.set_interp(interp);
        println!("\ninterpolation {interp:?}");
        // warm up
        part(&piano, 16, 4, 0.5, 0.45, 5.0);
        for (label, inst, poly, chord, every, hold, secs) in [
            ("16-voice piano part (4-note chords / 0.5 s)", &piano, 16usize, 4usize, 0.5f32, 1.5f32, 60.0f32),
            ("64-voice piano chords (8-note chords / 0.25 s)", &piano, 64, 8, 0.25, 2.0, 60.0),
            ("16-voice sustained strings", &strings, 16, 4, 1.0, 4.0, 60.0),
        ] {
            let (wall, vs) = part(inst, poly, chord, every, hold, secs);
            println!(
                "  {label}: {secs} s in {:.3} s → {:.0}× real time; avg {:.1} voices; {:.0} voice-seconds/s ({:.1} ns/voice-frame)",
                wall,
                secs as f64 / wall,
                vs / secs as f64,
                vs / wall,
                wall * 1e9 / (vs * RATE as f64)
            );
        }
    }
}

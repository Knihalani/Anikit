//! Voice rendering speed: seconds of audio one voice renders per wall-clock
//! second (× real time) for every physical/synth preset and drum kit, plus a
//! mixed 32-voice load.
//!
//! `cargo run --release -p anikit-music --example voice_bench`

use anikit_music::{physical, synth, Instrument, NoteOn, BLOCK, RATE};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

fn note(key: f32, vel: f32, seed: u64) -> NoteOn {
    NoteOn {
        key,
        vel,
        rate: RATE as f32,
        seed,
        rr: 0,
        len: f32::INFINITY,
    }
}

/// × real time for one voice over `secs` of audio: the best of ≈1 s runs
/// (to reject interference from other processes), each starting a new note
/// (note-on cost included) and restarting it whenever it ends.
fn bench(inst: &dyn Instrument, key: f32, secs: f32) -> f64 {
    let runs = (secs as usize).max(5);
    (0..runs)
        .map(|_| bench_once(inst, key, secs / runs as f32))
        .fold(0.0, f64::max)
}

fn bench_once(inst: &dyn Instrument, key: f32, secs: f32) -> f64 {
    let frames = (secs * RATE as f32) as usize;
    let mut l = [0.0f32; BLOCK];
    let mut r = [0.0f32; BLOCK];
    // Warm up.
    let mut v = inst.voice(&note(key, 0.8, 1));
    for _ in 0..200 {
        v.render(&mut l, &mut r);
    }
    let t = Instant::now();
    let mut v = inst.voice(&note(key, 0.8, 2));
    let mut done = 0;
    let mut seed = 3;
    while done < frames {
        if !v.render(&mut l, &mut r) {
            v = inst.voice(&note(key, 0.8, seed));
            seed += 1;
        }
        done += BLOCK;
    }
    black_box(&l);
    let el = t.elapsed().as_secs_f64();
    frames as f64 / RATE as f64 / el
}

fn main() {
    let secs = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20.0f32);
    // Optional name filter (substring), e.g. `voice_bench 20 pad`.
    let filter = std::env::args().nth(2).unwrap_or_default();
    println!("{:<16} {:>10}", "preset", "× RT/voice");
    let mut all: Vec<(String, Arc<dyn Instrument>, f32)> = Vec::new();
    for p in physical::PRESETS {
        all.push((
            p.to_string(),
            physical::preset(p).unwrap(),
            if p.contains("bass") { 40.0 } else { 60.0 },
        ));
    }
    for p in synth::PRESETS {
        all.push((
            p.to_string(),
            synth::preset(p).unwrap(),
            if p.contains("bass") { 40.0 } else { 60.0 },
        ));
    }
    for (name, inst, key) in all.iter().filter(|a| a.0.contains(&filter)) {
        println!("{:<16} {:>10.0}", name, bench(inst.as_ref(), *key, secs));
    }
    if !filter.is_empty() {
        return;
    }
    for k in synth::KITS {
        let inst = synth::kit(k).unwrap();
        let mut line = format!("kit {:<12}", k);
        for key in [36, 38, 39, 42, 46, 49] {
            line += &format!(
                " {key}:{:.0}",
                bench(inst.as_ref(), key as f32, secs * 0.25)
            );
        }
        println!("{line}");
    }

    // Mixed load: 32 voices from a spread of presets, re-triggered as they end.
    let mix: Vec<(Arc<dyn Instrument>, f32)> = [
        ("pad-warm", 60.0),
        ("pad-glass", 64.0),
        ("strings-synth", 67.0),
        ("supersaw", 72.0),
        ("epiano-fm", 60.0),
        ("bass-808", 36.0),
        ("lead-saw", 76.0),
        ("pluck", 69.0),
    ]
    .iter()
    .map(|(n, k)| (synth::preset(n).unwrap(), *k))
    .chain(
        [
            ("nylon-guitar", 52.0),
            ("steel-guitar", 59.0),
            ("marimba", 72.0),
            ("vibraphone", 65.0),
            ("kalimba", 74.0),
            ("harp", 55.0),
            ("church-bell", 48.0),
            ("electric-guitar", 64.0),
        ]
        .iter()
        .map(|(n, k)| (physical::preset(n).unwrap(), *k)),
    )
    .collect();
    let drums = synth::kit("909").unwrap();
    let frames = (secs * RATE as f32) as usize;
    let mut voices: Vec<Box<dyn anikit_music::Voice>> = Vec::new();
    let mut seed = 10;
    let spawn = |i: usize, seed: &mut u64| -> Box<dyn anikit_music::Voice> {
        *seed += 1;
        if i % 4 == 3 {
            drums.voice(&note([36.0, 38.0, 42.0, 46.0][i / 4 % 4], 0.8, *seed))
        } else {
            let (inst, k) = &mix[i % mix.len()];
            inst.voice(&note(*k + (i % 5) as f32, 0.7, *seed))
        }
    };
    for i in 0..32 {
        voices.push(spawn(i, &mut seed));
    }
    let mut l = [0.0f32; BLOCK];
    let mut r = [0.0f32; BLOCK];
    let t = Instant::now();
    let mut done = 0;
    while done < frames {
        l.fill(0.0);
        r.fill(0.0);
        for (i, v) in voices.iter_mut().enumerate() {
            if !v.render(&mut l, &mut r) {
                *v = spawn(i, &mut seed);
            }
        }
        done += BLOCK;
        black_box(&l);
    }
    let el = t.elapsed().as_secs_f64();
    println!(
        "mixed 32 voices: {:.1} × real time ({:.0} voice·× RT)",
        frames as f64 / RATE as f64 / el,
        32.0 * frames as f64 / RATE as f64 / el
    );
}

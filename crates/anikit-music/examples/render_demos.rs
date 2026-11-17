//! Render a short demo WAV for every physical / synth preset and drum kit,
//! and print sanity statistics (peak, pitch error, decay, brightness).
//!
//! `cargo run --release -p anikit-music --example render_demos -- <out dir>`

#[path = "../tests/support/mod.rs"]
mod support;

use anikit_music::{physical, synth, Instrument, RATE};
use support::*;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "demos".into());
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    println!(
        "{:<16} {:>7} {:>7} {:>8} {:>8} {:>8} {:>7}",
        "preset", "pk dB", "pk vel1", "A3 ¢", "A4 ¢", "end s", "hf%"
    );
    let mut all: Vec<(String, std::sync::Arc<dyn Instrument>)> = Vec::new();
    for p in physical::PRESETS {
        all.push((p.to_string(), physical::preset(p).unwrap()));
    }
    for p in synth::PRESETS {
        all.push((p.to_string(), synth::preset(p).unwrap()));
    }
    for (name, inst) in &all {
        let bass = name.contains("bass");
        let (ka, kb) = if bass { (45.0, 57.0) } else { (57.0, 69.0) };
        let ratio = partial_ratio(name);
        let mut cents = [0.0; 2];
        for (i, k) in [ka, kb].iter().enumerate() {
            cents[i] = measure_pitch(inst.as_ref(), *k, ratio);
        }
        let r = render(inst.as_ref(), &note(60.0, 0.8, 1), 6.0, 1.0);
        let r1 = render(inst.as_ref(), &note(60.0, 1.0, 1), 1.5, 1.0);
        let pk = peak(&r.l).max(peak(&r.r));
        let pk1 = peak(&r1.l).max(peak(&r1.r));
        let hf = energy_above(&mono(&r)[..RATE as usize], 4000.0) * 100.0;
        let end = r.ended.map(|e| e as f32 / RATE as f32).unwrap_or(f32::NAN);
        println!(
            "{:<16} {:>7.1} {:>7.1} {:>8.2} {:>8.2} {:>8.2} {:>7.2}",
            name,
            dbfs(pk),
            dbfs(pk1),
            cents[0],
            cents[1],
            end,
            hf
        );
        // Demo: a little phrase.
        let demo = phrase(inst.as_ref(), bass);
        write_wav(&dir.join(format!("{name}.wav")), &demo.0, &demo.1).unwrap();
    }
    for k in synth::KITS {
        let inst = synth::kit(k).unwrap();
        let mut line = format!("kit {:<10}", k);
        for key in [36, 38, 39, 42, 46, 49] {
            let r = render(inst.as_ref(), &note(key as f32, 0.8, 3), 6.0, 0.1);
            let pk = peak(&r.l).max(peak(&r.r));
            let end = r.ended.map(|e| e as f32 / RATE as f32).unwrap_or(f32::NAN);
            line += &format!(" {key}:{:.1}dB/{:.2}s", dbfs(pk), end);
        }
        println!("{line}");
        let demo = beat(inst.as_ref());
        write_wav(&dir.join(format!("kit-{k}.wav")), &demo.0, &demo.1).unwrap();
    }
}

/// Which partial carries the nominal pitch.
pub fn partial_ratio(name: &str) -> f64 {
    match name {
        "tubular-bells" => 2.0,
        "church-bell" => 2.0,
        _ => 1.0,
    }
}

fn mix_notes(
    inst: &dyn Instrument,
    notes: &[(f32, f32, f32, f32)],
    secs: f32,
) -> (Vec<f32>, Vec<f32>) {
    let total = (secs * RATE as f32) as usize;
    let mut l = vec![0.0f32; total];
    let mut r = vec![0.0f32; total];
    for (i, &(t, key, dur, vel)) in notes.iter().enumerate() {
        let start = (t * RATE as f32) as usize;
        let rr = render(inst, &note(key, vel, 100 + i as u64), secs - t, dur);
        for j in 0..rr.l.len().min(total - start) {
            l[start + j] += rr.l[j];
            r[start + j] += rr.r[j];
        }
    }
    (l, r)
}

fn phrase(inst: &dyn Instrument, bass: bool) -> (Vec<f32>, Vec<f32>) {
    let o = if bass { -24.0 } else { 0.0 };
    let mut n = Vec::new();
    // Arpeggio then a chord.
    for (i, k) in [60.0, 64.0, 67.0, 72.0, 76.0, 72.0, 67.0, 64.0]
        .iter()
        .enumerate()
    {
        n.push((
            i as f32 * 0.25,
            k + o,
            0.22,
            0.55 + 0.4 * ((i % 3) as f32 / 2.0),
        ));
    }
    for k in [57.0, 60.0, 64.0, 69.0] {
        n.push((2.2, k + o, 1.6, 0.75));
    }
    mix_notes(inst, &n, 6.0)
}

fn beat(inst: &dyn Instrument) -> (Vec<f32>, Vec<f32>) {
    let mut n = Vec::new();
    let s = 0.125;
    for i in 0..32 {
        let t = i as f32 * s;
        if i % 8 == 0 || i % 8 == 5 {
            n.push((t, 36.0, 0.1, 0.9));
        }
        if i % 8 == 4 {
            n.push((t, 38.0, 0.1, 0.85));
            n.push((t, 39.0, 0.1, 0.6));
        }
        n.push((
            t,
            if i % 4 == 2 { 46.0 } else { 42.0 },
            0.1,
            if i % 2 == 0 { 0.7 } else { 0.45 },
        ));
    }
    n.push((0.0, 49.0, 0.1, 0.7));
    n.push((3.0, 45.0, 0.1, 0.8));
    n.push((3.25, 48.0, 0.1, 0.8));
    n.push((3.5, 56.0, 0.1, 0.7));
    mix_notes(inst, &n, 6.0)
}

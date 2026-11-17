//! Load an SFZ (or a pack instrument name) and render a short phrase to a WAV.
//!
//! ```text
//! cargo run --release -p anikit-music --example sfz_play -- path/to/inst.sfz out.wav [lenient]
//! cargo run --release -p anikit-music --example sfz_play -- vsco-violins out.wav
//! ```

use anikit_music::sf2::sfz::{SfzInstrument, SfzOptions};
use anikit_music::{Instrument, NoteOn, Voice};
use std::sync::Arc;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (src, out) = (&a[0], &a[1]);
    let t = std::time::Instant::now();
    let inst: Arc<dyn Instrument> = if src.ends_with(".sfz") {
        let i = SfzInstrument::load_with(src, SfzOptions { skip_missing: a.get(2).is_some(), ..Default::default() }).unwrap_or_else(|e| panic!("{e}"));
        println!("{} regions, parsed in {:.1?}", i.regions(), t.elapsed());
        Arc::new(i)
    } else {
        anikit_music::sf2::packs::pack_instrument(src)
            .or_else(|| anikit_music::sf2::bank::by_name(src))
            .unwrap_or_else(|| panic!("unknown instrument {src}"))
    };
    let rate = 48_000usize;
    let notes = [(48.0, 0.0, 1.0), (55.0, 0.5, 1.0), (60.0, 1.0, 1.0), (64.0, 1.5, 1.5), (67.0, 2.0, 1.5), (72.0, 2.5, 1.5)];
    let total = rate * 6;
    let (mut l, mut r) = (vec![0f32; total], vec![0f32; total]);
    let mut voices: Vec<(Box<dyn Voice>, usize, bool)> = Vec::new();
    let mut next = 0;
    let t = std::time::Instant::now();
    for off in (0..total).step_by(128) {
        while next < notes.len() && (notes[next].1 * rate as f32) as usize <= off {
            let (k, _, len) = notes[next];
            let v = inst.voice(&NoteOn { key: k, vel: 0.5 + 0.08 * next as f32, rate: rate as f32, seed: next as u64, rr: next as u32, len });
            voices.push((v, off + (len * rate as f32) as usize, false));
            next += 1;
        }
        let end = (off + 128).min(total);
        for (v, rel, done) in voices.iter_mut() {
            if !*done && off >= *rel {
                v.release();
                *done = true;
            }
        }
        let (lo, ro) = (&mut l[off..end], &mut r[off..end]);
        voices.retain_mut(|(v, _, _)| v.render(lo, ro));
    }
    let peak = l.iter().chain(r.iter()).fold(0f32, |m, v| m.max(v.abs()));
    let rms = (l.iter().map(|v| (v * v) as f64).sum::<f64>() / total as f64).sqrt();
    println!("rendered 6 s in {:.1?} (incl. lazy sample decoding); peak {peak:.3}, rms {rms:.4}", t.elapsed());
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + total as u32 * 4).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    for v in [1u16, 2] {
        w.extend_from_slice(&v.to_le_bytes());
    }
    w.extend_from_slice(&(rate as u32).to_le_bytes());
    w.extend_from_slice(&(rate as u32 * 4).to_le_bytes());
    for v in [4u16, 16] {
        w.extend_from_slice(&v.to_le_bytes());
    }
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(total as u32 * 4).to_le_bytes());
    for i in 0..total {
        for s in [l[i], r[i]] {
            w.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
    }
    std::fs::write(out, w).unwrap();
}

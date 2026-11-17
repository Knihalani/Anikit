//! Composition statistics per style (for docs/video/music/DIAGNOSIS.md).
use anikit_music::{arrange, style, MusicSpec};
use std::collections::BTreeMap;

fn main() {
    if std::env::args().any(|a| a == "depth") {
        depth();
        return;
    }
    for s in style::STYLES {
        let spec = MusicSpec::from_json(&format!(r#"{{"style":"{}","length":"60s","seed":1}}"#, s.name)).unwrap();
        let a = style::compose(&spec, None).unwrap();
        let bpb = a.grid.beats_per_bar;
        // Harmony: reconstruct the chord list from the progression used.
        let prog = if a.key.is_minor() { s.prog_minor } else { s.prog_major };
        let chords: Vec<String> = prog.iter().map(|p| p.to_string()).collect();
        let ext = prog.iter().flat_map(|p| p.split_whitespace()).filter(|c| c.chars().any(|ch| ch.is_ascii_digit()) || c.contains("sus") || c.contains("add")).count();
        let tot = prog.iter().flat_map(|p| p.split_whitespace()).count();
        // Sections and layers.
        let secs: Vec<String> = a.sections.iter().map(|x| format!("{}:{}b@{:.2}", x.name, x.bars, x.energy)).collect();
        let mut per_track = Vec::new();
        for t in &a.tracks {
            let n = t.notes.len();
            if n == 0 { continue; }
            let vels: Vec<f32> = t.notes.iter().map(|x| x.vel).collect();
            let mean = vels.iter().sum::<f32>() / n as f32;
            let sd = (vels.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n as f32).sqrt();
            let lens: Vec<f64> = t.notes.iter().map(|x| x.len).collect();
            let distinct_len = { let mut l: Vec<i64> = lens.iter().map(|x| (x * 16.0).round() as i64).collect(); l.sort(); l.dedup(); l.len() };
            let keys: Vec<i32> = t.notes.iter().map(|x| x.key as i32).collect();
            let range = keys.iter().max().unwrap() - keys.iter().min().unwrap();
            // Repetition: fraction of bars whose note pattern equals the bar 4 bars earlier (exact copy).
            let mut bars: BTreeMap<i64, Vec<(i64, i32)>> = BTreeMap::new();
            for x in &t.notes { let b = (x.t / bpb).floor() as i64; bars.entry(b).or_default().push((((x.t - b as f64 * bpb) * 48.0).round() as i64, x.key as i32)); }
            let mut same = 0; let mut cmp = 0;
            for (b, v) in &bars { if let Some(p) = bars.get(&(b - 4)) { cmp += 1; if p == v { same += 1; } } }
            per_track.push(format!("{}[{} n, vel sd {:.2}, {} lengths, range {}st, 4-bar exact repeats {}/{}]", t.name, n, sd, distinct_len, range, same, cmp));
        }
        // Melody interval profile (lead-like track).
        let lead = a.tracks.iter().find(|t| ["lead", "melody", "right", "pluck", "bells", "glock"].contains(&t.role.as_str()));
        let mel = lead.map(|t| {
            let mut ns = t.notes.clone(); ns.sort_by(|a, b| a.t.total_cmp(&b.t));
            let iv: Vec<i32> = ns.windows(2).map(|w| (w[1].key - w[0].key) as i32).collect();
            let steps = iv.iter().filter(|i| i.abs() <= 2).count(); let leaps = iv.iter().filter(|i| i.abs() >= 5).count(); let rep = iv.iter().filter(|i| **i == 0).count();
            format!("melody {} notes: steps {:.0}% leaps {:.0}% repeats {:.0}%", ns.len(), 100.0 * steps as f32 / iv.len().max(1) as f32, 100.0 * leaps as f32 / iv.len().max(1) as f32, 100.0 * rep as f32 / iv.len().max(1) as f32)
        }).unwrap_or_else(|| "no melody".into());
        println!("## {} ({} bpm)\n  progressions {:?} — {}/{} chords extended/sus\n  sections {}\n  {}\n  {}\n", s.name, a.grid.bpm, chords, ext, tot, secs.join(" "), per_track.join("\n  "), mel);
        let _ = arrange::BASS_PATTERNS;
    }
}

#[allow(dead_code)]
pub fn depth() {
    let gm = anikit_music::sf2::gm();
    for (b, p, label) in [(0u16, 0u16, "piano"), (0, 4, "epiano"), (0, 48, "strings"), (0, 24, "nylon"), (0, 25, "steel"), (0, 33, "finger bass"), (0, 56, "trumpet"), (0, 60, "horn"), (0, 73, "flute"), (0, 40, "violin"), (128, 0, "std kit")] {
        let pr = gm.sf.presets.iter().find(|x| x.bank == b && x.program == p).unwrap();
        // velocity layers at middle C (or snare 38 for kits), samples total
        let k = if b == 128 { 38 } else { 60 };
        let mut layers: Vec<(u8, u8)> = pr.regions.iter().filter(|r| r.keys.0 <= k && k <= r.keys.1).map(|r| r.vels).collect();
        layers.sort(); layers.dedup();
        let mut samples: Vec<usize> = pr.regions.iter().map(|r| r.sample).collect();
        samples.sort(); samples.dedup();
        println!("{label:<12} '{}' regions {} distinct samples {} velocity ranges at key {k}: {:?}", pr.name, pr.regions.len(), samples.len(), layers);
    }
}

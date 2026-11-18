//! Sections, sound effects and the key range per track for a 60 s cue:
//! `key_range <style> [seed]` (range = min / 10th pct / median / 90th pct / max).
fn main() {
    let st = std::env::args().nth(1).unwrap_or_else(|| "pop".into());
    let seed = std::env::args().nth(2).unwrap_or_else(|| "1".into());
    let spec = anikit_music::MusicSpec::from_json(&format!(r#"{{"style":"{st}","length":"60s","seed":{seed}}}"#)).unwrap();
    let a = anikit_music::style::compose(&spec, None).unwrap();
    for s in &a.sections {
        println!("section {:<8} at {:6.2}s, {} bars, energy {:.2}", s.name, a.grid.secs(s.bar * a.grid.beats_per_bar), s.bars, s.energy);
    }
    for x in &a.sfx {
        println!("sfx {:<15} at {:6.2}s", x.name, x.at);
    }
    for t in &a.tracks {
        let mut k: Vec<i32> = t.notes.iter().map(|x| x.key as i32).collect();
        if k.is_empty() {
            continue;
        }
        k.sort();
        let q = |f: f32| k[((k.len() - 1) as f32 * f) as usize];
        println!("{:<10} {:<28} {:>3} {:>3} {:>3} {:>3} {:>3}", t.name, t.inst, k[0], q(0.1), q(0.5), q(0.9), k[k.len() - 1]);
    }
}

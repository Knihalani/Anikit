//! Which source (synth / sampled / physical) each style's parts use by default.
fn main() {
    for s in anikit_music::style::STYLES {
        let mut parts = vec![format!("drums={}[{}]", s.kit, anikit_music::instruments::source(s.kit))];
        for l in s.layers {
            parts.push(format!("{}={}[{}]", l.role, l.inst.split('|').next().unwrap(), anikit_music::instruments::source(l.inst)));
        }
        println!("{:<17} {}", s.name, parts.join("  "));
    }
}

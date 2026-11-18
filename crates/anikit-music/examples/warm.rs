//! Render a spec twice in one process and print both timings (cold vs warm
//! sample caches): `warm '<json>'`.
fn main() {
    let j = std::env::args().nth(1).unwrap();
    let spec = anikit_music::MusicSpec::from_json(&j).unwrap();
    for run in ["cold", "warm"] {
        let r = anikit_music::render(&spec, &Default::default()).unwrap();
        println!("{run}: {:?}", r.timings);
    }
}

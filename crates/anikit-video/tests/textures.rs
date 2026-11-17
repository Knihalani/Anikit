use anikit_video::eval::{Evaluator, Options};
use anikit_video::pipeline::textures::CompTextures;
use anikit_video::spec::parse_json;
use rayon::prelude::*;

const SRC: &str = r##"{ "fps": 30, "compositions": { "film": { "size": [64, 36], "duration": 2, "background": "#203040",
    "layers": [ { "shape": "circle", "width": 10, "height": 10, "fill": "#ffcc00",
                  "animate": [ { "from": { "x": 0 }, "to": { "x": 64 }, "duration": 1 } ] } ] } },
  "scenes": [ "x" ] }"##;

#[test]
fn deterministic_shared_capped_and_parallel() {
    let p = parse_json(SRC).unwrap();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    let comp = ev.composition("film").unwrap();
    let cache = CompTextures::new(usize::MAX);
    // 3,700 instances, instance i shows frame i (the second half of the film is static).
    let px: Vec<_> = (0..3700u32).into_par_iter().map(|i| cache.get(comp, (i % 60) as f32 / 30.0, (i % 2) as u8)).collect();
    let s = cache.stats();
    assert!((30..=34).contains(&s.renders), "{s:?}"); // 31 moving frames (lod = frame parity) + the held end, not 3,700
    assert_eq!(s.textures as u64, s.renders);
    // Same request → same pixels, in a fresh cache and in any order.
    let fresh = CompTextures::new(usize::MAX);
    for i in [3699u32, 17, 42, 1] {
        let a = &px[i as usize];
        let b = fresh.get(comp, (i % 60) as f32 / 30.0, (i % 2) as u8);
        assert_eq!(a.data_as_u8_slice(), b.data_as_u8_slice());
        assert_eq!((a.width() as u32, a.height() as u32), CompTextures::size(comp, (i % 2) as u8));
    }
    // Times inside one frame snap to it.
    assert!(std::sync::Arc::ptr_eq(&fresh.get(comp, 0.5, 0), &fresh.get(comp, 0.5 + 0.02, 0)));
    // A cap keeps memory bounded.
    let small = CompTextures::new(64 * 36 * 4 * 5);
    for i in 0..60 {
        small.get(comp, i as f32 / 30.0, 0);
    }
    assert!(small.stats().bytes <= 64 * 36 * 4 * 5, "{:?}", small.stats());
}

#[test]
fn components_reach_compositions_through_assets() {
    use anikit_video::components::Assets;
    let p = parse_json(SRC).unwrap();
    let ev = Evaluator::new(&p.spec, Options::default()).unwrap();
    let info = ev.assets.composition_info("film").unwrap();
    assert_eq!((info.width, info.height, info.duration, info.looped), (64, 36, 2.0, false));
    let a = ev.assets.composition_frame("film", 0.5, 1).unwrap();
    let b = CompTextures::new(usize::MAX).get(ev.composition("film").unwrap(), 0.5, 1);
    assert_eq!(a.data_as_u8_slice(), b.data_as_u8_slice());
    assert!(ev.assets.composition_info("nope").is_none());
}

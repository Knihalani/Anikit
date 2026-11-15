use anikit_core::color::ColorSpace;
use anikit_core::engine::{flags, Engine, TrackKind, TrackSpec};
use anikit_core::Timing;
fn main() {
    let mut e = Engine::new();
    let long = Timing { duration: 1e9, ..Default::default() };
    let ease = e.easing("inOutQuad").unwrap();
    for _ in 0..10_000 {
        let mut c = TrackSpec::number(vec![255.0, 0.0, 0.0, 1.0, 0.0, 0.0, 255.0, 1.0], 4).with_easings(vec![ease]);
        c.kind = TrackKind::Color(ColorSpace::Oklab);
        let h = e.create_anim(long, &[c], flags::DEFAULT).unwrap();
        e.play(h).unwrap();
    }
    for _ in 0..100 {
        e.tick(1.0);
    }
    println!("{}", e.updated().len());
}

//! Which components still change after their entrance has finished?
//! Prints kinds whose ops differ between consecutive frames at t = 4..5 s.
//! `cargo run --release -p anikit-video --example settle_check`
use anikit_video::components::{self, host::Host, Theme};

fn main() {
    let theme = Theme::default();
    let host = Host::bundled();
    for k in components::KINDS {
        let p = components::build(k.name, &serde_json::from_str(k.example).unwrap()).unwrap();
        let frames: Vec<_> = (120..150).map(|f| p.eval(f as f32 / 30.0, 1920.0, 1080.0, 30.0, &theme, &host)).collect();
        let changed = frames.windows(2).filter(|w| w[0] != w[1]).count();
        if changed > 0 {
            println!("{:16} changes in {changed}/29 frames", k.name);
        }
    }
}

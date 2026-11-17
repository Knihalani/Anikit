//! Print how far a domino cascade has spread over time (for framing shots).
//!
//! `cargo run --release -p anikit-video --example physics_front -- spec.json [seconds]`

use anikit_video::scene3d::physics;
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let spec: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let dur: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8.0);
    // The first scene3d layer with physics.
    let props = spec["scenes"].as_array().unwrap().iter().flat_map(|s| s["layers"].as_array().unwrap().iter()).find(|l| l.get("props").and_then(|p| p.get("physics")).is_some()).expect("no scene3d physics layer")["props"].clone();
    let props = match props {
        serde_json::Value::Object(m) => serde_json::Value::Object(anikit_video::components::snake_keys(m)),
        v => v,
    };
    let mut shapes = anikit_video::scene3d::spec::Shapes::new();
    let p = physics::parse(&props["physics"], &[], &mut shapes, &mut Vec::new(), &HashMap::new(), &[], Some(0.0)).unwrap();
    let t0 = std::time::Instant::now();
    let mut t = 0.0;
    while t <= dur + 1e-3 {
        println!("t {t:5.2}  front {:6.2} m", p.sim.front(t));
        t += 0.5;
    }
    let (steps, secs) = p.sim.run_to(dur);
    println!("{} bodies, {steps} steps, {secs:.1} simulated s, {:.1} s wall", p.sim.body_count(), t0.elapsed().as_secs_f64());
}

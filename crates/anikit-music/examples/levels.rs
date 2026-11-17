//! Print peak levels per preset across the keyboard (vel 0.8 and 1.0) and the
//! gain trim that would put the loudest forte note at −5 dBFS.
//!
//! `cargo run --release -p anikit-music --example levels`

#[path = "../tests/support/mod.rs"]
mod support;

use anikit_music::{physical, synth, Instrument};
use support::*;

fn main() {
    let mut all: Vec<(String, std::sync::Arc<dyn Instrument>)> = Vec::new();
    for p in physical::PRESETS {
        all.push((p.to_string(), physical::preset(p).unwrap()));
    }
    for p in synth::PRESETS {
        all.push((p.to_string(), synth::preset(p).unwrap()));
    }
    println!(
        "{:<16} {:>24} {:>8} {:>7}",
        "preset", "peak dB @ keys (vel .8)", "vel 1", "trim"
    );
    for (name, inst) in &all {
        let keys: [f32; 5] = if name.contains("bass") {
            [28.0, 40.0, 52.0, 64.0, 76.0]
        } else {
            [48.0, 60.0, 72.0, 84.0, 96.0]
        };
        let mut pk = Vec::new();
        for &k in &keys {
            let r = render(inst.as_ref(), &note(k, 0.8, 5), 2.0, 1.5);
            pk.push(dbfs(peak(&r.l).max(peak(&r.r))));
        }
        let r = render(inst.as_ref(), &note(keys[1], 1.0, 5), 2.0, 1.5);
        let p1 = dbfs(peak(&r.l).max(peak(&r.r)));
        let mx = pk.iter().cloned().fold(-200.0, f32::max);
        let trim = 10f32.powf((-5.0 - mx) / 20.0);
        println!(
            "{:<16} {:>5.1} {:>5.1} {:>5.1} {:>5.1} {:>5.1} {:>8.1} {:>7.3}",
            name, pk[0], pk[1], pk[2], pk[3], pk[4], p1, trim
        );
    }
    for k in synth::KITS {
        let inst = synth::kit(k).unwrap();
        let mut line = format!("kit {:<10}", k);
        for key in [
            24, 35, 36, 37, 38, 39, 41, 42, 46, 49, 51, 54, 56, 70, 75, 76,
        ] {
            let r = render(inst.as_ref(), &note(key as f32, 0.8, 3), 3.0, 0.1);
            line += &format!(" {key}:{:.0}", dbfs(peak(&r.l).max(peak(&r.r))));
        }
        println!("{line}");
    }
}
